use crate::{
    write_register, Register, RegisterWrite, RiscvCsrOp, RiscvCsrOperand,
    RiscvEnvironmentConfigCsr, RiscvEnvironmentConfigCsrInstruction, RiscvHartState,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    instruction: RiscvEnvironmentConfigCsrInstruction,
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

fn read(hart: &RiscvHartState, csr: RiscvEnvironmentConfigCsr) -> u64 {
    match csr {
        RiscvEnvironmentConfigCsr::Senvcfg => hart.supervisor_environment_config(),
    }
}

fn write(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvEnvironmentConfigCsr,
    value: u64,
) {
    let old_value = read(hart, csr);
    write_register(hart, writes, register, old_value);
    match csr {
        RiscvEnvironmentConfigCsr::Senvcfg => hart.set_supervisor_environment_config(value),
    }
}

fn operand(hart: &RiscvHartState, instruction: RiscvEnvironmentConfigCsrInstruction) -> u64 {
    match instruction.operand() {
        RiscvCsrOperand::Register(register) => hart.read(register),
        RiscvCsrOperand::Immediate(value) => u64::from(value),
    }
}
