use crate::{FloatRegister, MemoryAccessKind, Register, RiscvInstruction};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvTrapKind {
    IllegalInstruction,
    EnvironmentCall,
    Breakpoint,
    InstructionPageFault { address: u64 },
    LoadPageFault { address: u64 },
    StorePageFault { address: u64 },
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
    Gem5ResetStats {
        pc: u64,
        delay: u64,
        period: u64,
    },
    Gem5DumpStats {
        pc: u64,
        delay: u64,
        period: u64,
    },
    Gem5DumpResetStats {
        pc: u64,
        delay: u64,
        period: u64,
    },
    Gem5Checkpoint {
        pc: u64,
        delay: u64,
        period: u64,
    },
    Gem5SwitchCpu {
        pc: u64,
    },
    Gem5Hypercall {
        pc: u64,
        selector: u64,
        arguments: [u64; 5],
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
pub struct FloatRegisterWrite {
    register: FloatRegister,
    value: u64,
}

impl FloatRegisterWrite {
    pub const fn new(register: FloatRegister, value: u64) -> Self {
        Self { register, value }
    }

    pub const fn register(&self) -> FloatRegister {
        self.register
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvExecutionRecord {
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    pc: u64,
    next_pc: u64,
    register_writes: Vec<RegisterWrite>,
    float_register_writes: Vec<FloatRegisterWrite>,
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
        Self::new_with_instruction_bytes(
            instruction,
            4,
            pc,
            next_pc,
            register_writes,
            memory_access,
        )
    }

    pub fn new_with_instruction_bytes(
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        pc: u64,
        next_pc: u64,
        register_writes: Vec<RegisterWrite>,
        memory_access: Option<MemoryAccessKind>,
    ) -> Self {
        Self::new_with_instruction_bytes_and_float_register_writes(
            instruction,
            instruction_bytes,
            pc,
            next_pc,
            register_writes,
            Vec::new(),
            memory_access,
        )
    }

    pub fn new_with_instruction_bytes_and_float_register_writes(
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        pc: u64,
        next_pc: u64,
        register_writes: Vec<RegisterWrite>,
        float_register_writes: Vec<FloatRegisterWrite>,
        memory_access: Option<MemoryAccessKind>,
    ) -> Self {
        Self {
            instruction,
            instruction_bytes,
            pc,
            next_pc,
            register_writes,
            float_register_writes,
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
        Self::with_system_event_and_register_writes_with_instruction_bytes(
            instruction,
            4,
            pc,
            next_pc,
            system_event,
            register_writes,
        )
    }

    pub fn with_system_event_and_register_writes_with_instruction_bytes(
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        pc: u64,
        next_pc: u64,
        system_event: RiscvSystemEvent,
        register_writes: Vec<RegisterWrite>,
    ) -> Self {
        Self {
            instruction,
            instruction_bytes,
            pc,
            next_pc,
            register_writes,
            float_register_writes: Vec::new(),
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
        Self::with_trap_with_instruction_bytes(instruction, 4, pc, next_pc, trap)
    }

    pub fn with_trap_with_instruction_bytes(
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        pc: u64,
        next_pc: u64,
        trap: RiscvTrap,
    ) -> Self {
        Self {
            instruction,
            instruction_bytes,
            pc,
            next_pc,
            register_writes: Vec::new(),
            float_register_writes: Vec::new(),
            memory_access: None,
            trap: Some(trap),
            system_event: None,
        }
    }

    pub const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub const fn instruction_bytes(&self) -> u8 {
        self.instruction_bytes
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

    pub fn float_register_writes(&self) -> &[FloatRegisterWrite] {
        &self.float_register_writes
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
