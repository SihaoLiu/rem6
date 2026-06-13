use rem6_cpu::RiscvCore;
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};

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

fn register(index: u8) -> Register {
    Register::new(index).expect("valid RISC-V integer register")
}
