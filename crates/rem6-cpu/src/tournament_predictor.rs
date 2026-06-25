use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::CpuId;

const DEFAULT_COUNTER_BITS: u8 = 2;
const DEFAULT_INST_SHIFT: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TournamentBranchPredictorError {
    ZeroThreads,
    ZeroLocalEntries,
    LocalEntriesNotPowerOfTwo {
        entries: usize,
    },
    ZeroLocalHistoryEntries,
    LocalHistoryEntriesNotPowerOfTwo {
        entries: usize,
    },
    ZeroGlobalEntries,
    GlobalEntriesNotPowerOfTwo {
        entries: usize,
    },
    ZeroChoiceEntries,
    ChoiceEntriesNotPowerOfTwo {
        entries: usize,
    },
    LocalCounterBitsOutOfRange {
        bits: u8,
    },
    GlobalCounterBitsOutOfRange {
        bits: u8,
    },
    ChoiceCounterBitsOutOfRange {
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
        expected_global_history: u64,
        actual_global_history: u64,
        expected_local_history: Option<u64>,
        actual_local_history: Option<u64>,
    },
    SnapshotShapeMismatch {
        expected_threads: usize,
        actual_threads: usize,
        expected_local_entries: usize,
        actual_local_entries: usize,
        expected_local_history_entries: usize,
        actual_local_history_entries: usize,
        expected_global_entries: usize,
        actual_global_entries: usize,
        expected_choice_entries: usize,
        actual_choice_entries: usize,
    },
    SnapshotConfigMismatch {
        expected: TournamentBranchPredictorConfig,
        actual: TournamentBranchPredictorConfig,
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
    InvalidCheckpointLocalHistory {
        value: u64,
        max: u64,
    },
}

impl fmt::Display for TournamentBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "tournament predictor has no threads"),
            Self::ZeroLocalEntries => write!(formatter, "tournament local predictor table is empty"),
            Self::LocalEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "tournament local predictor entries {entries} are not a power of two"
            ),
            Self::ZeroLocalHistoryEntries => {
                write!(formatter, "tournament local history table is empty")
            }
            Self::LocalHistoryEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "tournament local history entries {entries} are not a power of two"
            ),
            Self::ZeroGlobalEntries => {
                write!(formatter, "tournament global predictor table is empty")
            }
            Self::GlobalEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "tournament global predictor entries {entries} are not a power of two"
            ),
            Self::ZeroChoiceEntries => {
                write!(formatter, "tournament choice predictor table is empty")
            }
            Self::ChoiceEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "tournament choice predictor entries {entries} are not a power of two"
            ),
            Self::LocalCounterBitsOutOfRange { bits } => write!(
                formatter,
                "tournament local counter width {bits} is outside 1..=8"
            ),
            Self::GlobalCounterBitsOutOfRange { bits } => write!(
                formatter,
                "tournament global counter width {bits} is outside 1..=8"
            ),
            Self::ChoiceCounterBitsOutOfRange { bits } => write!(
                formatter,
                "tournament choice counter width {bits} is outside 1..=8"
            ),
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "tournament instruction shift {bits} is outside 0..=63"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "tournament predictor thread {} is not configured",
                cpu.get()
            ),
            Self::HistoryUpdateOutOfOrder {
                cpu,
                expected_global_history,
                actual_global_history,
                expected_local_history,
                actual_local_history,
            } => write!(
                formatter,
                "tournament predictor thread {} global history is {expected_global_history}, but update record starts from {actual_global_history}; local history is {expected_local_history:?}, but update record starts from {actual_local_history:?}",
                cpu.get()
            ),
            Self::SnapshotShapeMismatch {
                expected_threads,
                actual_threads,
                expected_local_entries,
                actual_local_entries,
                expected_local_history_entries,
                actual_local_history_entries,
                expected_global_entries,
                actual_global_entries,
                expected_choice_entries,
                actual_choice_entries,
            } => write!(
                formatter,
                "tournament snapshot shape threads={actual_threads}, local_entries={actual_local_entries}, local_history_entries={actual_local_history_entries}, global_entries={actual_global_entries}, choice_entries={actual_choice_entries} does not match predictor threads={expected_threads}, local_entries={expected_local_entries}, local_history_entries={expected_local_history_entries}, global_entries={expected_global_entries}, choice_entries={expected_choice_entries}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "tournament snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "tournament checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "tournament checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "tournament checkpoint payload version {version} is not supported"
            ),
            Self::CheckpointValueTooLarge { name, value, max } => write!(
                formatter,
                "tournament checkpoint {name} value {value} exceeds maximum {max}"
            ),
            Self::InvalidCheckpointCounter { table, value, max } => write!(
                formatter,
                "tournament checkpoint {table} counter {value} exceeds maximum {max}"
            ),
            Self::InvalidCheckpointLocalHistory { value, max } => write!(
                formatter,
                "tournament checkpoint local history {value} exceeds maximum {max}"
            ),
        }
    }
}

impl Error for TournamentBranchPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentBranchPredictorConfig {
    threads: usize,
    local_entries: usize,
    local_history_entries: usize,
    global_entries: usize,
    choice_entries: usize,
    local_counter_bits: u8,
    global_counter_bits: u8,
    choice_counter_bits: u8,
    inst_shift: u8,
    local_history_bits: u8,
    global_history_bits: u8,
}

impl TournamentBranchPredictorConfig {
    pub fn new(
        threads: usize,
        local_entries: usize,
        local_history_entries: usize,
        global_entries: usize,
        choice_entries: usize,
    ) -> Result<Self, TournamentBranchPredictorError> {
        Self::with_options(
            threads,
            local_entries,
            local_history_entries,
            global_entries,
            choice_entries,
            DEFAULT_COUNTER_BITS,
            DEFAULT_COUNTER_BITS,
            DEFAULT_COUNTER_BITS,
            DEFAULT_INST_SHIFT,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        threads: usize,
        local_entries: usize,
        local_history_entries: usize,
        global_entries: usize,
        choice_entries: usize,
        local_counter_bits: u8,
        global_counter_bits: u8,
        choice_counter_bits: u8,
        inst_shift: u8,
    ) -> Result<Self, TournamentBranchPredictorError> {
        if threads == 0 {
            return Err(TournamentBranchPredictorError::ZeroThreads);
        }
        if local_entries == 0 {
            return Err(TournamentBranchPredictorError::ZeroLocalEntries);
        }
        if !local_entries.is_power_of_two() {
            return Err(TournamentBranchPredictorError::LocalEntriesNotPowerOfTwo {
                entries: local_entries,
            });
        }
        if local_history_entries == 0 {
            return Err(TournamentBranchPredictorError::ZeroLocalHistoryEntries);
        }
        if !local_history_entries.is_power_of_two() {
            return Err(
                TournamentBranchPredictorError::LocalHistoryEntriesNotPowerOfTwo {
                    entries: local_history_entries,
                },
            );
        }
        if global_entries == 0 {
            return Err(TournamentBranchPredictorError::ZeroGlobalEntries);
        }
        if !global_entries.is_power_of_two() {
            return Err(TournamentBranchPredictorError::GlobalEntriesNotPowerOfTwo {
                entries: global_entries,
            });
        }
        if choice_entries == 0 {
            return Err(TournamentBranchPredictorError::ZeroChoiceEntries);
        }
        if !choice_entries.is_power_of_two() {
            return Err(TournamentBranchPredictorError::ChoiceEntriesNotPowerOfTwo {
                entries: choice_entries,
            });
        }
        if !(1..=8).contains(&local_counter_bits) {
            return Err(TournamentBranchPredictorError::LocalCounterBitsOutOfRange {
                bits: local_counter_bits,
            });
        }
        if !(1..=8).contains(&global_counter_bits) {
            return Err(
                TournamentBranchPredictorError::GlobalCounterBitsOutOfRange {
                    bits: global_counter_bits,
                },
            );
        }
        if !(1..=8).contains(&choice_counter_bits) {
            return Err(
                TournamentBranchPredictorError::ChoiceCounterBitsOutOfRange {
                    bits: choice_counter_bits,
                },
            );
        }
        if inst_shift > 63 {
            return Err(TournamentBranchPredictorError::InstShiftOutOfRange { bits: inst_shift });
        }

        Ok(Self {
            threads,
            local_entries,
            local_history_entries,
            global_entries,
            choice_entries,
            local_counter_bits,
            global_counter_bits,
            choice_counter_bits,
            inst_shift,
            local_history_bits: local_entries.trailing_zeros() as u8,
            global_history_bits: global_entries
                .trailing_zeros()
                .max(choice_entries.trailing_zeros()) as u8,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn local_entries(&self) -> usize {
        self.local_entries
    }

    pub const fn local_history_entries(&self) -> usize {
        self.local_history_entries
    }

    pub const fn global_entries(&self) -> usize {
        self.global_entries
    }

    pub const fn choice_entries(&self) -> usize {
        self.choice_entries
    }

    pub const fn local_counter_bits(&self) -> u8 {
        self.local_counter_bits
    }

    pub const fn global_counter_bits(&self) -> u8 {
        self.global_counter_bits
    }

    pub const fn choice_counter_bits(&self) -> u8 {
        self.choice_counter_bits
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    pub const fn local_history_bits(&self) -> u8 {
        self.local_history_bits
    }

    pub const fn global_history_bits(&self) -> u8 {
        self.global_history_bits
    }

    fn local_predictor_mask(&self) -> u64 {
        self.local_entries as u64 - 1
    }

    fn local_history_table_mask(&self) -> u64 {
        self.local_history_entries as u64 - 1
    }

    fn global_history_mask(&self) -> u64 {
        self.global_entries as u64 - 1
    }

    fn choice_history_mask(&self) -> u64 {
        self.choice_entries as u64 - 1
    }

    fn history_register_mask(&self) -> u64 {
        bit_mask(self.global_history_bits)
    }

    fn local_counter_max(&self) -> u8 {
        counter_max(self.local_counter_bits)
    }

    fn global_counter_max(&self) -> u8 {
        counter_max(self.global_counter_bits)
    }

    fn choice_counter_max(&self) -> u8 {
        counter_max(self.choice_counter_bits)
    }

    fn local_threshold(&self) -> u8 {
        taken_threshold(self.local_counter_bits)
    }

    fn global_threshold(&self) -> u8 {
        taken_threshold(self.global_counter_bits)
    }

    fn choice_threshold(&self) -> u8 {
        taken_threshold(self.choice_counter_bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentBranchPredictor {
    config: TournamentBranchPredictorConfig,
    local_counters: Vec<u8>,
    local_history_table: Vec<u64>,
    global_counters: Vec<u8>,
    choice_counters: Vec<u8>,
    threads: Vec<TournamentThreadSnapshot>,
    lookup_count: u64,
    history_update_count: u64,
    update_count: u64,
    squash_count: u64,
}

impl TournamentBranchPredictor {
    pub fn new(config: TournamentBranchPredictorConfig) -> Self {
        Self {
            local_counters: vec![0; config.local_entries()],
            local_history_table: vec![0; config.local_history_entries()],
            global_counters: vec![0; config.global_entries()],
            choice_counters: vec![0; config.choice_entries()],
            threads: vec![TournamentThreadSnapshot::new(); config.threads()],
            config,
            lookup_count: 0,
            history_update_count: 0,
            update_count: 0,
            squash_count: 0,
        }
    }

    pub const fn config(&self) -> &TournamentBranchPredictorConfig {
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
    ) -> Result<TournamentPrediction, TournamentBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let global_history = self.threads[thread_index].global_history();
        let local_history_index = self.local_history_index(pc);
        let local_history = self.local_history_table[local_history_index];
        let local_predictor_index = self.local_predictor_index(local_history);
        let global_index = self.global_index(global_history);
        let choice_index = self.choice_index(global_history);
        let local_counter = self.local_counters[local_predictor_index];
        let global_counter = self.global_counters[global_index];
        let choice_counter = self.choice_counters[choice_index];
        let local_predicted_taken = local_counter > self.config.local_threshold();
        let global_predicted_taken = global_counter > self.config.global_threshold();
        let selection = if choice_counter > self.config.choice_threshold() {
            TournamentPredictorSelection::Global
        } else {
            TournamentPredictorSelection::Local
        };
        let predicted_taken = match selection {
            TournamentPredictorSelection::Local => local_predicted_taken,
            TournamentPredictorSelection::Global => global_predicted_taken,
        };

        self.lookup_count += 1;

        Ok(TournamentPrediction {
            history: TournamentHistory {
                cpu,
                pc,
                local_history_valid: true,
                local_history_index,
                local_predictor_index,
                global_index,
                choice_index,
                global_history_before: global_history,
                local_history_before: local_history,
                selection,
                local_predicted_taken,
                global_predicted_taken,
                predicted_taken,
                local_counter,
                global_counter,
                choice_counter,
            },
            lookup_count: self.lookup_count,
        })
    }

    pub fn predict_unconditional(
        &mut self,
        cpu: CpuId,
        pc: Address,
    ) -> Result<TournamentPrediction, TournamentBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let global_history = self.threads[thread_index].global_history();
        let global_index = self.global_index(global_history);
        let choice_index = self.choice_index(global_history);

        self.lookup_count += 1;

        Ok(TournamentPrediction {
            history: TournamentHistory {
                cpu,
                pc,
                local_history_valid: false,
                local_history_index: 0,
                local_predictor_index: 0,
                global_index,
                choice_index,
                global_history_before: global_history,
                local_history_before: 0,
                selection: TournamentPredictorSelection::Global,
                local_predicted_taken: true,
                global_predicted_taken: true,
                predicted_taken: true,
                local_counter: self.local_counters[0],
                global_counter: self.global_counters[global_index],
                choice_counter: self.choice_counters[choice_index],
            },
            lookup_count: self.lookup_count,
        })
    }

    pub fn update_history(
        &mut self,
        history: &TournamentHistory,
        taken: bool,
    ) -> Result<TournamentHistoryUpdate, TournamentBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_global_history = self.threads[thread_index].global_history();
        let old_local_history = if history.local_history_valid() {
            self.local_history_table[history.local_history_index()]
        } else {
            0
        };
        let expected_local_history = history.local_history_valid().then_some(old_local_history);
        let actual_local_history = history
            .local_history_valid()
            .then_some(history.local_history_before());
        if old_global_history != history.global_history_before()
            || expected_local_history != actual_local_history
        {
            return Err(TournamentBranchPredictorError::HistoryUpdateOutOfOrder {
                cpu: history.cpu(),
                expected_global_history: old_global_history,
                actual_global_history: history.global_history_before(),
                expected_local_history,
                actual_local_history,
            });
        }

        let new_global_history = self.shift_global_history(old_global_history, taken);
        self.threads[thread_index].global_history = new_global_history;

        let new_local_history = if history.local_history_valid() {
            let shifted = self.shift_local_history(old_local_history, taken);
            self.local_history_table[history.local_history_index()] = shifted;
            shifted
        } else {
            old_local_history
        };

        self.history_update_count += 1;

        Ok(TournamentHistoryUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            local_history_updated: history.local_history_valid(),
            local_history_index: history.local_history_index(),
            old_global_history,
            new_global_history,
            old_local_history,
            new_local_history,
            taken,
            history_update_count: self.history_update_count,
        })
    }

    pub fn train(
        &mut self,
        history: &TournamentHistory,
        actual_taken: bool,
        squashed: bool,
    ) -> Result<TournamentTrainingUpdate, TournamentBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_choice_counter = self.choice_counters[history.choice_index()];
        let old_global_counter = self.global_counters[history.global_index()];
        let old_local_counter = if history.local_history_valid() {
            self.local_counters[history.local_predictor_index()]
        } else {
            history.local_counter()
        };

        if squashed {
            let repaired_global_history =
                self.shift_global_history(history.global_history_before(), actual_taken);
            self.threads[thread_index].global_history = repaired_global_history;
            let repaired_local_history = if history.local_history_valid() {
                let repaired =
                    self.shift_local_history(history.local_predictor_index() as u64, actual_taken);
                self.local_history_table[history.local_history_index()] = repaired;
                Some(repaired)
            } else {
                None
            };
            self.squash_count += 1;

            return Ok(TournamentTrainingUpdate {
                cpu: history.cpu(),
                pc: history.pc(),
                local_history_valid: history.local_history_valid(),
                local_history_index: history.local_history_index(),
                local_predictor_index: history.local_predictor_index(),
                global_index: history.global_index(),
                choice_index: history.choice_index(),
                selection: history.selection(),
                actual_taken,
                squashed,
                old_choice_counter,
                new_choice_counter: old_choice_counter,
                old_local_counter,
                new_local_counter: old_local_counter,
                old_global_counter,
                new_global_counter: old_global_counter,
                update_count: self.update_count,
                repaired_global_history: Some(repaired_global_history),
                repaired_local_history,
            });
        }

        let mut new_choice_counter = old_choice_counter;
        if history.local_history_valid()
            && history.local_predicted_taken() != history.global_predicted_taken()
        {
            let global_correct = history.global_predicted_taken() == actual_taken;
            let local_correct = history.local_predicted_taken() == actual_taken;
            if global_correct && !local_correct {
                new_choice_counter =
                    saturating_counter(old_choice_counter, true, self.config.choice_counter_max());
            } else if local_correct && !global_correct {
                new_choice_counter =
                    saturating_counter(old_choice_counter, false, self.config.choice_counter_max());
            }
            self.choice_counters[history.choice_index()] = new_choice_counter;
        }

        let new_global_counter = saturating_counter(
            old_global_counter,
            actual_taken,
            self.config.global_counter_max(),
        );
        self.global_counters[history.global_index()] = new_global_counter;

        let new_local_counter = if history.local_history_valid() {
            let new_counter = saturating_counter(
                old_local_counter,
                actual_taken,
                self.config.local_counter_max(),
            );
            self.local_counters[history.local_predictor_index()] = new_counter;
            new_counter
        } else {
            old_local_counter
        };

        self.update_count += 1;

        Ok(TournamentTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            local_history_valid: history.local_history_valid(),
            local_history_index: history.local_history_index(),
            local_predictor_index: history.local_predictor_index(),
            global_index: history.global_index(),
            choice_index: history.choice_index(),
            selection: history.selection(),
            actual_taken,
            squashed,
            old_choice_counter,
            new_choice_counter,
            old_local_counter,
            new_local_counter,
            old_global_counter,
            new_global_counter,
            update_count: self.update_count,
            repaired_global_history: None,
            repaired_local_history: None,
        })
    }

    pub fn squash(
        &mut self,
        history: &TournamentHistory,
    ) -> Result<TournamentSquash, TournamentBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_global_history = self.threads[thread_index].global_history();
        self.threads[thread_index].global_history = history.global_history_before();

        let (old_local_history, restored_local_history) = if history.local_history_valid() {
            let old_history = self.local_history_table[history.local_history_index()];
            self.local_history_table[history.local_history_index()] =
                history.local_history_before();
            (Some(old_history), Some(history.local_history_before()))
        } else {
            (None, None)
        };

        self.squash_count += 1;

        Ok(TournamentSquash {
            cpu: history.cpu(),
            pc: history.pc(),
            old_global_history,
            restored_global_history: history.global_history_before(),
            old_local_history,
            restored_local_history,
            squash_count: self.squash_count,
        })
    }

    pub fn snapshot(&self) -> TournamentBranchPredictorSnapshot {
        TournamentBranchPredictorSnapshot {
            config: self.config.clone(),
            local_counters: self.local_counters.clone(),
            local_history_table: self.local_history_table.clone(),
            global_counters: self.global_counters.clone(),
            choice_counters: self.choice_counters.clone(),
            threads: self.threads.clone(),
            lookup_count: self.lookup_count,
            history_update_count: self.history_update_count,
            update_count: self.update_count,
            squash_count: self.squash_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &TournamentBranchPredictorSnapshot,
    ) -> Result<(), TournamentBranchPredictorError> {
        if self.config.threads() != snapshot.config.threads()
            || self.config.local_entries() != snapshot.config.local_entries()
            || self.config.local_history_entries() != snapshot.config.local_history_entries()
            || self.config.global_entries() != snapshot.config.global_entries()
            || self.config.choice_entries() != snapshot.config.choice_entries()
        {
            return Err(TournamentBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: self.config.threads(),
                actual_threads: snapshot.config.threads(),
                expected_local_entries: self.config.local_entries(),
                actual_local_entries: snapshot.config.local_entries(),
                expected_local_history_entries: self.config.local_history_entries(),
                actual_local_history_entries: snapshot.config.local_history_entries(),
                expected_global_entries: self.config.global_entries(),
                actual_global_entries: snapshot.config.global_entries(),
                expected_choice_entries: self.config.choice_entries(),
                actual_choice_entries: snapshot.config.choice_entries(),
            });
        }
        if self.config != snapshot.config {
            return Err(TournamentBranchPredictorError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }
        if snapshot.local_counters.len() != snapshot.config.local_entries()
            || snapshot.local_history_table.len() != snapshot.config.local_history_entries()
            || snapshot.global_counters.len() != snapshot.config.global_entries()
            || snapshot.choice_counters.len() != snapshot.config.choice_entries()
            || snapshot.threads.len() != snapshot.config.threads()
        {
            return Err(TournamentBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: snapshot.config.threads(),
                actual_threads: snapshot.threads.len(),
                expected_local_entries: snapshot.config.local_entries(),
                actual_local_entries: snapshot.local_counters.len(),
                expected_local_history_entries: snapshot.config.local_history_entries(),
                actual_local_history_entries: snapshot.local_history_table.len(),
                expected_global_entries: snapshot.config.global_entries(),
                actual_global_entries: snapshot.global_counters.len(),
                expected_choice_entries: snapshot.config.choice_entries(),
                actual_choice_entries: snapshot.choice_counters.len(),
            });
        }

        self.local_counters.clone_from(&snapshot.local_counters);
        self.local_history_table
            .clone_from(&snapshot.local_history_table);
        self.global_counters.clone_from(&snapshot.global_counters);
        self.choice_counters.clone_from(&snapshot.choice_counters);
        self.threads.clone_from(&snapshot.threads);
        self.lookup_count = snapshot.lookup_count;
        self.history_update_count = snapshot.history_update_count;
        self.update_count = snapshot.update_count;
        self.squash_count = snapshot.squash_count;
        Ok(())
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, TournamentBranchPredictorError> {
        let index = cpu.get() as usize;
        if index < self.threads.len() {
            Ok(index)
        } else {
            Err(TournamentBranchPredictorError::UnknownThread { cpu })
        }
    }

    fn local_history_index(&self, pc: Address) -> usize {
        ((pc.get() >> self.config.inst_shift()) & self.config.local_history_table_mask()) as usize
    }

    fn local_predictor_index(&self, local_history: u64) -> usize {
        (local_history & self.config.local_predictor_mask()) as usize
    }

    fn global_index(&self, global_history: u64) -> usize {
        (global_history & self.config.global_history_mask()) as usize
    }

    fn choice_index(&self, global_history: u64) -> usize {
        (global_history & self.config.choice_history_mask()) as usize
    }

    fn shift_global_history(&self, old_history: u64, taken: bool) -> u64 {
        shift_history(old_history, taken, self.config.history_register_mask())
    }

    fn shift_local_history(&self, old_history: u64, taken: bool) -> u64 {
        shift_history(old_history, taken, self.config.local_predictor_mask())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TournamentPredictorSelection {
    Local,
    Global,
}

impl TournamentPredictorSelection {
    pub const fn uses_global(self) -> bool {
        matches!(self, Self::Global)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentPrediction {
    history: TournamentHistory,
    lookup_count: u64,
}

impl TournamentPrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn local_history_valid(&self) -> bool {
        self.history.local_history_valid()
    }

    pub const fn local_history_index(&self) -> usize {
        self.history.local_history_index()
    }

    pub const fn local_predictor_index(&self) -> usize {
        self.history.local_predictor_index()
    }

    pub const fn global_index(&self) -> usize {
        self.history.global_index()
    }

    pub const fn choice_index(&self) -> usize {
        self.history.choice_index()
    }

    pub const fn global_history_before(&self) -> u64 {
        self.history.global_history_before()
    }

    pub const fn local_history_before(&self) -> u64 {
        self.history.local_history_before()
    }

    pub const fn selection(&self) -> TournamentPredictorSelection {
        self.history.selection()
    }

    pub const fn local_predicted_taken(&self) -> bool {
        self.history.local_predicted_taken()
    }

    pub const fn global_predicted_taken(&self) -> bool {
        self.history.global_predicted_taken()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn local_counter(&self) -> u8 {
        self.history.local_counter()
    }

    pub const fn global_counter(&self) -> u8 {
        self.history.global_counter()
    }

    pub const fn choice_counter(&self) -> u8 {
        self.history.choice_counter()
    }

    pub const fn history(&self) -> &TournamentHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentHistory {
    cpu: CpuId,
    pc: Address,
    local_history_valid: bool,
    local_history_index: usize,
    local_predictor_index: usize,
    global_index: usize,
    choice_index: usize,
    global_history_before: u64,
    local_history_before: u64,
    selection: TournamentPredictorSelection,
    local_predicted_taken: bool,
    global_predicted_taken: bool,
    predicted_taken: bool,
    local_counter: u8,
    global_counter: u8,
    choice_counter: u8,
}

impl TournamentHistory {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn local_history_valid(&self) -> bool {
        self.local_history_valid
    }

    pub const fn local_history_index(&self) -> usize {
        self.local_history_index
    }

    pub const fn local_predictor_index(&self) -> usize {
        self.local_predictor_index
    }

    pub const fn global_index(&self) -> usize {
        self.global_index
    }

    pub const fn choice_index(&self) -> usize {
        self.choice_index
    }

    pub const fn global_history_before(&self) -> u64 {
        self.global_history_before
    }

    pub const fn local_history_before(&self) -> u64 {
        self.local_history_before
    }

    pub const fn selection(&self) -> TournamentPredictorSelection {
        self.selection
    }

    pub const fn local_predicted_taken(&self) -> bool {
        self.local_predicted_taken
    }

    pub const fn global_predicted_taken(&self) -> bool {
        self.global_predicted_taken
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn local_counter(&self) -> u8 {
        self.local_counter
    }

    pub const fn global_counter(&self) -> u8 {
        self.global_counter
    }

    pub const fn choice_counter(&self) -> u8 {
        self.choice_counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentHistoryUpdate {
    cpu: CpuId,
    pc: Address,
    local_history_updated: bool,
    local_history_index: usize,
    old_global_history: u64,
    new_global_history: u64,
    old_local_history: u64,
    new_local_history: u64,
    taken: bool,
    history_update_count: u64,
}

impl TournamentHistoryUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn local_history_updated(&self) -> bool {
        self.local_history_updated
    }

    pub const fn local_history_index(&self) -> usize {
        self.local_history_index
    }

    pub const fn old_global_history(&self) -> u64 {
        self.old_global_history
    }

    pub const fn new_global_history(&self) -> u64 {
        self.new_global_history
    }

    pub const fn old_local_history(&self) -> u64 {
        self.old_local_history
    }

    pub const fn new_local_history(&self) -> u64 {
        self.new_local_history
    }

    pub const fn taken(&self) -> bool {
        self.taken
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    local_history_valid: bool,
    local_history_index: usize,
    local_predictor_index: usize,
    global_index: usize,
    choice_index: usize,
    selection: TournamentPredictorSelection,
    actual_taken: bool,
    squashed: bool,
    old_choice_counter: u8,
    new_choice_counter: u8,
    old_local_counter: u8,
    new_local_counter: u8,
    old_global_counter: u8,
    new_global_counter: u8,
    update_count: u64,
    repaired_global_history: Option<u64>,
    repaired_local_history: Option<u64>,
}

impl TournamentTrainingUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn local_history_valid(&self) -> bool {
        self.local_history_valid
    }

    pub const fn local_history_index(&self) -> usize {
        self.local_history_index
    }

    pub const fn local_predictor_index(&self) -> usize {
        self.local_predictor_index
    }

    pub const fn global_index(&self) -> usize {
        self.global_index
    }

    pub const fn choice_index(&self) -> usize {
        self.choice_index
    }

    pub const fn selection(&self) -> TournamentPredictorSelection {
        self.selection
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

    pub const fn old_local_counter(&self) -> u8 {
        self.old_local_counter
    }

    pub const fn new_local_counter(&self) -> u8 {
        self.new_local_counter
    }

    pub const fn old_global_counter(&self) -> u8 {
        self.old_global_counter
    }

    pub const fn new_global_counter(&self) -> u8 {
        self.new_global_counter
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn repaired_global_history(&self) -> Option<u64> {
        self.repaired_global_history
    }

    pub const fn repaired_local_history(&self) -> Option<u64> {
        self.repaired_local_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentSquash {
    cpu: CpuId,
    pc: Address,
    old_global_history: u64,
    restored_global_history: u64,
    old_local_history: Option<u64>,
    restored_local_history: Option<u64>,
    squash_count: u64,
}

impl TournamentSquash {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn old_global_history(&self) -> u64 {
        self.old_global_history
    }

    pub const fn restored_global_history(&self) -> u64 {
        self.restored_global_history
    }

    pub const fn old_local_history(&self) -> Option<u64> {
        self.old_local_history
    }

    pub const fn restored_local_history(&self) -> Option<u64> {
        self.restored_local_history
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TournamentThreadSnapshot {
    global_history: u64,
}

impl TournamentThreadSnapshot {
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
pub struct TournamentBranchPredictorSnapshot {
    config: TournamentBranchPredictorConfig,
    local_counters: Vec<u8>,
    local_history_table: Vec<u64>,
    global_counters: Vec<u8>,
    choice_counters: Vec<u8>,
    threads: Vec<TournamentThreadSnapshot>,
    lookup_count: u64,
    history_update_count: u64,
    update_count: u64,
    squash_count: u64,
}

impl TournamentBranchPredictorSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        config: TournamentBranchPredictorConfig,
        local_counters: Vec<u8>,
        local_history_table: Vec<u64>,
        global_counters: Vec<u8>,
        choice_counters: Vec<u8>,
        threads: Vec<TournamentThreadSnapshot>,
        lookup_count: u64,
        history_update_count: u64,
        update_count: u64,
        squash_count: u64,
    ) -> Self {
        Self {
            config,
            local_counters,
            local_history_table,
            global_counters,
            choice_counters,
            threads,
            lookup_count,
            history_update_count,
            update_count,
            squash_count,
        }
    }

    pub const fn config(&self) -> &TournamentBranchPredictorConfig {
        &self.config
    }

    pub fn local_counters(&self) -> &[u8] {
        &self.local_counters
    }

    pub fn local_history_table(&self) -> &[u64] {
        &self.local_history_table
    }

    pub fn global_counters(&self) -> &[u8] {
        &self.global_counters
    }

    pub fn choice_counters(&self) -> &[u8] {
        &self.choice_counters
    }

    pub fn threads(&self) -> &[TournamentThreadSnapshot] {
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

fn bit_mask(bits: u8) -> u64 {
    if bits >= u64::BITS as u8 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
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

fn shift_history(old_history: u64, taken: bool, mask: u64) -> u64 {
    let bit = if taken { 1 } else { 0 };
    ((old_history << 1) | bit) & mask
}
