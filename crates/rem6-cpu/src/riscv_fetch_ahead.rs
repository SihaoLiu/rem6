use rem6_isa_riscv::{RiscvHartState, RiscvInstruction, RiscvPrivilegeMode};
use rem6_memory::Address;

use crate::{
    BranchTargetKind, CpuFetchEvent, CpuFetchEventKind, RiscvBranchPredictorKind, RiscvCore,
    RiscvCoreState, RiscvCpuError, RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD, RISCV_LOCAL_TAGE_SC_L_THREAD,
    RISCV_LOCAL_TOURNAMENT_THREAD,
};

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

    pub(crate) fn record_fetch_ahead_speculation(&self, decision: &RiscvFetchAheadDecision) {
        let Some(speculation) = decision.branch_speculation() else {
            return;
        };
        let mut state = self.state.lock().expect("riscv core lock");
        if state
            .branch_speculations
            .contains_key(&speculation.sequence())
        {
            return;
        }
        let prediction = state.branch_predictor.predict_speculative_with_prediction(
            speculation.pc(),
            speculation.predicted_taken(),
            speculation.target(),
        );
        state
            .branch_speculations
            .insert(speculation.sequence(), prediction.id());
        let pending = state.branch_speculations.len() as u64;
        state.branch_speculation_summary.record_prediction(pending);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        predicted_taken: bool,
        target: Option<Address>,
    ) -> Self {
        Self {
            pc,
            branch_speculation: Some(RiscvFetchAheadSpeculation {
                sequence,
                pc: branch_pc,
                predicted_taken,
                target,
            }),
        }
    }

    pub(crate) const fn pc(self) -> Address {
        self.pc
    }

    pub(crate) const fn branch_speculation(self) -> Option<RiscvFetchAheadSpeculation> {
        self.branch_speculation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvFetchAheadSpeculation {
    sequence: u64,
    pc: Address,
    predicted_taken: bool,
    target: Option<Address>,
}

impl RiscvFetchAheadSpeculation {
    const fn sequence(self) -> u64 {
        self.sequence
    }

    const fn pc(self) -> Address {
        self.pc
    }

    const fn predicted_taken(self) -> bool {
        self.predicted_taken
    }

    const fn target(self) -> Option<Address> {
        self.target
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
    if let Some(target) = direct_jump_fetch_ahead_target(state, fetch_pc, instruction) {
        return Some(RiscvFetchAheadDecision::branch(
            target,
            sequence,
            fetch_pc,
            true,
            Some(target),
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
        prediction.predicted_taken,
        prediction.target,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvFetchAheadBranchPrediction {
    predicted_taken: bool,
    target: Option<Address>,
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
    match state.branch_predictor_kind {
        RiscvBranchPredictorKind::Basic => {
            let prediction = state.branch_predictor.predict(fetch_pc);
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target: if prediction.predicted_taken() {
                    target_lookup.target().or_else(|| prediction.target())
                } else {
                    None
                },
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
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
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
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
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
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
            })
        }
        RiscvBranchPredictorKind::TageScL => {
            let prediction = state
                .tage_sc_l_branch_predictor
                .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, fetch_pc, true)
                .ok()?;
            let target = prediction
                .predicted_taken()
                .then(|| conditional_branch_target(fetch_pc, instruction))
                .flatten();
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
            })
        }
        RiscvBranchPredictorKind::MultiperspectivePerceptron => {
            let prediction = state
                .multiperspective_perceptron
                .predict(
                    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
                    fetch_pc,
                    true,
                )
                .ok()?;
            let target = prediction
                .predicted_taken()
                .then(|| conditional_branch_target(fetch_pc, instruction))
                .flatten();
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target,
            })
        }
    }
}

fn selected_gshare_speculative_history(state: &RiscvCoreState) -> Option<u64> {
    let history = state
        .gshare_branch_predictor
        .global_history(RISCV_LOCAL_GSHARE_THREAD)
        .ok()?;
    selected_speculative_history(state, history, |history, taken| {
        state
            .gshare_branch_predictor
            .shifted_history(history, taken)
    })
}

fn selected_bimode_speculative_history(state: &RiscvCoreState) -> Option<u64> {
    let history = state
        .bimode_branch_predictor
        .global_history(RISCV_LOCAL_BIMODE_THREAD)
        .ok()?;
    selected_speculative_history(state, history, |history, taken| {
        state
            .bimode_branch_predictor
            .shifted_history(history, taken)
    })
}

fn selected_speculative_history(
    state: &RiscvCoreState,
    mut history: u64,
    mut shift_history: impl FnMut(u64, bool) -> u64,
) -> Option<u64> {
    for speculation in state.branch_speculations.values() {
        let pending = state.branch_predictor.pending_speculation(*speculation)?;
        history = shift_history(history, pending.predicted_taken());
    }
    Some(history)
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
    for (sequence, speculation) in &state.branch_speculations {
        let pending = state.branch_predictor.pending_speculation(*speculation)?;
        global_history = state
            .tournament_branch_predictor
            .shifted_global_history(global_history, pending.predicted_taken());
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

fn pending_speculation_updates_tournament_local_history(
    completed_fetches: &[&CpuFetchEvent],
    sequence: u64,
) -> Option<bool> {
    let fetch = completed_fetches
        .iter()
        .copied()
        .find(|event| event.request_id().sequence() == sequence)?;
    let data = fetch.data()?;
    let raw = match data {
        [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
        _ => return None,
    };
    let decoded = RiscvInstruction::decode_with_length(raw).ok()?;
    Some(instruction_is_conditional_branch(decoded.instruction()))
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
) -> Option<Address> {
    let kind = match instruction {
        RiscvInstruction::Jal { .. } => BranchTargetKind::DirectUnconditional,
        RiscvInstruction::Jalr { .. } => BranchTargetKind::IndirectUnconditional,
        _ => return None,
    };
    state.branch_target_buffer.lookup(fetch_pc, kind);
    match instruction {
        RiscvInstruction::Jal { offset, .. } => {
            checked_add_signed(fetch_pc.get(), offset.value()).map(Address::new)
        }
        RiscvInstruction::Jalr { rs1, offset, .. } => {
            checked_add_signed(state.hart.read(rs1), offset.value())
                .map(|target| Address::new(target & !1))
        }
        _ => None,
    }
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
        BranchTargetBuffer, BranchTargetBufferConfig, CpuCore, CpuFetchConfig, CpuFetchRecord,
        CpuId, CpuResetState, RiscvBranchPredictorKind, TournamentBranchPredictor,
        TournamentBranchPredictorConfig, DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES,
        RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
    };
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
                .map(RiscvFetchAheadDecision::pc),
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
        core.record_fetch_ahead_speculation(&decision);

        let event = core.execute_next_completed_fetch().unwrap().unwrap();
        let basic_update = event.branch_update().unwrap();
        assert!(!basic_update.predicted_taken());

        let cycle = event.in_order_pipeline_cycle().unwrap();
        let prediction = cycle.branch_predictions().first().unwrap();
        assert!(prediction.predicted_taken());
        assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
        assert!(!prediction.resolved_taken());
        assert_eq!(prediction.repair_target_pc(), Some(0x8004));
        assert_eq!(core.branch_speculation_summary().repairs(), 1);
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
        core.record_fetch_ahead_speculation(&first);

        assert_eq!(
            core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
            0
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
        core.record_fetch_ahead_speculation(&first);

        assert_eq!(
            core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
            0
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
        core.record_fetch_ahead_speculation(&first);

        assert_eq!(
            core.bimode_branch_predictor_snapshot().threads()[0].global_history(),
            0
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
        core.record_fetch_ahead_speculation(&first);

        assert_eq!(
            core.bimode_branch_predictor_snapshot().threads()[0].global_history(),
            0
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

        core.record_fetch_ahead_speculation(&decision);
        let captured = core.branch_predictor_checkpoint_payload();
        {
            let mut state = core.state.lock().expect("riscv core lock");
            assert_eq!(state.branch_speculations.len(), 1);
            assert_eq!(state.branch_predictor.pending_speculation_count(), 1);
            state.discard_branch_speculations();
            assert!(state.branch_speculations.is_empty());
            assert!(state.branch_predictor.pending_speculations().is_empty());
        }

        core.restore_branch_predictor_checkpoint_payload(captured)
            .unwrap();

        assert!(core
            .can_retire_completed_fetch_while_fetch_pending()
            .unwrap());
        core.execute_next_completed_fetch().unwrap().unwrap();
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.branch_speculations.is_empty());
        assert!(state.branch_predictor.pending_speculations().is_empty());
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
        core.record_fetch_ahead_speculation(&decision);
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
}
