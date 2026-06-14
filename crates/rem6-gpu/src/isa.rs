use crate::GpuWorkgroupId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuScalarRegister(u8);

impl GpuScalarRegister {
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuIsaProgram {
    instructions: Vec<GpuIsaInstruction>,
}

impl GpuIsaProgram {
    pub const fn empty() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }

    pub fn new(instructions: Vec<GpuIsaInstruction>) -> Self {
        Self { instructions }
    }

    pub fn instructions(&self) -> &[GpuIsaInstruction] {
        &self.instructions
    }

    pub(crate) fn execute(&self, workgroup: GpuWorkgroupId) -> GpuWorkgroupIsaState {
        let mut state = GpuWorkgroupIsaState::empty();
        for instruction in &self.instructions {
            instruction.execute(workgroup, &mut state);
            state.advance_pc();
        }
        state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuIsaInstruction {
    LoadWorkgroupId {
        dst: GpuScalarRegister,
    },
    MoveImmediate {
        dst: GpuScalarRegister,
        value: i64,
    },
    AddImmediate {
        dst: GpuScalarRegister,
        src: GpuScalarRegister,
        immediate: i64,
    },
}

impl GpuIsaInstruction {
    pub const fn load_workgroup_id(dst: GpuScalarRegister) -> Self {
        Self::LoadWorkgroupId { dst }
    }

    pub const fn move_immediate(dst: GpuScalarRegister, value: i64) -> Self {
        Self::MoveImmediate { dst, value }
    }

    pub const fn add_immediate(
        dst: GpuScalarRegister,
        src: GpuScalarRegister,
        immediate: i64,
    ) -> Self {
        Self::AddImmediate {
            dst,
            src,
            immediate,
        }
    }

    fn execute(self, workgroup: GpuWorkgroupId, state: &mut GpuWorkgroupIsaState) {
        match self {
            Self::LoadWorkgroupId { dst } => state.write_scalar(dst, i64::from(workgroup.get())),
            Self::MoveImmediate { dst, value } => state.write_scalar(dst, value),
            Self::AddImmediate {
                dst,
                src,
                immediate,
            } => {
                let value = state
                    .scalar_register(src)
                    .unwrap_or_default()
                    .wrapping_add(immediate);
                state.write_scalar(dst, value);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuWorkgroupIsaState {
    pc: usize,
    scalar_registers: Vec<(GpuScalarRegister, i64)>,
}

impl GpuWorkgroupIsaState {
    pub const fn empty() -> Self {
        Self {
            pc: 0,
            scalar_registers: Vec::new(),
        }
    }

    pub fn from_scalar_registers<const N: usize>(
        pc: usize,
        scalar_registers: [(GpuScalarRegister, i64); N],
    ) -> Self {
        let mut scalar_registers = scalar_registers.to_vec();
        scalar_registers.sort_by_key(|(register, _value)| *register);
        scalar_registers.dedup_by_key(|(register, _value)| *register);
        Self {
            pc,
            scalar_registers,
        }
    }

    pub const fn pc(&self) -> usize {
        self.pc
    }

    pub fn scalar_register(&self, register: GpuScalarRegister) -> Option<i64> {
        self.scalar_registers
            .iter()
            .find_map(|(candidate, value)| (*candidate == register).then_some(*value))
    }

    pub fn scalar_registers(&self) -> &[(GpuScalarRegister, i64)] {
        &self.scalar_registers
    }

    fn advance_pc(&mut self) {
        self.pc += 1;
    }

    fn write_scalar(&mut self, register: GpuScalarRegister, value: i64) {
        match self
            .scalar_registers
            .binary_search_by_key(&register, |(candidate, _value)| *candidate)
        {
            Ok(index) => self.scalar_registers[index].1 = value,
            Err(index) => self.scalar_registers.insert(index, (register, value)),
        }
    }
}
