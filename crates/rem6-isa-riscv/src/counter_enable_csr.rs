use crate::{
    write_register, Register, RegisterWrite, RiscvCounterEnableCsr,
    RiscvCounterEnableCsrInstruction, RiscvCsrOp, RiscvCsrOperand, RiscvHartState,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    instruction: RiscvCounterEnableCsrInstruction,
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

fn read(hart: &RiscvHartState, csr: RiscvCounterEnableCsr) -> u64 {
    hart.read_counter_enable_csr(csr)
}

fn write(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvCounterEnableCsr,
    value: u64,
) {
    let old_value = read(hart, csr);
    write_register(hart, writes, register, old_value);
    hart.write_counter_enable_csr(csr, value);
}

fn operand(hart: &RiscvHartState, instruction: RiscvCounterEnableCsrInstruction) -> u64 {
    match instruction.operand() {
        RiscvCsrOperand::Register(register) => hart.read(register),
        RiscvCsrOperand::Immediate(value) => u64::from(value),
    }
}
