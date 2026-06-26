use std::collections::BTreeMap;

use rem6_isa_riscv::{RiscvHartState, RiscvInstruction, RiscvPrivilegeMode};
use rem6_memory::Address;

use crate::{
    riscv_branch_kind::riscv_branch_target_kind, BiModeBranchPredictor, BranchPredictor,
    BranchSpeculationId, BranchTargetKind, BranchTargetPrediction, BranchTargetProvider,
    CpuFetchEvent, CpuFetchEventKind, GShareBranchPredictor,
    MultiperspectivePerceptronThreadSnapshot, RiscvBranchPredictorKind, RiscvCore, RiscvCoreState,
    RiscvCpuError, RiscvSelectedBranchSpeculation, TournamentBranchPredictor,
    RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
};

mod speculation;

const COMPLETED_FETCH_WINDOW: usize = 2;

impl RiscvCore {
    pub(crate) fn next_fetch_ahead_before_retire(&self) -> Option<RiscvFetchAheadDecision> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() || state.pending_fetch_prefix.is_some() {
            return None;
        }
        if hart_has_enabled_pending_interrupt(&state.hart) {
            return None;
        }

        let mut completed = fetch_events
            .iter()
            .filter(|event| {
                event.kind() == CpuFetchEventKind::Completed
                    && !state.executed_fetches.contains(&event.request_id())
            })
            .collect::<Vec<_>>();
        if completed.is_empty() || completed.len() >= completed_fetch_window(&state) {
            return None;
        }
        completed.sort_by_key(|event| event.request_id().sequence());

        let fetch = next_fetch_ahead_candidate(&state, &completed)?;
        let data = fetch.data()?;
        let raw = match data {
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => return None,
        };
        let Ok(decoded) = RiscvInstruction::decode_with_length(raw) else {
            return None;
        };
        let sequential_pc = Address::new(fetch.pc().get().wrapping_add(u64::from(decoded.bytes())));

        fetch_ahead_decision(
            &mut state,
            &completed,
            fetch.request_id().sequence(),
            fetch.pc(),
            sequential_pc,
            decoded.instruction(),
        )
    }

    pub(crate) fn set_fetch_ahead_pc(&self, pc: Address) {
        self.core.set_pc(pc);
    }

    pub(crate) fn prepare_fetch_ahead_speculation(
        &self,
        decision: &RiscvFetchAheadDecision,
    ) -> Result<Option<PreparedRiscvFetchAheadSpeculation>, RiscvCpuError> {
        let Some(speculation) = decision.branch_speculation() else {
            return Ok(None);
        };
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
        if state
            .branch_speculations
            .contains_key(&speculation.sequence)
        {
            return Ok(None);
        }
        let selected = speculation
            .selected_speculation
            .as_ref()
            .map(|selected| {
                preview_selected_branch_speculation(
                    &state,
                    &fetch_events,
                    speculation.sequence,
                    selected,
                )
            })
            .transpose()?;
        Ok(Some(PreparedRiscvFetchAheadSpeculation {
            speculation: speculation.clone(),
            selected,
        }))
    }

    pub(crate) fn record_prepared_fetch_ahead_speculation(
        &self,
        prepared: Option<PreparedRiscvFetchAheadSpeculation>,
    ) {
        let Some(prepared) = prepared else {
            return;
        };
        let mut state = self.state.lock().expect("riscv core lock");
        prepared.apply(&mut state);
    }

    pub(crate) fn can_retire_completed_fetch_while_fetch_pending(
        &self,
    ) -> Result<bool, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some()
            || state.pending_fetch_prefix.is_some()
            || hart_has_enabled_pending_interrupt(&state.hart)
        {
            return Ok(false);
        }

        can_retire_completed_fetch_with_branch_speculations(&mut state, &fetch_events)
    }
}

pub(crate) struct PreparedRiscvFetchAheadSpeculation {
    speculation: RiscvFetchAheadSpeculation,
    selected: Option<SelectedBranchRecordedState>,
}

impl PreparedRiscvFetchAheadSpeculation {
    fn apply(self, state: &mut RiscvCoreState) {
        let Self {
            speculation,
            selected,
        } = self;
        if state
            .branch_speculations
            .contains_key(&speculation.sequence)
        {
            return;
        }
        if let Some(selected) = selected {
            selected.apply(state);
        }
        let prediction = state.branch_predictor.predict_speculative_with_prediction(
            speculation.pc,
            speculation.predicted_taken,
            speculation.target,
        );
        state
            .branch_speculations
            .insert(speculation.sequence, prediction.id());
        if let Some(branch_target_prediction) = speculation.branch_target_prediction {
            state
                .branch_target_predictions
                .insert(speculation.sequence, branch_target_prediction);
        }
        let pending = state.branch_speculations.len() as u64;
        state.branch_speculation_summary.record_prediction(
            speculation.branch_kind,
            speculation.target_provider,
            pending,
        );
    }
}

struct SelectedBranchRecordingState<'a> {
    branch_predictor: &'a BranchPredictor,
    branch_speculations: &'a BTreeMap<u64, BranchSpeculationId>,
    selected_branch_speculations: BTreeMap<u64, RiscvSelectedBranchSpeculation>,
    gshare_branch_predictor: GShareBranchPredictor,
    bimode_branch_predictor: BiModeBranchPredictor,
    tournament_branch_predictor: TournamentBranchPredictor,
}

impl<'a> SelectedBranchRecordingState<'a> {
    fn new(state: &'a RiscvCoreState) -> Self {
        Self {
            branch_predictor: &state.branch_predictor,
            branch_speculations: &state.branch_speculations,
            selected_branch_speculations: state.selected_branch_speculations.clone(),
            gshare_branch_predictor: state.gshare_branch_predictor.clone(),
            bimode_branch_predictor: state.bimode_branch_predictor.clone(),
            tournament_branch_predictor: state.tournament_branch_predictor.clone(),
        }
    }

    fn finish(self) -> SelectedBranchRecordedState {
        SelectedBranchRecordedState {
            selected_branch_speculations: self.selected_branch_speculations,
            gshare_branch_predictor: self.gshare_branch_predictor,
            bimode_branch_predictor: self.bimode_branch_predictor,
            tournament_branch_predictor: self.tournament_branch_predictor,
        }
    }
}

struct SelectedBranchRecordedState {
    selected_branch_speculations: BTreeMap<u64, RiscvSelectedBranchSpeculation>,
    gshare_branch_predictor: GShareBranchPredictor,
    bimode_branch_predictor: BiModeBranchPredictor,
    tournament_branch_predictor: TournamentBranchPredictor,
}

impl SelectedBranchRecordedState {
    fn apply(self, state: &mut RiscvCoreState) {
        state.selected_branch_speculations = self.selected_branch_speculations;
        state.gshare_branch_predictor = self.gshare_branch_predictor;
        state.bimode_branch_predictor = self.bimode_branch_predictor;
        state.tournament_branch_predictor = self.tournament_branch_predictor;
    }
}

fn preview_selected_branch_speculation(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
    selected: &RiscvSelectedBranchSpeculation,
) -> Result<SelectedBranchRecordedState, RiscvCpuError> {
    let mut recording = SelectedBranchRecordingState::new(state);
    record_selected_branch_speculation(&mut recording, fetch_events, sequence, selected)?;
    Ok(recording.finish())
}

fn record_selected_branch_speculation(
    state: &mut SelectedBranchRecordingState<'_>,
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
    selected: &RiscvSelectedBranchSpeculation,
) -> Result<(), RiscvCpuError> {
    record_missing_selected_branch_speculations(state, fetch_events, sequence, selected)?;
    let recorded = match selected {
        RiscvSelectedBranchSpeculation::GShare { prediction, .. } => {
            record_gshare_selected_prediction(
                state,
                prediction.clone(),
                prediction.predicted_taken(),
            )?
        }
        RiscvSelectedBranchSpeculation::BiMode { prediction, .. } => {
            record_bimode_selected_prediction(
                state,
                prediction.clone(),
                prediction.predicted_taken(),
            )?
        }
        RiscvSelectedBranchSpeculation::Tournament { prediction, .. } => {
            record_tournament_selected_prediction(
                state,
                prediction.clone(),
                prediction.predicted_taken(),
            )?
        }
    };
    state
        .selected_branch_speculations
        .insert(sequence, recorded);
    Ok(())
}

fn record_missing_selected_branch_speculations(
    state: &mut SelectedBranchRecordingState<'_>,
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
    selected: &RiscvSelectedBranchSpeculation,
) -> Result<(), RiscvCpuError> {
    let latest_live_sequence = latest_live_selected_branch_speculation_sequence(
        &state.selected_branch_speculations,
        &mut |speculation| selected_branch_speculation_same_family(selected, speculation),
    );
    let missing = state
        .branch_speculations
        .iter()
        .filter(|(pending_sequence, _)| **pending_sequence < sequence)
        .filter(|(pending_sequence, _)| {
            latest_live_sequence.is_none_or(|latest| **pending_sequence > latest)
        })
        .map(|(pending_sequence, speculation)| (*pending_sequence, *speculation))
        .collect::<Vec<_>>();
    for (pending_sequence, speculation) in missing {
        let pending = state
            .branch_predictor
            .pending_speculation(speculation)
            .ok_or(crate::BranchPredictorError::UnknownSpeculation { id: speculation })
            .map_err(RiscvCpuError::BranchPredictor)?
            .clone();
        let recorded = replay_selected_branch_speculation(
            state,
            fetch_events,
            selected,
            pending_sequence,
            pending.pc(),
            pending.predicted_taken(),
        )?;
        state
            .selected_branch_speculations
            .insert(pending_sequence, recorded);
    }
    Ok(())
}

fn selected_branch_speculation_same_family(
    left: &RiscvSelectedBranchSpeculation,
    right: &RiscvSelectedBranchSpeculation,
) -> bool {
    matches!(
        (left, right),
        (
            RiscvSelectedBranchSpeculation::GShare { .. },
            RiscvSelectedBranchSpeculation::GShare { .. }
        ) | (
            RiscvSelectedBranchSpeculation::BiMode { .. },
            RiscvSelectedBranchSpeculation::BiMode { .. }
        ) | (
            RiscvSelectedBranchSpeculation::Tournament { .. },
            RiscvSelectedBranchSpeculation::Tournament { .. }
        )
    )
}

fn replay_selected_branch_speculation(
    state: &mut SelectedBranchRecordingState<'_>,
    fetch_events: &[CpuFetchEvent],
    selected: &RiscvSelectedBranchSpeculation,
    sequence: u64,
    pc: Address,
    predicted_taken: bool,
) -> Result<RiscvSelectedBranchSpeculation, RiscvCpuError> {
    match selected {
        RiscvSelectedBranchSpeculation::GShare { .. } => {
            let global_history = state
                .gshare_branch_predictor
                .global_history(RISCV_LOCAL_GSHARE_THREAD)
                .map_err(RiscvCpuError::GShareBranchPredictor)?;
            let prediction = state
                .gshare_branch_predictor
                .predict_with_global_history_and_direction(
                    RISCV_LOCAL_GSHARE_THREAD,
                    pc,
                    global_history,
                    predicted_taken,
                )
                .map_err(RiscvCpuError::GShareBranchPredictor)?;
            record_gshare_selected_prediction(state, prediction, predicted_taken)
        }
        RiscvSelectedBranchSpeculation::BiMode { .. } => {
            let global_history = state
                .bimode_branch_predictor
                .global_history(RISCV_LOCAL_BIMODE_THREAD)
                .map_err(RiscvCpuError::BiModeBranchPredictor)?;
            let prediction = state
                .bimode_branch_predictor
                .predict_with_global_history_and_direction(
                    RISCV_LOCAL_BIMODE_THREAD,
                    pc,
                    global_history,
                    predicted_taken,
                )
                .map_err(RiscvCpuError::BiModeBranchPredictor)?;
            record_bimode_selected_prediction(state, prediction, predicted_taken)
        }
        RiscvSelectedBranchSpeculation::Tournament { .. } => {
            let global_history = state
                .tournament_branch_predictor
                .global_history(RISCV_LOCAL_TOURNAMENT_THREAD)
                .map_err(RiscvCpuError::TournamentBranchPredictor)?;
            let updates_local = pending_speculation_updates_tournament_local_history_from_events(
                fetch_events,
                sequence,
            )
            .ok_or(RiscvCpuError::MissingBranchSpeculationInstruction { sequence })?;
            let prediction = if updates_local {
                let local_history = state.tournament_branch_predictor.local_history(pc);
                state
                    .tournament_branch_predictor
                    .predict_with_histories_and_direction(
                        RISCV_LOCAL_TOURNAMENT_THREAD,
                        pc,
                        global_history,
                        local_history,
                        predicted_taken,
                    )
                    .map_err(RiscvCpuError::TournamentBranchPredictor)?
            } else {
                debug_assert!(predicted_taken);
                state
                    .tournament_branch_predictor
                    .predict_unconditional_with_global_history(
                        RISCV_LOCAL_TOURNAMENT_THREAD,
                        pc,
                        global_history,
                    )
                    .map_err(RiscvCpuError::TournamentBranchPredictor)?
            };
            record_tournament_selected_prediction(state, prediction, predicted_taken)
        }
    }
}

fn record_gshare_selected_prediction(
    state: &mut SelectedBranchRecordingState<'_>,
    prediction: crate::GSharePrediction,
    taken: bool,
) -> Result<RiscvSelectedBranchSpeculation, RiscvCpuError> {
    let history_update = state
        .gshare_branch_predictor
        .update_history(prediction.history(), taken)
        .map_err(RiscvCpuError::GShareBranchPredictor)?;
    Ok(RiscvSelectedBranchSpeculation::GShare {
        prediction,
        history_update: Some(history_update),
    })
}

fn record_bimode_selected_prediction(
    state: &mut SelectedBranchRecordingState<'_>,
    prediction: crate::BiModePrediction,
    taken: bool,
) -> Result<RiscvSelectedBranchSpeculation, RiscvCpuError> {
    let history_update = state
        .bimode_branch_predictor
        .update_history(prediction.history(), taken)
        .map_err(RiscvCpuError::BiModeBranchPredictor)?;
    Ok(RiscvSelectedBranchSpeculation::BiMode {
        prediction,
        history_update: Some(history_update),
    })
}

fn record_tournament_selected_prediction(
    state: &mut SelectedBranchRecordingState<'_>,
    prediction: crate::TournamentPrediction,
    taken: bool,
) -> Result<RiscvSelectedBranchSpeculation, RiscvCpuError> {
    let history_update = state
        .tournament_branch_predictor
        .update_history(prediction.history(), taken)
        .map_err(RiscvCpuError::TournamentBranchPredictor)?;
    Ok(RiscvSelectedBranchSpeculation::Tournament {
        prediction,
        history_update: Some(history_update),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvFetchAheadDecision {
    pc: Address,
    branch_speculation: Option<RiscvFetchAheadSpeculation>,
}

impl RiscvFetchAheadDecision {
    const fn straight_line(pc: Address) -> Self {
        Self {
            pc,
            branch_speculation: None,
        }
    }

    const fn branch(
        pc: Address,
        sequence: u64,
        branch_pc: Address,
        branch_kind: BranchTargetKind,
        predicted_taken: bool,
        target: Option<Address>,
        selected_speculation: Option<RiscvSelectedBranchSpeculation>,
        branch_target_prediction: Option<BranchTargetPrediction>,
        target_provider: BranchTargetProvider,
    ) -> Self {
        Self {
            pc,
            branch_speculation: Some(RiscvFetchAheadSpeculation {
                sequence,
                pc: branch_pc,
                branch_kind,
                predicted_taken,
                target,
                selected_speculation,
                branch_target_prediction,
                target_provider,
            }),
        }
    }

    pub(crate) const fn pc(&self) -> Address {
        self.pc
    }

    pub(crate) const fn branch_speculation(&self) -> Option<&RiscvFetchAheadSpeculation> {
        self.branch_speculation.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvFetchAheadSpeculation {
    sequence: u64,
    pc: Address,
    branch_kind: BranchTargetKind,
    predicted_taken: bool,
    target: Option<Address>,
    selected_speculation: Option<RiscvSelectedBranchSpeculation>,
    branch_target_prediction: Option<BranchTargetPrediction>,
    target_provider: BranchTargetProvider,
}

#[cfg(test)]
impl RiscvFetchAheadSpeculation {
    const fn sequence(&self) -> u64 {
        self.sequence
    }

    const fn pc(&self) -> Address {
        self.pc
    }

    const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    const fn target(&self) -> Option<Address> {
        self.target
    }

    const fn branch_target_prediction(&self) -> Option<BranchTargetPrediction> {
        self.branch_target_prediction
    }
}

fn hart_has_enabled_pending_interrupt(hart: &RiscvHartState) -> bool {
    let pending = hart.machine_interrupt_pending() & hart.machine_interrupt_enable();
    if pending == 0 {
        return false;
    }

    let delegated = pending & hart.machine_interrupt_delegation();
    let machine_pending = pending & !hart.machine_interrupt_delegation();
    let privilege = hart.privilege_mode();
    if machine_pending != 0 {
        match privilege {
            RiscvPrivilegeMode::User | RiscvPrivilegeMode::Supervisor => return true,
            RiscvPrivilegeMode::Machine if hart.status().mie() => return true,
            RiscvPrivilegeMode::Machine => {}
        }
    }
    if delegated != 0 {
        match privilege {
            RiscvPrivilegeMode::User => return true,
            RiscvPrivilegeMode::Supervisor if hart.status().sie() => return true,
            RiscvPrivilegeMode::Supervisor | RiscvPrivilegeMode::Machine => {}
        }
    }

    false
}

fn can_retire_completed_fetch_with_branch_speculations(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Result<bool, RiscvCpuError> {
    discard_stale_branch_speculations_before_architectural_fetch(state, fetch_events)?;
    let Some(oldest_speculation_sequence) = state.branch_speculations.keys().next().copied() else {
        return Ok(true);
    };
    if state.branch_speculations.len() < state.branch_lookahead
        && has_pending_younger_fetch(state, fetch_events, oldest_speculation_sequence)
        && completed_unexecuted_fetch_count(state, fetch_events) < completed_fetch_window(state)
    {
        return Ok(false);
    }

    Ok(
        next_completed_fetch_sequence_for_architectural_pc(state, fetch_events)
            == Some(oldest_speculation_sequence),
    )
}

fn next_fetch_ahead_candidate<'a>(
    state: &RiscvCoreState,
    completed: &'a [&'a CpuFetchEvent],
) -> Option<&'a CpuFetchEvent> {
    let architectural = Address::new(state.hart.pc());
    if let Some(fetch) = completed
        .iter()
        .copied()
        .find(|event| event.pc() == architectural)
    {
        if !state
            .branch_speculations
            .contains_key(&fetch.request_id().sequence())
        {
            return Some(fetch);
        }
    }

    let oldest_speculation = state.branch_speculations.keys().next().copied()?;
    completed.iter().copied().find(|event| {
        event.request_id().sequence() > oldest_speculation
            && !state
                .branch_speculations
                .contains_key(&event.request_id().sequence())
    })
}

fn completed_fetch_window(state: &RiscvCoreState) -> usize {
    COMPLETED_FETCH_WINDOW.max(state.branch_lookahead.saturating_add(1))
}

fn discard_stale_branch_speculations_before_architectural_fetch(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
    loop {
        let Some(oldest_sequence) = state.branch_speculations.keys().next().copied() else {
            return Ok(());
        };
        let Some(architectural_sequence) =
            next_completed_fetch_sequence_for_architectural_pc(state, fetch_events)
        else {
            return Ok(());
        };
        if oldest_sequence >= architectural_sequence {
            return Ok(());
        }
        if branch_speculation_sequence_has_live_fetch(state, fetch_events, oldest_sequence) {
            return Ok(());
        }
        discard_branch_speculation_mapping(state, oldest_sequence)?;
    }
}

fn has_pending_younger_fetch(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    oldest_speculation_sequence: u64,
) -> bool {
    fetch_events.iter().any(|event| {
        event.kind() == CpuFetchEventKind::Issued
            && event.request_id().sequence() > oldest_speculation_sequence
            && !state.executed_fetches.contains(&event.request_id())
            && !fetch_request_has_response(fetch_events, event)
    })
}

fn fetch_request_has_response(fetch_events: &[CpuFetchEvent], issued: &CpuFetchEvent) -> bool {
    fetch_events.iter().any(|event| {
        event.request_id() == issued.request_id()
            && matches!(
                event.kind(),
                CpuFetchEventKind::Completed | CpuFetchEventKind::Retry | CpuFetchEventKind::Failed
            )
    })
}

fn completed_unexecuted_fetch_count(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> usize {
    fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
        })
        .count()
}

fn branch_speculation_sequence_has_live_fetch(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
) -> bool {
    fetch_events.iter().any(|event| {
        matches!(
            event.kind(),
            CpuFetchEventKind::Issued | CpuFetchEventKind::Completed
        ) && event.request_id().sequence() == sequence
            && !state.executed_fetches.contains(&event.request_id())
    })
}

fn discard_branch_speculation_mapping(
    state: &mut RiscvCoreState,
    sequence: u64,
) -> Result<(), RiscvCpuError> {
    let Some(speculation) = state.branch_speculations.remove(&sequence) else {
        return Ok(());
    };
    state.branch_target_predictions.remove(&sequence);
    let discard = state
        .branch_predictor
        .discard_speculation(speculation)
        .map_err(RiscvCpuError::BranchPredictor)?;
    state.branch_speculations.retain(|_, pending| {
        !discard
            .removed_youngers()
            .iter()
            .any(|removed| removed.id() == *pending)
    });
    let active_sequences = state
        .branch_speculations
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    state
        .branch_target_predictions
        .retain(|sequence, _| active_sequences.contains(sequence));
    state.rollback_inactive_selected_branch_speculations(&active_sequences)?;
    Ok(())
}

fn next_completed_fetch_sequence_for_architectural_pc(
    state: &RiscvCoreState,
    fetch_events: &[crate::CpuFetchEvent],
) -> Option<u64> {
    let architectural = Address::new(state.hart.pc());
    fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.pc() == architectural
                && !state.executed_fetches.contains(&event.request_id())
        })
        .map(|event| event.request_id().sequence())
        .min()
}

fn fetch_ahead_decision(
    state: &mut RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
    sequence: u64,
    fetch_pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadDecision> {
    if instruction_allows_straight_line_fetch_ahead(instruction) {
        return Some(RiscvFetchAheadDecision::straight_line(sequential_pc));
    }
    if let Some((target, branch_kind, branch_target_prediction, target_provider)) =
        direct_jump_fetch_ahead_target(state, fetch_pc, instruction)
    {
        let selected_speculation = selected_direct_branch_speculation(state, fetch_pc)?;
        return Some(RiscvFetchAheadDecision::branch(
            target,
            sequence,
            fetch_pc,
            branch_kind,
            true,
            Some(target),
            selected_speculation,
            Some(branch_target_prediction),
            target_provider,
        ));
    }
    if !instruction_is_conditional_branch(instruction) {
        return None;
    }

    let prediction =
        selected_conditional_branch_prediction(state, completed_fetches, fetch_pc, instruction)?;
    let pc = if prediction.predicted_taken {
        prediction.target?
    } else {
        sequential_pc
    };
    Some(RiscvFetchAheadDecision::branch(
        pc,
        sequence,
        fetch_pc,
        BranchTargetKind::DirectConditional,
        prediction.predicted_taken,
        prediction.target,
        prediction.selected_speculation,
        prediction.branch_target_prediction,
        prediction.target_provider,
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvFetchAheadBranchPrediction {
    predicted_taken: bool,
    target: Option<Address>,
    selected_speculation: Option<RiscvSelectedBranchSpeculation>,
    branch_target_prediction: Option<BranchTargetPrediction>,
    target_provider: BranchTargetProvider,
}

fn selected_conditional_branch_prediction(
    state: &mut RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
    fetch_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadBranchPrediction> {
    let target_lookup = state
        .branch_target_buffer
        .lookup(fetch_pc, BranchTargetKind::DirectConditional);
    let branch_target_prediction =
        BranchTargetPrediction::new(target_lookup.hit(), target_lookup.target());
    let mut prediction = match state.branch_predictor_kind {
        RiscvBranchPredictorKind::Basic => {
            let prediction = state.branch_predictor.predict(fetch_pc);
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target: if prediction.predicted_taken() {
                    target_lookup.target().or_else(|| prediction.target())
                } else {
                    None
                },
                selected_speculation: None,
                branch_target_prediction: None,
                target_provider: BranchTargetProvider::from_btb_prediction(
                    prediction.predicted_taken(),
                    branch_target_prediction,
                ),
            })
        }
        RiscvBranchPredictorKind::GShare => {
            let global_history = selected_gshare_speculative_history(state)?;
            let prediction = state
                .gshare_branch_predictor
                .predict_with_global_history(RISCV_LOCAL_GSHARE_THREAD, fetch_pc, global_history)
                .ok()?;
            let target = prediction
                .predicted_taken()
                .then(|| conditional_branch_target(fetch_pc, instruction))
                .flatten();
            let selected_speculation = Some(RiscvSelectedBranchSpeculation::GShare {
                prediction: prediction.clone(),
                history_update: None,
            });
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
                selected_speculation,
                branch_target_prediction: None,
                target_provider: BranchTargetProvider::NoTarget,
            })
        }
        RiscvBranchPredictorKind::BiMode => {
            let global_history = selected_bimode_speculative_history(state)?;
            let prediction = state
                .bimode_branch_predictor
                .predict_with_global_history(RISCV_LOCAL_BIMODE_THREAD, fetch_pc, global_history)
                .ok()?;
            let target = prediction
                .predicted_taken()
                .then(|| conditional_branch_target(fetch_pc, instruction))
                .flatten();
            let selected_speculation = Some(RiscvSelectedBranchSpeculation::BiMode {
                prediction: prediction.clone(),
                history_update: None,
            });
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
                selected_speculation,
                branch_target_prediction: None,
                target_provider: BranchTargetProvider::NoTarget,
            })
        }
        RiscvBranchPredictorKind::Tournament => {
            let (global_history, local_history) =
                selected_tournament_speculative_histories(state, completed_fetches, fetch_pc)?;
            let prediction = state
                .tournament_branch_predictor
                .predict_with_histories(
                    RISCV_LOCAL_TOURNAMENT_THREAD,
                    fetch_pc,
                    global_history,
                    local_history,
                )
                .ok()?;
            let target = prediction
                .predicted_taken()
                .then(|| conditional_branch_target(fetch_pc, instruction))
                .flatten();
            let selected_speculation = Some(RiscvSelectedBranchSpeculation::Tournament {
                prediction: prediction.clone(),
                history_update: None,
            });
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
                selected_speculation,
                branch_target_prediction: None,
                target_provider: BranchTargetProvider::NoTarget,
            })
        }
        RiscvBranchPredictorKind::TageScL => speculation::selected_tage_sc_l_branch_prediction(
            state,
            completed_fetches,
            fetch_pc,
            instruction,
        ),
        RiscvBranchPredictorKind::MultiperspectivePerceptron => {
            let thread = selected_multiperspective_speculative_thread(state, completed_fetches)?;
            let prediction = state
                .multiperspective_perceptron
                .predict_with_thread_snapshot(
                    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
                    fetch_pc,
                    true,
                    thread,
                )
                .ok()?;
            let target = prediction
                .predicted_taken()
                .then(|| conditional_branch_target(fetch_pc, instruction))
                .flatten();
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
                selected_speculation: None,
                branch_target_prediction: None,
                target_provider: BranchTargetProvider::NoTarget,
            })
        }
    }?;
    prediction.branch_target_prediction = Some(branch_target_prediction);
    Some(prediction)
}

fn selected_direct_branch_speculation(
    state: &mut RiscvCoreState,
    fetch_pc: Address,
) -> Option<Option<RiscvSelectedBranchSpeculation>> {
    match state.branch_predictor_kind {
        RiscvBranchPredictorKind::Basic
        | RiscvBranchPredictorKind::TageScL
        | RiscvBranchPredictorKind::MultiperspectivePerceptron => Some(None),
        RiscvBranchPredictorKind::GShare => {
            let global_history = selected_gshare_speculative_history(state)?;
            let prediction = state
                .gshare_branch_predictor
                .predict_unconditional_with_global_history(
                    RISCV_LOCAL_GSHARE_THREAD,
                    fetch_pc,
                    global_history,
                )
                .ok()?;
            Some(Some(RiscvSelectedBranchSpeculation::GShare {
                prediction,
                history_update: None,
            }))
        }
        RiscvBranchPredictorKind::BiMode => {
            let global_history = selected_bimode_speculative_history(state)?;
            let prediction = state
                .bimode_branch_predictor
                .predict_unconditional_with_global_history(
                    RISCV_LOCAL_BIMODE_THREAD,
                    fetch_pc,
                    global_history,
                )
                .ok()?;
            Some(Some(RiscvSelectedBranchSpeculation::BiMode {
                prediction,
                history_update: None,
            }))
        }
        RiscvBranchPredictorKind::Tournament => {
            let global_history = selected_tournament_speculative_global_history(state)?;
            let prediction = state
                .tournament_branch_predictor
                .predict_unconditional_with_global_history(
                    RISCV_LOCAL_TOURNAMENT_THREAD,
                    fetch_pc,
                    global_history,
                )
                .ok()?;
            Some(Some(RiscvSelectedBranchSpeculation::Tournament {
                prediction,
                history_update: None,
            }))
        }
    }
}

fn selected_gshare_speculative_history(state: &RiscvCoreState) -> Option<u64> {
    let history = state
        .gshare_branch_predictor
        .global_history(RISCV_LOCAL_GSHARE_THREAD)
        .ok()?;
    selected_speculative_history(
        state,
        history,
        |speculation| matches!(speculation, RiscvSelectedBranchSpeculation::GShare { .. }),
        |history, taken| {
            state
                .gshare_branch_predictor
                .shifted_history(history, taken)
        },
    )
}

fn selected_bimode_speculative_history(state: &RiscvCoreState) -> Option<u64> {
    let history = state
        .bimode_branch_predictor
        .global_history(RISCV_LOCAL_BIMODE_THREAD)
        .ok()?;
    selected_speculative_history(
        state,
        history,
        |speculation| matches!(speculation, RiscvSelectedBranchSpeculation::BiMode { .. }),
        |history, taken| {
            state
                .bimode_branch_predictor
                .shifted_history(history, taken)
        },
    )
}

fn selected_speculative_history(
    state: &RiscvCoreState,
    mut history: u64,
    mut family_history_is_live: impl FnMut(&RiscvSelectedBranchSpeculation) -> bool,
    mut shift_history: impl FnMut(u64, bool) -> u64,
) -> Option<u64> {
    let latest_live_sequence = latest_live_selected_branch_speculation_sequence(
        &state.selected_branch_speculations,
        &mut family_history_is_live,
    );
    for (sequence, speculation) in &state.branch_speculations {
        if latest_live_sequence.is_some_and(|latest| *sequence <= latest) {
            continue;
        }
        let pending = state.branch_predictor.pending_speculation(*speculation)?;
        history = shift_history(history, pending.predicted_taken());
    }
    Some(history)
}

fn latest_live_selected_branch_speculation_sequence(
    selected_branch_speculations: &BTreeMap<u64, RiscvSelectedBranchSpeculation>,
    family_history_is_live: &mut impl FnMut(&RiscvSelectedBranchSpeculation) -> bool,
) -> Option<u64> {
    selected_branch_speculations
        .iter()
        .rev()
        .find_map(|(sequence, speculation)| {
            family_history_is_live(speculation).then_some(*sequence)
        })
}

fn selected_tournament_speculative_global_history(state: &RiscvCoreState) -> Option<u64> {
    let history = state
        .tournament_branch_predictor
        .global_history(RISCV_LOCAL_TOURNAMENT_THREAD)
        .ok()?;
    selected_speculative_history(
        state,
        history,
        |speculation| {
            matches!(
                speculation,
                RiscvSelectedBranchSpeculation::Tournament { .. }
            )
        },
        |history, taken| {
            state
                .tournament_branch_predictor
                .shifted_global_history(history, taken)
        },
    )
}

fn selected_tournament_speculative_histories(
    state: &RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
    fetch_pc: Address,
) -> Option<(u64, u64)> {
    let mut global_history = state
        .tournament_branch_predictor
        .global_history(RISCV_LOCAL_TOURNAMENT_THREAD)
        .ok()?;
    let mut local_history = state.tournament_branch_predictor.local_history(fetch_pc);
    let latest_live_sequence = latest_live_selected_branch_speculation_sequence(
        &state.selected_branch_speculations,
        &mut |speculation| {
            matches!(
                speculation,
                RiscvSelectedBranchSpeculation::Tournament { .. }
            )
        },
    );
    for (sequence, speculation) in &state.branch_speculations {
        let global_history_is_live = latest_live_sequence.is_some_and(|latest| *sequence <= latest);
        if global_history_is_live
            && state
                .selected_branch_speculations
                .get(sequence)
                .is_some_and(|speculation| {
                    matches!(
                        speculation,
                        RiscvSelectedBranchSpeculation::Tournament { .. }
                    )
                })
        {
            continue;
        }
        let pending = state.branch_predictor.pending_speculation(*speculation)?;
        if !global_history_is_live {
            global_history = state
                .tournament_branch_predictor
                .shifted_global_history(global_history, pending.predicted_taken());
        }
        if state
            .tournament_branch_predictor
            .shares_local_history_entry(pending.pc(), fetch_pc)
            && pending_speculation_updates_tournament_local_history(completed_fetches, *sequence)?
        {
            local_history = state
                .tournament_branch_predictor
                .shifted_local_history(local_history, pending.predicted_taken());
        }
    }
    Some((global_history, local_history))
}

fn selected_multiperspective_speculative_thread(
    state: &RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
) -> Option<MultiperspectivePerceptronThreadSnapshot> {
    let mut thread = state
        .multiperspective_perceptron
        .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
        .ok()?
        .clone();
    for (sequence, speculation) in &state.branch_speculations {
        let pending = state.branch_predictor.pending_speculation(*speculation)?;
        let target = pending.target().or_else(|| {
            completed_fetch_instruction(completed_fetches, *sequence)
                .and_then(|instruction| conditional_branch_target(pending.pc(), instruction))
        })?;
        thread = state.multiperspective_perceptron.shifted_thread_snapshot(
            thread,
            pending.pc(),
            pending.predicted_taken(),
            target,
        );
    }
    Some(thread)
}

fn pending_speculation_updates_tournament_local_history(
    completed_fetches: &[&CpuFetchEvent],
    sequence: u64,
) -> Option<bool> {
    Some(instruction_is_conditional_branch(
        completed_fetch_instruction(completed_fetches, sequence)?,
    ))
}

fn pending_speculation_updates_tournament_local_history_from_events(
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
) -> Option<bool> {
    Some(instruction_is_conditional_branch(
        completed_fetch_instruction_from_events(fetch_events, sequence)?,
    ))
}

fn completed_fetch_instruction(
    completed_fetches: &[&CpuFetchEvent],
    sequence: u64,
) -> Option<RiscvInstruction> {
    let fetch = completed_fetches
        .iter()
        .copied()
        .find(|event| event.request_id().sequence() == sequence)?;
    decode_completed_fetch_instruction(fetch)
}

fn completed_fetch_instruction_from_events(
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
) -> Option<RiscvInstruction> {
    let fetch = fetch_events.iter().find(|event| {
        event.kind() == CpuFetchEventKind::Completed && event.request_id().sequence() == sequence
    })?;
    decode_completed_fetch_instruction(fetch)
}

fn decode_completed_fetch_instruction(fetch: &CpuFetchEvent) -> Option<RiscvInstruction> {
    let data = fetch.data()?;
    let raw = match data {
        [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
        _ => return None,
    };
    let decoded = RiscvInstruction::decode_with_length(raw).ok()?;
    Some(decoded.instruction())
}

fn conditional_branch_target(fetch_pc: Address, instruction: RiscvInstruction) -> Option<Address> {
    let offset = match instruction {
        RiscvInstruction::Beq { offset, .. }
        | RiscvInstruction::Bne { offset, .. }
        | RiscvInstruction::Blt { offset, .. }
        | RiscvInstruction::Bge { offset, .. }
        | RiscvInstruction::Bltu { offset, .. }
        | RiscvInstruction::Bgeu { offset, .. } => offset.value(),
        _ => return None,
    };
    checked_add_signed(fetch_pc.get(), offset).map(Address::new)
}

fn direct_jump_fetch_ahead_target(
    state: &mut RiscvCoreState,
    fetch_pc: Address,
    instruction: RiscvInstruction,
) -> Option<(
    Address,
    BranchTargetKind,
    BranchTargetPrediction,
    BranchTargetProvider,
)> {
    let kind = match instruction {
        RiscvInstruction::Jal { .. } | RiscvInstruction::Jalr { .. } => {
            riscv_branch_target_kind(instruction)
        }
        _ => return None,
    };
    let target_lookup = state.branch_target_buffer.lookup(fetch_pc, kind);
    let branch_target_prediction =
        BranchTargetPrediction::new(target_lookup.hit(), target_lookup.target());
    let target = match instruction {
        RiscvInstruction::Jal { offset, .. } => {
            checked_add_signed(fetch_pc.get(), offset.value()).map(Address::new)
        }
        RiscvInstruction::Jalr { rs1, offset, .. } => {
            checked_add_signed(state.hart.read(rs1), offset.value())
                .map(|target| Address::new(target & !1))
        }
        _ => None,
    }?;
    Some((
        target,
        kind,
        branch_target_prediction,
        BranchTargetProvider::NoTarget,
    ))
}

fn checked_add_signed(value: u64, offset: i64) -> Option<u64> {
    if offset >= 0 {
        value.checked_add(offset as u64)
    } else {
        value.checked_sub(offset.unsigned_abs())
    }
}

fn instruction_allows_straight_line_fetch_ahead(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Lui { .. }
            | RiscvInstruction::Auipc { .. }
            | RiscvInstruction::Addi { .. }
            | RiscvInstruction::Slti { .. }
            | RiscvInstruction::Sltiu { .. }
            | RiscvInstruction::Xori { .. }
            | RiscvInstruction::Ori { .. }
            | RiscvInstruction::Andi { .. }
            | RiscvInstruction::Slli { .. }
            | RiscvInstruction::Srli { .. }
            | RiscvInstruction::Srai { .. }
            | RiscvInstruction::Addiw { .. }
            | RiscvInstruction::Slliw { .. }
            | RiscvInstruction::Srliw { .. }
            | RiscvInstruction::Sraiw { .. }
            | RiscvInstruction::Add { .. }
            | RiscvInstruction::Sub { .. }
            | RiscvInstruction::Sll { .. }
            | RiscvInstruction::Slt { .. }
            | RiscvInstruction::Sltu { .. }
            | RiscvInstruction::Xor { .. }
            | RiscvInstruction::Srl { .. }
            | RiscvInstruction::Sra { .. }
            | RiscvInstruction::Or { .. }
            | RiscvInstruction::And { .. }
            | RiscvInstruction::Mul { .. }
            | RiscvInstruction::Mulh { .. }
            | RiscvInstruction::Mulhsu { .. }
            | RiscvInstruction::Mulhu { .. }
            | RiscvInstruction::Div { .. }
            | RiscvInstruction::Divu { .. }
            | RiscvInstruction::Rem { .. }
            | RiscvInstruction::Remu { .. }
            | RiscvInstruction::Mulw { .. }
            | RiscvInstruction::Divw { .. }
            | RiscvInstruction::Divuw { .. }
            | RiscvInstruction::Remw { .. }
            | RiscvInstruction::Remuw { .. }
            | RiscvInstruction::Addw { .. }
            | RiscvInstruction::Subw { .. }
            | RiscvInstruction::Sllw { .. }
            | RiscvInstruction::Srlw { .. }
            | RiscvInstruction::Sraw { .. }
            | RiscvInstruction::VectorSetVli { .. }
            | RiscvInstruction::VectorSetIvli { .. }
            | RiscvInstruction::VectorSetVl { .. }
            | RiscvInstruction::VectorFloat(_)
            | RiscvInstruction::VectorAddVv { .. }
            | RiscvInstruction::VectorAddVx { .. }
            | RiscvInstruction::VectorAddVi { .. }
            | RiscvInstruction::VectorSubVv { .. }
            | RiscvInstruction::VectorSubVx { .. }
            | RiscvInstruction::VectorMinUnsignedVv { .. }
            | RiscvInstruction::VectorMinUnsignedVx { .. }
            | RiscvInstruction::VectorMinSignedVv { .. }
            | RiscvInstruction::VectorMinSignedVx { .. }
            | RiscvInstruction::VectorMaxUnsignedVv { .. }
            | RiscvInstruction::VectorMaxUnsignedVx { .. }
            | RiscvInstruction::VectorMaxSignedVv { .. }
            | RiscvInstruction::VectorMaxSignedVx { .. }
            | RiscvInstruction::VectorMultiplyLowVv { .. }
            | RiscvInstruction::VectorMultiplyLowVx { .. }
            | RiscvInstruction::VectorMultiplyHighUnsignedVv { .. }
            | RiscvInstruction::VectorMultiplyHighUnsignedVx { .. }
            | RiscvInstruction::VectorMultiplyHighSignedUnsignedVv { .. }
            | RiscvInstruction::VectorMultiplyHighSignedUnsignedVx { .. }
            | RiscvInstruction::VectorMultiplyHighSignedVv { .. }
            | RiscvInstruction::VectorMultiplyHighSignedVx { .. }
            | RiscvInstruction::VectorDivideUnsignedVv { .. }
            | RiscvInstruction::VectorDivideUnsignedVx { .. }
            | RiscvInstruction::VectorDivideSignedVv { .. }
            | RiscvInstruction::VectorDivideSignedVx { .. }
            | RiscvInstruction::VectorRemainderUnsignedVv { .. }
            | RiscvInstruction::VectorRemainderUnsignedVx { .. }
            | RiscvInstruction::VectorRemainderSignedVv { .. }
            | RiscvInstruction::VectorRemainderSignedVx { .. }
            | RiscvInstruction::VectorIntegerCarryBorrow(..)
            | RiscvInstruction::VectorIntegerMultiplyAdd(..)
            | RiscvInstruction::VectorSlide(_)
            | RiscvInstruction::VectorGather(_)
            | RiscvInstruction::VectorMaskPrefix(_)
            | RiscvInstruction::VectorMaskIndex(_)
            | RiscvInstruction::VectorMergeVvm { .. }
            | RiscvInstruction::VectorMergeVxm { .. }
            | RiscvInstruction::VectorMergeVim { .. }
            | RiscvInstruction::VectorCompressVm(..)
            | RiscvInstruction::VectorNarrow(..)
            | RiscvInstruction::VectorAveraging(..)
            | RiscvInstruction::VectorFixedPointShift(..)
            | RiscvInstruction::VectorReduction(..)
            | RiscvInstruction::VectorSaturating(..)
            | RiscvInstruction::VectorMoveVv { .. }
            | RiscvInstruction::VectorMoveVx { .. }
            | RiscvInstruction::VectorMoveVi { .. }
            | RiscvInstruction::VectorScalarMove(_)
            | RiscvInstruction::VectorWholeMove(_)
            | RiscvInstruction::VectorMaskAndMm { .. }
            | RiscvInstruction::VectorMaskNandMm { .. }
            | RiscvInstruction::VectorMaskAndNotMm { .. }
            | RiscvInstruction::VectorMaskXorMm { .. }
            | RiscvInstruction::VectorMaskOrMm { .. }
            | RiscvInstruction::VectorMaskNorMm { .. }
            | RiscvInstruction::VectorMaskOrNotMm { .. }
            | RiscvInstruction::VectorMaskXnorMm { .. }
            | RiscvInstruction::VectorMaskReduction(_)
            | RiscvInstruction::VectorMaskEqualVv { .. }
            | RiscvInstruction::VectorMaskEqualVx { .. }
            | RiscvInstruction::VectorMaskEqualVi { .. }
            | RiscvInstruction::VectorMaskNotEqualVv { .. }
            | RiscvInstruction::VectorMaskNotEqualVx { .. }
            | RiscvInstruction::VectorMaskNotEqualVi { .. }
            | RiscvInstruction::VectorMaskLessUnsignedVv { .. }
            | RiscvInstruction::VectorMaskLessUnsignedVx { .. }
            | RiscvInstruction::VectorMaskLessSignedVv { .. }
            | RiscvInstruction::VectorMaskLessSignedVx { .. }
            | RiscvInstruction::VectorMaskLessEqualUnsignedVv { .. }
            | RiscvInstruction::VectorMaskLessEqualUnsignedVx { .. }
            | RiscvInstruction::VectorMaskLessEqualUnsignedVi { .. }
            | RiscvInstruction::VectorMaskLessEqualSignedVv { .. }
            | RiscvInstruction::VectorMaskLessEqualSignedVx { .. }
            | RiscvInstruction::VectorMaskLessEqualSignedVi { .. }
            | RiscvInstruction::VectorMaskGreaterUnsignedVx { .. }
            | RiscvInstruction::VectorMaskGreaterUnsignedVi { .. }
            | RiscvInstruction::VectorMaskGreaterSignedVx { .. }
            | RiscvInstruction::VectorMaskGreaterSignedVi { .. }
            | RiscvInstruction::VectorAndVv { .. }
            | RiscvInstruction::VectorAndVx { .. }
            | RiscvInstruction::VectorAndVi { .. }
            | RiscvInstruction::VectorOrVv { .. }
            | RiscvInstruction::VectorOrVx { .. }
            | RiscvInstruction::VectorOrVi { .. }
            | RiscvInstruction::VectorXorVv { .. }
            | RiscvInstruction::VectorXorVx { .. }
            | RiscvInstruction::VectorXorVi { .. }
            | RiscvInstruction::VectorShiftLeftLogicalVv { .. }
            | RiscvInstruction::VectorShiftLeftLogicalVx { .. }
            | RiscvInstruction::VectorShiftLeftLogicalVi { .. }
            | RiscvInstruction::VectorShiftRightLogicalVv { .. }
            | RiscvInstruction::VectorShiftRightLogicalVx { .. }
            | RiscvInstruction::VectorShiftRightLogicalVi { .. }
            | RiscvInstruction::VectorShiftRightArithmeticVv { .. }
            | RiscvInstruction::VectorShiftRightArithmeticVx { .. }
            | RiscvInstruction::VectorShiftRightArithmeticVi { .. }
    )
}

fn instruction_is_conditional_branch(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Beq { .. }
            | RiscvInstruction::Bne { .. }
            | RiscvInstruction::Blt { .. }
            | RiscvInstruction::Bge { .. }
            | RiscvInstruction::Bltu { .. }
            | RiscvInstruction::Bgeu { .. }
    )
}

#[cfg(test)]
mod tests;
