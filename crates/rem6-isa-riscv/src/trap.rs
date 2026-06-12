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

pub(crate) fn enter_pending_interrupt(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    pc: u64,
) -> Option<RiscvExecutionRecord> {
    let pending = hart.machine_interrupt_pending() & hart.machine_interrupt_enable();
    if pending == 0 {
        return None;
    }

    let previous_privilege = hart.privilege_mode();
    let delegated = pending & hart.machine_interrupt_delegation();
    let machine_pending = pending & !hart.machine_interrupt_delegation();
    if machine_interrupt_allowed(hart, previous_privilege) {
        if let Some(code) = interrupt_code(machine_pending) {
            return Some(enter_machine_trap(
                hart,
                instruction,
                instruction_bytes,
                pc,
                RiscvTrapKind::Interrupt { code },
            ));
        }
    }

    if supervisor_interrupt_allowed(hart, previous_privilege) {
        if let Some(code) = interrupt_code(delegated) {
            let cause = interrupt_trap_cause(code);
            return Some(enter_supervisor_trap(
                hart,
                instruction,
                instruction_bytes,
                pc,
                RiscvTrapKind::Interrupt { code },
                cause,
                previous_privilege,
            ));
        }
    }

    None
}

pub(crate) fn enter_synchronous_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    pc: u64,
    kind: RiscvTrapKind,
) -> RiscvExecutionRecord {
    let previous_privilege = hart.privilege_mode();
    let cause = machine_trap_cause(kind, previous_privilege);
    if exception_delegated_to_supervisor(hart, previous_privilege, cause) {
        enter_supervisor_trap(
            hart,
            instruction,
            instruction_bytes,
            pc,
            kind,
            cause,
            previous_privilege,
        )
    } else {
        enter_machine_trap(hart, instruction, instruction_bytes, pc, kind)
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
    instruction_bytes: u8,
    pc: u64,
    kind: RiscvTrapKind,
    cause: u64,
    previous_privilege: RiscvPrivilegeMode,
) -> RiscvExecutionRecord {
    let handler_pc = trap_handler_pc(hart.supervisor_trap_vector(), kind);
    hart.set_supervisor_exception_pc(pc);
    hart.set_supervisor_trap_cause(cause);
    hart.set_supervisor_trap_value(trap_value(kind));
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let status = hart.status();
    hart.set_status(
        status
            .with_spp(previous_privilege)
            .with_spie(status.sie())
            .with_sie(false),
    );
    hart.set_pc(handler_pc);
    RiscvExecutionRecord::with_trap_with_instruction_bytes(
        instruction,
        instruction_bytes,
        pc,
        handler_pc,
        RiscvTrap::new(kind, pc),
    )
}

fn enter_machine_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    pc: u64,
    kind: RiscvTrapKind,
) -> RiscvExecutionRecord {
    let previous_privilege = hart.privilege_mode();
    let cause = machine_trap_cause(kind, previous_privilege);
    let handler_pc = trap_handler_pc(hart.machine_trap_vector(), kind);
    hart.set_machine_exception_pc(pc);
    hart.set_machine_trap_cause(cause);
    hart.set_machine_trap_value(trap_value(kind));
    hart.set_privilege_mode(RiscvPrivilegeMode::Machine);
    let status = hart.status();
    hart.set_status(
        status
            .with_mpp(previous_privilege)
            .with_mpie(status.mie())
            .with_mie(false),
    );
    hart.set_pc(handler_pc);
    RiscvExecutionRecord::with_trap_with_instruction_bytes(
        instruction,
        instruction_bytes,
        pc,
        handler_pc,
        RiscvTrap::new(kind, pc),
    )
}

const fn trap_handler_pc(vector: u64, kind: RiscvTrapKind) -> u64 {
    let base = vector & !0b11;
    match (vector & 0b11, kind) {
        (1, RiscvTrapKind::Interrupt { code }) => base.wrapping_add(code.wrapping_mul(4)),
        _ => base,
    }
}

fn machine_interrupt_allowed(hart: &RiscvHartState, privilege: RiscvPrivilegeMode) -> bool {
    match privilege {
        RiscvPrivilegeMode::User | RiscvPrivilegeMode::Supervisor => true,
        RiscvPrivilegeMode::Machine => hart.status().mie(),
    }
}

fn supervisor_interrupt_allowed(hart: &RiscvHartState, privilege: RiscvPrivilegeMode) -> bool {
    match privilege {
        RiscvPrivilegeMode::User => true,
        RiscvPrivilegeMode::Supervisor => hart.status().sie(),
        RiscvPrivilegeMode::Machine => false,
    }
}

fn interrupt_code(pending: u64) -> Option<u64> {
    (pending != 0).then(|| u64::from(pending.trailing_zeros()))
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
        RiscvTrapKind::InstructionPageFault { .. } => 12,
        RiscvTrapKind::LoadPageFault { .. } => 13,
        RiscvTrapKind::StorePageFault { .. } => 15,
        RiscvTrapKind::Interrupt { code } => interrupt_trap_cause(code),
    }
}

const fn interrupt_trap_cause(code: u64) -> u64 {
    (1_u64 << 63) | code
}

const fn trap_value(kind: RiscvTrapKind) -> u64 {
    match kind {
        RiscvTrapKind::InstructionPageFault { address }
        | RiscvTrapKind::LoadPageFault { address }
        | RiscvTrapKind::StorePageFault { address } => address,
        RiscvTrapKind::IllegalInstruction
        | RiscvTrapKind::EnvironmentCall
        | RiscvTrapKind::Breakpoint
        | RiscvTrapKind::Interrupt { .. } => 0,
    }
}
