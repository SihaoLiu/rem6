use std::error::Error;
use std::fmt;

use rem6_memory::Address;

const WEAK_NOT_TAKEN: u8 = 1;
const TAKEN_THRESHOLD: u8 = 2;
const STRONGLY_TAKEN: u8 = 3;
const DEFAULT_HISTORY_BITS: u8 = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchPredictorError {
    ZeroTableEntries,
    HistoryBitsOutOfRange {
        bits: u8,
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
