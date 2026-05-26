use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse};

use crate::{virtio_device_error, VirtioError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciDeviceConfigSpec {
    bytes: Vec<u8>,
    writable: ByteMask,
}

impl VirtioPciDeviceConfigSpec {
    pub fn new(bytes: Vec<u8>, writable: ByteMask) -> Result<Self, VirtioError> {
        if bytes.is_empty() {
            return Err(VirtioError::EmptyDeviceConfig);
        }
        if bytes.len() as u64 != writable.len() {
            return Err(VirtioError::DeviceConfigWritableMaskSizeMismatch {
                bytes: bytes.len() as u64,
                mask: writable.len(),
            });
        }
        Ok(Self { bytes, writable })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn writable(&self) -> &ByteMask {
        &self.writable
    }
}

#[derive(Clone, Debug)]
pub struct VirtioPciDeviceConfigDevice {
    state: Arc<Mutex<VirtioPciDeviceConfigState>>,
}

impl VirtioPciDeviceConfigDevice {
    pub fn new(spec: VirtioPciDeviceConfigSpec) -> Self {
        Self {
            state: Arc::new(Mutex::new(VirtioPciDeviceConfigState {
                bytes: spec.bytes,
                writable: spec.writable,
                accesses: Vec::new(),
            })),
        }
    }

    pub fn range(&self) -> AddressRange {
        let state = self.state.lock().expect("virtio device config lock");
        AddressRange::new(
            Address::new(0),
            AccessSize::new(state.bytes.len() as u64).unwrap(),
        )
        .unwrap()
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.state
            .lock()
            .expect("virtio device config lock")
            .bytes
            .clone()
    }

    pub fn accesses(&self) -> Vec<VirtioPciDeviceConfigAccess> {
        self.state
            .lock()
            .expect("virtio device config lock")
            .accesses
            .clone()
    }

    pub fn snapshot(&self) -> VirtioPciDeviceConfigSnapshot {
        VirtioPciDeviceConfigSnapshot {
            state: self
                .state
                .lock()
                .expect("virtio device config lock")
                .clone(),
        }
    }

    pub fn restore(&self, snapshot: &VirtioPciDeviceConfigSnapshot) {
        *self.state.lock().expect("virtio device config lock") = snapshot.state.clone();
    }

    pub fn read_local(&self, address: Address, size: AccessSize) -> Result<Vec<u8>, MmioError> {
        self.read_at(MmioRequestId::new(0), address, size, 0)
    }

    pub fn write_local(
        &self,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
    ) -> Result<(), MmioError> {
        let size = AccessSize::new(data.len() as u64).map_err(MmioError::Memory)?;
        self.write_at(MmioRequestId::new(0), address, size, &data, &byte_mask, 0)
    }

    fn read_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
        tick: Tick,
    ) -> Result<Vec<u8>, MmioError> {
        let range = self.validate_access(request, address, size)?;
        let start = range.start().get() as usize;
        let end = range.end().get() as usize;
        let mut state = self.state.lock().expect("virtio device config lock");
        let data = state.bytes[start..end].to_vec();
        state.accesses.push(VirtioPciDeviceConfigAccess::read(
            tick,
            address,
            data.clone(),
        ));
        Ok(data)
    }

    fn write_at(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
        data: &[u8],
        byte_mask: &ByteMask,
        tick: Tick,
    ) -> Result<(), MmioError> {
        let range = self.validate_access(request, address, size)?;
        if data.len() as u64 != size.bytes() {
            return Err(MmioError::PayloadSizeMismatch {
                request,
                expected: size.bytes(),
                actual: data.len() as u64,
            });
        }
        if byte_mask.len() != size.bytes() {
            return Err(MmioError::ByteMaskSizeMismatch {
                request,
                expected: size.bytes(),
                actual: byte_mask.len(),
            });
        }

        let start = range.start().get() as usize;
        let end = range.end().get() as usize;
        let mut state = self.state.lock().expect("virtio device config lock");
        for (local, enabled) in byte_mask.bits().iter().copied().enumerate() {
            if enabled && !state.writable.bits()[start + local] {
                return Err(virtio_device_error(
                    request,
                    VirtioError::ReadOnlyDeviceConfigWrite {
                        offset: (start + local) as u64,
                    },
                ));
            }
        }

        if !byte_mask.bits().iter().any(|bit| *bit) {
            return Ok(());
        }

        let before = state.bytes[start..end].to_vec();
        for (local, enabled) in byte_mask.bits().iter().copied().enumerate() {
            if enabled {
                state.bytes[start + local] = data[local];
            }
        }
        let after = state.bytes[start..end].to_vec();
        state.accesses.push(VirtioPciDeviceConfigAccess::write(
            tick,
            address,
            data.to_vec(),
            byte_mask.clone(),
            before,
            after,
        ));
        Ok(())
    }

    fn validate_access(
        &self,
        request: MmioRequestId,
        address: Address,
        size: AccessSize,
    ) -> Result<AddressRange, MmioError> {
        let requested = AddressRange::new(address, size).map_err(MmioError::Memory)?;
        let device = self.range();
        if !device.contains_range(requested) {
            return Err(MmioError::DeviceBoundaryCrossed {
                request,
                device_start: device.start(),
                device_end: device.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }
        Ok(requested)
    }
}

impl MmioDevice for VirtioPciDeviceConfigDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    context.now(),
                )
                .map(|data| MmioResponse::completed(request.id(), Some(data))),
            MmioOperation::Write => {
                let data = request.data().ok_or(MmioError::MissingWriteData {
                    request: request.id(),
                })?;
                let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
                    request: request.id(),
                })?;
                self.write_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    data,
                    mask,
                    context.now(),
                )?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => self
                .read_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    context.now(),
                )
                .map(|data| MmioResponse::completed(request.id(), Some(data))),
            MmioOperation::Write => {
                let data = request.data().ok_or(MmioError::MissingWriteData {
                    request: request.id(),
                })?;
                let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
                    request: request.id(),
                })?;
                self.write_at(
                    request.id(),
                    request.range().start(),
                    request.size(),
                    data,
                    mask,
                    context.now(),
                )?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciDeviceConfigSnapshot {
    state: VirtioPciDeviceConfigState,
}

impl VirtioPciDeviceConfigSnapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.state.bytes
    }

    pub fn accesses(&self) -> Vec<VirtioPciDeviceConfigAccess> {
        self.state.accesses.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioPciDeviceConfigAccess {
    Read {
        tick: Tick,
        address: Address,
        data: Vec<u8>,
    },
    Write {
        tick: Tick,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
        before: Vec<u8>,
        after: Vec<u8>,
    },
}

impl VirtioPciDeviceConfigAccess {
    pub fn read(tick: Tick, address: Address, data: Vec<u8>) -> Self {
        Self::Read {
            tick,
            address,
            data,
        }
    }

    pub fn write(
        tick: Tick,
        address: Address,
        data: Vec<u8>,
        byte_mask: ByteMask,
        before: Vec<u8>,
        after: Vec<u8>,
    ) -> Self {
        Self::Write {
            tick,
            address,
            data,
            byte_mask,
            before,
            after,
        }
    }

    pub const fn tick(&self) -> Tick {
        match self {
            Self::Read { tick, .. } | Self::Write { tick, .. } => *tick,
        }
    }

    pub const fn address(&self) -> Address {
        match self {
            Self::Read { address, .. } | Self::Write { address, .. } => *address,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VirtioPciDeviceConfigState {
    bytes: Vec<u8>,
    writable: ByteMask,
    accesses: Vec<VirtioPciDeviceConfigAccess>,
}
