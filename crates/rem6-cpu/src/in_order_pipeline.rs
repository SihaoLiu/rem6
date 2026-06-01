use std::collections::BTreeMap;
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
        self.plan_with_redirect(instructions, None)
    }

    pub fn plan_with_redirect<I>(
        &self,
        instructions: I,
        redirect: Option<InOrderBranchRedirect>,
    ) -> InOrderPipelinePlan
    where
        I: IntoIterator<Item = InOrderPipelineInstruction>,
    {
        let mut ready = instructions.into_iter().collect::<Vec<_>>();
        ready.sort_by_key(|instruction| instruction.sequence());

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

        InOrderPipelinePlan {
            advanced,
            resource_blocked,
            ordering_blocked,
            flushed,
            redirect,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InOrderPipelineError {
    ZeroStageWidth { stage: InOrderPipelineStage },
    DuplicateStageWidth { stage: InOrderPipelineStage },
    MissingStageWidth { stage: InOrderPipelineStage },
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
        }
    }
}

impl Error for InOrderPipelineError {}
