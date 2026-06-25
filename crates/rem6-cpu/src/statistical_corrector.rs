use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::CpuId;

const MAX_LOG_ENTRIES: u8 = 20;
const MAX_HISTORY_BITS: u8 = 62;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatisticalCorrectorError {
    ZeroThreads,
    LogSizeOutOfRange {
        field: &'static str,
        bits: u8,
    },
    LogSizeUpTooSmall {
        bits: u8,
    },
    EmptyLengths {
        field: &'static str,
    },
    HistoryLengthOutOfRange {
        field: &'static str,
        bits: u8,
    },
    LocalHistoryEntriesNotPowerOfTwo {
        entries: usize,
    },
    CounterWidthOutOfRange {
        field: &'static str,
        bits: u8,
    },
    InstShiftOutOfRange {
        bits: u8,
    },
    UnknownThread {
        cpu: CpuId,
    },
    TableIndexOutOfRange {
        table: &'static str,
        index: usize,
        entries: usize,
    },
    CounterValueOutOfRange {
        value: i8,
        min: i8,
        max: i8,
    },
    SnapshotConfigMismatch {
        expected: Box<StatisticalCorrectorConfig>,
        actual: Box<StatisticalCorrectorConfig>,
    },
}

impl fmt::Display for StatisticalCorrectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "statistical corrector has no threads"),
            Self::LogSizeOutOfRange { field, bits } => write!(
                formatter,
                "statistical corrector {field} log size {bits} is outside 1..={MAX_LOG_ENTRIES}"
            ),
            Self::LogSizeUpTooSmall { bits } => write!(
                formatter,
                "statistical corrector update log size {bits} is smaller than 2"
            ),
            Self::EmptyLengths { field } => {
                write!(formatter, "statistical corrector {field} lengths are empty")
            }
            Self::HistoryLengthOutOfRange { field, bits } => write!(
                formatter,
                "statistical corrector {field} history length {bits} is outside 1..={MAX_HISTORY_BITS}"
            ),
            Self::LocalHistoryEntriesNotPowerOfTwo { entries } => write!(
                formatter,
                "statistical corrector local history entries {entries} is not a power of two"
            ),
            Self::CounterWidthOutOfRange { field, bits } => write!(
                formatter,
                "statistical corrector {field} counter width {bits} is outside the supported range"
            ),
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "statistical corrector instruction shift {bits} is outside 0..=63"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "statistical corrector thread {} is not configured",
                cpu.get()
            ),
            Self::TableIndexOutOfRange {
                table,
                index,
                entries,
            } => write!(
                formatter,
                "statistical corrector {table} index {index} is outside {entries} entries"
            ),
            Self::CounterValueOutOfRange { value, min, max } => write!(
                formatter,
                "statistical corrector counter value {value} is outside {min}..={max}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "statistical corrector snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
        }
    }
}

impl Error for StatisticalCorrectorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorConfig {
    pub(crate) threads: usize,
    pub(crate) log_bias: u8,
    pub(crate) log_size_up: u8,
    pub(crate) log_size_ups: u8,
    pub(crate) num_entries_first_local_histories: usize,
    pub(crate) global_lengths: Vec<u8>,
    pub(crate) backward_lengths: Vec<u8>,
    pub(crate) local_lengths: Vec<u8>,
    pub(crate) imli_lengths: Vec<u8>,
    pub(crate) log_global: u8,
    pub(crate) log_backward: u8,
    pub(crate) log_local: u8,
    pub(crate) log_imli: u8,
    pub(crate) global_weight_init: i8,
    pub(crate) backward_weight_init: i8,
    pub(crate) local_weight_init: i8,
    pub(crate) imli_weight_init: i8,
    pub(crate) chooser_conf_width: u8,
    pub(crate) update_threshold_width: u8,
    pub(crate) per_pc_threshold_width: u8,
    pub(crate) extra_weights_width: u8,
    pub(crate) counter_width: u8,
    pub(crate) initial_update_threshold: i16,
    pub(crate) inst_shift: u8,
    pub(crate) speculative_history: bool,
}

impl StatisticalCorrectorConfig {
    pub fn tage_sc_l_8kb(
        threads: usize,
        inst_shift: u8,
        speculative_history: bool,
    ) -> Result<Self, StatisticalCorrectorError> {
        Self::with_options(
            threads,
            7,
            6,
            64,
            vec![6, 3],
            vec![16, 8],
            vec![6, 3],
            vec![8],
            7,
            7,
            7,
            7,
            7,
            7,
            7,
            7,
            7,
            12,
            8,
            6,
            6,
            0,
            inst_shift,
            speculative_history,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        threads: usize,
        log_bias: u8,
        log_size_up: u8,
        num_entries_first_local_histories: usize,
        global_lengths: Vec<u8>,
        backward_lengths: Vec<u8>,
        local_lengths: Vec<u8>,
        imli_lengths: Vec<u8>,
        log_global: u8,
        log_backward: u8,
        log_local: u8,
        log_imli: u8,
        global_weight_init: i8,
        backward_weight_init: i8,
        local_weight_init: i8,
        imli_weight_init: i8,
        chooser_conf_width: u8,
        update_threshold_width: u8,
        per_pc_threshold_width: u8,
        extra_weights_width: u8,
        counter_width: u8,
        initial_update_threshold: i16,
        inst_shift: u8,
        speculative_history: bool,
    ) -> Result<Self, StatisticalCorrectorError> {
        if threads == 0 {
            return Err(StatisticalCorrectorError::ZeroThreads);
        }
        check_log_size("bias", log_bias)?;
        if log_size_up < 2 {
            return Err(StatisticalCorrectorError::LogSizeUpTooSmall { bits: log_size_up });
        }
        check_log_size("update", log_size_up)?;
        check_log_size("global", log_global)?;
        check_log_size("backward", log_backward)?;
        check_log_size("local", log_local)?;
        check_log_size("imli", log_imli)?;
        check_lengths("global", &global_lengths)?;
        check_lengths("backward", &backward_lengths)?;
        check_lengths("local", &local_lengths)?;
        check_lengths("imli", &imli_lengths)?;
        if !num_entries_first_local_histories.is_power_of_two() {
            return Err(
                StatisticalCorrectorError::LocalHistoryEntriesNotPowerOfTwo {
                    entries: num_entries_first_local_histories,
                },
            );
        }
        check_counter_width("chooser", chooser_conf_width)?;
        check_wide_counter_width("update threshold", update_threshold_width)?;
        check_wide_counter_width("per-pc update threshold", per_pc_threshold_width)?;
        check_counter_width("extra weights", extra_weights_width)?;
        check_counter_width("sc", counter_width)?;
        if inst_shift > 63 {
            return Err(StatisticalCorrectorError::InstShiftOutOfRange { bits: inst_shift });
        }

        Ok(Self {
            threads,
            log_bias,
            log_size_up,
            log_size_ups: log_size_up / 2,
            num_entries_first_local_histories,
            global_lengths,
            backward_lengths,
            local_lengths,
            imli_lengths,
            log_global,
            log_backward,
            log_local,
            log_imli,
            global_weight_init,
            backward_weight_init,
            local_weight_init,
            imli_weight_init,
            chooser_conf_width,
            update_threshold_width,
            per_pc_threshold_width,
            extra_weights_width,
            counter_width,
            initial_update_threshold,
            inst_shift,
            speculative_history,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    pub const fn speculative_history(&self) -> bool {
        self.speculative_history
    }

    fn bias_entries(&self) -> usize {
        1usize << self.log_bias
    }

    fn update_entries(&self) -> usize {
        1usize << self.log_size_up
    }

    fn update_weight_entries(&self) -> usize {
        1usize << self.log_size_ups
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticalCorrectorBranchKind {
    DirectConditional,
    DirectUnconditional,
    IndirectConditional,
    IndirectUnconditional,
}

impl StatisticalCorrectorBranchKind {
    const fn conditional(self) -> bool {
        matches!(self, Self::DirectConditional | Self::IndirectConditional)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorInput {
    previous_prediction: bool,
    bias_bit: bool,
    use_confidence_counter: bool,
    confidence_counter: i8,
    confidence_bits: u8,
    hit_bank: usize,
    alt_bank: usize,
    low_confidence: bool,
    medium_confidence: bool,
    high_confidence: bool,
    alternate_confidence: bool,
    initial_sum: i16,
}

impl StatisticalCorrectorInput {
    pub const fn new(previous_prediction: bool) -> Self {
        Self {
            previous_prediction,
            bias_bit: false,
            use_confidence_counter: false,
            confidence_counter: 0,
            confidence_bits: 1,
            hit_bank: 0,
            alt_bank: 0,
            low_confidence: false,
            medium_confidence: false,
            high_confidence: false,
            alternate_confidence: false,
            initial_sum: 0,
        }
    }

    pub const fn with_bias_bit(mut self, bias_bit: bool) -> Self {
        self.bias_bit = bias_bit;
        self
    }

    pub const fn with_tage_counter(mut self, counter: i8, bits: u8) -> Self {
        self.use_confidence_counter = true;
        self.confidence_counter = counter;
        self.confidence_bits = bits;
        self
    }

    pub const fn with_banks(mut self, hit_bank: usize, alt_bank: usize) -> Self {
        self.hit_bank = hit_bank;
        self.alt_bank = alt_bank;
        self
    }

    pub const fn with_confidences(
        mut self,
        low: bool,
        medium: bool,
        high: bool,
        alternate: bool,
    ) -> Self {
        self.low_confidence = low;
        self.medium_confidence = medium;
        self.high_confidence = high;
        self.alternate_confidence = alternate;
        self
    }

    pub const fn with_initial_sum(mut self, initial_sum: i16) -> Self {
        self.initial_sum = initial_sum;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Confidence {
    low: bool,
    medium: bool,
    high: bool,
    alternate: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrector {
    config: StatisticalCorrectorConfig,
    global_gehl: Vec<Vec<i8>>,
    backward_gehl: Vec<Vec<i8>>,
    local_gehl: Vec<Vec<i8>>,
    imli_gehl: Vec<Vec<i8>>,
    global_weights: Vec<i8>,
    backward_weights: Vec<i8>,
    local_weights: Vec<i8>,
    imli_weights: Vec<i8>,
    bias: Vec<i8>,
    bias_sk: Vec<i8>,
    bias_bank: Vec<i8>,
    bias_weights: Vec<i8>,
    update_threshold: i16,
    per_pc_update_threshold: Vec<i16>,
    threads: Vec<StatisticalCorrectorThreadSnapshot>,
    first_chooser: i8,
    second_chooser: i8,
    lookup_count: u64,
    update_count: u64,
    history_update_count: u64,
    correct_count: u64,
    wrong_count: u64,
}

impl StatisticalCorrector {
    pub fn new(config: StatisticalCorrectorConfig) -> Self {
        let mut predictor = Self {
            global_gehl: init_gehl_tables(&config.global_lengths, config.log_global),
            backward_gehl: init_gehl_tables(&config.backward_lengths, config.log_backward),
            local_gehl: init_gehl_tables(&config.local_lengths, config.log_local),
            imli_gehl: init_gehl_tables(&config.imli_lengths, config.log_imli),
            global_weights: vec![config.global_weight_init; config.update_weight_entries()],
            backward_weights: vec![config.backward_weight_init; config.update_weight_entries()],
            local_weights: vec![config.local_weight_init; config.update_weight_entries()],
            imli_weights: vec![config.imli_weight_init; config.update_weight_entries()],
            bias: vec![0; config.bias_entries()],
            bias_sk: vec![0; config.bias_entries()],
            bias_bank: vec![0; config.bias_entries()],
            bias_weights: vec![4; config.update_weight_entries()],
            update_threshold: 35 << 3,
            per_pc_update_threshold: vec![config.initial_update_threshold; config.update_entries()],
            threads: vec![StatisticalCorrectorThreadSnapshot::new(&config); config.threads()],
            first_chooser: 0,
            second_chooser: 0,
            lookup_count: 0,
            update_count: 0,
            history_update_count: 0,
            correct_count: 0,
            wrong_count: 0,
            config,
        };
        predictor.init_bias();
        predictor
    }

    pub const fn config(&self) -> &StatisticalCorrectorConfig {
        &self.config
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
        input: StatisticalCorrectorInput,
    ) -> Result<StatisticalCorrectorPrediction, StatisticalCorrectorError> {
        let thread_index = self.thread_index(cpu)?;
        self.lookup_count += 1;
        let thread_before = self.threads[thread_index].clone();
        let confidence = self.confidence(input);

        if !conditional {
            let history = StatisticalCorrectorHistory::unconditional(
                cpu,
                pc,
                input.previous_prediction,
                thread_before,
            );
            return Ok(StatisticalCorrectorPrediction {
                history,
                lookup_count: self.lookup_count,
            });
        }

        let local_entry = self.local_history_entry(pc);
        let local_history = thread_before.first_local_histories[local_entry];
        let update_index = self.update_index(pc);
        let update_weight_index = self.update_weight_index(pc);
        let bias_index = self.bias_index(pc, input.previous_prediction, input.bias_bit, confidence);
        let bias_sk_index = self.bias_sk_index(pc, input.previous_prediction, confidence);
        let bias_bank_index = self.bias_bank_index(input.previous_prediction, confidence, input);

        let mut linear_sum = input.initial_sum
            + self.bias_sum(
                bias_index,
                bias_sk_index,
                bias_bank_index,
                update_weight_index,
            );

        let global_indices = self.gehl_indices(
            pc,
            thread_before.global_history,
            &self.config.global_lengths,
            self.config.log_global,
        );
        linear_sum += self.gehl_sum(
            &self.global_gehl,
            &self.global_weights,
            &global_indices,
            update_weight_index,
        );

        let backward_indices = self.gehl_indices(
            pc,
            thread_before.backward_history,
            &self.config.backward_lengths,
            self.config.log_backward,
        );
        linear_sum += self.gehl_sum(
            &self.backward_gehl,
            &self.backward_weights,
            &backward_indices,
            update_weight_index,
        );

        let local_indices = self.gehl_indices(
            pc,
            local_history,
            &self.config.local_lengths,
            self.config.log_local,
        );
        linear_sum += self.gehl_sum(
            &self.local_gehl,
            &self.local_weights,
            &local_indices,
            update_weight_index,
        );

        let imli_indices = self.gehl_indices(
            pc,
            thread_before.imli_count,
            &self.config.imli_lengths,
            self.config.log_imli,
        );
        linear_sum += self.gehl_sum(
            &self.imli_gehl,
            &self.imli_weights,
            &imli_indices,
            update_weight_index,
        );

        let threshold = (self.update_threshold >> 3) + self.per_pc_update_threshold[update_index];
        let sc_predicted_taken = linear_sum >= 0;
        let mut used_sc_prediction = false;
        let mut predicted_taken = input.previous_prediction;

        if input.previous_prediction != sc_predicted_taken {
            let magnitude = linear_sum.abs();
            used_sc_prediction = true;
            if confidence.high {
                if magnitude < threshold / 4 {
                    used_sc_prediction = false;
                } else if magnitude < threshold / 2 {
                    used_sc_prediction = self.second_chooser < 0;
                }
            }
            if confidence.medium && magnitude < threshold / 4 {
                used_sc_prediction = self.first_chooser < 0;
            }
            if used_sc_prediction {
                predicted_taken = sc_predicted_taken;
            }
        }

        let history = StatisticalCorrectorHistory {
            cpu,
            pc,
            conditional,
            previous_prediction: input.previous_prediction,
            predicted_taken,
            sc_predicted_taken,
            used_sc_prediction,
            bias_bit: input.bias_bit,
            hit_bank: input.hit_bank,
            alt_bank: input.alt_bank,
            confidence,
            linear_sum,
            threshold,
            update_index,
            update_weight_index,
            bias_index,
            bias_sk_index,
            bias_bank_index,
            global_indices,
            backward_indices,
            local_indices,
            imli_indices,
            local_entry,
            local_history,
            thread_before,
        };

        Ok(StatisticalCorrectorPrediction {
            history,
            lookup_count: self.lookup_count,
        })
    }

    pub fn train(
        &mut self,
        history: &StatisticalCorrectorHistory,
        actual_taken: bool,
    ) -> Result<StatisticalCorrectorTrainingUpdate, StatisticalCorrectorError> {
        self.thread_index(history.cpu())?;
        let update_threshold_before = self.update_threshold;
        let per_pc_threshold_before = self.per_pc_update_threshold[history.update_index];
        let bias_before = (
            self.bias[history.bias_index],
            self.bias_sk[history.bias_sk_index],
            self.bias_bank[history.bias_bank_index],
        );
        let first_chooser_before = self.first_chooser;
        let second_chooser_before = self.second_chooser;

        if history.conditional() {
            self.update_chooser(history, actual_taken);
            if history.sc_predicted_taken == actual_taken {
                self.correct_count += 1;
            } else {
                self.wrong_count += 1;
            }

            if history.sc_predicted_taken != actual_taken
                || history.linear_sum.abs() < history.threshold
            {
                update_signed_i16(
                    &mut self.update_threshold,
                    history.sc_predicted_taken != actual_taken,
                    self.config.update_threshold_width,
                );
                update_signed_i16(
                    &mut self.per_pc_update_threshold[history.update_index],
                    history.sc_predicted_taken != actual_taken,
                    self.config.per_pc_threshold_width,
                );
                self.update_bias(history, actual_taken);
                let gehl_update = GehlUpdate {
                    update_weight_index: history.update_weight_index,
                    taken: actual_taken,
                    counter_width: self.config.counter_width,
                    weight_width: self.config.extra_weights_width,
                    linear_sum: history.linear_sum,
                };
                update_gehl_group(
                    &mut self.global_gehl,
                    &mut self.global_weights,
                    &history.global_indices,
                    gehl_update,
                );
                update_gehl_group(
                    &mut self.backward_gehl,
                    &mut self.backward_weights,
                    &history.backward_indices,
                    gehl_update,
                );
                update_gehl_group(
                    &mut self.local_gehl,
                    &mut self.local_weights,
                    &history.local_indices,
                    gehl_update,
                );
                update_gehl_group(
                    &mut self.imli_gehl,
                    &mut self.imli_weights,
                    &history.imli_indices,
                    gehl_update,
                );
            }
        }

        self.update_count += 1;

        Ok(StatisticalCorrectorTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            update_threshold_before,
            update_threshold_after: self.update_threshold,
            per_pc_threshold_before,
            per_pc_threshold_after: self.per_pc_update_threshold[history.update_index],
            bias_before,
            bias_after: (
                self.bias[history.bias_index],
                self.bias_sk[history.bias_sk_index],
                self.bias_bank[history.bias_bank_index],
            ),
            global_counter_after: first_counter(&self.global_gehl, &history.global_indices),
            backward_counter_after: first_counter(&self.backward_gehl, &history.backward_indices),
            local_counter_after: first_counter(&self.local_gehl, &history.local_indices),
            imli_counter_after: first_counter(&self.imli_gehl, &history.imli_indices),
            first_chooser_before,
            first_chooser_after: self.first_chooser,
            second_chooser_before,
            second_chooser_after: self.second_chooser,
            update_count: self.update_count,
        })
    }

    pub fn update_history(
        &mut self,
        history: &StatisticalCorrectorHistory,
        kind: StatisticalCorrectorBranchKind,
        taken: bool,
        target: Address,
        path_history: u64,
    ) -> Result<StatisticalCorrectorHistoryUpdate, StatisticalCorrectorError> {
        self.apply_history_update(history, kind, taken, target, path_history, false)
    }

    pub fn repair_history(
        &mut self,
        history: &StatisticalCorrectorHistory,
        kind: StatisticalCorrectorBranchKind,
        taken: bool,
        target: Address,
        path_history: u64,
    ) -> Result<StatisticalCorrectorHistoryUpdate, StatisticalCorrectorError> {
        self.apply_history_update(history, kind, taken, target, path_history, true)
    }

    pub fn write_bias_entries(
        &mut self,
        bias_index: usize,
        bias_sk_index: usize,
        bias_bank_index: usize,
        bias_value: i8,
        bias_sk_value: i8,
        bias_bank_value: i8,
    ) -> Result<(), StatisticalCorrectorError> {
        self.check_bias_index("bias", bias_index)?;
        self.check_bias_index("bias_sk", bias_sk_index)?;
        self.check_bias_index("bias_bank", bias_bank_index)?;
        for value in [bias_value, bias_sk_value, bias_bank_value] {
            check_counter_value(value, self.config.counter_width)?;
        }
        self.bias[bias_index] = bias_value;
        self.bias_sk[bias_sk_index] = bias_sk_value;
        self.bias_bank[bias_bank_index] = bias_bank_value;
        Ok(())
    }

    pub fn snapshot(&self) -> StatisticalCorrectorSnapshot {
        StatisticalCorrectorSnapshot {
            config: self.config.clone(),
            global_gehl: self.global_gehl.clone(),
            backward_gehl: self.backward_gehl.clone(),
            local_gehl: self.local_gehl.clone(),
            imli_gehl: self.imli_gehl.clone(),
            global_weights: self.global_weights.clone(),
            backward_weights: self.backward_weights.clone(),
            local_weights: self.local_weights.clone(),
            imli_weights: self.imli_weights.clone(),
            bias: self.bias.clone(),
            bias_sk: self.bias_sk.clone(),
            bias_bank: self.bias_bank.clone(),
            bias_weights: self.bias_weights.clone(),
            update_threshold: self.update_threshold,
            per_pc_update_threshold: self.per_pc_update_threshold.clone(),
            threads: self.threads.clone(),
            first_chooser: self.first_chooser,
            second_chooser: self.second_chooser,
            lookup_count: self.lookup_count,
            update_count: self.update_count,
            history_update_count: self.history_update_count,
            correct_count: self.correct_count,
            wrong_count: self.wrong_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &StatisticalCorrectorSnapshot,
    ) -> Result<(), StatisticalCorrectorError> {
        if self.config != snapshot.config {
            return Err(StatisticalCorrectorError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        self.global_gehl.clone_from(&snapshot.global_gehl);
        self.backward_gehl.clone_from(&snapshot.backward_gehl);
        self.local_gehl.clone_from(&snapshot.local_gehl);
        self.imli_gehl.clone_from(&snapshot.imli_gehl);
        self.global_weights.clone_from(&snapshot.global_weights);
        self.backward_weights.clone_from(&snapshot.backward_weights);
        self.local_weights.clone_from(&snapshot.local_weights);
        self.imli_weights.clone_from(&snapshot.imli_weights);
        self.bias.clone_from(&snapshot.bias);
        self.bias_sk.clone_from(&snapshot.bias_sk);
        self.bias_bank.clone_from(&snapshot.bias_bank);
        self.bias_weights.clone_from(&snapshot.bias_weights);
        self.update_threshold = snapshot.update_threshold;
        self.per_pc_update_threshold
            .clone_from(&snapshot.per_pc_update_threshold);
        self.threads.clone_from(&snapshot.threads);
        self.first_chooser = snapshot.first_chooser;
        self.second_chooser = snapshot.second_chooser;
        self.lookup_count = snapshot.lookup_count;
        self.update_count = snapshot.update_count;
        self.history_update_count = snapshot.history_update_count;
        self.correct_count = snapshot.correct_count;
        self.wrong_count = snapshot.wrong_count;
        Ok(())
    }

    fn apply_history_update(
        &mut self,
        history: &StatisticalCorrectorHistory,
        kind: StatisticalCorrectorBranchKind,
        taken: bool,
        target: Address,
        path_history: u64,
        repair: bool,
    ) -> Result<StatisticalCorrectorHistoryUpdate, StatisticalCorrectorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_thread = self.threads[thread_index].clone();
        let mut new_thread = if repair {
            history.thread_before.clone()
        } else {
            old_thread.clone()
        };
        new_thread.path_history = path_history;

        if kind.conditional() {
            new_thread.global_history = (new_thread.global_history << 1) | u64::from(taken);
            let backward = target.get() < history.pc.get();
            new_thread.backward_history =
                (new_thread.backward_history << 1) | u64::from(taken && backward);
            if backward {
                if taken {
                    let max = bit_mask_u64(self.config.imli_lengths[0]);
                    new_thread.imli_count = new_thread.imli_count.saturating_add(1).min(max);
                } else {
                    new_thread.imli_count = 0;
                }
            }
            let local_entry = self.local_history_entry(history.pc);
            new_thread.first_local_histories[local_entry] =
                (new_thread.first_local_histories[local_entry] << 1) | u64::from(taken);
        }

        self.threads[thread_index] = new_thread.clone();
        self.history_update_count += 1;

        Ok(StatisticalCorrectorHistoryUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            old_thread,
            new_thread,
            repaired: repair,
            history_update_count: self.history_update_count,
        })
    }

    fn init_bias(&mut self) {
        for index in 0..self.bias.len() {
            match index & 3 {
                0 => {
                    self.bias[index] = -32;
                    self.bias_sk[index] = -8;
                    self.bias_bank[index] = -32;
                }
                1 => {
                    self.bias[index] = 31;
                    self.bias_sk[index] = 7;
                    self.bias_bank[index] = 31;
                }
                2 => {
                    self.bias[index] = -1;
                    self.bias_sk[index] = -32;
                    self.bias_bank[index] = -1;
                }
                _ => {
                    self.bias[index] = 0;
                    self.bias_sk[index] = 31;
                    self.bias_bank[index] = 0;
                }
            }
        }
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, StatisticalCorrectorError> {
        let index = cpu.get() as usize;
        if index < self.config.threads() {
            Ok(index)
        } else {
            Err(StatisticalCorrectorError::UnknownThread { cpu })
        }
    }

    fn confidence(&self, input: StatisticalCorrectorInput) -> Confidence {
        if input.use_confidence_counter {
            let strength = (2 * i16::from(input.confidence_counter) + 1).abs();
            Confidence {
                low: strength == 1,
                medium: strength == 5,
                high: strength >= ((1i16 << input.confidence_bits) - 1),
                alternate: input.alternate_confidence,
            }
        } else {
            Confidence {
                low: input.low_confidence,
                medium: input.medium_confidence,
                high: input.high_confidence,
                alternate: input.alternate_confidence,
            }
        }
    }

    fn bias_index(
        &self,
        pc: Address,
        previous_prediction: bool,
        bias_bit: bool,
        confidence: Confidence,
    ) -> usize {
        let shifted = pc.get() >> self.config.inst_shift;
        (((((shifted ^ (shifted >> 2)) << 1) ^ u64::from(confidence.low && bias_bit)) << 1)
            + u64::from(previous_prediction)) as usize
            & (self.config.bias_entries() - 1)
    }

    fn bias_sk_index(
        &self,
        pc: Address,
        previous_prediction: bool,
        confidence: Confidence,
    ) -> usize {
        let shifted = pc.get() >> self.config.inst_shift;
        (((((shifted ^ (shifted >> (self.config.log_bias - 2))) << 1)
            ^ u64::from(confidence.high))
            << 1)
            + u64::from(previous_prediction)) as usize
            & (self.config.bias_entries() - 1)
    }

    fn bias_bank_index(
        &self,
        previous_prediction: bool,
        confidence: Confidence,
        input: StatisticalCorrectorInput,
    ) -> usize {
        (usize::from(previous_prediction)
            + (((input.hit_bank + 1) / 4) << 4)
            + (usize::from(confidence.high) << 1)
            + (usize::from(confidence.low) << 2)
            + (usize::from(input.alt_bank != 0) << 3))
            & (self.config.bias_entries() - 1)
    }

    fn update_index(&self, pc: Address) -> usize {
        let shifted = pc.get() >> self.config.inst_shift;
        ((shifted ^ (shifted >> 2)) as usize) & (self.config.update_entries() - 1)
    }

    fn update_weight_index(&self, pc: Address) -> usize {
        let shifted = pc.get() >> self.config.inst_shift;
        ((shifted ^ (shifted >> 2)) as usize) & (self.config.update_weight_entries() - 1)
    }

    fn local_history_entry(&self, pc: Address) -> usize {
        let shifted = pc.get() >> self.config.inst_shift;
        ((shifted ^ (shifted >> 2)) as usize) & (self.config.num_entries_first_local_histories - 1)
    }

    fn gehl_indices(&self, pc: Address, hist: u64, lengths: &[u8], log_entries: u8) -> Vec<usize> {
        lengths
            .iter()
            .enumerate()
            .map(|(index, length)| {
                let bhist = hist & bit_mask_u64(*length);
                let folded = (pc.get() >> self.config.inst_shift)
                    ^ bhist
                    ^ shift_right_or_zero(bhist, 8usize.saturating_sub(index))
                    ^ shift_right_or_zero(bhist, 16usize.saturating_sub(2 * index))
                    ^ shift_right_or_zero(bhist, 24usize.saturating_sub(3 * index))
                    ^ shift_right_or_zero(bhist, 32usize.saturating_sub(3 * index))
                    ^ shift_right_or_zero(bhist, 40usize.saturating_sub(4 * index));
                (folded as usize) & ((1usize << log_entries) - 1)
            })
            .collect()
    }

    fn bias_sum(
        &self,
        bias_index: usize,
        bias_sk_index: usize,
        bias_bank_index: usize,
        update_weight_index: usize,
    ) -> i16 {
        let sum = perceptron_term(self.bias[bias_index])
            + perceptron_term(self.bias_sk[bias_sk_index])
            + perceptron_term(self.bias_bank[bias_bank_index]);
        if self.bias_weights[update_weight_index] >= 0 {
            2 * sum
        } else {
            sum
        }
    }

    fn gehl_sum(
        &self,
        table: &[Vec<i8>],
        weights: &[i8],
        indices: &[usize],
        update_weight_index: usize,
    ) -> i16 {
        let sum = table
            .iter()
            .zip(indices)
            .fold(0, |sum, (row, index)| sum + perceptron_term(row[*index]));
        if weights[update_weight_index] >= 0 {
            2 * sum
        } else {
            sum
        }
    }

    fn update_chooser(&mut self, history: &StatisticalCorrectorHistory, actual_taken: bool) {
        if history.previous_prediction == history.sc_predicted_taken {
            return;
        }
        let magnitude = history.linear_sum.abs();
        if magnitude < history.threshold
            && history.confidence.high
            && magnitude >= history.threshold / 4
            && magnitude < history.threshold / 2
        {
            update_signed_i8(
                &mut self.second_chooser,
                history.previous_prediction == actual_taken,
                self.config.chooser_conf_width,
            );
        }
        if history.confidence.medium && magnitude < history.threshold / 4 {
            update_signed_i8(
                &mut self.first_chooser,
                history.previous_prediction == actual_taken,
                self.config.chooser_conf_width,
            );
        }
    }

    fn update_bias(&mut self, history: &StatisticalCorrectorHistory, actual_taken: bool) {
        let bias_sum = perceptron_term(self.bias[history.bias_index])
            + perceptron_term(self.bias_sk[history.bias_sk_index])
            + perceptron_term(self.bias_bank[history.bias_bank_index]);
        let xsum = history.linear_sum
            - if self.bias_weights[history.update_weight_index] >= 0 {
                bias_sum
            } else {
                0
            };
        if sign_non_negative(xsum + bias_sum) != sign_non_negative(xsum) {
            update_signed_i8(
                &mut self.bias_weights[history.update_weight_index],
                sign_non_negative(bias_sum) == actual_taken,
                self.config.extra_weights_width,
            );
        }
        update_signed_i8(
            &mut self.bias[history.bias_index],
            actual_taken,
            self.config.counter_width,
        );
        update_signed_i8(
            &mut self.bias_sk[history.bias_sk_index],
            actual_taken,
            self.config.counter_width,
        );
        update_signed_i8(
            &mut self.bias_bank[history.bias_bank_index],
            actual_taken,
            self.config.counter_width,
        );
    }

    fn check_bias_index(
        &self,
        table: &'static str,
        index: usize,
    ) -> Result<(), StatisticalCorrectorError> {
        if index >= self.bias.len() {
            Err(StatisticalCorrectorError::TableIndexOutOfRange {
                table,
                index,
                entries: self.bias.len(),
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorPrediction {
    history: StatisticalCorrectorHistory,
    lookup_count: u64,
}

impl StatisticalCorrectorPrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn sc_predicted_taken(&self) -> bool {
        self.history.sc_predicted_taken()
    }

    pub const fn used_sc_prediction(&self) -> bool {
        self.history.used_sc_prediction()
    }

    pub const fn linear_sum(&self) -> i16 {
        self.history.linear_sum()
    }

    pub const fn threshold(&self) -> i16 {
        self.history.threshold()
    }

    pub const fn bias_index(&self) -> usize {
        self.history.bias_index()
    }

    pub const fn bias_sk_index(&self) -> usize {
        self.history.bias_sk_index()
    }

    pub const fn bias_bank_index(&self) -> usize {
        self.history.bias_bank_index()
    }

    pub const fn update_index(&self) -> usize {
        self.history.update_index()
    }

    pub const fn update_weight_index(&self) -> usize {
        self.history.update_weight_index()
    }

    pub fn global_indices(&self) -> &[usize] {
        self.history.global_indices()
    }

    pub fn backward_indices(&self) -> &[usize] {
        self.history.backward_indices()
    }

    pub fn local_indices(&self) -> &[usize] {
        self.history.local_indices()
    }

    pub fn imli_indices(&self) -> &[usize] {
        self.history.imli_indices()
    }

    pub const fn local_history(&self) -> u64 {
        self.history.local_history()
    }

    pub const fn history(&self) -> &StatisticalCorrectorHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorHistory {
    cpu: CpuId,
    pc: Address,
    conditional: bool,
    previous_prediction: bool,
    predicted_taken: bool,
    sc_predicted_taken: bool,
    used_sc_prediction: bool,
    bias_bit: bool,
    hit_bank: usize,
    alt_bank: usize,
    confidence: Confidence,
    linear_sum: i16,
    threshold: i16,
    update_index: usize,
    update_weight_index: usize,
    bias_index: usize,
    bias_sk_index: usize,
    bias_bank_index: usize,
    global_indices: Vec<usize>,
    backward_indices: Vec<usize>,
    local_indices: Vec<usize>,
    imli_indices: Vec<usize>,
    local_entry: usize,
    local_history: u64,
    thread_before: StatisticalCorrectorThreadSnapshot,
}

impl StatisticalCorrectorHistory {
    fn unconditional(
        cpu: CpuId,
        pc: Address,
        previous_prediction: bool,
        thread_before: StatisticalCorrectorThreadSnapshot,
    ) -> Self {
        Self {
            cpu,
            pc,
            conditional: false,
            previous_prediction,
            predicted_taken: previous_prediction,
            sc_predicted_taken: previous_prediction,
            used_sc_prediction: false,
            bias_bit: false,
            hit_bank: 0,
            alt_bank: 0,
            confidence: Confidence {
                low: false,
                medium: false,
                high: false,
                alternate: false,
            },
            linear_sum: 0,
            threshold: 0,
            update_index: 0,
            update_weight_index: 0,
            bias_index: 0,
            bias_sk_index: 0,
            bias_bank_index: 0,
            global_indices: Vec::new(),
            backward_indices: Vec::new(),
            local_indices: Vec::new(),
            imli_indices: Vec::new(),
            local_entry: 0,
            local_history: 0,
            thread_before,
        }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn conditional(&self) -> bool {
        self.conditional
    }

    pub const fn previous_prediction(&self) -> bool {
        self.previous_prediction
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn sc_predicted_taken(&self) -> bool {
        self.sc_predicted_taken
    }

    pub const fn used_sc_prediction(&self) -> bool {
        self.used_sc_prediction
    }

    pub const fn linear_sum(&self) -> i16 {
        self.linear_sum
    }

    pub const fn threshold(&self) -> i16 {
        self.threshold
    }

    pub const fn update_index(&self) -> usize {
        self.update_index
    }

    pub const fn update_weight_index(&self) -> usize {
        self.update_weight_index
    }

    pub const fn bias_index(&self) -> usize {
        self.bias_index
    }

    pub const fn bias_sk_index(&self) -> usize {
        self.bias_sk_index
    }

    pub const fn bias_bank_index(&self) -> usize {
        self.bias_bank_index
    }

    pub fn global_indices(&self) -> &[usize] {
        &self.global_indices
    }

    pub fn backward_indices(&self) -> &[usize] {
        &self.backward_indices
    }

    pub fn local_indices(&self) -> &[usize] {
        &self.local_indices
    }

    pub fn imli_indices(&self) -> &[usize] {
        &self.imli_indices
    }

    pub const fn local_history(&self) -> u64 {
        self.local_history
    }

    pub const fn thread_before(&self) -> &StatisticalCorrectorThreadSnapshot {
        &self.thread_before
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    update_threshold_before: i16,
    update_threshold_after: i16,
    per_pc_threshold_before: i16,
    per_pc_threshold_after: i16,
    bias_before: (i8, i8, i8),
    bias_after: (i8, i8, i8),
    global_counter_after: Option<i8>,
    backward_counter_after: Option<i8>,
    local_counter_after: Option<i8>,
    imli_counter_after: Option<i8>,
    first_chooser_before: i8,
    first_chooser_after: i8,
    second_chooser_before: i8,
    second_chooser_after: i8,
    update_count: u64,
}

impl StatisticalCorrectorTrainingUpdate {
    pub const fn update_threshold_before(&self) -> i16 {
        self.update_threshold_before
    }

    pub const fn update_threshold_after(&self) -> i16 {
        self.update_threshold_after
    }

    pub const fn per_pc_threshold_before(&self) -> i16 {
        self.per_pc_threshold_before
    }

    pub const fn per_pc_threshold_after(&self) -> i16 {
        self.per_pc_threshold_after
    }

    pub const fn bias_before(&self) -> (i8, i8, i8) {
        self.bias_before
    }

    pub const fn bias_after(&self) -> (i8, i8, i8) {
        self.bias_after
    }

    pub const fn global_counter_after(&self) -> Option<i8> {
        self.global_counter_after
    }

    pub const fn backward_counter_after(&self) -> Option<i8> {
        self.backward_counter_after
    }

    pub const fn local_counter_after(&self) -> Option<i8> {
        self.local_counter_after
    }

    pub const fn imli_counter_after(&self) -> Option<i8> {
        self.imli_counter_after
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorHistoryUpdate {
    cpu: CpuId,
    pc: Address,
    old_thread: StatisticalCorrectorThreadSnapshot,
    new_thread: StatisticalCorrectorThreadSnapshot,
    repaired: bool,
    history_update_count: u64,
}

impl StatisticalCorrectorHistoryUpdate {
    pub const fn old_thread(&self) -> &StatisticalCorrectorThreadSnapshot {
        &self.old_thread
    }

    pub const fn new_thread(&self) -> &StatisticalCorrectorThreadSnapshot {
        &self.new_thread
    }

    pub const fn repaired(&self) -> bool {
        self.repaired
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorThreadSnapshot {
    pub(crate) global_history: u64,
    pub(crate) backward_history: u64,
    pub(crate) imli_count: u64,
    pub(crate) path_history: u64,
    pub(crate) first_local_histories: Vec<u64>,
}

impl StatisticalCorrectorThreadSnapshot {
    fn new(config: &StatisticalCorrectorConfig) -> Self {
        Self {
            global_history: 0,
            backward_history: 0,
            imli_count: 0,
            path_history: 0,
            first_local_histories: vec![0; config.num_entries_first_local_histories],
        }
    }

    pub(crate) fn from_parts(
        global_history: u64,
        backward_history: u64,
        imli_count: u64,
        path_history: u64,
        first_local_histories: Vec<u64>,
    ) -> Self {
        Self {
            global_history,
            backward_history,
            imli_count,
            path_history,
            first_local_histories,
        }
    }

    pub const fn global_history(&self) -> u64 {
        self.global_history
    }

    pub const fn backward_history(&self) -> u64 {
        self.backward_history
    }

    pub const fn imli_count(&self) -> u64 {
        self.imli_count
    }

    pub const fn path_history(&self) -> u64 {
        self.path_history
    }

    pub fn first_local_histories(&self) -> &[u64] {
        &self.first_local_histories
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticalCorrectorSnapshot {
    pub(crate) config: StatisticalCorrectorConfig,
    pub(crate) global_gehl: Vec<Vec<i8>>,
    pub(crate) backward_gehl: Vec<Vec<i8>>,
    pub(crate) local_gehl: Vec<Vec<i8>>,
    pub(crate) imli_gehl: Vec<Vec<i8>>,
    pub(crate) global_weights: Vec<i8>,
    pub(crate) backward_weights: Vec<i8>,
    pub(crate) local_weights: Vec<i8>,
    pub(crate) imli_weights: Vec<i8>,
    pub(crate) bias: Vec<i8>,
    pub(crate) bias_sk: Vec<i8>,
    pub(crate) bias_bank: Vec<i8>,
    pub(crate) bias_weights: Vec<i8>,
    pub(crate) update_threshold: i16,
    pub(crate) per_pc_update_threshold: Vec<i16>,
    pub(crate) threads: Vec<StatisticalCorrectorThreadSnapshot>,
    pub(crate) first_chooser: i8,
    pub(crate) second_chooser: i8,
    pub(crate) lookup_count: u64,
    pub(crate) update_count: u64,
    pub(crate) history_update_count: u64,
    pub(crate) correct_count: u64,
    pub(crate) wrong_count: u64,
}

impl StatisticalCorrectorSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        config: StatisticalCorrectorConfig,
        global_gehl: Vec<Vec<i8>>,
        backward_gehl: Vec<Vec<i8>>,
        local_gehl: Vec<Vec<i8>>,
        imli_gehl: Vec<Vec<i8>>,
        global_weights: Vec<i8>,
        backward_weights: Vec<i8>,
        local_weights: Vec<i8>,
        imli_weights: Vec<i8>,
        bias: Vec<i8>,
        bias_sk: Vec<i8>,
        bias_bank: Vec<i8>,
        bias_weights: Vec<i8>,
        update_threshold: i16,
        per_pc_update_threshold: Vec<i16>,
        threads: Vec<StatisticalCorrectorThreadSnapshot>,
        first_chooser: i8,
        second_chooser: i8,
        lookup_count: u64,
        update_count: u64,
        history_update_count: u64,
        correct_count: u64,
        wrong_count: u64,
    ) -> Self {
        Self {
            config,
            global_gehl,
            backward_gehl,
            local_gehl,
            imli_gehl,
            global_weights,
            backward_weights,
            local_weights,
            imli_weights,
            bias,
            bias_sk,
            bias_bank,
            bias_weights,
            update_threshold,
            per_pc_update_threshold,
            threads,
            first_chooser,
            second_chooser,
            lookup_count,
            update_count,
            history_update_count,
            correct_count,
            wrong_count,
        }
    }

    pub const fn config(&self) -> &StatisticalCorrectorConfig {
        &self.config
    }

    pub fn global_gehl(&self) -> &[Vec<i8>] {
        &self.global_gehl
    }

    pub fn bias(&self) -> &[i8] {
        &self.bias
    }

    pub fn threads(&self) -> &[StatisticalCorrectorThreadSnapshot] {
        &self.threads
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }

    pub const fn correct_count(&self) -> u64 {
        self.correct_count
    }

    pub const fn wrong_count(&self) -> u64 {
        self.wrong_count
    }
}

fn check_log_size(field: &'static str, bits: u8) -> Result<(), StatisticalCorrectorError> {
    if (1..=MAX_LOG_ENTRIES).contains(&bits) {
        Ok(())
    } else {
        Err(StatisticalCorrectorError::LogSizeOutOfRange { field, bits })
    }
}

fn check_lengths(field: &'static str, lengths: &[u8]) -> Result<(), StatisticalCorrectorError> {
    if lengths.is_empty() {
        return Err(StatisticalCorrectorError::EmptyLengths { field });
    }
    for bits in lengths {
        if !(1..=MAX_HISTORY_BITS).contains(bits) {
            return Err(StatisticalCorrectorError::HistoryLengthOutOfRange { field, bits: *bits });
        }
    }
    Ok(())
}

fn check_counter_width(field: &'static str, bits: u8) -> Result<(), StatisticalCorrectorError> {
    if (1..=8).contains(&bits) {
        Ok(())
    } else {
        Err(StatisticalCorrectorError::CounterWidthOutOfRange { field, bits })
    }
}

fn check_wide_counter_width(
    field: &'static str,
    bits: u8,
) -> Result<(), StatisticalCorrectorError> {
    if (1..=15).contains(&bits) {
        Ok(())
    } else {
        Err(StatisticalCorrectorError::CounterWidthOutOfRange { field, bits })
    }
}

fn init_gehl_tables(lengths: &[u8], log_entries: u8) -> Vec<Vec<i8>> {
    let entries = 1usize << log_entries;
    lengths
        .iter()
        .map(|_| {
            let mut row = vec![0; entries];
            for (index, entry) in row.iter_mut().enumerate().take(entries - 1) {
                if index & 1 == 0 {
                    *entry = -1;
                }
            }
            row
        })
        .collect()
}

#[derive(Clone, Copy)]
struct GehlUpdate {
    update_weight_index: usize,
    taken: bool,
    counter_width: u8,
    weight_width: u8,
    linear_sum: i16,
}

fn update_gehl_group(
    table: &mut [Vec<i8>],
    weights: &mut [i8],
    indices: &[usize],
    update: GehlUpdate,
) {
    let percsum = table
        .iter()
        .zip(indices)
        .fold(0, |sum, (row, index)| sum + perceptron_term(row[*index]));
    for (row, index) in table.iter_mut().zip(indices) {
        update_signed_i8(&mut row[*index], update.taken, update.counter_width);
    }
    let xsum = update.linear_sum
        - if weights[update.update_weight_index] >= 0 {
            percsum
        } else {
            0
        };
    if sign_non_negative(xsum + percsum) != sign_non_negative(xsum) {
        update_signed_i8(
            &mut weights[update.update_weight_index],
            sign_non_negative(percsum) == update.taken,
            update.weight_width,
        );
    }
}

fn first_counter(table: &[Vec<i8>], indices: &[usize]) -> Option<i8> {
    table
        .first()
        .zip(indices.first())
        .map(|(row, index)| row[*index])
}

fn perceptron_term(counter: i8) -> i16 {
    2 * i16::from(counter) + 1
}

fn update_signed_i8(counter: &mut i8, increment: bool, bits: u8) {
    let min = signed_min_i8(bits);
    let max = signed_max_i8(bits);
    if increment {
        *counter = counter.saturating_add(1).min(max);
    } else {
        *counter = counter.saturating_sub(1).max(min);
    }
}

fn update_signed_i16(counter: &mut i16, increment: bool, bits: u8) {
    let min = -(1i16 << (bits - 1));
    let max = (1i16 << (bits - 1)) - 1;
    if increment {
        *counter = counter.saturating_add(1).min(max);
    } else {
        *counter = counter.saturating_sub(1).max(min);
    }
}

fn check_counter_value(value: i8, bits: u8) -> Result<(), StatisticalCorrectorError> {
    let min = signed_min_i8(bits);
    let max = signed_max_i8(bits);
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(StatisticalCorrectorError::CounterValueOutOfRange { value, min, max })
    }
}

fn signed_min_i8(bits: u8) -> i8 {
    -(1i16 << (bits - 1)) as i8
}

fn signed_max_i8(bits: u8) -> i8 {
    ((1i16 << (bits - 1)) - 1) as i8
}

fn bit_mask_u64(bits: u8) -> u64 {
    if bits >= u64::BITS as u8 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn shift_right_or_zero(value: u64, shift: usize) -> u64 {
    if shift >= u64::BITS as usize {
        0
    } else {
        value >> shift
    }
}

fn sign_non_negative(value: i16) -> bool {
    value >= 0
}
