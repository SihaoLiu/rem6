use crate::{MemoryAccessKind, Register, RiscvInstruction};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvTrapKind {
    IllegalInstruction,
    EnvironmentCall,
    Breakpoint,
    Interrupt { code: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTrap {
    kind: RiscvTrapKind,
    pc: u64,
}

impl RiscvTrap {
    pub const fn new(kind: RiscvTrapKind, pc: u64) -> Self {
        Self { kind, pc }
    }

    pub const fn kind(self) -> RiscvTrapKind {
        self.kind
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSystemEvent {
    WaitForInterrupt {
        pc: u64,
    },
    SfenceVma {
        pc: u64,
        virtual_address: Option<u64>,
        address_space: Option<u64>,
    },
    Gem5Exit {
        pc: u64,
        delay: u64,
    },
    Gem5Fail {
        pc: u64,
        delay: u64,
        code: u64,
    },
    Gem5WorkBegin {
        pc: u64,
        work_id: u64,
        thread_id: u64,
    },
    Gem5WorkEnd {
        pc: u64,
        work_id: u64,
        thread_id: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterWrite {
    register: Register,
    value: u64,
}

impl RegisterWrite {
    pub const fn new(register: Register, value: u64) -> Self {
        Self { register, value }
    }

    pub const fn register(&self) -> Register {
        self.register
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvExecutionRecord {
    instruction: RiscvInstruction,
    pc: u64,
    next_pc: u64,
    register_writes: Vec<RegisterWrite>,
    memory_access: Option<MemoryAccessKind>,
    trap: Option<RiscvTrap>,
    system_event: Option<RiscvSystemEvent>,
}

impl RiscvExecutionRecord {
    pub fn new(
        instruction: RiscvInstruction,
        pc: u64,
        next_pc: u64,
        register_writes: Vec<RegisterWrite>,
        memory_access: Option<MemoryAccessKind>,
    ) -> Self {
        Self {
            instruction,
            pc,
            next_pc,
            register_writes,
            memory_access,
            trap: None,
            system_event: None,
        }
    }

    pub fn with_system_event(
        instruction: RiscvInstruction,
        pc: u64,
        next_pc: u64,
        system_event: RiscvSystemEvent,
    ) -> Self {
        Self::with_system_event_and_register_writes(
            instruction,
            pc,
            next_pc,
            system_event,
            Vec::new(),
        )
    }

    pub fn with_system_event_and_register_writes(
        instruction: RiscvInstruction,
        pc: u64,
        next_pc: u64,
        system_event: RiscvSystemEvent,
        register_writes: Vec<RegisterWrite>,
    ) -> Self {
        Self {
            instruction,
            pc,
            next_pc,
            register_writes,
            memory_access: None,
            trap: None,
            system_event: Some(system_event),
        }
    }

    pub fn with_trap(
        instruction: RiscvInstruction,
        pc: u64,
        next_pc: u64,
        trap: RiscvTrap,
    ) -> Self {
        Self {
            instruction,
            pc,
            next_pc,
            register_writes: Vec::new(),
            memory_access: None,
            trap: Some(trap),
            system_event: None,
        }
    }

    pub const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn next_pc(&self) -> u64 {
        self.next_pc
    }

    pub fn register_writes(&self) -> &[RegisterWrite] {
        &self.register_writes
    }

    pub fn memory_access(&self) -> Option<&MemoryAccessKind> {
        self.memory_access.as_ref()
    }

    pub fn trap(&self) -> Option<&RiscvTrap> {
        self.trap.as_ref()
    }

    pub fn system_event(&self) -> Option<&RiscvSystemEvent> {
        self.system_event.as_ref()
    }
}
