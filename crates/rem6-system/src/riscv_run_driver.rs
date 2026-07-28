use crate::{RiscvInstructionStats, RiscvO3RuntimeStats, RiscvSystemRunDriver, RiscvTrapEventPort};

impl Clone for RiscvSystemRunDriver {
    fn clone(&self) -> Self {
        Self {
            trap_port: self.trap_port.clone(),
            instruction_stats: self
                .instruction_stats
                .as_ref()
                .map(RiscvInstructionStats::shared),
            o3_runtime_stats: self.o3_runtime_stats.clone(),
            data_access_stats: self.data_access_stats.clone(),
            riscv_sbi_firmware: self.riscv_sbi_firmware.clone(),
            riscv_syscall_emulation: self.riscv_syscall_emulation.clone(),
            o3_runtime_trace_enabled: self.o3_runtime_trace_enabled,
        }
    }
}

impl RiscvSystemRunDriver {
    pub const fn new(trap_port: RiscvTrapEventPort) -> Self {
        Self {
            trap_port,
            instruction_stats: None,
            o3_runtime_stats: None,
            data_access_stats: None,
            riscv_sbi_firmware: None,
            riscv_syscall_emulation: None,
            o3_runtime_trace_enabled: false,
        }
    }

    pub fn with_instruction_stats(
        trap_port: RiscvTrapEventPort,
        instruction_stats: RiscvInstructionStats,
    ) -> Self {
        trap_port
            .controller()
            .lock()
            .expect("system host controller lock")
            .executor_mut()
            .attach_riscv_instruction_stats(&instruction_stats);
        Self {
            trap_port,
            instruction_stats: Some(instruction_stats),
            o3_runtime_stats: None,
            data_access_stats: None,
            riscv_sbi_firmware: None,
            riscv_syscall_emulation: None,
            o3_runtime_trace_enabled: false,
        }
    }

    pub const fn with_o3_runtime_trace_enabled(mut self, enabled: bool) -> Self {
        self.o3_runtime_trace_enabled = enabled;
        self
    }

    pub fn with_o3_runtime_stats(mut self, o3_runtime_stats: RiscvO3RuntimeStats) -> Self {
        self.trap_port
            .controller()
            .lock()
            .expect("system host controller lock")
            .executor_mut()
            .attach_riscv_o3_runtime_stats(o3_runtime_stats.clone());
        self.o3_runtime_stats = Some(o3_runtime_stats);
        self
    }

    pub const fn trap_port(&self) -> &RiscvTrapEventPort {
        &self.trap_port
    }

    pub const fn instruction_stats(&self) -> Option<&RiscvInstructionStats> {
        self.instruction_stats.as_ref()
    }
}
