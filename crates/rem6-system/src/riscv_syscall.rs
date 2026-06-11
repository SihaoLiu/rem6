use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::PartitionedScheduler;

use crate::{GuestEventId, RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError};

const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EXIT_GROUP: u64 = 94;
const RISCV_LINUX_BRK: u64 = 214;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSyscallRequest {
    pc: u64,
    number: u64,
    arguments: [u64; 6],
}

impl RiscvSyscallRequest {
    pub const fn new(pc: u64, number: u64, arguments: [u64; 6]) -> Self {
        Self {
            pc,
            number,
            arguments,
        }
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }

    pub const fn number(self) -> u64 {
        self.number
    }

    pub const fn arguments(self) -> [u64; 6] {
        self.arguments
    }

    pub const fn argument(self, index: usize) -> u64 {
        self.arguments[index]
    }

    pub fn from_pending_core_trap(core: &RiscvCore) -> Option<Self> {
        let trap = core.pending_trap()?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        if core.pending_trap_return_privilege_mode()? != RiscvPrivilegeMode::User {
            return None;
        }

        Some(Self::new(
            trap.pc(),
            core.read_register(register(17)),
            [
                core.read_register(register(10)),
                core.read_register(register(11)),
                core.read_register(register(12)),
                core.read_register(register(13)),
                core.read_register(register(14)),
                core.read_register(register(15)),
            ],
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallOutcome {
    Exit { code: i32 },
    Return { value: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSyscallState {
    program_break: u64,
}

impl RiscvSyscallState {
    pub const fn new(program_break: u64) -> Self {
        Self { program_break }
    }

    pub const fn program_break(self) -> u64 {
        self.program_break
    }

    fn set_program_break(&mut self, value: u64) {
        self.program_break = value;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvSyscallTable;

impl RiscvSyscallTable {
    pub const fn new() -> Self {
        Self
    }

    pub fn handle(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
    ) -> Option<RiscvSyscallOutcome> {
        match request.number() {
            RISCV_LINUX_EXIT | RISCV_LINUX_EXIT_GROUP => Some(RiscvSyscallOutcome::Exit {
                code: syscall_exit_code(request.argument(0)),
            }),
            RISCV_LINUX_BRK => Some(RiscvSyscallOutcome::Return {
                value: syscall_brk(request.argument(0), state),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RiscvSyscallEmulation {
    table: RiscvSyscallTable,
    state: Arc<Mutex<RiscvSyscallState>>,
}

impl RiscvSyscallEmulation {
    pub fn new(table: RiscvSyscallTable, state: RiscvSyscallState) -> Self {
        Self {
            table,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn linux_user() -> Self {
        Self::new(RiscvSyscallTable::new(), RiscvSyscallState::new(0))
    }

    pub const fn table(&self) -> RiscvSyscallTable {
        self.table
    }

    pub fn state(&self) -> RiscvSyscallState {
        *self.state.lock().expect("RISC-V syscall state lock")
    }

    pub fn handle_pending_core_trap(&self, core: &RiscvCore) -> Option<RiscvSyscallOutcome> {
        let mut state = self.state.lock().expect("RISC-V syscall state lock");
        self.table.handle(
            RiscvSyscallRequest::from_pending_core_trap(core)?,
            &mut state,
        )
    }
}

impl Default for RiscvSyscallEmulation {
    fn default() -> Self {
        Self::linux_user()
    }
}

impl RiscvSystemRunDriver {
    pub fn with_riscv_syscall_emulation(mut self) -> Self {
        self.riscv_syscall_emulation = Some(RiscvSyscallEmulation::linux_user());
        self
    }

    pub const fn riscv_syscall_emulation(&self) -> Option<&RiscvSyscallEmulation> {
        self.riscv_syscall_emulation.as_ref()
    }

    pub(crate) fn schedule_pending_core_events<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: Vec<RiscvCore>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
            self.trap_port
                .schedule_pending_core_traps_with_syscall_emulation(
                    scheduler, cores, syscalls, event_for,
                )
        } else {
            self.trap_port
                .schedule_pending_core_traps(scheduler, cores, event_for)
        }
    }

    pub(crate) fn schedule_pending_core_events_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: Vec<RiscvCore>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
            self.trap_port
                .schedule_pending_core_traps_with_syscall_emulation_parallel(
                    scheduler, cores, syscalls, event_for,
                )
        } else {
            self.trap_port
                .schedule_pending_core_traps_parallel(scheduler, cores, event_for)
        }
    }
}

fn register(index: u8) -> Register {
    Register::new(index).expect("valid RISC-V integer register")
}

fn syscall_exit_code(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

fn syscall_brk(requested: u64, state: &mut RiscvSyscallState) -> u64 {
    if requested != 0 {
        state.set_program_break(requested);
    }
    state.program_break()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_table_maps_exit_numbers_to_stop_codes() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT, [17; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Exit { code: 17 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT_GROUP, [19; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Exit { code: 19 })
        );
    }

    #[test]
    fn linux_table_tracks_program_break() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_BRK, [64, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 64 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_BRK, [0; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 64 })
        );
        assert_eq!(state.program_break(), 64);
    }

    #[test]
    fn linux_table_leaves_unknown_numbers_for_the_trap_path() {
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            RiscvSyscallTable::new()
                .handle(RiscvSyscallRequest::new(0x8000, 9999, [0; 6]), &mut state,),
            None
        );
    }
}
