use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};
use rem6_kernel::PartitionEventId;
use rem6_memory::Address;

use crate::{
    BranchUpdate, CpuFetchEvent, GShareHistoryUpdate, GSharePrediction, GShareTrainingUpdate,
    InOrderPipelineCycleRecord,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCpuExecutionEvent {
    fetch: CpuFetchEvent,
    instruction: RiscvInstruction,
    execution: RiscvExecutionRecord,
    branch_update: Option<BranchUpdate>,
    gshare_branch_update: Option<RiscvGShareBranchUpdate>,
    in_order_pipeline_cycle: Option<InOrderPipelineCycleRecord>,
    counts_as_retired_instruction: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGShareBranchUpdate {
    prediction: GSharePrediction,
    history_update: GShareHistoryUpdate,
    training_update: GShareTrainingUpdate,
}

impl RiscvGShareBranchUpdate {
    pub const fn new(
        prediction: GSharePrediction,
        history_update: GShareHistoryUpdate,
        training_update: GShareTrainingUpdate,
    ) -> Self {
        Self {
            prediction,
            history_update,
            training_update,
        }
    }

    pub const fn prediction(&self) -> &GSharePrediction {
        &self.prediction
    }

    pub const fn history_update(&self) -> &GShareHistoryUpdate {
        &self.history_update
    }

    pub const fn training_update(&self) -> &GShareTrainingUpdate {
        &self.training_update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreDriveAction {
    FetchIssued { event: PartitionEventId },
    InstructionExecuted(Box<RiscvCpuExecutionEvent>),
    DataAccessIssued { event: PartitionEventId },
}

impl RiscvCpuExecutionEvent {
    pub const fn new(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
    ) -> Self {
        Self::with_branch_update(fetch, instruction, execution, None)
    }

    pub const fn with_branch_update(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
        branch_update: Option<BranchUpdate>,
    ) -> Self {
        Self::with_branch_updates(fetch, instruction, execution, branch_update, None)
    }

    pub const fn with_branch_updates(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
        branch_update: Option<BranchUpdate>,
        gshare_branch_update: Option<RiscvGShareBranchUpdate>,
    ) -> Self {
        Self::with_branch_updates_pipeline_cycle_and_retired_instruction_counting(
            fetch,
            instruction,
            execution,
            branch_update,
            gshare_branch_update,
            None,
            true,
        )
    }

    pub const fn with_retired_instruction_counting(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
        branch_update: Option<BranchUpdate>,
        counts_as_retired_instruction: bool,
    ) -> Self {
        Self::with_branch_updates_pipeline_cycle_and_retired_instruction_counting(
            fetch,
            instruction,
            execution,
            branch_update,
            None,
            None,
            counts_as_retired_instruction,
        )
    }

    pub const fn with_branch_updates_and_retired_instruction_counting(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
        branch_update: Option<BranchUpdate>,
        gshare_branch_update: Option<RiscvGShareBranchUpdate>,
        counts_as_retired_instruction: bool,
    ) -> Self {
        Self::with_branch_updates_pipeline_cycle_and_retired_instruction_counting(
            fetch,
            instruction,
            execution,
            branch_update,
            gshare_branch_update,
            None,
            counts_as_retired_instruction,
        )
    }

    pub const fn with_branch_updates_pipeline_cycle_and_retired_instruction_counting(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
        branch_update: Option<BranchUpdate>,
        gshare_branch_update: Option<RiscvGShareBranchUpdate>,
        in_order_pipeline_cycle: Option<InOrderPipelineCycleRecord>,
        counts_as_retired_instruction: bool,
    ) -> Self {
        Self {
            fetch,
            instruction,
            execution,
            branch_update,
            gshare_branch_update,
            in_order_pipeline_cycle,
            counts_as_retired_instruction,
        }
    }

    pub fn fetch(&self) -> &CpuFetchEvent {
        &self.fetch
    }

    pub fn fetch_pc(&self) -> Address {
        self.fetch.pc()
    }

    pub const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub fn execution(&self) -> &RiscvExecutionRecord {
        &self.execution
    }

    pub fn branch_update(&self) -> Option<&BranchUpdate> {
        self.branch_update.as_ref()
    }

    pub fn gshare_branch_update(&self) -> Option<&RiscvGShareBranchUpdate> {
        self.gshare_branch_update.as_ref()
    }

    pub fn in_order_pipeline_cycle(&self) -> Option<&InOrderPipelineCycleRecord> {
        self.in_order_pipeline_cycle.as_ref()
    }

    pub(crate) fn set_in_order_pipeline_cycle(&mut self, cycle: InOrderPipelineCycleRecord) {
        self.in_order_pipeline_cycle = Some(cycle);
    }

    pub const fn counts_as_retired_instruction(&self) -> bool {
        self.counts_as_retired_instruction
    }
}
