use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_isa_riscv::{RiscvHartState, RiscvPmpSnapshot};
use rem6_memory::{Address, MemoryRequestId};

use crate::cpu_core::CpuCoreCheckpointState;
use crate::riscv_live_retire_gate::RiscvLiveRetireGatePolicy;
use crate::{
    BiModeBranchPredictorCheckpointPayload, BranchPredictorCheckpointPayload, CpuCore,
    GShareBranchPredictorCheckpointPayload, InOrderPipelineSnapshot,
    MultiperspectivePerceptronCheckpointPayload, O3RuntimeCheckpointPayload, RiscvCore,
    RiscvCoreState, RiscvHartRunState, RiscvO3LiveCheckpointError, RiscvO3LiveCheckpointPayload,
    TageScLBranchPredictorCheckpointPayload, TournamentBranchPredictorCheckpointPayload,
};

#[path = "riscv_core_checkpoint_restore/pending_address.rs"]
mod pending_address;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCoreCheckpointRestoreInput {
    pub hart: RiscvHartState,
    pub pmp: RiscvPmpSnapshot,
    pub run_state: RiscvHartRunState,
    pub pipeline: InOrderPipelineSnapshot,
    pub branch: BranchPredictorCheckpointPayload,
    pub gshare: GShareBranchPredictorCheckpointPayload,
    pub bimode: BiModeBranchPredictorCheckpointPayload,
    pub tournament: TournamentBranchPredictorCheckpointPayload,
    pub tage_sc_l: TageScLBranchPredictorCheckpointPayload,
    pub perceptron: MultiperspectivePerceptronCheckpointPayload,
    pub o3: O3RuntimeCheckpointPayload,
    pub live: Option<RiscvO3LiveCheckpointPayload>,
    live_data_handoff_present: bool,
}

impl RiscvCoreCheckpointRestoreInput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hart: RiscvHartState,
        pmp: RiscvPmpSnapshot,
        run_state: RiscvHartRunState,
        pipeline: InOrderPipelineSnapshot,
        branch: BranchPredictorCheckpointPayload,
        gshare: GShareBranchPredictorCheckpointPayload,
        bimode: BiModeBranchPredictorCheckpointPayload,
        tournament: TournamentBranchPredictorCheckpointPayload,
        tage_sc_l: TageScLBranchPredictorCheckpointPayload,
        perceptron: MultiperspectivePerceptronCheckpointPayload,
        o3: O3RuntimeCheckpointPayload,
        live: Option<RiscvO3LiveCheckpointPayload>,
    ) -> Self {
        Self {
            hart,
            pmp,
            run_state,
            pipeline,
            branch,
            gshare,
            bimode,
            tournament,
            tage_sc_l,
            perceptron,
            o3,
            live,
            live_data_handoff_present: false,
        }
    }

    pub fn with_pmp_snapshot(mut self, pmp: RiscvPmpSnapshot) -> Self {
        self.pmp = pmp;
        self
    }

    pub fn with_live_data_handoff_present(mut self, present: bool) -> Self {
        self.live_data_handoff_present = present;
        self
    }
}

#[derive(Debug)]
pub struct PreparedRiscvCoreRestore {
    cpu: CpuCoreCheckpointState,
    riscv: RiscvCoreState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreCheckpointRestoreError {
    InvalidStable(&'static str),
    InvalidLive(RiscvO3LiveCheckpointError),
    LiveDataHandoffNotRestorable,
}

impl fmt::Display for RiscvCoreCheckpointRestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStable(component) => {
                write!(formatter, "invalid stable RISC-V checkpoint {component}")
            }
            Self::InvalidLive(error) => write!(formatter, "{error}"),
            Self::LiveDataHandoffNotRestorable => {
                formatter.write_str("O3 live handoff cannot be restored")
            }
        }
    }
}

impl Error for RiscvCoreCheckpointRestoreError {}

impl RiscvCore {
    #[doc(hidden)]
    pub fn checkpoint_hart_state(&self) -> RiscvHartState {
        self.state.lock().expect("riscv core lock").hart.clone()
    }

    pub fn prepare_checkpoint_restore(
        &self,
        input: RiscvCoreCheckpointRestoreInput,
    ) -> Result<PreparedRiscvCoreRestore, RiscvCoreCheckpointRestoreError> {
        if input.live_data_handoff_present {
            return Err(RiscvCoreCheckpointRestoreError::LiveDataHandoffNotRestorable);
        }
        let (cpu, riscv) = {
            let cpu = self.core.state.lock().expect("cpu core lock");
            let riscv = self.state.lock().expect("riscv core lock");
            (CpuCore::checkpoint_state_from_guard(&cpu), riscv.clone())
        };
        let detached = RiscvCore {
            core: CpuCore::from_checkpoint_state(cpu),
            state: Arc::new(Mutex::new(riscv)),
        };
        prepare_stable(&detached, &input)?;
        if let Some(live) = input.live.as_ref() {
            prepare_and_install_live(&detached, input.o3.clone(), live)?;
        }
        let cpu = detached.core.checkpoint_state();
        let riscv = detached.state.lock().expect("riscv core lock").clone();
        Ok(PreparedRiscvCoreRestore { cpu, riscv })
    }

    pub fn install_prepared_checkpoint_restore(&self, prepared: PreparedRiscvCoreRestore) {
        let mut cpu_state = self.core.state.lock().expect("cpu core lock");
        let mut riscv_state = self.state.lock().expect("riscv core lock");
        CpuCore::install_checkpoint_state_into_guard(&mut cpu_state, prepared.cpu);
        *riscv_state = prepared.riscv;
    }
}

fn prepare_stable(
    core: &RiscvCore,
    input: &RiscvCoreCheckpointRestoreInput,
) -> Result<(), RiscvCoreCheckpointRestoreError> {
    core.restore_pmp_snapshot(&input.pmp)
        .map_err(|_| stable("PMP"))?;
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart = input.hart.clone();
        state.reservation = None;
        crate::riscv_checker::sync_checker_hart(&mut state);
    }
    core.redirect_pc(Address::new(input.hart.pc()));
    match input.run_state {
        RiscvHartRunState::Started => core.set_hart_started(),
        RiscvHartRunState::StartPending => core.set_hart_start_pending(),
        RiscvHartRunState::StopPending => core.set_hart_stop_pending(),
        RiscvHartRunState::SuspendPending => core.set_hart_suspend_pending(),
        RiscvHartRunState::ResumePending => core.set_hart_resume_pending(),
        RiscvHartRunState::Stopped => core.set_hart_stopped(),
        RiscvHartRunState::Suspended => core.set_hart_suspended(),
    }
    core.restore_in_order_pipeline_snapshot(input.pipeline.clone())
        .map_err(|_| stable("in-order pipeline"))?;
    core.restore_branch_predictor_checkpoint_payload(input.branch.clone())
        .map_err(|_| stable("branch predictor"))?;
    core.restore_gshare_branch_predictor_checkpoint_payload(input.gshare.clone())
        .map_err(|_| stable("gshare predictor"))?;
    core.restore_bimode_branch_predictor_checkpoint_payload(input.bimode.clone())
        .map_err(|_| stable("bimode predictor"))?;
    core.restore_tournament_branch_predictor_checkpoint_payload(input.tournament.clone())
        .map_err(|_| stable("tournament predictor"))?;
    core.restore_tage_sc_l_branch_predictor_checkpoint_payload(input.tage_sc_l.clone())
        .map_err(|_| stable("TAGE-SC-L predictor"))?;
    core.restore_multiperspective_perceptron_checkpoint_payload(input.perceptron.clone())
        .map_err(|_| stable("multiperspective perceptron"))?;
    core.restore_o3_runtime_checkpoint_payload(input.o3.clone())
        .map_err(|_| stable("O3 runtime"))?;
    core.state
        .lock()
        .expect("riscv core lock")
        .scrub_checkpoint_restore_transients();
    Ok(())
}

fn prepare_and_install_live(
    core: &RiscvCore,
    stable_o3: O3RuntimeCheckpointPayload,
    live: &RiscvO3LiveCheckpointPayload,
) -> Result<(), RiscvCoreCheckpointRestoreError> {
    if live.wake.scheduler_instance_raw == 0
        || live.wake.partition != core.partition()
        || live.wake.tick != live.service.requested_tick
        || live.wake.tick < live.captured_tick
    {
        return Err(live_error("live wake authority is inconsistent"));
    }
    let operational_fetch = pending_address::operational_fetch_projection(core, live)?;
    let fetch_agent = core.agent();
    let completed_data_request_invalid = live.completed_result.as_ref().is_some_and(|result| {
        result.data_request.agent() != fetch_agent
            || result.data_request.sequence() <= result.fetch_request.sequence()
            || result.data_request.sequence() >= live.next_fetch_request_sequence
            || operational_fetch.requests.contains(&result.data_request)
    });
    if completed_data_request_invalid {
        return Err(live_error("live fetch membership is inconsistent"));
    }

    let mut replacement_riscv = core.state.lock().expect("riscv core lock").clone();
    let prepared_live = replacement_riscv
        .o3_runtime
        .prepare_live_checkpoint_restore(stable_o3, live)
        .map_err(RiscvCoreCheckpointRestoreError::InvalidLive)?;
    let (runtime, restored_events, memory_result_authorization) = prepared_live.into_parts();
    replacement_riscv.events.retain(|event| {
        event.fetch().request_id().sequence() < live.next_fetch_request_sequence
            && !operational_fetch
                .requests
                .contains(&event.fetch().request_id())
    });
    replacement_riscv.events.extend(restored_events);
    let retain_operational_data_history =
        live.profile == crate::RiscvO3LiveCheckpointProfile::CompletedFpLoad;
    replacement_riscv.data_events.retain(|event| {
        event.tick() <= live.captured_tick
            && event.fetch_request_id().sequence() < live.next_fetch_request_sequence
            && (retain_operational_data_history
                || !operational_fetch
                    .requests
                    .contains(&event.fetch_request_id()))
    });
    trim_and_extend_requests(
        &mut replacement_riscv.executed_fetches,
        live.next_fetch_request_sequence,
        &operational_fetch.requests,
        &live.executed_fetch_requests,
    );
    trim_and_extend_requests(
        &mut replacement_riscv.issued_data_for_fetches,
        live.next_fetch_request_sequence,
        &operational_fetch.requests,
        &live.issued_fetch_requests,
    );
    if let Some((request, authorization)) = memory_result_authorization {
        if !replacement_riscv
            .memory_result_window_authorizations
            .is_empty()
        {
            return Err(live_error(
                "stable restore retained memory-result authorization",
            ));
        }
        replacement_riscv
            .memory_result_window_authorizations
            .insert(request, authorization);
    }
    replacement_riscv.o3_runtime = runtime;
    if live.profile == crate::RiscvO3LiveCheckpointProfile::CompletedFpLoad {
        replacement_riscv
            .live_retire_gate
            .set_policy(RiscvLiveRetireGatePolicy::detailed());
    }
    replacement_riscv
        .o3_writeback_wake
        .restore_desired_unscheduled(live.wake.tick);

    let mut replacement_cpu = core.core.checkpoint_state();
    replacement_cpu.replace_operational_fetch(
        live.next_fetch_pc,
        live.next_fetch_request_sequence,
        operational_fetch.fetches,
    );
    core.core.install_checkpoint_state(replacement_cpu);
    *core.state.lock().expect("riscv core lock") = replacement_riscv;
    Ok(())
}

impl RiscvCoreState {
    fn scrub_checkpoint_restore_transients(&mut self) {
        self.pending_fetch_prefix = None;
        self.source_local_checkpoint_capture_deadlines.clear();
        self.source_local_checkpoint_restore_deadlines.clear();
        self.pending_terminal_memory_result = None;
        if let Some(frontend) = self.data_translation.as_mut() {
            frontend.clear_pending();
        }
        self.pending_data_translations.clear();
        self.ready_translated_data.clear();
        self.outstanding_data.clear();
        self.buffered_o3_effects.clear();
        self.translated_scalar_load_window_fetches.clear();
        self.memory_result_window_authorizations.clear();
        self.pending_trap = None;
        self.pending_trap_event = None;
        self.reservation = None;
        self.htm.clear_active_preserving_history();
        self.htm_hart_checkpoint = None;
        self.pending_callback_error = None;
        self.producer_forwarded_scalar_continuation = None;
        self.selected_branch_speculations.clear();
        self.o3_writeback_wake.clear();
        self.forget_in_order_pipeline_wakes();
        self.rebound_in_order_execute_waits.clear();
        self.o3_force_normal_execute_fetches.clear();
    }
}

fn trim_and_extend_requests(
    destination: &mut BTreeSet<MemoryRequestId>,
    next_sequence: u64,
    replaced: &BTreeSet<MemoryRequestId>,
    restored: &[MemoryRequestId],
) {
    destination.retain(|request| request.sequence() < next_sequence && !replaced.contains(request));
    destination.extend(restored.iter().copied());
}

fn stable(component: &'static str) -> RiscvCoreCheckpointRestoreError {
    RiscvCoreCheckpointRestoreError::InvalidStable(component)
}

fn live_error(reason: &'static str) -> RiscvCoreCheckpointRestoreError {
    RiscvCoreCheckpointRestoreError::InvalidLive(RiscvO3LiveCheckpointError::InvalidProfileShape {
        reason,
    })
}
