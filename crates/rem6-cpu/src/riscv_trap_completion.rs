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

    pub fn complete_pending_supervisor_exception_delivery(&self) -> Option<RiscvTrap> {
        let mut state = self.state.lock().expect("riscv core lock");
        let trap = state.pending_trap?;
        if !matches!(
            trap.kind(),
            RiscvTrapKind::IllegalInstruction
                | RiscvTrapKind::InstructionPageFault { .. }
                | RiscvTrapKind::LoadPageFault { .. }
                | RiscvTrapKind::StorePageFault { .. }
        ) {
            return None;
        }
        if state.hart.privilege_mode() != RiscvPrivilegeMode::Supervisor {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId, CpuResetState};
    use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId, CacheLineLayout, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    fn core_with_pending_trap(
        mode: RiscvPrivilegeMode,
        kind: RiscvTrapKind,
    ) -> (RiscvCore, RiscvTrap) {
        let core = RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(0),
                    PartitionId::new(0),
                    AgentId::new(7),
                    Address::new(0x8000),
                ),
                CpuFetchConfig::new(
                    TransportEndpointId::new("cpu0.ifetch").unwrap(),
                    MemoryRouteId::new(0),
                    CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
        );
        let trap = RiscvTrap::new(kind, 0x8020);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.hart.set_privilege_mode(mode);
            state.hart.set_pc(0x9000);
            state.pending_trap = Some(trap);
        }
        (core, trap)
    }

    fn fetch_event(pc: u64) -> crate::CpuFetchEvent {
        crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRequestId::new(AgentId::new(7), 0),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            vec![0; 4],
        )
    }

    fn pending_trap_event(trap: RiscvTrap) -> crate::RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::decode(0x0000_0013).unwrap();
        crate::RiscvCpuExecutionEvent::new(
            fetch_event(trap.pc()),
            instruction,
            RiscvExecutionRecord::with_trap(instruction, trap.pc(), 0x9000, trap),
        )
    }

    #[test]
    fn supervisor_exception_delivery_consumes_delegated_page_faults() {
        for kind in [
            RiscvTrapKind::InstructionPageFault { address: 0x1000 },
            RiscvTrapKind::LoadPageFault { address: 0x2000 },
            RiscvTrapKind::StorePageFault { address: 0x3000 },
        ] {
            let (core, trap) = core_with_pending_trap(RiscvPrivilegeMode::Supervisor, kind);
            core.state
                .lock()
                .expect("riscv core lock")
                .pending_trap_event = Some(pending_trap_event(trap));

            assert_eq!(
                core.complete_pending_supervisor_exception_delivery(),
                Some(trap)
            );
            let state = core.state.lock().expect("riscv core lock");
            assert_eq!(state.pending_trap, None);
            assert_eq!(state.pending_trap_event, None);
            drop(state);
            assert_eq!(core.inner().pc(), Address::new(0x9000));
        }
    }

    #[test]
    fn supervisor_exception_delivery_keeps_host_stop_traps_pending() {
        for kind in [RiscvTrapKind::EnvironmentCall, RiscvTrapKind::Breakpoint] {
            let (core, trap) = core_with_pending_trap(RiscvPrivilegeMode::Supervisor, kind);

            assert_eq!(core.complete_pending_supervisor_exception_delivery(), None);
            assert_eq!(core.pending_trap(), Some(trap));
        }
    }

    #[test]
    fn supervisor_exception_delivery_waits_until_hart_vectors_to_supervisor() {
        let (core, trap) = core_with_pending_trap(
            RiscvPrivilegeMode::Machine,
            RiscvTrapKind::InstructionPageFault { address: 0x1000 },
        );

        assert_eq!(core.complete_pending_supervisor_exception_delivery(), None);
        assert_eq!(core.pending_trap(), Some(trap));
    }
}
