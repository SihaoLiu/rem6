use crate::{RiscvInstructionStats, RiscvSystemRunDriver, RiscvTrapEventPort};

impl RiscvSystemRunDriver {
    pub const fn new(trap_port: RiscvTrapEventPort) -> Self {
        Self {
            trap_port,
            instruction_stats: None,
            data_access_stats: None,
            riscv_syscall_emulation: None,
        }
    }

    pub const fn with_instruction_stats(
        trap_port: RiscvTrapEventPort,
        instruction_stats: RiscvInstructionStats,
    ) -> Self {
        Self {
            trap_port,
            instruction_stats: Some(instruction_stats),
            data_access_stats: None,
            riscv_syscall_emulation: None,
        }
    }

    pub const fn trap_port(&self) -> &RiscvTrapEventPort {
        &self.trap_port
    }

    pub const fn instruction_stats(&self) -> Option<&RiscvInstructionStats> {
        self.instruction_stats.as_ref()
    }
}
