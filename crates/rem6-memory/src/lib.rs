use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

mod translation;
mod translation_tlb;

pub use translation::{
    TranslationAccessKind, TranslationCompletion, TranslationError, TranslationFault,
    TranslationFaultKind, TranslationPageMap, TranslationPageMapSnapshot, TranslationPageMapping,
    TranslationPagePermissions, TranslationPageSize, TranslationQueue, TranslationQueueConfig,
    TranslationQueueEntrySnapshot, TranslationQueueSnapshot, TranslationRequest,
    TranslationRequestId, TranslationResolution,
};
pub use translation_tlb::{
    TranslationTlb, TranslationTlbConfig, TranslationTlbEntrySnapshot, TranslationTlbLookup,
    TranslationTlbLookupKind, TranslationTlbSnapshot, TranslationTlbStats,
};

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryTargetId(u32);

impl MemoryTargetId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
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
    AccessSizeTooLarge {
        size: AccessSize,
    },
    AddressOverflow {
        start: Address,
        size: AccessSize,
    },
    ZeroCacheLineSize,
    NonPowerOfTwoCacheLineSize {
        bytes: u64,
    },
    PayloadSizeMismatch {
        expected: AccessSize,
        actual: u64,
    },
    ByteMaskSizeMismatch {
        expected: AccessSize,
        actual: u64,
    },
    MissingRequestData {
        request: MemoryRequestId,
    },
    UnexpectedRequestData {
        request: MemoryRequestId,
    },
    MissingByteMask {
        request: MemoryRequestId,
    },
    UnexpectedByteMask {
        request: MemoryRequestId,
    },
    UnalignedLineAddress {
        address: Address,
        line_size: u64,
    },
    LineLayoutMismatch {
        request: MemoryRequestId,
        expected: CacheLineLayout,
        actual: CacheLineLayout,
    },
    CrossLineAccess {
        request: MemoryRequestId,
        start: Address,
        size: AccessSize,
        line_size: u64,
    },
    UnmappedLine {
        line: Address,
    },
    UnmappedAddress {
        address: Address,
    },
    OverlappingAddressRegion {
        existing: AddressRange,
        requested: AddressRange,
    },
    RequestCrossesAddressRegion {
        request: MemoryRequestId,
        range: AddressRange,
    },
    DuplicateMemoryTarget {
        target: MemoryTargetId,
    },
    UnknownMemoryTarget {
        target: MemoryTargetId,
    },
    MissingResponseData {
        request: MemoryRequestId,
    },
    UnexpectedResponseData {
        request: MemoryRequestId,
    },
    ResponseNotExpected {
        request: MemoryRequestId,
    },
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
            Self::LineLayoutMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "request {} from agent {} uses {}-byte lines but target expects {}-byte lines",
                request.sequence(),
                request.agent().get(),
                actual.bytes(),
                expected.bytes()
            ),
            Self::CrossLineAccess {
                request,
                start,
                size,
                line_size,
            } => write!(
                formatter,
                "request {} from agent {} crosses a {line_size}-byte line at {:#x}+{}",
                request.sequence(),
                request.agent().get(),
                start.get(),
                size.bytes()
            ),
            Self::UnmappedLine { line } => {
                write!(formatter, "line {:#x} is not mapped", line.get())
            }
            Self::UnmappedAddress { address } => {
                write!(formatter, "address {:#x} is not mapped", address.get())
            }
            Self::OverlappingAddressRegion {
                existing,
                requested,
            } => write!(
                formatter,
                "address region {:#x}..{:#x} overlaps existing region {:#x}..{:#x}",
                requested.start().get(),
                requested.end().get(),
                existing.start().get(),
                existing.end().get()
            ),
            Self::RequestCrossesAddressRegion { request, range } => write!(
                formatter,
                "request {} from agent {} crosses address region boundary at {:#x}..{:#x}",
                request.sequence(),
                request.agent().get(),
                range.start().get(),
                range.end().get()
            ),
            Self::DuplicateMemoryTarget { target } => {
                write!(
                    formatter,
                    "memory target {} is already declared",
                    target.get()
                )
            }
            Self::UnknownMemoryTarget { target } => {
                write!(formatter, "memory target {} is not declared", target.get())
            }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineMemoryStore {
    layout: CacheLineLayout,
    lines: BTreeMap<Address, Vec<u8>>,
}

impl LineMemoryStore {
    pub fn new(layout: CacheLineLayout) -> Self {
        Self {
            layout,
            lines: BTreeMap::new(),
        }
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line_data(&self, line: Address) -> Option<Vec<u8>> {
        self.lines.get(&self.layout.line_address(line)).cloned()
    }

    pub fn snapshot(&self) -> LineMemorySnapshot {
        LineMemorySnapshot::new(
            self.layout,
            self.lines
                .iter()
                .map(|(line, data)| MemoryLineSnapshot::new(*line, data.clone()))
                .collect(),
        )
    }

    pub fn restore(&mut self, snapshot: &LineMemorySnapshot) -> Result<(), MemoryError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn from_snapshot(snapshot: &LineMemorySnapshot) -> Result<Self, MemoryError> {
        let mut store = Self::new(snapshot.layout());
        for line in snapshot.lines() {
            store.insert_line(line.line(), line.data().to_vec())?;
        }
        Ok(store)
    }

    pub fn insert_line(&mut self, line: Address, data: Vec<u8>) -> Result<(), MemoryError> {
        if self.layout.line_offset(line) != 0 {
            return Err(MemoryError::UnalignedLineAddress {
                address: line,
                line_size: self.layout.bytes(),
            });
        }
        self.validate_line_data(data.len() as u64)?;
        self.lines.insert(line, data);
        Ok(())
    }

    pub fn respond(
        &mut self,
        request: &MemoryRequest,
    ) -> Result<Option<MemoryResponse>, MemoryError> {
        self.check_line_layout(request)?;
        self.check_single_line(request)?;
        match request.operation() {
            MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::ReadUnique => {
                let data = self.read_slice(request)?;
                MemoryResponse::completed(request, Some(data)).map(Some)
            }
            MemoryOperation::Write | MemoryOperation::Atomic => {
                self.apply_write(request)?;
                if request.returns_data() {
                    let data = self.read_slice(request)?;
                    MemoryResponse::completed(request, Some(data)).map(Some)
                } else {
                    MemoryResponse::completed(request, None).map(Some)
                }
            }
            MemoryOperation::Upgrade | MemoryOperation::Invalidate => {
                self.require_line(request.line_address())?;
                MemoryResponse::completed(request, None).map(Some)
            }
            MemoryOperation::PrefetchRead | MemoryOperation::PrefetchWrite => {
                self.require_line(request.line_address())?;
                Ok(None)
            }
            MemoryOperation::WritebackClean | MemoryOperation::WritebackDirty => {
                self.replace_line(request)?;
                Ok(None)
            }
            MemoryOperation::CleanEvict => {
                self.require_line(request.line_address())?;
                Ok(None)
            }
        }
    }

    fn validate_line_data(&self, actual: u64) -> Result<(), MemoryError> {
        if actual != self.layout.bytes() {
            return Err(MemoryError::PayloadSizeMismatch {
                expected: AccessSize::new(self.layout.bytes())?,
                actual,
            });
        }

        Ok(())
    }

    fn check_line_layout(&self, request: &MemoryRequest) -> Result<(), MemoryError> {
        let actual = request.line_layout();
        if actual != self.layout {
            return Err(MemoryError::LineLayoutMismatch {
                request: request.id(),
                expected: self.layout,
                actual,
            });
        }

        Ok(())
    }

    fn check_single_line(&self, request: &MemoryRequest) -> Result<(), MemoryError> {
        if request.line_span() != 1 {
            return Err(MemoryError::CrossLineAccess {
                request: request.id(),
                start: request.range().start(),
                size: request.size(),
                line_size: self.layout.bytes(),
            });
        }

        Ok(())
    }

    fn require_line(&self, line: Address) -> Result<(), MemoryError> {
        if self.lines.contains_key(&self.layout.line_address(line)) {
            return Ok(());
        }

        Err(MemoryError::UnmappedLine {
            line: self.layout.line_address(line),
        })
    }

    fn line_mut(&mut self, line: Address) -> Result<&mut Vec<u8>, MemoryError> {
        let line = self.layout.line_address(line);
        self.lines
            .get_mut(&line)
            .ok_or(MemoryError::UnmappedLine { line })
    }

    fn line_ref(&self, line: Address) -> Result<&[u8], MemoryError> {
        let line = self.layout.line_address(line);
        self.lines
            .get(&line)
            .map(Vec::as_slice)
            .ok_or(MemoryError::UnmappedLine { line })
    }

    fn read_slice(&self, request: &MemoryRequest) -> Result<Vec<u8>, MemoryError> {
        let offset = request.line_offset() as usize;
        let size = request.size().bytes() as usize;
        let line = self.line_ref(request.line_address())?;
        Ok(line[offset..offset + size].to_vec())
    }

    fn apply_write(&mut self, request: &MemoryRequest) -> Result<(), MemoryError> {
        let offset = request.line_offset() as usize;
        let payload = request.data().ok_or(MemoryError::MissingRequestData {
            request: request.id(),
        })?;
        let mask = request.byte_mask();
        let line = self.line_mut(request.line_address())?;
        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                line[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn replace_line(&mut self, request: &MemoryRequest) -> Result<(), MemoryError> {
        let data = request.data().ok_or(MemoryError::MissingRequestData {
            request: request.id(),
        })?;
        self.validate_line_data(data.len() as u64)?;
        self.lines.insert(request.line_address(), data.to_vec());
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryLineSnapshot {
    line: Address,
    data: Vec<u8>,
}

impl MemoryLineSnapshot {
    pub fn new(line: Address, data: Vec<u8>) -> Self {
        Self { line, data }
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineMemorySnapshot {
    layout: CacheLineLayout,
    lines: Vec<MemoryLineSnapshot>,
}

impl LineMemorySnapshot {
    pub fn new(layout: CacheLineLayout, lines: Vec<MemoryLineSnapshot>) -> Self {
        Self { layout, lines }
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub fn lines(&self) -> &[MemoryLineSnapshot] {
        &self.lines
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

    pub fn contains(self, address: Address) -> bool {
        self.start.get() <= address.get() && address.get() < self.end.get()
    }

    pub fn contains_range(self, range: AddressRange) -> bool {
        self.start.get() <= range.start().get() && range.end().get() <= self.end.get()
    }

    pub fn overlaps(self, other: AddressRange) -> bool {
        self.start.get() < other.end().get() && other.start().get() < self.end.get()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddressDecoder {
    regions: Vec<(MemoryTargetId, AddressRange)>,
}

impl AddressDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        target: MemoryTargetId,
        start: Address,
        size: AccessSize,
    ) -> Result<(), MemoryError> {
        let requested = AddressRange::new(start, size)?;
        if let Some((_, existing)) = self
            .regions
            .iter()
            .find(|(_, existing)| existing.overlaps(requested))
        {
            return Err(MemoryError::OverlappingAddressRegion {
                existing: *existing,
                requested,
            });
        }

        self.regions.push((target, requested));
        self.regions
            .sort_by_key(|(_, range)| (range.start(), range.end()));
        Ok(())
    }

    pub fn decode(&self, address: Address) -> Result<MemoryTargetId, MemoryError> {
        self.regions
            .iter()
            .find_map(|(target, range)| range.contains(address).then_some(*target))
            .ok_or(MemoryError::UnmappedAddress { address })
    }

    pub fn decode_request(&self, request: &MemoryRequest) -> Result<MemoryTargetId, MemoryError> {
        let range = request.range();
        let Some((target, region)) = self
            .regions
            .iter()
            .find(|(_, region)| region.contains(range.start()))
        else {
            return Err(MemoryError::UnmappedAddress {
                address: range.start(),
            });
        };

        if !region.contains_range(range) {
            return Err(MemoryError::RequestCrossesAddressRegion {
                request: request.id(),
                range,
            });
        }

        Ok(*target)
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    pub fn regions(&self) -> &[(MemoryTargetId, AddressRange)] {
        &self.regions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMemoryOutcome {
    target: MemoryTargetId,
    response: Option<MemoryResponse>,
}

impl PartitionedMemoryOutcome {
    fn new(target: MemoryTargetId, response: Option<MemoryResponse>) -> Self {
        Self { target, response }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub fn response(&self) -> Option<&MemoryResponse> {
        self.response.as_ref()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PartitionedMemoryStore {
    decoder: AddressDecoder,
    partitions: BTreeMap<MemoryTargetId, LineMemoryStore>,
}

impl PartitionedMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_partition(
        &mut self,
        target: MemoryTargetId,
        layout: CacheLineLayout,
    ) -> Result<(), MemoryError> {
        if self.partitions.contains_key(&target) {
            return Err(MemoryError::DuplicateMemoryTarget { target });
        }

        self.partitions.insert(target, LineMemoryStore::new(layout));
        Ok(())
    }

    pub fn map_region(
        &mut self,
        target: MemoryTargetId,
        start: Address,
        size: AccessSize,
    ) -> Result<(), MemoryError> {
        self.require_partition(target)?;
        self.decoder.insert(target, start, size)
    }

    pub fn insert_line(
        &mut self,
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    ) -> Result<(), MemoryError> {
        self.partition_mut(target)?.insert_line(line, data)
    }

    pub fn respond(
        &mut self,
        request: &MemoryRequest,
    ) -> Result<PartitionedMemoryOutcome, MemoryError> {
        let target = self.decoder.decode_request(request)?;
        let response = self.partition_mut(target)?.respond(request)?;
        Ok(PartitionedMemoryOutcome::new(target, response))
    }

    pub fn snapshot(&self) -> PartitionedMemorySnapshot {
        PartitionedMemorySnapshot::new(
            self.partitions
                .iter()
                .map(|(target, store)| MemoryPartitionSnapshot::new(*target, store.snapshot()))
                .collect(),
            self.decoder.regions().to_vec(),
        )
    }

    pub fn restore(&mut self, snapshot: &PartitionedMemorySnapshot) -> Result<(), MemoryError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn from_snapshot(snapshot: &PartitionedMemorySnapshot) -> Result<Self, MemoryError> {
        let mut store = Self::new();
        for partition in snapshot.partitions() {
            store.add_partition(partition.target(), partition.store().layout())?;
        }
        for (target, range) in snapshot.regions() {
            store.map_region(*target, range.start(), range.size())?;
        }
        for partition in snapshot.partitions() {
            for line in partition.store().lines() {
                store.insert_line(partition.target(), line.line(), line.data().to_vec())?;
            }
        }
        Ok(store)
    }

    pub fn line_data(&self, target: MemoryTargetId, line: Address) -> Result<Vec<u8>, MemoryError> {
        let partition = self.partition(target)?;
        partition.line_data(line).ok_or(MemoryError::UnmappedLine {
            line: partition.layout().line_address(line),
        })
    }

    pub fn line_count(&self, target: MemoryTargetId) -> Result<usize, MemoryError> {
        Ok(self.partition(target)?.line_count())
    }

    pub fn contains_partition(&self, target: MemoryTargetId) -> bool {
        self.partitions.contains_key(&target)
    }

    pub fn partition_layout(&self, target: MemoryTargetId) -> Result<CacheLineLayout, MemoryError> {
        Ok(self.partition(target)?.layout())
    }

    pub fn partition_count(&self) -> usize {
        self.partitions.len()
    }

    pub fn region_count(&self) -> usize {
        self.decoder.region_count()
    }

    pub fn regions(&self) -> &[(MemoryTargetId, AddressRange)] {
        self.decoder.regions()
    }

    pub fn decode_request(&self, request: &MemoryRequest) -> Result<MemoryTargetId, MemoryError> {
        self.decoder.decode_request(request)
    }

    fn require_partition(&self, target: MemoryTargetId) -> Result<(), MemoryError> {
        if self.partitions.contains_key(&target) {
            return Ok(());
        }

        Err(MemoryError::UnknownMemoryTarget { target })
    }

    fn partition(&self, target: MemoryTargetId) -> Result<&LineMemoryStore, MemoryError> {
        self.partitions
            .get(&target)
            .ok_or(MemoryError::UnknownMemoryTarget { target })
    }

    fn partition_mut(
        &mut self,
        target: MemoryTargetId,
    ) -> Result<&mut LineMemoryStore, MemoryError> {
        self.partitions
            .get_mut(&target)
            .ok_or(MemoryError::UnknownMemoryTarget { target })
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
pub struct MemoryPartitionSnapshot {
    target: MemoryTargetId,
    store: LineMemorySnapshot,
}

impl MemoryPartitionSnapshot {
    pub fn new(target: MemoryTargetId, store: LineMemorySnapshot) -> Self {
        Self { target, store }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.store.layout()
    }

    pub fn lines(&self) -> &[MemoryLineSnapshot] {
        self.store.lines()
    }

    fn store(&self) -> &LineMemorySnapshot {
        &self.store
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMemorySnapshot {
    partitions: Vec<MemoryPartitionSnapshot>,
    regions: Vec<(MemoryTargetId, AddressRange)>,
}

impl PartitionedMemorySnapshot {
    pub fn new(
        partitions: Vec<MemoryPartitionSnapshot>,
        regions: Vec<(MemoryTargetId, AddressRange)>,
    ) -> Self {
        Self {
            partitions,
            regions,
        }
    }

    pub fn partitions(&self) -> &[MemoryPartitionSnapshot] {
        &self.partitions
    }

    pub fn regions(&self) -> &[(MemoryTargetId, AddressRange)] {
        &self.regions
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

    pub fn clean_evict(
        id: MemoryRequestId,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        if line_layout.line_offset(address) != 0 {
            return Err(MemoryError::UnalignedLineAddress {
                address,
                line_size: line_layout.bytes(),
            });
        }

        let size = AccessSize::new(line_layout.bytes())?;
        Self::new(
            id,
            MemoryOperation::CleanEvict,
            address,
            size,
            line_layout,
            None,
            None,
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

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
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
