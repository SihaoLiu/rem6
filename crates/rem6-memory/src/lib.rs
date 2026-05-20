use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Address(u64);

impl Address {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccessSize(u64);

impl AccessSize {
    pub fn new(bytes: u64) -> Result<Self, MemoryError> {
        if bytes == 0 {
            return Err(MemoryError::ZeroAccessSize);
        }

        Ok(Self(bytes))
    }

    pub const fn bytes(self) -> u64 {
        self.0
    }

    fn as_usize(self) -> Result<usize, MemoryError> {
        self.0
            .try_into()
            .map_err(|_| MemoryError::AccessSizeTooLarge { size: self })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AgentId(u32);

impl AgentId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryRequestId {
    agent: AgentId,
    sequence: u64,
}

impl MemoryRequestId {
    pub const fn new(agent: AgentId, sequence: u64) -> Self {
        Self { agent, sequence }
    }

    pub const fn agent(self) -> AgentId {
        self.agent
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryOperation {
    InstructionFetch,
    ReadShared,
    ReadUnique,
    Write,
    Upgrade,
    Atomic,
    PrefetchRead,
    PrefetchWrite,
    WritebackClean,
    WritebackDirty,
    CleanEvict,
    Invalidate,
}

impl MemoryOperation {
    pub const fn coherence_intent(self) -> CoherenceIntent {
        match self {
            Self::InstructionFetch => CoherenceIntent::InstructionFetch,
            Self::ReadShared | Self::PrefetchRead => CoherenceIntent::ReadShared,
            Self::ReadUnique => CoherenceIntent::ReadUnique,
            Self::Write | Self::PrefetchWrite | Self::Atomic => CoherenceIntent::WriteUnique,
            Self::Upgrade => CoherenceIntent::Upgrade,
            Self::WritebackClean => CoherenceIntent::WritebackClean,
            Self::WritebackDirty => CoherenceIntent::WritebackDirty,
            Self::CleanEvict => CoherenceIntent::CleanEvict,
            Self::Invalidate => CoherenceIntent::Invalidate,
        }
    }

    pub const fn requires_response(self) -> bool {
        !matches!(
            self,
            Self::PrefetchRead
                | Self::PrefetchWrite
                | Self::WritebackClean
                | Self::WritebackDirty
                | Self::CleanEvict
        )
    }

    pub const fn returns_data(self) -> bool {
        matches!(
            self,
            Self::InstructionFetch | Self::ReadShared | Self::ReadUnique | Self::Atomic
        )
    }

    pub const fn carries_request_data(self) -> bool {
        matches!(
            self,
            Self::Write | Self::Atomic | Self::WritebackClean | Self::WritebackDirty
        )
    }

    pub const fn requires_writable(self) -> bool {
        matches!(
            self,
            Self::ReadUnique | Self::Write | Self::Upgrade | Self::Atomic | Self::PrefetchWrite
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoherenceIntent {
    InstructionFetch,
    ReadShared,
    ReadUnique,
    WriteUnique,
    Upgrade,
    WritebackClean,
    WritebackDirty,
    CleanEvict,
    Invalidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryError {
    ZeroAccessSize,
    AccessSizeTooLarge { size: AccessSize },
    AddressOverflow { start: Address, size: AccessSize },
    ZeroCacheLineSize,
    NonPowerOfTwoCacheLineSize { bytes: u64 },
    PayloadSizeMismatch { expected: AccessSize, actual: u64 },
    ByteMaskSizeMismatch { expected: AccessSize, actual: u64 },
    MissingRequestData { request: MemoryRequestId },
    UnexpectedRequestData { request: MemoryRequestId },
    MissingByteMask { request: MemoryRequestId },
    UnexpectedByteMask { request: MemoryRequestId },
    UnalignedLineAddress { address: Address, line_size: u64 },
    MissingResponseData { request: MemoryRequestId },
    UnexpectedResponseData { request: MemoryRequestId },
    ResponseNotExpected { request: MemoryRequestId },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroAccessSize => write!(formatter, "memory access size must be nonzero"),
            Self::AccessSizeTooLarge { size } => {
                write!(
                    formatter,
                    "access size {} does not fit host usize",
                    size.bytes()
                )
            }
            Self::AddressOverflow { start, size } => write!(
                formatter,
                "address {:#x} overflows for {} bytes",
                start.get(),
                size.bytes()
            ),
            Self::ZeroCacheLineSize => write!(formatter, "cache line size must be nonzero"),
            Self::NonPowerOfTwoCacheLineSize { bytes } => {
                write!(formatter, "cache line size {bytes} is not a power of two")
            }
            Self::PayloadSizeMismatch { expected, actual } => write!(
                formatter,
                "payload has {actual} bytes but request expects {}",
                expected.bytes()
            ),
            Self::ByteMaskSizeMismatch { expected, actual } => write!(
                formatter,
                "byte mask has {actual} bits but request expects {}",
                expected.bytes()
            ),
            Self::MissingRequestData { request } => write!(
                formatter,
                "request {} from agent {} requires payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedRequestData { request } => write!(
                formatter,
                "request {} from agent {} must not carry payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingByteMask { request } => write!(
                formatter,
                "request {} from agent {} requires a byte mask",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedByteMask { request } => write!(
                formatter,
                "request {} from agent {} must not carry a byte mask",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnalignedLineAddress { address, line_size } => write!(
                formatter,
                "address {:#x} is not aligned to cache line size {line_size}",
                address.get()
            ),
            Self::MissingResponseData { request } => write!(
                formatter,
                "response to request {} from agent {} requires payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedResponseData { request } => write!(
                formatter,
                "response to request {} from agent {} must not carry payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::ResponseNotExpected { request } => write!(
                formatter,
                "request {} from agent {} does not expect a response",
                request.sequence(),
                request.agent().get()
            ),
        }
    }
}

impl Error for MemoryError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheLineLayout {
    bytes: u64,
}

impl CacheLineLayout {
    pub fn new(bytes: u64) -> Result<Self, MemoryError> {
        if bytes == 0 {
            return Err(MemoryError::ZeroCacheLineSize);
        }
        if !bytes.is_power_of_two() {
            return Err(MemoryError::NonPowerOfTwoCacheLineSize { bytes });
        }

        Ok(Self { bytes })
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub fn line_address(self, address: Address) -> Address {
        Address::new(address.get() & !(self.bytes - 1))
    }

    pub fn line_offset(self, address: Address) -> u64 {
        address.get() - self.line_address(address).get()
    }

    fn line_span(self, range: AddressRange) -> u64 {
        let first = self.line_address(range.start()).get();
        let last = self.line_address(Address::new(range.end().get() - 1)).get();
        ((last - first) / self.bytes) + 1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressRange {
    start: Address,
    size: AccessSize,
    end: Address,
}

impl AddressRange {
    pub fn new(start: Address, size: AccessSize) -> Result<Self, MemoryError> {
        let end = start
            .get()
            .checked_add(size.bytes())
            .map(Address::new)
            .ok_or(MemoryError::AddressOverflow { start, size })?;

        Ok(Self { start, size, end })
    }

    pub const fn start(self) -> Address {
        self.start
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }

    pub const fn end(self) -> Address {
        self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteMask {
    bits: Vec<bool>,
}

impl ByteMask {
    pub fn full(size: AccessSize) -> Result<Self, MemoryError> {
        Ok(Self {
            bits: vec![true; size.as_usize()?],
        })
    }

    pub fn from_bits(bits: Vec<bool>) -> Result<Self, MemoryError> {
        if bits.is_empty() {
            return Err(MemoryError::ZeroAccessSize);
        }

        Ok(Self { bits })
    }

    pub fn bits(&self) -> &[bool] {
        &self.bits
    }

    pub fn len(&self) -> u64 {
        self.bits.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRequest {
    id: MemoryRequestId,
    operation: MemoryOperation,
    range: AddressRange,
    line_layout: CacheLineLayout,
    data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
}

impl MemoryRequest {
    pub fn read_shared(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::ReadShared,
            address,
            size,
            line_layout,
            None,
            None,
        )
    }

    pub fn read_unique(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::ReadUnique,
            address,
            size,
            line_layout,
            None,
            None,
        )
    }

    pub fn instruction_fetch(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::InstructionFetch,
            address,
            size,
            line_layout,
            None,
            None,
        )
    }

    pub fn write(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        data: Vec<u8>,
        byte_mask: ByteMask,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::Write,
            address,
            size,
            line_layout,
            Some(data),
            Some(byte_mask),
        )
    }

    pub fn upgrade(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::Upgrade,
            address,
            size,
            line_layout,
            None,
            None,
        )
    }

    pub fn writeback_dirty(
        id: MemoryRequestId,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::writeback(
            id,
            MemoryOperation::WritebackDirty,
            address,
            data,
            line_layout,
        )
    }

    pub fn writeback_clean(
        id: MemoryRequestId,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::writeback(
            id,
            MemoryOperation::WritebackClean,
            address,
            data,
            line_layout,
        )
    }

    fn writeback(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        if line_layout.line_offset(address) != 0 {
            return Err(MemoryError::UnalignedLineAddress {
                address,
                line_size: line_layout.bytes(),
            });
        }

        let size = AccessSize::new(line_layout.bytes())?;
        Self::new(id, operation, address, size, line_layout, Some(data), None)
    }

    fn new(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
        data: Option<Vec<u8>>,
        byte_mask: Option<ByteMask>,
    ) -> Result<Self, MemoryError> {
        let range = AddressRange::new(address, size)?;
        Self::validate_payload(id, operation, size, data.as_deref())?;
        Self::validate_mask(id, operation, size, byte_mask.as_ref())?;

        Ok(Self {
            id,
            operation,
            range,
            line_layout,
            data,
            byte_mask,
        })
    }

    fn validate_payload(
        id: MemoryRequestId,
        operation: MemoryOperation,
        size: AccessSize,
        data: Option<&[u8]>,
    ) -> Result<(), MemoryError> {
        match (operation.carries_request_data(), data) {
            (true, Some(bytes)) if bytes.len() as u64 == size.bytes() => Ok(()),
            (true, Some(bytes)) => Err(MemoryError::PayloadSizeMismatch {
                expected: size,
                actual: bytes.len() as u64,
            }),
            (true, None) => Err(MemoryError::MissingRequestData { request: id }),
            (false, Some(_)) => Err(MemoryError::UnexpectedRequestData { request: id }),
            (false, None) => Ok(()),
        }
    }

    fn validate_mask(
        id: MemoryRequestId,
        operation: MemoryOperation,
        size: AccessSize,
        byte_mask: Option<&ByteMask>,
    ) -> Result<(), MemoryError> {
        match (operation, byte_mask) {
            (MemoryOperation::Write, Some(mask)) if mask.len() == size.bytes() => Ok(()),
            (MemoryOperation::Write, Some(mask)) => Err(MemoryError::ByteMaskSizeMismatch {
                expected: size,
                actual: mask.len(),
            }),
            (MemoryOperation::Write, None) => Err(MemoryError::MissingByteMask { request: id }),
            (_, Some(_)) => Err(MemoryError::UnexpectedByteMask { request: id }),
            (_, None) => Ok(()),
        }
    }

    pub const fn id(&self) -> MemoryRequestId {
        self.id
    }

    pub const fn operation(&self) -> MemoryOperation {
        self.operation
    }

    pub const fn coherence_intent(&self) -> CoherenceIntent {
        self.operation.coherence_intent()
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn line_address(&self) -> Address {
        self.line_layout.line_address(self.range.start())
    }

    pub fn line_offset(&self) -> u64 {
        self.line_layout.line_offset(self.range.start())
    }

    pub fn line_span(&self) -> u64 {
        self.line_layout.line_span(self.range)
    }

    pub const fn size(&self) -> AccessSize {
        self.range.size()
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn byte_mask(&self) -> Option<&ByteMask> {
        self.byte_mask.as_ref()
    }

    pub const fn requires_response(&self) -> bool {
        self.operation.requires_response()
    }

    pub const fn returns_data(&self) -> bool {
        self.operation.returns_data()
    }

    pub const fn carries_data(&self) -> bool {
        self.operation.carries_request_data()
    }

    pub const fn requires_writable(&self) -> bool {
        self.operation.requires_writable()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseStatus {
    Completed,
    Retry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryResponse {
    request_id: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MemoryResponse {
    pub fn completed(request: &MemoryRequest, data: Option<Vec<u8>>) -> Result<Self, MemoryError> {
        if !request.requires_response() {
            return Err(MemoryError::ResponseNotExpected {
                request: request.id(),
            });
        }

        Self::validate_response_data(request, data.as_deref())?;
        Ok(Self {
            request_id: request.id(),
            status: ResponseStatus::Completed,
            data,
        })
    }

    pub fn retry(request: &MemoryRequest) -> Self {
        Self {
            request_id: request.id(),
            status: ResponseStatus::Retry,
            data: None,
        }
    }

    fn validate_response_data(
        request: &MemoryRequest,
        data: Option<&[u8]>,
    ) -> Result<(), MemoryError> {
        match (request.returns_data(), data) {
            (true, Some(bytes)) if bytes.len() as u64 == request.size().bytes() => Ok(()),
            (true, Some(bytes)) => Err(MemoryError::PayloadSizeMismatch {
                expected: request.size(),
                actual: bytes.len() as u64,
            }),
            (true, None) => Err(MemoryError::MissingResponseData {
                request: request.id(),
            }),
            (false, Some(_)) => Err(MemoryError::UnexpectedResponseData {
                request: request.id(),
            }),
            (false, None) => Ok(()),
        }
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}
