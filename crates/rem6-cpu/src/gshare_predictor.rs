use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::CpuId;

const DEFAULT_COUNTER_BITS: u8 = 2;
const DEFAULT_INST_SHIFT: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GShareBranchPredictorError {
    ZeroThreads,
    ZeroTableEntries,
    TableEntriesNotPowerOfTwo {
        entries: usize,
    },
    CounterBitsOutOfRange {
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
        expected_entries: usize,
        actual_entries: usize,
    },
    SnapshotConfigMismatch {
        expected: GShareBranchPredictorConfig,
        actual: GShareBranchPredictorConfig,
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
        value: u8,
        max: u8,
    },
}

impl fmt::Display for GShareBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "gshare predictor has no threads"),
            Self::ZeroTableEntries => write!(formatter, "gshare predictor table is empty"),
            Self::TableEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "gshare predictor table entries {entries} are not a power of two"
            ),
            Self::CounterBitsOutOfRange { bits } => write!(
                formatter,
                "gshare predictor counter width {bits} is outside 1..=8"
            ),
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "gshare predictor instruction shift {bits} is outside 0..=63"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "gshare predictor thread {} is not configured",
                cpu.get()
            ),
            Self::HistoryUpdateOutOfOrder {
                cpu,
                expected_history,
                actual_history,
            } => write!(
                formatter,
                "gshare predictor thread {} history is {expected_history}, but update record starts from {actual_history}",
                cpu.get()
            ),
            Self::SnapshotShapeMismatch {
                expected_threads,
                actual_threads,
                expected_entries,
                actual_entries,
            } => write!(
                formatter,
                "gshare predictor snapshot shape threads={actual_threads}, entries={actual_entries} does not match predictor threads={expected_threads}, entries={expected_entries}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "gshare predictor snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "gshare predictor checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "gshare predictor checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "gshare predictor checkpoint payload version {version} is not supported"
            ),
            Self::CheckpointValueTooLarge { name, value, max } => write!(
                formatter,
                "gshare predictor checkpoint {name} value {value} exceeds maximum {max}"
            ),
            Self::InvalidCheckpointCounter { value, max } => write!(
                formatter,
                "gshare predictor checkpoint counter {value} exceeds maximum {max}"
            ),
        }
    }
}

impl Error for GShareBranchPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GShareBranchPredictorConfig {
    threads: usize,
    table_entries: usize,
    counter_bits: u8,
    inst_shift: u8,
    history_bits: u8,
}

impl GShareBranchPredictorConfig {
    pub fn new(threads: usize, table_entries: usize) -> Result<Self, GShareBranchPredictorError> {
        Self::with_options(
            threads,
            table_entries,
            DEFAULT_COUNTER_BITS,
            DEFAULT_INST_SHIFT,
        )
    }

    pub fn with_options(
        threads: usize,
        table_entries: usize,
        counter_bits: u8,
        inst_shift: u8,
    ) -> Result<Self, GShareBranchPredictorError> {
        if threads == 0 {
            return Err(GShareBranchPredictorError::ZeroThreads);
        }
        if table_entries == 0 {
            return Err(GShareBranchPredictorError::ZeroTableEntries);
        }
        if !table_entries.is_power_of_two() {
            return Err(GShareBranchPredictorError::TableEntriesNotPowerOfTwo {
                entries: table_entries,
            });
        }
        if !(1..=8).contains(&counter_bits) {
            return Err(GShareBranchPredictorError::CounterBitsOutOfRange { bits: counter_bits });
        }
        if inst_shift > 63 {
            return Err(GShareBranchPredictorError::InstShiftOutOfRange { bits: inst_shift });
        }

        Ok(Self {
            threads,
            table_entries,
            counter_bits,
            inst_shift,
            history_bits: table_entries.trailing_zeros() as u8,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }

    pub const fn counter_bits(&self) -> u8 {
        self.counter_bits
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    pub const fn history_bits(&self) -> u8 {
        self.history_bits
    }

    fn history_mask(&self) -> u64 {
        self.table_entries as u64 - 1
    }

    fn counter_max(&self) -> u8 {
        ((1u16 << self.counter_bits) - 1) as u8
    }

    fn taken_threshold(&self) -> u8 {
        (1u16 << (self.counter_bits - 1)) as u8 - 1
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GShareBranchPredictor {
    config: GShareBranchPredictorConfig,
    counters: Vec<u8>,
    threads: Vec<GShareThreadSnapshot>,
    lookup_count: u64,
    history_update_count: u64,
    update_count: u64,
    squash_count: u64,
}

impl GShareBranchPredictor {
    pub fn new(config: GShareBranchPredictorConfig) -> Self {
        Self {
            counters: vec![0; config.table_entries()],
            threads: vec![GShareThreadSnapshot::new(); config.threads()],
            config,
            lookup_count: 0,
            history_update_count: 0,
            update_count: 0,
            squash_count: 0,
        }
    }

    pub const fn config(&self) -> &GShareBranchPredictorConfig {
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
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let global_history = self.threads[thread_index].global_history();
        self.predict_with_history(cpu, pc, global_history)
    }

    pub(crate) fn predict_with_global_history(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
        self.thread_index(cpu)?;
        self.predict_with_history(cpu, pc, global_history & self.config.history_mask())
    }

    pub(crate) fn global_history(&self, cpu: CpuId) -> Result<u64, GShareBranchPredictorError> {
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
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
        let index = self.index(pc, global_history);
        let counter = self.counters[index];
        let predicted_taken = counter > self.config.taken_threshold();
        let history = GShareHistory {
            cpu,
            pc,
            index,
            global_history_before: global_history,
            predicted_taken,
            counter,
        };

        self.lookup_count += 1;

        Ok(GSharePrediction {
            history,
            lookup_count: self.lookup_count,
        })
    }

    pub fn predict_unconditional(
        &mut self,
        cpu: CpuId,
        pc: Address,
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let global_history = self.threads[thread_index].global_history();
        self.predict_unconditional_with_history(cpu, pc, global_history)
    }

    pub(crate) fn predict_unconditional_with_global_history(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
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
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
        let index = self.index(pc, global_history);
        let counter = self.counters[index];
        let history = GShareHistory {
            cpu,
            pc,
            index,
            global_history_before: global_history,
            predicted_taken: true,
            counter,
        };

        self.lookup_count += 1;

        Ok(GSharePrediction {
            history,
            lookup_count: self.lookup_count,
        })
    }

    pub(crate) fn predict_with_global_history_and_direction(
        &mut self,
        cpu: CpuId,
        pc: Address,
        global_history: u64,
        predicted_taken: bool,
    ) -> Result<GSharePrediction, GShareBranchPredictorError> {
        self.thread_index(cpu)?;
        let global_history = global_history & self.config.history_mask();
        let index = self.index(pc, global_history);
        let counter = self.counters[index];
        let history = GShareHistory {
            cpu,
            pc,
            index,
            global_history_before: global_history,
            predicted_taken,
            counter,
        };

        self.lookup_count += 1;

        Ok(GSharePrediction {
            history,
            lookup_count: self.lookup_count,
        })
    }

    pub fn update_history(
        &mut self,
        history: &GShareHistory,
        taken: bool,
    ) -> Result<GShareHistoryUpdate, GShareBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_history = self.threads[thread_index].global_history();
        if old_history != history.global_history_before() {
            return Err(GShareBranchPredictorError::HistoryUpdateOutOfOrder {
                cpu: history.cpu(),
                expected_history: old_history,
                actual_history: history.global_history_before(),
            });
        }
        let new_history = self.shift_history(old_history, taken);
        self.threads[thread_index].global_history = new_history;
        self.history_update_count += 1;

        Ok(GShareHistoryUpdate {
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
        history: &GShareHistory,
        actual_taken: bool,
        squashed: bool,
    ) -> Result<GShareTrainingUpdate, GShareBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_counter = self.counters[history.index()];

        if squashed {
            let repaired_history =
                self.shift_history(history.global_history_before(), actual_taken);
            self.threads[thread_index].global_history = repaired_history;
            self.squash_count += 1;

            return Ok(GShareTrainingUpdate {
                cpu: history.cpu(),
                pc: history.pc(),
                index: history.index(),
                actual_taken,
                squashed,
                old_counter,
                new_counter: old_counter,
                update_count: self.update_count,
                repaired_history: Some(repaired_history),
            });
        }

        let new_counter = saturating_counter(old_counter, actual_taken, self.config.counter_max());
        self.counters[history.index()] = new_counter;
        self.update_count += 1;

        Ok(GShareTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            index: history.index(),
            actual_taken,
            squashed,
            old_counter,
            new_counter,
            update_count: self.update_count,
            repaired_history: None,
        })
    }

    pub fn squash(
        &mut self,
        history: &GShareHistory,
    ) -> Result<GShareSquash, GShareBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_history = self.threads[thread_index].global_history();
        self.threads[thread_index].global_history = history.global_history_before();
        self.squash_count += 1;

        Ok(GShareSquash {
            cpu: history.cpu(),
            pc: history.pc(),
            old_history,
            restored_history: history.global_history_before(),
            squash_count: self.squash_count,
        })
    }

    pub fn snapshot(&self) -> GShareBranchPredictorSnapshot {
        GShareBranchPredictorSnapshot {
            config: self.config.clone(),
            counters: self.counters.clone(),
            threads: self.threads.clone(),
            lookup_count: self.lookup_count,
            history_update_count: self.history_update_count,
            update_count: self.update_count,
            squash_count: self.squash_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &GShareBranchPredictorSnapshot,
    ) -> Result<(), GShareBranchPredictorError> {
        if self.config.threads() != snapshot.config.threads()
            || self.config.table_entries() != snapshot.config.table_entries()
        {
            return Err(GShareBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: self.config.threads(),
                actual_threads: snapshot.config.threads(),
                expected_entries: self.config.table_entries(),
                actual_entries: snapshot.config.table_entries(),
            });
        }
        if self.config != snapshot.config {
            return Err(GShareBranchPredictorError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }
        if snapshot.counters.len() != snapshot.config.table_entries()
            || snapshot.threads.len() != snapshot.config.threads()
        {
            return Err(GShareBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: snapshot.config.threads(),
                actual_threads: snapshot.threads.len(),
                expected_entries: snapshot.config.table_entries(),
                actual_entries: snapshot.counters.len(),
            });
        }

        self.counters.clone_from(&snapshot.counters);
        self.threads.clone_from(&snapshot.threads);
        self.lookup_count = snapshot.lookup_count;
        self.history_update_count = snapshot.history_update_count;
        self.update_count = snapshot.update_count;
        self.squash_count = snapshot.squash_count;
        Ok(())
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, GShareBranchPredictorError> {
        let index = cpu.get() as usize;
        if index < self.threads.len() {
            Ok(index)
        } else {
            Err(GShareBranchPredictorError::UnknownThread { cpu })
        }
    }

    fn index(&self, pc: Address, global_history: u64) -> usize {
        (((pc.get() >> self.config.inst_shift()) ^ global_history) & self.config.history_mask())
            as usize
    }

    fn shift_history(&self, old_history: u64, taken: bool) -> u64 {
        ((old_history << 1) | u64::from(taken)) & self.config.history_mask()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GSharePrediction {
    history: GShareHistory,
    lookup_count: u64,
}

impl GSharePrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn index(&self) -> usize {
        self.history.index()
    }

    pub const fn global_history_before(&self) -> u64 {
        self.history.global_history_before()
    }

    pub const fn counter(&self) -> u8 {
        self.history.counter()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn history(&self) -> &GShareHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GShareHistory {
    cpu: CpuId,
    pc: Address,
    index: usize,
    global_history_before: u64,
    predicted_taken: bool,
    counter: u8,
}

impl GShareHistory {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn global_history_before(&self) -> u64 {
        self.global_history_before
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn counter(&self) -> u8 {
        self.counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GShareHistoryUpdate {
    cpu: CpuId,
    pc: Address,
    old_history: u64,
    new_history: u64,
    taken: bool,
    history_update_count: u64,
}

impl GShareHistoryUpdate {
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
pub struct GShareTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    index: usize,
    actual_taken: bool,
    squashed: bool,
    old_counter: u8,
    new_counter: u8,
    update_count: u64,
    repaired_history: Option<u64>,
}

impl GShareTrainingUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn actual_taken(&self) -> bool {
        self.actual_taken
    }

    pub const fn squashed(&self) -> bool {
        self.squashed
    }

    pub const fn old_counter(&self) -> u8 {
        self.old_counter
    }

    pub const fn new_counter(&self) -> u8 {
        self.new_counter
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn repaired_history(&self) -> Option<u64> {
        self.repaired_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GShareSquash {
    cpu: CpuId,
    pc: Address,
    old_history: u64,
    restored_history: u64,
    squash_count: u64,
}

impl GShareSquash {
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
pub struct GShareThreadSnapshot {
    global_history: u64,
}

impl GShareThreadSnapshot {
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
pub struct GShareBranchPredictorSnapshot {
    config: GShareBranchPredictorConfig,
    counters: Vec<u8>,
    threads: Vec<GShareThreadSnapshot>,
    lookup_count: u64,
    history_update_count: u64,
    update_count: u64,
    squash_count: u64,
}

impl GShareBranchPredictorSnapshot {
    pub(crate) fn from_parts(
        config: GShareBranchPredictorConfig,
        counters: Vec<u8>,
        threads: Vec<GShareThreadSnapshot>,
        lookup_count: u64,
        history_update_count: u64,
        update_count: u64,
        squash_count: u64,
    ) -> Self {
        Self {
            config,
            counters,
            threads,
            lookup_count,
            history_update_count,
            update_count,
            squash_count,
        }
    }

    pub const fn config(&self) -> &GShareBranchPredictorConfig {
        &self.config
    }

    pub fn counters(&self) -> &[u8] {
        &self.counters
    }

    pub fn threads(&self) -> &[GShareThreadSnapshot] {
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

fn saturating_counter(counter: u8, taken: bool, max: u8) -> u8 {
    if taken {
        counter.saturating_add(1).min(max)
    } else {
        counter.saturating_sub(1)
    }
}
