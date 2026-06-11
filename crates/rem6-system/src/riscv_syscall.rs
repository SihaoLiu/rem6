use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::PartitionedScheduler;

use crate::{GuestEventId, RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError};

const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EXIT_GROUP: u64 = 94;

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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvSyscallTable;

impl RiscvSyscallTable {
    pub const fn new() -> Self {
        Self
    }

    pub fn handle(self, request: RiscvSyscallRequest) -> Option<RiscvSyscallOutcome> {
        match request.number() {
            RISCV_LINUX_EXIT | RISCV_LINUX_EXIT_GROUP => Some(RiscvSyscallOutcome::Exit {
                code: syscall_exit_code(request.argument(0)),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSyscallEmulation {
    table: RiscvSyscallTable,
}

impl RiscvSyscallEmulation {
    pub const fn new(table: RiscvSyscallTable) -> Self {
        Self { table }
    }

    pub const fn linux_user() -> Self {
        Self::new(RiscvSyscallTable::new())
    }

    pub const fn table(self) -> RiscvSyscallTable {
        self.table
    }

    pub fn handle_pending_core_trap(self, core: &RiscvCore) -> Option<RiscvSyscallOutcome> {
        self.table
            .handle(RiscvSyscallRequest::from_pending_core_trap(core)?)
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
        if let Some(syscalls) = self.riscv_syscall_emulation {
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
        if let Some(syscalls) = self.riscv_syscall_emulation {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_table_maps_exit_numbers_to_stop_codes() {
        let table = RiscvSyscallTable::new();

        assert_eq!(
            table.handle(RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT, [17; 6])),
            Some(RiscvSyscallOutcome::Exit { code: 17 })
        );
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_EXIT_GROUP,
                [19; 6]
            )),
            Some(RiscvSyscallOutcome::Exit { code: 19 })
        );
    }

    #[test]
    fn linux_table_leaves_unknown_numbers_for_the_trap_path() {
        assert_eq!(
            RiscvSyscallTable::new().handle(RiscvSyscallRequest::new(0x8000, 214, [0; 6])),
            None
        );
    }
}
