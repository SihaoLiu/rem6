use rem6_cpu::RiscvCore;
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};

use crate::RiscvSystemRunDriver;

const SBI_SUCCESS: u64 = 0;
const SBI_ERR_NOT_SUPPORTED: u64 = (-2_i64) as u64;
const SBI_BASE_EXTENSION: u64 = 0x10;
const SBI_BASE_GET_SPEC_VERSION: u64 = 0;
const SBI_SPEC_VERSION_0_2: u64 = 2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvSbiFirmware;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSbiRequest {
    extension: u64,
    function: u64,
}

impl RiscvSbiRequest {
    pub fn from_pending_core_trap(core: &RiscvCore) -> Option<Self> {
        let trap = core.pending_trap()?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        if core.privilege_mode() != RiscvPrivilegeMode::Machine {
            return None;
        }
        if core.pending_trap_return_privilege_mode()? != RiscvPrivilegeMode::Supervisor {
            return None;
        }

        Some(Self {
            extension: core.read_register(register(17)),
            function: core.read_register(register(16)),
        })
    }

    pub const fn extension(self) -> u64 {
        self.extension
    }

    pub const fn function(self) -> u64 {
        self.function
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSbiOutcome {
    Return { error: u64, value: u64 },
}

impl RiscvSbiOutcome {
    pub const fn success(value: u64) -> Self {
        Self::Return {
            error: SBI_SUCCESS,
            value,
        }
    }

    pub const fn not_supported() -> Self {
        Self::Return {
            error: SBI_ERR_NOT_SUPPORTED,
            value: 0,
        }
    }
}

impl RiscvSbiFirmware {
    pub const fn new() -> Self {
        Self
    }

    pub fn handle_pending_core_trap(&self, core: &RiscvCore) -> Option<RiscvSbiOutcome> {
        let request = RiscvSbiRequest::from_pending_core_trap(core)?;
        Some(match (request.extension(), request.function()) {
            (SBI_BASE_EXTENSION, SBI_BASE_GET_SPEC_VERSION) => {
                RiscvSbiOutcome::success(SBI_SPEC_VERSION_0_2)
            }
            _ => RiscvSbiOutcome::not_supported(),
        })
    }
}

impl RiscvSystemRunDriver {
    pub fn with_riscv_sbi_firmware(mut self) -> Self {
        self.riscv_sbi_firmware = Some(RiscvSbiFirmware::new());
        self
    }

    pub const fn riscv_sbi_firmware(&self) -> Option<&RiscvSbiFirmware> {
        self.riscv_sbi_firmware.as_ref()
    }
}

fn register(index: u8) -> Register {
    Register::new(index).expect("valid RISC-V integer register")
}
