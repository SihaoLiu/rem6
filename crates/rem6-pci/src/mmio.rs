use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext};
use rem6_memory::{Address, AddressRange, ByteMask};
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse};

use crate::{PciError, PciHostBarRange, PciHostBridge};

#[derive(Clone, Debug)]
pub struct PciConfigMmioDevice {
    host: Arc<Mutex<PciHostBridge>>,
}

impl PciConfigMmioDevice {
    pub fn new(host: PciHostBridge) -> Self {
        Self::from_shared(Arc::new(Mutex::new(host)))
    }

    pub const fn from_shared(host: Arc<Mutex<PciHostBridge>>) -> Self {
        Self { host }
    }

    pub fn host(&self) -> Arc<Mutex<PciHostBridge>> {
        Arc::clone(&self.host)
    }

    pub fn config_range(&self) -> AddressRange {
        self.host
            .lock()
            .expect("PCI config host lock")
            .aperture()
            .range()
    }
}

impl MmioDevice for PciConfigMmioDevice {
    fn respond(
        &self,
        _context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let mut host = self.host.lock().expect("PCI config host lock");
        respond_config_mmio(&mut host, request)
    }

    fn respond_parallel(
        &self,
        _context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let mut host = self.host.lock().expect("PCI config host lock");
        respond_config_mmio(&mut host, request)
    }
}

pub struct PciBarMmioDevice<D> {
    host_bar_range: PciHostBarRange,
    local_range: AddressRange,
    inner: D,
}

impl<D> PciBarMmioDevice<D> {
    pub fn new(host_bar_range: PciHostBarRange, inner: D) -> Self {
        let local_range = AddressRange::new(Address::new(0), host_bar_range.host_range().size())
            .expect("PCI BAR local range mirrors an existing host BAR range");
        Self {
            host_bar_range,
            local_range,
            inner,
        }
    }

    pub const fn host_bar_range(&self) -> &PciHostBarRange {
        &self.host_bar_range
    }

    pub const fn host_range(&self) -> AddressRange {
        self.host_bar_range.host_range()
    }

    pub const fn local_range(&self) -> AddressRange {
        self.local_range
    }
}

impl<D> MmioDevice for PciBarMmioDevice<D>
where
    D: MmioDevice,
{
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let local_request = self.local_request(request)?;
        self.inner.respond(context, &local_request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let local_request = self.local_request(request)?;
        self.inner.respond_parallel(context, &local_request)
    }
}

impl<D> PciBarMmioDevice<D> {
    fn local_request(&self, request: &MmioRequest) -> Result<MmioRequest, MmioError> {
        let host_range = self.host_range();
        let requested = request.range();
        if !host_range.contains_range(requested) {
            return Err(MmioError::DeviceBoundaryCrossed {
                request: request.id(),
                device_start: host_range.start(),
                device_end: host_range.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }

        let offset = requested.start().get() - host_range.start().get();
        let local_address = Address::new(offset);
        match request.operation() {
            MmioOperation::Read => MmioRequest::read(request.id(), local_address, request.size()),
            MmioOperation::Write => {
                let data = request
                    .data()
                    .ok_or(MmioError::MissingWriteData {
                        request: request.id(),
                    })?
                    .to_vec();
                let byte_mask = request
                    .byte_mask()
                    .ok_or(MmioError::MissingByteMask {
                        request: request.id(),
                    })?
                    .clone();
                MmioRequest::write(request.id(), local_address, data, byte_mask)
            }
        }
    }
}

fn respond_config_mmio(
    host: &mut PciHostBridge,
    request: &MmioRequest,
) -> Result<MmioResponse, MmioError> {
    match request.operation() {
        MmioOperation::Read => host
            .read_config_address(request.range().start(), request.size())
            .map(|data| MmioResponse::completed(request.id(), Some(data)))
            .map_err(|error| pci_device_error(request.id(), error)),
        MmioOperation::Write => {
            write_masked_config(host, request)?;
            Ok(MmioResponse::completed(request.id(), None))
        }
    }
}

fn write_masked_config(host: &mut PciHostBridge, request: &MmioRequest) -> Result<(), MmioError> {
    let payload = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    if payload.len() as u64 != request.size().bytes() {
        return Err(MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: request.size().bytes(),
            actual: payload.len() as u64,
        });
    }
    let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
        request: request.id(),
    })?;
    if mask.len() != request.size().bytes() {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: request.size().bytes(),
            actual: mask.len(),
        });
    }
    if !mask.bits().iter().any(|bit| *bit) {
        return Ok(());
    }
    crate::access_size_from_len(payload.len())
        .map_err(|error| pci_device_error(request.id(), error))?;
    if mask.bits().iter().all(|bit| *bit) {
        host.write_config_address(request.range().start(), payload)
            .map_err(|error| pci_device_error(request.id(), error))?;
        return Ok(());
    }

    for chunk in masked_config_write_chunks(mask) {
        let start = request
            .range()
            .start()
            .get()
            .checked_add(chunk.start as u64)
            .map(Address::new)
            .ok_or(MmioError::AddressOverflow {
                address: request.range().start(),
                offset: chunk.start as u64,
            })?;
        host.write_config_address(start, &payload[chunk.start..chunk.end])
            .map_err(|error| pci_device_error(request.id(), error))?;
    }
    Ok(())
}

fn masked_config_write_chunks(mask: &ByteMask) -> Vec<std::ops::Range<usize>> {
    let mut chunks = Vec::new();
    let mut index = 0;
    while index < mask.len() as usize {
        if !mask.bits()[index] {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < mask.len() as usize && mask.bits()[end] {
            end += 1;
        }
        push_config_write_chunks(&mut chunks, index, end);
        index = end;
    }
    chunks
}

fn push_config_write_chunks(
    chunks: &mut Vec<std::ops::Range<usize>>,
    mut start: usize,
    end: usize,
) {
    while start < end {
        let remaining = end - start;
        let chunk_len = if remaining >= 4 {
            4
        } else if remaining >= 2 {
            2
        } else {
            1
        };
        chunks.push(start..start + chunk_len);
        start += chunk_len;
    }
}

fn pci_device_error(request: MmioRequestId, error: PciError) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
