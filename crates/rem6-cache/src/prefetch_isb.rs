use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::allocation::max_vector_len;
use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferConfig {
    line_size: u64,
    counter_bits: u32,
    max_counter: u8,
    chunk_size: u64,
    degree: u32,
    training_entries: usize,
    address_map_entries: usize,
    prefetch_candidates_per_entry: usize,
}

impl IrregularStreamBufferConfig {
    pub fn new(
        line_size: u64,
        counter_bits: u32,
        chunk_size: u64,
        degree: u32,
        training_entries: usize,
        address_map_entries: usize,
        prefetch_candidates_per_entry: usize,
    ) -> Result<Self, IrregularStreamBufferError> {
        if line_size == 0 {
            return Err(IrregularStreamBufferError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(IrregularStreamBufferError::LineSizeNotPowerOfTwo { line_size });
        }
        if !(1..=8).contains(&counter_bits) {
            return Err(IrregularStreamBufferError::CounterBitsOutOfRange { counter_bits });
        }
        if chunk_size == 0 {
            return Err(IrregularStreamBufferError::ZeroChunkSize);
        }
        if degree == 0 {
            return Err(IrregularStreamBufferError::ZeroDegree);
        }
        if training_entries == 0 {
            return Err(IrregularStreamBufferError::ZeroTrainingEntries);
        }
        validate_isb_vector_length(
            "training entries",
            training_entries,
            maximum_isb_training_entries(),
        )?;
        if address_map_entries == 0 {
            return Err(IrregularStreamBufferError::ZeroAddressMapEntries);
        }
        validate_isb_vector_length(
            "address map entries",
            address_map_entries,
            maximum_isb_address_map_entries(),
        )?;
        if prefetch_candidates_per_entry == 0 {
            return Err(IrregularStreamBufferError::ZeroPrefetchCandidatesPerEntry);
        }
        if !prefetch_candidates_per_entry.is_power_of_two() {
            return Err(
                IrregularStreamBufferError::PrefetchCandidatesPerEntryNotPowerOfTwo {
                    prefetch_candidates_per_entry,
                },
            );
        }
        validate_isb_vector_length(
            "prefetch candidates per entry",
            prefetch_candidates_per_entry,
            maximum_isb_mapping_slots(),
        )?;

        let max_counter = ((1_u16 << counter_bits) - 1) as u8;
        Ok(Self {
            line_size,
            counter_bits,
            max_counter,
            chunk_size,
            degree,
            training_entries,
            address_map_entries,
            prefetch_candidates_per_entry,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn counter_bits(&self) -> u32 {
        self.counter_bits
    }

    pub const fn max_counter(&self) -> u8 {
        self.max_counter
    }

    pub const fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    pub const fn degree(&self) -> u32 {
        self.degree
    }

    pub const fn training_entries(&self) -> usize {
        self.training_entries
    }

    pub const fn address_map_entries(&self) -> usize {
        self.address_map_entries
    }

    pub const fn prefetch_candidates_per_entry(&self) -> usize {
        self.prefetch_candidates_per_entry
    }

    fn candidates_per_entry_u64(&self) -> u64 {
        self.prefetch_candidates_per_entry as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IrregularStreamBufferError {
    ZeroLineSize,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    CounterBitsOutOfRange {
        counter_bits: u32,
    },
    ZeroChunkSize,
    ZeroDegree,
    ZeroTrainingEntries,
    ZeroAddressMapEntries,
    ZeroPrefetchCandidatesPerEntry,
    PrefetchCandidatesPerEntryNotPowerOfTwo {
        prefetch_candidates_per_entry: usize,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    StructuralAddressCounterOverflow {
        current: u64,
        chunk_size: u64,
    },
    SnapshotConfigMismatch {
        expected: Box<IrregularStreamBufferConfig>,
        actual: Box<IrregularStreamBufferConfig>,
    },
    SnapshotTrainingTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotPhysicalMappingTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotStructuralMappingTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotMappingShapeMismatch {
        amc_address: u64,
        entries: usize,
        expected: usize,
    },
    SnapshotCounterOutOfRange {
        amc_address: u64,
        index: usize,
        counter: u8,
        max_counter: u8,
    },
    SnapshotTrainingLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPhysicalMappingLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotStructuralMappingLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotTrainingLruUnknownKey {
        pc: u64,
        secure: bool,
    },
    SnapshotPhysicalMappingLruUnknownKey {
        amc_address: u64,
        secure: bool,
    },
    SnapshotStructuralMappingLruUnknownKey {
        amc_address: u64,
        secure: bool,
    },
}

impl fmt::Display for IrregularStreamBufferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "irregular stream buffer line size is zero"),
            Self::LineSizeNotPowerOfTwo { line_size } => write!(
                formatter,
                "irregular stream buffer line size {line_size} is not a power of two"
            ),
            Self::CounterBitsOutOfRange { counter_bits } => write!(
                formatter,
                "irregular stream buffer counter bit count {counter_bits} is outside 1..=8"
            ),
            Self::ZeroChunkSize => write!(formatter, "irregular stream buffer chunk size is zero"),
            Self::ZeroDegree => write!(formatter, "irregular stream buffer degree is zero"),
            Self::ZeroTrainingEntries => {
                write!(formatter, "irregular stream buffer training unit has no entries")
            }
            Self::ZeroAddressMapEntries => write!(
                formatter,
                "irregular stream buffer address mapping cache has no entries"
            ),
            Self::ZeroPrefetchCandidatesPerEntry => write!(
                formatter,
                "irregular stream buffer mapping entries have no candidate slots"
            ),
            Self::PrefetchCandidatesPerEntryNotPowerOfTwo {
                prefetch_candidates_per_entry,
            } => write!(
                formatter,
                "irregular stream buffer candidate count {prefetch_candidates_per_entry} is not a power of two"
            ),
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "irregular stream buffer {field} length {length} exceeds maximum {maximum}"
            ),
            Self::StructuralAddressCounterOverflow {
                current,
                chunk_size,
            } => write!(
                formatter,
                "irregular stream buffer structural address counter {current} overflows by chunk size {chunk_size}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "irregular stream buffer snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotTrainingTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "irregular stream buffer snapshot has {entries} training entries for {max_entries} slots"
            ),
            Self::SnapshotPhysicalMappingTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "irregular stream buffer snapshot has {entries} physical mapping entries for {max_entries} slots"
            ),
            Self::SnapshotStructuralMappingTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "irregular stream buffer snapshot has {entries} structural mapping entries for {max_entries} slots"
            ),
            Self::SnapshotMappingShapeMismatch {
                amc_address,
                entries,
                expected,
            } => write!(
                formatter,
                "irregular stream buffer snapshot mapping entry {amc_address} has {entries} slots instead of {expected}"
            ),
            Self::SnapshotCounterOutOfRange {
                amc_address,
                index,
                counter,
                max_counter,
            } => write!(
                formatter,
                "irregular stream buffer snapshot mapping entry {amc_address} slot {index} has counter {counter} above {max_counter}"
            ),
            Self::SnapshotTrainingLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "irregular stream buffer snapshot training LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotPhysicalMappingLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "irregular stream buffer snapshot physical mapping LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotStructuralMappingLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "irregular stream buffer snapshot structural mapping LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotTrainingLruUnknownKey { pc, secure } => write!(
                formatter,
                "irregular stream buffer snapshot training LRU references missing key pc {pc:#x}, secure {secure}"
            ),
            Self::SnapshotPhysicalMappingLruUnknownKey {
                amc_address,
                secure,
            } => write!(
                formatter,
                "irregular stream buffer snapshot physical mapping LRU references missing entry {amc_address}, secure {secure}"
            ),
            Self::SnapshotStructuralMappingLruUnknownKey {
                amc_address,
                secure,
            } => write!(
                formatter,
                "irregular stream buffer snapshot structural mapping LRU references missing entry {amc_address}, secure {secure}"
            ),
        }
    }
}

impl Error for IrregularStreamBufferError {}

fn maximum_isb_training_entries() -> usize {
    max_vector_len::<TrainingEntry>()
        .min(max_vector_len::<IrregularStreamBufferTrainingEntrySnapshot>())
        .min(max_vector_len::<TrainingKey>())
        .min(max_vector_len::<IrregularStreamBufferTrainingKeySnapshot>())
}

fn maximum_isb_address_map_entries() -> usize {
    max_vector_len::<AddressMappingEntry>()
        .min(max_vector_len::<IrregularStreamBufferMappingEntrySnapshot>())
        .min(max_vector_len::<MappingKey>())
        .min(max_vector_len::<IrregularStreamBufferMappingKeySnapshot>())
}

fn maximum_isb_mapping_slots() -> usize {
    max_vector_len::<AddressMapping>()
        .min(max_vector_len::<IrregularStreamBufferMappingSnapshot>())
        .min(max_vector_len::<IrregularStreamBufferCandidate>())
}

fn validate_isb_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), IrregularStreamBufferError> {
    if length > maximum {
        return Err(IrregularStreamBufferError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl IrregularStreamBufferAccess {
    pub const fn new(requestor: AgentId, pc: u64, address: Address, secure: bool) -> Self {
        Self {
            requestor,
            pc,
            address,
            secure,
        }
    }

    pub const fn requestor(&self) -> AgentId {
        self.requestor
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    physical_block: u64,
    structural_address: u64,
    stride: i64,
    degree_index: u32,
}

impl IrregularStreamBufferCandidate {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn physical_block(&self) -> u64 {
        self.physical_block
    }

    pub const fn structural_address(&self) -> u64 {
        self.structural_address
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for IrregularStreamBufferCandidate {
    fn address(&self) -> Address {
        self.address()
    }

    fn source_address(&self) -> Address {
        self.source_address()
    }

    fn context(&self) -> AgentId {
        self.context()
    }

    fn pc(&self) -> u64 {
        self.pc()
    }

    fn secure(&self) -> bool {
        self.secure()
    }

    fn stride(&self) -> i64 {
        self.stride()
    }

    fn degree_index(&self) -> u32 {
        self.degree_index()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferTrainingEntrySnapshot {
    pc: u64,
    secure: bool,
    last_block: u64,
}

impl IrregularStreamBufferTrainingEntrySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn last_block(&self) -> u64 {
        self.last_block
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferMappingSnapshot {
    address: u64,
    counter: u8,
}

impl IrregularStreamBufferMappingSnapshot {
    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn counter(&self) -> u8 {
        self.counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferMappingEntrySnapshot {
    amc_address: u64,
    secure: bool,
    mappings: Vec<IrregularStreamBufferMappingSnapshot>,
}

impl IrregularStreamBufferMappingEntrySnapshot {
    pub const fn amc_address(&self) -> u64 {
        self.amc_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub fn mappings(&self) -> &[IrregularStreamBufferMappingSnapshot] {
        &self.mappings
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferTrainingKeySnapshot {
    pc: u64,
    secure: bool,
}

impl IrregularStreamBufferTrainingKeySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferMappingKeySnapshot {
    amc_address: u64,
    secure: bool,
}

impl IrregularStreamBufferMappingKeySnapshot {
    pub const fn amc_address(&self) -> u64 {
        self.amc_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferSnapshot {
    config: IrregularStreamBufferConfig,
    structural_address_counter: u64,
    training_entries: Vec<IrregularStreamBufferTrainingEntrySnapshot>,
    training_lru: Vec<IrregularStreamBufferTrainingKeySnapshot>,
    physical_mapping_entries: Vec<IrregularStreamBufferMappingEntrySnapshot>,
    physical_mapping_lru: Vec<IrregularStreamBufferMappingKeySnapshot>,
    structural_mapping_entries: Vec<IrregularStreamBufferMappingEntrySnapshot>,
    structural_mapping_lru: Vec<IrregularStreamBufferMappingKeySnapshot>,
    last_candidates: Vec<IrregularStreamBufferCandidate>,
}

impl IrregularStreamBufferSnapshot {
    pub const fn config(&self) -> &IrregularStreamBufferConfig {
        &self.config
    }

    pub const fn structural_address_counter(&self) -> u64 {
        self.structural_address_counter
    }

    pub fn training_entries(&self) -> &[IrregularStreamBufferTrainingEntrySnapshot] {
        &self.training_entries
    }

    pub fn training_lru(&self) -> &[IrregularStreamBufferTrainingKeySnapshot] {
        &self.training_lru
    }

    pub fn physical_mapping_entries(&self) -> &[IrregularStreamBufferMappingEntrySnapshot] {
        &self.physical_mapping_entries
    }

    pub fn physical_mapping_lru(&self) -> &[IrregularStreamBufferMappingKeySnapshot] {
        &self.physical_mapping_lru
    }

    pub fn structural_mapping_entries(&self) -> &[IrregularStreamBufferMappingEntrySnapshot] {
        &self.structural_mapping_entries
    }

    pub fn structural_mapping_lru(&self) -> &[IrregularStreamBufferMappingKeySnapshot] {
        &self.structural_mapping_lru
    }

    pub fn last_candidates(&self) -> &[IrregularStreamBufferCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TrainingKey {
    pc: u64,
    secure: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrainingEntry {
    last_block: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MappingKey {
    amc_address: u64,
    secure: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AddressMapping {
    address: u64,
    counter: u8,
}

impl AddressMapping {
    fn is_valid(&self) -> bool {
        self.counter > 0
    }

    fn reset_to_one(&mut self) {
        self.counter = 1;
    }

    fn increment(&mut self, max_counter: u8) {
        self.counter = self.counter.saturating_add(1).min(max_counter);
    }

    fn decrement(&mut self) {
        self.counter = self.counter.saturating_sub(1);
    }

    fn snapshot(&self) -> IrregularStreamBufferMappingSnapshot {
        IrregularStreamBufferMappingSnapshot {
            address: self.address,
            counter: self.counter,
        }
    }

    fn from_snapshot(snapshot: &IrregularStreamBufferMappingSnapshot) -> Self {
        Self {
            address: snapshot.address(),
            counter: snapshot.counter(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AddressMappingEntry {
    mappings: Vec<AddressMapping>,
}

impl AddressMappingEntry {
    fn new(slot_count: usize) -> Self {
        Self {
            mappings: vec![AddressMapping::default(); slot_count],
        }
    }

    fn snapshot(&self, key: MappingKey) -> IrregularStreamBufferMappingEntrySnapshot {
        IrregularStreamBufferMappingEntrySnapshot {
            amc_address: key.amc_address,
            secure: key.secure,
            mappings: self.mappings.iter().map(AddressMapping::snapshot).collect(),
        }
    }

    fn from_snapshot(snapshot: &IrregularStreamBufferMappingEntrySnapshot) -> Self {
        Self {
            mappings: snapshot
                .mappings()
                .iter()
                .map(AddressMapping::from_snapshot)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrregularStreamBufferPrefetcher {
    config: IrregularStreamBufferConfig,
    training_unit: BTreeMap<TrainingKey, TrainingEntry>,
    training_lru: VecDeque<TrainingKey>,
    physical_mapping_cache: BTreeMap<MappingKey, AddressMappingEntry>,
    physical_mapping_lru: VecDeque<MappingKey>,
    structural_mapping_cache: BTreeMap<MappingKey, AddressMappingEntry>,
    structural_mapping_lru: VecDeque<MappingKey>,
    structural_address_counter: u64,
    last_candidates: Vec<IrregularStreamBufferCandidate>,
}

impl IrregularStreamBufferPrefetcher {
    pub fn new(config: IrregularStreamBufferConfig) -> Self {
        Self {
            config,
            training_unit: BTreeMap::new(),
            training_lru: VecDeque::new(),
            physical_mapping_cache: BTreeMap::new(),
            physical_mapping_lru: VecDeque::new(),
            structural_mapping_cache: BTreeMap::new(),
            structural_mapping_lru: VecDeque::new(),
            structural_address_counter: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &IrregularStreamBufferConfig {
        &self.config
    }

    pub fn observe(
        &mut self,
        access: IrregularStreamBufferAccess,
    ) -> Result<&[IrregularStreamBufferCandidate], IrregularStreamBufferError> {
        self.last_candidates.clear();
        let block = self.block_number(access.address());
        let key = TrainingKey {
            pc: access.pc(),
            secure: access.secure(),
        };

        let correlated_block = self.training_unit.get(&key).map(|entry| entry.last_block);
        if correlated_block.is_some() {
            self.touch_training_lru(key);
        } else {
            self.insert_training_entry(key, block);
        }
        let entry = self
            .training_unit
            .get_mut(&key)
            .expect("training entry exists after lookup or insertion");
        entry.last_block = block;

        if let Some(previous_block) = correlated_block {
            self.train_correlation(previous_block, block, access.secure())?;
        }
        self.emit_predictions(access, block);
        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> IrregularStreamBufferSnapshot {
        IrregularStreamBufferSnapshot {
            config: self.config.clone(),
            structural_address_counter: self.structural_address_counter,
            training_entries: self
                .training_unit
                .iter()
                .map(|(key, entry)| IrregularStreamBufferTrainingEntrySnapshot {
                    pc: key.pc,
                    secure: key.secure,
                    last_block: entry.last_block,
                })
                .collect(),
            training_lru: self
                .training_lru
                .iter()
                .map(|key| IrregularStreamBufferTrainingKeySnapshot {
                    pc: key.pc,
                    secure: key.secure,
                })
                .collect(),
            physical_mapping_entries: self
                .physical_mapping_cache
                .iter()
                .map(|(key, entry)| entry.snapshot(*key))
                .collect(),
            physical_mapping_lru: self
                .physical_mapping_lru
                .iter()
                .map(|key| IrregularStreamBufferMappingKeySnapshot {
                    amc_address: key.amc_address,
                    secure: key.secure,
                })
                .collect(),
            structural_mapping_entries: self
                .structural_mapping_cache
                .iter()
                .map(|(key, entry)| entry.snapshot(*key))
                .collect(),
            structural_mapping_lru: self
                .structural_mapping_lru
                .iter()
                .map(|key| IrregularStreamBufferMappingKeySnapshot {
                    amc_address: key.amc_address,
                    secure: key.secure,
                })
                .collect(),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &IrregularStreamBufferSnapshot,
    ) -> Result<(), IrregularStreamBufferError> {
        self.validate_snapshot(snapshot)?;

        self.training_unit = snapshot
            .training_entries()
            .iter()
            .map(|entry| {
                (
                    TrainingKey {
                        pc: entry.pc(),
                        secure: entry.secure(),
                    },
                    TrainingEntry {
                        last_block: entry.last_block(),
                    },
                )
            })
            .collect();
        self.training_lru = snapshot
            .training_lru()
            .iter()
            .map(|key| TrainingKey {
                pc: key.pc(),
                secure: key.secure(),
            })
            .collect();
        self.physical_mapping_cache = snapshot
            .physical_mapping_entries()
            .iter()
            .map(|entry| {
                (
                    MappingKey {
                        amc_address: entry.amc_address(),
                        secure: entry.secure(),
                    },
                    AddressMappingEntry::from_snapshot(entry),
                )
            })
            .collect();
        self.physical_mapping_lru = snapshot
            .physical_mapping_lru()
            .iter()
            .map(|key| MappingKey {
                amc_address: key.amc_address(),
                secure: key.secure(),
            })
            .collect();
        self.structural_mapping_cache = snapshot
            .structural_mapping_entries()
            .iter()
            .map(|entry| {
                (
                    MappingKey {
                        amc_address: entry.amc_address(),
                        secure: entry.secure(),
                    },
                    AddressMappingEntry::from_snapshot(entry),
                )
            })
            .collect();
        self.structural_mapping_lru = snapshot
            .structural_mapping_lru()
            .iter()
            .map(|key| MappingKey {
                amc_address: key.amc_address(),
                secure: key.secure(),
            })
            .collect();
        self.structural_address_counter = snapshot.structural_address_counter();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn training_entry_count(&self) -> usize {
        self.training_unit.len()
    }

    pub fn physical_mapping_entry_count(&self) -> usize {
        self.physical_mapping_cache.len()
    }

    pub fn structural_mapping_entry_count(&self) -> usize {
        self.structural_mapping_cache.len()
    }

    pub const fn structural_address_counter(&self) -> u64 {
        self.structural_address_counter
    }

    pub fn physical_to_structural(&self, address: Address, secure: bool) -> Option<(u64, u8)> {
        let block = self.block_number(address);
        let (key, index) = self.mapping_key_and_index(block, secure);
        let mapping = self.physical_mapping_cache.get(&key)?.mappings.get(index)?;
        mapping
            .is_valid()
            .then_some((mapping.address, mapping.counter))
    }

    pub fn structural_to_physical(
        &self,
        structural_address: u64,
        secure: bool,
    ) -> Option<(Address, u8)> {
        let (key, index) = self.mapping_key_and_index(structural_address, secure);
        let mapping = self
            .structural_mapping_cache
            .get(&key)?
            .mappings
            .get(index)?;
        if !mapping.is_valid() {
            return None;
        }
        let address = mapping.address.checked_mul(self.config.line_size())?;
        Some((Address::new(address), mapping.counter))
    }

    pub fn last_candidates(&self) -> &[IrregularStreamBufferCandidate] {
        &self.last_candidates
    }

    fn validate_snapshot(
        &self,
        snapshot: &IrregularStreamBufferSnapshot,
    ) -> Result<(), IrregularStreamBufferError> {
        if snapshot.config() != &self.config {
            return Err(IrregularStreamBufferError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.training_entries().len() > self.config.training_entries() {
            return Err(IrregularStreamBufferError::SnapshotTrainingTooLarge {
                entries: snapshot.training_entries().len(),
                max_entries: self.config.training_entries(),
            });
        }
        if snapshot.physical_mapping_entries().len() > self.config.address_map_entries() {
            return Err(
                IrregularStreamBufferError::SnapshotPhysicalMappingTooLarge {
                    entries: snapshot.physical_mapping_entries().len(),
                    max_entries: self.config.address_map_entries(),
                },
            );
        }
        if snapshot.structural_mapping_entries().len() > self.config.address_map_entries() {
            return Err(
                IrregularStreamBufferError::SnapshotStructuralMappingTooLarge {
                    entries: snapshot.structural_mapping_entries().len(),
                    max_entries: self.config.address_map_entries(),
                },
            );
        }
        validate_mapping_entries(&self.config, snapshot.physical_mapping_entries())?;
        validate_mapping_entries(&self.config, snapshot.structural_mapping_entries())?;
        self.validate_training_lru(snapshot)?;
        self.validate_mapping_lru(
            snapshot.physical_mapping_lru(),
            snapshot.physical_mapping_entries(),
            true,
        )?;
        self.validate_mapping_lru(
            snapshot.structural_mapping_lru(),
            snapshot.structural_mapping_entries(),
            false,
        )?;
        Ok(())
    }

    fn validate_training_lru(
        &self,
        snapshot: &IrregularStreamBufferSnapshot,
    ) -> Result<(), IrregularStreamBufferError> {
        if snapshot.training_lru().len() != snapshot.training_entries().len() {
            return Err(
                IrregularStreamBufferError::SnapshotTrainingLruShapeMismatch {
                    entries: snapshot.training_lru().len(),
                    table_entries: snapshot.training_entries().len(),
                },
            );
        }
        let keys = snapshot
            .training_entries()
            .iter()
            .map(|entry| TrainingKey {
                pc: entry.pc(),
                secure: entry.secure(),
            })
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for key in snapshot.training_lru() {
            let lru_key = TrainingKey {
                pc: key.pc(),
                secure: key.secure(),
            };
            if !keys.contains(&lru_key) || !seen.insert(lru_key) {
                return Err(IrregularStreamBufferError::SnapshotTrainingLruUnknownKey {
                    pc: key.pc(),
                    secure: key.secure(),
                });
            }
        }
        Ok(())
    }

    fn validate_mapping_lru(
        &self,
        lru: &[IrregularStreamBufferMappingKeySnapshot],
        entries: &[IrregularStreamBufferMappingEntrySnapshot],
        physical: bool,
    ) -> Result<(), IrregularStreamBufferError> {
        if lru.len() != entries.len() {
            if physical {
                return Err(
                    IrregularStreamBufferError::SnapshotPhysicalMappingLruShapeMismatch {
                        entries: lru.len(),
                        table_entries: entries.len(),
                    },
                );
            }
            return Err(
                IrregularStreamBufferError::SnapshotStructuralMappingLruShapeMismatch {
                    entries: lru.len(),
                    table_entries: entries.len(),
                },
            );
        }
        let keys = entries
            .iter()
            .map(|entry| MappingKey {
                amc_address: entry.amc_address(),
                secure: entry.secure(),
            })
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for key in lru {
            let lru_key = MappingKey {
                amc_address: key.amc_address(),
                secure: key.secure(),
            };
            if keys.contains(&lru_key) && seen.insert(lru_key) {
                continue;
            }
            if physical {
                return Err(
                    IrregularStreamBufferError::SnapshotPhysicalMappingLruUnknownKey {
                        amc_address: key.amc_address(),
                        secure: key.secure(),
                    },
                );
            }
            return Err(
                IrregularStreamBufferError::SnapshotStructuralMappingLruUnknownKey {
                    amc_address: key.amc_address(),
                    secure: key.secure(),
                },
            );
        }
        Ok(())
    }

    fn train_correlation(
        &mut self,
        previous_block: u64,
        current_block: u64,
        secure: bool,
    ) -> Result<(), IrregularStreamBufferError> {
        let mut mapping_a = self.ensure_physical_mapping(previous_block, secure);
        let mut mapping_b = self.ensure_physical_mapping(current_block, secure);
        if mapping_a.is_valid() && mapping_b.is_valid() {
            if mapping_b.address == mapping_a.address.saturating_add(1) {
                mapping_b.increment(self.config.max_counter());
                self.write_physical_mapping(current_block, secure, mapping_b);
            } else if mapping_b.counter == 1 {
                mapping_b.address = mapping_a.address.saturating_add(1);
                self.write_physical_mapping(current_block, secure, mapping_b);
                self.add_structural_to_physical(mapping_b.address, secure, current_block);
            } else {
                mapping_b.decrement();
                self.write_physical_mapping(current_block, secure, mapping_b);
            }
            return Ok(());
        }

        if !mapping_a.is_valid() {
            mapping_a.address = self.structural_address_counter;
            mapping_a.reset_to_one();
            self.structural_address_counter = self
                .structural_address_counter
                .checked_add(self.config.chunk_size())
                .ok_or(
                    IrregularStreamBufferError::StructuralAddressCounterOverflow {
                        current: self.structural_address_counter,
                        chunk_size: self.config.chunk_size(),
                    },
                )?;
            self.write_physical_mapping(previous_block, secure, mapping_a);
            self.add_structural_to_physical(mapping_a.address, secure, previous_block);
        }

        mapping_b.address = mapping_a.address.saturating_add(1);
        mapping_b.reset_to_one();
        self.write_physical_mapping(current_block, secure, mapping_b);
        self.add_structural_to_physical(mapping_b.address, secure, current_block);
        Ok(())
    }

    fn emit_predictions(&mut self, access: IrregularStreamBufferAccess, block: u64) {
        let Some(mapping) = self.peek_physical_mapping(block, access.secure()) else {
            return;
        };
        if !mapping.is_valid() {
            return;
        }

        let base_structural = mapping.address;
        let (sp_key, sp_index) = self.mapping_key_and_index(base_structural, access.secure());
        let Some(sp_entry) = self.structural_mapping_cache.get(&sp_key) else {
            return;
        };

        for degree in 1..=self.config.degree() {
            let candidate_index = sp_index.saturating_add(degree as usize);
            if candidate_index >= self.config.prefetch_candidates_per_entry() {
                break;
            }
            let Some(sp_mapping) = sp_entry.mappings.get(candidate_index) else {
                break;
            };
            if !sp_mapping.is_valid() {
                continue;
            }
            let Some(candidate_address) = sp_mapping.address.checked_mul(self.config.line_size())
            else {
                continue;
            };
            let address = Address::new(candidate_address);
            let source_address = self.block_address(access.address());
            self.last_candidates.push(IrregularStreamBufferCandidate {
                address,
                source_address,
                context: access.requestor(),
                pc: access.pc(),
                secure: access.secure(),
                physical_block: sp_mapping.address,
                structural_address: base_structural.saturating_add(degree as u64),
                stride: byte_stride(source_address, address),
                degree_index: degree,
            });
        }
    }

    fn insert_training_entry(&mut self, key: TrainingKey, last_block: u64) {
        while self.training_unit.len() >= self.config.training_entries() {
            let Some(victim) = self.training_lru.pop_back() else {
                break;
            };
            self.training_unit.remove(&victim);
        }
        self.training_unit.insert(key, TrainingEntry { last_block });
        self.touch_training_lru(key);
    }

    fn touch_training_lru(&mut self, key: TrainingKey) {
        self.training_lru.retain(|entry| *entry != key);
        self.training_lru.push_front(key);
    }

    fn ensure_physical_mapping(&mut self, block: u64, secure: bool) -> AddressMapping {
        let (key, index) = self.mapping_key_and_index(block, secure);
        if !self.physical_mapping_cache.contains_key(&key) {
            self.trim_physical_mapping_for_insert();
            self.physical_mapping_cache.insert(
                key,
                AddressMappingEntry::new(self.config.prefetch_candidates_per_entry()),
            );
        }
        self.touch_physical_mapping_lru(key);
        self.physical_mapping_cache
            .get(&key)
            .expect("physical mapping entry exists after insertion")
            .mappings[index]
    }

    fn write_physical_mapping(&mut self, block: u64, secure: bool, mapping: AddressMapping) {
        let (key, index) = self.mapping_key_and_index(block, secure);
        if !self.physical_mapping_cache.contains_key(&key) {
            self.trim_physical_mapping_for_insert();
            self.physical_mapping_cache.insert(
                key,
                AddressMappingEntry::new(self.config.prefetch_candidates_per_entry()),
            );
        }
        self.touch_physical_mapping_lru(key);
        self.physical_mapping_cache
            .get_mut(&key)
            .expect("physical mapping entry exists after insertion")
            .mappings[index] = mapping;
    }

    fn peek_physical_mapping(&self, block: u64, secure: bool) -> Option<AddressMapping> {
        let (key, index) = self.mapping_key_and_index(block, secure);
        self.physical_mapping_cache
            .get(&key)
            .and_then(|entry| entry.mappings.get(index).copied())
    }

    fn add_structural_to_physical(
        &mut self,
        structural_address: u64,
        secure: bool,
        physical_block: u64,
    ) {
        let (key, index) = self.mapping_key_and_index(structural_address, secure);
        if !self.structural_mapping_cache.contains_key(&key) {
            self.trim_structural_mapping_for_insert();
            self.structural_mapping_cache.insert(
                key,
                AddressMappingEntry::new(self.config.prefetch_candidates_per_entry()),
            );
        }
        self.touch_structural_mapping_lru(key);
        let mapping = &mut self
            .structural_mapping_cache
            .get_mut(&key)
            .expect("structural mapping entry exists after insertion")
            .mappings[index];
        mapping.address = physical_block;
        mapping.reset_to_one();
    }

    fn trim_physical_mapping_for_insert(&mut self) {
        while self.physical_mapping_cache.len() >= self.config.address_map_entries() {
            let Some(victim) = self.physical_mapping_lru.pop_back() else {
                break;
            };
            self.physical_mapping_cache.remove(&victim);
        }
    }

    fn trim_structural_mapping_for_insert(&mut self) {
        while self.structural_mapping_cache.len() >= self.config.address_map_entries() {
            let Some(victim) = self.structural_mapping_lru.pop_back() else {
                break;
            };
            self.structural_mapping_cache.remove(&victim);
        }
    }

    fn touch_physical_mapping_lru(&mut self, key: MappingKey) {
        self.physical_mapping_lru.retain(|entry| *entry != key);
        self.physical_mapping_lru.push_front(key);
    }

    fn touch_structural_mapping_lru(&mut self, key: MappingKey) {
        self.structural_mapping_lru.retain(|entry| *entry != key);
        self.structural_mapping_lru.push_front(key);
    }

    fn block_number(&self, address: Address) -> u64 {
        address.get() / self.config.line_size()
    }

    fn block_address(&self, address: Address) -> Address {
        Address::new(self.block_number(address) * self.config.line_size())
    }

    fn mapping_key_and_index(&self, block: u64, secure: bool) -> (MappingKey, usize) {
        let candidates = self.config.candidates_per_entry_u64();
        (
            MappingKey {
                amc_address: block / candidates,
                secure,
            },
            (block % candidates) as usize,
        )
    }
}

fn validate_mapping_entries(
    config: &IrregularStreamBufferConfig,
    entries: &[IrregularStreamBufferMappingEntrySnapshot],
) -> Result<(), IrregularStreamBufferError> {
    for entry in entries {
        if entry.mappings().len() != config.prefetch_candidates_per_entry() {
            return Err(IrregularStreamBufferError::SnapshotMappingShapeMismatch {
                amc_address: entry.amc_address(),
                entries: entry.mappings().len(),
                expected: config.prefetch_candidates_per_entry(),
            });
        }
        for (index, mapping) in entry.mappings().iter().enumerate() {
            if mapping.counter() > config.max_counter() {
                return Err(IrregularStreamBufferError::SnapshotCounterOutOfRange {
                    amc_address: entry.amc_address(),
                    index,
                    counter: mapping.counter(),
                    max_counter: config.max_counter(),
                });
            }
        }
    }
    Ok(())
}

fn byte_stride(source_address: Address, candidate_address: Address) -> i64 {
    let stride = candidate_address.get() as i128 - source_address.get() as i128;
    stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}
