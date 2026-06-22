use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrap, RiscvTrapKind};
use rem6_memory::Address;

use crate::{riscv_checker, RiscvCore};

impl RiscvCore {
    pub fn complete_pending_user_environment_call(&self, return_value: u64) -> Option<RiscvTrap> {
        let mut state = self.state.lock().expect("riscv core lock");
        let trap = state.pending_trap?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        let return_privilege = match state.hart.privilege_mode() {
            RiscvPrivilegeMode::Machine => state.hart.status().mpp(),
            RiscvPrivilegeMode::Supervisor => state.hart.status().spp(),
            RiscvPrivilegeMode::User => RiscvPrivilegeMode::User,
        };
        if return_privilege != RiscvPrivilegeMode::User {
            return None;
        }

        state.pending_trap = None;
        state.pending_trap_event = None;
        state.pending_fetch_prefix = None;
        state.discard_branch_speculations();
        state.hart.set_privilege_mode(RiscvPrivilegeMode::User);
        state.hart.write(
            Register::new(10).expect("valid RISC-V integer register"),
            return_value,
        );
        let next_pc = Address::new(trap.pc().wrapping_add(4));
        state.hart.set_pc(next_pc.get());
        riscv_checker::sync_checker_hart(&mut state);
        drop(state);
        self.core.reset_fetch_stream_to_pc(next_pc);
        Some(trap)
    }

    pub fn complete_pending_supervisor_environment_call(
        &self,
        error: u64,
        value: u64,
    ) -> Option<RiscvTrap> {
        self.complete_pending_supervisor_environment_call_with_registers(error, Some(value))
    }

    pub fn complete_pending_supervisor_legacy_environment_call(
        &self,
        value: u64,
    ) -> Option<RiscvTrap> {
        self.complete_pending_supervisor_environment_call_with_registers(value, None)
    }

    fn complete_pending_supervisor_environment_call_with_registers(
        &self,
        value: u64,
        extra_value: Option<u64>,
    ) -> Option<RiscvTrap> {
        let mut state = self.state.lock().expect("riscv core lock");
        let trap = state.pending_trap?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        let return_privilege = match state.hart.privilege_mode() {
            RiscvPrivilegeMode::Machine => state.hart.status().mpp(),
            RiscvPrivilegeMode::Supervisor => state.hart.status().spp(),
            RiscvPrivilegeMode::User => RiscvPrivilegeMode::User,
        };
        if return_privilege != RiscvPrivilegeMode::Supervisor {
            return None;
        }

        state.pending_trap = None;
        state.pending_trap_event = None;
        state.pending_fetch_prefix = None;
        state.discard_branch_speculations();
        state
            .hart
            .set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        state.hart.write(
            Register::new(10).expect("valid RISC-V integer register"),
            value,
        );
        if let Some(extra_value) = extra_value {
            state.hart.write(
                Register::new(11).expect("valid RISC-V integer register"),
                extra_value,
            );
        }
        let next_pc = Address::new(trap.pc().wrapping_add(4));
        state.hart.set_pc(next_pc.get());
        riscv_checker::sync_checker_hart(&mut state);
        drop(state);
        self.core.reset_fetch_stream_to_pc(next_pc);
        Some(trap)
    }

    pub fn complete_pending_interrupt_delivery(&self) -> Option<RiscvTrap> {
        let mut state = self.state.lock().expect("riscv core lock");
        let trap = state.pending_trap?;
        if !matches!(trap.kind(), RiscvTrapKind::Interrupt { .. }) {
            return None;
        }

        let next_pc = Address::new(state.hart.pc());
        state.pending_trap = None;
        state.pending_trap_event = None;
        state.pending_fetch_prefix = None;
        state.discard_branch_speculations();
        riscv_checker::sync_checker_hart(&mut state);
        drop(state);
        self.core.reset_fetch_stream_to_pc(next_pc);
        Some(trap)
    }
}
