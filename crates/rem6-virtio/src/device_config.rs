use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse};

use crate::{virtio_device_error, VirtioError};

const VIRTIO_DEVICE_CONFIG_SNAPSHOT_MAGIC: &[u8; 8] = b"VIODCFG1";
const VIRTIO_DEVICE_CONFIG_SNAPSHOT_VERSION: u16 = 1;
const VIRTIO_DEVICE_CONFIG_ACCESS_HEADER_BYTES: usize = 25;
const U64_BYTES: usize = 8;

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

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(VIRTIO_DEVICE_CONFIG_SNAPSHOT_MAGIC);
        payload.extend_from_slice(&VIRTIO_DEVICE_CONFIG_SNAPSHOT_VERSION.to_le_bytes());
        write_bytes(&mut payload, &self.state.bytes);
        write_mask(&mut payload, &self.state.writable);
        payload.extend_from_slice(&(self.state.accesses.len() as u64).to_le_bytes());
        for access in &self.state.accesses {
            encode_access(&mut payload, access);
        }
        payload
    }

    pub fn from_bytes(payload: &[u8]) -> Result<Self, VirtioError> {
        let mut cursor = VirtioPciDeviceConfigSnapshotCursor::new(payload);
        cursor.read_magic()?;
        if cursor.read_u16()? != VIRTIO_DEVICE_CONFIG_SNAPSHOT_VERSION {
            return Err(VirtioError::InvalidDeviceConfigSnapshot);
        }
        let bytes = cursor.read_vec()?;
        if bytes.is_empty() {
            return Err(VirtioError::InvalidDeviceConfigSnapshot);
        }
        let writable = cursor.read_mask()?;
        if writable.len() != bytes.len() as u64 {
            return Err(VirtioError::InvalidDeviceConfigSnapshot);
        }
        let access_count = usize::try_from(cursor.read_u64()?)
            .map_err(|_| VirtioError::InvalidDeviceConfigSnapshot)?;
        if access_count > cursor.remaining() / VIRTIO_DEVICE_CONFIG_ACCESS_HEADER_BYTES {
            return Err(VirtioError::InvalidDeviceConfigSnapshot);
        }
        let mut accesses = Vec::with_capacity(access_count);
        for _ in 0..access_count {
            let access = cursor.read_access()?;
            validate_access_shape(bytes.len(), &access)?;
            accesses.push(access);
        }
        cursor.finish()?;
        Ok(Self {
            state: VirtioPciDeviceConfigState {
                bytes,
                writable,
                accesses,
            },
        })
    }
}

fn validate_access_shape(
    config_len: usize,
    access: &VirtioPciDeviceConfigAccess,
) -> Result<(), VirtioError> {
    let (address, len) = match access {
        VirtioPciDeviceConfigAccess::Read { address, data, .. }
        | VirtioPciDeviceConfigAccess::Write { address, data, .. } => (*address, data.len()),
    };
    if len == 0 {
        return Err(VirtioError::InvalidDeviceConfigSnapshot);
    }
    let start =
        usize::try_from(address.get()).map_err(|_| VirtioError::InvalidDeviceConfigSnapshot)?;
    let end = start
        .checked_add(len)
        .ok_or(VirtioError::InvalidDeviceConfigSnapshot)?;
    if end <= config_len {
        Ok(())
    } else {
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    }
}

fn encode_access(payload: &mut Vec<u8>, access: &VirtioPciDeviceConfigAccess) {
    match access {
        VirtioPciDeviceConfigAccess::Read {
            tick,
            address,
            data,
        } => {
            payload.push(0);
            payload.extend_from_slice(&tick.to_le_bytes());
            payload.extend_from_slice(&address.get().to_le_bytes());
            write_bytes(payload, data);
        }
        VirtioPciDeviceConfigAccess::Write {
            tick,
            address,
            data,
            byte_mask,
            before,
            after,
        } => {
            payload.push(1);
            payload.extend_from_slice(&tick.to_le_bytes());
            payload.extend_from_slice(&address.get().to_le_bytes());
            write_bytes(payload, data);
            write_mask(payload, byte_mask);
            write_bytes(payload, before);
            write_bytes(payload, after);
        }
    }
}

fn write_bytes(payload: &mut Vec<u8>, bytes: &[u8]) {
    payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    payload.extend_from_slice(bytes);
}

fn write_mask(payload: &mut Vec<u8>, mask: &ByteMask) {
    payload.extend_from_slice(&mask.len().to_le_bytes());
    for enabled in mask.bits() {
        payload.push(u8::from(*enabled));
    }
}

struct VirtioPciDeviceConfigSnapshotCursor<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> VirtioPciDeviceConfigSnapshotCursor<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }

    fn read_magic(&mut self) -> Result<(), VirtioError> {
        if self.read_exact(VIRTIO_DEVICE_CONFIG_SNAPSHOT_MAGIC.len())?
            == VIRTIO_DEVICE_CONFIG_SNAPSHOT_MAGIC
        {
            Ok(())
        } else {
            Err(VirtioError::InvalidDeviceConfigSnapshot)
        }
    }

    fn read_u8(&mut self) -> Result<u8, VirtioError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, VirtioError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes(
            bytes.try_into().expect("snapshot u16 width is fixed"),
        ))
    }

    fn read_u64(&mut self) -> Result<u64, VirtioError> {
        let bytes = self.read_exact(U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("snapshot u64 width is fixed"),
        ))
    }

    fn read_vec(&mut self) -> Result<Vec<u8>, VirtioError> {
        let len = usize::try_from(self.read_u64()?)
            .map_err(|_| VirtioError::InvalidDeviceConfigSnapshot)?;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_mask(&mut self) -> Result<ByteMask, VirtioError> {
        let len = usize::try_from(self.read_u64()?)
            .map_err(|_| VirtioError::InvalidDeviceConfigSnapshot)?;
        if len > self.remaining() {
            return Err(VirtioError::InvalidDeviceConfigSnapshot);
        }
        let mut bits = Vec::with_capacity(len);
        for _ in 0..len {
            bits.push(match self.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(VirtioError::InvalidDeviceConfigSnapshot),
            });
        }
        ByteMask::from_bits(bits).map_err(|_| VirtioError::InvalidDeviceConfigSnapshot)
    }

    fn read_access(&mut self) -> Result<VirtioPciDeviceConfigAccess, VirtioError> {
        let kind = self.read_u8()?;
        let tick = self.read_u64()?;
        let address = Address::new(self.read_u64()?);
        match kind {
            0 => Ok(VirtioPciDeviceConfigAccess::read(
                tick,
                address,
                self.read_vec()?,
            )),
            1 => {
                let data = self.read_vec()?;
                let byte_mask = self.read_mask()?;
                let before = self.read_vec()?;
                let after = self.read_vec()?;
                if byte_mask.len() != data.len() as u64
                    || before.len() != data.len()
                    || after.len() != data.len()
                {
                    return Err(VirtioError::InvalidDeviceConfigSnapshot);
                }
                Ok(VirtioPciDeviceConfigAccess::write(
                    tick, address, data, byte_mask, before, after,
                ))
            }
            _ => Err(VirtioError::InvalidDeviceConfigSnapshot),
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], VirtioError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(VirtioError::InvalidDeviceConfigSnapshot)?;
        let bytes = self
            .payload
            .get(self.offset..end)
            .ok_or(VirtioError::InvalidDeviceConfigSnapshot)?;
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), VirtioError> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(VirtioError::InvalidDeviceConfigSnapshot)
        }
    }

    fn remaining(&self) -> usize {
        self.payload.len() - self.offset
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
