use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const CHECKPOINT_MAGIC: [u8; 4] = *b"RIOP";
const CHECKPOINT_VERSION_V1: u8 = 1;
const CHECKPOINT_VERSION_V2: u8 = 2;
const CHECKPOINT_VERSION_V3: u8 = 3;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize = CHECKPOINT_MAGIC.len()
    + 1
    + U64_BYTES
    + InOrderPipelineStage::ALL.len() * U32_BYTES
    + U32_BYTES;
const CHECKPOINT_V1_INSTRUCTION_BYTES: usize = U64_BYTES + 1;
const CHECKPOINT_V2_INSTRUCTION_BYTES: usize = U64_BYTES + 1 + 1 + U64_BYTES + U64_BYTES;
const CHECKPOINT_V3_INSTRUCTION_BYTES: usize = CHECKPOINT_V2_INSTRUCTION_BYTES + U64_BYTES;

mod checkpoint;
mod drive;
mod error;

use drive::InOrderPipelineRetirement;
pub use error::InOrderPipelineError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InOrderPipelineStage {
    Fetch1,
    Fetch2,
    Decode,
    Execute,
    Commit,
}

impl InOrderPipelineStage {
    pub const ALL: [Self; 5] = [
        Self::Fetch1,
        Self::Fetch2,
        Self::Decode,
        Self::Execute,
        Self::Commit,
    ];

    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Fetch1 => Some(Self::Fetch2),
            Self::Fetch2 => Some(Self::Decode),
            Self::Decode => Some(Self::Execute),
            Self::Execute => Some(Self::Commit),
            Self::Commit => None,
        }
    }
}

impl fmt::Display for InOrderPipelineStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch1 => write!(formatter, "fetch1"),
            Self::Fetch2 => write!(formatter, "fetch2"),
            Self::Decode => write!(formatter, "decode"),
            Self::Execute => write!(formatter, "execute"),
            Self::Commit => write!(formatter, "commit"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineStageWidth {
    stage: InOrderPipelineStage,
    slots: usize,
}

impl InOrderPipelineStageWidth {
    pub fn new(stage: InOrderPipelineStage, slots: usize) -> Result<Self, InOrderPipelineError> {
        if slots == 0 {
            return Err(InOrderPipelineError::ZeroStageWidth { stage });
        }

        Ok(Self { stage, slots })
    }

    pub const fn stage(self) -> InOrderPipelineStage {
        self.stage
    }

    pub const fn slots(self) -> usize {
        self.slots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineConfig {
    widths: BTreeMap<InOrderPipelineStage, usize>,
}

impl InOrderPipelineConfig {
    pub fn new<I>(widths: I) -> Result<Self, InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineStageWidth>,
    {
        let mut configured = BTreeMap::new();
        for width in widths {
            if configured.insert(width.stage(), width.slots()).is_some() {
                return Err(InOrderPipelineError::DuplicateStageWidth {
                    stage: width.stage(),
                });
            }
        }

        for stage in InOrderPipelineStage::ALL {
            if !configured.contains_key(&stage) {
                return Err(InOrderPipelineError::MissingStageWidth { stage });
            }
        }

        Ok(Self { widths: configured })
    }

    pub fn width(&self, stage: InOrderPipelineStage) -> usize {
        self.widths[&stage]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineInstruction {
    sequence: u64,
    stage: InOrderPipelineStage,
    execute_wait_cycles: Option<(u64, u64)>,
    execute_wait_key: Option<u64>,
}

impl InOrderPipelineInstruction {
    pub const fn new(sequence: u64, stage: InOrderPipelineStage) -> Self {
        Self {
            sequence,
            stage,
            execute_wait_cycles: None,
            execute_wait_key: None,
        }
    }

    pub const fn with_execute_wait(mut self, total_cycles: u64, remaining_cycles: u64) -> Self {
        self.execute_wait_cycles = Some((total_cycles, remaining_cycles));
        self.execute_wait_key = None;
        self
    }

    pub(crate) const fn with_execute_wait_key(
        mut self,
        total_cycles: u64,
        remaining_cycles: u64,
        key: u64,
    ) -> Self {
        self.execute_wait_cycles = Some((total_cycles, remaining_cycles));
        self.execute_wait_key = Some(key);
        self
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn stage(self) -> InOrderPipelineStage {
        self.stage
    }

    pub const fn execute_wait_total_cycles(self) -> Option<u64> {
        match self.execute_wait_cycles {
            Some((total_cycles, _)) => Some(total_cycles),
            None => None,
        }
    }

    pub const fn execute_wait_remaining_cycles(self) -> Option<u64> {
        match self.execute_wait_cycles {
            Some((_, remaining_cycles)) => Some(remaining_cycles),
            None => None,
        }
    }

    pub(crate) const fn execute_wait_key(self) -> Option<u64> {
        self.execute_wait_key
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InOrderPipelineStallCause {
    FetchWait,
    DataWait,
    ExecuteWait,
}

impl InOrderPipelineStallCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FetchWait => "fetch_wait",
            Self::DataWait => "data_wait",
            Self::ExecuteWait => "execute_wait",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InOrderPipelineRedirectCause {
    BranchPrediction,
    Interrupt,
    Trap,
}

impl InOrderPipelineRedirectCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BranchPrediction => "branch_prediction",
            Self::Interrupt => "interrupt_redirect",
            Self::Trap => "trap_redirect",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderBranchRedirect {
    sequence: u64,
    resolved_stage: InOrderPipelineStage,
    target_pc: u64,
    cause: InOrderPipelineRedirectCause,
}

impl InOrderBranchRedirect {
    pub const fn branch_prediction(
        sequence: u64,
        resolved_stage: InOrderPipelineStage,
        target_pc: u64,
    ) -> Self {
        Self::with_cause(
            sequence,
            resolved_stage,
            target_pc,
            InOrderPipelineRedirectCause::BranchPrediction,
        )
    }

    pub const fn trap(sequence: u64, resolved_stage: InOrderPipelineStage, target_pc: u64) -> Self {
        Self::with_cause(
            sequence,
            resolved_stage,
            target_pc,
            InOrderPipelineRedirectCause::Trap,
        )
    }

    pub const fn interrupt(
        sequence: u64,
        resolved_stage: InOrderPipelineStage,
        target_pc: u64,
    ) -> Self {
        Self::with_cause(
            sequence,
            resolved_stage,
            target_pc,
            InOrderPipelineRedirectCause::Interrupt,
        )
    }

    const fn with_cause(
        sequence: u64,
        resolved_stage: InOrderPipelineStage,
        target_pc: u64,
        cause: InOrderPipelineRedirectCause,
    ) -> Self {
        Self {
            sequence,
            resolved_stage,
            target_pc,
            cause,
        }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn resolved_stage(self) -> InOrderPipelineStage {
        self.resolved_stage
    }

    pub const fn target_pc(self) -> u64 {
        self.target_pc
    }

    pub const fn cause(self) -> InOrderPipelineRedirectCause {
        self.cause
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderBranchPrediction {
    sequence: u64,
    resolved_stage: InOrderPipelineStage,
    fetch_pc: u64,
    conditional: bool,
    predicted_taken: bool,
    predicted_target_pc: Option<u64>,
    resolved_taken: bool,
    resolved_target_pc: Option<u64>,
}

impl InOrderBranchPrediction {
    pub const fn new(
        sequence: u64,
        resolved_stage: InOrderPipelineStage,
        fetch_pc: u64,
        conditional: bool,
        predicted_taken: bool,
        predicted_target_pc: Option<u64>,
        resolved_taken: bool,
        resolved_target_pc: Option<u64>,
    ) -> Self {
        Self {
            sequence,
            resolved_stage,
            fetch_pc,
            conditional,
            predicted_taken,
            predicted_target_pc,
            resolved_taken,
            resolved_target_pc,
        }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn resolved_stage(self) -> InOrderPipelineStage {
        self.resolved_stage
    }

    pub const fn fetch_pc(self) -> u64 {
        self.fetch_pc
    }

    pub const fn is_conditional(self) -> bool {
        self.conditional
    }

    pub const fn predicted_taken(self) -> bool {
        self.predicted_taken
    }

    pub const fn predicted_target_pc(self) -> Option<u64> {
        self.predicted_target_pc
    }

    pub const fn resolved_taken(self) -> bool {
        self.resolved_taken
    }

    pub const fn resolved_target_pc(self) -> Option<u64> {
        self.resolved_target_pc
    }

    pub fn mispredicted(self) -> bool {
        if self.predicted_taken != self.resolved_taken {
            return true;
        }

        self.predicted_taken && self.predicted_target_pc != self.resolved_target_pc
    }

    fn repair_target_pc(self) -> Result<Option<u64>, InOrderPipelineError> {
        if !self.mispredicted() {
            return Ok(None);
        }

        self.resolved_target_pc
            .ok_or(InOrderPipelineError::MissingBranchPredictionRepairTarget {
                sequence: self.sequence,
            })
            .map(Some)
    }

    fn redirect(self) -> Result<Option<InOrderBranchRedirect>, InOrderPipelineError> {
        self.repair_target_pc().map(|target| {
            target.map(|target_pc| {
                InOrderBranchRedirect::branch_prediction(
                    self.sequence,
                    self.resolved_stage,
                    target_pc,
                )
            })
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderBranchPredictionRecord {
    prediction: InOrderBranchPrediction,
    repair_target_pc: Option<u64>,
}

impl InOrderBranchPredictionRecord {
    fn from_prediction(prediction: InOrderBranchPrediction) -> Result<Self, InOrderPipelineError> {
        Ok(Self {
            prediction,
            repair_target_pc: prediction.repair_target_pc()?,
        })
    }

    pub const fn sequence(&self) -> u64 {
        self.prediction.sequence()
    }

    pub const fn resolved_stage(&self) -> InOrderPipelineStage {
        self.prediction.resolved_stage()
    }

    pub const fn fetch_pc(&self) -> u64 {
        self.prediction.fetch_pc()
    }

    pub const fn is_conditional(&self) -> bool {
        self.prediction.is_conditional()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.prediction.predicted_taken()
    }

    pub const fn predicted_target_pc(&self) -> Option<u64> {
        self.prediction.predicted_target_pc()
    }

    pub const fn resolved_taken(&self) -> bool {
        self.prediction.resolved_taken()
    }

    pub const fn resolved_target_pc(&self) -> Option<u64> {
        self.prediction.resolved_target_pc()
    }

    pub fn mispredicted(&self) -> bool {
        self.prediction.mispredicted()
    }

    pub const fn repair_target_pc(&self) -> Option<u64> {
        self.repair_target_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineAdvance {
    instruction: InOrderPipelineInstruction,
    destination_stage: Option<InOrderPipelineStage>,
    retires: bool,
}

impl InOrderPipelineAdvance {
    fn new(instruction: InOrderPipelineInstruction, allows_retirement: bool) -> Self {
        let destination_stage = instruction.stage().next();
        Self {
            instruction,
            destination_stage,
            retires: allows_retirement && destination_stage.is_none(),
        }
    }

    pub const fn sequence(self) -> u64 {
        self.instruction.sequence()
    }

    pub const fn source_stage(self) -> InOrderPipelineStage {
        self.instruction.stage()
    }

    pub const fn destination_stage(self) -> Option<InOrderPipelineStage> {
        self.destination_stage
    }

    pub const fn retires(self) -> bool {
        self.retires
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelinePlan {
    advanced: Vec<InOrderPipelineAdvance>,
    resource_blocked: Vec<InOrderPipelineInstruction>,
    ordering_blocked: Vec<InOrderPipelineInstruction>,
    flushed: Vec<InOrderPipelineInstruction>,
    redirect: Option<InOrderBranchRedirect>,
}

impl InOrderPipelinePlan {
    fn resource_stall<I>(instructions: I) -> Result<Self, InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        let mut instructions = canonical_in_flight(instructions)?;
        let ordering_blocked = if instructions.is_empty() {
            Vec::new()
        } else {
            instructions.split_off(1)
        };
        Ok(Self {
            advanced: Vec::new(),
            resource_blocked: instructions,
            ordering_blocked,
            flushed: Vec::new(),
            redirect: None,
        })
    }

    pub fn advanced(&self) -> &[InOrderPipelineAdvance] {
        &self.advanced
    }

    pub fn resource_blocked(&self) -> &[InOrderPipelineInstruction] {
        &self.resource_blocked
    }

    pub fn ordering_blocked(&self) -> &[InOrderPipelineInstruction] {
        &self.ordering_blocked
    }

    pub fn flushed(&self) -> &[InOrderPipelineInstruction] {
        &self.flushed
    }

    pub fn redirect(&self) -> Option<&InOrderBranchRedirect> {
        self.redirect.as_ref()
    }

    pub fn redirect_cause(&self) -> Option<InOrderPipelineRedirectCause> {
        self.redirect().map(|redirect| redirect.cause())
    }

    pub fn flush_cause(&self) -> Option<InOrderPipelineRedirectCause> {
        if self.flushed.is_empty() {
            None
        } else {
            self.redirect_cause()
        }
    }

    pub fn flushed_for_cause(
        &self,
        cause: InOrderPipelineRedirectCause,
    ) -> &[InOrderPipelineInstruction] {
        if self.flush_cause() == Some(cause) {
            &self.flushed
        } else {
            &[]
        }
    }

    pub fn advanced_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.advanced.iter().map(|advance| advance.sequence())
    }

    pub fn flushed_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.flushed
            .iter()
            .map(|instruction| instruction.sequence())
    }

    pub fn has_blocked_work(&self) -> bool {
        !self.resource_blocked.is_empty() || !self.ordering_blocked.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineSnapshot {
    config: InOrderPipelineConfig,
    cycle: u64,
    in_flight: Vec<InOrderPipelineInstruction>,
}

impl InOrderPipelineSnapshot {
    pub fn new<I>(config: InOrderPipelineConfig, in_flight: I) -> Self
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        Self {
            config,
            cycle: 0,
            in_flight: in_flight.into_iter().collect(),
        }
    }

    pub fn with_cycle<I>(config: InOrderPipelineConfig, cycle: u64, in_flight: I) -> Self
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        Self {
            config,
            cycle,
            in_flight: in_flight.into_iter().collect(),
        }
    }

    pub const fn config(&self) -> &InOrderPipelineConfig {
        &self.config
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub fn in_flight(&self) -> &[InOrderPipelineInstruction] {
        &self.in_flight
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineCheckpointPayload {
    snapshot: InOrderPipelineSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineState {
    config: InOrderPipelineConfig,
    cycle: u64,
    in_flight: Vec<InOrderPipelineInstruction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineCycleRecord {
    cycle: u64,
    stall_cycle_count: u64,
    stall_cause: Option<InOrderPipelineStallCause>,
    before: InOrderPipelineSnapshot,
    plan: InOrderPipelinePlan,
    branch_predictions: Vec<InOrderBranchPredictionRecord>,
    after: InOrderPipelineSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineCycleSummary {
    cycle: u64,
    stall_cycle_count: u64,
    advanced_count: usize,
    retired_count: usize,
    flushed_count: usize,
    resource_blocked_count: usize,
    ordering_blocked_count: usize,
    branch_prediction_count: usize,
    correct_branch_prediction_count: usize,
    branch_misprediction_count: usize,
    conditional_branch_prediction_count: usize,
    conditional_branch_predicted_taken_count: usize,
    conditional_branch_misprediction_count: usize,
    branch_prediction_flushed_count: usize,
    branch_prediction_redirect_count: usize,
    interrupt_redirect_count: usize,
    interrupt_redirect_flushed_count: usize,
    interrupt_redirect_flush_cycle_count: usize,
    trap_redirect_count: usize,
    trap_redirect_flushed_count: usize,
    trap_redirect_flush_cycle_count: usize,
    state_changed: bool,
    redirect_cause: Option<InOrderPipelineRedirectCause>,
    flush_cause: Option<InOrderPipelineRedirectCause>,
    redirect_target_pc: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineRunSummary {
    cycle_count: usize,
    first_cycle: Option<u64>,
    last_cycle: Option<u64>,
    stall_cycle_count: u64,
    advanced_count: usize,
    retired_count: usize,
    flushed_count: usize,
    flush_cycle_count: usize,
    resource_blocked_count: usize,
    ordering_blocked_count: usize,
    branch_prediction_count: usize,
    correct_branch_prediction_count: usize,
    branch_misprediction_count: usize,
    conditional_branch_prediction_count: usize,
    conditional_branch_predicted_taken_count: usize,
    conditional_branch_misprediction_count: usize,
    branch_prediction_flushed_count: usize,
    branch_prediction_flush_cycle_count: usize,
    redirect_count: usize,
    branch_prediction_redirect_count: usize,
    interrupt_redirect_count: usize,
    interrupt_redirect_flushed_count: usize,
    interrupt_redirect_flush_cycle_count: usize,
    trap_redirect_count: usize,
    trap_redirect_flushed_count: usize,
    trap_redirect_flush_cycle_count: usize,
    state_changed_cycle_count: usize,
}

impl InOrderPipelineRunSummary {
    const EMPTY: Self = Self {
        cycle_count: 0,
        first_cycle: None,
        last_cycle: None,
        stall_cycle_count: 0,
        advanced_count: 0,
        retired_count: 0,
        flushed_count: 0,
        flush_cycle_count: 0,
        resource_blocked_count: 0,
        ordering_blocked_count: 0,
        branch_prediction_count: 0,
        correct_branch_prediction_count: 0,
        branch_misprediction_count: 0,
        conditional_branch_prediction_count: 0,
        conditional_branch_predicted_taken_count: 0,
        conditional_branch_misprediction_count: 0,
        branch_prediction_flushed_count: 0,
        branch_prediction_flush_cycle_count: 0,
        redirect_count: 0,
        branch_prediction_redirect_count: 0,
        interrupt_redirect_count: 0,
        interrupt_redirect_flushed_count: 0,
        interrupt_redirect_flush_cycle_count: 0,
        trap_redirect_count: 0,
        trap_redirect_flushed_count: 0,
        trap_redirect_flush_cycle_count: 0,
        state_changed_cycle_count: 0,
    };

    pub fn from_cycle_records<I>(records: I) -> Self
    where
        I: IntoIterator<Item = InOrderPipelineCycleRecord>,
    {
        Self::from_cycle_summaries(records.into_iter().map(|record| record.summary()))
    }

    pub fn from_cycle_summaries<I>(summaries: I) -> Self
    where
        I: IntoIterator<Item = InOrderPipelineCycleSummary>,
    {
        let mut summary = Self::EMPTY;

        for cycle in summaries {
            summary.cycle_count += 1;
            summary.first_cycle = Some(
                summary
                    .first_cycle
                    .map_or(cycle.cycle(), |first| first.min(cycle.cycle())),
            );
            summary.last_cycle = Some(
                summary
                    .last_cycle
                    .map_or(cycle.cycle(), |last| last.max(cycle.cycle())),
            );
            summary.advanced_count += cycle.advanced_count();
            summary.stall_cycle_count += cycle.stall_cycle_count();
            summary.retired_count += cycle.retired_count();
            summary.flushed_count += cycle.flushed_count();
            if cycle.flushed_count() > 0 {
                summary.flush_cycle_count += 1;
            }
            summary.resource_blocked_count += cycle.resource_blocked_count();
            summary.ordering_blocked_count += cycle.ordering_blocked_count();
            summary.branch_prediction_count += cycle.branch_prediction_count();
            summary.correct_branch_prediction_count += cycle.correct_branch_prediction_count();
            summary.branch_misprediction_count += cycle.branch_misprediction_count();
            summary.conditional_branch_prediction_count +=
                cycle.conditional_branch_prediction_count();
            summary.conditional_branch_predicted_taken_count +=
                cycle.conditional_branch_predicted_taken_count();
            summary.conditional_branch_misprediction_count +=
                cycle.conditional_branch_misprediction_count();
            summary.branch_prediction_flushed_count += cycle.branch_prediction_flushed_count();
            if cycle.branch_prediction_flushed_count() > 0 {
                summary.branch_prediction_flush_cycle_count += 1;
            }
            if cycle.redirect_target_pc().is_some() {
                summary.redirect_count += 1;
            }
            summary.branch_prediction_redirect_count += cycle.branch_prediction_redirect_count();
            summary.interrupt_redirect_count += cycle.interrupt_redirect_count();
            summary.interrupt_redirect_flushed_count += cycle.interrupt_redirect_flushed_count();
            summary.interrupt_redirect_flush_cycle_count +=
                cycle.interrupt_redirect_flush_cycle_count();
            summary.trap_redirect_count += cycle.trap_redirect_count();
            summary.trap_redirect_flushed_count += cycle.trap_redirect_flushed_count();
            summary.trap_redirect_flush_cycle_count += cycle.trap_redirect_flush_cycle_count();
            if cycle.state_changed() {
                summary.state_changed_cycle_count += 1;
            }
        }

        summary
    }

    pub fn merge_disjoint(self, other: Self) -> Result<Self, InOrderPipelineError> {
        validate_disjoint_run_summary_windows(self, other)?;

        Ok(Self {
            cycle_count: self.cycle_count + other.cycle_count,
            first_cycle: merge_min_cycle(self.first_cycle, other.first_cycle),
            last_cycle: merge_max_cycle(self.last_cycle, other.last_cycle),
            stall_cycle_count: self.stall_cycle_count + other.stall_cycle_count,
            advanced_count: self.advanced_count + other.advanced_count,
            retired_count: self.retired_count + other.retired_count,
            flushed_count: self.flushed_count + other.flushed_count,
            flush_cycle_count: self.flush_cycle_count + other.flush_cycle_count,
            resource_blocked_count: self.resource_blocked_count + other.resource_blocked_count,
            ordering_blocked_count: self.ordering_blocked_count + other.ordering_blocked_count,
            branch_prediction_count: self.branch_prediction_count + other.branch_prediction_count,
            correct_branch_prediction_count: self.correct_branch_prediction_count
                + other.correct_branch_prediction_count,
            branch_misprediction_count: self.branch_misprediction_count
                + other.branch_misprediction_count,
            conditional_branch_prediction_count: self.conditional_branch_prediction_count
                + other.conditional_branch_prediction_count,
            conditional_branch_predicted_taken_count: self.conditional_branch_predicted_taken_count
                + other.conditional_branch_predicted_taken_count,
            conditional_branch_misprediction_count: self.conditional_branch_misprediction_count
                + other.conditional_branch_misprediction_count,
            branch_prediction_flushed_count: self.branch_prediction_flushed_count
                + other.branch_prediction_flushed_count,
            branch_prediction_flush_cycle_count: self.branch_prediction_flush_cycle_count
                + other.branch_prediction_flush_cycle_count,
            redirect_count: self.redirect_count + other.redirect_count,
            branch_prediction_redirect_count: self.branch_prediction_redirect_count
                + other.branch_prediction_redirect_count,
            interrupt_redirect_count: self.interrupt_redirect_count
                + other.interrupt_redirect_count,
            interrupt_redirect_flushed_count: self.interrupt_redirect_flushed_count
                + other.interrupt_redirect_flushed_count,
            interrupt_redirect_flush_cycle_count: self.interrupt_redirect_flush_cycle_count
                + other.interrupt_redirect_flush_cycle_count,
            trap_redirect_count: self.trap_redirect_count + other.trap_redirect_count,
            trap_redirect_flushed_count: self.trap_redirect_flushed_count
                + other.trap_redirect_flushed_count,
            trap_redirect_flush_cycle_count: self.trap_redirect_flush_cycle_count
                + other.trap_redirect_flush_cycle_count,
            state_changed_cycle_count: self.state_changed_cycle_count
                + other.state_changed_cycle_count,
        })
    }

    pub const fn is_empty(self) -> bool {
        self.cycle_count == 0
    }

    pub const fn cycle_count(self) -> usize {
        self.cycle_count
    }

    pub const fn first_cycle(self) -> Option<u64> {
        self.first_cycle
    }

    pub const fn last_cycle(self) -> Option<u64> {
        self.last_cycle
    }

    pub const fn advanced_count(self) -> usize {
        self.advanced_count
    }

    pub const fn stall_cycle_count(self) -> u64 {
        self.stall_cycle_count
    }

    pub const fn retired_count(self) -> usize {
        self.retired_count
    }

    pub const fn flushed_count(self) -> usize {
        self.flushed_count
    }

    pub const fn flush_cycle_count(self) -> usize {
        self.flush_cycle_count
    }

    pub const fn resource_blocked_count(self) -> usize {
        self.resource_blocked_count
    }

    pub const fn ordering_blocked_count(self) -> usize {
        self.ordering_blocked_count
    }

    pub const fn branch_prediction_count(self) -> usize {
        self.branch_prediction_count
    }

    pub const fn correct_branch_prediction_count(self) -> usize {
        self.correct_branch_prediction_count
    }

    pub const fn branch_misprediction_count(self) -> usize {
        self.branch_misprediction_count
    }

    pub const fn conditional_branch_prediction_count(self) -> usize {
        self.conditional_branch_prediction_count
    }

    pub const fn conditional_branch_predicted_taken_count(self) -> usize {
        self.conditional_branch_predicted_taken_count
    }

    pub const fn conditional_branch_misprediction_count(self) -> usize {
        self.conditional_branch_misprediction_count
    }

    pub const fn branch_prediction_flushed_count(self) -> usize {
        self.branch_prediction_flushed_count
    }

    pub const fn branch_prediction_flush_cycle_count(self) -> usize {
        self.branch_prediction_flush_cycle_count
    }

    pub const fn redirect_count(self) -> usize {
        self.redirect_count
    }

    pub const fn branch_prediction_redirect_count(self) -> usize {
        self.branch_prediction_redirect_count
    }

    pub const fn interrupt_redirect_count(self) -> usize {
        self.interrupt_redirect_count
    }

    pub const fn interrupt_redirect_flushed_count(self) -> usize {
        self.interrupt_redirect_flushed_count
    }

    pub const fn interrupt_redirect_flush_cycle_count(self) -> usize {
        self.interrupt_redirect_flush_cycle_count
    }

    pub const fn trap_redirect_count(self) -> usize {
        self.trap_redirect_count
    }

    pub const fn trap_redirect_flushed_count(self) -> usize {
        self.trap_redirect_flushed_count
    }

    pub const fn trap_redirect_flush_cycle_count(self) -> usize {
        self.trap_redirect_flush_cycle_count
    }

    pub const fn state_changed_cycle_count(self) -> usize {
        self.state_changed_cycle_count
    }
}

fn merge_min_cycle(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(cycle), None) | (None, Some(cycle)) => Some(cycle),
        (None, None) => None,
    }
}

fn merge_max_cycle(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(cycle), None) | (None, Some(cycle)) => Some(cycle),
        (None, None) => None,
    }
}

fn validate_disjoint_run_summary_windows(
    left: InOrderPipelineRunSummary,
    right: InOrderPipelineRunSummary,
) -> Result<(), InOrderPipelineError> {
    let (Some(left_first), Some(left_last), Some(right_first), Some(right_last)) = (
        left.first_cycle(),
        left.last_cycle(),
        right.first_cycle(),
        right.last_cycle(),
    ) else {
        return Ok(());
    };

    if left_first <= right_last && right_first <= left_last {
        return Err(InOrderPipelineError::OverlappingRunSummaryMerge {
            left_first_cycle: left_first,
            left_last_cycle: left_last,
            right_first_cycle: right_first,
            right_last_cycle: right_last,
        });
    }

    Ok(())
}

impl InOrderPipelineCycleSummary {
    pub const fn cycle(self) -> u64 {
        self.cycle
    }

    pub const fn advanced_count(self) -> usize {
        self.advanced_count
    }

    pub const fn stall_cycle_count(self) -> u64 {
        self.stall_cycle_count
    }

    pub const fn retired_count(self) -> usize {
        self.retired_count
    }

    pub const fn flushed_count(self) -> usize {
        self.flushed_count
    }

    pub const fn resource_blocked_count(self) -> usize {
        self.resource_blocked_count
    }

    pub const fn ordering_blocked_count(self) -> usize {
        self.ordering_blocked_count
    }

    pub const fn branch_prediction_count(self) -> usize {
        self.branch_prediction_count
    }

    pub const fn correct_branch_prediction_count(self) -> usize {
        self.correct_branch_prediction_count
    }

    pub const fn branch_misprediction_count(self) -> usize {
        self.branch_misprediction_count
    }

    pub const fn conditional_branch_prediction_count(self) -> usize {
        self.conditional_branch_prediction_count
    }

    pub const fn conditional_branch_predicted_taken_count(self) -> usize {
        self.conditional_branch_predicted_taken_count
    }

    pub const fn conditional_branch_misprediction_count(self) -> usize {
        self.conditional_branch_misprediction_count
    }

    pub const fn branch_prediction_flushed_count(self) -> usize {
        self.branch_prediction_flushed_count
    }

    pub const fn branch_prediction_redirect_count(self) -> usize {
        self.branch_prediction_redirect_count
    }

    pub const fn interrupt_redirect_count(self) -> usize {
        self.interrupt_redirect_count
    }

    pub const fn interrupt_redirect_flushed_count(self) -> usize {
        self.interrupt_redirect_flushed_count
    }

    pub const fn interrupt_redirect_flush_cycle_count(self) -> usize {
        self.interrupt_redirect_flush_cycle_count
    }

    pub const fn trap_redirect_count(self) -> usize {
        self.trap_redirect_count
    }

    pub const fn trap_redirect_flushed_count(self) -> usize {
        self.trap_redirect_flushed_count
    }

    pub const fn trap_redirect_flush_cycle_count(self) -> usize {
        self.trap_redirect_flush_cycle_count
    }

    pub const fn state_changed(self) -> bool {
        self.state_changed
    }

    pub const fn redirect_cause(self) -> Option<InOrderPipelineRedirectCause> {
        self.redirect_cause
    }

    pub const fn flush_cause(self) -> Option<InOrderPipelineRedirectCause> {
        self.flush_cause
    }

    pub const fn redirect_target_pc(self) -> Option<u64> {
        self.redirect_target_pc
    }
}

impl InOrderPipelineCycleRecord {
    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub const fn before(&self) -> &InOrderPipelineSnapshot {
        &self.before
    }

    pub const fn stall_cycle_count(&self) -> u64 {
        self.stall_cycle_count
    }

    pub const fn stall_cause(&self) -> Option<InOrderPipelineStallCause> {
        self.stall_cause
    }

    pub(crate) fn set_stall_cause(&mut self, stall_cause: Option<InOrderPipelineStallCause>) {
        self.stall_cause = stall_cause;
    }

    pub const fn plan(&self) -> &InOrderPipelinePlan {
        &self.plan
    }

    pub(crate) fn completes_sequence(&self, sequence: u64) -> bool {
        self.plan.advanced().iter().any(|advance| {
            advance.sequence() == sequence
                && (advance.retires()
                    || self.plan.redirect().is_some_and(|redirect| {
                        redirect.cause() == InOrderPipelineRedirectCause::Interrupt
                            && redirect.sequence() == sequence
                    }))
        })
    }

    pub fn branch_predictions(&self) -> &[InOrderBranchPredictionRecord] {
        &self.branch_predictions
    }

    pub const fn after(&self) -> &InOrderPipelineSnapshot {
        &self.after
    }

    pub fn summary(&self) -> InOrderPipelineCycleSummary {
        let retired_count = self
            .plan
            .advanced()
            .iter()
            .filter(|advance| advance.retires())
            .count();
        let branch_prediction_count = self.branch_predictions.len();
        let branch_misprediction_count = self
            .branch_predictions
            .iter()
            .filter(|prediction| prediction.mispredicted())
            .count();
        let conditional_branch_prediction_count = self
            .branch_predictions
            .iter()
            .filter(|prediction| prediction.is_conditional())
            .count();
        let conditional_branch_predicted_taken_count = self
            .branch_predictions
            .iter()
            .filter(|prediction| prediction.is_conditional() && prediction.predicted_taken())
            .count();
        let conditional_branch_misprediction_count = self
            .branch_predictions
            .iter()
            .filter(|prediction| prediction.is_conditional() && prediction.mispredicted())
            .count();
        let redirect_cause = self.plan.redirect_cause();
        let flush_cause = self.plan.flush_cause();
        let branch_prediction_flushed_count = self
            .plan
            .flushed_for_cause(InOrderPipelineRedirectCause::BranchPrediction)
            .len();
        let branch_prediction_redirect_count =
            usize::from(redirect_cause == Some(InOrderPipelineRedirectCause::BranchPrediction));
        let interrupt_redirect_count =
            usize::from(redirect_cause == Some(InOrderPipelineRedirectCause::Interrupt));
        let interrupt_redirect_flushed_count = self
            .plan
            .flushed_for_cause(InOrderPipelineRedirectCause::Interrupt)
            .len();
        let interrupt_redirect_flush_cycle_count =
            usize::from(interrupt_redirect_flushed_count > 0);
        let trap_redirect_count =
            usize::from(redirect_cause == Some(InOrderPipelineRedirectCause::Trap));
        let trap_redirect_flushed_count = self
            .plan
            .flushed_for_cause(InOrderPipelineRedirectCause::Trap)
            .len();
        let trap_redirect_flush_cycle_count = usize::from(trap_redirect_flushed_count > 0);

        InOrderPipelineCycleSummary {
            cycle: self.cycle,
            stall_cycle_count: self.stall_cycle_count,
            advanced_count: self.plan.advanced().len(),
            retired_count,
            flushed_count: self.plan.flushed().len(),
            resource_blocked_count: self.plan.resource_blocked().len(),
            ordering_blocked_count: self.plan.ordering_blocked().len(),
            branch_prediction_count,
            correct_branch_prediction_count: branch_prediction_count - branch_misprediction_count,
            branch_misprediction_count,
            conditional_branch_prediction_count,
            conditional_branch_predicted_taken_count,
            conditional_branch_misprediction_count,
            branch_prediction_flushed_count,
            branch_prediction_redirect_count,
            interrupt_redirect_count,
            interrupt_redirect_flushed_count,
            interrupt_redirect_flush_cycle_count,
            trap_redirect_count,
            trap_redirect_flushed_count,
            trap_redirect_flush_cycle_count,
            state_changed: self.before.in_flight() != self.after.in_flight(),
            redirect_cause,
            flush_cause,
            redirect_target_pc: self.plan.redirect().map(|redirect| redirect.target_pc()),
        }
    }
}

impl InOrderPipelineState {
    pub const fn new(config: InOrderPipelineConfig) -> Self {
        Self {
            config,
            cycle: 0,
            in_flight: Vec::new(),
        }
    }

    pub fn restore(snapshot: InOrderPipelineSnapshot) -> Result<Self, InOrderPipelineError> {
        Ok(Self {
            config: snapshot.config,
            cycle: snapshot.cycle,
            in_flight: canonical_in_flight(snapshot.in_flight)?,
        })
    }

    pub const fn config(&self) -> &InOrderPipelineConfig {
        &self.config
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub fn in_flight(&self) -> &[InOrderPipelineInstruction] {
        &self.in_flight
    }

    pub fn replace_in_flight<I>(&mut self, instructions: I) -> Result<(), InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        self.in_flight = canonical_in_flight(instructions)?;
        Ok(())
    }

    pub fn contains_sequence(&self, sequence: u64) -> bool {
        self.in_flight
            .iter()
            .any(|instruction| instruction.sequence() == sequence)
    }

    pub fn enqueue_fetch(&mut self, sequence: u64) -> Result<(), InOrderPipelineError> {
        self.enqueue_fetch_recorded(sequence).map(|_| ())
    }

    pub fn enqueue_fetch_recorded(
        &mut self,
        sequence: u64,
    ) -> Result<Option<InOrderPipelineCycleRecord>, InOrderPipelineError> {
        if self.contains_sequence(sequence) {
            return Ok(None);
        }
        let fetch1_occupancy = self
            .in_flight
            .iter()
            .filter(|instruction| instruction.stage() == InOrderPipelineStage::Fetch1)
            .count();
        let fetch1_has_slot = fetch1_occupancy < self.config.width(InOrderPipelineStage::Fetch1);
        let commit_busy = self
            .in_flight
            .iter()
            .any(|instruction| instruction.stage() == InOrderPipelineStage::Commit);
        let record = if !self.in_flight.is_empty() && !fetch1_has_slot && !commit_busy {
            Some(self.try_advance_cycle_recorded()?)
        } else {
            None
        };
        self.in_flight.push(InOrderPipelineInstruction::new(
            sequence,
            InOrderPipelineStage::Fetch1,
        ));
        self.in_flight = canonical_in_flight(self.in_flight.iter().copied())?;
        Ok(record)
    }

    pub fn plan_cycle(&self) -> InOrderPipelinePlan {
        InOrderPipelineScheduler::new(self.config.clone()).plan(self.in_flight.iter().copied())
    }

    pub fn advance_cycle(&mut self) -> InOrderPipelinePlan {
        self.try_advance_cycle()
            .expect("in-order pipeline cycle advance failed")
    }

    pub fn advance_cycle_recorded(&mut self) -> InOrderPipelineCycleRecord {
        self.try_advance_cycle_recorded()
            .expect("in-order pipeline recorded cycle advance failed")
    }

    pub fn try_advance_cycle_recorded(
        &mut self,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        self.try_advance_cycle_recorded_with_redirect(None)
    }

    pub fn try_record_resource_stall_cycle(
        &mut self,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        self.try_record_resource_stall_cycle_with_optional_cause(None)
    }

    pub fn try_record_resource_stall_cycle_with_cause(
        &mut self,
        cause: InOrderPipelineStallCause,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        self.try_record_resource_stall_cycle_with_optional_cause(Some(cause))
    }

    fn try_record_resource_stall_cycle_with_optional_cause(
        &mut self,
        stall_cause: Option<InOrderPipelineStallCause>,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        let plan = InOrderPipelinePlan::resource_stall(self.in_flight.iter().copied())?;
        self.cycle = next_cycle(self.cycle)?;
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            stall_cycle_count: 1,
            stall_cause,
            before,
            plan,
            branch_predictions: Vec::new(),
            after,
        })
    }

    pub fn try_advance_cycle_recorded_with_redirect(
        &mut self,
        redirect: Option<InOrderBranchRedirect>,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        let plan = self.advance_cycle_with_redirect(redirect)?;
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            stall_cycle_count: 0,
            stall_cause: None,
            before,
            plan,
            branch_predictions: Vec::new(),
            after,
        })
    }

    pub fn try_advance_cycle_recorded_with_prediction(
        &mut self,
        prediction: Option<InOrderBranchPrediction>,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        validate_branch_prediction(prediction, before.in_flight())?;
        let redirect = prediction
            .map(|prediction| prediction.redirect())
            .transpose()?
            .flatten();
        let plan = self.advance_cycle_with_redirect(redirect)?;
        let branch_predictions = prediction
            .map(InOrderBranchPredictionRecord::from_prediction)
            .transpose()?
            .into_iter()
            .collect();
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            stall_cycle_count: 0,
            stall_cause: None,
            before,
            plan,
            branch_predictions,
            after,
        })
    }

    pub(crate) fn try_advance_cycle_recorded_retiring_sequence(
        &mut self,
        sequence: u64,
        prediction: Option<InOrderBranchPrediction>,
        redirect: Option<InOrderBranchRedirect>,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        validate_branch_prediction(prediction, before.in_flight())?;
        let redirect = prediction
            .map(|prediction| prediction.redirect())
            .transpose()?
            .flatten()
            .or(redirect);
        let plan = self.advance_cycle_with_redirect_and_retirement(
            redirect,
            InOrderPipelineRetirement::Sequence(sequence),
        )?;
        let branch_predictions = prediction
            .map(InOrderBranchPredictionRecord::from_prediction)
            .transpose()?
            .into_iter()
            .collect();
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            stall_cycle_count: 0,
            stall_cause: None,
            before,
            plan,
            branch_predictions,
            after,
        })
    }

    pub fn try_advance_cycle(&mut self) -> Result<InOrderPipelinePlan, InOrderPipelineError> {
        self.advance_cycle_with_redirect(None)
    }

    pub fn advance_cycle_with_redirect(
        &mut self,
        redirect: Option<InOrderBranchRedirect>,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError> {
        self.advance_cycle_with_redirect_and_retirement(redirect, InOrderPipelineRetirement::Any)
    }

    fn advance_cycle_with_redirect_and_retirement(
        &mut self,
        redirect: Option<InOrderBranchRedirect>,
        retirement: InOrderPipelineRetirement,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError> {
        let next_cycle = next_cycle(self.cycle)?;
        let plan = InOrderPipelineScheduler::new(self.config.clone())
            .plan_with_redirect_and_retirement(
                self.in_flight.iter().copied(),
                redirect,
                retirement,
            )?;
        self.apply_plan(&plan);
        self.cycle = next_cycle;
        Ok(plan)
    }

    pub fn snapshot(&self) -> InOrderPipelineSnapshot {
        InOrderPipelineSnapshot {
            config: self.config.clone(),
            cycle: self.cycle,
            in_flight: self.in_flight.clone(),
        }
    }

    fn apply_plan(&mut self, plan: &InOrderPipelinePlan) {
        let mut next = Vec::new();

        for advance in plan.advanced() {
            if let Some(destination_stage) = advance.destination_stage() {
                next.push(InOrderPipelineInstruction {
                    stage: destination_stage,
                    ..advance.instruction
                });
            }
        }

        next.extend(plan.resource_blocked().iter().copied());
        next.extend(plan.ordering_blocked().iter().copied());
        self.in_flight = canonical_in_flight(next)
            .expect("cycle plan cannot create duplicate in-flight instruction sequences");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineScheduler {
    config: InOrderPipelineConfig,
}

impl InOrderPipelineScheduler {
    pub const fn new(config: InOrderPipelineConfig) -> Self {
        Self { config }
    }

    pub const fn config(&self) -> &InOrderPipelineConfig {
        &self.config
    }

    pub fn plan<I>(&self, instructions: I) -> InOrderPipelinePlan
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        match self.plan_ready(instructions, None, InOrderPipelineRetirement::Any) {
            Ok(plan) => plan,
            Err(error) => unreachable!("planning without a redirect cannot fail: {error}"),
        }
    }

    pub fn plan_with_redirect<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        self.plan_ready(instructions, redirect, InOrderPipelineRetirement::Any)
    }

    fn plan_with_redirect_and_retirement<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
        retirement: InOrderPipelineRetirement,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        self.plan_ready(instructions, redirect, retirement)
    }

    fn plan_ready<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
        retirement: InOrderPipelineRetirement,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        let mut ready = instructions.into_iter().collect::<Vec<_>>();
        ready.sort_by_key(|instruction| instruction.sequence());
        validate_redirect(redirect, &ready)?;

        let mut used_slots = BTreeMap::new();
        let mut advanced = Vec::new();
        let mut resource_blocked = Vec::new();
        let mut ordering_blocked = Vec::new();
        let mut flushed = Vec::new();
        let mut older_blocked = false;

        for instruction in ready {
            if redirect.is_some_and(|redirect| instruction.sequence() > redirect.sequence()) {
                flushed.push(instruction);
                continue;
            }

            if older_blocked {
                ordering_blocked.push(instruction);
                continue;
            }

            let stage = instruction.stage();
            if stage == InOrderPipelineStage::Commit && !retirement.allows(instruction.sequence()) {
                resource_blocked.push(instruction);
                older_blocked = true;
                continue;
            }

            let used = used_slots.entry(stage).or_insert(0);
            if *used >= self.config.width(stage) {
                resource_blocked.push(instruction);
                older_blocked = true;
            } else {
                *used += 1;
                let allows_retirement = !redirect.is_some_and(|redirect| {
                    redirect.cause() == InOrderPipelineRedirectCause::Interrupt
                        && redirect.sequence() == instruction.sequence()
                });
                advanced.push(InOrderPipelineAdvance::new(instruction, allows_retirement));
            }
        }

        Ok(InOrderPipelinePlan {
            advanced,
            resource_blocked,
            ordering_blocked,
            flushed,
            redirect,
        })
    }
}

fn validate_redirect(
    redirect: Option<InOrderBranchRedirect>,
    ready: &[InOrderPipelineInstruction],
) -> Result<(), InOrderPipelineError> {
    let Some(redirect) = redirect else {
        return Ok(());
    };

    let Some(instruction) = ready
        .iter()
        .find(|instruction| instruction.sequence() == redirect.sequence())
    else {
        return Err(InOrderPipelineError::MissingBranchRedirectInstruction {
            sequence: redirect.sequence(),
        });
    };

    if instruction.stage() != redirect.resolved_stage() {
        return Err(InOrderPipelineError::BranchRedirectStageMismatch {
            sequence: redirect.sequence(),
            expected: redirect.resolved_stage(),
            actual: instruction.stage(),
        });
    }

    Ok(())
}

fn validate_branch_prediction(
    prediction: Option<InOrderBranchPrediction>,
    ready: &[InOrderPipelineInstruction],
) -> Result<(), InOrderPipelineError> {
    let Some(prediction) = prediction else {
        return Ok(());
    };

    let Some(instruction) = ready
        .iter()
        .find(|instruction| instruction.sequence() == prediction.sequence())
    else {
        return Err(InOrderPipelineError::MissingBranchPredictionInstruction {
            sequence: prediction.sequence(),
        });
    };

    if instruction.stage() != prediction.resolved_stage() {
        return Err(InOrderPipelineError::BranchPredictionStageMismatch {
            sequence: prediction.sequence(),
            expected: prediction.resolved_stage(),
            actual: instruction.stage(),
        });
    }

    Ok(())
}

fn next_cycle(cycle: u64) -> Result<u64, InOrderPipelineError> {
    cycle
        .checked_add(1)
        .ok_or(InOrderPipelineError::CycleCursorOverflow { cycle })
}

fn canonical_in_flight<I>(
    instructions: I,
) -> Result<Vec<InOrderPipelineInstruction>, InOrderPipelineError>
where
    I: IntoIterator<Item = InOrderPipelineInstruction>,
{
    let mut seen = BTreeSet::new();
    let mut in_flight = Vec::new();

    for instruction in instructions {
        if !seen.insert(instruction.sequence()) {
            return Err(InOrderPipelineError::DuplicateInFlightInstruction {
                sequence: instruction.sequence(),
            });
        }
        if let Some((total_cycles, remaining_cycles)) = instruction.execute_wait_cycles {
            let valid_cycles = total_cycles > 0 && remaining_cycles <= total_cycles;
            let valid_stage = instruction.stage() == InOrderPipelineStage::Execute
                || (instruction.stage() == InOrderPipelineStage::Commit && remaining_cycles == 0);
            if !valid_cycles || !valid_stage {
                return Err(InOrderPipelineError::ExecuteWaitStageMismatch {
                    sequence: instruction.sequence(),
                    stage: instruction.stage(),
                    total_cycles,
                    remaining_cycles,
                });
            }
        }
        in_flight.push(instruction);
    }

    in_flight.sort_by_key(|instruction| instruction.sequence());
    Ok(in_flight)
}
