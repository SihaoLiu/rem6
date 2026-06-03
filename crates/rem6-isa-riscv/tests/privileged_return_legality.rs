use rem6_isa_riscv::{
    RiscvHartState, RiscvInstruction, RiscvPrivilegeMode, RiscvStatusWord, RiscvTrap, RiscvTrapKind,
};

#[test]
fn hart_traps_user_machine_return_as_illegal_instruction() {
    let mut hart = RiscvHartState::new(0x7400);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_machine_trap_vector(0x9001);
    hart.set_machine_exception_pc(0xaaaa);
    hart.set_status(
        RiscvStatusWord::new(0)
            .with_mpp(RiscvPrivilegeMode::Supervisor)
            .with_mpie(true)
            .with_mie(true),
    );

    let record = hart
        .execute(RiscvInstruction::decode(0x3020_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7400);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7400);
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::User);
    assert!(hart.status().mpie());
    assert!(!hart.status().mie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7400))
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_delegates_supervisor_machine_return_illegal_instruction() {
    let mut hart = RiscvHartState::new(0x7500);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_supervisor_trap_vector(0x8101);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 2);
    hart.set_status(RiscvStatusWord::new(0).with_sie(true));

    let record = hart
        .execute(RiscvInstruction::decode(0x3020_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7500);
    assert_eq!(record.next_pc(), 0x8100);
    assert_eq!(hart.pc(), 0x8100);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7500);
    assert_eq!(hart.supervisor_trap_cause(), 2);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::Supervisor);
    assert!(hart.status().spie());
    assert!(!hart.status().sie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7500))
    );
}

#[test]
fn hart_traps_user_supervisor_return_as_illegal_instruction() {
    let mut hart = RiscvHartState::new(0x7600);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_supervisor_exception_pc(0xbbbb);
    hart.set_machine_trap_vector(0x9201);
    hart.set_status(
        RiscvStatusWord::new(0)
            .with_spp(RiscvPrivilegeMode::Supervisor)
            .with_spie(true)
            .with_sie(true),
    );

    let record = hart
        .execute(RiscvInstruction::decode(0x1020_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7600);
    assert_eq!(record.next_pc(), 0x9200);
    assert_eq!(hart.pc(), 0x9200);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7600);
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::User);
    assert_eq!(hart.supervisor_exception_pc(), 0xbbbb);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7600))
    );
}

#[test]
fn hart_keeps_supervisor_return_legal_in_machine_mode() {
    let mut hart = RiscvHartState::new(0x7700);
    hart.set_supervisor_exception_pc(0x9300);
    hart.set_status(
        RiscvStatusWord::new(0)
            .with_spp(RiscvPrivilegeMode::User)
            .with_spie(true)
            .with_sie(false)
            .with_mprv(true),
    );

    let record = hart
        .execute(RiscvInstruction::decode(0x1020_0073).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7700);
    assert_eq!(record.next_pc(), 0x9300);
    assert_eq!(hart.pc(), 0x9300);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::User);
    assert!(hart.status().sie());
    assert!(hart.status().spie());
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(!hart.status().mprv());
    assert_eq!(record.trap(), None);
}
