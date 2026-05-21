use std::error::Error;
use std::fmt;

use rem6_memory::Address;

const WEAK_NOT_TAKEN: u8 = 1;
const TAKEN_THRESHOLD: u8 = 2;
const STRONGLY_TAKEN: u8 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchPredictorError {
    ZeroTableEntries,
    SnapshotTableEntriesMismatch { expected: usize, actual: usize },
}

impl fmt::Display for BranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTableEntries => write!(formatter, "branch predictor table is empty"),
            Self::SnapshotTableEntriesMismatch { expected, actual } => write!(
                formatter,
                "branch predictor snapshot has {actual} entries but predictor has {expected}"
            ),
        }
    }
}

impl Error for BranchPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictorConfig {
    table_entries: usize,
}

impl BranchPredictorConfig {
    pub fn new(table_entries: usize) -> Result<Self, BranchPredictorError> {
        if table_entries == 0 {
            return Err(BranchPredictorError::ZeroTableEntries);
        }

        Ok(Self { table_entries })
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchPredictor {
    config: BranchPredictorConfig,
    counters: Vec<u8>,
    targets: Vec<Option<Address>>,
    update_count: u64,
}

impl BranchPredictor {
    pub fn new(config: BranchPredictorConfig) -> Self {
        Self {
            counters: vec![WEAK_NOT_TAKEN; config.table_entries()],
            targets: vec![None; config.table_entries()],
            config,
            update_count: 0,
        }
    }

    pub const fn config(&self) -> &BranchPredictorConfig {
        &self.config
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
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

    pub fn snapshot(&self) -> BranchPredictorSnapshot {
        BranchPredictorSnapshot {
            config: self.config.clone(),
            counters: self.counters.clone(),
            targets: self.targets.clone(),
            update_count: self.update_count,
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

        self.counters.clone_from(&snapshot.counters);
        self.targets.clone_from(&snapshot.targets);
        self.update_count = snapshot.update_count;
        Ok(())
    }

    fn index(&self, pc: Address) -> usize {
        ((pc.get() >> 2) % self.config.table_entries() as u64) as usize
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
}

fn saturating_branch_counter(counter: u8, taken: bool) -> u8 {
    match taken {
        true => counter.saturating_add(1).min(STRONGLY_TAKEN),
        false => counter.saturating_sub(1),
    }
}
