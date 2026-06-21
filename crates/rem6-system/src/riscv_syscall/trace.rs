use rem6_cpu::CpuId;
use rem6_kernel::Tick;

use super::{RiscvSyscallOutcome, RiscvSyscallRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallTraceOutcome {
    Blocked,
    Exit { code: i32 },
    Return { value: u64 },
}

impl RiscvSyscallTraceOutcome {
    pub const fn from_outcome(outcome: RiscvSyscallOutcome) -> Self {
        match outcome {
            RiscvSyscallOutcome::Blocked => Self::Blocked,
            RiscvSyscallOutcome::Exit { code } => Self::Exit { code },
            RiscvSyscallOutcome::Return { value } => Self::Return { value },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSyscallTraceRecord {
    cpu: CpuId,
    pc: u64,
    number: u64,
    arguments: [u64; 6],
    tick: Tick,
    outcome: RiscvSyscallTraceOutcome,
}

impl RiscvSyscallTraceRecord {
    pub const fn from_request_outcome(
        cpu: CpuId,
        request: RiscvSyscallRequest,
        tick: Tick,
        outcome: RiscvSyscallOutcome,
    ) -> Self {
        Self {
            cpu,
            pc: request.pc(),
            number: request.number(),
            arguments: request.arguments(),
            tick,
            outcome: RiscvSyscallTraceOutcome::from_outcome(outcome),
        }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn number(&self) -> u64 {
        self.number
    }

    pub const fn arguments(&self) -> [u64; 6] {
        self.arguments
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn outcome(&self) -> RiscvSyscallTraceOutcome {
        self.outcome
    }
}
