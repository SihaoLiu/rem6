use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::return_address_stack::{ReturnAddressStackError, ReturnAddressStackOperationId};

mod math;

use math::{history_mask, saturating_branch_counter};

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
    },
    InvalidCheckpointFlag {
        name: &'static str,
        value: u8,
    },
    InvalidCheckpointSpeculationIndex {
        index: usize,
        table_entries: usize,
    },
    InvalidCheckpointSpeculationPcIndex {
        pc: Address,
        index: usize,
        expected: usize,
    },
    InvalidCheckpointSpeculationOrder {
        sequence: u64,
        id: BranchSpeculationId,
        expected: BranchSpeculationId,
    },
    InvalidCheckpointNextSpeculation {
        next: BranchSpeculationId,
        pending: BranchSpeculationId,
    },
    InvalidCheckpointNextSpeculationOverflow {
        next: BranchSpeculationId,
    },
    InvalidBranchTargetBufferCheckpoint {
        error: BranchTargetBufferError,
    },
    InvalidReturnAddressStackCheckpoint {
        error: ReturnAddressStackError,
    },
    InvalidCheckpointReturnAddressStackDepth {
        depth: usize,
        entries: usize,
    },
    InvalidCheckpointReturnAddressStackOperationOrder {
        id: ReturnAddressStackOperationId,
        expected: ReturnAddressStackOperationId,
    },
    InvalidCheckpointReturnAddressStackOperation {
        id: ReturnAddressStackOperationId,
    },
    InvalidCheckpointReturnAddressStackNextOperation {
        next: ReturnAddressStackOperationId,
        pending: ReturnAddressStackOperationId,
    },
    InvalidCheckpointReturnAddressStackNextOperationOverflow {
        next: ReturnAddressStackOperationId,
    },
    DuplicateCheckpointReturnAddressStackOperation {
        id: ReturnAddressStackOperationId,
    },
    MissingCheckpointReturnAddressStackOperation {
        id: ReturnAddressStackOperationId,
    },
    UnmappedCheckpointReturnAddressStackOperation {
        id: ReturnAddressStackOperationId,
    },
    DuplicateCheckpointSpeculationSequence {
        sequence: u64,
    },
    DuplicateCheckpointSpeculationId {
        id: BranchSpeculationId,
    },
    MissingCheckpointSpeculation {
        id: BranchSpeculationId,
    },
    UnmappedCheckpointSpeculation {
        id: BranchSpeculationId,
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
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "branch predictor checkpoint has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "branch predictor checkpoint magic is invalid")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "branch predictor checkpoint version {version} is not supported"
            ),
            Self::CheckpointValueTooLarge { name, value, max } => write!(
                formatter,
                "branch predictor checkpoint {name} value {value} exceeds {max}"
            ),
            Self::InvalidCheckpointCounter { value } => write!(
                formatter,
                "branch predictor checkpoint counter value {value} is invalid"
            ),
            Self::InvalidCheckpointFlag { name, value } => write!(
                formatter,
                "branch predictor checkpoint flag {name} has invalid value {value}"
            ),
            Self::InvalidCheckpointSpeculationIndex {
                index,
                table_entries,
            } => write!(
                formatter,
                "branch predictor checkpoint speculation index {index} is outside {table_entries} entries"
            ),
            Self::InvalidCheckpointSpeculationPcIndex {
                pc,
                index,
                expected,
            } => write!(
                formatter,
                "branch predictor checkpoint speculation for PC {} has index {index}; expected {expected}",
                pc.get()
            ),
            Self::InvalidCheckpointSpeculationOrder {
                sequence,
                id,
                expected,
            } => write!(
                formatter,
                "branch predictor checkpoint sequence {sequence} maps speculation {}; expected {}",
                id.get(),
                expected.get()
            ),
            Self::InvalidCheckpointNextSpeculation { next, pending } => write!(
                formatter,
                "branch predictor checkpoint next speculation {} does not advance beyond pending speculation {}",
                next.get(),
                pending.get()
            ),
            Self::InvalidCheckpointNextSpeculationOverflow { next } => write!(
                formatter,
                "branch predictor checkpoint next speculation {} cannot advance",
                next.get()
            ),
            Self::InvalidBranchTargetBufferCheckpoint { error } => write!(
                formatter,
                "branch predictor checkpoint has invalid branch target buffer snapshot: {error}"
            ),
            Self::InvalidReturnAddressStackCheckpoint { error } => write!(
                formatter,
                "branch predictor checkpoint has invalid return-address stack snapshot: {error}"
            ),
            Self::InvalidCheckpointReturnAddressStackDepth { depth, entries } => write!(
                formatter,
                "branch predictor checkpoint return-address stack has depth {depth}; expected at most {entries}"
            ),
            Self::InvalidCheckpointReturnAddressStackOperationOrder { id, expected } => write!(
                formatter,
                "branch predictor checkpoint return-address stack operation {}; expected {}",
                id.get(),
                expected.get()
            ),
            Self::InvalidCheckpointReturnAddressStackOperation { id } => write!(
                formatter,
                "branch predictor checkpoint return-address stack operation {} is inconsistent",
                id.get()
            ),
            Self::InvalidCheckpointReturnAddressStackNextOperation { next, pending } => write!(
                formatter,
                "branch predictor checkpoint next return-address stack operation {} does not advance beyond pending operation {}",
                next.get(),
                pending.get()
            ),
            Self::InvalidCheckpointReturnAddressStackNextOperationOverflow { next } => write!(
                formatter,
                "branch predictor checkpoint next return-address stack operation {} cannot advance",
                next.get()
            ),
            Self::DuplicateCheckpointReturnAddressStackOperation { id } => write!(
                formatter,
                "branch predictor checkpoint repeats return-address stack operation id {}",
                id.get()
            ),
            Self::MissingCheckpointReturnAddressStackOperation { id } => write!(
                formatter,
                "branch predictor checkpoint maps unknown return-address stack operation id {}",
                id.get()
            ),
            Self::UnmappedCheckpointReturnAddressStackOperation { id } => write!(
                formatter,
                "branch predictor checkpoint leaves return-address stack operation id {} without an active sequence",
                id.get()
            ),
            Self::DuplicateCheckpointSpeculationSequence { sequence } => write!(
                formatter,
                "branch predictor checkpoint repeats active speculation sequence {sequence}"
            ),
            Self::DuplicateCheckpointSpeculationId { id } => write!(
                formatter,
                "branch predictor checkpoint repeats active speculation id {}",
                id.get()
            ),
            Self::MissingCheckpointSpeculation { id } => write!(
                formatter,
                "branch predictor checkpoint maps unknown speculation id {}",
                id.get()
            ),
            Self::UnmappedCheckpointSpeculation { id } => write!(
                formatter,
                "branch predictor checkpoint leaves speculation id {} without an active sequence",
                id.get()
            ),
        }
    }
}

impl Error for BranchPredictorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidBranchTargetBufferCheckpoint { error } => Some(error),
            Self::InvalidReturnAddressStackCheckpoint { error } => Some(error),
            _ => None,
        }
    }
}

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
        self.push_speculation(prediction)
    }

    pub(crate) fn predict_speculative_with_prediction(
        &mut self,
        pc: Address,
        predicted_taken: bool,
        target: Option<Address>,
    ) -> BranchSpeculation {
        let index = self.index(pc);
        let prediction = BranchPrediction {
            pc,
            index,
            predicted_taken,
            target: predicted_taken.then_some(target).flatten(),
            counter: self.counters[index],
        };
        self.push_speculation(prediction)
    }

    fn push_speculation(&mut self, prediction: BranchPrediction) -> BranchSpeculation {
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

    pub(crate) fn pending_speculation(
        &self,
        id: BranchSpeculationId,
    ) -> Option<&BranchSpeculation> {
        self.pending_speculations
            .iter()
            .find(|speculation| speculation.id() == id)
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

    pub fn discard_speculation(
        &mut self,
        id: BranchSpeculationId,
    ) -> Result<BranchSpeculationDiscard, BranchPredictorError> {
        let Some(index) = self
            .pending_speculations
            .iter()
            .position(|speculation| speculation.id() == id)
        else {
            return Err(BranchPredictorError::UnknownSpeculation { id });
        };

        let restored_history = self.pending_speculations[index].history_before();
        let mut discarded_and_youngers = self.pending_speculations.split_off(index);
        let discarded = discarded_and_youngers.remove(0);
        self.speculative_history = restored_history;

        Ok(BranchSpeculationDiscard {
            discarded,
            removed_youngers: discarded_and_youngers,
            restored_history,
        })
    }

    pub fn discard_all_speculations(&mut self) -> Vec<BranchSpeculation> {
        let discarded = std::mem::take(&mut self.pending_speculations);
        self.speculative_history = self.committed_history;
        discarded
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
    pub(crate) pc: Address,
    pub(crate) index: usize,
    pub(crate) predicted_taken: bool,
    pub(crate) target: Option<Address>,
    pub(crate) counter: u8,
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
    pub(crate) id: BranchSpeculationId,
    pub(crate) prediction: BranchPrediction,
    pub(crate) history_before: u64,
    pub(crate) history_after: u64,
    pub(crate) history_taken: bool,
    pub(crate) repaired: bool,
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
pub struct BranchSpeculationDiscard {
    discarded: BranchSpeculation,
    removed_youngers: Vec<BranchSpeculation>,
    restored_history: u64,
}

impl BranchSpeculationDiscard {
    pub const fn discarded(&self) -> &BranchSpeculation {
        &self.discarded
    }

    pub fn removed_youngers(&self) -> &[BranchSpeculation] {
        &self.removed_youngers
    }

    pub const fn restored_history(&self) -> u64 {
        self.restored_history
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

    pub const fn predicted_target(&self) -> Option<Address> {
        self.prediction.target()
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
    pub(crate) config: BranchPredictorConfig,
    pub(crate) counters: Vec<u8>,
    pub(crate) targets: Vec<Option<Address>>,
    pub(crate) update_count: u64,
    pub(crate) committed_history: u64,
    pub(crate) speculative_history: u64,
    pub(crate) next_speculation: BranchSpeculationId,
    pub(crate) pending_speculations: Vec<BranchSpeculation>,
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
    SnapshotEntrySetMismatch(Address, usize, usize),
    DuplicateSnapshotEntry(Address),
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
            Self::SnapshotEntrySetMismatch(pc, expected_set, actual_set) => write!(
                formatter,
                "branch target buffer snapshot entry for PC {} is in set {actual_set}; expected {expected_set}",
                pc.get()
            ),
            Self::DuplicateSnapshotEntry(pc) => write!(
                formatter,
                "branch target buffer snapshot repeats entry for PC {}",
                pc.get()
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

impl BranchTargetKind {
    pub const COUNT: usize = 8;

    pub const ALL: [Self; Self::COUNT] = [
        Self::NoBranch,
        Self::Return,
        Self::CallDirect,
        Self::CallIndirect,
        Self::DirectConditional,
        Self::DirectUnconditional,
        Self::IndirectConditional,
        Self::IndirectUnconditional,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::NoBranch => 0,
            Self::Return => 1,
            Self::CallDirect => 2,
            Self::CallIndirect => 3,
            Self::DirectConditional => 4,
            Self::DirectUnconditional => 5,
            Self::IndirectConditional => 6,
            Self::IndirectUnconditional => 7,
        }
    }

    pub const fn canonical_stat_name(self) -> &'static str {
        match self {
            Self::NoBranch => "no_branch",
            Self::Return => "return",
            Self::CallDirect => "call_direct",
            Self::CallIndirect => "call_indirect",
            Self::DirectConditional => "direct_conditional",
            Self::DirectUnconditional => "direct_unconditional",
            Self::IndirectConditional => "indirect_conditional",
            Self::IndirectUnconditional => "indirect_unconditional",
        }
    }

    pub const fn gem5_branch_type_name(self) -> &'static str {
        match self {
            Self::NoBranch => "NoBranch",
            Self::Return => "Return",
            Self::CallDirect => "CallDirect",
            Self::CallIndirect => "CallIndirect",
            Self::DirectConditional => "DirectCond",
            Self::DirectUnconditional => "DirectUncond",
            Self::IndirectConditional => "IndirectCond",
            Self::IndirectUnconditional => "IndirectUncond",
        }
    }

    pub const fn is_indirect_non_return(self) -> bool {
        matches!(
            self,
            Self::CallIndirect | Self::IndirectConditional | Self::IndirectUnconditional
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchTargetKindCounts {
    values: [u64; BranchTargetKind::COUNT],
}

impl BranchTargetKindCounts {
    pub const fn value(self, kind: BranchTargetKind) -> u64 {
        self.values[kind.index()]
    }

    pub fn total(self) -> u64 {
        self.values
            .into_iter()
            .fold(0_u64, |total, value| total.saturating_add(value))
    }

    pub const fn values(self) -> [u64; BranchTargetKind::COUNT] {
        self.values
    }

    pub(crate) fn increment(&mut self, kind: BranchTargetKind) {
        let value = &mut self.values[kind.index()];
        *value = value.saturating_add(1);
    }

    pub(crate) fn set_for_checkpoint(&mut self, kind: BranchTargetKind, value: u64) {
        self.values[kind.index()] = value;
    }
}

impl Default for BranchTargetKindCounts {
    fn default() -> Self {
        Self {
            values: [0; BranchTargetKind::COUNT],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchTargetProvider {
    NoTarget,
    BTB,
    RAS,
    Indirect,
}

impl BranchTargetProvider {
    pub const COUNT: usize = 4;

    pub const ALL: [Self; Self::COUNT] = [Self::NoTarget, Self::BTB, Self::RAS, Self::Indirect];

    pub const fn index(self) -> usize {
        match self {
            Self::NoTarget => 0,
            Self::BTB => 1,
            Self::RAS => 2,
            Self::Indirect => 3,
        }
    }

    pub const fn canonical_stat_name(self) -> &'static str {
        match self {
            Self::NoTarget => "no_target",
            Self::BTB => "btb",
            Self::RAS => "ras",
            Self::Indirect => "indirect",
        }
    }

    pub const fn gem5_target_provider_name(self) -> &'static str {
        match self {
            Self::NoTarget => "NoTarget",
            Self::BTB => "BTB",
            Self::RAS => "RAS",
            Self::Indirect => "Indirect",
        }
    }

    pub const fn from_btb_prediction(
        predicted_taken: bool,
        prediction: BranchTargetPrediction,
    ) -> Self {
        if predicted_taken && prediction.hit() {
            Self::BTB
        } else {
            Self::NoTarget
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchTargetProviderCounts {
    values: [u64; BranchTargetProvider::COUNT],
}

impl BranchTargetProviderCounts {
    pub const fn value(self, provider: BranchTargetProvider) -> u64 {
        self.values[provider.index()]
    }

    pub fn total(self) -> u64 {
        self.values
            .into_iter()
            .fold(0_u64, |total, value| total.saturating_add(value))
    }

    pub const fn values(self) -> [u64; BranchTargetProvider::COUNT] {
        self.values
    }

    pub(crate) fn increment(&mut self, provider: BranchTargetProvider) {
        let value = &mut self.values[provider.index()];
        *value = value.saturating_add(1);
    }
}

impl Default for BranchTargetProviderCounts {
    fn default() -> Self {
        Self {
            values: [0; BranchTargetProvider::COUNT],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchTargetPrediction {
    hit: bool,
    target: Option<Address>,
}

impl BranchTargetPrediction {
    pub const fn new(hit: bool, target: Option<Address>) -> Self {
        Self { hit, target }
    }

    pub const fn hit(self) -> bool {
        self.hit
    }

    pub const fn target(self) -> Option<Address> {
        self.target
    }
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
    lookup_kind_counts: BranchTargetKindCounts,
    hit_kind_counts: BranchTargetKindCounts,
    miss_kind_counts: BranchTargetKindCounts,
    update_kind_counts: BranchTargetKindCounts,
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
            lookup_kind_counts: BranchTargetKindCounts::default(),
            hit_kind_counts: BranchTargetKindCounts::default(),
            miss_kind_counts: BranchTargetKindCounts::default(),
            update_kind_counts: BranchTargetKindCounts::default(),
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

    pub const fn lookup_kind_counts(&self) -> BranchTargetKindCounts {
        self.lookup_kind_counts
    }

    pub const fn hit_kind_counts(&self) -> BranchTargetKindCounts {
        self.hit_kind_counts
    }

    pub const fn miss_kind_counts(&self) -> BranchTargetKindCounts {
        self.miss_kind_counts
    }

    pub const fn update_kind_counts(&self) -> BranchTargetKindCounts {
        self.update_kind_counts
    }

    pub fn valid(&self, pc: Address) -> bool {
        self.find_entry(pc).is_some()
    }

    pub fn lookup(&mut self, pc: Address, kind: BranchTargetKind) -> BranchTargetLookup {
        self.lookup_count += 1;
        self.lookup_kind_counts.increment(kind);
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
                self.hit_kind_counts.increment(kind);
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
                self.miss_kind_counts.increment(kind);
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
        self.update_kind_counts.increment(kind);
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
            lookup_kind_counts: self.lookup_kind_counts,
            hit_kind_counts: self.hit_kind_counts,
            miss_kind_counts: self.miss_kind_counts,
            update_kind_counts: self.update_kind_counts,
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
        self.lookup_kind_counts = snapshot.lookup_kind_counts;
        self.hit_kind_counts = snapshot.hit_kind_counts;
        self.miss_kind_counts = snapshot.miss_kind_counts;
        self.update_kind_counts = snapshot.update_kind_counts;
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
    pub(crate) pc: Address,
    pub(crate) target: Address,
    pub(crate) kind: BranchTargetKind,
    pub(crate) set: usize,
    pub(crate) way: usize,
    pub(crate) last_used: u64,
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
    pub(crate) config: BranchTargetBufferConfig,
    pub(crate) entries: Vec<Option<BranchTargetEntry>>,
    pub(crate) access_sequence: u64,
    pub(crate) lookup_count: u64,
    pub(crate) hit_count: u64,
    pub(crate) miss_count: u64,
    pub(crate) update_count: u64,
    pub(crate) eviction_count: u64,
    pub(crate) lookup_kind_counts: BranchTargetKindCounts,
    pub(crate) hit_kind_counts: BranchTargetKindCounts,
    pub(crate) miss_kind_counts: BranchTargetKindCounts,
    pub(crate) update_kind_counts: BranchTargetKindCounts,
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

    pub const fn lookup_kind_counts(&self) -> BranchTargetKindCounts {
        self.lookup_kind_counts
    }

    pub const fn hit_kind_counts(&self) -> BranchTargetKindCounts {
        self.hit_kind_counts
    }

    pub const fn miss_kind_counts(&self) -> BranchTargetKindCounts {
        self.miss_kind_counts
    }

    pub const fn update_kind_counts(&self) -> BranchTargetKindCounts {
        self.update_kind_counts
    }
}
