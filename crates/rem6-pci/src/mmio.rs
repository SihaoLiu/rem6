use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext};
use rem6_memory::AddressRange;
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse};

use crate::{PciError, PciHostBridge};

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
            let data = merged_write_data(host, request)?;
            if let Some(data) = data {
                host.write_config_address(request.range().start(), &data)
                    .map_err(|error| pci_device_error(request.id(), error))?;
            }
            Ok(MmioResponse::completed(request.id(), None))
        }
    }
}

fn merged_write_data(
    host: &PciHostBridge,
    request: &MmioRequest,
) -> Result<Option<Vec<u8>>, MmioError> {
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
        return Ok(None);
    }
    if mask.bits().iter().all(|bit| *bit) {
        return Ok(Some(payload.to_vec()));
    }

    let mut merged = host
        .read_config_address(request.range().start(), request.size())
        .map_err(|error| pci_device_error(request.id(), error))?;
    for (index, byte) in payload.iter().enumerate() {
        if mask.bits()[index] {
            merged[index] = *byte;
        }
    }
    Ok(Some(merged))
}

fn pci_device_error(request: MmioRequestId, error: PciError) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
