use crate::{
    RiscvExecutionRecord, RiscvHartState, RiscvInstruction, RiscvPrivilegeMode, RiscvTrap,
    RiscvTrapKind,
};

pub(crate) const fn supervisor_return_allowed(privilege: RiscvPrivilegeMode) -> bool {
    !matches!(privilege, RiscvPrivilegeMode::User)
}

pub(crate) const fn machine_return_allowed(privilege: RiscvPrivilegeMode) -> bool {
    matches!(privilege, RiscvPrivilegeMode::Machine)
}

pub(crate) fn enter_synchronous_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    pc: u64,
    kind: RiscvTrapKind,
) -> RiscvExecutionRecord {
    let previous_privilege = hart.privilege_mode();
    let cause = machine_trap_cause(kind, previous_privilege);
    if exception_delegated_to_supervisor(hart, previous_privilege, cause) {
        enter_supervisor_trap(hart, instruction, pc, kind, cause, previous_privilege)
    } else {
        enter_machine_trap(hart, instruction, pc, kind)
    }
}

fn exception_delegated_to_supervisor(
    hart: &RiscvHartState,
    previous_privilege: RiscvPrivilegeMode,
    cause: u64,
) -> bool {
    if matches!(previous_privilege, RiscvPrivilegeMode::Machine) || cause >= u64::BITS as u64 {
        return false;
    }
    (hart.machine_exception_delegation() & (1_u64 << (cause as u32))) != 0
}

fn enter_supervisor_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    pc: u64,
    kind: RiscvTrapKind,
    cause: u64,
    previous_privilege: RiscvPrivilegeMode,
) -> RiscvExecutionRecord {
    let handler_pc = hart.supervisor_trap_vector() & !0b11;
    hart.set_supervisor_exception_pc(pc);
    hart.set_supervisor_trap_cause(cause);
    hart.set_supervisor_trap_value(0);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let status = hart.status();
    hart.set_status(
        status
            .with_spp(previous_privilege)
            .with_spie(status.sie())
            .with_sie(false),
    );
    hart.set_pc(handler_pc);
    RiscvExecutionRecord::with_trap(instruction, pc, handler_pc, RiscvTrap::new(kind, pc))
}

fn enter_machine_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    pc: u64,
    kind: RiscvTrapKind,
) -> RiscvExecutionRecord {
    let previous_privilege = hart.privilege_mode();
    let cause = machine_trap_cause(kind, previous_privilege);
    let handler_pc = hart.machine_trap_vector() & !0b11;
    hart.set_machine_exception_pc(pc);
    hart.set_machine_trap_cause(cause);
    hart.set_machine_trap_value(0);
    hart.set_privilege_mode(RiscvPrivilegeMode::Machine);
    let status = hart.status();
    hart.set_status(
        status
            .with_mpp(previous_privilege)
            .with_mpie(status.mie())
            .with_mie(false),
    );
    hart.set_pc(handler_pc);
    RiscvExecutionRecord::with_trap(instruction, pc, handler_pc, RiscvTrap::new(kind, pc))
}

const fn machine_trap_cause(kind: RiscvTrapKind, privilege: RiscvPrivilegeMode) -> u64 {
    match kind {
        RiscvTrapKind::IllegalInstruction => 2,
        RiscvTrapKind::EnvironmentCall => match privilege {
            RiscvPrivilegeMode::User => 8,
            RiscvPrivilegeMode::Supervisor => 9,
            RiscvPrivilegeMode::Machine => 11,
        },
        RiscvTrapKind::Breakpoint => 3,
    }
}
