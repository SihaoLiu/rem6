use std::error::Error;
use std::fmt;

use rem6_coherence::{ChiHarnessError, HarnessError, MesiHarnessError, MoesiHarnessError};
use rem6_kernel::Tick;
use rem6_memory::{
    Address, MemoryError, MemoryOperation, MemoryRequest, MemoryRequestId, MemoryTargetId,
};

use crate::RiscvDataCacheProtocol;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvDataCacheControllerError {
    Msi(HarnessError),
    Mesi(MesiHarnessError),
    Moesi(MoesiHarnessError),
    Chi(ChiHarnessError),
    MissingResponse { request: MemoryRequestId },
    Memory(MemoryError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataCacheControllerErrorRecord {
    tick: Tick,
    request_id: MemoryRequestId,
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    address: Address,
    line: Address,
    operation: MemoryOperation,
    error: RiscvDataCacheControllerError,
}

impl RiscvDataCacheControllerError {
    pub const fn missing_response(request: MemoryRequestId) -> Self {
        Self::MissingResponse { request }
    }
}

impl fmt::Display for RiscvDataCacheControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Msi(error) => write!(formatter, "{error}"),
            Self::Mesi(error) => write!(formatter, "{error}"),
            Self::Moesi(error) => write!(formatter, "{error}"),
            Self::Chi(error) => write!(formatter, "{error}"),
            Self::MissingResponse { request } => write!(
                formatter,
                "data-cache controller did not record response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvDataCacheControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Msi(error) => Some(error),
            Self::Mesi(error) => Some(error),
            Self::Moesi(error) => Some(error),
            Self::Chi(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::MissingResponse { .. } => None,
        }
    }
}

impl From<HarnessError> for RiscvDataCacheControllerError {
    fn from(error: HarnessError) -> Self {
        Self::Msi(error)
    }
}

impl From<MesiHarnessError> for RiscvDataCacheControllerError {
    fn from(error: MesiHarnessError) -> Self {
        Self::Mesi(error)
    }
}

impl From<MoesiHarnessError> for RiscvDataCacheControllerError {
    fn from(error: MoesiHarnessError) -> Self {
        Self::Moesi(error)
    }
}

impl From<ChiHarnessError> for RiscvDataCacheControllerError {
    fn from(error: ChiHarnessError) -> Self {
        Self::Chi(error)
    }
}

impl From<MemoryError> for RiscvDataCacheControllerError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl RiscvDataCacheControllerErrorRecord {
    pub(crate) fn from_request(
        tick: Tick,
        request: &MemoryRequest,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        error: RiscvDataCacheControllerError,
    ) -> Self {
        Self {
            tick,
            request_id: request.id(),
            protocol,
            target,
            address: request.range().start(),
            line: request.line_address(),
            operation: request.operation(),
            error,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn protocol(&self) -> RiscvDataCacheProtocol {
        self.protocol
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn operation(&self) -> MemoryOperation {
        self.operation
    }

    pub const fn error(&self) -> &RiscvDataCacheControllerError {
        &self.error
    }
}

impl fmt::Display for RiscvDataCacheControllerErrorRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "data-cache controller {:?} failed request {} from agent {} at tick {} for address {:#x}: {}",
            self.protocol,
            self.request_id.sequence(),
            self.request_id.agent().get(),
            self.tick,
            self.address.get(),
            self.error
        )
    }
}
