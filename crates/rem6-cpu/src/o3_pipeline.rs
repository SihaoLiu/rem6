use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

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

fn issue_slots(
    capacities: &BTreeMap<(O3IssueQueueId, O3IssueOpClass), usize>,
    instruction: &O3ReadyInstruction,
) -> usize {
    capacities
        .get(&(instruction.queue(), instruction.op_class()))
        .copied()
        .unwrap_or_default()
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
