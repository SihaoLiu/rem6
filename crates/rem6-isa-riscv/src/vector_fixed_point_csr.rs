use crate::{
    write_register, Register, RegisterWrite, RiscvCsrOp, RiscvCsrOperand, RiscvHartState,
    RiscvVectorFixedPointState,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvVectorFixedPointCsr {
    Vxsat,
    Vxrm,
    Vcsr,
}

impl RiscvVectorFixedPointCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Vxsat => 0x009,
            Self::Vxrm => 0x00a,
            Self::Vcsr => 0x00f,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x009 => Some(Self::Vxsat),
            0x00a => Some(Self::Vxrm),
            0x00f => Some(Self::Vcsr),
            _ => None,
        }
    }

    pub const fn read(self, state: RiscvVectorFixedPointState) -> u64 {
        match self {
            Self::Vxsat => state.vxsat() as u64,
            Self::Vxrm => state.vxrm_bits() as u64,
            Self::Vcsr => state.vcsr_bits() as u64,
        }
    }

    pub fn write(
        self,
        mut state: RiscvVectorFixedPointState,
        value: u64,
    ) -> RiscvVectorFixedPointState {
        match self {
            Self::Vxsat => state.write_vxsat_bit(value & 0b1 != 0),
            Self::Vxrm => state.write_vxrm_bits(value as u8),
            Self::Vcsr => state.write_vcsr_bits(value as u8),
        }
        state
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvVectorFixedPointCsrInstruction {
    rd: Register,
    csr: RiscvVectorFixedPointCsr,
    op: RiscvCsrOp,
    operand: RiscvCsrOperand,
}

impl RiscvVectorFixedPointCsrInstruction {
    pub const fn read(rd: Register, csr: RiscvVectorFixedPointCsr) -> Self {
        Self {
            rd,
            csr,
            op: RiscvCsrOp::Read,
            operand: RiscvCsrOperand::Immediate(0),
        }
    }

    pub const fn register(
        rd: Register,
        csr: RiscvVectorFixedPointCsr,
        op: RiscvCsrOp,
        rs1: Register,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Register(rs1),
        }
    }

    pub const fn immediate(
        rd: Register,
        csr: RiscvVectorFixedPointCsr,
        op: RiscvCsrOp,
        zimm: u8,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Immediate(zimm),
        }
    }

    pub const fn rd(self) -> Register {
        self.rd
    }

    pub const fn csr(self) -> RiscvVectorFixedPointCsr {
        self.csr
    }

    pub const fn op(self) -> RiscvCsrOp {
        self.op
    }

    pub const fn operand(self) -> RiscvCsrOperand {
        self.operand
    }
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    instruction: RiscvVectorFixedPointCsrInstruction,
) {
    match instruction.op() {
        RiscvCsrOp::Read => {
            write_register(
                hart,
                writes,
                instruction.rd(),
                read(hart, instruction.csr()),
            );
        }
        RiscvCsrOp::Write => write(
            hart,
            writes,
            instruction.rd(),
            instruction.csr(),
            operand(hart, instruction),
        ),
        RiscvCsrOp::Set => {
            let value = read(hart, instruction.csr()) | operand(hart, instruction);
            write(hart, writes, instruction.rd(), instruction.csr(), value);
        }
        RiscvCsrOp::Clear => {
            let value = read(hart, instruction.csr()) & !operand(hart, instruction);
            write(hart, writes, instruction.rd(), instruction.csr(), value);
        }
    }
}

fn read(hart: &RiscvHartState, csr: RiscvVectorFixedPointCsr) -> u64 {
    csr.read(hart.vector_fixed_point())
}

fn write(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvVectorFixedPointCsr,
    value: u64,
) {
    let old_value = read(hart, csr);
    write_register(hart, writes, register, old_value);
    hart.set_vector_fixed_point(csr.write(hart.vector_fixed_point(), value));
}

fn operand(hart: &RiscvHartState, instruction: RiscvVectorFixedPointCsrInstruction) -> u64 {
    match instruction.operand() {
        RiscvCsrOperand::Register(register) => hart.read(register),
        RiscvCsrOperand::Immediate(value) => u64::from(value),
    }
}
