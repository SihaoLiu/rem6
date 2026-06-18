use rem6_cpu::{CpuId, RiscvCore};
use rem6_kernel::PartitionedScheduler;

use crate::{GuestEventId, RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError};

impl RiscvSystemRunDriver {
    pub(crate) fn schedule_pending_core_events<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: Vec<RiscvCore>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        if self.riscv_sbi_firmware.is_some() {
            self.trap_port
                .schedule_pending_core_traps_with_riscv_emulation(
                    scheduler,
                    cores,
                    self.riscv_sbi_firmware.as_ref(),
                    self.riscv_syscall_emulation.as_ref(),
                    event_for,
                )
        } else if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
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
        if self.riscv_sbi_firmware.is_some() {
            self.trap_port
                .schedule_pending_core_traps_with_riscv_emulation_parallel(
                    scheduler,
                    cores,
                    self.riscv_sbi_firmware.as_ref(),
                    self.riscv_syscall_emulation.as_ref(),
                    event_for,
                )
        } else if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
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
