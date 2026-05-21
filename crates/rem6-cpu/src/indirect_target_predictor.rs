use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::branch_predictor::BranchTargetKind;
use crate::CpuId;

const DEFAULT_TAG_BITS: u8 = 16;
const DEFAULT_PATH_LENGTH: usize = 4;
const DEFAULT_SPECULATIVE_PATH_LENGTH: usize = 8;
const DEFAULT_INST_SHIFT: u8 = 2;
const DEFAULT_HISTORY_BITS: u8 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndirectTargetPredictorError {
    ZeroThreads,
    ZeroSets,
    SetCountNotPowerOfTwo {
        sets: usize,
    },
    ZeroWays,
    TagBitsOutOfRange {
        bits: u8,
    },
    ZeroPathLength,
    InstShiftOutOfRange {
        bits: u8,
    },
    HistoryBitsOutOfRange {
        bits: u8,
    },
    UnknownThread {
        cpu: CpuId,
    },
    SnapshotShapeMismatch {
        expected_threads: usize,
        actual_threads: usize,
        expected_sets: usize,
        actual_sets: usize,
        expected_ways: usize,
        actual_ways: usize,
    },
    SnapshotConfigMismatch {
        expected: IndirectTargetPredictorConfig,
        actual: IndirectTargetPredictorConfig,
    },
}

impl fmt::Display for IndirectTargetPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "indirect target predictor has no threads"),
            Self::ZeroSets => write!(formatter, "indirect target predictor has no sets"),
            Self::SetCountNotPowerOfTwo { sets } => write!(
                formatter,
                "indirect target predictor set count {sets} is not a power of two"
            ),
            Self::ZeroWays => write!(formatter, "indirect target predictor has no ways"),
            Self::TagBitsOutOfRange { bits } => write!(
                formatter,
                "indirect target predictor tag width {bits} is outside 1..=64"
            ),
            Self::ZeroPathLength => {
                write!(formatter, "indirect target predictor path length is zero")
            }
            Self::InstShiftOutOfRange { bits } => write!(
                formatter,
                "indirect target predictor instruction shift {bits} is outside 0..=63"
            ),
            Self::HistoryBitsOutOfRange { bits } => write!(
                formatter,
                "indirect target predictor history width {bits} is outside 1..=64"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "indirect target predictor thread {} is not configured",
                cpu.get()
            ),
            Self::SnapshotShapeMismatch {
                expected_threads,
                actual_threads,
                expected_sets,
                actual_sets,
                expected_ways,
                actual_ways,
            } => write!(
                formatter,
                "indirect target predictor snapshot shape threads={actual_threads}, sets={actual_sets}, ways={actual_ways} does not match predictor threads={expected_threads}, sets={expected_sets}, ways={expected_ways}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "indirect target predictor snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
        }
    }
}

impl Error for IndirectTargetPredictorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetPredictorConfig {
    threads: usize,
    sets: usize,
    ways: usize,
    tag_bits: u8,
    path_length: usize,
    speculative_path_length: usize,
    inst_shift: u8,
    history_bits: u8,
    hash_ghr: bool,
    hash_targets: bool,
}

impl IndirectTargetPredictorConfig {
    pub fn new(
        threads: usize,
        sets: usize,
        ways: usize,
    ) -> Result<Self, IndirectTargetPredictorError> {
        Self::with_options(
            threads,
            sets,
            ways,
            DEFAULT_TAG_BITS,
            DEFAULT_PATH_LENGTH,
            DEFAULT_SPECULATIVE_PATH_LENGTH,
            DEFAULT_INST_SHIFT,
            DEFAULT_HISTORY_BITS,
            true,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        threads: usize,
        sets: usize,
        ways: usize,
        tag_bits: u8,
        path_length: usize,
        speculative_path_length: usize,
        inst_shift: u8,
        history_bits: u8,
        hash_ghr: bool,
        hash_targets: bool,
    ) -> Result<Self, IndirectTargetPredictorError> {
        if threads == 0 {
            return Err(IndirectTargetPredictorError::ZeroThreads);
        }
        if sets == 0 {
            return Err(IndirectTargetPredictorError::ZeroSets);
        }
        if !sets.is_power_of_two() {
            return Err(IndirectTargetPredictorError::SetCountNotPowerOfTwo { sets });
        }
        if ways == 0 {
            return Err(IndirectTargetPredictorError::ZeroWays);
        }
        if !(1..=64).contains(&tag_bits) {
            return Err(IndirectTargetPredictorError::TagBitsOutOfRange { bits: tag_bits });
        }
        if path_length == 0 {
            return Err(IndirectTargetPredictorError::ZeroPathLength);
        }
        if inst_shift > 63 {
            return Err(IndirectTargetPredictorError::InstShiftOutOfRange { bits: inst_shift });
        }
        if !(1..=64).contains(&history_bits) {
            return Err(IndirectTargetPredictorError::HistoryBitsOutOfRange { bits: history_bits });
        }

        Ok(Self {
            threads,
            sets,
            ways,
            tag_bits,
            path_length,
            speculative_path_length,
            inst_shift,
            history_bits,
            hash_ghr,
            hash_targets,
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn sets(&self) -> usize {
        self.sets
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }

    pub const fn tag_bits(&self) -> u8 {
        self.tag_bits
    }

    pub const fn path_length(&self) -> usize {
        self.path_length
    }

    pub const fn speculative_path_length(&self) -> usize {
        self.speculative_path_length
    }

    pub const fn inst_shift(&self) -> u8 {
        self.inst_shift
    }

    pub const fn history_bits(&self) -> u8 {
        self.history_bits
    }

    pub const fn hash_ghr(&self) -> bool {
        self.hash_ghr
    }

    pub const fn hash_targets(&self) -> bool {
        self.hash_targets
    }

    fn history_mask(&self) -> u64 {
        bit_mask(self.history_bits)
    }

    fn tag_mask(&self) -> u64 {
        bit_mask(self.tag_bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetPredictor {
    config: IndirectTargetPredictorConfig,
    entries: Vec<Option<IndirectTargetEntry>>,
    threads: Vec<IndirectTargetThreadSnapshot>,
    access_sequence: u64,
    lookup_count: u64,
    hit_count: u64,
    miss_count: u64,
    target_record_count: u64,
    indirect_record_count: u64,
    eviction_count: u64,
    speculative_overflow_count: u64,
}

impl IndirectTargetPredictor {
    pub fn new(config: IndirectTargetPredictorConfig) -> Self {
        Self {
            entries: vec![None; config.sets() * config.ways()],
            threads: vec![IndirectTargetThreadSnapshot::new(); config.threads()],
            config,
            access_sequence: 0,
            lookup_count: 0,
            hit_count: 0,
            miss_count: 0,
            target_record_count: 0,
            indirect_record_count: 0,
            eviction_count: 0,
            speculative_overflow_count: 0,
        }
    }

    pub const fn config(&self) -> &IndirectTargetPredictorConfig {
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

    pub const fn target_record_count(&self) -> u64 {
        self.target_record_count
    }

    pub const fn indirect_record_count(&self) -> u64 {
        self.indirect_record_count
    }

    pub const fn eviction_count(&self) -> u64 {
        self.eviction_count
    }

    pub const fn speculative_overflow_count(&self) -> u64 {
        self.speculative_overflow_count
    }

    pub fn reset(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
        for thread in &mut self.threads {
            thread.ghr = 0;
            thread.path_history.clear();
        }

        self.access_sequence = 0;
        self.lookup_count = 0;
        self.hit_count = 0;
        self.miss_count = 0;
        self.target_record_count = 0;
        self.indirect_record_count = 0;
        self.eviction_count = 0;
        self.speculative_overflow_count = 0;
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        sequence: IndirectTargetSequence,
        pc: Address,
        kind: BranchTargetKind,
    ) -> Result<IndirectTargetPrediction, IndirectTargetPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let looked_up_target = is_indirect_no_return(kind);
        let set = self.set_index(pc, &self.threads[thread_index]);
        let tag = self.tag(pc);
        let mut history = IndirectTargetHistory {
            cpu,
            sequence,
            pc,
            kind,
            set,
            tag,
            hit: false,
            target: None,
            ghr_before: self.threads[thread_index].ghr,
            path_length_before: self.threads[thread_index].path_history.len(),
            looked_up_target,
        };

        if !looked_up_target {
            return Ok(IndirectTargetPrediction {
                history,
                way: None,
                lookup_count: self.lookup_count,
            });
        }

        self.lookup_count += 1;
        let mut hit = None;
        for way in 0..self.config.ways() {
            let index = self.entry_index(set, way);
            let Some(entry) = &self.entries[index] else {
                continue;
            };
            if entry.pc() == pc && entry.tag() == tag {
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
                    .expect("hit index contains indirect target entry");
                entry.last_used = access_sequence;
                history.hit = true;
                history.target = Some(entry.target());

                Ok(IndirectTargetPrediction {
                    history,
                    way: Some(way),
                    lookup_count: self.lookup_count,
                })
            }
            None => {
                self.miss_count += 1;
                Ok(IndirectTargetPrediction {
                    history,
                    way: None,
                    lookup_count: self.lookup_count,
                })
            }
        }
    }

    pub fn update(
        &mut self,
        history: &IndirectTargetHistory,
        taken: bool,
        target: Address,
        kind: BranchTargetKind,
        squashed: bool,
    ) -> Result<IndirectTargetUpdate, IndirectTargetPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let was_indirect = is_indirect_no_return(kind);

        if squashed {
            self.threads[thread_index].ghr = history.ghr_before();
            if was_indirect {
                self.pop_speculative_path_entry(thread_index);
            }
        }

        let target_record_location = (squashed && was_indirect && taken).then(|| {
            (
                self.set_index(history.pc(), &self.threads[thread_index]),
                self.tag(history.pc()),
            )
        });

        let mut indirect_recorded = false;
        if was_indirect {
            self.threads[thread_index]
                .path_history
                .push(IndirectTargetPathEntry {
                    pc: history.pc(),
                    target,
                    sequence: history.sequence(),
                });
            self.indirect_record_count += 1;
            indirect_recorded = true;
        }

        let ghr_before = self.threads[thread_index].ghr;
        let ghr_after = self.shift_history(ghr_before, taken);
        self.threads[thread_index].ghr = ghr_after;

        let replaced = if let Some((set, tag)) = target_record_location {
            self.target_record_count += 1;
            self.record_target(set, tag, history.pc(), target)
        } else {
            None
        };

        Ok(IndirectTargetUpdate {
            cpu: history.cpu(),
            sequence: history.sequence(),
            pc: history.pc(),
            kind,
            taken,
            target,
            squashed,
            ghr_before,
            ghr_after,
            indirect_recorded,
            target_recorded: target_record_location.is_some(),
            replaced,
            path_length_after: self.threads[thread_index].path_history.len(),
        })
    }

    pub fn squash(
        &mut self,
        history: &IndirectTargetHistory,
    ) -> Result<IndirectTargetSquash, IndirectTargetPredictorError> {
        let thread_index = self.thread_index(history.cpu())?;
        let removed_path_entry = if is_indirect_no_return(history.kind()) {
            Some(self.pop_speculative_path_entry(thread_index))
        } else {
            None
        }
        .flatten();
        let ghr_before = self.threads[thread_index].ghr;
        self.threads[thread_index].ghr = history.ghr_before();

        Ok(IndirectTargetSquash {
            cpu: history.cpu(),
            sequence: history.sequence(),
            pc: history.pc(),
            ghr_before,
            ghr_after: history.ghr_before(),
            removed_path_entry,
        })
    }

    pub fn commit(
        &mut self,
        cpu: CpuId,
    ) -> Result<IndirectTargetCommit, IndirectTargetPredictorError> {
        let thread_index = self.thread_index(cpu)?;
        let max_path_entries = self
            .config
            .path_length()
            .saturating_add(self.config.speculative_path_length());
        let mut trimmed = 0;
        while self.threads[thread_index].path_history.len() > max_path_entries {
            self.threads[thread_index].path_history.remove(0);
            trimmed += 1;
        }

        Ok(IndirectTargetCommit {
            cpu,
            trimmed,
            remaining_path_length: self.threads[thread_index].path_history.len(),
        })
    }

    pub fn snapshot(&self) -> IndirectTargetPredictorSnapshot {
        IndirectTargetPredictorSnapshot {
            config: self.config.clone(),
            entries: self.entries.clone(),
            threads: self.threads.clone(),
            access_sequence: self.access_sequence,
            lookup_count: self.lookup_count,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            target_record_count: self.target_record_count,
            indirect_record_count: self.indirect_record_count,
            eviction_count: self.eviction_count,
            speculative_overflow_count: self.speculative_overflow_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &IndirectTargetPredictorSnapshot,
    ) -> Result<(), IndirectTargetPredictorError> {
        if self.config.threads() != snapshot.config.threads()
            || self.config.sets() != snapshot.config.sets()
            || self.config.ways() != snapshot.config.ways()
        {
            return Err(IndirectTargetPredictorError::SnapshotShapeMismatch {
                expected_threads: self.config.threads(),
                actual_threads: snapshot.config.threads(),
                expected_sets: self.config.sets(),
                actual_sets: snapshot.config.sets(),
                expected_ways: self.config.ways(),
                actual_ways: snapshot.config.ways(),
            });
        }
        if self.config != snapshot.config {
            return Err(IndirectTargetPredictorError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }

        self.entries.clone_from(&snapshot.entries);
        self.threads.clone_from(&snapshot.threads);
        self.access_sequence = snapshot.access_sequence;
        self.lookup_count = snapshot.lookup_count;
        self.hit_count = snapshot.hit_count;
        self.miss_count = snapshot.miss_count;
        self.target_record_count = snapshot.target_record_count;
        self.indirect_record_count = snapshot.indirect_record_count;
        self.eviction_count = snapshot.eviction_count;
        self.speculative_overflow_count = snapshot.speculative_overflow_count;
        Ok(())
    }

    fn record_target(
        &mut self,
        set: usize,
        tag: u64,
        pc: Address,
        target: Address,
    ) -> Option<IndirectTargetEntry> {
        let access_sequence = self.next_access_sequence();

        for way in 0..self.config.ways() {
            let index = self.entry_index(set, way);
            let Some(entry) = &mut self.entries[index] else {
                continue;
            };
            if entry.pc() == pc && entry.tag() == tag {
                entry.target = target;
                entry.last_used = access_sequence;
                return None;
            }
        }

        let (way, index, replaced) = self.replacement_slot(set);
        if replaced.is_some() {
            self.eviction_count += 1;
        }
        self.entries[index] = Some(IndirectTargetEntry {
            pc,
            target,
            tag,
            set,
            way,
            last_used: access_sequence,
        });
        replaced
    }

    fn replacement_slot(&self, set: usize) -> (usize, usize, Option<IndirectTargetEntry>) {
        for way in 0..self.config.ways() {
            let index = self.entry_index(set, way);
            if self.entries[index].is_none() {
                return (way, index, None);
            }
        }

        let victim_way = (0..self.config.ways())
            .min_by_key(|way| {
                self.entries[self.entry_index(set, *way)]
                    .as_ref()
                    .expect("full indirect target set has entry")
                    .last_used
            })
            .expect("ways is non-zero");
        let victim_index = self.entry_index(set, victim_way);
        (victim_way, victim_index, self.entries[victim_index].clone())
    }

    fn pop_speculative_path_entry(
        &mut self,
        thread_index: usize,
    ) -> Option<IndirectTargetPathEntry> {
        if self.threads[thread_index].path_history.len() < self.config.path_length() {
            self.speculative_overflow_count += 1;
        }
        self.threads[thread_index].path_history.pop()
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, IndirectTargetPredictorError> {
        let index = cpu.get() as usize;
        if index < self.threads.len() {
            Ok(index)
        } else {
            Err(IndirectTargetPredictorError::UnknownThread { cpu })
        }
    }

    fn set_index(&self, pc: Address, thread: &IndirectTargetThreadSnapshot) -> usize {
        let mut hash = pc.get() >> self.config.inst_shift();
        if self.config.hash_ghr() {
            hash ^= thread.ghr();
        }
        if self.config.hash_targets() {
            let set_bits = self.config.sets().trailing_zeros() as usize;
            let hash_shift = set_bits / self.config.path_length();
            for (path_index, entry) in thread
                .path_history()
                .iter()
                .rev()
                .take(self.config.path_length())
                .enumerate()
            {
                let shift = self.config.inst_shift() as usize + path_index * hash_shift;
                if shift < 64 {
                    hash ^= entry.target().get() >> shift;
                }
            }
        }

        (hash & (self.config.sets() as u64 - 1)) as usize
    }

    fn tag(&self, pc: Address) -> u64 {
        (pc.get() >> self.config.inst_shift()) & self.config.tag_mask()
    }

    fn entry_index(&self, set: usize, way: usize) -> usize {
        set * self.config.ways() + way
    }

    fn shift_history(&self, old: u64, taken: bool) -> u64 {
        ((old << 1) | u64::from(taken)) & self.config.history_mask()
    }

    fn next_access_sequence(&mut self) -> u64 {
        let access_sequence = self.access_sequence;
        self.access_sequence = self.access_sequence.saturating_add(1);
        access_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetPrediction {
    history: IndirectTargetHistory,
    way: Option<usize>,
    lookup_count: u64,
}

impl IndirectTargetPrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn sequence(&self) -> IndirectTargetSequence {
        self.history.sequence()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn kind(&self) -> BranchTargetKind {
        self.history.kind()
    }

    pub const fn set(&self) -> usize {
        self.history.set()
    }

    pub const fn tag(&self) -> u64 {
        self.history.tag()
    }

    pub const fn way(&self) -> Option<usize> {
        self.way
    }

    pub const fn hit(&self) -> bool {
        self.history.hit()
    }

    pub const fn target(&self) -> Option<Address> {
        self.history.target()
    }

    pub const fn history(&self) -> &IndirectTargetHistory {
        &self.history
    }

    pub const fn looked_up_target(&self) -> bool {
        self.history.looked_up_target()
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetUpdate {
    cpu: CpuId,
    sequence: IndirectTargetSequence,
    pc: Address,
    kind: BranchTargetKind,
    taken: bool,
    target: Address,
    squashed: bool,
    ghr_before: u64,
    ghr_after: u64,
    indirect_recorded: bool,
    target_recorded: bool,
    replaced: Option<IndirectTargetEntry>,
    path_length_after: usize,
}

impl IndirectTargetUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn sequence(&self) -> IndirectTargetSequence {
        self.sequence
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub const fn taken(&self) -> bool {
        self.taken
    }

    pub const fn target(&self) -> Address {
        self.target
    }

    pub const fn squashed(&self) -> bool {
        self.squashed
    }

    pub const fn ghr_before(&self) -> u64 {
        self.ghr_before
    }

    pub const fn ghr_after(&self) -> u64 {
        self.ghr_after
    }

    pub const fn indirect_recorded(&self) -> bool {
        self.indirect_recorded
    }

    pub const fn target_recorded(&self) -> bool {
        self.target_recorded
    }

    pub const fn replaced(&self) -> Option<&IndirectTargetEntry> {
        self.replaced.as_ref()
    }

    pub const fn path_length_after(&self) -> usize {
        self.path_length_after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetSquash {
    cpu: CpuId,
    sequence: IndirectTargetSequence,
    pc: Address,
    ghr_before: u64,
    ghr_after: u64,
    removed_path_entry: Option<IndirectTargetPathEntry>,
}

impl IndirectTargetSquash {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn sequence(&self) -> IndirectTargetSequence {
        self.sequence
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn ghr_before(&self) -> u64 {
        self.ghr_before
    }

    pub const fn ghr_after(&self) -> u64 {
        self.ghr_after
    }

    pub const fn removed_path_entry(&self) -> Option<&IndirectTargetPathEntry> {
        self.removed_path_entry.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetCommit {
    cpu: CpuId,
    trimmed: usize,
    remaining_path_length: usize,
}

impl IndirectTargetCommit {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn trimmed(&self) -> usize {
        self.trimmed
    }

    pub const fn remaining_path_length(&self) -> usize {
        self.remaining_path_length
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IndirectTargetSequence(u64);

impl IndirectTargetSequence {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetHistory {
    cpu: CpuId,
    sequence: IndirectTargetSequence,
    pc: Address,
    kind: BranchTargetKind,
    set: usize,
    tag: u64,
    hit: bool,
    target: Option<Address>,
    ghr_before: u64,
    path_length_before: usize,
    looked_up_target: bool,
}

impl IndirectTargetHistory {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn sequence(&self) -> IndirectTargetSequence {
        self.sequence
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn tag(&self) -> u64 {
        self.tag
    }

    pub const fn hit(&self) -> bool {
        self.hit
    }

    pub const fn target(&self) -> Option<Address> {
        self.target
    }

    pub const fn ghr_before(&self) -> u64 {
        self.ghr_before
    }

    pub const fn path_length_before(&self) -> usize {
        self.path_length_before
    }

    pub const fn looked_up_target(&self) -> bool {
        self.looked_up_target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetEntry {
    pc: Address,
    target: Address,
    tag: u64,
    set: usize,
    way: usize,
    last_used: u64,
}

impl IndirectTargetEntry {
    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn target(&self) -> Address {
        self.target
    }

    pub const fn tag(&self) -> u64 {
        self.tag
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
pub struct IndirectTargetPathEntry {
    pc: Address,
    target: Address,
    sequence: IndirectTargetSequence,
}

impl IndirectTargetPathEntry {
    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn target(&self) -> Address {
        self.target
    }

    pub const fn sequence(&self) -> IndirectTargetSequence {
        self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetThreadSnapshot {
    ghr: u64,
    path_history: Vec<IndirectTargetPathEntry>,
}

impl IndirectTargetThreadSnapshot {
    const fn new() -> Self {
        Self {
            ghr: 0,
            path_history: Vec::new(),
        }
    }

    pub const fn ghr(&self) -> u64 {
        self.ghr
    }

    pub fn path_history(&self) -> &[IndirectTargetPathEntry] {
        &self.path_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectTargetPredictorSnapshot {
    config: IndirectTargetPredictorConfig,
    entries: Vec<Option<IndirectTargetEntry>>,
    threads: Vec<IndirectTargetThreadSnapshot>,
    access_sequence: u64,
    lookup_count: u64,
    hit_count: u64,
    miss_count: u64,
    target_record_count: u64,
    indirect_record_count: u64,
    eviction_count: u64,
    speculative_overflow_count: u64,
}

impl IndirectTargetPredictorSnapshot {
    pub const fn config(&self) -> &IndirectTargetPredictorConfig {
        &self.config
    }

    pub fn entries(&self) -> &[Option<IndirectTargetEntry>] {
        &self.entries
    }

    pub fn threads(&self) -> &[IndirectTargetThreadSnapshot] {
        &self.threads
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

    pub const fn target_record_count(&self) -> u64 {
        self.target_record_count
    }

    pub const fn indirect_record_count(&self) -> u64 {
        self.indirect_record_count
    }

    pub const fn eviction_count(&self) -> u64 {
        self.eviction_count
    }

    pub const fn speculative_overflow_count(&self) -> u64 {
        self.speculative_overflow_count
    }
}

const fn is_indirect_no_return(kind: BranchTargetKind) -> bool {
    matches!(
        kind,
        BranchTargetKind::CallIndirect
            | BranchTargetKind::IndirectConditional
            | BranchTargetKind::IndirectUnconditional
    )
}

fn bit_mask(bits: u8) -> u64 {
    if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}
