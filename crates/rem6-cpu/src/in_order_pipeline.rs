use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

const CHECKPOINT_MAGIC: [u8; 4] = *b"RIOP";
const CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_HEADER_BYTES: usize = CHECKPOINT_MAGIC.len()
    + 1
    + U64_BYTES
    + InOrderPipelineStage::ALL.len() * U32_BYTES
    + U32_BYTES;
const CHECKPOINT_INSTRUCTION_BYTES: usize = U64_BYTES + 1;

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
}

impl InOrderPipelineInstruction {
    pub const fn new(sequence: u64, stage: InOrderPipelineStage) -> Self {
        Self { sequence, stage }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn stage(self) -> InOrderPipelineStage {
        self.stage
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
pub struct InOrderBranchRedirect {
    sequence: u64,
    resolved_stage: InOrderPipelineStage,
    target_pc: u64,
}

impl InOrderBranchRedirect {
    pub const fn new(sequence: u64, resolved_stage: InOrderPipelineStage, target_pc: u64) -> Self {
        Self {
            sequence,
            resolved_stage,
            target_pc,
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
                InOrderBranchRedirect::new(self.sequence, self.resolved_stage, target_pc)
            })
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderBranchPredictionRecord {
    prediction: InOrderBranchPrediction,
    repair_target_pc: Option<u64>,
    flushed: Vec<InOrderPipelineInstruction>,
}

impl InOrderBranchPredictionRecord {
    fn from_plan(prediction: InOrderBranchPrediction, plan: &InOrderPipelinePlan) -> Self {
        let repair_target_pc = plan.redirect().map(|redirect| redirect.target_pc());

        Self {
            prediction,
            repair_target_pc,
            flushed: plan.flushed().to_vec(),
        }
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

    pub fn flushed(&self) -> &[InOrderPipelineInstruction] {
        &self.flushed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineAdvance {
    instruction: InOrderPipelineInstruction,
    destination_stage: Option<InOrderPipelineStage>,
}

impl InOrderPipelineAdvance {
    fn new(instruction: InOrderPipelineInstruction) -> Self {
        Self {
            instruction,
            destination_stage: instruction.stage().next(),
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
        self.destination_stage.is_none()
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
        Ok(Self {
            advanced: Vec::new(),
            resource_blocked: canonical_in_flight(instructions)?,
            ordering_blocked: Vec::new(),
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

impl InOrderPipelineCheckpointPayload {
    pub fn from_state(state: &InOrderPipelineState) -> Self {
        Self {
            snapshot: state.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: InOrderPipelineSnapshot) -> Result<Self, InOrderPipelineError> {
        let state = InOrderPipelineState::restore(snapshot)?;
        Ok(Self {
            snapshot: state.snapshot(),
        })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, InOrderPipelineError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected: CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(InOrderPipelineError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = payload[offset];
        offset += 1;
        if version != CHECKPOINT_VERSION {
            return Err(InOrderPipelineError::UnsupportedCheckpointVersion { version });
        }

        let cycle = read_u64(payload, &mut offset);
        let mut widths = Vec::with_capacity(InOrderPipelineStage::ALL.len());
        for stage in InOrderPipelineStage::ALL {
            let slots = read_u32(payload, &mut offset) as usize;
            widths.push(InOrderPipelineStageWidth::new(stage, slots)?);
        }
        let config = InOrderPipelineConfig::new(widths)?;
        let instruction_count = read_u32(payload, &mut offset) as usize;
        let instruction_bytes = instruction_count
            .checked_mul(CHECKPOINT_INSTRUCTION_BYTES)
            .ok_or(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected: usize::MAX,
                actual: payload.len(),
            })?;
        let expected = CHECKPOINT_HEADER_BYTES
            .checked_add(instruction_bytes)
            .ok_or(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected: usize::MAX,
                actual: payload.len(),
            })?;
        if payload.len() != expected {
            return Err(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let mut in_flight = Vec::with_capacity(instruction_count);
        for _ in 0..instruction_count {
            let sequence = read_u64(payload, &mut offset);
            let stage = decode_checkpoint_stage(payload[offset])?;
            offset += 1;
            in_flight.push(InOrderPipelineInstruction::new(sequence, stage));
        }

        Self::from_snapshot(InOrderPipelineSnapshot::with_cycle(
            config, cycle, in_flight,
        ))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("in-order checkpoint payload values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, InOrderPipelineError> {
        let in_flight = self.snapshot.in_flight();
        let mut payload = Vec::with_capacity(
            CHECKPOINT_HEADER_BYTES + in_flight.len() * CHECKPOINT_INSTRUCTION_BYTES,
        );
        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(CHECKPOINT_VERSION);
        payload.extend_from_slice(&self.snapshot.cycle().to_le_bytes());
        for stage in InOrderPipelineStage::ALL {
            let width = encode_checkpoint_u32("stage width", self.snapshot.config().width(stage))?;
            payload.extend_from_slice(&width.to_le_bytes());
        }
        let in_flight_count =
            encode_checkpoint_u32("in-flight instruction count", in_flight.len())?;
        payload.extend_from_slice(&in_flight_count.to_le_bytes());
        for instruction in in_flight {
            payload.extend_from_slice(&instruction.sequence().to_le_bytes());
            payload.push(encode_checkpoint_stage(instruction.stage()));
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &InOrderPipelineSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> InOrderPipelineSnapshot {
        self.snapshot
    }
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
    state_changed: bool,
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

fn encode_checkpoint_stage(stage: InOrderPipelineStage) -> u8 {
    match stage {
        InOrderPipelineStage::Fetch1 => 0,
        InOrderPipelineStage::Fetch2 => 1,
        InOrderPipelineStage::Decode => 2,
        InOrderPipelineStage::Execute => 3,
        InOrderPipelineStage::Commit => 4,
    }
}

fn decode_checkpoint_stage(code: u8) -> Result<InOrderPipelineStage, InOrderPipelineError> {
    match code {
        0 => Ok(InOrderPipelineStage::Fetch1),
        1 => Ok(InOrderPipelineStage::Fetch2),
        2 => Ok(InOrderPipelineStage::Decode),
        3 => Ok(InOrderPipelineStage::Execute),
        4 => Ok(InOrderPipelineStage::Commit),
        _ => Err(InOrderPipelineError::InvalidCheckpointStageCode { code }),
    }
}

fn encode_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, InOrderPipelineError> {
    u32::try_from(value).map_err(|_| InOrderPipelineError::CheckpointValueTooLarge {
        field,
        value,
        maximum: CHECKPOINT_U32_MAX,
    })
}

fn read_u32(payload: &[u8], offset: &mut usize) -> u32 {
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    u32::from_le_bytes(bytes)
}

fn read_u64(payload: &[u8], offset: &mut usize) -> u64 {
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    u64::from_le_bytes(bytes)
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

    pub const fn state_changed(self) -> bool {
        self.state_changed
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
        let branch_prediction_flushed_count = self
            .branch_predictions
            .iter()
            .map(|prediction| prediction.flushed().len())
            .sum();

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
            state_changed: self.before.in_flight() != self.after.in_flight(),
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
            .map(|prediction| InOrderBranchPredictionRecord::from_plan(prediction, &plan))
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
        let plan =
            self.advance_cycle_with_redirect_and_retire_sequence(redirect, Some(sequence))?;
        let branch_predictions = prediction
            .map(|prediction| InOrderBranchPredictionRecord::from_plan(prediction, &plan))
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
        self.advance_cycle_with_redirect_and_retire_sequence(redirect, None)
    }

    fn advance_cycle_with_redirect_and_retire_sequence(
        &mut self,
        redirect: Option<InOrderBranchRedirect>,
        retire_sequence: Option<u64>,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError> {
        let next_cycle = next_cycle(self.cycle)?;
        let plan = InOrderPipelineScheduler::new(self.config.clone())
            .plan_with_redirect_and_retire_sequence(
                self.in_flight.iter().copied(),
                redirect,
                retire_sequence,
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
                next.push(InOrderPipelineInstruction::new(
                    advance.sequence(),
                    destination_stage,
                ));
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
        match self.plan_ready(instructions, None, None) {
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
        self.plan_ready(instructions, redirect, None)
    }

    pub(crate) fn plan_with_redirect_and_retire_sequence<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
        retire_sequence: Option<u64>,
    ) -> Result<InOrderPipelinePlan, InOrderPipelineError>
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        self.plan_ready(instructions, redirect, retire_sequence)
    }

    fn plan_ready<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
        retire_sequence: Option<u64>,
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
            if stage == InOrderPipelineStage::Commit
                && retire_sequence.is_some_and(|sequence| sequence != instruction.sequence())
            {
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
                advanced.push(InOrderPipelineAdvance::new(instruction));
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
        in_flight.push(instruction);
    }

    in_flight.sort_by_key(|instruction| instruction.sequence());
    Ok(in_flight)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InOrderPipelineError {
    ZeroStageWidth {
        stage: InOrderPipelineStage,
    },
    DuplicateStageWidth {
        stage: InOrderPipelineStage,
    },
    MissingStageWidth {
        stage: InOrderPipelineStage,
    },
    DuplicateInFlightInstruction {
        sequence: u64,
    },
    MissingBranchRedirectInstruction {
        sequence: u64,
    },
    BranchRedirectStageMismatch {
        sequence: u64,
        expected: InOrderPipelineStage,
        actual: InOrderPipelineStage,
    },
    MissingBranchPredictionInstruction {
        sequence: u64,
    },
    BranchPredictionStageMismatch {
        sequence: u64,
        expected: InOrderPipelineStage,
        actual: InOrderPipelineStage,
    },
    MissingBranchPredictionRepairTarget {
        sequence: u64,
    },
    CycleCursorOverflow {
        cycle: u64,
    },
    OverlappingRunSummaryMerge {
        left_first_cycle: u64,
        left_last_cycle: u64,
        right_first_cycle: u64,
        right_last_cycle: u64,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    InvalidCheckpointStageCode {
        code: u8,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for InOrderPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroStageWidth { stage } => {
                write!(formatter, "in-order {stage} width must be positive")
            }
            Self::DuplicateStageWidth { stage } => {
                write!(
                    formatter,
                    "in-order {stage} width is configured more than once"
                )
            }
            Self::MissingStageWidth { stage } => {
                write!(formatter, "in-order {stage} width is not configured")
            }
            Self::DuplicateInFlightInstruction { sequence } => write!(
                formatter,
                "in-order pipeline has duplicate in-flight instruction sequence {sequence}"
            ),
            Self::MissingBranchRedirectInstruction { sequence } => write!(
                formatter,
                "in-order branch redirect instruction sequence {sequence} is not in flight"
            ),
            Self::BranchRedirectStageMismatch {
                sequence,
                expected,
                actual,
            } => write!(
                formatter,
                "in-order branch redirect instruction sequence {sequence} resolved at {expected}, but in-flight stage is {actual}"
            ),
            Self::MissingBranchPredictionInstruction { sequence } => write!(
                formatter,
                "in-order branch prediction instruction sequence {sequence} is not in flight"
            ),
            Self::BranchPredictionStageMismatch {
                sequence,
                expected,
                actual,
            } => write!(
                formatter,
                "in-order branch prediction instruction sequence {sequence} resolved at {expected}, but in-flight stage is {actual}"
            ),
            Self::MissingBranchPredictionRepairTarget { sequence } => write!(
                formatter,
                "in-order branch prediction instruction sequence {sequence} needs a repair target PC"
            ),
            Self::CycleCursorOverflow { cycle } => {
                write!(formatter, "in-order pipeline cycle cursor {cycle} cannot advance")
            }
            Self::OverlappingRunSummaryMerge {
                left_first_cycle,
                left_last_cycle,
                right_first_cycle,
                right_last_cycle,
            } => write!(
                formatter,
                "in-order run summary windows overlap: left {left_first_cycle}..={left_last_cycle}, right {right_first_cycle}..={right_last_cycle}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "in-order checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "in-order checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "in-order checkpoint payload version {version} is not supported"
            ),
            Self::InvalidCheckpointStageCode { code } => {
                write!(formatter, "in-order checkpoint payload has invalid stage code {code}")
            }
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "in-order checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for InOrderPipelineError {}
