use rem6_isa_riscv::{RiscvHartState, RiscvInstruction, RiscvPrivilegeMode};
use rem6_memory::Address;

use crate::{
    CpuFetchEvent, CpuFetchEventKind, RiscvBranchPredictorKind, RiscvCore, RiscvCoreState,
    RiscvCpuError, RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
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
    sequence: u64,
    fetch_pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadDecision> {
    if instruction_allows_straight_line_fetch_ahead(instruction) {
        return Some(RiscvFetchAheadDecision::straight_line(sequential_pc));
    }
    if let Some(target) = direct_jump_fetch_ahead_target(state, fetch_pc, instruction) {
        return Some(RiscvFetchAheadDecision::straight_line(target));
    }
    if !instruction_is_conditional_branch(instruction) {
        return None;
    }

    let prediction = selected_conditional_branch_prediction(state, fetch_pc, instruction)?;
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
    fetch_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadBranchPrediction> {
    match state.branch_predictor_kind {
        RiscvBranchPredictorKind::Basic => {
            let prediction = state.branch_predictor.predict(fetch_pc);
            Some(RiscvFetchAheadBranchPrediction {
                predicted_taken: prediction.predicted_taken(),
                target: prediction.target(),
            })
        }
        RiscvBranchPredictorKind::GShare => {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, fetch_pc)
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
            let prediction = state
                .bimode_branch_predictor
                .predict(RISCV_LOCAL_BIMODE_THREAD, fetch_pc)
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
            let prediction = state
                .tournament_branch_predictor
                .predict(RISCV_LOCAL_TOURNAMENT_THREAD, fetch_pc)
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
    state: &RiscvCoreState,
    fetch_pc: Address,
    instruction: RiscvInstruction,
) -> Option<Address> {
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
        CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId, CpuResetState, RiscvBranchPredictorKind,
        RISCV_LOCAL_GSHARE_THREAD,
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
        core.core.state.lock().expect("cpu core lock").events.push(
            crate::CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    4,
                    PartitionId::new(0),
                    MemoryRouteId::new(0),
                    endpoint("cpu0.ifetch"),
                    request(0),
                    Address::new(0x8000),
                    AccessSize::new(4).unwrap(),
                ),
                data,
            ),
        );
        core
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
        assert_eq!(decision.branch_speculation(), None);
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
