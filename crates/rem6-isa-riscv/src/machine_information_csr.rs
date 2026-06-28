use crate::{
    write_register, RegisterWrite, RiscvCsrOp, RiscvHartState,
    RiscvMachineInformationCsrInstruction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MachineInformationCsrOutcome {
    Complete,
    IllegalInstruction,
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    register_writes: &mut Vec<RegisterWrite>,
    instruction: RiscvMachineInformationCsrInstruction,
) -> MachineInformationCsrOutcome {
    let csr = instruction.csr();
    if instruction.op() != RiscvCsrOp::Read && csr.write_traps() {
        return MachineInformationCsrOutcome::IllegalInstruction;
    }

    write_register(
        hart,
        register_writes,
        instruction.rd(),
        csr.read_for_xlen_bits(hart.hart_id(), hart.xlen().bits() as u8),
    );
    MachineInformationCsrOutcome::Complete
}
