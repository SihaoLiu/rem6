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
mod tests {
    use super::*;
    use crate::{
        BranchPredictor, BranchPredictorCheckpointPayload, BranchPredictorConfig,
        BranchTargetBuffer, BranchTargetBufferConfig, BranchTargetProvider, CpuCore,
        CpuFetchConfig, CpuFetchRecord, CpuId, CpuResetState, InOrderPipelineError,
        InOrderPipelineInstruction, InOrderPipelineSnapshot, InOrderPipelineStage,
        MultiperspectivePerceptron, MultiperspectivePerceptronConfig,
        MultiperspectivePerceptronFeature, OutstandingFetch, RiscvBranchPredictorKind,
        TournamentBranchPredictor, TournamentBranchPredictorConfig,
        DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES, RISCV_LOCAL_BIMODE_THREAD,
        RISCV_LOCAL_GSHARE_THREAD, RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
        RISCV_LOCAL_TOURNAMENT_THREAD,
    };
    use rem6_isa_riscv::Register;
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId, CacheLineLayout, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    fn endpoint(name: &str) -> TransportEndpointId {
        TransportEndpointId::new(name).unwrap()
    }

    fn layout() -> CacheLineLayout {
        CacheLineLayout::new(16).unwrap()
    }

    fn request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    fn b_type(offset: i32, rs1: u8, rs2: u8, funct3: u32) -> u32 {
        let imm = offset as u32;
        ((imm & 0x1000) << 19)
            | ((imm & 0x07e0) << 20)
            | (u32::from(rs2) << 20)
            | (u32::from(rs1) << 15)
            | (funct3 << 12)
            | ((imm & 0x001e) << 7)
            | ((imm & 0x0800) >> 4)
            | 0x63
    }

    fn j_type(offset: i32, rd: u8) -> u32 {
        let imm = offset as u32;
        (((imm >> 20) & 0x1) << 31)
            | (((imm >> 1) & 0x3ff) << 21)
            | (((imm >> 11) & 0x1) << 20)
            | (((imm >> 12) & 0xff) << 12)
            | (u32::from(rd) << 7)
            | 0x6f
    }

    fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
        ((imm as u32 & 0x0fff) << 20)
            | (u32::from(rs1) << 15)
            | (funct3 << 12)
            | (u32::from(rd) << 7)
            | opcode
    }

    fn completed(sequence: u64, pc: u64) -> crate::CpuFetchEvent {
        crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            vec![0; 4],
        )
    }

    fn core_with_completed_fetch(data: Vec<u8>) -> RiscvCore {
        core_with_completed_fetches([(0, 0x8000, data)])
    }

    fn btb_entry_kind(core: &RiscvCore, pc: u64) -> Option<BranchTargetKind> {
        core.branch_target_buffer_snapshot()
            .entries()
            .iter()
            .flatten()
            .find(|entry| entry.pc() == Address::new(pc))
            .map(|entry| entry.kind())
    }

    fn core_with_completed_fetches(
        fetches: impl IntoIterator<Item = (u64, u64, Vec<u8>)>,
    ) -> RiscvCore {
        let core = RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(0),
                    PartitionId::new(0),
                    AgentId::new(7),
                    Address::new(0x8000),
                ),
                CpuFetchConfig::new(
                    endpoint("cpu0.ifetch"),
                    MemoryRouteId::new(0),
                    layout(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
        );
        let mut core_state = core.core.state.lock().expect("cpu core lock");
        for (sequence, pc, data) in fetches {
            core_state.events.push(crate::CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    4,
                    PartitionId::new(0),
                    MemoryRouteId::new(0),
                    endpoint("cpu0.ifetch"),
                    request(sequence),
                    Address::new(pc),
                    AccessSize::new(4).unwrap(),
                ),
                data,
            ));
        }
        drop(core_state);
        core
    }

    fn core_with_recorded_selected_direct_speculation(kind: RiscvBranchPredictorKind) -> RiscvCore {
        let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());
        core.set_branch_predictor_kind(kind);
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        record_fetch_ahead_speculation(&core, &decision).unwrap();
        core
    }

    fn record_fetch_ahead_speculation(
        core: &RiscvCore,
        decision: &RiscvFetchAheadDecision,
    ) -> Result<(), RiscvCpuError> {
        let prepared = core.prepare_fetch_ahead_speculation(decision)?;
        core.record_prepared_fetch_ahead_speculation(prepared);
        Ok(())
    }

    fn selected_family_global_history(
        state: &RiscvCoreState,
        kind: RiscvBranchPredictorKind,
    ) -> u64 {
        match kind {
            RiscvBranchPredictorKind::GShare => {
                state.gshare_branch_predictor.snapshot().threads()[0].global_history()
            }
            RiscvBranchPredictorKind::BiMode => {
                state.bimode_branch_predictor.snapshot().threads()[0].global_history()
            }
            RiscvBranchPredictorKind::Tournament => {
                state.tournament_branch_predictor.snapshot().threads()[0].global_history()
            }
            other => panic!("unsupported selected predictor family: {other:?}"),
        }
    }

    fn selected_family_speculation_count(
        state: &RiscvCoreState,
        kind: RiscvBranchPredictorKind,
    ) -> usize {
        state
            .selected_branch_speculations
            .values()
            .filter(|speculation| {
                matches!(
                    (kind, speculation),
                    (
                        RiscvBranchPredictorKind::GShare,
                        RiscvSelectedBranchSpeculation::GShare { .. }
                    ) | (
                        RiscvBranchPredictorKind::BiMode,
                        RiscvSelectedBranchSpeculation::BiMode { .. }
                    ) | (
                        RiscvBranchPredictorKind::Tournament,
                        RiscvSelectedBranchSpeculation::Tournament { .. }
                    )
                )
            })
            .count()
    }

    fn restore_selected_family_checkpoint(core: &RiscvCore, kind: RiscvBranchPredictorKind) {
        match kind {
            RiscvBranchPredictorKind::GShare => core
                .restore_gshare_branch_predictor_checkpoint_payload(
                    core.gshare_branch_predictor_checkpoint_payload(),
                )
                .unwrap(),
            RiscvBranchPredictorKind::BiMode => core
                .restore_bimode_branch_predictor_checkpoint_payload(
                    core.bimode_branch_predictor_checkpoint_payload(),
                )
                .unwrap(),
            RiscvBranchPredictorKind::Tournament => core
                .restore_tournament_branch_predictor_checkpoint_payload(
                    core.tournament_branch_predictor_checkpoint_payload(),
                )
                .unwrap(),
            other => panic!("unsupported selected predictor family: {other:?}"),
        }
    }

    fn train_selected_gshare_taken(state: &mut RiscvCoreState, pc: Address) {
        for _ in 0..2 {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
                .unwrap();
            state
                .gshare_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        let trained = state
            .gshare_branch_predictor
            .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
            .unwrap();
        assert!(trained.predicted_taken());
    }

    fn train_selected_bimode_taken(state: &mut RiscvCoreState, pc: Address) {
        for _ in 0..4 {
            let prediction = state
                .bimode_branch_predictor
                .predict(RISCV_LOCAL_BIMODE_THREAD, pc)
                .unwrap();
            state
                .bimode_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        let trained = state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, pc)
            .unwrap();
        assert!(trained.predicted_taken());
    }

    fn use_small_tournament_predictor(state: &mut RiscvCoreState) {
        state.tournament_branch_predictor = TournamentBranchPredictor::new(
            TournamentBranchPredictorConfig::new(1, 2, 2, 2, 2).unwrap(),
        );
    }

    fn use_local_bias_multiperspective_perceptron(state: &mut RiscvCoreState) {
        state.multiperspective_perceptron = MultiperspectivePerceptron::new(
            MultiperspectivePerceptronConfig::with_options(
                1,
                0,
                1,
                1,
                16,
                -4,
                1,
                -5,
                5,
                -1,
                1,
                1,
                4,
                -2,
                0,
                0,
                0,
                64,
                2,
                2,
                0,
                0xff,
                false,
                true,
                0,
                4,
                3,
                128,
                1,
                false,
                vec![MultiperspectivePerceptronFeature::bias(64, 1, 6)],
            )
            .unwrap(),
        )
        .unwrap();
    }

    fn train_selected_tournament_local_history_one_taken(state: &mut RiscvCoreState, pc: Address) {
        let history_seed = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
            .unwrap();
        state
            .tournament_branch_predictor
            .update_history(history_seed.history(), true)
            .unwrap();
        for _ in 0..2 {
            let prediction = state
                .tournament_branch_predictor
                .predict(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
                .unwrap();
            assert_eq!(prediction.local_history_before(), 1);
            assert_eq!(prediction.local_predictor_index(), 1);
            state
                .tournament_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        state
            .tournament_branch_predictor
            .squash(history_seed.history())
            .unwrap();
    }

    fn train_selected_tournament_global_history_one_taken(
        state: &mut RiscvCoreState,
        training_pc: Address,
    ) {
        let history_seed = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, training_pc)
            .unwrap();
        state
            .tournament_branch_predictor
            .update_history(history_seed.history(), true)
            .unwrap();
        for _ in 0..2 {
            let prediction = state
                .tournament_branch_predictor
                .predict_unconditional(RISCV_LOCAL_TOURNAMENT_THREAD, Address::new(0xa000))
                .unwrap();
            assert_eq!(prediction.global_history_before(), 1);
            state
                .tournament_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        for _ in 0..2 {
            let prediction = state
                .tournament_branch_predictor
                .predict(RISCV_LOCAL_TOURNAMENT_THREAD, training_pc)
                .unwrap();
            assert_eq!(prediction.global_history_before(), 1);
            assert_eq!(prediction.local_history_before(), 1);
            assert!(!prediction.local_predicted_taken());
            assert!(prediction.global_predicted_taken());
            state
                .tournament_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        state
            .tournament_branch_predictor
            .squash(history_seed.history())
            .unwrap();
    }

    fn insert_pending_branch_speculation(
        state: &mut RiscvCoreState,
        sequence: u64,
        pc: Address,
        target: Address,
    ) {
        let speculation =
            state
                .branch_predictor
                .predict_speculative_with_prediction(pc, true, Some(target));
        state.branch_speculations.insert(sequence, speculation.id());
    }

    #[test]
    fn fetch_ahead_accepts_compressed_straight_line_instruction() {
        let mut fetch_data = Vec::new();
        fetch_data.extend_from_slice(&0x0001_u16.to_le_bytes());
        fetch_data.extend_from_slice(&0x0000_0073_u32.to_le_bytes()[..2]);
        let core = core_with_completed_fetch(fetch_data);

        assert_eq!(
            core.next_fetch_ahead_before_retire()
                .map(|decision| decision.pc()),
            Some(Address::new(0x8002))
        );
    }

    #[test]
    fn fetch_ahead_uses_direct_jal_target() {
        let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());

        let decision = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(decision.pc(), Address::new(0x800c));
        assert_eq!(
            decision.branch_speculation().map(|speculation| {
                (
                    speculation.sequence(),
                    speculation.pc(),
                    speculation.predicted_taken(),
                    speculation.target(),
                )
            }),
            Some((0, Address::new(0x8000), true, Some(Address::new(0x800c))))
        );
    }

    #[test]
    fn selected_gshare_speculation_controls_retire_branch_prediction() {
        let branch = b_type(8, 0, 0, 0x1).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            train_selected_gshare_taken(&mut state, Address::new(0x8000));
        }

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x8008));
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let basic_update = event.branch_update().unwrap();
        assert!(!basic_update.predicted_taken());

        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
        assert!(!prediction.resolved_taken());
        assert_eq!(prediction.repair_target_pc(), Some(0x8004));
        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 1);
        assert_eq!(
            summary
                .lookup_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.lookup_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .committed_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.committed_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .mispredicted_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.mispredicted_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .corrected_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.corrected_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .target_wrong_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.target_wrong_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .target_provider()
                .value(BranchTargetProvider::NoTarget),
            1
        );
        assert_eq!(
            summary.target_provider().value(BranchTargetProvider::BTB),
            0
        );
        assert_eq!(summary.target_provider().total(), 1);
        assert_eq!(
            summary
                .mispredict_due_to_predictor()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.mispredict_due_to_predictor().total(), 1);
        assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 0);
    }

    #[test]
    fn selected_gshare_direct_speculation_redirect_discards_live_history() {
        let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
        {
            let state = core.state.lock().expect("riscv core lock");
            assert_eq!(state.selected_branch_speculations.len(), 1);
            assert_eq!(
                state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
                1
            );
        }

        core.redirect_pc(Address::new(0x9000));

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.branch_speculations.is_empty());
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }

    #[test]
    fn selected_record_failure_does_not_leave_generic_branch_speculation() {
        let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let prediction = state
                .gshare_branch_predictor
                .predict_unconditional(RISCV_LOCAL_GSHARE_THREAD, Address::new(0x9000))
                .unwrap();
            state
                .gshare_branch_predictor
                .update_history(prediction.history(), true)
                .unwrap();
            assert_eq!(
                state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
                1
            );
        }

        assert!(record_fetch_ahead_speculation(&core, &decision).is_err());

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.branch_speculations.is_empty());
        assert!(state.selected_branch_speculations.is_empty());
        assert!(state.branch_predictor.pending_speculations().is_empty());
    }

    #[test]
    fn prepared_fetch_issue_records_fetch_ahead_speculation_before_sync_failure() {
        let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        let prepared = core.prepare_fetch_ahead_speculation(&decision).unwrap();
        let config = RiscvCore::default_in_order_pipeline_snapshot()
            .config()
            .clone();
        core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
            config,
            u64::MAX,
            [InOrderPipelineInstruction::new(
                99,
                InOrderPipelineStage::Fetch1,
            )],
        ))
        .unwrap();
        let issue = OutstandingFetch {
            tick: 4,
            partition: PartitionId::new(0),
            route: MemoryRouteId::new(0),
            endpoint: endpoint("cpu0.ifetch"),
            request_id: request(1),
            pc: decision.pc(),
            size: AccessSize::new(4).unwrap(),
            line_layout: layout(),
        };

        let error = core
            .record_prepared_fetch_issue_with_prepared_fetch_ahead(issue, prepared)
            .unwrap_err();

        assert!(matches!(
            error,
            RiscvCpuError::InOrderPipeline(InOrderPipelineError::CycleCursorOverflow {
                cycle: u64::MAX
            })
        ));
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.branch_speculations.contains_key(&0));
        assert!(state.branch_predictor.pending_speculation_count() > 0);
    }

    #[test]
    fn selected_gshare_direct_speculation_after_restore_uses_pending_generic_history() {
        let core = core_with_completed_fetch(j_type(8, 0).to_le_bytes().to_vec());
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        let selected = {
            let mut state = core.state.lock().expect("riscv core lock");
            insert_pending_branch_speculation(
                &mut state,
                0,
                Address::new(0x8000),
                Address::new(0x8008),
            );
            assert_eq!(state.branch_speculations.len(), 1);
            assert!(state.selected_branch_speculations.is_empty());
            assert_eq!(
                state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );

            selected_direct_branch_speculation(&mut state, Address::new(0x8008))
                .unwrap()
                .expect("selected direct speculation")
        };
        if let RiscvSelectedBranchSpeculation::GShare { prediction, .. } = &selected {
            assert_eq!(prediction.global_history_before(), 1);
        } else {
            panic!("expected selected GShare speculation");
        }
        let decision = RiscvFetchAheadDecision::branch(
            Address::new(0x8010),
            1,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            true,
            Some(Address::new(0x8010)),
            Some(selected),
            None,
            BranchTargetProvider::NoTarget,
        );

        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 2);
        assert_eq!(state.selected_branch_speculations.len(), 2);
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            3
        );
    }

    #[test]
    fn selected_tournament_direct_replay_after_restore_keeps_local_history_unchanged() {
        let core = core_with_completed_fetches([
            (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
            (1, 0x8008, j_type(8, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        let selected = {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            insert_pending_branch_speculation(
                &mut state,
                0,
                Address::new(0x8000),
                Address::new(0x8008),
            );
            assert_eq!(state.branch_speculations.len(), 1);
            assert!(state.selected_branch_speculations.is_empty());
            assert_eq!(
                state
                    .tournament_branch_predictor
                    .snapshot()
                    .local_history_table()[0],
                0
            );

            selected_direct_branch_speculation(&mut state, Address::new(0x8008))
                .unwrap()
                .expect("selected direct speculation")
        };
        if let RiscvSelectedBranchSpeculation::Tournament { prediction, .. } = &selected {
            assert_eq!(prediction.global_history_before(), 1);
            assert!(!prediction.local_history_valid());
        } else {
            panic!("expected selected Tournament speculation");
        }
        let decision = RiscvFetchAheadDecision::branch(
            Address::new(0x8010),
            1,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            true,
            Some(Address::new(0x8010)),
            Some(selected),
            None,
            BranchTargetProvider::NoTarget,
        );

        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 2);
        assert_eq!(state.selected_branch_speculations.len(), 2);
        let replayed = state.selected_branch_speculations.get(&0).unwrap();
        let RiscvSelectedBranchSpeculation::Tournament { prediction, .. } = replayed else {
            panic!("expected replayed Tournament speculation");
        };
        assert!(!prediction.local_history_valid());
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            1
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .history_update_count(),
            2
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[0],
            0
        );
    }

    #[test]
    fn selected_tournament_replay_uses_completed_event_after_issued_event() {
        let core = core_with_completed_fetches(std::iter::empty::<(u64, u64, Vec<u8>)>());
        {
            let mut core_state = core.core.state.lock().expect("cpu core lock");
            core_state
                .events
                .push(crate::CpuFetchEvent::issued(CpuFetchRecord::new(
                    3,
                    PartitionId::new(0),
                    MemoryRouteId::new(0),
                    endpoint("cpu0.ifetch"),
                    request(0),
                    Address::new(0x8000),
                    AccessSize::new(4).unwrap(),
                )));
            core_state.events.push(crate::CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    4,
                    PartitionId::new(0),
                    MemoryRouteId::new(0),
                    endpoint("cpu0.ifetch"),
                    request(0),
                    Address::new(0x8000),
                    AccessSize::new(4).unwrap(),
                ),
                j_type(8, 0).to_le_bytes().to_vec(),
            ));
        }
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        let selected = {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            insert_pending_branch_speculation(
                &mut state,
                0,
                Address::new(0x8000),
                Address::new(0x8008),
            );
            selected_direct_branch_speculation(&mut state, Address::new(0x8008))
                .unwrap()
                .expect("selected direct speculation")
        };
        let decision = RiscvFetchAheadDecision::branch(
            Address::new(0x8010),
            1,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            true,
            Some(Address::new(0x8010)),
            Some(selected),
            None,
            BranchTargetProvider::NoTarget,
        );

        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 2);
        assert_eq!(state.selected_branch_speculations.len(), 2);
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .history_update_count(),
            2
        );
    }

    #[test]
    fn selected_tournament_replay_requires_completed_instruction_metadata() {
        let core = core_with_completed_fetches([(1, 0x8008, j_type(8, 0).to_le_bytes().to_vec())]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        let selected = {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            insert_pending_branch_speculation(
                &mut state,
                0,
                Address::new(0x8000),
                Address::new(0x8008),
            );
            selected_direct_branch_speculation(&mut state, Address::new(0x8008))
                .unwrap()
                .expect("selected direct speculation")
        };
        let decision = RiscvFetchAheadDecision::branch(
            Address::new(0x8010),
            1,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            true,
            Some(Address::new(0x8010)),
            Some(selected),
            None,
            BranchTargetProvider::NoTarget,
        );

        let error = record_fetch_ahead_speculation(&core, &decision).unwrap_err();

        assert!(matches!(
            error,
            RiscvCpuError::MissingBranchSpeculationInstruction { sequence: 0 }
        ));
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 1);
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(state.branch_predictor.pending_speculation_count(), 1);
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .history_update_count(),
            0
        );
    }

    #[test]
    fn selected_tournament_replay_failure_discards_partial_recording() {
        let core = core_with_completed_fetches([(0, 0x8000, j_type(8, 0).to_le_bytes().to_vec())]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        let selected = {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            insert_pending_branch_speculation(
                &mut state,
                0,
                Address::new(0x8000),
                Address::new(0x8008),
            );
            insert_pending_branch_speculation(
                &mut state,
                1,
                Address::new(0x8008),
                Address::new(0x8010),
            );
            selected_direct_branch_speculation(&mut state, Address::new(0x8010))
                .unwrap()
                .expect("selected direct speculation")
        };
        let decision = RiscvFetchAheadDecision::branch(
            Address::new(0x8018),
            2,
            Address::new(0x8010),
            BranchTargetKind::DirectUnconditional,
            true,
            Some(Address::new(0x8018)),
            Some(selected),
            None,
            BranchTargetProvider::NoTarget,
        );

        let error = record_fetch_ahead_speculation(&core, &decision).unwrap_err();

        assert!(matches!(
            error,
            RiscvCpuError::MissingBranchSpeculationInstruction { sequence: 1 }
        ));
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 2);
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(state.branch_predictor.pending_speculation_count(), 2);
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .history_update_count(),
            0
        );
    }

    #[test]
    fn generic_branch_checkpoint_restore_rolls_back_selected_family_history() {
        let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
        assert_eq!(
            core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );

        core.restore_branch_predictor_checkpoint_payload(
            RiscvCore::default_branch_predictor_checkpoint_payload(),
        )
        .unwrap();

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.branch_speculations.is_empty());
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }

    #[test]
    fn selected_family_checkpoint_payloads_use_committed_fetch_ahead_history() {
        let gshare =
            core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
        assert_eq!(
            gshare.gshare_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        assert_eq!(
            gshare
                .gshare_branch_predictor_checkpoint_payload()
                .snapshot()
                .threads()[0]
                .global_history(),
            0
        );

        let bimode =
            core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::BiMode);
        assert_eq!(
            bimode.bimode_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        assert_eq!(
            bimode
                .bimode_branch_predictor_checkpoint_payload()
                .snapshot()
                .threads()[0]
                .global_history(),
            0
        );

        let tournament =
            core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::Tournament);
        assert_eq!(
            tournament.tournament_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        assert_eq!(
            tournament
                .tournament_branch_predictor_checkpoint_payload()
                .snapshot()
                .threads()[0]
                .global_history(),
            0
        );
    }

    #[test]
    fn selected_family_checkpoint_restore_forgets_unreflected_selected_speculation() {
        for kind in [
            RiscvBranchPredictorKind::GShare,
            RiscvBranchPredictorKind::BiMode,
            RiscvBranchPredictorKind::Tournament,
        ] {
            let core = core_with_recorded_selected_direct_speculation(kind);
            {
                let state = core.state.lock().expect("riscv core lock");
                assert_eq!(selected_family_speculation_count(&state, kind), 1);
                assert_eq!(selected_family_global_history(&state, kind), 1);
            }

            restore_selected_family_checkpoint(&core, kind);

            let state = core.state.lock().expect("riscv core lock");
            assert_eq!(selected_family_speculation_count(&state, kind), 0);
            assert_eq!(selected_family_global_history(&state, kind), 0);
        }
    }

    #[test]
    fn btb_misprediction_counts_taken_fetch_prediction_without_btb_target() {
        let branch = b_type(8, 0, 0, 0x0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let pc = Address::new(0x8000);
            for _ in 0..2 {
                let prediction = state
                    .gshare_branch_predictor
                    .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
                    .unwrap();
                state
                    .gshare_branch_predictor
                    .train(prediction.history(), true, false)
                    .unwrap();
            }
        }

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x8008));
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
        assert!(prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
        assert!(!prediction.mispredicted());

        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 0);
        assert_eq!(summary.btb_mispredictions(), 1);
        assert_eq!(summary.predicted_taken_btb_misses(), 1);
        assert_eq!(
            summary
                .lookup_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.lookup_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .committed_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.committed_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .mispredicted_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(summary.mispredicted_branch_kinds().total(), 0);
        assert_eq!(
            summary
                .corrected_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(summary.corrected_branch_kinds().total(), 0);
        assert_eq!(
            summary
                .target_wrong_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(summary.target_wrong_branch_kinds().total(), 0);
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(
            summary
                .mispredict_due_to_predictor()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(summary.mispredict_due_to_predictor().total(), 0);
        assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 0);
    }

    #[test]
    fn target_provider_counts_no_target_when_warm_btb_conditional_predicts_not_taken() {
        let branch = b_type(8, 0, 0, 0x1).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.branch_target_buffer.update(
                Address::new(0x8000),
                Address::new(0x8008),
                BranchTargetKind::DirectConditional,
            );
        }

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x8004));
        assert_eq!(
            decision
                .branch_speculation()
                .map(|speculation| (speculation.predicted_taken(), speculation.target())),
            Some((false, None))
        );
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(!prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), None);
        assert!(!prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), None);
        assert!(!prediction.mispredicted());

        let summary = core.branch_speculation_summary();
        assert_eq!(
            summary
                .target_provider()
                .value(BranchTargetProvider::NoTarget),
            1
        );
        assert_eq!(
            summary.target_provider().value(BranchTargetProvider::BTB),
            0
        );
        assert_eq!(summary.target_provider().total(), 1);
        assert_eq!(summary.committed_branch_kinds().total(), 1);
        assert_eq!(summary.mispredicted_branch_kinds().total(), 0);
    }

    #[test]
    fn target_provider_counts_no_target_when_gshare_uses_static_conditional_target() {
        let branch = b_type(8, 0, 0, 0x0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let pc = Address::new(0x8000);
            for _ in 0..2 {
                let prediction = state
                    .gshare_branch_predictor
                    .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
                    .unwrap();
                state
                    .gshare_branch_predictor
                    .train(prediction.history(), true, false)
                    .unwrap();
            }
            state.branch_target_buffer.update(
                pc,
                Address::new(0x8010),
                BranchTargetKind::DirectConditional,
            );
        }

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x8008));
        assert_eq!(
            decision
                .branch_speculation()
                .map(|speculation| (speculation.predicted_taken(), speculation.target())),
            Some((true, Some(Address::new(0x8008))))
        );
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
        assert!(prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
        assert!(!prediction.mispredicted());

        let summary = core.branch_speculation_summary();
        assert_eq!(
            summary
                .target_provider()
                .value(BranchTargetProvider::NoTarget),
            1
        );
        assert_eq!(
            summary.target_provider().value(BranchTargetProvider::BTB),
            0
        );
        assert_eq!(summary.target_provider().total(), 1);
        assert_eq!(summary.committed_branch_kinds().total(), 1);
        assert_eq!(summary.mispredicted_branch_kinds().total(), 0);
    }

    #[test]
    fn btb_misprediction_counts_taken_fetch_prediction_with_wrong_btb_target() {
        let branch = b_type(8, 0, 0, 0x0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let pc = Address::new(0x8000);
            state
                .branch_predictor
                .update(pc, true, Some(Address::new(0x8008)));
            state
                .branch_predictor
                .update(pc, true, Some(Address::new(0x8008)));
            state.branch_target_buffer.update(
                pc,
                Address::new(0x8010),
                BranchTargetKind::DirectConditional,
            );
        }

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x8010));
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x8010));
        assert!(prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
        assert!(prediction.mispredicted());
        assert_eq!(prediction.repair_target_pc(), Some(0x8008));

        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 1);
        assert_eq!(summary.btb_mispredictions(), 1);
        assert_eq!(summary.predicted_taken_btb_misses(), 0);
        assert_eq!(
            summary
                .lookup_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.lookup_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .committed_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.committed_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .mispredicted_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.mispredicted_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .corrected_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.corrected_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .target_wrong_branch_kinds()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(summary.target_wrong_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .target_provider()
                .value(BranchTargetProvider::NoTarget),
            0
        );
        assert_eq!(
            summary.target_provider().value(BranchTargetProvider::BTB),
            1
        );
        assert_eq!(summary.target_provider().total(), 1);
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::DirectConditional),
            0
        );
    }

    #[test]
    fn btb_misprediction_counts_direct_jump_cold_btb_without_branch_type_lane() {
        let jump = j_type(12, 0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(jump);

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x800c));
        assert!(prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), Some(0x800c));
        assert!(!prediction.mispredicted());

        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 0);
        assert_eq!(summary.btb_mispredictions(), 1);
        assert_eq!(summary.predicted_taken_btb_misses(), 1);
        assert_eq!(
            summary
                .lookup_branch_kinds()
                .value(BranchTargetKind::DirectUnconditional),
            1
        );
        assert_eq!(summary.lookup_branch_kinds().total(), 1);
        assert_eq!(
            summary
                .target_wrong_branch_kinds()
                .value(BranchTargetKind::DirectUnconditional),
            0
        );
        assert_eq!(summary.target_wrong_branch_kinds().total(), 0);
        assert_eq!(
            summary
                .target_provider()
                .value(BranchTargetProvider::NoTarget),
            1
        );
        assert_eq!(
            summary.target_provider().value(BranchTargetProvider::BTB),
            0
        );
        assert_eq!(summary.target_provider().total(), 1);
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::DirectUnconditional),
            0
        );
        assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 0);
    }

    #[test]
    fn target_provider_counts_no_target_when_direct_jump_uses_static_target() {
        let jump = j_type(12, 0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(jump);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.branch_target_buffer.update(
                Address::new(0x8000),
                Address::new(0x8010),
                BranchTargetKind::DirectUnconditional,
            );
        }

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        assert_eq!(
            decision
                .branch_speculation()
                .map(|speculation| (speculation.predicted_taken(), speculation.target())),
            Some((true, Some(Address::new(0x800c))))
        );
        record_fetch_ahead_speculation(&core, &decision).unwrap();

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x800c));
        assert!(prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), Some(0x800c));
        assert!(!prediction.mispredicted());

        let summary = core.branch_speculation_summary();
        assert_eq!(
            summary
                .target_provider()
                .value(BranchTargetProvider::NoTarget),
            1
        );
        assert_eq!(
            summary.target_provider().value(BranchTargetProvider::BTB),
            0
        );
        assert_eq!(summary.target_provider().total(), 1);
        assert_eq!(
            summary
                .lookup_branch_kinds()
                .value(BranchTargetKind::DirectUnconditional),
            1
        );
    }

    #[test]
    fn btb_update_classifies_direct_link_jump_as_call_direct() {
        let jump = j_type(12, 1).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(jump);

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        record_fetch_ahead_speculation(&core, &decision).unwrap();
        core.execute_next_completed_fetch().unwrap().unwrap();

        assert_eq!(
            btb_entry_kind(&core, 0x8000),
            Some(BranchTargetKind::CallDirect)
        );
    }

    #[test]
    fn btb_mispredict_due_to_btb_miss_counts_indirect_unconditional_target_change() {
        let jalr = i_type(0, 6, 0x0, 0, 0x67).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(jalr);
        let target_register = Register::new(6).unwrap();
        core.write_register(target_register, 0x800c);

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        assert_eq!(
            decision.branch_speculation().map(|speculation| {
                (
                    speculation.sequence(),
                    speculation.pc(),
                    speculation.predicted_taken(),
                    speculation.target(),
                    speculation.branch_target_prediction(),
                )
            }),
            Some((
                0,
                Address::new(0x8000),
                true,
                Some(Address::new(0x800c)),
                Some(BranchTargetPrediction::new(false, None)),
            ))
        );
        record_fetch_ahead_speculation(&core, &decision).unwrap();
        core.write_register(target_register, 0x8010);

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x800c));
        assert!(prediction.resolved_taken());
        assert_eq!(prediction.resolved_target_pc(), Some(0x8010));
        assert!(prediction.mispredicted());
        assert_eq!(prediction.repair_target_pc(), Some(0x8010));

        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 1);
        assert_eq!(summary.btb_mispredictions(), 1);
        assert_eq!(summary.predicted_taken_btb_misses(), 1);
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::IndirectUnconditional),
            1
        );
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 1);
        assert_eq!(
            btb_entry_kind(&core, 0x8000),
            Some(BranchTargetKind::IndirectUnconditional)
        );
    }

    #[test]
    fn btb_mispredict_due_to_btb_miss_counts_indirect_call_target_change() {
        let jalr = i_type(0, 6, 0x0, 1, 0x67).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(jalr);
        let target_register = Register::new(6).unwrap();
        core.write_register(target_register, 0x800c);

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        record_fetch_ahead_speculation(&core, &decision).unwrap();
        core.write_register(target_register, 0x8010);

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.mispredicted());
        assert_eq!(prediction.repair_target_pc(), Some(0x8010));

        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 1);
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::CallIndirect),
            1
        );
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::IndirectUnconditional),
            0
        );
        assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 1);
        assert_eq!(
            btb_entry_kind(&core, 0x8000),
            Some(BranchTargetKind::CallIndirect)
        );
    }

    #[test]
    fn btb_mispredict_due_to_btb_miss_counts_return_target_change() {
        let jalr = i_type(0, 1, 0x0, 0, 0x67).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(jalr);
        let target_register = Register::new(1).unwrap();
        core.write_register(target_register, 0x800c);

        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(0x800c));
        record_fetch_ahead_speculation(&core, &decision).unwrap();
        core.write_register(target_register, 0x8010);

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.mispredicted());
        assert_eq!(prediction.repair_target_pc(), Some(0x8010));

        let summary = core.branch_speculation_summary();
        assert_eq!(summary.repairs(), 1);
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::Return),
            1
        );
        assert_eq!(
            summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::IndirectUnconditional),
            0
        );
        assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 1);
        assert_eq!(
            btb_entry_kind(&core, 0x8000),
            Some(BranchTargetKind::Return)
        );
    }

    #[test]
    fn selected_gshare_fetch_ahead_uses_speculative_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let first_pc = Address::new(0x8000);
            for _ in 0..2 {
                let prediction = state
                    .gshare_branch_predictor
                    .predict(RISCV_LOCAL_GSHARE_THREAD, first_pc)
                    .unwrap();
                state
                    .gshare_branch_predictor
                    .train(prediction.history(), true, false)
                    .unwrap();
            }

            let history_seed = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, Address::new(0x9000))
                .unwrap();
            state
                .gshare_branch_predictor
                .update_history(history_seed.history(), true)
                .unwrap();
            let second_pc = Address::new(0x8008);
            for _ in 0..2 {
                let prediction = state
                    .gshare_branch_predictor
                    .predict(RISCV_LOCAL_GSHARE_THREAD, second_pc)
                    .unwrap();
                assert_eq!(prediction.global_history_before(), 1);
                state
                    .gshare_branch_predictor
                    .train(prediction.history(), true, false)
                    .unwrap();
            }
            state
                .gshare_branch_predictor
                .squash(history_seed.history())
                .unwrap();
            assert_eq!(
                state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
        }

        let first = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(first.pc(), Address::new(0x8008));
        core.set_fetch_ahead_pc(first.pc());
        record_fetch_ahead_speculation(&core, &first).unwrap();

        assert_eq!(
            core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
    }

    #[test]
    fn selected_gshare_fetch_ahead_uses_direct_jump_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let history_seed = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, Address::new(0x9000))
                .unwrap();
            state
                .gshare_branch_predictor
                .update_history(history_seed.history(), true)
                .unwrap();
            let second_pc = Address::new(0x8008);
            for _ in 0..2 {
                let prediction = state
                    .gshare_branch_predictor
                    .predict(RISCV_LOCAL_GSHARE_THREAD, second_pc)
                    .unwrap();
                assert_eq!(prediction.global_history_before(), 1);
                state
                    .gshare_branch_predictor
                    .train(prediction.history(), true, false)
                    .unwrap();
            }
            state
                .gshare_branch_predictor
                .squash(history_seed.history())
                .unwrap();
            assert_eq!(
                state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
        }

        let first = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(first.pc(), Address::new(0x8008));
        core.set_fetch_ahead_pc(first.pc());
        record_fetch_ahead_speculation(&core, &first).unwrap();

        assert_eq!(
            core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
    }

    #[test]
    fn selected_bimode_fetch_ahead_uses_speculative_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::BiMode);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            train_selected_bimode_taken(&mut state, Address::new(0x8000));

            let history_seed = state
                .bimode_branch_predictor
                .predict(RISCV_LOCAL_BIMODE_THREAD, Address::new(0x9000))
                .unwrap();
            state
                .bimode_branch_predictor
                .update_history(history_seed.history(), true)
                .unwrap();
            let second_pc = Address::new(0x8008);
            train_selected_bimode_taken(&mut state, second_pc);
            let trained_second = state
                .bimode_branch_predictor
                .predict(RISCV_LOCAL_BIMODE_THREAD, second_pc)
                .unwrap();
            assert_eq!(trained_second.global_history_before(), 1);
            assert!(trained_second.predicted_taken());
            state
                .bimode_branch_predictor
                .squash(history_seed.history())
                .unwrap();
            assert_eq!(
                state.bimode_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
        }

        let first = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(first.pc(), Address::new(0x8008));
        core.set_fetch_ahead_pc(first.pc());
        record_fetch_ahead_speculation(&core, &first).unwrap();

        assert_eq!(
            core.bimode_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
    }

    #[test]
    fn selected_bimode_fetch_ahead_uses_direct_jump_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::BiMode);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            let history_seed = state
                .bimode_branch_predictor
                .predict(RISCV_LOCAL_BIMODE_THREAD, Address::new(0x9000))
                .unwrap();
            state
                .bimode_branch_predictor
                .update_history(history_seed.history(), true)
                .unwrap();
            let second_pc = Address::new(0x8008);
            train_selected_bimode_taken(&mut state, second_pc);
            let trained_second = state
                .bimode_branch_predictor
                .predict(RISCV_LOCAL_BIMODE_THREAD, second_pc)
                .unwrap();
            assert_eq!(trained_second.global_history_before(), 1);
            assert!(trained_second.predicted_taken());
            state
                .bimode_branch_predictor
                .squash(history_seed.history())
                .unwrap();
            assert_eq!(
                state.bimode_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
        }

        let first = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(first.pc(), Address::new(0x8008));
        core.set_fetch_ahead_pc(first.pc());
        record_fetch_ahead_speculation(&core, &first).unwrap();

        assert_eq!(
            core.bimode_branch_predictor_snapshot().threads()[0].global_history(),
            1
        );
        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
    }

    #[test]
    fn selected_tournament_fetch_ahead_uses_pending_local_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            let older_pc = Address::new(0x8000);
            let younger_pc = Address::new(0x8008);
            train_selected_tournament_local_history_one_taken(&mut state, younger_pc);
            let base_prediction = state
                .tournament_branch_predictor
                .predict(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc)
                .unwrap();
            assert_eq!(base_prediction.local_history_before(), 0);
            assert!(!base_prediction.predicted_taken());
            let overlay_prediction = state
                .tournament_branch_predictor
                .predict_with_histories(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc, 0, 1)
                .unwrap();
            assert!(overlay_prediction.predicted_taken());
            insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
            let fetch_events = core.core.fetch_events();
            let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
            assert_eq!(
                selected_tournament_speculative_histories(&state, &completed_fetches, younger_pc),
                Some((1, 1))
            );
            assert_eq!(
                state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
            assert_eq!(
                state
                    .tournament_branch_predictor
                    .snapshot()
                    .local_history_table()[0],
                0
            );
        }

        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[0],
            0
        );
    }

    #[test]
    fn selected_tournament_fetch_ahead_uses_pending_conditional_global_history_for_younger_branch()
    {
        let core = core_with_completed_fetches([
            (0, 0x8000, b_type(4, 0, 0, 0).to_le_bytes().to_vec()),
            (1, 0x8004, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            let older_pc = Address::new(0x8000);
            let younger_pc = Address::new(0x8004);
            train_selected_tournament_global_history_one_taken(&mut state, Address::new(0x9000));
            assert!(!state
                .tournament_branch_predictor
                .shares_local_history_entry(older_pc, younger_pc));
            let base_prediction = state
                .tournament_branch_predictor
                .predict(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc)
                .unwrap();
            assert_eq!(base_prediction.global_history_before(), 0);
            assert_eq!(base_prediction.local_history_before(), 0);
            assert!(!base_prediction.predicted_taken());
            let overlay_prediction = state
                .tournament_branch_predictor
                .predict_with_histories(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc, 1, 0)
                .unwrap();
            assert!(overlay_prediction.predicted_taken());
            insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
            let fetch_events = core.core.fetch_events();
            let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
            assert_eq!(
                selected_tournament_speculative_histories(&state, &completed_fetches, younger_pc),
                Some((1, 0))
            );
            assert_eq!(
                state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
            assert_eq!(
                state
                    .tournament_branch_predictor
                    .snapshot()
                    .local_history_table()[1],
                0
            );
        }

        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x800c));
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[1],
            0
        );
    }

    #[test]
    fn selected_tournament_fetch_ahead_uses_direct_jump_global_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            use_small_tournament_predictor(&mut state);
            let older_pc = Address::new(0x8000);
            let younger_pc = Address::new(0x8008);
            train_selected_tournament_global_history_one_taken(&mut state, Address::new(0x9000));
            let base_prediction = state
                .tournament_branch_predictor
                .predict(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc)
                .unwrap();
            assert_eq!(base_prediction.global_history_before(), 0);
            assert_eq!(base_prediction.local_history_before(), 0);
            assert!(!base_prediction.predicted_taken());
            let overlay_prediction = state
                .tournament_branch_predictor
                .predict_with_histories(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc, 1, 0)
                .unwrap();
            assert!(overlay_prediction.predicted_taken());
            insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
            let fetch_events = core.core.fetch_events();
            let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
            assert_eq!(
                selected_tournament_speculative_histories(&state, &completed_fetches, younger_pc),
                Some((1, 0))
            );
            assert_eq!(
                state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
                0
            );
            assert_eq!(
                state
                    .tournament_branch_predictor
                    .snapshot()
                    .local_history_table()[0],
                0
            );
        }

        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[0],
            0
        );
    }

    #[test]
    fn selected_multiperspective_fetch_ahead_uses_pending_local_history_for_younger_branch() {
        let core = core_with_completed_fetches([
            (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
            (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        ]);
        core.set_branch_predictor_kind(RiscvBranchPredictorKind::MultiperspectivePerceptron);
        core.set_branch_lookahead(2);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            use_local_bias_multiperspective_perceptron(&mut state);
            let older_pc = Address::new(0x8000);
            let younger_pc = Address::new(0x8008);
            let base_prediction = state
                .multiperspective_perceptron
                .predict(
                    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
                    younger_pc,
                    true,
                )
                .unwrap();
            assert!(!base_prediction.predicted_taken());
            insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
            assert_eq!(
                state
                    .multiperspective_perceptron
                    .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
                    .unwrap()
                    .local_history_for(younger_pc),
                0
            );
        }

        let second = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(second.pc(), Address::new(0x8010));
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state
                .multiperspective_perceptron
                .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
                .unwrap()
                .local_history_for(Address::new(0x8008)),
            0
        );
    }

    #[test]
    fn checkpoint_payload_restores_live_fetch_ahead_branch_speculation() {
        let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        let decision = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(decision.pc(), Address::new(0x8004));
        assert_eq!(
            decision
                .branch_speculation()
                .map(|speculation| { (speculation.sequence(), speculation.pc()) }),
            Some((0, Address::new(0x8000)))
        );

        record_fetch_ahead_speculation(&core, &decision).unwrap();
        let captured = core.branch_predictor_checkpoint_payload();
        {
            let mut state = core.state.lock().expect("riscv core lock");
            assert_eq!(state.branch_speculations.len(), 1);
            assert_eq!(state.branch_target_predictions.len(), 1);
            assert_eq!(state.branch_predictor.pending_speculation_count(), 1);
            state.discard_branch_speculations();
            assert!(state.branch_speculations.is_empty());
            assert!(state.branch_target_predictions.is_empty());
            assert!(state.branch_predictor.pending_speculations().is_empty());
        }

        core.restore_branch_predictor_checkpoint_payload(captured)
            .unwrap();
        assert_eq!(
            core.state
                .lock()
                .expect("riscv core lock")
                .branch_target_predictions
                .len(),
            1
        );

        assert!(core
            .can_retire_completed_fetch_while_fetch_pending()
            .unwrap());
        core.execute_next_completed_fetch().unwrap().unwrap();
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.branch_speculations.is_empty());
        assert!(state.branch_target_predictions.is_empty());
        assert!(state.branch_predictor.pending_speculations().is_empty());
        assert_eq!(state.branch_speculation_summary.btb_mispredictions(), 1);
        assert_eq!(
            state
                .branch_speculation_summary
                .btb_mispredict_due_to_btb_miss()
                .value(BranchTargetKind::DirectConditional),
            1
        );
        assert_eq!(
            state
                .branch_speculation_summary
                .mispredict_due_to_predictor()
                .value(BranchTargetKind::DirectConditional),
            0
        );
        assert_eq!(
            state
                .branch_speculation_summary
                .mispredict_due_to_predictor()
                .total(),
            0
        );
        assert_eq!(
            state
                .branch_speculation_summary
                .predicted_taken_btb_misses(),
            0
        );
    }

    #[test]
    fn checkpoint_restored_basic_predictor_target_steers_with_cold_btb() {
        let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state
                .branch_predictor
                .update(Address::new(0x8000), true, Some(Address::new(0x8008)));
        }
        let captured = core.branch_predictor_checkpoint_payload();
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.branch_target_buffer.invalidate();
        }

        core.restore_branch_predictor_checkpoint_payload(captured)
            .unwrap();
        let decision = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(decision.pc(), Address::new(0x8008));
        assert_eq!(
            decision.branch_speculation().map(|speculation| {
                (
                    speculation.sequence(),
                    speculation.pc(),
                    speculation.predicted_taken(),
                    speculation.target(),
                )
            }),
            Some((0, Address::new(0x8000), true, Some(Address::new(0x8008))))
        );
        let btb = core.branch_target_buffer_snapshot();
        assert_eq!(btb.lookup_count(), 1);
        assert_eq!(btb.hit_count(), 0);
    }

    #[test]
    fn checkpoint_restore_ignores_polluted_btb_target() {
        let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state
                .branch_predictor
                .update(Address::new(0x8000), true, Some(Address::new(0x8008)));
            state.branch_target_buffer.update(
                Address::new(0x8000),
                Address::new(0x8008),
                BranchTargetKind::DirectConditional,
            );
        }
        let captured = core.branch_predictor_checkpoint_payload();
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.branch_target_buffer.update(
                Address::new(0x8000),
                Address::new(0x8010),
                BranchTargetKind::DirectConditional,
            );
        }

        core.restore_branch_predictor_checkpoint_payload(captured)
            .unwrap();
        let decision = core.next_fetch_ahead_before_retire().unwrap();

        assert_eq!(decision.pc(), Address::new(0x8008));
        assert_eq!(
            decision.branch_speculation().map(|speculation| {
                (
                    speculation.sequence(),
                    speculation.pc(),
                    speculation.predicted_taken(),
                    speculation.target(),
                )
            }),
            Some((0, Address::new(0x8000), true, Some(Address::new(0x8008))))
        );
        let btb = core.branch_target_buffer_snapshot();
        assert_eq!(btb.lookup_count(), 1);
        assert_eq!(btb.hit_count(), 1);
    }

    #[test]
    fn checkpoint_restore_rejects_bad_btb_shape_without_partial_state_change() {
        let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
        let core = core_with_completed_fetch(branch);
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        record_fetch_ahead_speculation(&core, &decision).unwrap();
        let original_predictor = core.branch_predictor_snapshot();
        let original_btb = core.branch_target_buffer_snapshot();
        let original_speculations = {
            let state = core.state.lock().expect("riscv core lock");
            state.branch_speculations.clone()
        };
        let mut alternate_predictor = BranchPredictor::new(
            BranchPredictorConfig::new(DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES)
                .expect("default RISC-V branch predictor entries are valid"),
        );
        alternate_predictor.update(Address::new(0x9000), true, Some(Address::new(0x9008)));
        let incompatible_btb =
            BranchTargetBuffer::new(BranchTargetBufferConfig::new(8, 2).unwrap()).snapshot();
        let payload = BranchPredictorCheckpointPayload::from_snapshots(
            alternate_predictor.snapshot(),
            incompatible_btb,
            [],
        )
        .unwrap();

        let error = core
            .restore_branch_predictor_checkpoint_payload(payload)
            .unwrap_err();

        assert!(matches!(
            error,
            crate::BranchPredictorError::InvalidBranchTargetBufferCheckpoint { .. }
        ));
        assert_eq!(core.branch_predictor_snapshot(), original_predictor);
        assert_eq!(core.branch_target_buffer_snapshot(), original_btb);
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations, original_speculations);
    }

    #[test]
    fn retired_fetch_gate_repairs_stale_oldest_branch_speculation() {
        let mut state = RiscvCoreState::new(0x1186a, 0);
        let stale = state
            .branch_predictor
            .predict_speculative(Address::new(0x1000));
        state.branch_speculations.insert(1, stale.id());
        state.executed_fetches.insert(request(1));

        assert!(can_retire_completed_fetch_with_branch_speculations(
            &mut state,
            &[completed(2, 0x1186a)]
        )
        .unwrap());
        assert!(state.branch_speculations.is_empty());
        assert!(state.branch_predictor.pending_speculations().is_empty());
    }

    #[test]
    fn retired_fetch_gate_discards_stale_selected_gshare_speculation() {
        let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
        let mut state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            1
        );
        state.executed_fetches.insert(request(0));

        assert!(can_retire_completed_fetch_with_branch_speculations(
            &mut state,
            &[completed(1, 0x8000)]
        )
        .unwrap());

        assert!(state.branch_speculations.is_empty());
        assert!(state.selected_branch_speculations.is_empty());
        assert!(state.branch_predictor.pending_speculations().is_empty());
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }
}
