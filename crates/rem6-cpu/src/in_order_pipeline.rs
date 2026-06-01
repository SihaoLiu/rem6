use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

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
pub struct InOrderPipelineState {
    config: InOrderPipelineConfig,
    cycle: u64,
    in_flight: Vec<InOrderPipelineInstruction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InOrderPipelineCycleRecord {
    cycle: u64,
    before: InOrderPipelineSnapshot,
    plan: InOrderPipelinePlan,
    after: InOrderPipelineSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineCycleSummary {
    cycle: u64,
    advanced_count: usize,
    retired_count: usize,
    flushed_count: usize,
    resource_blocked_count: usize,
    ordering_blocked_count: usize,
    state_changed: bool,
    redirect_target_pc: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InOrderPipelineRunSummary {
    cycle_count: usize,
    first_cycle: Option<u64>,
    last_cycle: Option<u64>,
    advanced_count: usize,
    retired_count: usize,
    flushed_count: usize,
    resource_blocked_count: usize,
    ordering_blocked_count: usize,
    redirect_count: usize,
    state_changed_cycle_count: usize,
}

impl InOrderPipelineRunSummary {
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
        let mut summary = Self {
            cycle_count: 0,
            first_cycle: None,
            last_cycle: None,
            advanced_count: 0,
            retired_count: 0,
            flushed_count: 0,
            resource_blocked_count: 0,
            ordering_blocked_count: 0,
            redirect_count: 0,
            state_changed_cycle_count: 0,
        };

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
            summary.retired_count += cycle.retired_count();
            summary.flushed_count += cycle.flushed_count();
            summary.resource_blocked_count += cycle.resource_blocked_count();
            summary.ordering_blocked_count += cycle.ordering_blocked_count();
            if cycle.redirect_target_pc().is_some() {
                summary.redirect_count += 1;
            }
            if cycle.state_changed() {
                summary.state_changed_cycle_count += 1;
            }
        }

        summary
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

    pub const fn redirect_count(self) -> usize {
        self.redirect_count
    }

    pub const fn state_changed_cycle_count(self) -> usize {
        self.state_changed_cycle_count
    }
}

impl InOrderPipelineCycleSummary {
    pub const fn cycle(self) -> u64 {
        self.cycle
    }

    pub const fn advanced_count(self) -> usize {
        self.advanced_count
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

    pub const fn plan(&self) -> &InOrderPipelinePlan {
        &self.plan
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

        InOrderPipelineCycleSummary {
            cycle: self.cycle,
            advanced_count: self.plan.advanced().len(),
            retired_count,
            flushed_count: self.plan.flushed().len(),
            resource_blocked_count: self.plan.resource_blocked().len(),
            ordering_blocked_count: self.plan.ordering_blocked().len(),
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

    pub fn try_advance_cycle_recorded_with_redirect(
        &mut self,
        redirect: Option<InOrderBranchRedirect>,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        let plan = self.advance_cycle_with_redirect(redirect)?;
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            before,
            plan,
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
        let next_cycle = next_cycle(self.cycle)?;
        let plan = InOrderPipelineScheduler::new(self.config.clone())
            .plan_with_redirect(self.in_flight.iter().copied(), redirect)?;
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
        match self.plan_ready(instructions, None) {
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
        self.plan_ready(instructions, redirect)
    }

    fn plan_ready<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
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
    CycleCursorOverflow {
        cycle: u64,
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
            Self::CycleCursorOverflow { cycle } => {
                write!(formatter, "in-order pipeline cycle cursor {cycle} cannot advance")
            }
        }
    }
}

impl Error for InOrderPipelineError {}
