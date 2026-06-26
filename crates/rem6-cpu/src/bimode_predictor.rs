use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::CpuId;

const DEFAULT_COUNTER_BITS: u8 = 2;
const DEFAULT_INST_SHIFT: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BiModeBranchPredictorError {
    ZeroThreads,
    ZeroChoiceEntries,
    ChoiceEntriesNotPowerOfTwo {
        entries: usize,
    },
    ZeroGlobalEntries,
    GlobalEntriesNotPowerOfTwo {
        entries: usize,
    },
    ChoiceCounterBitsOutOfRange {
        bits: u8,
    },
    GlobalCounterBitsOutOfRange {
        bits: u8,
    },
    InstShiftOutOfRange {
        bits: u8,
    },
    UnknownThread {
        cpu: CpuId,
    },
    HistoryUpdateOutOfOrder {
        cpu: CpuId,
        expected_history: u64,
        actual_history: u64,
    },
    SnapshotShapeMismatch {
        expected_threads: usize,
        actual_threads: usize,
        expected_choice_entries: usize,
        actual_choice_entries: usize,
        expected_global_entries: usize,
        actual_global_entries: usize,
    },
    SnapshotConfigMismatch {
        expected: BiModeBranchPredictorConfig,
        actual: BiModeBranchPredictorConfig,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    CheckpointValueTooLarge {
        name: &'static str,
        value: usize,
        max: usize,
    },
    InvalidCheckpointCounter {
        table: &'static str,
        value: u8,
        max: u8,
    },
}

impl fmt::Display for BiModeBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "bimode predictor has no threads"),
            Self::ZeroChoiceEntries => write!(formatter, "bimode choice table is empty"),
            Self::ChoiceEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "bimode choice table entries {entries} are not a power of two"
            ),
            Self::ZeroGlobalEntries => write!(formatter, "bimode global table is empty"),
            Self::GlobalEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "bimode global table entries {entries} are not a power of two"
            ),
            Self::ChoiceCounterBitsOutOfRange { bits } => write!(
                formatter,
                "bimode choice counter width {bits} is outside 1..=8"
            ),
            Self::GlobalCounterBitsOutOfRange { bits } => write!(
                formatter,
                "bimode global counter width {bits} is outside 1..=8"
            ),
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "bimode instruction shift {bits} is outside 0..=63"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "bimode predictor thread {} is not configured",
                cpu.get()
            ),
            Self::HistoryUpdateOutOfOrder {
                cpu,
                expected_history,
                actual_history,
            } => write!(
                formatter,
                "bimode predictor thread {} history is {expected_history}, but update record starts from {actual_history}",
                cpu.get()
            ),
            Self::SnapshotShapeMismatch {
                expected_threads,
                actual_threads,
                expected_choice_entries,
                actual_choice_entries,
                expected_global_entries,
                actual_global_entries,
            } => write!(
                formatter,
                "bimode snapshot shape threads={actual_threads}, choice_entries={actual_choice_entries}, global_entries={actual_global_entries} does not match predictor threads={expected_threads}, choice_entries={expected_choice_entries}, global_entries={expected_global_entries}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "bimode snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "bimode checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "bimode checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "bimode checkpoint payload version {version} is not supported"
            ),
            Self::CheckpointValueTooLarge { name, value, max } => write!(
                formatter,
                "bimode checkpoint {name} value {value} exceeds maximum {max}"
            ),
            Self::InvalidCheckpointCounter { table, value, max } => write!(
                formatter,
                "bimode checkpoint {table} counter {value} exceeds maximum {max}"
            ),
        }
    }
}

impl Error for BiModeBranchPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeBranchPredictorConfig {
    threads: usize,
    choice_entries: usize,
    global_entries: usize,
    choice_counter_bits: u8,
    global_counter_bits: u8,
    inst_shift: u8,
    history_bits: u8,
}

impl BiModeBranchPredictorConfig {
    pub fn new(
        threads: usize,
        choice_entries: usize,
        global_entries: usize,
    ) -> Result<Self, BiModeBranchPredictorError> {
        Self::with_options(
            threads,
            choice_entries,
            global_entries,
            DEFAULT_COUNTER_BITS,
            DEFAULT_COUNTER_BITS,
            DEFAULT_INST_SHIFT,
        )
    }

    pub fn with_options(
        threads: usize,
        choice_entries: usize,
        global_entries: usize,
        choice_counter_bits: u8,
        global_counter_bits: u8,
        inst_shift: u8,
    ) -> Result<Self, BiModeBranchPredictorError> {
        if threads == 0 {
            return Err(BiModeBranchPredictorError::ZeroThreads);
        }
        if choice_entries == 0 {
            return Err(BiModeBranchPredictorError::ZeroChoiceEntries);
        }
        if !choice_entries.is_power_of_two() {
            return Err(BiModeBranchPredictorError::ChoiceEntriesNotPowerOfTwo {
                entries: choice_entries,
            });
        }
        if global_entries == 0 {
            return Err(BiModeBranchPredictorError::ZeroGlobalEntries);
        }
        if !global_entries.is_power_of_two() {
            return Err(BiModeBranchPredictorError::GlobalEntriesNotPowerOfTwo {
                entries: global_entries,
            });
        }
        if !(1..=8).contains(&choice_counter_bits) {
            return Err(BiModeBranchPredictorError::ChoiceCounterBitsOutOfRange {
                bits: choice_counter_bits,
            });
        }
        if !(1..=8).contains(&global_counter_bits) {
            return Err(BiModeBranchPredictorError::GlobalCounterBitsOutOfRange {
                bits: global_counter_bits,
            });
        }
        if inst_shift > 63 {
            return Err(BiModeBranchPredictorError::InstShiftOutOfRange { bits: inst_shift });
        }

        Ok(Self {
            threads,
            choice_entries,
            global_entries,
            choice_counter_bits,
            global_counter_bits,
            inst_shift,
            history_bits: global_entries.trailing_zeros() as u8,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn choice_entries(&self) -> usize {
        self.choice_entries
    }

    pub const fn global_entries(&self) -> usize {
        self.global_entries
    }

    pub const fn choice_counter_bits(&self) -> u8 {
        self.choice_counter_bits
    }

    pub const fn global_counter_bits(&self) -> u8 {
        self.global_counter_bits
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    pub const fn history_bits(&self) -> u8 {
        self.history_bits
    }

    fn history_mask(&self) -> u64 {
        self.global_entries as u64 - 1
    }

    fn choice_mask(&self) -> u64 {
        self.choice_entries as u64 - 1
    }

    fn choice_counter_max(&self) -> u8 {
        counter_max(self.choice_counter_bits)
    }

    fn global_counter_max(&self) -> u8 {
        counter_max(self.global_counter_bits)
    }

    fn choice_threshold(&self) -> u8 {
        taken_threshold(self.choice_counter_bits)
    }

    fn global_threshold(&self) -> u8 {
        taken_threshold(self.global_counter_bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeBranchPredictor {
    config: BiModeBranchPredictorConfig,
    choice_counters: Vec<u8>,
    taken_counters: Vec<u8>,
    not_taken_counters: Vec<u8>,
    threads: Vec<BiModeThreadSnapshot>,
    lookup_count: u64,
    history_update_count: u64,
    update_count: u64,
    squash_count: u64,
}

impl BiModeBranchPredictor {
    pub fn new(config: BiModeBranchPredictorConfig) -> Self {
        Self {
            choice_counters: vec![0; config.choice_entries()],
            taken_counters: vec![0; config.global_entries()],
            not_taken_counters: vec![0; config.global_entries()],
            threads: vec![BiModeThreadSnapshot::new(); config.threads()],
            config,
            lookup_count: 0,
            history_update_count: 0,
            update_count: 0,
            squash_count: 0,
        }
    }

    pub const fn config(&self) -> &BiModeBranchPredictorConfig {
        &self.config
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        pc: Address,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let global_history = self.threads[thread_index].global_history();
        self.predict_with_history(cpu, pc, global_history)
    }

    pub(crate) fn predict_with_global_history(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        self.thread_index(cpu)?;
        self.predict_with_history(cpu, pc, global_history & self.config.history_mask())
    }

    pub(crate) fn global_history(&self, cpu: CpuId) -> Result<u64, BiModeBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        Ok(self.threads[thread_index].global_history())
    }

    pub(crate) fn shifted_history(&self, old_history: u64, taken: bool) -> u64 {
        self.shift_history(old_history, taken)
    }

    fn predict_with_history(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        let choice_index = self.choice_index(pc);
        let global_index = self.global_index(pc, global_history);
        let choice_counter = self.choice_counters[choice_index];
        let taken_counter = self.taken_counters[global_index];
        let not_taken_counter = self.not_taken_counters[global_index];
        let selected_array = if choice_counter > self.config.choice_threshold() {
            BiModeDirectionArray::Taken
        } else {
            BiModeDirectionArray::NotTaken
        };
        let taken_prediction = taken_counter > self.config.global_threshold();
        let not_taken_prediction = not_taken_counter > self.config.global_threshold();
        let predicted_taken = match selected_array {
            BiModeDirectionArray::Taken => taken_prediction,
            BiModeDirectionArray::NotTaken => not_taken_prediction,
        };

        self.lookup_count += 1;

        Ok(BiModePrediction {
            history: BiModeHistory {
                cpu,
                pc,
                choice_index,
                global_index,
                global_history_before: global_history,
                selected_array,
                taken_prediction,
                not_taken_prediction,
                predicted_taken,
                choice_counter,
                taken_counter,
                not_taken_counter,
            },
            lookup_count: self.lookup_count,
        })
    }

    pub fn predict_unconditional(
        &mut self,
        cpu: CpuId,
        pc: Address,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let global_history = self.threads[thread_index].global_history();
        self.predict_unconditional_with_history(cpu, pc, global_history)
    }

    pub(crate) fn predict_unconditional_with_global_history(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        self.thread_index(cpu)?;
        self.predict_unconditional_with_history(
            cpu,
            pc,
            global_history & self.config.history_mask(),
        )
    }

    fn predict_unconditional_with_history(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        let choice_index = self.choice_index(pc);
        let global_index = self.global_index(pc, global_history);

        self.lookup_count += 1;

        Ok(BiModePrediction {
            history: BiModeHistory {
                cpu,
                pc,
                choice_index,
                global_index,
                global_history_before: global_history,
                selected_array: BiModeDirectionArray::Taken,
                taken_prediction: true,
                not_taken_prediction: true,
                predicted_taken: true,
                choice_counter: self.choice_counters[choice_index],
                taken_counter: self.taken_counters[global_index],
                not_taken_counter: self.not_taken_counters[global_index],
            },
            lookup_count: self.lookup_count,
        })
    }

    pub(crate) fn predict_with_global_history_and_direction(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
        predicted_taken: bool,
    ) -> Result<BiModePrediction, BiModeBranchPredictorError> {
        self.thread_index(cpu)?;
        let global_history = global_history & self.config.history_mask();
        let choice_index = self.choice_index(pc);
        let global_index = self.global_index(pc, global_history);
        let choice_counter = self.choice_counters[choice_index];
        let taken_counter = self.taken_counters[global_index];
        let not_taken_counter = self.not_taken_counters[global_index];
        let selected_array = if choice_counter > self.config.choice_threshold() {
            BiModeDirectionArray::Taken
        } else {
            BiModeDirectionArray::NotTaken
        };
        let taken_prediction = taken_counter > self.config.global_threshold();
        let not_taken_prediction = not_taken_counter > self.config.global_threshold();

        self.lookup_count += 1;

        Ok(BiModePrediction {
            history: BiModeHistory {
                cpu,
                pc,
                choice_index,
                global_index,
                global_history_before: global_history,
                selected_array,
                taken_prediction,
                not_taken_prediction,
                predicted_taken,
                choice_counter,
                taken_counter,
                not_taken_counter,
            },
            lookup_count: self.lookup_count,
        })
    }

    pub fn update_history(
        &mut self,
        history: &BiModeHistory,
        taken: bool,
    ) -> Result<BiModeHistoryUpdate, BiModeBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_history = self.threads[thread_index].global_history();
        if old_history != history.global_history_before() {
            return Err(BiModeBranchPredictorError::HistoryUpdateOutOfOrder {
                cpu: history.cpu(),
                expected_history: old_history,
                actual_history: history.global_history_before(),
            });
        }
        let new_history = self.shift_history(old_history, taken);
        self.threads[thread_index].global_history = new_history;
        self.history_update_count += 1;

        Ok(BiModeHistoryUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            old_history,
            new_history,
            taken,
            history_update_count: self.history_update_count,
        })
    }

    pub fn train(
        &mut self,
        history: &BiModeHistory,
        actual_taken: bool,
        squashed: bool,
    ) -> Result<BiModeTrainingUpdate, BiModeBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_choice_counter = self.choice_counters[history.choice_index()];
        let old_taken_counter = self.taken_counters[history.global_index()];
        let old_not_taken_counter = self.not_taken_counters[history.global_index()];

        if squashed {
            let repaired_history =
                self.shift_history(history.global_history_before(), actual_taken);
            self.threads[thread_index].global_history = repaired_history;
            self.squash_count += 1;

            return Ok(BiModeTrainingUpdate {
                cpu: history.cpu(),
                pc: history.pc(),
                choice_index: history.choice_index(),
                global_index: history.global_index(),
                selected_array: history.selected_array(),
                actual_taken,
                squashed,
                old_choice_counter,
                new_choice_counter: old_choice_counter,
                old_taken_counter,
                new_taken_counter: old_taken_counter,
                old_not_taken_counter,
                new_not_taken_counter: old_not_taken_counter,
                update_count: self.update_count,
                repaired_history: Some(repaired_history),
            });
        }

        let mut new_taken_counter = old_taken_counter;
        let mut new_not_taken_counter = old_not_taken_counter;
        match history.selected_array() {
            BiModeDirectionArray::Taken => {
                new_taken_counter = saturating_counter(
                    old_taken_counter,
                    actual_taken,
                    self.config.global_counter_max(),
                );
                self.taken_counters[history.global_index()] = new_taken_counter;
            }
            BiModeDirectionArray::NotTaken => {
                new_not_taken_counter = saturating_counter(
                    old_not_taken_counter,
                    actual_taken,
                    self.config.global_counter_max(),
                );
                self.not_taken_counters[history.global_index()] = new_not_taken_counter;
            }
        }

        let update_choice = if history.predicted_taken() == actual_taken {
            history.predicted_taken() == history.selected_array().predicts_taken_bias()
        } else {
            true
        };

        let new_choice_counter = if update_choice {
            let new_counter = saturating_counter(
                old_choice_counter,
                actual_taken,
                self.config.choice_counter_max(),
            );
            self.choice_counters[history.choice_index()] = new_counter;
            new_counter
        } else {
            old_choice_counter
        };

        self.update_count += 1;

        Ok(BiModeTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            choice_index: history.choice_index(),
            global_index: history.global_index(),
            selected_array: history.selected_array(),
            actual_taken,
            squashed,
            old_choice_counter,
            new_choice_counter,
            old_taken_counter,
            new_taken_counter,
            old_not_taken_counter,
            new_not_taken_counter,
            update_count: self.update_count,
            repaired_history: None,
        })
    }

    pub fn squash(
        &mut self,
        history: &BiModeHistory,
    ) -> Result<BiModeSquash, BiModeBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_history = self.threads[thread_index].global_history();
        self.threads[thread_index].global_history = history.global_history_before();
        self.squash_count += 1;

        Ok(BiModeSquash {
            cpu: history.cpu(),
            pc: history.pc(),
            old_history,
            restored_history: history.global_history_before(),
            squash_count: self.squash_count,
        })
    }

    pub fn snapshot(&self) -> BiModeBranchPredictorSnapshot {
        BiModeBranchPredictorSnapshot {
            config: self.config.clone(),
            choice_counters: self.choice_counters.clone(),
            taken_counters: self.taken_counters.clone(),
            not_taken_counters: self.not_taken_counters.clone(),
            threads: self.threads.clone(),
            lookup_count: self.lookup_count,
            history_update_count: self.history_update_count,
            update_count: self.update_count,
            squash_count: self.squash_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &BiModeBranchPredictorSnapshot,
    ) -> Result<(), BiModeBranchPredictorError> {
        if self.config.threads() != snapshot.config.threads()
            || self.config.choice_entries() != snapshot.config.choice_entries()
            || self.config.global_entries() != snapshot.config.global_entries()
        {
            return Err(BiModeBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: self.config.threads(),
                actual_threads: snapshot.config.threads(),
                expected_choice_entries: self.config.choice_entries(),
                actual_choice_entries: snapshot.config.choice_entries(),
                expected_global_entries: self.config.global_entries(),
                actual_global_entries: snapshot.config.global_entries(),
            });
        }
        if self.config != snapshot.config {
            return Err(BiModeBranchPredictorError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }
        if snapshot.choice_counters.len() != snapshot.config.choice_entries()
            || snapshot.taken_counters.len() != snapshot.config.global_entries()
            || snapshot.not_taken_counters.len() != snapshot.config.global_entries()
            || snapshot.threads.len() != snapshot.config.threads()
        {
            return Err(BiModeBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: snapshot.config.threads(),
                actual_threads: snapshot.threads.len(),
                expected_choice_entries: snapshot.config.choice_entries(),
                actual_choice_entries: snapshot.choice_counters.len(),
                expected_global_entries: snapshot.config.global_entries(),
                actual_global_entries: snapshot
                    .taken_counters
                    .len()
                    .max(snapshot.not_taken_counters.len()),
            });
        }

        self.choice_counters.clone_from(&snapshot.choice_counters);
        self.taken_counters.clone_from(&snapshot.taken_counters);
        self.not_taken_counters
            .clone_from(&snapshot.not_taken_counters);
        self.threads.clone_from(&snapshot.threads);
        self.lookup_count = snapshot.lookup_count;
        self.history_update_count = snapshot.history_update_count;
        self.update_count = snapshot.update_count;
        self.squash_count = snapshot.squash_count;
        Ok(())
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, BiModeBranchPredictorError> {
        let index = cpu.get() as usize;
        if index < self.threads.len() {
            Ok(index)
        } else {
            Err(BiModeBranchPredictorError::UnknownThread { cpu })
        }
    }

    fn choice_index(&self, pc: Address) -> usize {
        ((pc.get() >> self.config.inst_shift()) & self.config.choice_mask()) as usize
    }

    fn global_index(&self, pc: Address, global_history: u64) -> usize {
        (((pc.get() >> self.config.inst_shift()) ^ global_history) & self.config.history_mask())
            as usize
    }

    fn shift_history(&self, old_history: u64, taken: bool) -> u64 {
        ((old_history << 1) | u64::from(taken)) & self.config.history_mask()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BiModeDirectionArray {
    Taken,
    NotTaken,
}

impl BiModeDirectionArray {
    const fn predicts_taken_bias(self) -> bool {
        matches!(self, Self::Taken)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModePrediction {
    history: BiModeHistory,
    lookup_count: u64,
}

impl BiModePrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn choice_index(&self) -> usize {
        self.history.choice_index()
    }

    pub const fn global_index(&self) -> usize {
        self.history.global_index()
    }

    pub const fn global_history_before(&self) -> u64 {
        self.history.global_history_before()
    }

    pub const fn selected_array(&self) -> BiModeDirectionArray {
        self.history.selected_array()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn choice_counter(&self) -> u8 {
        self.history.choice_counter()
    }

    pub const fn taken_counter(&self) -> u8 {
        self.history.taken_counter()
    }

    pub const fn not_taken_counter(&self) -> u8 {
        self.history.not_taken_counter()
    }

    pub const fn history(&self) -> &BiModeHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeHistory {
    cpu: CpuId,
    pc: Address,
    choice_index: usize,
    global_index: usize,
    global_history_before: u64,
    selected_array: BiModeDirectionArray,
    taken_prediction: bool,
    not_taken_prediction: bool,
    predicted_taken: bool,
    choice_counter: u8,
    taken_counter: u8,
    not_taken_counter: u8,
}

impl BiModeHistory {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn choice_index(&self) -> usize {
        self.choice_index
    }

    pub const fn global_index(&self) -> usize {
        self.global_index
    }

    pub const fn global_history_before(&self) -> u64 {
        self.global_history_before
    }

    pub const fn selected_array(&self) -> BiModeDirectionArray {
        self.selected_array
    }

    pub const fn taken_prediction(&self) -> bool {
        self.taken_prediction
    }

    pub const fn not_taken_prediction(&self) -> bool {
        self.not_taken_prediction
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn choice_counter(&self) -> u8 {
        self.choice_counter
    }

    pub const fn taken_counter(&self) -> u8 {
        self.taken_counter
    }

    pub const fn not_taken_counter(&self) -> u8 {
        self.not_taken_counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeHistoryUpdate {
    cpu: CpuId,
    pc: Address,
    old_history: u64,
    new_history: u64,
    taken: bool,
    history_update_count: u64,
}

impl BiModeHistoryUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn old_history(&self) -> u64 {
        self.old_history
    }

    pub const fn new_history(&self) -> u64 {
        self.new_history
    }

    pub const fn taken(&self) -> bool {
        self.taken
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    choice_index: usize,
    global_index: usize,
    selected_array: BiModeDirectionArray,
    actual_taken: bool,
    squashed: bool,
    old_choice_counter: u8,
    new_choice_counter: u8,
    old_taken_counter: u8,
    new_taken_counter: u8,
    old_not_taken_counter: u8,
    new_not_taken_counter: u8,
    update_count: u64,
    repaired_history: Option<u64>,
}

impl BiModeTrainingUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn choice_index(&self) -> usize {
        self.choice_index
    }

    pub const fn global_index(&self) -> usize {
        self.global_index
    }

    pub const fn selected_array(&self) -> BiModeDirectionArray {
        self.selected_array
    }

    pub const fn actual_taken(&self) -> bool {
        self.actual_taken
    }

    pub const fn squashed(&self) -> bool {
        self.squashed
    }

    pub const fn old_choice_counter(&self) -> u8 {
        self.old_choice_counter
    }

    pub const fn new_choice_counter(&self) -> u8 {
        self.new_choice_counter
    }

    pub const fn old_taken_counter(&self) -> u8 {
        self.old_taken_counter
    }

    pub const fn new_taken_counter(&self) -> u8 {
        self.new_taken_counter
    }

    pub const fn old_not_taken_counter(&self) -> u8 {
        self.old_not_taken_counter
    }

    pub const fn new_not_taken_counter(&self) -> u8 {
        self.new_not_taken_counter
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn repaired_history(&self) -> Option<u64> {
        self.repaired_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeSquash {
    cpu: CpuId,
    pc: Address,
    old_history: u64,
    restored_history: u64,
    squash_count: u64,
}

impl BiModeSquash {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn old_history(&self) -> u64 {
        self.old_history
    }

    pub const fn restored_history(&self) -> u64 {
        self.restored_history
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeThreadSnapshot {
    global_history: u64,
}

impl BiModeThreadSnapshot {
    const fn new() -> Self {
        Self { global_history: 0 }
    }

    pub(crate) const fn from_global_history(global_history: u64) -> Self {
        Self { global_history }
    }

    pub const fn global_history(&self) -> u64 {
        self.global_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BiModeBranchPredictorSnapshot {
    config: BiModeBranchPredictorConfig,
    choice_counters: Vec<u8>,
    taken_counters: Vec<u8>,
    not_taken_counters: Vec<u8>,
    threads: Vec<BiModeThreadSnapshot>,
    lookup_count: u64,
    history_update_count: u64,
    update_count: u64,
    squash_count: u64,
}

impl BiModeBranchPredictorSnapshot {
    pub(crate) fn from_parts(
        config: BiModeBranchPredictorConfig,
        choice_counters: Vec<u8>,
        taken_counters: Vec<u8>,
        not_taken_counters: Vec<u8>,
        threads: Vec<BiModeThreadSnapshot>,
        lookup_count: u64,
        history_update_count: u64,
        update_count: u64,
        squash_count: u64,
    ) -> Self {
        Self {
            config,
            choice_counters,
            taken_counters,
            not_taken_counters,
            threads,
            lookup_count,
            history_update_count,
            update_count,
            squash_count,
        }
    }

    pub const fn config(&self) -> &BiModeBranchPredictorConfig {
        &self.config
    }

    pub fn choice_counters(&self) -> &[u8] {
        &self.choice_counters
    }

    pub fn taken_counters(&self) -> &[u8] {
        &self.taken_counters
    }

    pub fn not_taken_counters(&self) -> &[u8] {
        &self.not_taken_counters
    }

    pub fn threads(&self) -> &[BiModeThreadSnapshot] {
        &self.threads
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }
}

fn counter_max(bits: u8) -> u8 {
    ((1u16 << bits) - 1) as u8
}

fn taken_threshold(bits: u8) -> u8 {
    (1u16 << (bits - 1)) as u8 - 1
}

fn saturating_counter(counter: u8, taken: bool, max: u8) -> u8 {
    if taken {
        counter.saturating_add(1).min(max)
    } else {
        counter.saturating_sub(1)
    }
}
