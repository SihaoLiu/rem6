use rem6_isa_riscv::{
    Register, RegisterWrite, RiscvHartState, RiscvInstruction, RiscvPrivilegeMode, RiscvStatusWord,
    RiscvTrap, RiscvTrapKind,
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
fn hart_traps_supervisor_machine_status_csr_read() {
    let mut hart = RiscvHartState::new(0x7800);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_machine_trap_vector(0x9001);
    hart.set_status(RiscvStatusWord::new(0).with_mie(true));

    let record = hart
        .execute(RiscvInstruction::decode(csr_read_type(0x300, 5)).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7800);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7800);
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::Supervisor);
    assert!(!hart.status().mie());
    assert!(hart.status().mpie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7800))
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_delegates_user_supervisor_trap_csr_write_illegal_instruction() {
    let mut hart = RiscvHartState::new(0x7900);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_supervisor_trap_vector(0x8101);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 2);
    hart.set_status(RiscvStatusWord::new(0).with_sie(true));
    hart.write(reg(2), 0x9201);

    let record = hart
        .execute(RiscvInstruction::decode(csr_type(0x105, 2, 0x1, 5)).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7900);
    assert_eq!(record.next_pc(), 0x8100);
    assert_eq!(hart.pc(), 0x8100);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7900);
    assert_eq!(hart.supervisor_trap_cause(), 2);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(hart.supervisor_trap_vector(), 0x8101);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(hart.status().spie());
    assert!(!hart.status().sie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7900))
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_allows_machine_access_to_supervisor_trap_csr() {
    let mut hart = RiscvHartState::new(0x7a00);
    hart.set_supervisor_trap_vector(0x8101);
    hart.write(reg(2), 0x8201);

    let record = hart
        .execute(RiscvInstruction::decode(csr_type(0x105, 2, 0x1, 5)).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7a00);
    assert_eq!(record.next_pc(), 0x7a04);
    assert_eq!(hart.pc(), 0x7a04);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.supervisor_trap_vector(), 0x8201);
    assert_eq!(
        record.register_writes(),
        &[RegisterWrite::new(reg(5), 0x8101)]
    );
    assert_eq!(record.trap(), None);
}

#[test]
fn hart_allows_supervisor_user_counter_csr_read() {
    let mut hart = RiscvHartState::new(0x7b00);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);

    let record = hart
        .execute(RiscvInstruction::decode(csr_read_type(0xc00, 5)).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7b00);
    assert_eq!(record.next_pc(), 0x7b04);
    assert_eq!(hart.pc(), 0x7b04);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(5), 0)]);
    assert_eq!(record.trap(), None);
}

#[test]
fn hart_traps_supervisor_machine_counter_csr_read() {
    let mut hart = RiscvHartState::new(0x7c00);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_machine_trap_vector(0x9000);

    let record = hart
        .execute(RiscvInstruction::decode(csr_read_type(0xb00, 5)).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7c00);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7c00);
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7c00))
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}
