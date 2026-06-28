use crate::{
    write_register, Register, RegisterWrite, RiscvGdbXlen, RiscvHartState, RiscvInstruction,
    RiscvStatusCsr,
};

pub(crate) fn instruction_csr(instruction: RiscvInstruction) -> Option<RiscvStatusCsr> {
    match instruction {
        RiscvInstruction::ReadStatusCsr { csr, .. }
        | RiscvInstruction::WriteStatusCsr { csr, .. }
        | RiscvInstruction::SetStatusCsr { csr, .. }
        | RiscvInstruction::ClearStatusCsr { csr, .. }
        | RiscvInstruction::WriteStatusCsrImmediate { csr, .. }
        | RiscvInstruction::SetStatusCsrImmediate { csr, .. }
        | RiscvInstruction::ClearStatusCsrImmediate { csr, .. } => Some(csr),
        _ => None,
    }
}

pub(crate) fn allowed(hart: &RiscvHartState, csr: RiscvStatusCsr) -> bool {
    csr != RiscvStatusCsr::Mstatush || hart.xlen() == RiscvGdbXlen::Rv32
}

pub(crate) fn read(hart: &RiscvHartState, csr: RiscvStatusCsr) -> u64 {
    csr.read_for_xlen(hart.xlen(), hart.status())
}

pub(crate) fn write(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvStatusCsr,
    value: u64,
) {
    let old_value = read(hart, csr);
    write_register(hart, writes, register, old_value);
    hart.set_status(csr.write_for_xlen(hart.xlen(), hart.status(), value));
}
