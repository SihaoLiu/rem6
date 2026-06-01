use std::collections::BTreeMap;

mod address_map;
mod error;
mod line_checkpoint;
mod ordering;
mod partition_checkpoint;
mod request;
mod translation;
mod translation_tlb;

pub use address_map::{AddressDecode, AddressDecoder, AddressInterleave, AddressMapRegion};
pub use error::MemoryError;
pub use line_checkpoint::LineMemoryCheckpointPayload;
pub use partition_checkpoint::PartitionedMemoryCheckpointPayload;
pub use request::{MemoryRequest, MemoryResponse, ResponseStatus};
pub use translation::{
    TranslationAccessKind, TranslationCompletion, TranslationError, TranslationFault,
    TranslationFaultKind, TranslationPageMap, TranslationPageMapCheckpointPayload,
    TranslationPageMapSnapshot, TranslationPageMapping, TranslationPagePermissions,
    TranslationPageSize, TranslationQueue, TranslationQueueCheckpointPayload,
    TranslationQueueConfig, TranslationQueueEntrySnapshot, TranslationQueueSnapshot,
    TranslationRequest, TranslationRequestId, TranslationResolution, TranslationSegment,
    TranslationSegmentedResolution,
};
pub use translation_tlb::{
    TranslationAddressSpaceId, TranslationTlb, TranslationTlbCheckpointPayload,
    TranslationTlbConfig, TranslationTlbEntryScope, TranslationTlbEntrySnapshot,
    TranslationTlbLookup, TranslationTlbLookupKind, TranslationTlbSnapshot, TranslationTlbStats,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryAtomicOp {
    Swap,
    Add,
    Xor,
    Or,
    And,
    MinSigned,
    MaxSigned,
    MinUnsigned,
    MaxUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryBarrierSet {
    read: bool,
    write: bool,
}

impl MemoryBarrierSet {
    pub const fn new(read: bool, write: bool) -> Self {
        Self { read, write }
    }

    pub const fn memory() -> Self {
        Self {
            read: true,
            write: true,
        }
    }

    pub const fn read(self) -> bool {
        self.read
    }

    pub const fn write(self) -> bool {
        self.write
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAccessOrdering {
    before: Option<MemoryBarrierSet>,
    after: Option<MemoryBarrierSet>,
}

impl MemoryAccessOrdering {
    pub const fn new(before: Option<MemoryBarrierSet>, after: Option<MemoryBarrierSet>) -> Self {
        Self { before, after }
    }

    pub const fn none() -> Self {
        Self {
            before: None,
            after: None,
        }
    }

    pub const fn before(self) -> Option<MemoryBarrierSet> {
        self.before
    }

    pub const fn after(self) -> Option<MemoryBarrierSet> {
        self.after
    }

    pub const fn is_ordered(self) -> bool {
        self.before.is_some() || self.after.is_some()
    }
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
            if store.lines.contains_key(&line.line()) {
                return Err(MemoryError::DuplicateMemoryLine { line: line.line() });
            }
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
            MemoryOperation::Write => {
                self.apply_write(request)?;
                MemoryResponse::completed(request, None).map(Some)
            }
            MemoryOperation::Atomic => {
                let data = self.read_slice(request)?;
                let write_data = request.atomic_write_data(&data)?;
                self.apply_write_data(request, &write_data)?;
                MemoryResponse::completed(request, Some(data)).map(Some)
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

    fn validate_access_range(&self, range: AddressRange) -> Result<(), MemoryError> {
        let mut line = self.layout.line_address(range.start());
        let last = self
            .layout
            .line_address(Address::new(range.end().get() - 1));
        loop {
            self.require_line(line)?;
            if line == last {
                return Ok(());
            }
            line = Address::new(line.get() + self.layout.bytes());
        }
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
        let payload = request.data().ok_or(MemoryError::MissingRequestData {
            request: request.id(),
        })?;
        self.apply_write_data(request, payload)
    }

    fn apply_write_data(
        &mut self,
        request: &MemoryRequest,
        payload: &[u8],
    ) -> Result<(), MemoryError> {
        let offset = request.line_offset() as usize;
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

    pub fn map_region_with_policy(
        &mut self,
        target: MemoryTargetId,
        region: AddressMapRegion,
    ) -> Result<(), MemoryError> {
        self.require_partition(target)?;
        self.decoder.insert_region(target, region)
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
            if store.partitions.contains_key(&partition.target()) {
                return Err(MemoryError::DuplicateMemoryTarget {
                    target: partition.target(),
                });
            }
            let partition_store = LineMemoryStore::from_snapshot(partition.store())?;
            store.partitions.insert(partition.target(), partition_store);
        }
        for (target, range) in snapshot.regions() {
            store.map_region_with_policy(*target, range.clone())?;
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

    pub fn regions(&self) -> &[(MemoryTargetId, AddressMapRegion)] {
        self.decoder.regions()
    }

    pub fn decode_request(&self, request: &MemoryRequest) -> Result<MemoryTargetId, MemoryError> {
        self.decoder.decode_request(request)
    }

    pub fn validate_access_range(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<MemoryTargetId, MemoryError> {
        let range = AddressRange::new(address, size)?;
        let decode = self.decoder.decode_range_detail(range)?;
        self.partition(decode.target())?
            .validate_access_range(range)?;
        Ok(decode.target())
    }

    pub fn decode_detail(&self, address: Address) -> Result<AddressDecode, MemoryError> {
        self.decoder.decode_detail(address)
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
    regions: Vec<(MemoryTargetId, AddressMapRegion)>,
}

impl PartitionedMemorySnapshot {
    pub fn new(
        partitions: Vec<MemoryPartitionSnapshot>,
        regions: Vec<(MemoryTargetId, AddressMapRegion)>,
    ) -> Self {
        Self {
            partitions,
            regions,
        }
    }

    pub fn partitions(&self) -> &[MemoryPartitionSnapshot] {
        &self.partitions
    }

    pub fn regions(&self) -> &[(MemoryTargetId, AddressMapRegion)] {
        &self.regions
    }
}
