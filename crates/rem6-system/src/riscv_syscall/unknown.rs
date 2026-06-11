use rem6_kernel::Tick;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvUnknownSyscallRecord {
    pc: u64,
    number: u64,
    arguments: [u64; 6],
    tick: Tick,
}

impl RiscvUnknownSyscallRecord {
    pub const fn new(pc: u64, number: u64, arguments: [u64; 6], tick: Tick) -> Self {
        Self {
            pc,
            number,
            arguments,
            tick,
        }
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
}
