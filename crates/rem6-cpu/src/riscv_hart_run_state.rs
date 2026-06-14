use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvStatusWord};
use rem6_memory::Address;

use crate::{CpuCore, RiscvCore};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvHartRunState {
    Started,
    StartPending,
    Stopped,
    Suspended,
}

impl CpuCore {
    pub(super) fn reset_fetch_to_pc(&self, pc: Address) {
        let mut state = self.state.lock().expect("cpu core lock");
        state.pc = pc;
        state.outstanding.clear();
        state.events.clear();
    }
}

impl RiscvCore {
    pub fn hart_run_state(&self) -> RiscvHartRunState {
        self.state.lock().expect("riscv core lock").run_state
    }

    pub fn is_hart_started(&self) -> bool {
        self.hart_run_state() == RiscvHartRunState::Started
    }

    pub fn has_explicit_hart_run_state(&self) -> bool {
        self.state
            .lock()
            .expect("riscv core lock")
            .run_state_explicit
    }

    pub fn set_hart_started(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::Started;
        state.run_state_explicit = true;
    }

    pub fn set_hart_start_pending(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::StartPending;
        state.run_state_explicit = true;
    }

    pub fn set_hart_stopped(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::Stopped;
        state.run_state_explicit = true;
    }

    pub fn set_hart_suspended(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::Suspended;
        state.run_state_explicit = true;
    }

    pub fn start_supervisor_hart(&self, entry: Address, opaque: u64) {
        self.enter_supervisor_hart(entry, opaque, true);
    }

    pub fn complete_pending_supervisor_hart_start(&self, entry: Address, opaque: u64) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        if state.run_state != RiscvHartRunState::StartPending {
            return false;
        }
        let hart_id = state.hart.hart_id();
        state.run_state = RiscvHartRunState::Started;
        state.run_state_explicit = true;
        state.hart.set_pc(entry.get());
        state
            .hart
            .set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        state.hart.set_translation_satp(0);
        state.hart.set_status(RiscvStatusWord::new(0));
        state.hart.write(
            Register::new(10).expect("valid RISC-V integer register"),
            hart_id,
        );
        state.hart.write(
            Register::new(11).expect("valid RISC-V integer register"),
            opaque,
        );
        state.pending_fetch_prefix = None;
        state.executed_fetches.clear();
        state.issued_data_for_fetches.clear();
        state.pending_data_translations.clear();
        state.ready_translated_data.clear();
        state.outstanding_data.clear();
        state.pending_trap = None;
        state.pending_trap_event = None;
        state.reservation = None;
        drop(state);
        self.core.reset_fetch_to_pc(entry);
        true
    }

    pub fn resume_nonretentive_supervisor_hart(&self, entry: Address, opaque: u64) {
        self.enter_supervisor_hart(entry, opaque, true);
    }

    fn enter_supervisor_hart(&self, entry: Address, opaque: u64, reset_supervisor_state: bool) {
        let hart_id = self.hart_id();
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::Started;
        state.run_state_explicit = true;
        state.hart.set_pc(entry.get());
        state
            .hart
            .set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        if reset_supervisor_state {
            state.hart.set_translation_satp(0);
            state.hart.set_status(RiscvStatusWord::new(0));
        }
        state.hart.write(
            Register::new(10).expect("valid RISC-V integer register"),
            hart_id,
        );
        state.hart.write(
            Register::new(11).expect("valid RISC-V integer register"),
            opaque,
        );
        state.pending_fetch_prefix = None;
        state.executed_fetches.clear();
        state.issued_data_for_fetches.clear();
        state.pending_data_translations.clear();
        state.ready_translated_data.clear();
        state.outstanding_data.clear();
        state.pending_trap = None;
        state.pending_trap_event = None;
        state.reservation = None;
        drop(state);
        self.core.reset_fetch_to_pc(entry);
    }
}
