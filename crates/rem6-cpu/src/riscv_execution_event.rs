use rem6_isa_riscv::{MemoryAccessKind, RiscvExecutionRecord, RiscvInstruction};
use rem6_kernel::PartitionEventId;
use rem6_memory::Address;

use crate::{
    BiModeHistoryUpdate, BiModePrediction, BiModeTrainingUpdate, BranchUpdate, CpuFetchEvent,
    GShareHistoryUpdate, GSharePrediction, GShareTrainingUpdate, InOrderPipelineCycleRecord,
    MultiperspectivePerceptronPrediction, MultiperspectivePerceptronTrainingUpdate,
    RiscvDataAccessEventKind, TageScLPrediction, TageScLTrainingUpdate, TournamentHistoryUpdate,
    TournamentPrediction, TournamentTrainingUpdate,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCpuExecutionEvent {
    fetch: CpuFetchEvent,
    instruction: RiscvInstruction,
    execution: RiscvExecutionRecord,
    branch_update: Option<BranchUpdate>,
    selected_branch_predicted_taken: Option<bool>,
    selected_branch_predicted_target: Option<Address>,
    gshare_branch_update: Option<RiscvGShareBranchUpdate>,
    bimode_branch_update: Option<RiscvBiModeBranchUpdate>,
    tournament_branch_update: Option<RiscvTournamentBranchUpdate>,
    tage_sc_l_branch_update: Option<RiscvTageScLBranchUpdate>,
    multiperspective_perceptron_branch_update: Option<RiscvMultiperspectivePerceptronBranchUpdate>,
    in_order_pipeline_cycle: Option<InOrderPipelineCycleRecord>,
    in_order_pipeline_data_wait_cycles: u64,
    data_access_event_kind: Option<RiscvDataAccessEventKind>,
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
pub struct RiscvBiModeBranchUpdate {
    prediction: BiModePrediction,
    history_update: BiModeHistoryUpdate,
    training_update: BiModeTrainingUpdate,
}

impl RiscvBiModeBranchUpdate {
    pub const fn new(
        prediction: BiModePrediction,
        history_update: BiModeHistoryUpdate,
        training_update: BiModeTrainingUpdate,
    ) -> Self {
        Self {
            prediction,
            history_update,
            training_update,
        }
    }

    pub const fn prediction(&self) -> &BiModePrediction {
        &self.prediction
    }

    pub const fn history_update(&self) -> &BiModeHistoryUpdate {
        &self.history_update
    }

    pub const fn training_update(&self) -> &BiModeTrainingUpdate {
        &self.training_update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTournamentBranchUpdate {
    prediction: TournamentPrediction,
    history_update: TournamentHistoryUpdate,
    training_update: TournamentTrainingUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTageScLBranchUpdate {
    prediction: TageScLPrediction,
    training_update: TageScLTrainingUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvMultiperspectivePerceptronBranchUpdate {
    prediction: MultiperspectivePerceptronPrediction,
    training_update: MultiperspectivePerceptronTrainingUpdate,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvRetiredBranchUpdates {
    branch_update: Option<BranchUpdate>,
    selected_branch_predicted_taken: Option<bool>,
    selected_branch_predicted_target: Option<Address>,
    gshare_branch_update: Option<RiscvGShareBranchUpdate>,
    bimode_branch_update: Option<RiscvBiModeBranchUpdate>,
    tournament_branch_update: Option<RiscvTournamentBranchUpdate>,
    tage_sc_l_branch_update: Option<RiscvTageScLBranchUpdate>,
    multiperspective_perceptron_branch_update: Option<RiscvMultiperspectivePerceptronBranchUpdate>,
}

impl RiscvRetiredBranchUpdates {
    pub(crate) fn new(
        branch_update: BranchUpdate,
        gshare_branch_update: RiscvGShareBranchUpdate,
        bimode_branch_update: RiscvBiModeBranchUpdate,
        tournament_branch_update: RiscvTournamentBranchUpdate,
        tage_sc_l_branch_update: RiscvTageScLBranchUpdate,
        multiperspective_perceptron_branch_update: RiscvMultiperspectivePerceptronBranchUpdate,
    ) -> Self {
        Self {
            branch_update: Some(branch_update),
            selected_branch_predicted_taken: None,
            selected_branch_predicted_target: None,
            gshare_branch_update: Some(gshare_branch_update),
            bimode_branch_update: Some(bimode_branch_update),
            tournament_branch_update: Some(tournament_branch_update),
            tage_sc_l_branch_update: Some(tage_sc_l_branch_update),
            multiperspective_perceptron_branch_update: Some(
                multiperspective_perceptron_branch_update,
            ),
        }
    }

    pub(crate) const fn branch_update(&self) -> Option<&BranchUpdate> {
        self.branch_update.as_ref()
    }

    pub(crate) fn set_selected_branch_prediction(
        &mut self,
        predicted_taken: bool,
        predicted_target: Option<Address>,
    ) {
        self.selected_branch_predicted_taken = Some(predicted_taken);
        self.selected_branch_predicted_target = predicted_target;
    }
}

impl RiscvTournamentBranchUpdate {
    pub const fn new(
        prediction: TournamentPrediction,
        history_update: TournamentHistoryUpdate,
        training_update: TournamentTrainingUpdate,
    ) -> Self {
        Self {
            prediction,
            history_update,
            training_update,
        }
    }

    pub const fn prediction(&self) -> &TournamentPrediction {
        &self.prediction
    }

    pub const fn history_update(&self) -> &TournamentHistoryUpdate {
        &self.history_update
    }

    pub const fn training_update(&self) -> &TournamentTrainingUpdate {
        &self.training_update
    }
}

impl RiscvTageScLBranchUpdate {
    pub const fn new(
        prediction: TageScLPrediction,
        training_update: TageScLTrainingUpdate,
    ) -> Self {
        Self {
            prediction,
            training_update,
        }
    }

    pub const fn prediction(&self) -> &TageScLPrediction {
        &self.prediction
    }

    pub const fn training_update(&self) -> &TageScLTrainingUpdate {
        &self.training_update
    }
}

impl RiscvMultiperspectivePerceptronBranchUpdate {
    pub const fn new(
        prediction: MultiperspectivePerceptronPrediction,
        training_update: MultiperspectivePerceptronTrainingUpdate,
    ) -> Self {
        Self {
            prediction,
            training_update,
        }
    }

    pub const fn prediction(&self) -> &MultiperspectivePerceptronPrediction {
        &self.prediction
    }

    pub const fn training_update(&self) -> &MultiperspectivePerceptronTrainingUpdate {
        &self.training_update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreDriveAction {
    FetchIssued { event: PartitionEventId },
    PipelineCycleScheduled { event: PartitionEventId },
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
        Self {
            fetch,
            instruction,
            execution,
            branch_update,
            selected_branch_predicted_taken: None,
            selected_branch_predicted_target: None,
            gshare_branch_update,
            bimode_branch_update: None,
            tournament_branch_update: None,
            tage_sc_l_branch_update: None,
            multiperspective_perceptron_branch_update: None,
            in_order_pipeline_cycle: None,
            in_order_pipeline_data_wait_cycles: 0,
            data_access_event_kind: None,
            counts_as_retired_instruction: true,
        }
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
            selected_branch_predicted_taken: None,
            selected_branch_predicted_target: None,
            gshare_branch_update,
            bimode_branch_update: None,
            tournament_branch_update: None,
            tage_sc_l_branch_update: None,
            multiperspective_perceptron_branch_update: None,
            in_order_pipeline_cycle,
            in_order_pipeline_data_wait_cycles: 0,
            data_access_event_kind: None,
            counts_as_retired_instruction,
        }
    }

    pub(crate) fn with_all_branch_updates_pipeline_cycle_and_retired_instruction_counting(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
        branch_updates: RiscvRetiredBranchUpdates,
        in_order_pipeline_cycle: Option<InOrderPipelineCycleRecord>,
        in_order_pipeline_data_wait_cycles: u64,
        counts_as_retired_instruction: bool,
    ) -> Self {
        let RiscvRetiredBranchUpdates {
            branch_update,
            selected_branch_predicted_taken,
            selected_branch_predicted_target,
            gshare_branch_update,
            bimode_branch_update,
            tournament_branch_update,
            tage_sc_l_branch_update,
            multiperspective_perceptron_branch_update,
        } = branch_updates;
        Self {
            fetch,
            instruction,
            execution,
            branch_update,
            selected_branch_predicted_taken,
            selected_branch_predicted_target,
            gshare_branch_update,
            bimode_branch_update,
            tournament_branch_update,
            tage_sc_l_branch_update,
            multiperspective_perceptron_branch_update,
            in_order_pipeline_cycle,
            in_order_pipeline_data_wait_cycles,
            data_access_event_kind: None,
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

    pub(crate) const fn selected_branch_prediction(&self) -> Option<(bool, Option<Address>)> {
        match self.selected_branch_predicted_taken {
            Some(predicted_taken) => Some((predicted_taken, self.selected_branch_predicted_target)),
            None => None,
        }
    }

    pub fn gshare_branch_update(&self) -> Option<&RiscvGShareBranchUpdate> {
        self.gshare_branch_update.as_ref()
    }

    pub fn bimode_branch_update(&self) -> Option<&RiscvBiModeBranchUpdate> {
        self.bimode_branch_update.as_ref()
    }

    pub fn tournament_branch_update(&self) -> Option<&RiscvTournamentBranchUpdate> {
        self.tournament_branch_update.as_ref()
    }

    pub fn tage_sc_l_branch_update(&self) -> Option<&RiscvTageScLBranchUpdate> {
        self.tage_sc_l_branch_update.as_ref()
    }

    pub fn multiperspective_perceptron_branch_update(
        &self,
    ) -> Option<&RiscvMultiperspectivePerceptronBranchUpdate> {
        self.multiperspective_perceptron_branch_update.as_ref()
    }

    pub fn in_order_pipeline_cycle(&self) -> Option<&InOrderPipelineCycleRecord> {
        self.in_order_pipeline_cycle.as_ref()
    }

    pub const fn in_order_pipeline_data_wait_cycles(&self) -> u64 {
        self.in_order_pipeline_data_wait_cycles
    }

    pub const fn data_access_event_kind(&self) -> Option<RiscvDataAccessEventKind> {
        self.data_access_event_kind
    }

    pub(crate) fn set_in_order_pipeline_cycle(&mut self, cycle: InOrderPipelineCycleRecord) {
        self.in_order_pipeline_cycle = Some(cycle);
    }

    pub(crate) fn set_in_order_pipeline_data_wait_cycles(&mut self, cycles: u64) {
        self.in_order_pipeline_data_wait_cycles = cycles;
    }

    pub(crate) fn set_data_access_event_kind(&mut self, kind: RiscvDataAccessEventKind) {
        self.data_access_event_kind = Some(kind);
    }

    pub(crate) fn clear_data_access_retirement(&mut self) {
        self.data_access_event_kind = None;
        self.in_order_pipeline_cycle = None;
        self.in_order_pipeline_data_wait_cycles = 0;
    }

    pub const fn counts_as_retired_instruction(&self) -> bool {
        self.counts_as_retired_instruction
    }

    pub fn is_scalar_memory_access(&self) -> bool {
        matches!(
            self.execution.memory_access(),
            Some(MemoryAccessKind::Load { .. } | MemoryAccessKind::Store { .. })
        )
    }
}
