use rem6_isa_riscv::{
    RiscvCounterEnableCsr, RiscvCounterInhibitCsr, RiscvEnvironmentConfigCsr, RiscvHartState,
    RiscvMachineTrapCsr,
};

pub(super) fn read_machine_trap_csr(hart: &RiscvHartState, csr: RiscvMachineTrapCsr) -> u64 {
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

pub(super) fn write_machine_trap_csr(
    hart: &mut RiscvHartState,
    csr: RiscvMachineTrapCsr,
    value: u64,
) {
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

pub(super) fn read_environment_config_csr(
    hart: &RiscvHartState,
    csr: RiscvEnvironmentConfigCsr,
) -> u64 {
    match csr {
        RiscvEnvironmentConfigCsr::Senvcfg => hart.supervisor_environment_config(),
        RiscvEnvironmentConfigCsr::Menvcfg => hart.machine_environment_config(),
    }
}

pub(super) fn write_environment_config_csr(
    hart: &mut RiscvHartState,
    csr: RiscvEnvironmentConfigCsr,
    value: u64,
) {
    match csr {
        RiscvEnvironmentConfigCsr::Senvcfg => hart.set_supervisor_environment_config(value),
        RiscvEnvironmentConfigCsr::Menvcfg => hart.set_machine_environment_config(value),
    }
}

pub(super) fn read_counter_enable_csr(hart: &RiscvHartState, csr: RiscvCounterEnableCsr) -> u64 {
    match csr {
        RiscvCounterEnableCsr::Scounteren => hart.supervisor_counter_enable(),
        RiscvCounterEnableCsr::Mcounteren => hart.machine_counter_enable(),
    }
}

pub(super) fn write_counter_enable_csr(
    hart: &mut RiscvHartState,
    csr: RiscvCounterEnableCsr,
    value: u64,
) {
    match csr {
        RiscvCounterEnableCsr::Scounteren => hart.set_supervisor_counter_enable(value),
        RiscvCounterEnableCsr::Mcounteren => hart.set_machine_counter_enable(value),
    }
}

pub(super) fn read_counter_inhibit_csr(hart: &RiscvHartState, csr: RiscvCounterInhibitCsr) -> u64 {
    match csr {
        RiscvCounterInhibitCsr::Mcountinhibit => hart.machine_counter_inhibit(),
    }
}

pub(super) fn write_counter_inhibit_csr(
    hart: &mut RiscvHartState,
    csr: RiscvCounterInhibitCsr,
    value: u64,
) {
    match csr {
        RiscvCounterInhibitCsr::Mcountinhibit => hart.set_machine_counter_inhibit(value),
    }
}
