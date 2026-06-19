use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvStatusWord};
use rem6_memory::Address;

use crate::{riscv_checker, RiscvCore};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvHartRunState {
    Started,
    StartPending,
    StopPending,
    SuspendPending,
    ResumePending,
    Stopped,
    Suspended,
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

    pub fn set_hart_stop_pending(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::StopPending;
        state.run_state_explicit = true;
    }

    pub fn set_hart_suspend_pending(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::SuspendPending;
        state.run_state_explicit = true;
    }

    pub fn set_hart_resume_pending(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.run_state = RiscvHartRunState::ResumePending;
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
        self.enter_supervisor_hart_if(Some(RiscvHartRunState::StartPending), entry, opaque, true)
    }

    pub fn complete_pending_hart_stop(&self) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        if state.run_state != RiscvHartRunState::StopPending {
            return false;
        }
        state.run_state = RiscvHartRunState::Stopped;
        state.run_state_explicit = true;
        true
    }

    pub fn complete_pending_hart_suspend(&self) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        if state.run_state != RiscvHartRunState::SuspendPending {
            return false;
        }
        state.run_state = if state.hart.machine_interrupt_pending() != 0 {
            RiscvHartRunState::Started
        } else {
            RiscvHartRunState::Suspended
        };
        state.run_state_explicit = true;
        true
    }

    pub fn resume_nonretentive_supervisor_hart(&self, entry: Address, opaque: u64) {
        self.enter_supervisor_hart(entry, opaque, true);
    }

    pub fn resume_pending_nonretentive_supervisor_hart(&self, entry: Address, opaque: u64) -> bool {
        self.enter_supervisor_hart_if(Some(RiscvHartRunState::ResumePending), entry, opaque, true)
    }

    fn enter_supervisor_hart(&self, entry: Address, opaque: u64, reset_supervisor_state: bool) {
        self.enter_supervisor_hart_if(None, entry, opaque, reset_supervisor_state);
    }

    fn enter_supervisor_hart_if(
        &self,
        expected_run_state: Option<RiscvHartRunState>,
        entry: Address,
        opaque: u64,
        reset_supervisor_state: bool,
    ) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        if let Some(expected_run_state) = expected_run_state {
            if state.run_state != expected_run_state {
                return false;
            }
        }
        let hart_id = state.hart.hart_id();
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
        state.discard_branch_speculations();
        state.issued_data_for_fetches.clear();
        state.pending_data_translations.clear();
        state.ready_translated_data.clear();
        state.outstanding_data.clear();
        state.pending_trap = None;
        state.pending_trap_event = None;
        state.reservation = None;
        riscv_checker::sync_checker_hart(&mut state);
        drop(state);
        self.core.reset_fetch_stream_to_pc(entry);
        true
    }
}
