use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::CpuId;

const DEFAULT_LOG_SIZE: u8 = 8;
const DEFAULT_LOG_ASSOC: u8 = 2;
const DEFAULT_AGE_BITS: u8 = 8;
const DEFAULT_CONFIDENCE_BITS: u8 = 2;
const DEFAULT_TAG_BITS: u8 = 14;
const DEFAULT_ITER_BITS: u8 = 14;
const DEFAULT_WITH_LOOP_BITS: u8 = 7;
const DEFAULT_INST_SHIFT: u8 = 2;
const DEFAULT_INITIAL_ITER: u16 = 1;
const DEFAULT_INITIAL_AGE: u8 = u8::MAX;
const MAX_LOG_SIZE: u8 = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopBranchPredictorError {
    ZeroThreads,
    LogSizeOutOfRange {
        bits: u8,
    },
    LogAssociativityExceedsSize {
        log_size: u8,
        log_assoc: u8,
    },
    AgeBitsOutOfRange {
        bits: u8,
    },
    ConfidenceBitsOutOfRange {
        bits: u8,
    },
    TagBitsOutOfRange {
        bits: u8,
    },
    IterBitsOutOfRange {
        bits: u8,
    },
    WithLoopBitsOutOfRange {
        bits: u8,
    },
    InstShiftOutOfRange {
        bits: u8,
    },
    InitialIterOutOfRange {
        value: u16,
        max: u16,
    },
    InitialAgeOutOfRange {
        value: u8,
        max: u8,
    },
    UnknownThread {
        cpu: CpuId,
    },
    SnapshotShapeMismatch {
        expected_entries: usize,
        actual_entries: usize,
        expected_sets: usize,
        actual_sets: usize,
    },
    SnapshotConfigMismatch {
        expected: LoopBranchPredictorConfig,
        actual: LoopBranchPredictorConfig,
    },
}

impl fmt::Display for LoopBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "loop predictor has no threads"),
            Self::LogSizeOutOfRange { bits } => write!(
                formatter,
                "loop predictor log table size {bits} is outside 1..={MAX_LOG_SIZE}"
            ),
            Self::LogAssociativityExceedsSize {
                log_size,
                log_assoc,
            } => write!(
                formatter,
                "loop predictor log associativity {log_assoc} exceeds log table size {log_size}"
            ),
            Self::AgeBitsOutOfRange { bits } => {
                write!(formatter, "loop predictor age width {bits} is outside 1..=8")
            }
            Self::ConfidenceBitsOutOfRange { bits } => write!(
                formatter,
                "loop predictor confidence width {bits} is outside 1..=8"
            ),
            Self::TagBitsOutOfRange { bits } => write!(
                formatter,
                "loop predictor tag width {bits} is outside 1..=16"
            ),
            Self::IterBitsOutOfRange { bits } => write!(
                formatter,
                "loop predictor iteration width {bits} is outside 1..=16"
            ),
            Self::WithLoopBitsOutOfRange { bits } => write!(
                formatter,
                "loop predictor use counter width {bits} is outside 1..=8"
            ),
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "loop predictor instruction shift {bits} is outside 0..=63"
            ),
            Self::InitialIterOutOfRange { value, max } => write!(
                formatter,
                "loop predictor initial iteration {value} exceeds maximum {max}"
            ),
            Self::InitialAgeOutOfRange { value, max } => write!(
                formatter,
                "loop predictor initial age {value} exceeds maximum {max}"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "loop predictor thread {} is not configured",
                cpu.get()
            ),
            Self::SnapshotShapeMismatch {
                expected_entries,
                actual_entries,
                expected_sets,
                actual_sets,
            } => write!(
                formatter,
                "loop predictor snapshot shape entries={actual_entries}, sets={actual_sets} does not match predictor entries={expected_entries}, sets={expected_sets}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "loop predictor snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
        }
    }
}

impl Error for LoopBranchPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopBranchPredictorConfig {
    threads: usize,
    log_size: u8,
    log_assoc: u8,
    age_bits: u8,
    confidence_bits: u8,
    tag_bits: u8,
    iter_bits: u8,
    with_loop_bits: u8,
    inst_shift: u8,
    use_direction_bit: bool,
    use_speculation: bool,
    use_hashing: bool,
    restrict_allocation: bool,
    initial_iter: u16,
    initial_age: u8,
    optional_age_reset: bool,
}

impl LoopBranchPredictorConfig {
    pub fn new(threads: usize) -> Result<Self, LoopBranchPredictorError> {
        Self::with_options(
            threads,
            DEFAULT_LOG_SIZE,
            DEFAULT_LOG_ASSOC,
            DEFAULT_AGE_BITS,
            DEFAULT_CONFIDENCE_BITS,
            DEFAULT_TAG_BITS,
            DEFAULT_ITER_BITS,
            DEFAULT_WITH_LOOP_BITS,
            DEFAULT_INST_SHIFT,
            false,
            false,
            false,
            false,
            DEFAULT_INITIAL_ITER,
            DEFAULT_INITIAL_AGE,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        threads: usize,
        log_size: u8,
        log_assoc: u8,
        age_bits: u8,
        confidence_bits: u8,
        tag_bits: u8,
        iter_bits: u8,
        with_loop_bits: u8,
        inst_shift: u8,
        use_direction_bit: bool,
        use_speculation: bool,
        use_hashing: bool,
        restrict_allocation: bool,
        initial_iter: u16,
        initial_age: u8,
        optional_age_reset: bool,
    ) -> Result<Self, LoopBranchPredictorError> {
        if threads == 0 {
            return Err(LoopBranchPredictorError::ZeroThreads);
        }
        if !(1..=MAX_LOG_SIZE).contains(&log_size) {
            return Err(LoopBranchPredictorError::LogSizeOutOfRange { bits: log_size });
        }
        if log_assoc > log_size {
            return Err(LoopBranchPredictorError::LogAssociativityExceedsSize {
                log_size,
                log_assoc,
            });
        }
        if !(1..=8).contains(&age_bits) {
            return Err(LoopBranchPredictorError::AgeBitsOutOfRange { bits: age_bits });
        }
        if !(1..=8).contains(&confidence_bits) {
            return Err(LoopBranchPredictorError::ConfidenceBitsOutOfRange {
                bits: confidence_bits,
            });
        }
        if !(1..=16).contains(&tag_bits) {
            return Err(LoopBranchPredictorError::TagBitsOutOfRange { bits: tag_bits });
        }
        if !(1..=16).contains(&iter_bits) {
            return Err(LoopBranchPredictorError::IterBitsOutOfRange { bits: iter_bits });
        }
        if !(1..=8).contains(&with_loop_bits) {
            return Err(LoopBranchPredictorError::WithLoopBitsOutOfRange {
                bits: with_loop_bits,
            });
        }
        if inst_shift > 63 {
            return Err(LoopBranchPredictorError::InstShiftOutOfRange { bits: inst_shift });
        }

        let iter_mask = bit_mask_u16(iter_bits);
        if initial_iter > iter_mask {
            return Err(LoopBranchPredictorError::InitialIterOutOfRange {
                value: initial_iter,
                max: iter_mask,
            });
        }

        let age_mask = bit_mask_u8(age_bits);
        if initial_age > age_mask {
            return Err(LoopBranchPredictorError::InitialAgeOutOfRange {
                value: initial_age,
                max: age_mask,
            });
        }

        Ok(Self {
            threads,
            log_size,
            log_assoc,
            age_bits,
            confidence_bits,
            tag_bits,
            iter_bits,
            with_loop_bits,
            inst_shift,
            use_direction_bit,
            use_speculation,
            use_hashing,
            restrict_allocation,
            initial_iter,
            initial_age,
            optional_age_reset,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn log_size(&self) -> u8 {
        self.log_size
    }

    pub const fn log_assoc(&self) -> u8 {
        self.log_assoc
    }

    pub const fn age_bits(&self) -> u8 {
        self.age_bits
    }

    pub const fn confidence_bits(&self) -> u8 {
        self.confidence_bits
    }

    pub const fn tag_bits(&self) -> u8 {
        self.tag_bits
    }

    pub const fn iter_bits(&self) -> u8 {
        self.iter_bits
    }

    pub const fn with_loop_bits(&self) -> u8 {
        self.with_loop_bits
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    pub const fn use_direction_bit(&self) -> bool {
        self.use_direction_bit
    }

    pub const fn use_speculation(&self) -> bool {
        self.use_speculation
    }

    pub const fn use_hashing(&self) -> bool {
        self.use_hashing
    }

    pub const fn restrict_allocation(&self) -> bool {
        self.restrict_allocation
    }

    pub const fn initial_iter(&self) -> u16 {
        self.initial_iter
    }

    pub const fn initial_age(&self) -> u8 {
        self.initial_age
    }

    pub const fn optional_age_reset(&self) -> bool {
        self.optional_age_reset
    }

    pub fn entries(&self) -> usize {
        1usize << self.log_size
    }

    pub fn associativity(&self) -> usize {
        1usize << self.log_assoc
    }

    pub fn sets(&self) -> usize {
        self.entries() >> self.log_assoc
    }

    fn set_mask(&self) -> u64 {
        self.sets() as u64 - 1
    }

    fn tag_mask(&self) -> u16 {
        bit_mask_u16(self.tag_bits)
    }

    fn iter_mask(&self) -> u16 {
        bit_mask_u16(self.iter_bits)
    }

    fn age_max(&self) -> u8 {
        bit_mask_u8(self.age_bits)
    }

    fn confidence_max(&self) -> u8 {
        bit_mask_u8(self.confidence_bits)
    }

    fn signed_min(&self) -> i16 {
        -(1i16 << (self.with_loop_bits - 1))
    }

    fn signed_max(&self) -> i16 {
        (1i16 << (self.with_loop_bits - 1)) - 1
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopBranchPredictor {
    config: LoopBranchPredictorConfig,
    entries: Vec<LoopEntrySnapshot>,
    allocation_cursors: Vec<usize>,
    loop_use_counter: i16,
    lookup_count: u64,
    update_count: u64,
    squash_count: u64,
    used_count: u64,
    correct_count: u64,
    wrong_count: u64,
}

impl LoopBranchPredictor {
    pub fn new(config: LoopBranchPredictorConfig) -> Self {
        Self {
            entries: vec![LoopEntrySnapshot::new(); config.entries()],
            allocation_cursors: vec![0; config.sets()],
            config,
            loop_use_counter: -1,
            lookup_count: 0,
            update_count: 0,
            squash_count: 0,
            used_count: 0,
            correct_count: 0,
            wrong_count: 0,
        }
    }

    pub const fn config(&self) -> &LoopBranchPredictorConfig {
        &self.config
    }

    pub const fn loop_use_counter(&self) -> i16 {
        self.loop_use_counter
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }

    pub const fn used_count(&self) -> u64 {
        self.used_count
    }

    pub const fn correct_count(&self) -> u64 {
        self.correct_count
    }

    pub const fn wrong_count(&self) -> u64 {
        self.wrong_count
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
        previous_prediction: bool,
    ) -> Result<LoopPrediction, LoopBranchPredictorError> {
        self.thread_index(cpu)?;
        self.lookup_count += 1;

        if !conditional {
            return Ok(LoopPrediction {
                history: LoopHistory::unconditional(cpu, pc, previous_prediction),
                lookup_count: self.lookup_count,
            });
        }

        let loop_index = self.loop_index(pc);
        let loop_index_b = self.loop_index_b(pc);
        let loop_tag = self.loop_tag(pc);
        let mut loop_hit = None;
        let mut final_index = None;
        let mut loop_prediction = false;
        let mut loop_prediction_valid = false;
        let mut current_iter_before = None;
        let mut current_iter_spec_before = None;
        let mut num_iter = None;
        let mut entry_direction = None;

        for way in 0..self.config.associativity() {
            let index = self.final_index(loop_index, loop_index_b, way);
            if self.entries[index].tag == loop_tag {
                let entry = &self.entries[index];
                loop_hit = Some(way);
                final_index = Some(index);
                loop_prediction_valid = self.calc_conf(index);
                current_iter_before = Some(entry.current_iter);
                current_iter_spec_before = Some(entry.current_iter_spec);
                num_iter = Some(entry.num_iter);
                entry_direction = Some(entry.direction);
                let iter = if self.config.use_speculation() {
                    entry.current_iter_spec
                } else {
                    entry.current_iter
                };
                loop_prediction = self.loop_prediction_for_entry(entry, iter);
                break;
            }
        }

        let loop_prediction_used = self.loop_use_counter >= 0 && loop_prediction_valid;
        let predicted_taken = if loop_prediction_used {
            loop_prediction
        } else {
            previous_prediction
        };

        if self.config.use_speculation() {
            if let Some(index) = final_index {
                self.update_speculative_iter(index, predicted_taken);
            }
        }

        Ok(LoopPrediction {
            history: LoopHistory {
                cpu,
                pc,
                conditional,
                previous_prediction,
                predicted_taken,
                loop_prediction,
                loop_prediction_valid,
                loop_prediction_used,
                loop_index,
                loop_index_b,
                loop_tag,
                loop_hit,
                final_index,
                current_iter_before,
                current_iter_spec_before,
                num_iter,
                entry_direction,
            },
            lookup_count: self.lookup_count,
        })
    }

    pub fn train(
        &mut self,
        history: &LoopHistory,
        actual_taken: bool,
    ) -> Result<LoopTrainingUpdate, LoopBranchPredictorError> {
        self.thread_index(history.cpu())?;
        let loop_use_counter_before = self.loop_use_counter;
        let mut allocated_index = None;
        let mut freed_index = None;

        if !history.conditional() {
            self.update_count += 1;
            return Ok(self.training_update(
                history,
                actual_taken,
                loop_use_counter_before,
                allocated_index,
                freed_index,
            ));
        }

        if history.loop_prediction_used() {
            self.used_count += 1;
            if history.loop_predicted_taken() == actual_taken {
                self.correct_count += 1;
            } else {
                self.wrong_count += 1;
            }
        }

        if history.loop_prediction_valid()
            && history.predicted_taken() != history.loop_predicted_taken()
        {
            self.update_loop_use_counter(history.loop_predicted_taken() == actual_taken);
        }

        if let Some(index) = history.final_index() {
            if history.loop_prediction_valid() {
                if actual_taken != history.loop_predicted_taken() {
                    self.free_entry(index, true);
                    freed_index = Some(index);
                    self.update_count += 1;
                    return Ok(self.training_update(
                        history,
                        actual_taken,
                        loop_use_counter_before,
                        allocated_index,
                        freed_index,
                    ));
                }

                if history.loop_predicted_taken() != history.previous_prediction() {
                    let age_max = self.config.age_max();
                    update_unsigned_counter(&mut self.entries[index].age, true, age_max);
                }
            }

            self.update_hit(index, actual_taken, &mut freed_index);
        } else if self.should_allocate(history, actual_taken) {
            allocated_index = self.allocate_entry(history, actual_taken);
        }

        self.update_count += 1;
        Ok(self.training_update(
            history,
            actual_taken,
            loop_use_counter_before,
            allocated_index,
            freed_index,
        ))
    }

    pub fn squash(
        &mut self,
        history: &LoopHistory,
    ) -> Result<LoopSquash, LoopBranchPredictorError> {
        self.thread_index(history.cpu())?;
        let restored_current_iter_spec = if let (Some(index), Some(iter)) =
            (history.final_index(), history.current_iter_spec_before())
        {
            let old_iter = self.entries[index].current_iter_spec;
            self.entries[index].current_iter_spec = iter;
            Some((old_iter, iter))
        } else {
            None
        };

        self.squash_count += 1;

        Ok(LoopSquash {
            cpu: history.cpu(),
            pc: history.pc(),
            final_index: history.final_index(),
            old_current_iter_spec: restored_current_iter_spec.map(|(old, _)| old),
            restored_current_iter_spec: restored_current_iter_spec.map(|(_, restored)| restored),
            squash_count: self.squash_count,
        })
    }

    pub fn snapshot(&self) -> LoopBranchPredictorSnapshot {
        LoopBranchPredictorSnapshot {
            config: self.config.clone(),
            entries: self.entries.clone(),
            allocation_cursors: self.allocation_cursors.clone(),
            loop_use_counter: self.loop_use_counter,
            lookup_count: self.lookup_count,
            update_count: self.update_count,
            squash_count: self.squash_count,
            used_count: self.used_count,
            correct_count: self.correct_count,
            wrong_count: self.wrong_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &LoopBranchPredictorSnapshot,
    ) -> Result<(), LoopBranchPredictorError> {
        if self.config.entries() != snapshot.config.entries()
            || self.config.sets() != snapshot.config.sets()
        {
            return Err(LoopBranchPredictorError::SnapshotShapeMismatch {
                expected_entries: self.config.entries(),
                actual_entries: snapshot.config.entries(),
                expected_sets: self.config.sets(),
                actual_sets: snapshot.config.sets(),
            });
        }
        if self.config != snapshot.config {
            return Err(LoopBranchPredictorError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }

        self.entries.clone_from(&snapshot.entries);
        self.allocation_cursors
            .clone_from(&snapshot.allocation_cursors);
        self.loop_use_counter = snapshot.loop_use_counter;
        self.lookup_count = snapshot.lookup_count;
        self.update_count = snapshot.update_count;
        self.squash_count = snapshot.squash_count;
        self.used_count = snapshot.used_count;
        self.correct_count = snapshot.correct_count;
        self.wrong_count = snapshot.wrong_count;
        Ok(())
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, LoopBranchPredictorError> {
        let index = cpu.get() as usize;
        if index < self.config.threads() {
            Ok(index)
        } else {
            Err(LoopBranchPredictorError::UnknownThread { cpu })
        }
    }

    fn calc_conf(&self, index: usize) -> bool {
        self.entries[index].confidence == self.config.confidence_max()
    }

    fn loop_index(&self, pc: Address) -> usize {
        let mut shifted_pc = pc.get() >> self.config.inst_shift();
        if self.config.use_hashing() {
            shifted_pc ^= pc.get();
        }
        ((shifted_pc & self.config.set_mask()) << self.config.log_assoc()) as usize
    }

    fn loop_index_b(&self, pc: Address) -> usize {
        if self.config.use_hashing() {
            let pc_shift = self.config.log_size() - self.config.log_assoc();
            ((pc.get() >> pc_shift) & self.config.set_mask()) as usize
        } else {
            0
        }
    }

    fn loop_tag(&self, pc: Address) -> u16 {
        if self.config.use_hashing() {
            let pc_shift = self.config.log_size() - self.config.log_assoc();
            (((pc.get() >> pc_shift) ^ (pc.get() >> (pc_shift + self.config.tag_bits()))) as u16)
                & self.config.tag_mask()
        } else {
            let pc_shift =
                self.config.inst_shift() + self.config.log_size() - self.config.log_assoc();
            ((pc.get() >> pc_shift) as u16) & self.config.tag_mask()
        }
    }

    fn final_index(&self, loop_index: usize, loop_index_b: usize, way: usize) -> usize {
        if self.config.use_hashing() {
            (loop_index ^ ((loop_index_b >> way) << self.config.log_assoc())) + way
        } else {
            loop_index + way
        }
    }

    fn loop_prediction_for_entry(&self, entry: &LoopEntrySnapshot, iter: u16) -> bool {
        if iter.wrapping_add(1) == entry.num_iter {
            if self.config.use_direction_bit() {
                !entry.direction
            } else {
                false
            }
        } else if self.config.use_direction_bit() {
            entry.direction
        } else {
            true
        }
    }

    fn update_speculative_iter(&mut self, index: usize, taken: bool) {
        if taken != self.entries[index].direction {
            self.entries[index].current_iter_spec = 0;
        } else {
            self.entries[index].current_iter_spec =
                self.shift_iter(self.entries[index].current_iter_spec);
        }
    }

    fn update_loop_use_counter(&mut self, increment: bool) {
        if increment {
            self.loop_use_counter = (self.loop_use_counter + 1).min(self.config.signed_max());
        } else {
            self.loop_use_counter = (self.loop_use_counter - 1).max(self.config.signed_min());
        }
    }

    fn update_hit(&mut self, index: usize, actual_taken: bool, freed_index: &mut Option<usize>) {
        self.entries[index].current_iter = self.shift_iter(self.entries[index].current_iter);
        if self.entries[index].current_iter > self.entries[index].num_iter {
            self.entries[index].confidence = 0;
            if self.entries[index].num_iter != 0 {
                self.entries[index].num_iter = 0;
                if self.config.optional_age_reset() {
                    self.entries[index].age = 0;
                }
            }
        }

        let steady_direction = if self.config.use_direction_bit() {
            self.entries[index].direction
        } else {
            true
        };

        if actual_taken != steady_direction {
            if self.entries[index].current_iter == self.entries[index].num_iter {
                let confidence_max = self.config.confidence_max();
                update_unsigned_counter(&mut self.entries[index].confidence, true, confidence_max);
                if self.entries[index].num_iter < 3 {
                    self.entries[index].direction = actual_taken;
                    self.free_entry(index, true);
                    *freed_index = Some(index);
                }
            } else if self.entries[index].num_iter == 0 {
                self.entries[index].confidence = 0;
                self.entries[index].num_iter = self.entries[index].current_iter;
            } else {
                self.entries[index].num_iter = 0;
                if self.config.optional_age_reset() {
                    self.entries[index].age = 0;
                }
                self.entries[index].confidence = 0;
                *freed_index = Some(index);
            }
            self.entries[index].current_iter = 0;
        }

        if self.config.use_speculation() {
            self.entries[index].current_iter_spec = self.entries[index].current_iter;
        }
    }

    fn should_allocate(&self, history: &LoopHistory, actual_taken: bool) -> bool {
        if self.config.use_direction_bit() {
            history.predicted_taken() != actual_taken
        } else {
            actual_taken
        }
    }

    fn allocate_entry(&mut self, history: &LoopHistory, actual_taken: bool) -> Option<usize> {
        let set = history.loop_index() >> self.config.log_assoc();
        let start_way = self.allocation_cursors[set] & (self.config.associativity() - 1);
        let attempts = if self.config.restrict_allocation() {
            1
        } else {
            self.config.associativity()
        };

        for offset in 0..attempts {
            let way = (start_way + offset) & (self.config.associativity() - 1);
            let index = self.final_index(history.loop_index(), history.loop_index_b(), way);
            if self.entries[index].age == 0 {
                self.entries[index] = LoopEntrySnapshot {
                    num_iter: 0,
                    current_iter: self.config.initial_iter(),
                    current_iter_spec: self.config.initial_iter(),
                    confidence: 0,
                    tag: history.loop_tag(),
                    age: self.config.initial_age(),
                    direction: !actual_taken,
                };
                self.allocation_cursors[set] = (way + 1) & (self.config.associativity() - 1);
                return Some(index);
            }
            self.entries[index].age = self.entries[index].age.saturating_sub(1);
            if self.config.restrict_allocation() {
                break;
            }
        }

        None
    }

    fn free_entry(&mut self, index: usize, clear_age: bool) {
        self.entries[index].num_iter = 0;
        self.entries[index].confidence = 0;
        self.entries[index].current_iter = 0;
        self.entries[index].current_iter_spec = 0;
        if clear_age {
            self.entries[index].age = 0;
        }
    }

    fn shift_iter(&self, iter: u16) -> u16 {
        iter.wrapping_add(1) & self.config.iter_mask()
    }

    fn training_update(
        &self,
        history: &LoopHistory,
        actual_taken: bool,
        loop_use_counter_before: i16,
        allocated_index: Option<usize>,
        freed_index: Option<usize>,
    ) -> LoopTrainingUpdate {
        LoopTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            conditional: history.conditional(),
            loop_hit: history.loop_hit(),
            final_index: history.final_index(),
            allocated_index,
            freed_index,
            loop_prediction_valid: history.loop_prediction_valid(),
            loop_prediction_used: history.loop_prediction_used(),
            loop_prediction: history.loop_predicted_taken(),
            predicted_taken: history.predicted_taken(),
            loop_use_counter_before,
            loop_use_counter_after: self.loop_use_counter,
            update_count: self.update_count,
            used_count: self.used_count,
            correct_count: self.correct_count,
            wrong_count: self.wrong_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopPrediction {
    history: LoopHistory,
    lookup_count: u64,
}

impl LoopPrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn conditional(&self) -> bool {
        self.history.conditional()
    }

    pub const fn previous_prediction(&self) -> bool {
        self.history.previous_prediction()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn loop_predicted_taken(&self) -> bool {
        self.history.loop_predicted_taken()
    }

    pub const fn loop_prediction_valid(&self) -> bool {
        self.history.loop_prediction_valid()
    }

    pub const fn loop_prediction_used(&self) -> bool {
        self.history.loop_prediction_used()
    }

    pub const fn loop_index(&self) -> usize {
        self.history.loop_index()
    }

    pub const fn loop_index_b(&self) -> usize {
        self.history.loop_index_b()
    }

    pub const fn loop_tag(&self) -> u16 {
        self.history.loop_tag()
    }

    pub const fn loop_hit(&self) -> Option<usize> {
        self.history.loop_hit()
    }

    pub const fn final_index(&self) -> Option<usize> {
        self.history.final_index()
    }

    pub const fn current_iter_before(&self) -> Option<u16> {
        self.history.current_iter_before()
    }

    pub const fn current_iter_spec_before(&self) -> Option<u16> {
        self.history.current_iter_spec_before()
    }

    pub const fn history(&self) -> &LoopHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopHistory {
    cpu: CpuId,
    pc: Address,
    conditional: bool,
    previous_prediction: bool,
    predicted_taken: bool,
    loop_prediction: bool,
    loop_prediction_valid: bool,
    loop_prediction_used: bool,
    loop_index: usize,
    loop_index_b: usize,
    loop_tag: u16,
    loop_hit: Option<usize>,
    final_index: Option<usize>,
    current_iter_before: Option<u16>,
    current_iter_spec_before: Option<u16>,
    num_iter: Option<u16>,
    entry_direction: Option<bool>,
}

impl LoopHistory {
    fn unconditional(cpu: CpuId, pc: Address, previous_prediction: bool) -> Self {
        Self {
            cpu,
            pc,
            conditional: false,
            previous_prediction,
            predicted_taken: previous_prediction,
            loop_prediction: false,
            loop_prediction_valid: false,
            loop_prediction_used: false,
            loop_index: 0,
            loop_index_b: 0,
            loop_tag: 0,
            loop_hit: None,
            final_index: None,
            current_iter_before: None,
            current_iter_spec_before: None,
            num_iter: None,
            entry_direction: None,
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

    pub const fn loop_predicted_taken(&self) -> bool {
        self.loop_prediction
    }

    pub const fn loop_prediction_valid(&self) -> bool {
        self.loop_prediction_valid
    }

    pub const fn loop_prediction_used(&self) -> bool {
        self.loop_prediction_used
    }

    pub const fn loop_index(&self) -> usize {
        self.loop_index
    }

    pub const fn loop_index_b(&self) -> usize {
        self.loop_index_b
    }

    pub const fn loop_tag(&self) -> u16 {
        self.loop_tag
    }

    pub const fn loop_hit(&self) -> Option<usize> {
        self.loop_hit
    }

    pub const fn final_index(&self) -> Option<usize> {
        self.final_index
    }

    pub const fn current_iter_before(&self) -> Option<u16> {
        self.current_iter_before
    }

    pub const fn current_iter_spec_before(&self) -> Option<u16> {
        self.current_iter_spec_before
    }

    pub const fn num_iter(&self) -> Option<u16> {
        self.num_iter
    }

    pub const fn entry_direction(&self) -> Option<bool> {
        self.entry_direction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    conditional: bool,
    loop_hit: Option<usize>,
    final_index: Option<usize>,
    allocated_index: Option<usize>,
    freed_index: Option<usize>,
    loop_prediction_valid: bool,
    loop_prediction_used: bool,
    loop_prediction: bool,
    predicted_taken: bool,
    loop_use_counter_before: i16,
    loop_use_counter_after: i16,
    update_count: u64,
    used_count: u64,
    correct_count: u64,
    wrong_count: u64,
}

impl LoopTrainingUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn actual_taken(&self) -> bool {
        self.actual_taken
    }

    pub const fn conditional(&self) -> bool {
        self.conditional
    }

    pub const fn loop_hit(&self) -> Option<usize> {
        self.loop_hit
    }

    pub const fn final_index(&self) -> Option<usize> {
        self.final_index
    }

    pub const fn allocated_index(&self) -> Option<usize> {
        self.allocated_index
    }

    pub const fn freed_index(&self) -> Option<usize> {
        self.freed_index
    }

    pub const fn loop_prediction_valid(&self) -> bool {
        self.loop_prediction_valid
    }

    pub const fn loop_prediction_used(&self) -> bool {
        self.loop_prediction_used
    }

    pub const fn loop_predicted_taken(&self) -> bool {
        self.loop_prediction
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn loop_use_counter_before(&self) -> i16 {
        self.loop_use_counter_before
    }

    pub const fn loop_use_counter_after(&self) -> i16 {
        self.loop_use_counter_after
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn used_count(&self) -> u64 {
        self.used_count
    }

    pub const fn correct_count(&self) -> u64 {
        self.correct_count
    }

    pub const fn wrong_count(&self) -> u64 {
        self.wrong_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopSquash {
    cpu: CpuId,
    pc: Address,
    final_index: Option<usize>,
    old_current_iter_spec: Option<u16>,
    restored_current_iter_spec: Option<u16>,
    squash_count: u64,
}

impl LoopSquash {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn final_index(&self) -> Option<usize> {
        self.final_index
    }

    pub const fn old_current_iter_spec(&self) -> Option<u16> {
        self.old_current_iter_spec
    }

    pub const fn restored_current_iter_spec(&self) -> Option<u16> {
        self.restored_current_iter_spec
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopEntrySnapshot {
    num_iter: u16,
    current_iter: u16,
    current_iter_spec: u16,
    confidence: u8,
    tag: u16,
    age: u8,
    direction: bool,
}

impl LoopEntrySnapshot {
    const fn new() -> Self {
        Self {
            num_iter: 0,
            current_iter: 0,
            current_iter_spec: 0,
            confidence: 0,
            tag: 0,
            age: 0,
            direction: false,
        }
    }

    pub const fn num_iter(&self) -> u16 {
        self.num_iter
    }

    pub const fn current_iter(&self) -> u16 {
        self.current_iter
    }

    pub const fn current_iter_spec(&self) -> u16 {
        self.current_iter_spec
    }

    pub const fn confidence(&self) -> u8 {
        self.confidence
    }

    pub const fn tag(&self) -> u16 {
        self.tag
    }

    pub const fn age(&self) -> u8 {
        self.age
    }

    pub const fn direction(&self) -> bool {
        self.direction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopBranchPredictorSnapshot {
    config: LoopBranchPredictorConfig,
    entries: Vec<LoopEntrySnapshot>,
    allocation_cursors: Vec<usize>,
    loop_use_counter: i16,
    lookup_count: u64,
    update_count: u64,
    squash_count: u64,
    used_count: u64,
    correct_count: u64,
    wrong_count: u64,
}

impl LoopBranchPredictorSnapshot {
    pub const fn config(&self) -> &LoopBranchPredictorConfig {
        &self.config
    }

    pub fn entries(&self) -> &[LoopEntrySnapshot] {
        &self.entries
    }

    pub fn allocation_cursors(&self) -> &[usize] {
        &self.allocation_cursors
    }

    pub const fn loop_use_counter(&self) -> i16 {
        self.loop_use_counter
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn squash_count(&self) -> u64 {
        self.squash_count
    }

    pub const fn used_count(&self) -> u64 {
        self.used_count
    }

    pub const fn correct_count(&self) -> u64 {
        self.correct_count
    }

    pub const fn wrong_count(&self) -> u64 {
        self.wrong_count
    }
}

fn bit_mask_u16(bits: u8) -> u16 {
    if bits >= u16::BITS as u8 {
        u16::MAX
    } else {
        ((1u32 << bits) - 1) as u16
    }
}

fn bit_mask_u8(bits: u8) -> u8 {
    if bits >= u8::BITS as u8 {
        u8::MAX
    } else {
        ((1u16 << bits) - 1) as u8
    }
}

fn update_unsigned_counter(counter: &mut u8, increment: bool, max: u8) {
    if increment {
        *counter = counter.saturating_add(1).min(max);
    } else {
        *counter = counter.saturating_sub(1);
    }
}
