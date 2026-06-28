use crate::{
    write_register, Register, RegisterWrite, RiscvHartState, RiscvMachineTrapCsr,
    RiscvSupervisorTrapCsr,
};

pub(crate) fn read_machine(hart: &RiscvHartState, csr: RiscvMachineTrapCsr) -> u64 {
    match csr {
        RiscvMachineTrapCsr::Medeleg => hart.machine_exception_delegation(),
        RiscvMachineTrapCsr::Mideleg => hart.machine_interrupt_delegation(),
        RiscvMachineTrapCsr::Mtvec => hart.machine_trap_vector(),
        RiscvMachineTrapCsr::Mscratch => hart.machine_scratch(),
        RiscvMachineTrapCsr::Mepc => hart.machine_exception_pc(),
        RiscvMachineTrapCsr::Mcause => hart.machine_trap_cause(),
        RiscvMachineTrapCsr::Mtval => hart.machine_trap_value(),
    }
}

pub(crate) fn write_machine(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvMachineTrapCsr,
    value: u64,
) {
    let old_value = read_machine(hart, csr);
    write_register(hart, writes, register, old_value);
    match csr {
        RiscvMachineTrapCsr::Medeleg => hart.set_machine_exception_delegation(value),
        RiscvMachineTrapCsr::Mideleg => hart.set_machine_interrupt_delegation(value),
        RiscvMachineTrapCsr::Mtvec => hart.set_machine_trap_vector(value),
        RiscvMachineTrapCsr::Mscratch => hart.set_machine_scratch(value),
        RiscvMachineTrapCsr::Mepc => hart.set_machine_exception_pc(value),
        RiscvMachineTrapCsr::Mcause => hart.set_machine_trap_cause(value),
        RiscvMachineTrapCsr::Mtval => hart.set_machine_trap_value(value),
    }
}

pub(crate) fn read_supervisor(hart: &RiscvHartState, csr: RiscvSupervisorTrapCsr) -> u64 {
    match csr {
        RiscvSupervisorTrapCsr::Stvec => hart.supervisor_trap_vector(),
        RiscvSupervisorTrapCsr::Sscratch => hart.supervisor_scratch(),
        RiscvSupervisorTrapCsr::Sepc => hart.supervisor_exception_pc(),
        RiscvSupervisorTrapCsr::Scause => hart.supervisor_trap_cause(),
        RiscvSupervisorTrapCsr::Stval => hart.supervisor_trap_value(),
    }
}

pub(crate) fn write_supervisor(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvSupervisorTrapCsr,
    value: u64,
) {
    let old_value = read_supervisor(hart, csr);
    write_register(hart, writes, register, old_value);
    match csr {
        RiscvSupervisorTrapCsr::Stvec => hart.set_supervisor_trap_vector(value),
        RiscvSupervisorTrapCsr::Sscratch => hart.set_supervisor_scratch(value),
        RiscvSupervisorTrapCsr::Sepc => hart.set_supervisor_exception_pc(value),
        RiscvSupervisorTrapCsr::Scause => hart.set_supervisor_trap_cause(value),
        RiscvSupervisorTrapCsr::Stval => hart.set_supervisor_trap_value(value),
    }
}
