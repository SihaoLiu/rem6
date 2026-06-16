use rem6_isa_riscv::{RiscvHartState, RiscvInstruction, RiscvPrivilegeMode};
use rem6_memory::Address;

use crate::{CpuFetchEventKind, RiscvCore, RiscvCoreState};

const COMPLETED_FETCH_WINDOW: usize = 2;

impl RiscvCore {
    pub(crate) fn next_fetch_ahead_before_retire(&self) -> Option<RiscvFetchAheadDecision> {
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
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
        if completed.is_empty() || completed.len() >= COMPLETED_FETCH_WINDOW {
            return None;
        }
        completed.sort_by_key(|event| event.request_id().sequence());

        let fetch = completed[0];
        if fetch.pc() != Address::new(state.hart.pc()) {
            return None;
        }
        let data = fetch.data()?;
        let raw = match data {
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => return None,
        };
        let Ok(decoded) = RiscvInstruction::decode_with_length(raw) else {
            return None;
        };
        if decoded.bytes() != 4 {
            return None;
        }
        let sequential_pc = Address::new(fetch.pc().get().wrapping_add(u64::from(decoded.bytes())));

        fetch_ahead_decision(
            &state,
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
        let prediction = state.branch_predictor.predict_speculative(speculation.pc());
        state
            .branch_speculations
            .insert(speculation.sequence(), prediction.id());
    }

    pub(crate) fn can_retire_completed_fetch_while_fetch_pending(&self) -> bool {
        let state = self.state.lock().expect("riscv core lock");
        state.pending_trap.is_none()
            && state.pending_fetch_prefix.is_none()
            && state.branch_speculations.is_empty()
            && !hart_has_enabled_pending_interrupt(&state.hart)
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

    const fn branch(pc: Address, sequence: u64, branch_pc: Address) -> Self {
        Self {
            pc,
            branch_speculation: Some(RiscvFetchAheadSpeculation {
                sequence,
                pc: branch_pc,
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
}

impl RiscvFetchAheadSpeculation {
    const fn sequence(self) -> u64 {
        self.sequence
    }

    const fn pc(self) -> Address {
        self.pc
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

fn fetch_ahead_decision(
    state: &RiscvCoreState,
    sequence: u64,
    fetch_pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadDecision> {
    if instruction_allows_straight_line_fetch_ahead(instruction) {
        return Some(RiscvFetchAheadDecision::straight_line(sequential_pc));
    }
    if !instruction_is_conditional_branch(instruction) {
        return None;
    }

    let prediction = state.branch_predictor.predict(fetch_pc);
    let pc = if prediction.predicted_taken() {
        prediction.target()?
    } else {
        sequential_pc
    };
    Some(RiscvFetchAheadDecision::branch(pc, sequence, fetch_pc))
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
