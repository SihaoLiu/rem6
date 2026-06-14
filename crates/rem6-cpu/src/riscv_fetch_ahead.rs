use rem6_isa_riscv::{RiscvHartState, RiscvInstruction, RiscvPrivilegeMode};
use rem6_memory::Address;

use crate::{CpuFetchEventKind, RiscvCore};

const COMPLETED_FETCH_WINDOW: usize = 2;

impl RiscvCore {
    pub(crate) fn should_issue_fetch_ahead_before_retire(&self) -> bool {
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() || state.pending_fetch_prefix.is_some() {
            return false;
        }
        if hart_has_enabled_pending_interrupt(&state.hart) {
            return false;
        }

        let mut completed = fetch_events
            .iter()
            .filter(|event| {
                event.kind() == CpuFetchEventKind::Completed
                    && !state.executed_fetches.contains(&event.request_id())
            })
            .collect::<Vec<_>>();
        if completed.is_empty() || completed.len() >= COMPLETED_FETCH_WINDOW {
            return false;
        }
        completed.sort_by_key(|event| event.request_id().sequence());

        let fetch = completed[0];
        if fetch.pc() != Address::new(state.hart.pc()) {
            return false;
        }
        let Some(data) = fetch.data() else {
            return false;
        };
        let raw = match data {
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => return false,
        };
        let Ok(decoded) = RiscvInstruction::decode_with_length(raw) else {
            return false;
        };
        if decoded.bytes() != 4 {
            return false;
        }

        instruction_allows_fetch_ahead(decoded.instruction())
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

fn instruction_allows_fetch_ahead(instruction: RiscvInstruction) -> bool {
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
    )
}
