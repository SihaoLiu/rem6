use crate::{
    write_register, Register, RegisterWrite, RiscvCsrOp, RiscvCsrOperand, RiscvHartState,
    RiscvVectorFixedPointCsr, RiscvVectorFixedPointCsrInstruction,
};

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
