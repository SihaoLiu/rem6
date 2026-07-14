use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

const O3_WRITEBACK_CHECKPOINT_MAGIC: [u8; 4] = *b"O3WB";
const O3_WRITEBACK_CHECKPOINT_VERSION: u8 = 1;
const O3_PENDING_STATE_CHECKPOINT_MAGIC: [u8; 4] = *b"O3PS";
const O3_PENDING_STATE_CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const O3_WRITEBACK_CHECKPOINT_HEADER_BYTES: usize =
    O3_WRITEBACK_CHECKPOINT_MAGIC.len() + 1 + 1 + U32_BYTES + U64_BYTES + U32_BYTES;
const O3_WRITEBACK_CHECKPOINT_COMPLETION_BYTES: usize = U64_BYTES;
const O3_PENDING_STATE_CHECKPOINT_HEADER_BYTES: usize =
    O3_PENDING_STATE_CHECKPOINT_MAGIC.len() + 1 + U32_BYTES + U32_BYTES + U32_BYTES;
const O3_PENDING_READY_INSTRUCTION_BYTES: usize = U64_BYTES + U32_BYTES + 1 + U32_BYTES + U32_BYTES;
const O3_WRITEBACK_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3PipelineStage {
    Fetch,
    Decode,
    Rename,
    Iew,
    Commit,
}

impl fmt::Display for O3PipelineStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch => write!(formatter, "fetch"),
            Self::Decode => write!(formatter, "decode"),
            Self::Rename => write!(formatter, "rename"),
            Self::Iew => write!(formatter, "IEW"),
            Self::Commit => write!(formatter, "commit"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct O3IssueQueueId(u32);

impl O3IssueQueueId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3IssueOpClass {
    IntAlu,
    IntMult,
    Float,
    Memory,
    Branch,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3IssueQueueCapacity {
    queue: O3IssueQueueId,
    op_class: O3IssueOpClass,
    slots: usize,
}

impl O3IssueQueueCapacity {
    pub fn new(
        queue: O3IssueQueueId,
        op_class: O3IssueOpClass,
        slots: usize,
    ) -> Result<Self, O3PipelineError> {
        if slots == 0 {
            return Err(O3PipelineError::ZeroIssueQueueCapacity { queue, op_class });
        }
        Ok(Self {
            queue,
            op_class,
            slots,
        })
    }

    pub const fn queue(self) -> O3IssueQueueId {
        self.queue
    }

    pub const fn op_class(self) -> O3IssueOpClass {
        self.op_class
    }

    pub const fn slots(self) -> usize {
        self.slots
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct O3DependencyScopeId(u64);

impl O3DependencyScopeId {
    const VECTOR_REDUCTION_PARTIAL_KIND: u64 = 1;
    const VECTOR_REDUCTION_RESULT_KIND: u64 = 2;
    const VECTOR_REDUCTION_KIND_SHIFT: u32 = 62;
    const VECTOR_REDUCTION_INDEX_BITS: u32 = 20;
    const VECTOR_REDUCTION_INDEX_MASK: u64 = (1 << Self::VECTOR_REDUCTION_INDEX_BITS) - 1;
    const VECTOR_REDUCTION_GROUP_MASK: u64 =
        (1 << (Self::VECTOR_REDUCTION_KIND_SHIFT - Self::VECTOR_REDUCTION_INDEX_BITS)) - 1;

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn vector_reduction_partial(
        group: O3VectorReductionGroupId,
        index: u32,
    ) -> Result<Self, O3PipelineError> {
        Self::vector_reduction_scope(Self::VECTOR_REDUCTION_PARTIAL_KIND, group, index)
    }

    fn vector_reduction_result(group: O3VectorReductionGroupId) -> Result<Self, O3PipelineError> {
        Self::vector_reduction_scope(Self::VECTOR_REDUCTION_RESULT_KIND, group, 0)
    }

    fn vector_reduction_scope(
        kind: u64,
        group: O3VectorReductionGroupId,
        index: u32,
    ) -> Result<Self, O3PipelineError> {
        if group.get() > Self::VECTOR_REDUCTION_GROUP_MASK
            || u64::from(index) > Self::VECTOR_REDUCTION_INDEX_MASK
        {
            return Err(O3PipelineError::DependencyScopeEncodingOverflow {
                group: group.get(),
                index,
            });
        }

        Ok(Self(
            (kind << Self::VECTOR_REDUCTION_KIND_SHIFT)
                | (group.get() << Self::VECTOR_REDUCTION_INDEX_BITS)
                | u64::from(index),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3ReadyInstruction {
    sequence: u64,
    queue: O3IssueQueueId,
    op_class: O3IssueOpClass,
}

impl O3ReadyInstruction {
    pub const fn new(sequence: u64, queue: O3IssueQueueId, op_class: O3IssueOpClass) -> Self {
        Self {
            sequence,
            queue,
            op_class,
        }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn queue(self) -> O3IssueQueueId {
        self.queue
    }

    pub const fn op_class(self) -> O3IssueOpClass {
        self.op_class
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3ScopedReadyInstruction {
    instruction: O3ReadyInstruction,
    waits_on: Vec<O3DependencyScopeId>,
    produces: Vec<O3DependencyScopeId>,
}

impl O3ScopedReadyInstruction {
    pub fn new(sequence: u64, queue: O3IssueQueueId, op_class: O3IssueOpClass) -> Self {
        Self {
            instruction: O3ReadyInstruction::new(sequence, queue, op_class),
            waits_on: Vec::new(),
            produces: Vec::new(),
        }
    }

    pub fn with_waits_on<I>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = O3DependencyScopeId>,
    {
        self.waits_on = canonical_scopes(scopes);
        self
    }

    pub fn with_produces<I>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = O3DependencyScopeId>,
    {
        self.produces = canonical_scopes(scopes);
        self
    }

    pub const fn sequence(&self) -> u64 {
        self.instruction.sequence()
    }

    pub const fn queue(&self) -> O3IssueQueueId {
        self.instruction.queue()
    }

    pub const fn op_class(&self) -> O3IssueOpClass {
        self.instruction.op_class()
    }

    pub fn waits_on(&self) -> &[O3DependencyScopeId] {
        &self.waits_on
    }

    pub fn produces(&self) -> &[O3DependencyScopeId] {
        &self.produces
    }

    pub const fn ready_instruction(&self) -> O3ReadyInstruction {
        self.instruction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3DistributedIssuePlan {
    issue_width: usize,
    issued: Vec<O3ReadyInstruction>,
    blocked: Vec<O3ReadyInstruction>,
}

impl O3DistributedIssuePlan {
    pub const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub fn issued(&self) -> &[O3ReadyInstruction] {
        &self.issued
    }

    pub fn blocked(&self) -> &[O3ReadyInstruction] {
        &self.blocked
    }

    pub fn issued_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.issued.iter().map(|instruction| instruction.sequence())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3ScopedIssuePlan {
    issue_width: usize,
    issued: Vec<O3ScopedReadyInstruction>,
    resource_blocked: Vec<O3ScopedReadyInstruction>,
    dependency_blocked: Vec<O3ScopedReadyInstruction>,
}

impl O3ScopedIssuePlan {
    pub const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub fn issued(&self) -> &[O3ScopedReadyInstruction] {
        &self.issued
    }

    pub fn resource_blocked(&self) -> &[O3ScopedReadyInstruction] {
        &self.resource_blocked
    }

    pub fn dependency_blocked(&self) -> &[O3ScopedReadyInstruction] {
        &self.dependency_blocked
    }

    pub fn issued_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.issued.iter().map(|instruction| instruction.sequence())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3DistributedIssueScheduler {
    issue_width: usize,
    capacities: BTreeMap<(O3IssueQueueId, O3IssueOpClass), usize>,
}

impl O3DistributedIssueScheduler {
    pub fn new<I>(issue_width: usize, capacities: I) -> Result<Self, O3PipelineError>
    where
        I: IntoIterator<Item = O3IssueQueueCapacity>,
    {
        if issue_width == 0 {
            return Err(O3PipelineError::ZeroIssueWidth);
        }

        Ok(Self {
            issue_width,
            capacities: capacities
                .into_iter()
                .map(|capacity| ((capacity.queue(), capacity.op_class()), capacity.slots()))
                .collect(),
        })
    }

    pub const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub fn queue_capacity(&self, queue: O3IssueQueueId, op_class: O3IssueOpClass) -> usize {
        self.capacities
            .get(&(queue, op_class))
            .copied()
            .unwrap_or_default()
    }

    pub fn plan<I>(&self, ready: I) -> O3DistributedIssuePlan
    where
        I: IntoIterator<Item = O3ReadyInstruction>,
    {
        let mut remaining_capacity = self.capacities.clone();
        let mut pending = ready.into_iter().collect::<Vec<_>>();
        pending.sort_by_key(|instruction| instruction.sequence());

        let mut issued = Vec::new();
        while issued.len() < self.issue_width {
            let Some(index) = pending
                .iter()
                .position(|instruction| issue_slots(&remaining_capacity, instruction) != 0)
            else {
                break;
            };
            let instruction = pending.remove(index);
            if let Some(slots) =
                remaining_capacity.get_mut(&(instruction.queue(), instruction.op_class()))
            {
                *slots -= 1;
            }
            issued.push(instruction);
        }

        let blocked = pending
            .into_iter()
            .filter(|instruction| issue_slots(&remaining_capacity, instruction) == 0)
            .collect();

        O3DistributedIssuePlan {
            issue_width: self.issue_width,
            issued,
            blocked,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3ScopedIssueScheduler {
    issue_width: usize,
    capacities: BTreeMap<(O3IssueQueueId, O3IssueOpClass), usize>,
}

impl O3ScopedIssueScheduler {
    pub fn new<I>(issue_width: usize, capacities: I) -> Result<Self, O3PipelineError>
    where
        I: IntoIterator<Item = O3IssueQueueCapacity>,
    {
        if issue_width == 0 {
            return Err(O3PipelineError::ZeroIssueWidth);
        }

        Ok(Self {
            issue_width,
            capacities: capacities
                .into_iter()
                .map(|capacity| ((capacity.queue(), capacity.op_class()), capacity.slots()))
                .collect(),
        })
    }

    pub const fn issue_width(&self) -> usize {
        self.issue_width
    }

    pub fn plan<R, I>(&self, resolved_scopes: R, ready: I) -> O3ScopedIssuePlan
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        self.try_plan(resolved_scopes, ready)
            .expect("scoped issue plan must not contain duplicate dependency producers")
    }

    pub fn try_plan<R, I>(
        &self,
        resolved_scopes: R,
        ready: I,
    ) -> Result<O3ScopedIssuePlan, O3PipelineError>
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        let resolved_scopes = resolved_scopes.into_iter().collect::<BTreeSet<_>>();
        let mut pending = ready.into_iter().collect::<Vec<_>>();
        pending.sort_by_key(|instruction| instruction.sequence());
        validate_unique_dependency_producers(&pending)?;

        let mut remaining_capacity = self.capacities.clone();
        let mut issued = Vec::new();
        while issued.len() < self.issue_width {
            let Some(index) = pending.iter().position(|instruction| {
                dependency_ready(&resolved_scopes, instruction)
                    && scoped_issue_slots(&remaining_capacity, instruction) != 0
            }) else {
                break;
            };

            let instruction = pending.remove(index);
            if let Some(slots) =
                remaining_capacity.get_mut(&(instruction.queue(), instruction.op_class()))
            {
                *slots -= 1;
            }
            issued.push(instruction);
        }

        let mut resource_blocked = Vec::new();
        let mut dependency_blocked = Vec::new();
        for instruction in pending {
            if dependency_ready(&resolved_scopes, &instruction) {
                resource_blocked.push(instruction);
            } else {
                dependency_blocked.push(instruction);
            }
        }

        Ok(O3ScopedIssuePlan {
            issue_width: self.issue_width,
            issued,
            resource_blocked,
            dependency_blocked,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct O3VectorReductionGroupId(u64);

impl O3VectorReductionGroupId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3VectorReductionOrdering {
    Ordered,
    Unordered,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3VectorReductionMicroOp {
    sequence: u64,
    waits_on: Vec<O3DependencyScopeId>,
    produces: Vec<O3DependencyScopeId>,
    requires_serialize_after: bool,
}

impl O3VectorReductionMicroOp {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn waits_on(&self) -> &[O3DependencyScopeId] {
        &self.waits_on
    }

    pub fn produces(&self) -> &[O3DependencyScopeId] {
        &self.produces
    }

    pub const fn requires_serialize_after(&self) -> bool {
        self.requires_serialize_after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3VectorReductionDependencyPlan {
    group: O3VectorReductionGroupId,
    ordering: O3VectorReductionOrdering,
    partial_count: usize,
    architectural_result_scope: O3DependencyScopeId,
    micro_ops: Vec<O3VectorReductionMicroOp>,
}

impl O3VectorReductionDependencyPlan {
    pub fn new(
        group: O3VectorReductionGroupId,
        first_sequence: u64,
        partial_count: usize,
        ordering: O3VectorReductionOrdering,
    ) -> Result<Self, O3PipelineError> {
        if partial_count == 0 {
            return Err(O3PipelineError::ZeroVectorReductionMicroOps { group });
        }

        let architectural_result_scope = O3DependencyScopeId::vector_reduction_result(group)?;
        let mut partial_scopes = Vec::with_capacity(partial_count);
        for index in 0..partial_count {
            let index = u32::try_from(index).map_err(|_| {
                O3PipelineError::DependencyScopeEncodingOverflow {
                    group: group.get(),
                    index: u32::MAX,
                }
            })?;
            partial_scopes.push(O3DependencyScopeId::vector_reduction_partial(group, index)?);
        }

        let mut micro_ops = Vec::with_capacity(partial_count + 1);
        for index in 0..partial_count {
            let sequence = first_sequence.checked_add(index as u64).ok_or(
                O3PipelineError::VectorReductionSequenceOverflow {
                    first_sequence,
                    micro_ops: partial_count + 1,
                },
            )?;
            let waits_on = match ordering {
                O3VectorReductionOrdering::Unordered => Vec::new(),
                O3VectorReductionOrdering::Ordered if index == 0 => Vec::new(),
                O3VectorReductionOrdering::Ordered => vec![partial_scopes[index - 1]],
            };
            micro_ops.push(O3VectorReductionMicroOp {
                sequence,
                waits_on,
                produces: vec![partial_scopes[index]],
                requires_serialize_after: false,
            });
        }

        let publish_sequence = first_sequence.checked_add(partial_count as u64).ok_or(
            O3PipelineError::VectorReductionSequenceOverflow {
                first_sequence,
                micro_ops: partial_count + 1,
            },
        )?;
        let publish_waits = match ordering {
            O3VectorReductionOrdering::Unordered => partial_scopes,
            O3VectorReductionOrdering::Ordered => {
                vec![*partial_scopes
                    .last()
                    .expect("vector reduction partial scopes are nonempty")]
            }
        };
        micro_ops.push(O3VectorReductionMicroOp {
            sequence: publish_sequence,
            waits_on: publish_waits,
            produces: vec![architectural_result_scope],
            requires_serialize_after: false,
        });

        Ok(Self {
            group,
            ordering,
            partial_count,
            architectural_result_scope,
            micro_ops,
        })
    }

    pub const fn group(&self) -> O3VectorReductionGroupId {
        self.group
    }

    pub const fn ordering(&self) -> O3VectorReductionOrdering {
        self.ordering
    }

    pub fn micro_ops(&self) -> &[O3VectorReductionMicroOp] {
        &self.micro_ops
    }

    pub fn partial_micro_ops(&self) -> &[O3VectorReductionMicroOp] {
        &self.micro_ops[..self.partial_count]
    }

    pub fn publish_micro_op(&self) -> &O3VectorReductionMicroOp {
        &self.micro_ops[self.partial_count]
    }

    pub const fn architectural_result_scope(&self) -> O3DependencyScopeId {
        self.architectural_result_scope
    }
}

fn issue_slots(
    capacities: &BTreeMap<(O3IssueQueueId, O3IssueOpClass), usize>,
    instruction: &O3ReadyInstruction,
) -> usize {
    capacities
        .get(&(instruction.queue(), instruction.op_class()))
        .copied()
        .unwrap_or_default()
}

fn scoped_issue_slots(
    capacities: &BTreeMap<(O3IssueQueueId, O3IssueOpClass), usize>,
    instruction: &O3ScopedReadyInstruction,
) -> usize {
    capacities
        .get(&(instruction.queue(), instruction.op_class()))
        .copied()
        .unwrap_or_default()
}

fn dependency_ready(
    resolved_scopes: &BTreeSet<O3DependencyScopeId>,
    instruction: &O3ScopedReadyInstruction,
) -> bool {
    instruction
        .waits_on()
        .iter()
        .all(|scope| resolved_scopes.contains(scope))
}

fn validate_unique_dependency_producers(
    ready: &[O3ScopedReadyInstruction],
) -> Result<(), O3PipelineError> {
    let mut producers = BTreeMap::new();
    for instruction in ready {
        for scope in instruction.produces() {
            if producers.insert(*scope, instruction.sequence()).is_some() {
                return Err(O3PipelineError::DuplicateDependencyProducer { scope: *scope });
            }
        }
    }
    Ok(())
}

fn canonical_scopes<I>(scopes: I) -> Vec<O3DependencyScopeId>
where
    I: IntoIterator<Item = O3DependencyScopeId>,
{
    scopes
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum O3PipelineError {
    ZeroDownstreamWidth {
        downstream: O3PipelineStage,
    },
    ZeroWritebackWidth {
        source: O3PipelineStage,
    },
    ZeroIssueWidth,
    ZeroIssueQueueCapacity {
        queue: O3IssueQueueId,
        op_class: O3IssueOpClass,
    },
    EarlyThresholdOverflow {
        downstream: O3PipelineStage,
        backward_signal_delay_cycles: u64,
        downstream_width: usize,
    },
    WritebackWindowOverflow {
        source: O3PipelineStage,
        writeback_width: usize,
        future_cycles: u64,
    },
    DuplicateWritebackOccupiedSlot {
        source: O3PipelineStage,
        slot: usize,
    },
    WritebackOccupiedSlotOutOfRange {
        source: O3PipelineStage,
        slot: usize,
        writeback_width: usize,
    },
    DuplicateDependencyProducer {
        scope: O3DependencyScopeId,
    },
    DependencyScopeEncodingOverflow {
        group: u64,
        index: u32,
    },
    ZeroVectorReductionMicroOps {
        group: O3VectorReductionGroupId,
    },
    VectorReductionSequenceOverflow {
        first_sequence: u64,
        micro_ops: usize,
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
    InvalidCheckpointOpClassCode {
        code: u8,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for O3PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDownstreamWidth { downstream } => {
                write!(formatter, "O3 {downstream} downstream width must be positive")
            }
            Self::ZeroWritebackWidth { source } => {
                write!(formatter, "O3 {source} writeback width must be positive")
            }
            Self::ZeroIssueWidth => write!(formatter, "O3 issue width must be positive"),
            Self::ZeroIssueQueueCapacity { queue, op_class } => write!(
                formatter,
                "O3 issue queue {} capacity for {op_class:?} must be positive",
                queue.get()
            ),
            Self::EarlyThresholdOverflow {
                downstream,
                backward_signal_delay_cycles,
                downstream_width,
            } => write!(
                formatter,
                "O3 {downstream} unblock threshold overflows for delay {backward_signal_delay_cycles} and width {downstream_width}"
            ),
            Self::WritebackWindowOverflow {
                source,
                writeback_width,
                future_cycles,
            } => write!(
                formatter,
                "O3 {source} writeback window overflows for width {writeback_width} and future {future_cycles}"
            ),
            Self::DuplicateWritebackOccupiedSlot { source, slot } => write!(
                formatter,
                "O3 {source} writeback occupied slot {slot} appears more than once"
            ),
            Self::WritebackOccupiedSlotOutOfRange {
                source,
                slot,
                writeback_width,
            } => write!(
                formatter,
                "O3 {source} writeback occupied slot {slot} is out of range for width {writeback_width}"
            ),
            Self::DuplicateDependencyProducer { scope } => write!(
                formatter,
                "O3 dependency scope {} has more than one producer",
                scope.get()
            ),
            Self::DependencyScopeEncodingOverflow { group, index } => write!(
                formatter,
                "O3 dependency scope cannot encode vector reduction group {group} index {index}"
            ),
            Self::ZeroVectorReductionMicroOps { group } => write!(
                formatter,
                "O3 vector reduction group {} must have at least one micro-op",
                group.get()
            ),
            Self::VectorReductionSequenceOverflow {
                first_sequence,
                micro_ops,
            } => write!(
                formatter,
                "O3 vector reduction sequence overflows from first sequence {first_sequence} across {micro_ops} micro-ops"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "O3 checkpoint payload has {actual} bytes but expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "O3 checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "O3 checkpoint payload version {version} is not supported"
            ),
            Self::InvalidCheckpointStageCode { code } => {
                write!(formatter, "O3 checkpoint payload has invalid stage code {code}")
            }
            Self::InvalidCheckpointOpClassCode { code } => {
                write!(formatter, "O3 checkpoint payload has invalid op-class code {code}")
            }
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "O3 checkpoint field {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for O3PipelineError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3UnblockDecisionReason {
    SkidBufferAboveEarlyThreshold,
    SignalDelayCovered,
    SkidBufferEmpty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3UnblockDecision {
    should_signal_unblock: bool,
    reason: O3UnblockDecisionReason,
    skid_entries: usize,
    early_unblock_threshold_entries: usize,
    cycles_to_drain: u64,
}

impl O3UnblockDecision {
    pub const fn should_signal_unblock(self) -> bool {
        self.should_signal_unblock
    }

    pub const fn reason(self) -> O3UnblockDecisionReason {
        self.reason
    }

    pub const fn skid_entries(self) -> usize {
        self.skid_entries
    }

    pub const fn early_unblock_threshold_entries(self) -> usize {
        self.early_unblock_threshold_entries
    }

    pub const fn cycles_to_drain(self) -> u64 {
        self.cycles_to_drain
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3UnblockPolicy {
    upstream: O3PipelineStage,
    downstream: O3PipelineStage,
    backward_signal_delay_cycles: u64,
    downstream_width: usize,
    early_unblock_threshold_entries: usize,
}

impl O3UnblockPolicy {
    pub fn new(
        upstream: O3PipelineStage,
        downstream: O3PipelineStage,
        backward_signal_delay_cycles: u64,
        downstream_width: usize,
    ) -> Result<Self, O3PipelineError> {
        if downstream_width == 0 {
            return Err(O3PipelineError::ZeroDownstreamWidth { downstream });
        }
        let threshold = (backward_signal_delay_cycles as u128) * (downstream_width as u128);
        if threshold > (usize::MAX as u128) {
            return Err(O3PipelineError::EarlyThresholdOverflow {
                downstream,
                backward_signal_delay_cycles,
                downstream_width,
            });
        }
        Ok(Self {
            upstream,
            downstream,
            backward_signal_delay_cycles,
            downstream_width,
            early_unblock_threshold_entries: threshold as usize,
        })
    }

    pub const fn upstream(&self) -> O3PipelineStage {
        self.upstream
    }

    pub const fn downstream(&self) -> O3PipelineStage {
        self.downstream
    }

    pub const fn backward_signal_delay_cycles(&self) -> u64 {
        self.backward_signal_delay_cycles
    }

    pub const fn downstream_width(&self) -> usize {
        self.downstream_width
    }

    pub const fn early_unblock_threshold_entries(&self) -> usize {
        self.early_unblock_threshold_entries
    }

    pub const fn empty_only_would_signal(&self, skid_entries: usize) -> bool {
        skid_entries == 0
    }

    pub fn decision(&self, skid_entries: usize) -> O3UnblockDecision {
        let cycles_to_drain = cycles_to_drain(skid_entries, self.downstream_width);
        let (should_signal_unblock, reason) = if skid_entries == 0 {
            (true, O3UnblockDecisionReason::SkidBufferEmpty)
        } else if skid_entries <= self.early_unblock_threshold_entries {
            (true, O3UnblockDecisionReason::SignalDelayCovered)
        } else {
            (
                false,
                O3UnblockDecisionReason::SkidBufferAboveEarlyThreshold,
            )
        };

        O3UnblockDecision {
            should_signal_unblock,
            reason,
            skid_entries,
            early_unblock_threshold_entries: self.early_unblock_threshold_entries,
            cycles_to_drain,
        }
    }
}

fn cycles_to_drain(skid_entries: usize, downstream_width: usize) -> u64 {
    if skid_entries == 0 {
        return 0;
    }
    ((skid_entries - 1) / downstream_width + 1) as u64
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3WritebackAdmission {
    ready_index: usize,
    cycle_offset: u64,
    slot: usize,
}

impl O3WritebackAdmission {
    pub const fn ready_index(self) -> usize {
        self.ready_index
    }

    pub const fn cycle_offset(self) -> u64 {
        self.cycle_offset
    }

    pub const fn slot(self) -> usize {
        self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3WritebackTransferPlan {
    ready_count: usize,
    admitted_count: usize,
    deferred_count: usize,
    admissions: Vec<O3WritebackAdmission>,
}

impl O3WritebackTransferPlan {
    pub const fn ready_count(&self) -> usize {
        self.ready_count
    }

    pub const fn admitted_count(&self) -> usize {
        self.admitted_count
    }

    pub const fn deferred_count(&self) -> usize {
        self.deferred_count
    }

    pub const fn has_deferred(&self) -> bool {
        self.deferred_count != 0
    }

    pub fn admissions(&self) -> &[O3WritebackAdmission] {
        &self.admissions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3WritebackCompletion {
    sequence: u64,
}

impl O3WritebackCompletion {
    pub const fn new(sequence: u64) -> Self {
        Self { sequence }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3WritebackCompletionAdmission {
    completion: O3WritebackCompletion,
    cycle_offset: u64,
    slot: usize,
}

impl O3WritebackCompletionAdmission {
    pub const fn completion(self) -> O3WritebackCompletion {
        self.completion
    }

    pub const fn cycle_offset(self) -> u64 {
        self.cycle_offset
    }

    pub const fn slot(self) -> usize {
        self.slot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3WritebackTransferCycle {
    new_ready_count: usize,
    deferred_before_count: usize,
    admissions: Vec<O3WritebackCompletionAdmission>,
    deferred: Vec<O3WritebackCompletion>,
}

impl O3WritebackTransferCycle {
    pub const fn new_ready_count(&self) -> usize {
        self.new_ready_count
    }

    pub const fn deferred_before_count(&self) -> usize {
        self.deferred_before_count
    }

    pub fn admissions(&self) -> &[O3WritebackCompletionAdmission] {
        &self.admissions
    }

    pub fn admitted_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.admissions
            .iter()
            .map(|admission| admission.completion.sequence())
    }

    pub fn deferred(&self) -> &[O3WritebackCompletion] {
        &self.deferred
    }

    pub fn deferred_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.deferred.iter().map(|completion| completion.sequence())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3WritebackTransferSnapshot {
    policy: O3WritebackTransferPolicy,
    deferred: Vec<O3WritebackCompletion>,
}

impl O3WritebackTransferSnapshot {
    pub fn new<I>(policy: O3WritebackTransferPolicy, deferred: I) -> Self
    where
        I: IntoIterator<Item = O3WritebackCompletion>,
    {
        Self {
            policy,
            deferred: deferred.into_iter().collect(),
        }
    }

    pub const fn policy(&self) -> &O3WritebackTransferPolicy {
        &self.policy
    }

    pub fn deferred(&self) -> &[O3WritebackCompletion] {
        &self.deferred
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3WritebackTransferCheckpointPayload {
    snapshot: O3WritebackTransferSnapshot,
}

impl O3WritebackTransferCheckpointPayload {
    pub fn from_buffer(buffer: &O3WritebackTransferBuffer) -> Result<Self, O3PipelineError> {
        Self::from_snapshot(buffer.snapshot())
    }

    pub fn from_snapshot(snapshot: O3WritebackTransferSnapshot) -> Result<Self, O3PipelineError> {
        encode_checkpoint_u32("writeback_width", snapshot.policy.writeback_width())?;
        encode_checkpoint_u32("deferred_count", snapshot.deferred.len())?;
        checkpoint_payload_size(
            snapshot.deferred.len(),
            O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
        )?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, O3PipelineError> {
        if payload.len() < O3_WRITEBACK_CHECKPOINT_HEADER_BYTES {
            return Err(O3PipelineError::InvalidCheckpointPayloadSize {
                expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[..O3_WRITEBACK_CHECKPOINT_MAGIC.len()] != O3_WRITEBACK_CHECKPOINT_MAGIC {
            return Err(O3PipelineError::InvalidCheckpointMagic);
        }

        let version = payload[O3_WRITEBACK_CHECKPOINT_MAGIC.len()];
        if version != O3_WRITEBACK_CHECKPOINT_VERSION {
            return Err(O3PipelineError::UnsupportedCheckpointVersion { version });
        }

        let mut offset = O3_WRITEBACK_CHECKPOINT_MAGIC.len() + 1;
        let source = decode_checkpoint_stage(payload[offset])?;
        offset += 1;
        let writeback_width = read_u32(payload, &mut offset) as usize;
        let future_cycles = read_u64(payload, &mut offset);
        let deferred_count = read_u32(payload, &mut offset) as usize;
        let expected = checkpoint_payload_size(deferred_count, payload.len())?;
        if payload.len() != expected {
            return Err(O3PipelineError::InvalidCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let policy = O3WritebackTransferPolicy::new(source, writeback_width, future_cycles)?;
        let mut deferred = Vec::with_capacity(deferred_count);
        for _ in 0..deferred_count {
            deferred.push(O3WritebackCompletion::new(read_u64(payload, &mut offset)));
        }

        Self::from_snapshot(O3WritebackTransferSnapshot::new(policy, deferred))
    }

    pub fn encode(&self) -> Vec<u8> {
        let deferred_count = encode_checkpoint_u32("deferred_count", self.snapshot.deferred.len())
            .expect("checkpoint payload was validated before construction");
        let writeback_width =
            encode_checkpoint_u32("writeback_width", self.snapshot.policy.writeback_width())
                .expect("checkpoint payload was validated before construction");
        let capacity = checkpoint_payload_size(
            self.snapshot.deferred.len(),
            O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
        )
        .expect("checkpoint payload was validated before construction");
        let mut payload = Vec::with_capacity(capacity);
        payload.extend_from_slice(&O3_WRITEBACK_CHECKPOINT_MAGIC);
        payload.push(O3_WRITEBACK_CHECKPOINT_VERSION);
        payload.push(encode_checkpoint_stage(self.snapshot.policy.source()));
        payload.extend_from_slice(&writeback_width.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.policy.future_cycles().to_le_bytes());
        payload.extend_from_slice(&deferred_count.to_le_bytes());
        for completion in &self.snapshot.deferred {
            payload.extend_from_slice(&completion.sequence().to_le_bytes());
        }
        payload
    }

    pub const fn snapshot(&self) -> &O3WritebackTransferSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> O3WritebackTransferSnapshot {
        self.snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3PendingStateSnapshot {
    resolved_dependency_scopes: Vec<O3DependencyScopeId>,
    ready: Vec<O3ScopedReadyInstruction>,
    writeback: O3WritebackTransferSnapshot,
}

impl O3PendingStateSnapshot {
    pub fn new<R, I>(
        resolved_dependency_scopes: R,
        ready: I,
        writeback: O3WritebackTransferSnapshot,
    ) -> Result<Self, O3PipelineError>
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        let resolved_dependency_scopes = canonical_scopes(resolved_dependency_scopes);
        let mut ready = ready.into_iter().collect::<Vec<_>>();
        ready.sort_by_key(|instruction| {
            (
                instruction.sequence(),
                instruction.queue().get(),
                encode_checkpoint_op_class(instruction.op_class()),
            )
        });
        validate_unique_dependency_producers(&ready)?;

        let snapshot = Self {
            resolved_dependency_scopes,
            ready,
            writeback,
        };
        validate_pending_state_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn resolved_dependency_scopes(&self) -> &[O3DependencyScopeId] {
        &self.resolved_dependency_scopes
    }

    pub fn ready(&self) -> &[O3ScopedReadyInstruction] {
        &self.ready
    }

    pub const fn writeback(&self) -> &O3WritebackTransferSnapshot {
        &self.writeback
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3PendingStateCheckpointPayload {
    snapshot: O3PendingStateSnapshot,
}

impl O3PendingStateCheckpointPayload {
    pub fn from_snapshot(snapshot: O3PendingStateSnapshot) -> Result<Self, O3PipelineError> {
        validate_pending_state_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, O3PipelineError> {
        if payload.len() < O3_PENDING_STATE_CHECKPOINT_HEADER_BYTES {
            return Err(O3PipelineError::InvalidCheckpointPayloadSize {
                expected: O3_PENDING_STATE_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[..O3_PENDING_STATE_CHECKPOINT_MAGIC.len()] != O3_PENDING_STATE_CHECKPOINT_MAGIC {
            return Err(O3PipelineError::InvalidCheckpointMagic);
        }

        let version = payload[O3_PENDING_STATE_CHECKPOINT_MAGIC.len()];
        if version != O3_PENDING_STATE_CHECKPOINT_VERSION {
            return Err(O3PipelineError::UnsupportedCheckpointVersion { version });
        }

        let mut offset = O3_PENDING_STATE_CHECKPOINT_MAGIC.len() + 1;
        let writeback_payload_len = read_checkpoint_u32(payload, &mut offset)? as usize;
        let resolved_count = read_checkpoint_u32(payload, &mut offset)? as usize;
        let ready_count = read_checkpoint_u32(payload, &mut offset)? as usize;

        ensure_checkpoint_remaining(payload, offset, writeback_payload_len)?;
        let writeback_end = offset + writeback_payload_len;
        let writeback =
            O3WritebackTransferCheckpointPayload::decode(&payload[offset..writeback_end])?
                .into_snapshot();
        offset = writeback_end;

        let resolved_bytes = checkpoint_field_bytes(resolved_count, U64_BYTES, payload.len())?;
        ensure_checkpoint_remaining(payload, offset, resolved_bytes)?;
        let mut resolved_dependency_scopes = Vec::with_capacity(resolved_count);
        for _ in 0..resolved_count {
            resolved_dependency_scopes.push(O3DependencyScopeId::new(read_checkpoint_u64(
                payload,
                &mut offset,
            )?));
        }

        let minimum_ready_bytes = checkpoint_field_bytes(
            ready_count,
            O3_PENDING_READY_INSTRUCTION_BYTES,
            payload.len(),
        )?;
        ensure_checkpoint_remaining(payload, offset, minimum_ready_bytes)?;
        let mut ready = Vec::with_capacity(ready_count);
        for _ in 0..ready_count {
            let sequence = read_checkpoint_u64(payload, &mut offset)?;
            let queue = O3IssueQueueId::new(read_checkpoint_u32(payload, &mut offset)?);
            let op_class = decode_checkpoint_op_class(read_checkpoint_u8(payload, &mut offset)?)?;
            let waits_on_count = read_checkpoint_u32(payload, &mut offset)? as usize;
            let produces_count = read_checkpoint_u32(payload, &mut offset)? as usize;
            let waits_on = read_checkpoint_scopes(payload, &mut offset, waits_on_count)?;
            let produces = read_checkpoint_scopes(payload, &mut offset, produces_count)?;

            ready.push(
                O3ScopedReadyInstruction::new(sequence, queue, op_class)
                    .with_waits_on(waits_on)
                    .with_produces(produces),
            );
        }

        if offset != payload.len() {
            return Err(O3PipelineError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshot(O3PendingStateSnapshot::new(
            resolved_dependency_scopes,
            ready,
            writeback,
        )?)
    }

    pub fn encode(&self) -> Vec<u8> {
        let writeback_payload =
            O3WritebackTransferCheckpointPayload::from_snapshot(self.snapshot.writeback.clone())
                .expect("pending-state checkpoint payload was validated before construction")
                .encode();
        let writeback_payload_len =
            encode_checkpoint_u32("writeback_payload_length", writeback_payload.len())
                .expect("pending-state checkpoint payload was validated before construction");
        let resolved_count = encode_checkpoint_u32(
            "resolved_dependency_scope_count",
            self.snapshot.resolved_dependency_scopes.len(),
        )
        .expect("pending-state checkpoint payload was validated before construction");
        let ready_count =
            encode_checkpoint_u32("ready_instruction_count", self.snapshot.ready.len())
                .expect("pending-state checkpoint payload was validated before construction");

        let mut payload = Vec::new();
        payload.extend_from_slice(&O3_PENDING_STATE_CHECKPOINT_MAGIC);
        payload.push(O3_PENDING_STATE_CHECKPOINT_VERSION);
        payload.extend_from_slice(&writeback_payload_len.to_le_bytes());
        payload.extend_from_slice(&resolved_count.to_le_bytes());
        payload.extend_from_slice(&ready_count.to_le_bytes());
        payload.extend_from_slice(&writeback_payload);
        for scope in &self.snapshot.resolved_dependency_scopes {
            payload.extend_from_slice(&scope.get().to_le_bytes());
        }
        for instruction in &self.snapshot.ready {
            let waits_on_count =
                encode_checkpoint_u32("waits_on_count", instruction.waits_on().len())
                    .expect("pending-state checkpoint payload was validated before construction");
            let produces_count =
                encode_checkpoint_u32("produces_count", instruction.produces().len())
                    .expect("pending-state checkpoint payload was validated before construction");
            payload.extend_from_slice(&instruction.sequence().to_le_bytes());
            payload.extend_from_slice(&instruction.queue().get().to_le_bytes());
            payload.push(encode_checkpoint_op_class(instruction.op_class()));
            payload.extend_from_slice(&waits_on_count.to_le_bytes());
            payload.extend_from_slice(&produces_count.to_le_bytes());
            for scope in instruction.waits_on() {
                payload.extend_from_slice(&scope.get().to_le_bytes());
            }
            for scope in instruction.produces() {
                payload.extend_from_slice(&scope.get().to_le_bytes());
            }
        }
        payload
    }

    pub const fn snapshot(&self) -> &O3PendingStateSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> O3PendingStateSnapshot {
        self.snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3WritebackTransferBuffer {
    policy: O3WritebackTransferPolicy,
    deferred: VecDeque<O3WritebackCompletion>,
}

impl O3WritebackTransferBuffer {
    pub fn new(policy: O3WritebackTransferPolicy) -> Self {
        Self {
            policy,
            deferred: VecDeque::new(),
        }
    }

    pub const fn policy(&self) -> &O3WritebackTransferPolicy {
        &self.policy
    }

    pub fn pending_deferred_count(&self) -> usize {
        self.deferred.len()
    }

    pub fn is_empty(&self) -> bool {
        self.deferred.is_empty()
    }

    pub fn snapshot(&self) -> O3WritebackTransferSnapshot {
        O3WritebackTransferSnapshot::new(self.policy.clone(), self.deferred.iter().copied())
    }

    pub fn from_snapshot(snapshot: O3WritebackTransferSnapshot) -> Result<Self, O3PipelineError> {
        let mut deferred = VecDeque::new();
        deferred.extend(snapshot.deferred);
        Ok(Self {
            policy: snapshot.policy,
            deferred,
        })
    }

    pub fn restore(
        &mut self,
        snapshot: O3WritebackTransferSnapshot,
    ) -> Result<(), O3PipelineError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn plan_cycle<I>(&mut self, ready: I) -> O3WritebackTransferCycle
    where
        I: IntoIterator<Item = O3WritebackCompletion>,
    {
        self.plan_cycle_with_occupied_slots(std::iter::empty::<usize>(), ready)
            .expect("empty writeback occupied slot set is valid")
    }

    pub fn plan_cycle_with_occupied_slots<I, O>(
        &mut self,
        occupied_slots: O,
        ready: I,
    ) -> Result<O3WritebackTransferCycle, O3PipelineError>
    where
        I: IntoIterator<Item = O3WritebackCompletion>,
        O: IntoIterator<Item = usize>,
    {
        let source = self.policy.source();
        let writeback_width = self.policy.writeback_width();
        let mut occupied_slots = occupied_slots.into_iter().collect::<Vec<_>>();
        occupied_slots.sort_unstable();

        for slots in occupied_slots.windows(2) {
            if slots[0] == slots[1] {
                return Err(O3PipelineError::DuplicateWritebackOccupiedSlot {
                    source,
                    slot: slots[0],
                });
            }
        }
        for slot in &occupied_slots {
            if *slot >= writeback_width {
                return Err(O3PipelineError::WritebackOccupiedSlotOutOfRange {
                    source,
                    slot: *slot,
                    writeback_width,
                });
            }
        }

        let deferred_before_count = self.deferred.len();
        let new_ready = ready.into_iter().collect::<Vec<_>>();
        let new_ready_count = new_ready.len();

        let mut ordered = Vec::with_capacity(deferred_before_count + new_ready_count);
        while let Some(completion) = self.deferred.pop_front() {
            ordered.push(completion);
        }
        ordered.extend(new_ready);

        let available_count = self.policy.capacity_entries() - occupied_slots.len();
        let admitted_count = ordered.len().min(available_count);
        let mut admissions = Vec::with_capacity(admitted_count);
        let mut ready_index = 0;

        'window: for cycle_offset in 0..=self.policy.future_cycles() {
            for slot in 0..writeback_width {
                if ready_index == admitted_count {
                    break 'window;
                }
                if cycle_offset == 0 && occupied_slots.binary_search(&slot).is_ok() {
                    continue;
                }
                admissions.push(O3WritebackCompletionAdmission {
                    completion: ordered[ready_index],
                    cycle_offset,
                    slot,
                });
                ready_index += 1;
            }
        }

        let deferred = ordered.into_iter().skip(admitted_count).collect::<Vec<_>>();
        self.deferred.extend(deferred.iter().copied());

        Ok(O3WritebackTransferCycle {
            new_ready_count,
            deferred_before_count,
            admissions,
            deferred,
        })
    }
}

fn encode_checkpoint_stage(stage: O3PipelineStage) -> u8 {
    match stage {
        O3PipelineStage::Fetch => 0,
        O3PipelineStage::Decode => 1,
        O3PipelineStage::Rename => 2,
        O3PipelineStage::Iew => 3,
        O3PipelineStage::Commit => 4,
    }
}

fn decode_checkpoint_stage(code: u8) -> Result<O3PipelineStage, O3PipelineError> {
    match code {
        0 => Ok(O3PipelineStage::Fetch),
        1 => Ok(O3PipelineStage::Decode),
        2 => Ok(O3PipelineStage::Rename),
        3 => Ok(O3PipelineStage::Iew),
        4 => Ok(O3PipelineStage::Commit),
        _ => Err(O3PipelineError::InvalidCheckpointStageCode { code }),
    }
}

fn encode_checkpoint_op_class(op_class: O3IssueOpClass) -> u8 {
    match op_class {
        O3IssueOpClass::IntAlu => 0,
        O3IssueOpClass::IntMult => 1,
        O3IssueOpClass::Float => 2,
        O3IssueOpClass::Memory => 3,
        O3IssueOpClass::Branch => 4,
        O3IssueOpClass::System => 5,
    }
}

fn decode_checkpoint_op_class(code: u8) -> Result<O3IssueOpClass, O3PipelineError> {
    match code {
        0 => Ok(O3IssueOpClass::IntAlu),
        1 => Ok(O3IssueOpClass::IntMult),
        2 => Ok(O3IssueOpClass::Float),
        3 => Ok(O3IssueOpClass::Memory),
        4 => Ok(O3IssueOpClass::Branch),
        5 => Ok(O3IssueOpClass::System),
        _ => Err(O3PipelineError::InvalidCheckpointOpClassCode { code }),
    }
}

fn encode_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, O3PipelineError> {
    u32::try_from(value).map_err(|_| O3PipelineError::CheckpointValueTooLarge {
        field,
        value,
        maximum: O3_WRITEBACK_CHECKPOINT_U32_MAX,
    })
}

fn validate_pending_state_snapshot(
    snapshot: &O3PendingStateSnapshot,
) -> Result<(), O3PipelineError> {
    encode_checkpoint_u32(
        "resolved_dependency_scope_count",
        snapshot.resolved_dependency_scopes.len(),
    )?;
    encode_checkpoint_u32("ready_instruction_count", snapshot.ready.len())?;
    let writeback_payload =
        O3WritebackTransferCheckpointPayload::from_snapshot(snapshot.writeback.clone())?.encode();
    encode_checkpoint_u32("writeback_payload_length", writeback_payload.len())?;
    for instruction in &snapshot.ready {
        encode_checkpoint_u32("waits_on_count", instruction.waits_on().len())?;
        encode_checkpoint_u32("produces_count", instruction.produces().len())?;
    }
    Ok(())
}

fn ensure_checkpoint_remaining(
    payload: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<(), O3PipelineError> {
    let expected =
        offset
            .checked_add(byte_count)
            .ok_or(O3PipelineError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            })?;
    if payload.len() < expected {
        return Err(O3PipelineError::InvalidCheckpointPayloadSize {
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}

fn checkpoint_field_bytes(
    count: usize,
    bytes_per_item: usize,
    actual: usize,
) -> Result<usize, O3PipelineError> {
    count
        .checked_mul(bytes_per_item)
        .ok_or(O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_PENDING_STATE_CHECKPOINT_HEADER_BYTES,
            actual,
        })
}

fn read_checkpoint_u8(payload: &[u8], offset: &mut usize) -> Result<u8, O3PipelineError> {
    ensure_checkpoint_remaining(payload, *offset, 1)?;
    let value = payload[*offset];
    *offset += 1;
    Ok(value)
}

fn read_checkpoint_u32(payload: &[u8], offset: &mut usize) -> Result<u32, O3PipelineError> {
    ensure_checkpoint_remaining(payload, *offset, U32_BYTES)?;
    Ok(read_u32(payload, offset))
}

fn read_checkpoint_u64(payload: &[u8], offset: &mut usize) -> Result<u64, O3PipelineError> {
    ensure_checkpoint_remaining(payload, *offset, U64_BYTES)?;
    Ok(read_u64(payload, offset))
}

fn read_checkpoint_scopes(
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<O3DependencyScopeId>, O3PipelineError> {
    let bytes = checkpoint_field_bytes(count, U64_BYTES, payload.len())?;
    ensure_checkpoint_remaining(payload, *offset, bytes)?;
    let mut scopes = Vec::with_capacity(count);
    for _ in 0..count {
        scopes.push(O3DependencyScopeId::new(read_checkpoint_u64(
            payload, offset,
        )?));
    }
    Ok(scopes)
}

fn checkpoint_payload_size(deferred_count: usize, actual: usize) -> Result<usize, O3PipelineError> {
    let completion_bytes = deferred_count
        .checked_mul(O3_WRITEBACK_CHECKPOINT_COMPLETION_BYTES)
        .ok_or(O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
            actual,
        })?;
    O3_WRITEBACK_CHECKPOINT_HEADER_BYTES
        .checked_add(completion_bytes)
        .ok_or(O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
            actual,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o3_writeback_checkpoint_size_rejects_overflow() {
        assert_eq!(
            checkpoint_payload_size(usize::MAX, O3_WRITEBACK_CHECKPOINT_HEADER_BYTES).unwrap_err(),
            O3PipelineError::InvalidCheckpointPayloadSize {
                expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
                actual: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
            }
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3WritebackTransferPolicy {
    source: O3PipelineStage,
    writeback_width: usize,
    future_cycles: u64,
    capacity_entries: usize,
}

impl O3WritebackTransferPolicy {
    pub fn new(
        source: O3PipelineStage,
        writeback_width: usize,
        future_cycles: u64,
    ) -> Result<Self, O3PipelineError> {
        if writeback_width == 0 {
            return Err(O3PipelineError::ZeroWritebackWidth { source });
        }
        let Some(cycle_count) = future_cycles.checked_add(1) else {
            return Err(O3PipelineError::WritebackWindowOverflow {
                source,
                writeback_width,
                future_cycles,
            });
        };
        let capacity = (cycle_count as u128) * (writeback_width as u128);
        if capacity > (usize::MAX as u128) {
            return Err(O3PipelineError::WritebackWindowOverflow {
                source,
                writeback_width,
                future_cycles,
            });
        }

        Ok(Self {
            source,
            writeback_width,
            future_cycles,
            capacity_entries: capacity as usize,
        })
    }

    pub const fn source(&self) -> O3PipelineStage {
        self.source
    }

    pub const fn writeback_width(&self) -> usize {
        self.writeback_width
    }

    pub const fn future_cycles(&self) -> u64 {
        self.future_cycles
    }

    pub const fn capacity_entries(&self) -> usize {
        self.capacity_entries
    }

    pub fn plan_ready_count(&self, ready_count: usize) -> O3WritebackTransferPlan {
        let admitted_count = ready_count.min(self.capacity_entries);
        let deferred_count = ready_count - admitted_count;
        let mut admissions = Vec::with_capacity(admitted_count);

        for ready_index in 0..admitted_count {
            admissions.push(O3WritebackAdmission {
                ready_index,
                cycle_offset: (ready_index / self.writeback_width) as u64,
                slot: ready_index % self.writeback_width,
            });
        }

        O3WritebackTransferPlan {
            ready_count,
            admitted_count,
            deferred_count,
            admissions,
        }
    }
}
