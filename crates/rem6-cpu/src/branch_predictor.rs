use std::error::Error;
use std::fmt;

use rem6_memory::Address;

const WEAK_NOT_TAKEN: u8 = 1;
const TAKEN_THRESHOLD: u8 = 2;
const STRONGLY_TAKEN: u8 = 3;
const DEFAULT_HISTORY_BITS: u8 = 64;
const DEFAULT_MAX_BRANCH_TARGET_BUFFER_ENTRIES: usize = 4096;
const DEFAULT_MAX_BRANCH_TARGET_BUFFER_ASSOCIATIVITY: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchPredictorError {
    ZeroTableEntries,
    HistoryBitsOutOfRange {
        bits: u8,
    },
    ReturnTargetProtectionDisabled {
        profile: BranchTargetSafetyProfile,
        return_address_stack_enabled: bool,
        indirect_targets_hashed: bool,
    },
    SnapshotTableEntriesMismatch {
        expected: usize,
        actual: usize,
    },
    SnapshotHistoryBitsMismatch {
        expected: u8,
        actual: u8,
    },
    UnknownSpeculation {
        id: BranchSpeculationId,
    },
    OutOfOrderSpeculationCommit {
        expected: BranchSpeculationId,
        actual: BranchSpeculationId,
    },
}

impl fmt::Display for BranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTableEntries => write!(formatter, "branch predictor table is empty"),
            Self::HistoryBitsOutOfRange { bits } => write!(
                formatter,
                "branch predictor history length {bits} is outside 1..=64"
            ),
            Self::ReturnTargetProtectionDisabled {
                profile,
                return_address_stack_enabled,
                indirect_targets_hashed,
            } => write!(
                formatter,
                "{profile} branch target safety requires RAS or indirect target hashing, got ras={return_address_stack_enabled} indirect_hash_targets={indirect_targets_hashed}"
            ),
            Self::SnapshotTableEntriesMismatch { expected, actual } => write!(
                formatter,
                "branch predictor snapshot has {actual} entries but predictor has {expected}"
            ),
            Self::SnapshotHistoryBitsMismatch { expected, actual } => write!(
                formatter,
                "branch predictor snapshot has {actual} history bits but predictor has {expected}"
            ),
            Self::UnknownSpeculation { id } => write!(
                formatter,
                "branch predictor speculation {} is not pending",
                id.get()
            ),
            Self::OutOfOrderSpeculationCommit { expected, actual } => write!(
                formatter,
                "branch predictor speculation {} cannot commit before pending speculation {}",
                actual.get(),
                expected.get()
            ),
        }
    }
}

impl Error for BranchPredictorError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BranchTargetSafetyProfile {
    RiscvO3FullSystem,
}

impl fmt::Display for BranchTargetSafetyProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RiscvO3FullSystem => write!(formatter, "RISC-V O3 full-system"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchTargetSafetyConfig {
    profile: BranchTargetSafetyProfile,
    return_address_stack_enabled: bool,
    indirect_targets_hashed: bool,
}

impl BranchTargetSafetyConfig {
    pub fn riscv_o3_full_system(
        return_address_stack_enabled: bool,
        indirect_targets_hashed: bool,
    ) -> Result<Self, BranchPredictorError> {
        let config = Self {
            profile: BranchTargetSafetyProfile::RiscvO3FullSystem,
            return_address_stack_enabled,
            indirect_targets_hashed,
        };
        if !config.return_target_protected() {
            return Err(BranchPredictorError::ReturnTargetProtectionDisabled {
                profile: config.profile,
                return_address_stack_enabled,
                indirect_targets_hashed,
            });
        }
        Ok(config)
    }

    pub const fn profile(self) -> BranchTargetSafetyProfile {
        self.profile
    }

    pub const fn return_address_stack_enabled(self) -> bool {
        self.return_address_stack_enabled
    }

    pub const fn indirect_targets_hashed(self) -> bool {
        self.indirect_targets_hashed
    }

    pub const fn return_target_protected(self) -> bool {
        self.return_address_stack_enabled || self.indirect_targets_hashed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictorConfig {
    table_entries: usize,
    history_bits: u8,
}

impl BranchPredictorConfig {
    pub fn new(table_entries: usize) -> Result<Self, BranchPredictorError> {
        Self::with_history_bits(table_entries, DEFAULT_HISTORY_BITS)
    }

    pub fn with_history_bits(
        table_entries: usize,
        history_bits: u8,
    ) -> Result<Self, BranchPredictorError> {
        if table_entries == 0 {
            return Err(BranchPredictorError::ZeroTableEntries);
        }
        if !(1..=64).contains(&history_bits) {
            return Err(BranchPredictorError::HistoryBitsOutOfRange { bits: history_bits });
        }

        Ok(Self {
            table_entries,
            history_bits,
        })
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }

    pub const fn history_bits(&self) -> u8 {
        self.history_bits
    }

    fn history_mask(&self) -> u64 {
        history_mask(self.history_bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictor {
    config: BranchPredictorConfig,
    counters: Vec<u8>,
    targets: Vec<Option<Address>>,
    update_count: u64,
    committed_history: u64,
    speculative_history: u64,
    next_speculation: BranchSpeculationId,
    pending_speculations: Vec<BranchSpeculation>,
}

impl BranchPredictor {
    pub fn new(config: BranchPredictorConfig) -> Self {
        Self {
            counters: vec![WEAK_NOT_TAKEN; config.table_entries()],
            targets: vec![None; config.table_entries()],
            config,
            update_count: 0,
            committed_history: 0,
            speculative_history: 0,
            next_speculation: BranchSpeculationId::new(0),
            pending_speculations: Vec::new(),
        }
    }

    pub const fn config(&self) -> &BranchPredictorConfig {
        &self.config
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn committed_history(&self) -> u64 {
        self.committed_history
    }

    pub const fn speculative_history(&self) -> u64 {
        self.speculative_history
    }

    pub fn pending_speculations(&self) -> &[BranchSpeculation] {
        &self.pending_speculations
    }

    pub fn pending_speculation_count(&self) -> usize {
        self.pending_speculations.len()
    }

    pub fn predict(&self, pc: Address) -> BranchPrediction {
        let index = self.index(pc);
        let counter = self.counters[index];
        let predicted_taken = counter >= TAKEN_THRESHOLD;
        let target = predicted_taken.then_some(self.targets[index]).flatten();

        BranchPrediction {
            pc,
            index,
            predicted_taken,
            target,
            counter,
        }
    }

    pub fn predict_speculative(&mut self, pc: Address) -> BranchSpeculation {
        let prediction = self.predict(pc);
        let history_before = self.speculative_history;
        let history_taken = prediction.predicted_taken();
        let history_after = self.shift_history(history_before, history_taken);
        let speculation = BranchSpeculation {
            id: self.next_speculation,
            prediction,
            history_before,
            history_after,
            history_taken,
            repaired: false,
        };

        self.next_speculation = BranchSpeculationId::new(self.next_speculation.get() + 1);
        self.speculative_history = history_after;
        self.pending_speculations.push(speculation.clone());
        speculation
    }

    pub fn update(
        &mut self,
        pc: Address,
        actual_taken: bool,
        actual_target: Option<Address>,
    ) -> BranchUpdate {
        let prediction = self.predict(pc);
        let new_counter = saturating_branch_counter(prediction.counter(), actual_taken);

        self.counters[prediction.index()] = new_counter;
        if actual_taken {
            self.targets[prediction.index()] = actual_target;
        }
        self.update_count += 1;

        BranchUpdate {
            prediction,
            actual_taken,
            actual_target,
            new_counter,
            update_count: self.update_count,
        }
    }

    pub fn commit_speculation(
        &mut self,
        id: BranchSpeculationId,
    ) -> Result<BranchSpeculation, BranchPredictorError> {
        let Some(oldest) = self.pending_speculations.first() else {
            return Err(BranchPredictorError::UnknownSpeculation { id });
        };

        if oldest.id() != id {
            return Err(BranchPredictorError::OutOfOrderSpeculationCommit {
                expected: oldest.id(),
                actual: id,
            });
        }

        let committed = self.pending_speculations.remove(0);
        self.committed_history = committed.history_after();
        if self.pending_speculations.is_empty() {
            self.speculative_history = self.committed_history;
        }
        Ok(committed)
    }

    pub fn repair_speculation(
        &mut self,
        id: BranchSpeculationId,
        actual_taken: bool,
    ) -> Result<BranchSpeculationRepair, BranchPredictorError> {
        let Some(index) = self
            .pending_speculations
            .iter()
            .position(|speculation| speculation.id() == id)
        else {
            return Err(BranchPredictorError::UnknownSpeculation { id });
        };

        let removed_youngers = self.pending_speculations.split_off(index + 1);
        let old_history_after = self.pending_speculations[index].history_after();
        let history_before = self.pending_speculations[index].history_before();
        let new_history_after = self.shift_history(history_before, actual_taken);

        let repaired = &mut self.pending_speculations[index];
        repaired.history_taken = actual_taken;
        repaired.history_after = new_history_after;
        repaired.repaired = true;

        let repaired = repaired.clone();
        self.speculative_history = new_history_after;

        Ok(BranchSpeculationRepair {
            repaired,
            removed_youngers,
            history_before,
            old_history_after,
            new_history_after,
        })
    }

    pub fn snapshot(&self) -> BranchPredictorSnapshot {
        BranchPredictorSnapshot {
            config: self.config.clone(),
            counters: self.counters.clone(),
            targets: self.targets.clone(),
            update_count: self.update_count,
            committed_history: self.committed_history,
            speculative_history: self.speculative_history,
            next_speculation: self.next_speculation,
            pending_speculations: self.pending_speculations.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &BranchPredictorSnapshot,
    ) -> Result<(), BranchPredictorError> {
        if snapshot.config.table_entries() != self.config.table_entries() {
            return Err(BranchPredictorError::SnapshotTableEntriesMismatch {
                expected: self.config.table_entries(),
                actual: snapshot.config.table_entries(),
            });
        }
        if snapshot.config.history_bits() != self.config.history_bits() {
            return Err(BranchPredictorError::SnapshotHistoryBitsMismatch {
                expected: self.config.history_bits(),
                actual: snapshot.config.history_bits(),
            });
        }

        self.counters.clone_from(&snapshot.counters);
        self.targets.clone_from(&snapshot.targets);
        self.update_count = snapshot.update_count;
        self.committed_history = snapshot.committed_history;
        self.speculative_history = snapshot.speculative_history;
        self.next_speculation = snapshot.next_speculation;
        self.pending_speculations
            .clone_from(&snapshot.pending_speculations);
        Ok(())
    }

    fn index(&self, pc: Address) -> usize {
        ((pc.get() >> 2) % self.config.table_entries() as u64) as usize
    }

    fn shift_history(&self, history: u64, taken: bool) -> u64 {
        ((history << 1) | u64::from(taken)) & self.config.history_mask()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BranchSpeculationId(u64);

impl BranchSpeculationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPrediction {
    pc: Address,
    index: usize,
    predicted_taken: bool,
    target: Option<Address>,
    counter: u8,
}

impl BranchPrediction {
    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn target(&self) -> Option<Address> {
        self.target
    }

    pub const fn counter(&self) -> u8 {
        self.counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchSpeculation {
    id: BranchSpeculationId,
    prediction: BranchPrediction,
    history_before: u64,
    history_after: u64,
    history_taken: bool,
    repaired: bool,
}

impl BranchSpeculation {
    pub const fn id(&self) -> BranchSpeculationId {
        self.id
    }

    pub const fn prediction(&self) -> &BranchPrediction {
        &self.prediction
    }

    pub const fn pc(&self) -> Address {
        self.prediction.pc()
    }

    pub const fn index(&self) -> usize {
        self.prediction.index()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.prediction.predicted_taken()
    }

    pub const fn target(&self) -> Option<Address> {
        self.prediction.target()
    }

    pub const fn counter(&self) -> u8 {
        self.prediction.counter()
    }

    pub const fn history_before(&self) -> u64 {
        self.history_before
    }

    pub const fn history_after(&self) -> u64 {
        self.history_after
    }

    pub const fn history_taken(&self) -> bool {
        self.history_taken
    }

    pub const fn repaired(&self) -> bool {
        self.repaired
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchSpeculationRepair {
    repaired: BranchSpeculation,
    removed_youngers: Vec<BranchSpeculation>,
    history_before: u64,
    old_history_after: u64,
    new_history_after: u64,
}

impl BranchSpeculationRepair {
    pub const fn repaired(&self) -> &BranchSpeculation {
        &self.repaired
    }

    pub fn removed_youngers(&self) -> &[BranchSpeculation] {
        &self.removed_youngers
    }

    pub const fn history_before(&self) -> u64 {
        self.history_before
    }

    pub const fn old_history_after(&self) -> u64 {
        self.old_history_after
    }

    pub const fn new_history_after(&self) -> u64 {
        self.new_history_after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchUpdate {
    prediction: BranchPrediction,
    actual_taken: bool,
    actual_target: Option<Address>,
    new_counter: u8,
    update_count: u64,
}

impl BranchUpdate {
    pub const fn pc(&self) -> Address {
        self.prediction.pc()
    }

    pub const fn index(&self) -> usize {
        self.prediction.index()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.prediction.predicted_taken()
    }

    pub const fn actual_taken(&self) -> bool {
        self.actual_taken
    }

    pub const fn actual_target(&self) -> Option<Address> {
        self.actual_target
    }

    pub const fn old_counter(&self) -> u8 {
        self.prediction.counter()
    }

    pub const fn new_counter(&self) -> u8 {
        self.new_counter
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictorSnapshot {
    config: BranchPredictorConfig,
    counters: Vec<u8>,
    targets: Vec<Option<Address>>,
    update_count: u64,
    committed_history: u64,
    speculative_history: u64,
    next_speculation: BranchSpeculationId,
    pending_speculations: Vec<BranchSpeculation>,
}

impl BranchPredictorSnapshot {
    pub const fn config(&self) -> &BranchPredictorConfig {
        &self.config
    }

    pub fn counters(&self) -> &[u8] {
        &self.counters
    }

    pub fn targets(&self) -> &[Option<Address>] {
        &self.targets
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn committed_history(&self) -> u64 {
        self.committed_history
    }

    pub const fn speculative_history(&self) -> u64 {
        self.speculative_history
    }

    pub const fn next_speculation(&self) -> BranchSpeculationId {
        self.next_speculation
    }

    pub fn pending_speculations(&self) -> &[BranchSpeculation] {
        &self.pending_speculations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchTargetBufferError {
    ZeroEntries,
    ZeroAssociativity,
    EntriesExceedLimit {
        entries: usize,
        max_entries: usize,
    },
    AssociativityExceedsLimit {
        associativity: usize,
        max_associativity: usize,
    },
    AssociativityExceedsEntries {
        entries: usize,
        associativity: usize,
    },
    EntriesNotDivisibleByAssociativity {
        entries: usize,
        associativity: usize,
    },
    SetCountNotPowerOfTwo {
        sets: usize,
    },
    SnapshotShapeMismatch {
        expected_entries: usize,
        expected_associativity: usize,
        actual_entries: usize,
        actual_associativity: usize,
    },
}

impl fmt::Display for BranchTargetBufferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEntries => write!(formatter, "branch target buffer is empty"),
            Self::ZeroAssociativity => write!(
                formatter,
                "branch target buffer associativity must be non-zero"
            ),
            Self::EntriesExceedLimit {
                entries,
                max_entries,
            } => write!(
                formatter,
                "branch target buffer entries {entries} exceed limit {max_entries}"
            ),
            Self::AssociativityExceedsLimit {
                associativity,
                max_associativity,
            } => write!(
                formatter,
                "branch target buffer associativity {associativity} exceeds limit {max_associativity}"
            ),
            Self::AssociativityExceedsEntries {
                entries,
                associativity,
            } => write!(
                formatter,
                "branch target buffer associativity {associativity} exceeds entries {entries}"
            ),
            Self::EntriesNotDivisibleByAssociativity {
                entries,
                associativity,
            } => write!(
                formatter,
                "branch target buffer entries {entries} are not divisible by associativity {associativity}"
            ),
            Self::SetCountNotPowerOfTwo { sets } => write!(
                formatter,
                "branch target buffer set count {sets} is not a power of two"
            ),
            Self::SnapshotShapeMismatch {
                expected_entries,
                expected_associativity,
                actual_entries,
                actual_associativity,
            } => write!(
                formatter,
                "branch target buffer snapshot has {actual_entries} entries and associativity {actual_associativity} but buffer has {expected_entries} entries and associativity {expected_associativity}"
            ),
        }
    }
}

impl Error for BranchTargetBufferError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTargetBufferConfig {
    entries: usize,
    associativity: usize,
    sets: usize,
}

impl BranchTargetBufferConfig {
    pub fn new(entries: usize, associativity: usize) -> Result<Self, BranchTargetBufferError> {
        Self::with_limits(
            entries,
            associativity,
            DEFAULT_MAX_BRANCH_TARGET_BUFFER_ENTRIES,
            DEFAULT_MAX_BRANCH_TARGET_BUFFER_ASSOCIATIVITY,
        )
    }

    pub fn with_limits(
        entries: usize,
        associativity: usize,
        max_entries: usize,
        max_associativity: usize,
    ) -> Result<Self, BranchTargetBufferError> {
        if entries == 0 {
            return Err(BranchTargetBufferError::ZeroEntries);
        }
        if associativity == 0 {
            return Err(BranchTargetBufferError::ZeroAssociativity);
        }
        if entries > max_entries {
            return Err(BranchTargetBufferError::EntriesExceedLimit {
                entries,
                max_entries,
            });
        }
        if associativity > max_associativity {
            return Err(BranchTargetBufferError::AssociativityExceedsLimit {
                associativity,
                max_associativity,
            });
        }
        if associativity > entries {
            return Err(BranchTargetBufferError::AssociativityExceedsEntries {
                entries,
                associativity,
            });
        }
        if !entries.is_multiple_of(associativity) {
            return Err(
                BranchTargetBufferError::EntriesNotDivisibleByAssociativity {
                    entries,
                    associativity,
                },
            );
        }

        let sets = entries / associativity;
        if !sets.is_power_of_two() {
            return Err(BranchTargetBufferError::SetCountNotPowerOfTwo { sets });
        }

        Ok(Self {
            entries,
            associativity,
            sets,
        })
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }

    pub const fn associativity(&self) -> usize {
        self.associativity
    }

    pub const fn sets(&self) -> usize {
        self.sets
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchTargetKind {
    NoBranch,
    DirectConditional,
    DirectUnconditional,
    IndirectConditional,
    IndirectUnconditional,
    CallDirect,
    CallIndirect,
    Return,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTargetBuffer {
    config: BranchTargetBufferConfig,
    entries: Vec<Option<BranchTargetEntry>>,
    access_sequence: u64,
    lookup_count: u64,
    hit_count: u64,
    miss_count: u64,
    update_count: u64,
    eviction_count: u64,
}

impl BranchTargetBuffer {
    pub fn new(config: BranchTargetBufferConfig) -> Self {
        let entries = vec![None; config.entries()];
        Self {
            config,
            entries,
            access_sequence: 0,
            lookup_count: 0,
            hit_count: 0,
            miss_count: 0,
            update_count: 0,
            eviction_count: 0,
        }
    }

    pub const fn config(&self) -> &BranchTargetBufferConfig {
        &self.config
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn hit_count(&self) -> u64 {
        self.hit_count
    }

    pub const fn miss_count(&self) -> u64 {
        self.miss_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn eviction_count(&self) -> u64 {
        self.eviction_count
    }

    pub fn valid(&self, pc: Address) -> bool {
        self.find_entry(pc).is_some()
    }

    pub fn lookup(&mut self, pc: Address, kind: BranchTargetKind) -> BranchTargetLookup {
        self.lookup_count += 1;
        let set = self.set_index(pc);
        let mut hit = None;

        for way in 0..self.config.associativity() {
            let index = self.entry_index(set, way);
            let Some(entry) = &self.entries[index] else {
                continue;
            };
            if entry.pc() == pc {
                hit = Some((index, way));
                break;
            }
        }

        match hit {
            Some((index, way)) => {
                self.hit_count += 1;
                let access_sequence = self.next_access_sequence();
                let entry = self.entries[index]
                    .as_mut()
                    .expect("hit index contains entry");
                entry.last_used = access_sequence;
                let entry = entry.clone();
                BranchTargetLookup {
                    pc,
                    kind,
                    set,
                    way: Some(way),
                    hit: true,
                    entry: Some(entry.clone()),
                    target: Some(entry.target()),
                    lookup_count: self.lookup_count,
                }
            }
            None => {
                self.miss_count += 1;
                BranchTargetLookup {
                    pc,
                    kind,
                    set,
                    way: None,
                    hit: false,
                    entry: None,
                    target: None,
                    lookup_count: self.lookup_count,
                }
            }
        }
    }

    pub fn update(
        &mut self,
        pc: Address,
        target: Address,
        kind: BranchTargetKind,
    ) -> BranchTargetUpdate {
        self.update_count += 1;
        let set = self.set_index(pc);
        let access_sequence = self.next_access_sequence();

        for way in 0..self.config.associativity() {
            let index = self.entry_index(set, way);
            let Some(entry) = &mut self.entries[index] else {
                continue;
            };
            if entry.pc() == pc {
                entry.target = target;
                entry.kind = kind;
                entry.last_used = access_sequence;
                return BranchTargetUpdate {
                    pc,
                    target,
                    kind,
                    set,
                    way,
                    replaced: None,
                    update_count: self.update_count,
                };
            }
        }

        let (way, index, replaced) = self.replacement_slot(set);
        let entry = BranchTargetEntry {
            pc,
            target,
            kind,
            set,
            way,
            last_used: access_sequence,
        };
        self.entries[index] = Some(entry);
        if replaced.is_some() {
            self.eviction_count += 1;
        }

        BranchTargetUpdate {
            pc,
            target,
            kind,
            set,
            way,
            replaced,
            update_count: self.update_count,
        }
    }

    pub fn invalidate(&mut self) {
        self.entries.fill(None);
    }

    pub fn snapshot(&self) -> BranchTargetBufferSnapshot {
        BranchTargetBufferSnapshot {
            config: self.config.clone(),
            entries: self.entries.clone(),
            access_sequence: self.access_sequence,
            lookup_count: self.lookup_count,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            update_count: self.update_count,
            eviction_count: self.eviction_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &BranchTargetBufferSnapshot,
    ) -> Result<(), BranchTargetBufferError> {
        if snapshot.config.entries() != self.config.entries()
            || snapshot.config.associativity() != self.config.associativity()
        {
            return Err(BranchTargetBufferError::SnapshotShapeMismatch {
                expected_entries: self.config.entries(),
                expected_associativity: self.config.associativity(),
                actual_entries: snapshot.config.entries(),
                actual_associativity: snapshot.config.associativity(),
            });
        }

        self.entries.clone_from(&snapshot.entries);
        self.access_sequence = snapshot.access_sequence;
        self.lookup_count = snapshot.lookup_count;
        self.hit_count = snapshot.hit_count;
        self.miss_count = snapshot.miss_count;
        self.update_count = snapshot.update_count;
        self.eviction_count = snapshot.eviction_count;
        Ok(())
    }

    fn find_entry(&self, pc: Address) -> Option<&BranchTargetEntry> {
        let set = self.set_index(pc);
        (0..self.config.associativity())
            .map(|way| self.entry_index(set, way))
            .filter_map(|index| self.entries[index].as_ref())
            .find(|entry| entry.pc() == pc)
    }

    fn replacement_slot(&self, set: usize) -> (usize, usize, Option<BranchTargetEntry>) {
        for way in 0..self.config.associativity() {
            let index = self.entry_index(set, way);
            if self.entries[index].is_none() {
                return (way, index, None);
            }
        }

        let victim_way = (0..self.config.associativity())
            .min_by_key(|way| {
                self.entries[self.entry_index(set, *way)]
                    .as_ref()
                    .expect("full set has entry")
                    .last_used
            })
            .expect("associativity is non-zero");
        let victim_index = self.entry_index(set, victim_way);
        (victim_way, victim_index, self.entries[victim_index].clone())
    }

    fn set_index(&self, pc: Address) -> usize {
        ((pc.get() >> 2) % self.config.sets() as u64) as usize
    }

    fn entry_index(&self, set: usize, way: usize) -> usize {
        set * self.config.associativity() + way
    }

    fn next_access_sequence(&mut self) -> u64 {
        let access_sequence = self.access_sequence;
        self.access_sequence = self.access_sequence.saturating_add(1);
        access_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTargetEntry {
    pc: Address,
    target: Address,
    kind: BranchTargetKind,
    set: usize,
    way: usize,
    last_used: u64,
}

impl BranchTargetEntry {
    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn target(&self) -> Address {
        self.target
    }

    pub const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn last_used(&self) -> u64 {
        self.last_used
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTargetLookup {
    pc: Address,
    kind: BranchTargetKind,
    set: usize,
    way: Option<usize>,
    hit: bool,
    entry: Option<BranchTargetEntry>,
    target: Option<Address>,
    lookup_count: u64,
}

impl BranchTargetLookup {
    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> Option<usize> {
        self.way
    }

    pub const fn hit(&self) -> bool {
        self.hit
    }

    pub const fn entry(&self) -> Option<&BranchTargetEntry> {
        self.entry.as_ref()
    }

    pub const fn target(&self) -> Option<Address> {
        self.target
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTargetUpdate {
    pc: Address,
    target: Address,
    kind: BranchTargetKind,
    set: usize,
    way: usize,
    replaced: Option<BranchTargetEntry>,
    update_count: u64,
}

impl BranchTargetUpdate {
    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn target(&self) -> Address {
        self.target
    }

    pub const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn replaced(&self) -> Option<&BranchTargetEntry> {
        self.replaced.as_ref()
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchTargetBufferSnapshot {
    config: BranchTargetBufferConfig,
    entries: Vec<Option<BranchTargetEntry>>,
    access_sequence: u64,
    lookup_count: u64,
    hit_count: u64,
    miss_count: u64,
    update_count: u64,
    eviction_count: u64,
}

impl BranchTargetBufferSnapshot {
    pub const fn config(&self) -> &BranchTargetBufferConfig {
        &self.config
    }

    pub fn entries(&self) -> &[Option<BranchTargetEntry>] {
        &self.entries
    }

    pub const fn access_sequence(&self) -> u64 {
        self.access_sequence
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn hit_count(&self) -> u64 {
        self.hit_count
    }

    pub const fn miss_count(&self) -> u64 {
        self.miss_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn eviction_count(&self) -> u64 {
        self.eviction_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReturnAddressStackError {
    ZeroEntries,
    SnapshotEntriesMismatch {
        expected: usize,
        actual: usize,
    },
    UnknownOperation {
        id: ReturnAddressStackOperationId,
    },
    OutOfOrderOperationCommit {
        expected: ReturnAddressStackOperationId,
        actual: ReturnAddressStackOperationId,
    },
}

impl fmt::Display for ReturnAddressStackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEntries => write!(formatter, "return address stack is empty"),
            Self::SnapshotEntriesMismatch { expected, actual } => write!(
                formatter,
                "return address stack snapshot has {actual} entries but stack has {expected}"
            ),
            Self::UnknownOperation { id } => write!(
                formatter,
                "return address stack operation {} is not pending",
                id.get()
            ),
            Self::OutOfOrderOperationCommit { expected, actual } => write!(
                formatter,
                "return address stack operation {} cannot commit before pending operation {}",
                actual.get(),
                expected.get()
            ),
        }
    }
}

impl Error for ReturnAddressStackError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackConfig {
    entries: usize,
}

impl ReturnAddressStackConfig {
    pub fn new(entries: usize) -> Result<Self, ReturnAddressStackError> {
        if entries == 0 {
            return Err(ReturnAddressStackError::ZeroEntries);
        }

        Ok(Self { entries })
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStack {
    config: ReturnAddressStackConfig,
    stack: Vec<Address>,
    next_operation: ReturnAddressStackOperationId,
    pending_operations: Vec<ReturnAddressStackOperation>,
}

impl ReturnAddressStack {
    pub fn new(config: ReturnAddressStackConfig) -> Self {
        Self {
            config,
            stack: Vec::new(),
            next_operation: ReturnAddressStackOperationId::new(0),
            pending_operations: Vec::new(),
        }
    }

    pub const fn config(&self) -> &ReturnAddressStackConfig {
        &self.config
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn top(&self) -> Option<Address> {
        self.stack.last().copied()
    }

    pub fn stack_entries(&self) -> &[Address] {
        &self.stack
    }

    pub const fn next_operation(&self) -> ReturnAddressStackOperationId {
        self.next_operation
    }

    pub fn pending_operations(&self) -> &[ReturnAddressStackOperation] {
        &self.pending_operations
    }

    pub fn pending_operation_count(&self) -> usize {
        self.pending_operations.len()
    }

    pub fn push_speculative(&mut self, return_address: Address) -> ReturnAddressStackOperation {
        let stack_before = self.stack.clone();
        if self.stack.len() == self.config.entries() {
            self.stack.remove(0);
        }
        self.stack.push(return_address);
        let stack_after = self.stack.clone();

        self.record_operation(
            ReturnAddressStackOperationKind::Push,
            Some(return_address),
            None,
            stack_before,
            stack_after,
        )
    }

    pub fn pop_speculative(&mut self) -> ReturnAddressStackOperation {
        let stack_before = self.stack.clone();
        let predicted_return = self.stack.pop();
        let stack_after = self.stack.clone();

        self.record_operation(
            ReturnAddressStackOperationKind::Pop,
            None,
            predicted_return,
            stack_before,
            stack_after,
        )
    }

    pub fn commit_operation(
        &mut self,
        id: ReturnAddressStackOperationId,
    ) -> Result<ReturnAddressStackOperation, ReturnAddressStackError> {
        let Some(oldest) = self.pending_operations.first() else {
            return Err(ReturnAddressStackError::UnknownOperation { id });
        };

        if oldest.id() != id {
            return Err(ReturnAddressStackError::OutOfOrderOperationCommit {
                expected: oldest.id(),
                actual: id,
            });
        }

        Ok(self.pending_operations.remove(0))
    }

    pub fn squash_from(
        &mut self,
        id: ReturnAddressStackOperationId,
    ) -> Result<ReturnAddressStackRepair, ReturnAddressStackError> {
        let Some(index) = self
            .pending_operations
            .iter()
            .position(|operation| operation.id() == id)
        else {
            return Err(ReturnAddressStackError::UnknownOperation { id });
        };

        let mut removed = self.pending_operations.split_off(index);
        let reverted = removed.remove(0);
        let removed_youngers = removed;
        self.stack.clone_from(&reverted.stack_before);

        Ok(ReturnAddressStackRepair {
            restored_stack: self.stack.clone(),
            reverted,
            removed_youngers,
        })
    }

    pub fn snapshot(&self) -> ReturnAddressStackSnapshot {
        ReturnAddressStackSnapshot {
            config: self.config.clone(),
            stack: self.stack.clone(),
            next_operation: self.next_operation,
            pending_operations: self.pending_operations.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &ReturnAddressStackSnapshot,
    ) -> Result<(), ReturnAddressStackError> {
        if snapshot.config.entries() != self.config.entries() {
            return Err(ReturnAddressStackError::SnapshotEntriesMismatch {
                expected: self.config.entries(),
                actual: snapshot.config.entries(),
            });
        }

        self.stack.clone_from(&snapshot.stack);
        self.next_operation = snapshot.next_operation;
        self.pending_operations
            .clone_from(&snapshot.pending_operations);
        Ok(())
    }

    fn record_operation(
        &mut self,
        kind: ReturnAddressStackOperationKind,
        pushed_address: Option<Address>,
        predicted_return: Option<Address>,
        stack_before: Vec<Address>,
        stack_after: Vec<Address>,
    ) -> ReturnAddressStackOperation {
        let operation = ReturnAddressStackOperation {
            id: self.next_operation,
            kind,
            pushed_address,
            predicted_return,
            stack_before,
            stack_after,
        };
        self.next_operation = ReturnAddressStackOperationId::new(self.next_operation.get() + 1);
        self.pending_operations.push(operation.clone());
        operation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReturnAddressStackOperationId(u64);

impl ReturnAddressStackOperationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnAddressStackOperationKind {
    Push,
    Pop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackOperation {
    id: ReturnAddressStackOperationId,
    kind: ReturnAddressStackOperationKind,
    pushed_address: Option<Address>,
    predicted_return: Option<Address>,
    stack_before: Vec<Address>,
    stack_after: Vec<Address>,
}

impl ReturnAddressStackOperation {
    pub const fn id(&self) -> ReturnAddressStackOperationId {
        self.id
    }

    pub const fn kind(&self) -> ReturnAddressStackOperationKind {
        self.kind
    }

    pub const fn pushed_address(&self) -> Option<Address> {
        self.pushed_address
    }

    pub const fn predicted_return(&self) -> Option<Address> {
        self.predicted_return
    }

    pub fn stack_before(&self) -> &[Address] {
        &self.stack_before
    }

    pub fn stack_after(&self) -> &[Address] {
        &self.stack_after
    }

    pub fn depth_before(&self) -> usize {
        self.stack_before.len()
    }

    pub fn depth_after(&self) -> usize {
        self.stack_after.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackRepair {
    restored_stack: Vec<Address>,
    reverted: ReturnAddressStackOperation,
    removed_youngers: Vec<ReturnAddressStackOperation>,
}

impl ReturnAddressStackRepair {
    pub fn restored_stack(&self) -> &[Address] {
        &self.restored_stack
    }

    pub const fn reverted(&self) -> &ReturnAddressStackOperation {
        &self.reverted
    }

    pub fn removed_youngers(&self) -> &[ReturnAddressStackOperation] {
        &self.removed_youngers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnAddressStackSnapshot {
    config: ReturnAddressStackConfig,
    stack: Vec<Address>,
    next_operation: ReturnAddressStackOperationId,
    pending_operations: Vec<ReturnAddressStackOperation>,
}

impl ReturnAddressStackSnapshot {
    pub const fn config(&self) -> &ReturnAddressStackConfig {
        &self.config
    }

    pub fn stack_entries(&self) -> &[Address] {
        &self.stack
    }

    pub const fn next_operation(&self) -> ReturnAddressStackOperationId {
        self.next_operation
    }

    pub fn pending_operations(&self) -> &[ReturnAddressStackOperation] {
        &self.pending_operations
    }
}

fn saturating_branch_counter(counter: u8, taken: bool) -> u8 {
    match taken {
        true => counter.saturating_add(1).min(STRONGLY_TAKEN),
        false => counter.saturating_sub(1),
    }
}

fn history_mask(bits: u8) -> u64 {
    if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}
