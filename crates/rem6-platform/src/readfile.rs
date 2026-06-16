use std::error::Error;
use std::fmt;
use std::sync::Arc;

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext};
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_mmio::{
    MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse, MmioRoute,
};

const PLATFORM_READFILE_MAX_TRANSFER_BYTES: u64 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformReadfileConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformReadfileMmioDevice {
    range: AddressRange,
    payload: Arc<[u8]>,
}

impl PlatformReadfileMmioDevice {
    pub fn new(
        base: Address,
        size: AccessSize,
        payload: Vec<u8>,
    ) -> Result<Self, PlatformReadfileError> {
        let payload_bytes = payload.len() as u64;
        if payload_bytes > size.bytes() {
            return Err(PlatformReadfileError::PayloadExceedsWindow {
                payload_bytes,
                window_bytes: size.bytes(),
            });
        }

        Ok(Self {
            range: AddressRange::new(base, size).map_err(PlatformReadfileError::Memory)?,
            payload: payload.into(),
        })
    }

    pub const fn base(&self) -> Address {
        self.range.start()
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn respond_request(&self, request: &MmioRequest) -> Result<MmioResponse, MmioError> {
        let requested = request.range();
        if !self.range.contains_range(requested) {
            return Err(MmioError::DeviceBoundaryCrossed {
                request: request.id(),
                device_start: self.range.start(),
                device_end: self.range.end(),
                requested_start: requested.start(),
                requested_end: requested.end(),
            });
        }

        if request.operation() == MmioOperation::Write {
            return Err(MmioError::AccessDenied {
                request: request.id(),
                operation: request.operation(),
                access: MmioAccess::ReadOnly,
            });
        }

        let response_bytes = request.size().bytes();
        if response_bytes > PLATFORM_READFILE_MAX_TRANSFER_BYTES {
            return Err(MmioError::TransferTooLarge {
                request: request.id(),
                bytes: response_bytes,
                maximum: PLATFORM_READFILE_MAX_TRANSFER_BYTES,
            });
        }

        let offset = requested.start().get() - self.range.start().get();
        let response_len = usize_for_request(request, response_bytes, "response length")?;
        let mut data = vec![0; response_len];

        if offset < self.payload.len() as u64 {
            let offset = usize_for_request(request, offset, "offset")?;
            let available = self.payload.len() - offset;
            let copied = response_len.min(available);
            data[..copied].copy_from_slice(&self.payload[offset..offset + copied]);
        }

        Ok(MmioResponse::completed(request.id(), Some(data)))
    }
}

impl MmioDevice for PlatformReadfileMmioDevice {
    fn respond(
        &self,
        _context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_request(request)
    }

    fn respond_parallel(
        &self,
        _context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_request(request)
    }
}

fn usize_for_request(
    request: &MmioRequest,
    value: u64,
    field: &'static str,
) -> Result<usize, MmioError> {
    value.try_into().map_err(|_| MmioError::DeviceError {
        request: request.id(),
        message: format!("readfile {field} {value} exceeds host addressable size"),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformReadfileError {
    PayloadExceedsWindow {
        payload_bytes: u64,
        window_bytes: u64,
    },
    Memory(rem6_memory::MemoryError),
}

impl fmt::Display for PlatformReadfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadExceedsWindow {
                payload_bytes,
                window_bytes,
            } => write!(
                formatter,
                "readfile payload has {payload_bytes} bytes but MMIO window has {window_bytes}"
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlatformReadfileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}
