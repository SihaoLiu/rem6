use rem6_isa_riscv::{
    Register, RegisterWrite, RiscvHartState, RiscvInstruction, RiscvMachineTrapCsr,
    RiscvPrivilegeMode, RiscvStatusWord, RiscvTrap, RiscvTrapKind,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn csr_type(csr: u16, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (u32::from(csr) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0b1110011
}

fn csr_read_type(csr: u16, rd: u8) -> u32 {
    csr_type(csr, 0, 0x2, rd)
}

#[test]
fn hart_decodes_and_executes_machine_trap_delegation_csrs() {
    let mut hart = RiscvHartState::new(0x4000);
    hart.write(reg(2), 1 << 8);
    hart.write(reg(3), 1 << 5);
    hart.write(reg(4), 1 << 8);

    let write_medeleg = RiscvInstruction::decode(csr_type(0x302, 2, 0x1, 5)).unwrap();
    let set_mideleg = RiscvInstruction::decode(csr_type(0x303, 3, 0x2, 6)).unwrap();
    let clear_medeleg = RiscvInstruction::decode(csr_type(0x302, 4, 0x3, 7)).unwrap();
    let read_mideleg = RiscvInstruction::decode(csr_read_type(0x303, 8)).unwrap();

    assert_eq!(
        write_medeleg,
        RiscvInstruction::WriteMachineTrapCsr {
            rd: reg(5),
            csr: RiscvMachineTrapCsr::Medeleg,
            rs1: reg(2),
        }
    );
    assert_eq!(
        set_mideleg,
        RiscvInstruction::SetMachineTrapCsr {
            rd: reg(6),
            csr: RiscvMachineTrapCsr::Mideleg,
            rs1: reg(3),
        }
    );

    let write_record = hart.execute(write_medeleg).unwrap();
    assert_eq!(hart.machine_exception_delegation(), 1 << 8);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(
        write_record.register_writes(),
        &[RegisterWrite::new(reg(5), 0)]
    );

    hart.execute(set_mideleg).unwrap();
    assert_eq!(hart.machine_interrupt_delegation(), 1 << 5);
    assert_eq!(hart.read(reg(6)), 0);

    hart.execute(clear_medeleg).unwrap();
    assert_eq!(hart.machine_exception_delegation(), 0);
    assert_eq!(hart.read(reg(7)), 1 << 8);

    hart.execute(read_mideleg).unwrap();
    assert_eq!(hart.read(reg(8)), 1 << 5);
}

#[test]
fn hart_delegates_user_environment_call_to_supervisor_trap_vector() {
    let mut hart = RiscvHartState::new(0x7000);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_supervisor_trap_vector(0x8101);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 8);
    hart.set_status(RiscvStatusWord::new(0).with_sie(true));

    let record = hart
        .execute(RiscvInstruction::decode(0x0000_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7000);
    assert_eq!(record.next_pc(), 0x8100);
    assert_eq!(hart.pc(), 0x8100);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7000);
    assert_eq!(hart.supervisor_trap_cause(), 8);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(hart.status().spie());
    assert!(!hart.status().sie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x7000))
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_delegates_supervisor_environment_call_to_supervisor_trap_vector() {
    let mut hart = RiscvHartState::new(0x7100);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_supervisor_trap_vector(0x8201);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 9);
    hart.set_status(RiscvStatusWord::new(0).with_sie(false));

    let record = hart
        .execute(RiscvInstruction::decode(0x0000_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7100);
    assert_eq!(record.next_pc(), 0x8200);
    assert_eq!(hart.pc(), 0x8200);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7100);
    assert_eq!(hart.supervisor_trap_cause(), 9);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::Supervisor);
    assert!(!hart.status().spie());
    assert!(!hart.status().sie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x7100))
    );
}

#[test]
fn hart_delegates_user_breakpoint_to_supervisor_trap_vector() {
    let mut hart = RiscvHartState::new(0x7180);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_supervisor_trap_vector(0x8301);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 3);
    hart.set_status(RiscvStatusWord::new(0).with_sie(true));

    let record = hart
        .execute(RiscvInstruction::decode(0x0010_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7180);
    assert_eq!(record.next_pc(), 0x8300);
    assert_eq!(hart.pc(), 0x8300);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7180);
    assert_eq!(hart.supervisor_trap_cause(), 3);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(hart.status().spie());
    assert!(!hart.status().sie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::Breakpoint, 0x7180))
    );
}

#[test]
fn hart_keeps_machine_environment_call_in_machine_trap_vector() {
    let mut hart = RiscvHartState::new(0x7200);
    hart.set_privilege_mode(RiscvPrivilegeMode::Machine);
    hart.set_supervisor_trap_vector(0x8100);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 11);

    let record = hart
        .execute(RiscvInstruction::decode(0x0000_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7200);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7200);
    assert_eq!(hart.machine_trap_cause(), 11);
    assert_eq!(hart.supervisor_exception_pc(), 0);
    assert_eq!(hart.supervisor_trap_cause(), 0);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x7200))
    );
}
