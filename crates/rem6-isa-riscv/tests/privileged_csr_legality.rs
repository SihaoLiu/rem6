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
fn hart_allows_supervisor_enabled_user_counter_csr_read() {
    let mut hart = RiscvHartState::new(0x7b00);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_machine_counter_enable(1);

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
fn hart_traps_supervisor_disabled_user_counter_csr_read() {
    let mut hart = RiscvHartState::new(0x7b20);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_machine_trap_vector(0x9000);

    let record = hart
        .execute(RiscvInstruction::decode(csr_read_type(0xc00, 5)).unwrap())
        .unwrap();

    assert_eq!(record.pc(), 0x7b20);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7b20);
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7b20))
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_allows_user_counter_csr_read_only_when_machine_and_supervisor_enable_bits_are_set() {
    let mut denied = RiscvHartState::new(0x7b40);
    denied.set_privilege_mode(RiscvPrivilegeMode::User);
    denied.set_machine_counter_enable(1);
    denied.set_machine_trap_vector(0x9000);

    let denied_record = denied
        .execute(RiscvInstruction::decode(csr_read_type(0xc00, 5)).unwrap())
        .unwrap();

    assert_eq!(denied_record.pc(), 0x7b40);
    assert_eq!(denied_record.next_pc(), 0x9000);
    assert_eq!(denied.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(
        denied_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7b40))
    );
    assert_eq!(denied.read(reg(5)), 0);

    let mut allowed = RiscvHartState::new(0x7b60);
    allowed.set_privilege_mode(RiscvPrivilegeMode::User);
    allowed.set_machine_counter_enable(1);
    allowed.set_supervisor_counter_enable(1);

    let allowed_record = allowed
        .execute(RiscvInstruction::decode(csr_read_type(0xc00, 5)).unwrap())
        .unwrap();

    assert_eq!(allowed_record.pc(), 0x7b60);
    assert_eq!(allowed_record.next_pc(), 0x7b64);
    assert_eq!(allowed.privilege_mode(), RiscvPrivilegeMode::User);
    assert_eq!(
        allowed_record.register_writes(),
        &[RegisterWrite::new(reg(5), 0)]
    );
    assert_eq!(allowed_record.trap(), None);
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

#[test]
fn hart_traps_lower_privilege_machine_identity_csr_reads() {
    let cases = [
        (RiscvPrivilegeMode::Supervisor, 0xf11, 0x7d00),
        (RiscvPrivilegeMode::User, 0xf12, 0x7d10),
        (RiscvPrivilegeMode::Supervisor, 0xf13, 0x7d20),
        (RiscvPrivilegeMode::User, 0xf14, 0x7d30),
    ];

    for (mode, csr, pc) in cases {
        let mut hart = RiscvHartState::new(pc);
        hart.set_privilege_mode(mode);
        hart.set_machine_trap_vector(0x9000);

        let record = hart
            .execute(RiscvInstruction::decode(csr_read_type(csr, 5)).unwrap())
            .unwrap();

        assert_eq!(record.pc(), pc);
        assert_eq!(record.next_pc(), 0x9000);
        assert_eq!(hart.pc(), 0x9000);
        assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
        assert_eq!(hart.machine_exception_pc(), pc);
        assert_eq!(hart.machine_trap_cause(), 2);
        assert_eq!(hart.machine_trap_value(), 0);
        assert_eq!(hart.status().mpp(), mode);
        assert_eq!(
            record.trap(),
            Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, pc))
        );
        assert_eq!(record.register_writes(), &[]);
        assert_eq!(hart.read(reg(5)), 0);
    }
}

#[test]
fn hart_traps_lower_privilege_machine_isa_csr_accesses() {
    let cases = [
        (
            RiscvPrivilegeMode::Supervisor,
            csr_read_type(0x301, 5),
            0x7d80,
        ),
        (RiscvPrivilegeMode::User, csr_type(0x301, 1, 0x1, 5), 0x7d90),
    ];

    for (mode, raw, pc) in cases {
        let mut hart = RiscvHartState::new(pc);
        hart.set_privilege_mode(mode);
        hart.set_machine_trap_vector(0x9000);
        hart.write(reg(1), 0xffff);

        let record = hart
            .execute(RiscvInstruction::decode(raw).unwrap())
            .unwrap();

        assert_eq!(record.pc(), pc);
        assert_eq!(record.next_pc(), 0x9000);
        assert_eq!(hart.pc(), 0x9000);
        assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
        assert_eq!(hart.machine_exception_pc(), pc);
        assert_eq!(hart.machine_trap_cause(), 2);
        assert_eq!(hart.machine_trap_value(), 0);
        assert_eq!(hart.status().mpp(), mode);
        assert_eq!(
            record.trap(),
            Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, pc))
        );
        assert_eq!(record.register_writes(), &[]);
        assert_eq!(hart.read(reg(5)), 0);
    }
}

#[test]
fn hart_traps_machine_identity_csr_write_attempts() {
    let identity_csrs = [0xf11, 0xf12, 0xf13, 0xf14];
    let write_forms = [(1, 0x1), (1, 0x2), (1, 0x3), (1, 0x5), (1, 0x6), (1, 0x7)];
    let mut pc = 0x7e00;

    for csr in identity_csrs {
        for (operand, funct3) in write_forms {
            let raw = csr_type(csr, operand, funct3, 5);
            let mut hart = RiscvHartState::new(pc);
            hart.set_machine_trap_vector(0x9000);
            hart.write(reg(1), 0xffff);

            let record = hart
                .execute(RiscvInstruction::decode(raw).unwrap())
                .unwrap();

            assert_eq!(
                record.trap(),
                Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, pc))
            );
            assert_eq!(record.pc(), pc);
            assert_eq!(record.next_pc(), 0x9000);
            assert_eq!(record.register_writes(), &[]);
            assert_eq!(hart.pc(), 0x9000);
            assert_eq!(hart.machine_exception_pc(), pc);
            assert_eq!(hart.machine_trap_cause(), 2);
            assert_eq!(hart.machine_trap_value(), 0);
            assert_eq!(hart.read(reg(5)), 0);

            pc += 4;
        }
    }
}
