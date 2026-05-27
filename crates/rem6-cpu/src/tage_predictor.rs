use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::CpuId;

const MAX_LOG_TABLE_SIZE: u8 = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TageBranchPredictorError {
    ZeroThreads,
    ZeroHistoryTables,
    HistoryRangeInvalid {
        min_history: usize,
        max_history: usize,
    },
    TableVectorLengthMismatch {
        expected: usize,
        actual_tag_widths: usize,
        actual_log_sizes: usize,
    },
    BimodalTagWidthNonZero {
        width: u8,
    },
    TableLogSizeOutOfRange {
        bank: usize,
        bits: u8,
    },
    TagWidthOutOfRange {
        bank: usize,
        bits: u8,
    },
    CounterBitsOutOfRange {
        bits: u8,
    },
    UsefulBitsOutOfRange {
        bits: u8,
    },
    PathHistoryBitsOutOfRange {
        bits: u8,
    },
    UseAltBitsOutOfRange {
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
        expected_path_history: u32,
        actual_path_history: u32,
        expected_global_history: u64,
        actual_global_history: u64,
    },
    UnknownBank {
        bank: usize,
    },
    TableIndexOutOfRange {
        bank: usize,
        index: usize,
        entries: usize,
    },
    CounterValueOutOfRange {
        value: i8,
        min: i8,
        max: i8,
    },
    UsefulValueOutOfRange {
        value: u8,
        max: u8,
    },
    SnapshotShapeMismatch {
        expected_history_tables: usize,
        actual_history_tables: usize,
        expected_bimodal_entries: usize,
        actual_bimodal_entries: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<TageBranchPredictorConfig>,
        actual: Box<TageBranchPredictorConfig>,
    },
}

impl fmt::Display for TageBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "tage predictor has no threads"),
            Self::ZeroHistoryTables => write!(formatter, "tage predictor has no history tables"),
            Self::HistoryRangeInvalid {
                min_history,
                max_history,
            } => write!(
                formatter,
                "tage predictor history range min={min_history}, max={max_history} is invalid"
            ),
            Self::TableVectorLengthMismatch {
                expected,
                actual_tag_widths,
                actual_log_sizes,
            } => write!(
                formatter,
                "tage predictor table vectors expected length {expected}, got tag_widths={actual_tag_widths}, log_sizes={actual_log_sizes}"
            ),
            Self::BimodalTagWidthNonZero { width } => {
                write!(formatter, "tage bimodal tag width {width} must be zero")
            }
            Self::TableLogSizeOutOfRange { bank, bits } => write!(
                formatter,
                "tage table {bank} log size {bits} is outside 1..={MAX_LOG_TABLE_SIZE}"
            ),
            Self::TagWidthOutOfRange { bank, bits } => write!(
                formatter,
                "tage table {bank} tag width {bits} is outside 1..=16"
            ),
            Self::CounterBitsOutOfRange { bits } => {
                write!(formatter, "tage counter width {bits} is outside 2..=8")
            }
            Self::UsefulBitsOutOfRange { bits } => {
                write!(formatter, "tage useful width {bits} is outside 1..=2")
            }
            Self::PathHistoryBitsOutOfRange { bits } => {
                write!(formatter, "tage path history width {bits} is outside 1..=31")
            }
            Self::UseAltBitsOutOfRange { bits } => {
                write!(formatter, "tage alt-on-new width {bits} is outside 1..=8")
            }
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "tage instruction shift {bits} is outside 0..=63"
            ),
            Self::UnknownThread { cpu } => {
                write!(formatter, "tage thread {} is not configured", cpu.get())
            }
            Self::HistoryUpdateOutOfOrder {
                cpu,
                expected_path_history,
                actual_path_history,
                expected_global_history,
                actual_global_history,
            } => write!(
                formatter,
                "tage thread {} path history is {expected_path_history}, but update record starts from {actual_path_history}; global history is {expected_global_history}, but update record starts from {actual_global_history}",
                cpu.get()
            ),
            Self::UnknownBank { bank } => write!(formatter, "tage bank {bank} is not configured"),
            Self::TableIndexOutOfRange {
                bank,
                index,
                entries,
            } => write!(
                formatter,
                "tage table {bank} index {index} is outside {entries} entries"
            ),
            Self::CounterValueOutOfRange { value, min, max } => write!(
                formatter,
                "tage counter value {value} is outside {min}..={max}"
            ),
            Self::UsefulValueOutOfRange { value, max } => write!(
                formatter,
                "tage useful value {value} exceeds maximum {max}"
            ),
            Self::SnapshotShapeMismatch {
                expected_history_tables,
                actual_history_tables,
                expected_bimodal_entries,
                actual_bimodal_entries,
            } => write!(
                formatter,
                "tage snapshot shape history_tables={actual_history_tables}, bimodal_entries={actual_bimodal_entries} does not match predictor history_tables={expected_history_tables}, bimodal_entries={expected_bimodal_entries}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "tage snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
        }
    }
}

impl Error for TageBranchPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageBranchPredictorConfig {
    threads: usize,
    history_tables: usize,
    min_history: usize,
    max_history: usize,
    tag_widths: Vec<u8>,
    log_table_sizes: Vec<u8>,
    log_ratio_bimodal_hysteresis: u8,
    counter_bits: u8,
    useful_bits: u8,
    path_history_bits: u8,
    log_useful_reset_period: u8,
    use_alt_on_new_counters: usize,
    use_alt_on_new_bits: u8,
    max_allocations: usize,
    inst_shift: u8,
    taken_only_history: bool,
    speculative_history: bool,
    history_lengths: Vec<usize>,
}

impl TageBranchPredictorConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        threads: usize,
        history_tables: usize,
        min_history: usize,
        max_history: usize,
        tag_widths: Vec<u8>,
        log_table_sizes: Vec<u8>,
        log_ratio_bimodal_hysteresis: u8,
        counter_bits: u8,
        useful_bits: u8,
        path_history_bits: u8,
        log_useful_reset_period: u8,
        use_alt_on_new_counters: usize,
        use_alt_on_new_bits: u8,
        max_allocations: usize,
        inst_shift: u8,
        taken_only_history: bool,
        speculative_history: bool,
    ) -> Result<Self, TageBranchPredictorError> {
        if threads == 0 {
            return Err(TageBranchPredictorError::ZeroThreads);
        }
        if history_tables == 0 {
            return Err(TageBranchPredictorError::ZeroHistoryTables);
        }
        if min_history == 0 || min_history > max_history {
            return Err(TageBranchPredictorError::HistoryRangeInvalid {
                min_history,
                max_history,
            });
        }
        let expected = history_tables + 1;
        if tag_widths.len() != expected || log_table_sizes.len() != expected {
            return Err(TageBranchPredictorError::TableVectorLengthMismatch {
                expected,
                actual_tag_widths: tag_widths.len(),
                actual_log_sizes: log_table_sizes.len(),
            });
        }
        if tag_widths[0] != 0 {
            return Err(TageBranchPredictorError::BimodalTagWidthNonZero {
                width: tag_widths[0],
            });
        }
        for (bank, bits) in log_table_sizes.iter().copied().enumerate() {
            if !(1..=MAX_LOG_TABLE_SIZE).contains(&bits) {
                return Err(TageBranchPredictorError::TableLogSizeOutOfRange { bank, bits });
            }
        }
        for (bank, bits) in tag_widths.iter().copied().enumerate().skip(1) {
            if !(1..=16).contains(&bits) {
                return Err(TageBranchPredictorError::TagWidthOutOfRange { bank, bits });
            }
        }
        if !(2..=8).contains(&counter_bits) {
            return Err(TageBranchPredictorError::CounterBitsOutOfRange { bits: counter_bits });
        }
        if !(1..=2).contains(&useful_bits) {
            return Err(TageBranchPredictorError::UsefulBitsOutOfRange { bits: useful_bits });
        }
        if !(1..=31).contains(&path_history_bits) {
            return Err(TageBranchPredictorError::PathHistoryBitsOutOfRange {
                bits: path_history_bits,
            });
        }
        if !(1..=8).contains(&use_alt_on_new_bits) {
            return Err(TageBranchPredictorError::UseAltBitsOutOfRange {
                bits: use_alt_on_new_bits,
            });
        }
        if inst_shift > 63 {
            return Err(TageBranchPredictorError::InstShiftOutOfRange { bits: inst_shift });
        }

        let mut history_lengths = vec![0; expected];
        if history_tables == 1 {
            history_lengths[1] = max_history;
        } else {
            history_lengths[1] = min_history;
            history_lengths[history_tables] = max_history;
            for (bank, slot) in history_lengths
                .iter_mut()
                .enumerate()
                .take(history_tables + 1)
                .skip(2)
            {
                let exponent = (bank - 1) as f64 / (history_tables - 1) as f64;
                *slot = ((min_history as f64
                    * (max_history as f64 / min_history as f64).powf(exponent))
                    + 0.5) as usize;
            }
        }

        Ok(Self {
            threads,
            history_tables,
            min_history,
            max_history,
            tag_widths,
            log_table_sizes,
            log_ratio_bimodal_hysteresis,
            counter_bits,
            useful_bits,
            path_history_bits,
            log_useful_reset_period,
            use_alt_on_new_counters,
            use_alt_on_new_bits,
            max_allocations,
            inst_shift,
            taken_only_history,
            speculative_history,
            history_lengths,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn history_tables(&self) -> usize {
        self.history_tables
    }

    pub fn history_lengths(&self) -> &[usize] {
        &self.history_lengths
    }

    pub fn log_table_sizes(&self) -> &[u8] {
        &self.log_table_sizes
    }

    pub fn tag_widths(&self) -> &[u8] {
        &self.tag_widths
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    fn table_entries(&self, bank: usize) -> usize {
        1usize << self.log_table_sizes[bank]
    }

    fn table_mask(&self, bank: usize) -> u64 {
        self.table_entries(bank) as u64 - 1
    }

    fn tag_mask(&self, bank: usize) -> u16 {
        bit_mask_u16(self.tag_widths[bank])
    }

    fn path_mask(&self) -> u32 {
        bit_mask_u32(self.path_history_bits)
    }

    fn counter_min(&self) -> i8 {
        -(1i16 << (self.counter_bits - 1)) as i8
    }

    fn counter_max(&self) -> i8 {
        ((1i16 << (self.counter_bits - 1)) - 1) as i8
    }

    fn useful_max(&self) -> u8 {
        bit_mask_u8(self.useful_bits)
    }

    fn use_alt_min(&self) -> i8 {
        -(1i16 << (self.use_alt_on_new_bits - 1)) as i8
    }

    fn use_alt_max(&self) -> i8 {
        ((1i16 << (self.use_alt_on_new_bits - 1)) - 1) as i8
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageBranchPredictor {
    config: TageBranchPredictorConfig,
    bimodal_prediction: Vec<bool>,
    bimodal_hysteresis: Vec<bool>,
    tagged_tables: Vec<Vec<TageTableEntry>>,
    threads: Vec<TageThreadSnapshot>,
    use_alt_on_new_counters: Vec<i8>,
    t_counter: u64,
    lookup_count: u64,
    update_count: u64,
    history_update_count: u64,
}

impl TageBranchPredictor {
    pub fn new(config: TageBranchPredictorConfig) -> Self {
        let mut tagged_tables = Vec::with_capacity(config.history_tables() + 1);
        tagged_tables.push(Vec::new());
        for bank in 1..=config.history_tables() {
            tagged_tables.push(vec![TageTableEntry::new(); config.table_entries(bank)]);
        }
        let bimodal_entries = config.table_entries(0);
        let hysteresis_entries = (bimodal_entries >> config.log_ratio_bimodal_hysteresis).max(1);
        Self {
            bimodal_prediction: vec![false; bimodal_entries],
            bimodal_hysteresis: vec![true; hysteresis_entries],
            tagged_tables,
            threads: vec![TageThreadSnapshot::new(&config); config.threads()],
            use_alt_on_new_counters: vec![0; config.use_alt_on_new_counters],
            config,
            t_counter: 0,
            lookup_count: 0,
            update_count: 0,
            history_update_count: 0,
        }
    }

    pub const fn config(&self) -> &TageBranchPredictorConfig {
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
    ) -> Result<TagePrediction, TageBranchPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        self.lookup_count += 1;

        let thread_before = self.threads[thread_index].clone();
        let mut table_indices = vec![0; self.config.history_tables() + 1];
        let mut table_tags = vec![0; self.config.history_tables() + 1];
        let bimodal_index = self.bimodal_index(pc);
        table_indices[0] = bimodal_index;

        if !conditional {
            return Ok(TagePrediction {
                history: TageHistory::new_unconditional(
                    cpu,
                    pc,
                    bimodal_index,
                    table_indices,
                    table_tags,
                    thread_before,
                ),
                lookup_count: self.lookup_count,
            });
        }

        for bank in 1..=self.config.history_tables() {
            table_indices[bank] = self.tagged_index(thread_index, pc, bank);
            table_tags[bank] = self.tagged_tag(thread_index, pc, bank);
        }

        let hit_bank =
            self.find_hit_bank(&table_indices, &table_tags, self.config.history_tables());
        let alternate_bank = hit_bank
            .and_then(|bank| bank.checked_sub(1))
            .and_then(|upper| self.find_hit_bank(&table_indices, &table_tags, upper));
        let bimodal_prediction = self.bimodal_prediction[bimodal_index];
        let alternate_prediction = if let Some(bank) = alternate_bank {
            self.tagged_tables[bank][table_indices[bank]].counter >= 0
        } else {
            bimodal_prediction
        };
        let (longest_prediction, pseudo_new_allocation, provider, predicted_taken) =
            if let Some(bank) = hit_bank {
                let entry = &self.tagged_tables[bank][table_indices[bank]];
                let longest_prediction = entry.counter >= 0;
                let pseudo_new_allocation = (2 * i16::from(entry.counter) + 1).abs() <= 1;
                if self.use_alt_on_new_counters[0] < 0 || !pseudo_new_allocation {
                    (
                        longest_prediction,
                        pseudo_new_allocation,
                        TageProvider::TageLongestMatch,
                        longest_prediction,
                    )
                } else {
                    let provider = if alternate_bank.is_some() {
                        TageProvider::TageAlternateMatch
                    } else {
                        TageProvider::BimodalAlternateMatch
                    };
                    (
                        longest_prediction,
                        pseudo_new_allocation,
                        provider,
                        alternate_prediction,
                    )
                }
            } else {
                (
                    bimodal_prediction,
                    false,
                    TageProvider::BimodalOnly,
                    bimodal_prediction,
                )
            };

        Ok(TagePrediction {
            history: TageHistory {
                cpu,
                pc,
                conditional,
                bimodal_index,
                table_indices,
                table_tags,
                hit_bank,
                alternate_bank,
                provider,
                predicted_taken,
                alternate_prediction,
                longest_prediction,
                pseudo_new_allocation,
                thread_before,
                history_bits: 0,
                history_bit_count: 0,
                modified_history: false,
            },
            lookup_count: self.lookup_count,
        })
    }

    pub fn update_history(
        &mut self,
        history: &TageHistory,
        taken: bool,
        target: Address,
    ) -> Result<TageHistoryUpdate, TageBranchPredictorError> {
        self.apply_history_update(history, taken, target, false)
    }

    pub fn repair_history(
        &mut self,
        history: &TageHistory,
        taken: bool,
        target: Address,
    ) -> Result<TageHistoryUpdate, TageBranchPredictorError> {
        self.apply_history_update(history, taken, target, true)
    }

    pub fn train(
        &mut self,
        history: &TageHistory,
        actual_taken: bool,
    ) -> Result<TageTrainingUpdate, TageBranchPredictorError> {
        self.thread_index(history.cpu())?;
        let use_alt_counter_before = self.use_alt_on_new_counters[0];
        let mut allocated_entries = Vec::new();
        let mut updated_bank = None;
        let mut updated_alt_bank = None;

        if !history.conditional() {
            self.update_count += 1;
            return Ok(self.training_update(
                history,
                actual_taken,
                use_alt_counter_before,
                allocated_entries,
                updated_bank,
                updated_alt_bank,
            ));
        }

        if let Some(bank) = history.hit_bank() {
            if history.pseudo_new_allocation() {
                if history.longest_match_predicted_taken() == actual_taken {
                    allocated_entries.clear();
                }
                if history.longest_match_predicted_taken() != history.alternate_predicted_taken() {
                    update_signed_counter(
                        &mut self.use_alt_on_new_counters[0],
                        history.alternate_predicted_taken() == actual_taken,
                        self.config.use_alt_min(),
                        self.config.use_alt_max(),
                    );
                }
            }
            updated_bank = Some(bank);
        }

        let allocate = history.predicted_taken() != actual_taken
            && history.hit_bank().unwrap_or(0) < self.config.history_tables();
        if allocate {
            allocated_entries = self.allocate_entries(history, actual_taken);
        }

        self.t_counter += 1;
        self.reset_useful_if_needed();

        if let Some(bank) = history.hit_bank() {
            let index = history.tagged_indices()[bank];
            update_signed_counter(
                &mut self.tagged_tables[bank][index].counter,
                actual_taken,
                self.config.counter_min(),
                self.config.counter_max(),
            );
            if self.tagged_tables[bank][index].useful == 0 {
                if let Some(alt_bank) = history.alternate_bank() {
                    let alt_index = history.tagged_indices()[alt_bank];
                    update_signed_counter(
                        &mut self.tagged_tables[alt_bank][alt_index].counter,
                        actual_taken,
                        self.config.counter_min(),
                        self.config.counter_max(),
                    );
                    updated_alt_bank = Some(alt_bank);
                } else {
                    self.update_bimodal(history.bimodal_index(), actual_taken);
                }
            }
            if history.predicted_taken() != history.alternate_predicted_taken() {
                update_unsigned_counter(
                    &mut self.tagged_tables[bank][index].useful,
                    history.predicted_taken() == actual_taken,
                    self.config.useful_max(),
                );
            }
        } else {
            self.update_bimodal(history.bimodal_index(), actual_taken);
        }

        self.update_count += 1;
        Ok(self.training_update(
            history,
            actual_taken,
            use_alt_counter_before,
            allocated_entries,
            updated_bank,
            updated_alt_bank,
        ))
    }

    pub fn write_tagged_entry(
        &mut self,
        bank: usize,
        index: usize,
        tag: u16,
        counter: i8,
        useful: u8,
    ) -> Result<(), TageBranchPredictorError> {
        self.check_tagged_index(bank, index)?;
        if counter < self.config.counter_min() || counter > self.config.counter_max() {
            return Err(TageBranchPredictorError::CounterValueOutOfRange {
                value: counter,
                min: self.config.counter_min(),
                max: self.config.counter_max(),
            });
        }
        if useful > self.config.useful_max() {
            return Err(TageBranchPredictorError::UsefulValueOutOfRange {
                value: useful,
                max: self.config.useful_max(),
            });
        }
        self.tagged_tables[bank][index] = TageTableEntry {
            counter,
            tag: tag & self.config.tag_mask(bank),
            useful,
        };
        Ok(())
    }

    pub fn write_bimodal_entry(
        &mut self,
        index: usize,
        prediction: bool,
        hysteresis: bool,
    ) -> Result<(), TageBranchPredictorError> {
        if index >= self.bimodal_prediction.len() {
            return Err(TageBranchPredictorError::TableIndexOutOfRange {
                bank: 0,
                index,
                entries: self.bimodal_prediction.len(),
            });
        }
        self.bimodal_prediction[index] = prediction;
        self.bimodal_hysteresis[index >> self.config.log_ratio_bimodal_hysteresis] = hysteresis;
        Ok(())
    }

    pub fn snapshot(&self) -> TageBranchPredictorSnapshot {
        TageBranchPredictorSnapshot {
            config: self.config.clone(),
            bimodal_prediction: self.bimodal_prediction.clone(),
            bimodal_hysteresis: self.bimodal_hysteresis.clone(),
            tagged_tables: self.tagged_tables.clone(),
            threads: self.threads.clone(),
            use_alt_on_new_counters: self.use_alt_on_new_counters.clone(),
            t_counter: self.t_counter,
            lookup_count: self.lookup_count,
            update_count: self.update_count,
            history_update_count: self.history_update_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &TageBranchPredictorSnapshot,
    ) -> Result<(), TageBranchPredictorError> {
        if self.config.history_tables() != snapshot.config.history_tables()
            || self.bimodal_prediction.len() != snapshot.bimodal_prediction.len()
        {
            return Err(TageBranchPredictorError::SnapshotShapeMismatch {
                expected_history_tables: self.config.history_tables(),
                actual_history_tables: snapshot.config.history_tables(),
                expected_bimodal_entries: self.bimodal_prediction.len(),
                actual_bimodal_entries: snapshot.bimodal_prediction.len(),
            });
        }
        if self.config != snapshot.config {
            return Err(TageBranchPredictorError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        self.bimodal_prediction
            .clone_from(&snapshot.bimodal_prediction);
        self.bimodal_hysteresis
            .clone_from(&snapshot.bimodal_hysteresis);
        self.tagged_tables.clone_from(&snapshot.tagged_tables);
        self.threads.clone_from(&snapshot.threads);
        self.use_alt_on_new_counters
            .clone_from(&snapshot.use_alt_on_new_counters);
        self.t_counter = snapshot.t_counter;
        self.lookup_count = snapshot.lookup_count;
        self.update_count = snapshot.update_count;
        self.history_update_count = snapshot.history_update_count;
        Ok(())
    }

    fn training_update(
        &self,
        history: &TageHistory,
        actual_taken: bool,
        use_alt_counter_before: i8,
        allocated_entries: Vec<(usize, usize)>,
        updated_bank: Option<usize>,
        updated_alt_bank: Option<usize>,
    ) -> TageTrainingUpdate {
        TageTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            provider: history.provider(),
            predicted_taken: history.predicted_taken(),
            use_alt_counter_before,
            use_alt_counter_after: self.use_alt_on_new_counters[0],
            allocated_entries,
            updated_bank,
            updated_alt_bank,
            t_counter_after: self.t_counter,
            update_count: self.update_count,
        }
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, TageBranchPredictorError> {
        let index = cpu.get() as usize;
        if index < self.threads.len() {
            Ok(index)
        } else {
            Err(TageBranchPredictorError::UnknownThread { cpu })
        }
    }

    fn check_tagged_index(
        &self,
        bank: usize,
        index: usize,
    ) -> Result<(), TageBranchPredictorError> {
        if bank == 0 || bank > self.config.history_tables() {
            return Err(TageBranchPredictorError::UnknownBank { bank });
        }
        if index >= self.tagged_tables[bank].len() {
            return Err(TageBranchPredictorError::TableIndexOutOfRange {
                bank,
                index,
                entries: self.tagged_tables[bank].len(),
            });
        }
        Ok(())
    }

    fn bimodal_index(&self, pc: Address) -> usize {
        ((pc.get() >> self.config.inst_shift) & self.config.table_mask(0)) as usize
    }

    fn tagged_index(&self, thread: usize, pc: Address, bank: usize) -> usize {
        let hlen =
            self.config.history_lengths[bank].min(self.config.path_history_bits as usize) as u8;
        let shifted_pc = pc.get() >> self.config.inst_shift;
        let log_size = self.config.log_table_sizes[bank];
        let folded = self.threads[thread].compute_indices[bank].compressed();
        let path = self.path_hash(self.threads[thread].path_history, hlen, bank);
        let shift = (i16::from(log_size) - bank as i16).unsigned_abs() as u8 + 1;
        (shifted_pc ^ (shifted_pc >> shift) ^ u64::from(folded) ^ u64::from(path)) as usize
            & (self.config.table_entries(bank) - 1)
    }

    fn path_hash(&self, path_history: u32, size: u8, bank: usize) -> u32 {
        let log_size = self.config.log_table_sizes[bank];
        let mask = bit_mask_u32(log_size);
        let mut value = path_history & bit_mask_u32(size);
        let a1 = value & mask;
        let mut a2 = value >> log_size;
        a2 = ((a2 << bank) & mask) + (a2 >> (log_size.saturating_sub(bank as u8)));
        value = a1 ^ a2;
        ((value << bank) & mask) + (value >> (log_size.saturating_sub(bank as u8)))
    }

    fn tagged_tag(&self, thread: usize, pc: Address, bank: usize) -> u16 {
        let shifted_pc = pc.get() >> self.config.inst_shift;
        let tag = shifted_pc
            ^ u64::from(self.threads[thread].compute_tags[0][bank].compressed())
            ^ (u64::from(self.threads[thread].compute_tags[1][bank].compressed()) << 1);
        tag as u16 & self.config.tag_mask(bank)
    }

    fn find_hit_bank(
        &self,
        table_indices: &[usize],
        table_tags: &[u16],
        max_bank: usize,
    ) -> Option<usize> {
        (1..=max_bank)
            .rev()
            .find(|&bank| self.tagged_tables[bank][table_indices[bank]].tag == table_tags[bank])
    }

    fn apply_history_update(
        &mut self,
        history: &TageHistory,
        taken: bool,
        target: Address,
        repair: bool,
    ) -> Result<TageHistoryUpdate, TageBranchPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let old_path_history = self.threads[thread_index].path_history();
        let old_global_history = self.threads[thread_index].global_history_value();
        if repair {
            self.threads[thread_index].clone_from(history.thread_before());
        } else if &self.threads[thread_index] != history.thread_before() {
            return Err(TageBranchPredictorError::HistoryUpdateOutOfOrder {
                cpu: history.cpu(),
                expected_path_history: old_path_history,
                actual_path_history: history.thread_before().path_history(),
                expected_global_history: old_global_history,
                actual_global_history: history.thread_before().global_history_value(),
            });
        }
        let (history_bits, history_bit_count) =
            self.update_path_and_global_history(thread_index, history.pc(), taken, target);
        self.history_update_count += 1;
        Ok(TageHistoryUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            old_path_history,
            new_path_history: self.threads[thread_index].path_history(),
            old_global_history,
            new_global_history: self.threads[thread_index].global_history_value(),
            history_bits,
            history_bit_count,
            history_update_count: self.history_update_count,
        })
    }

    fn update_path_and_global_history(
        &mut self,
        thread_index: usize,
        pc: Address,
        taken: bool,
        target: Address,
    ) -> (u64, u8) {
        if !self.config.taken_only_history || taken {
            let path_bit = ((pc.get() >> self.config.inst_shift) & 1) as u32;
            self.threads[thread_index].path_history =
                ((self.threads[thread_index].path_history << 1) | path_bit)
                    & self.config.path_mask();
        }

        let (history_bits, history_bit_count) = if self.config.taken_only_history {
            if taken {
                (
                    (((pc.get() >> self.config.inst_shift) >> 2)
                        ^ ((target.get() >> self.config.inst_shift) >> 3)),
                    2,
                )
            } else {
                (0, 0)
            }
        } else {
            (u64::from(taken), 1)
        };

        for bit_index in 0..history_bit_count {
            let bit = ((history_bits >> bit_index) & 1) as u8;
            self.update_global_history_bit(thread_index, bit);
        }

        (history_bits, history_bit_count)
    }

    fn update_global_history_bit(&mut self, thread_index: usize, bit: u8) {
        let old_history = self.threads[thread_index].global_history.clone();
        self.threads[thread_index].global_history.insert(0, bit & 1);
        self.threads[thread_index]
            .global_history
            .truncate(self.config.max_history + 1);
        for bank in 1..=self.config.history_tables() {
            let falling_index = self.config.history_lengths[bank].saturating_sub(1);
            let falling_bit = old_history.get(falling_index).copied().unwrap_or(0);
            self.threads[thread_index].compute_indices[bank].update(bit, falling_bit);
            self.threads[thread_index].compute_tags[0][bank].update(bit, falling_bit);
            self.threads[thread_index].compute_tags[1][bank].update(bit, falling_bit);
        }
    }

    fn allocate_entries(&mut self, history: &TageHistory, taken: bool) -> Vec<(usize, usize)> {
        let start = history.hit_bank().unwrap_or(0) + 1;
        let mut allocated = Vec::new();
        for bank in start..=self.config.history_tables() {
            let index = history.tagged_indices()[bank];
            if self.tagged_tables[bank][index].useful == 0 {
                self.tagged_tables[bank][index] = TageTableEntry {
                    counter: if taken { 0 } else { -1 },
                    tag: history.tagged_tags()[bank],
                    useful: 0,
                };
                allocated.push((bank, index));
                if allocated.len() == self.config.max_allocations {
                    break;
                }
            }
        }
        if allocated.is_empty() && start <= self.config.history_tables() {
            let bank = start;
            let index = history.tagged_indices()[bank];
            self.tagged_tables[bank][index].useful = 0;
        }
        allocated
    }

    fn reset_useful_if_needed(&mut self) {
        if self.config.log_useful_reset_period == 0 {
            return;
        }
        let period_mask = (1u64 << self.config.log_useful_reset_period) - 1;
        if (self.t_counter & period_mask) != 0 {
            return;
        }
        for bank in 1..=self.config.history_tables() {
            for entry in &mut self.tagged_tables[bank] {
                entry.useful >>= 1;
            }
        }
    }

    fn update_bimodal(&mut self, index: usize, taken: bool) {
        let hysteresis_index = index >> self.config.log_ratio_bimodal_hysteresis;
        let mut inter = ((u8::from(self.bimodal_prediction[index])) << 1)
            + u8::from(self.bimodal_hysteresis[hysteresis_index]);
        if taken {
            inter = (inter + 1).min(3);
        } else {
            inter = inter.saturating_sub(1);
        }
        self.bimodal_prediction[index] = (inter >> 1) != 0;
        self.bimodal_hysteresis[hysteresis_index] = (inter & 1) != 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TageProvider {
    BimodalOnly,
    TageLongestMatch,
    BimodalAlternateMatch,
    TageAlternateMatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagePrediction {
    history: TageHistory,
    lookup_count: u64,
}

impl TagePrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn bimodal_index(&self) -> usize {
        self.history.bimodal_index()
    }

    pub fn tagged_indices(&self) -> &[usize] {
        self.history.tagged_indices()
    }

    pub fn tagged_tags(&self) -> &[u16] {
        self.history.tagged_tags()
    }

    pub const fn hit_bank(&self) -> Option<usize> {
        self.history.hit_bank()
    }

    pub const fn alternate_bank(&self) -> Option<usize> {
        self.history.alternate_bank()
    }

    pub const fn provider(&self) -> TageProvider {
        self.history.provider()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn alternate_predicted_taken(&self) -> bool {
        self.history.alternate_predicted_taken()
    }

    pub const fn longest_match_predicted_taken(&self) -> bool {
        self.history.longest_match_predicted_taken()
    }

    pub const fn pseudo_new_allocation(&self) -> bool {
        self.history.pseudo_new_allocation()
    }

    pub const fn history(&self) -> &TageHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageHistory {
    cpu: CpuId,
    pc: Address,
    conditional: bool,
    bimodal_index: usize,
    table_indices: Vec<usize>,
    table_tags: Vec<u16>,
    hit_bank: Option<usize>,
    alternate_bank: Option<usize>,
    provider: TageProvider,
    predicted_taken: bool,
    alternate_prediction: bool,
    longest_prediction: bool,
    pseudo_new_allocation: bool,
    thread_before: TageThreadSnapshot,
    history_bits: u64,
    history_bit_count: u8,
    modified_history: bool,
}

impl TageHistory {
    fn new_unconditional(
        cpu: CpuId,
        pc: Address,
        bimodal_index: usize,
        table_indices: Vec<usize>,
        table_tags: Vec<u16>,
        thread_before: TageThreadSnapshot,
    ) -> Self {
        Self {
            cpu,
            pc,
            conditional: false,
            bimodal_index,
            table_indices,
            table_tags,
            hit_bank: None,
            alternate_bank: None,
            provider: TageProvider::BimodalOnly,
            predicted_taken: true,
            alternate_prediction: true,
            longest_prediction: true,
            pseudo_new_allocation: false,
            thread_before,
            history_bits: 0,
            history_bit_count: 0,
            modified_history: false,
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

    pub const fn bimodal_index(&self) -> usize {
        self.bimodal_index
    }

    pub fn tagged_indices(&self) -> &[usize] {
        &self.table_indices
    }

    pub fn tagged_tags(&self) -> &[u16] {
        &self.table_tags
    }

    pub const fn hit_bank(&self) -> Option<usize> {
        self.hit_bank
    }

    pub const fn alternate_bank(&self) -> Option<usize> {
        self.alternate_bank
    }

    pub const fn provider(&self) -> TageProvider {
        self.provider
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn alternate_predicted_taken(&self) -> bool {
        self.alternate_prediction
    }

    pub const fn longest_match_predicted_taken(&self) -> bool {
        self.longest_prediction
    }

    pub const fn pseudo_new_allocation(&self) -> bool {
        self.pseudo_new_allocation
    }

    pub const fn thread_before(&self) -> &TageThreadSnapshot {
        &self.thread_before
    }

    pub const fn history_bits(&self) -> u64 {
        self.history_bits
    }

    pub const fn history_bit_count(&self) -> u8 {
        self.history_bit_count
    }

    pub const fn modified_history(&self) -> bool {
        self.modified_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageHistoryUpdate {
    cpu: CpuId,
    pc: Address,
    old_path_history: u32,
    new_path_history: u32,
    old_global_history: u64,
    new_global_history: u64,
    history_bits: u64,
    history_bit_count: u8,
    history_update_count: u64,
}

impl TageHistoryUpdate {
    pub const fn old_path_history(&self) -> u32 {
        self.old_path_history
    }

    pub const fn new_path_history(&self) -> u32 {
        self.new_path_history
    }

    pub const fn old_global_history(&self) -> u64 {
        self.old_global_history
    }

    pub const fn new_global_history(&self) -> u64 {
        self.new_global_history
    }

    pub const fn history_bits(&self) -> u64 {
        self.history_bits
    }

    pub const fn history_bit_count(&self) -> u8 {
        self.history_bit_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    provider: TageProvider,
    predicted_taken: bool,
    use_alt_counter_before: i8,
    use_alt_counter_after: i8,
    allocated_entries: Vec<(usize, usize)>,
    updated_bank: Option<usize>,
    updated_alt_bank: Option<usize>,
    t_counter_after: u64,
    update_count: u64,
}

impl TageTrainingUpdate {
    pub fn allocated_entries(&self) -> &[(usize, usize)] {
        &self.allocated_entries
    }

    pub const fn updated_bank(&self) -> Option<usize> {
        self.updated_bank
    }

    pub const fn updated_alt_bank(&self) -> Option<usize> {
        self.updated_alt_bank
    }

    pub const fn use_alt_counter_before(&self) -> i8 {
        self.use_alt_counter_before
    }

    pub const fn use_alt_counter_after(&self) -> i8 {
        self.use_alt_counter_after
    }

    pub const fn t_counter_after(&self) -> u64 {
        self.t_counter_after
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageTableEntry {
    counter: i8,
    tag: u16,
    useful: u8,
}

impl TageTableEntry {
    const fn new() -> Self {
        Self {
            counter: 0,
            tag: 0,
            useful: 0,
        }
    }

    pub const fn counter(&self) -> i8 {
        self.counter
    }

    pub const fn tag(&self) -> u16 {
        self.tag
    }

    pub const fn useful(&self) -> u8 {
        self.useful
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldedHistorySnapshot {
    compressed: u32,
    compressed_bits: u8,
    original_bits: usize,
    outpoint: usize,
}

impl FoldedHistorySnapshot {
    fn new(original_bits: usize, compressed_bits: u8) -> Self {
        Self {
            compressed: 0,
            compressed_bits,
            original_bits,
            outpoint: original_bits % compressed_bits as usize,
        }
    }

    pub const fn compressed(&self) -> u32 {
        self.compressed
    }

    fn update(&mut self, newest_bit: u8, falling_bit: u8) {
        self.compressed = (self.compressed << 1) | u32::from(newest_bit & 1);
        self.compressed ^= u32::from(falling_bit & 1) << self.outpoint;
        self.compressed ^= self.compressed >> self.compressed_bits;
        self.compressed &= bit_mask_u32(self.compressed_bits);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageThreadSnapshot {
    path_history: u32,
    non_spec_path_history: u32,
    global_history: Vec<u8>,
    compute_indices: Vec<FoldedHistorySnapshot>,
    compute_tags: [Vec<FoldedHistorySnapshot>; 2],
}

impl TageThreadSnapshot {
    fn new(config: &TageBranchPredictorConfig) -> Self {
        let mut compute_indices = Vec::with_capacity(config.history_tables() + 1);
        let mut compute_tags0 = Vec::with_capacity(config.history_tables() + 1);
        let mut compute_tags1 = Vec::with_capacity(config.history_tables() + 1);
        compute_indices.push(FoldedHistorySnapshot::new(1, 1));
        compute_tags0.push(FoldedHistorySnapshot::new(1, 1));
        compute_tags1.push(FoldedHistorySnapshot::new(1, 1));
        for bank in 1..=config.history_tables() {
            let hist = config.history_lengths[bank];
            compute_indices.push(FoldedHistorySnapshot::new(
                hist,
                config.log_table_sizes[bank],
            ));
            compute_tags0.push(FoldedHistorySnapshot::new(hist, config.tag_widths[bank]));
            compute_tags1.push(FoldedHistorySnapshot::new(
                hist,
                config.tag_widths[bank] - 1,
            ));
        }
        Self {
            path_history: 0,
            non_spec_path_history: 0,
            global_history: vec![0; config.max_history + 1],
            compute_indices,
            compute_tags: [compute_tags0, compute_tags1],
        }
    }

    pub const fn path_history(&self) -> u32 {
        self.path_history
    }

    pub const fn non_spec_path_history(&self) -> u32 {
        self.non_spec_path_history
    }

    pub fn global_history(&self) -> &[u8] {
        &self.global_history
    }

    pub fn compute_indices(&self) -> &[FoldedHistorySnapshot] {
        &self.compute_indices
    }

    pub fn compute_tags(&self) -> &[Vec<FoldedHistorySnapshot>; 2] {
        &self.compute_tags
    }

    pub fn global_history_value(&self) -> u64 {
        self.global_history
            .iter()
            .take(64)
            .enumerate()
            .fold(0, |value, (index, bit)| {
                value | (u64::from(*bit & 1) << index)
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageBranchPredictorSnapshot {
    config: TageBranchPredictorConfig,
    bimodal_prediction: Vec<bool>,
    bimodal_hysteresis: Vec<bool>,
    tagged_tables: Vec<Vec<TageTableEntry>>,
    threads: Vec<TageThreadSnapshot>,
    use_alt_on_new_counters: Vec<i8>,
    t_counter: u64,
    lookup_count: u64,
    update_count: u64,
    history_update_count: u64,
}

impl TageBranchPredictorSnapshot {
    pub const fn config(&self) -> &TageBranchPredictorConfig {
        &self.config
    }

    pub fn bimodal_prediction(&self) -> &[bool] {
        &self.bimodal_prediction
    }

    pub fn bimodal_hysteresis(&self) -> &[bool] {
        &self.bimodal_hysteresis
    }

    pub fn tagged_tables(&self) -> &[Vec<TageTableEntry>] {
        &self.tagged_tables
    }

    pub fn threads(&self) -> &[TageThreadSnapshot] {
        &self.threads
    }

    pub fn use_alt_on_new_counters(&self) -> &[i8] {
        &self.use_alt_on_new_counters
    }

    pub const fn t_counter(&self) -> u64 {
        self.t_counter
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

fn bit_mask_u8(bits: u8) -> u8 {
    if bits >= u8::BITS as u8 {
        u8::MAX
    } else {
        ((1u16 << bits) - 1) as u8
    }
}

fn bit_mask_u16(bits: u8) -> u16 {
    if bits >= u16::BITS as u8 {
        u16::MAX
    } else {
        ((1u32 << bits) - 1) as u16
    }
}

fn bit_mask_u32(bits: u8) -> u32 {
    if bits >= u32::BITS as u8 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}

fn update_signed_counter(counter: &mut i8, increment: bool, min: i8, max: i8) {
    if increment {
        *counter = counter.saturating_add(1).min(max);
    } else {
        *counter = counter.saturating_sub(1).max(min);
    }
}

fn update_unsigned_counter(counter: &mut u8, increment: bool, max: u8) {
    if increment {
        *counter = counter.saturating_add(1).min(max);
    } else {
        *counter = counter.saturating_sub(1);
    }
}
