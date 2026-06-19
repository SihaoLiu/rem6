use rem6_isa_riscv::{
    Immediate, Register, RegisterWrite, RiscvHartState, RiscvInstruction, RiscvPrivilegeMode,
    RiscvStatusWord,
};

const INTERRUPT_BIT: u64 = 1_u64 << 63;
const SSIP: u64 = 1 << 1;
const MSIP: u64 = 1 << 3;
const STIP: u64 = 1 << 5;
const MTIP: u64 = 1 << 7;
const SEIP: u64 = 1 << 9;
const MEIP: u64 = 1 << 11;

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

fn addi(rd: u8, rs1: u8, imm: i16) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(i64::from(imm)),
    }
}

fn write_csr(hart: &mut RiscvHartState, csr: u16, value: u64) {
    hart.write(reg(2), value);
    hart.execute(RiscvInstruction::decode(csr_type(csr, 2, 0x1, 0)).unwrap())
        .unwrap();
}

fn read_csr(hart: &mut RiscvHartState, csr: u16, rd: u8) -> u64 {
    hart.execute(RiscvInstruction::decode(csr_read_type(csr, rd)).unwrap())
        .unwrap();
    hart.read(reg(rd))
}

#[test]
fn hart_reads_and_writes_interrupt_enable_and_pending_csrs() {
    let mut hart = RiscvHartState::new(0x4000);

    write_csr(&mut hart, 0x304, SSIP);
    write_csr(&mut hart, 0x344, SSIP);

    assert_eq!(read_csr(&mut hart, 0x304, 5), SSIP);
    assert_eq!(read_csr(&mut hart, 0x344, 6), SSIP);

    write_csr(&mut hart, 0x303, SSIP);
    write_csr(&mut hart, 0x104, SSIP);
    write_csr(&mut hart, 0x144, SSIP);

    assert_eq!(read_csr(&mut hart, 0x104, 7), SSIP);
    assert_eq!(read_csr(&mut hart, 0x144, 8), SSIP);
    assert_eq!(read_csr(&mut hart, 0x304, 9) & SSIP, SSIP);
    assert_eq!(read_csr(&mut hart, 0x344, 10) & SSIP, SSIP);
}

#[test]
fn hart_takes_enabled_supervisor_software_interrupt_in_machine_mode_when_not_delegated() {
    let mut hart = RiscvHartState::new(0x5000);
    hart.set_machine_trap_vector(0x9000);
    write_csr(&mut hart, 0x304, SSIP);
    write_csr(&mut hart, 0x344, SSIP);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_pc(0x7000);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7000);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7000);
    assert_eq!(hart.machine_trap_cause(), INTERRUPT_BIT | 1);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().mpp(), RiscvPrivilegeMode::User);
    assert!(!hart.status().mie());
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(record.trap().unwrap().pc(), 0x7000);
}

#[test]
fn hart_vectors_machine_interrupts_when_mtvec_is_vectored() {
    let mut hart = RiscvHartState::new(0x5050);
    hart.set_machine_trap_vector(0x9001);
    write_csr(&mut hart, 0x304, STIP);
    write_csr(&mut hart, 0x344, STIP);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_pc(0x7050);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7050);
    assert_eq!(record.next_pc(), 0x9014);
    assert_eq!(hart.pc(), 0x9014);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7050);
    assert_eq!(hart.machine_trap_cause(), INTERRUPT_BIT | 5);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_prioritizes_external_machine_interrupt_over_lower_pending_machine_interrupts() {
    let mut hart = RiscvHartState::new(0x5080);
    hart.set_machine_trap_vector(0x9001);
    let pending = MEIP | MSIP | MTIP;
    hart.set_machine_interrupt_enable(pending);
    hart.set_machine_interrupt_pending(pending);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_pc(0x7080);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7080);
    assert_eq!(record.next_pc(), 0x902c);
    assert_eq!(hart.pc(), 0x902c);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x7080);
    assert_eq!(hart.machine_trap_cause(), INTERRUPT_BIT | 11);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_delegates_user_supervisor_software_interrupt_to_supervisor_vector() {
    let mut hart = RiscvHartState::new(0x5100);
    hart.set_supervisor_trap_vector(0x8100);
    hart.set_machine_trap_vector(0x9000);
    write_csr(&mut hart, 0x303, SSIP);
    write_csr(&mut hart, 0x104, SSIP);
    write_csr(&mut hart, 0x144, SSIP);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_pc(0x7100);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7100);
    assert_eq!(record.next_pc(), 0x8100);
    assert_eq!(hart.pc(), 0x8100);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7100);
    assert_eq!(hart.supervisor_trap_cause(), INTERRUPT_BIT | 1);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(!hart.status().sie());
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(record.trap().unwrap().pc(), 0x7100);
}

#[test]
fn hart_prioritizes_external_delegated_supervisor_interrupt_over_lower_pending_interrupts() {
    let mut hart = RiscvHartState::new(0x5180);
    hart.set_supervisor_trap_vector(0x8101);
    hart.set_machine_trap_vector(0x9000);
    let pending = SEIP | SSIP | STIP;
    hart.set_machine_interrupt_delegation(pending);
    hart.set_machine_interrupt_enable(pending);
    hart.set_machine_interrupt_pending(pending);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_pc(0x7180);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7180);
    assert_eq!(record.next_pc(), 0x8124);
    assert_eq!(hart.pc(), 0x8124);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7180);
    assert_eq!(hart.supervisor_trap_cause(), INTERRUPT_BIT | 9);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_vectors_delegated_supervisor_interrupts_when_stvec_is_vectored() {
    let mut hart = RiscvHartState::new(0x5150);
    hart.set_supervisor_trap_vector(0x8101);
    hart.set_machine_trap_vector(0x9000);
    write_csr(&mut hart, 0x303, STIP);
    write_csr(&mut hart, 0x104, STIP);
    write_csr(&mut hart, 0x144, STIP);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_pc(0x7150);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7150);
    assert_eq!(record.next_pc(), 0x8114);
    assert_eq!(hart.pc(), 0x8114);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x7150);
    assert_eq!(hart.supervisor_trap_cause(), INTERRUPT_BIT | 5);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_masks_delegated_supervisor_interrupt_while_in_machine_mode() {
    let mut hart = RiscvHartState::new(0x5200);
    hart.set_machine_trap_vector(0x9000);
    hart.set_status(RiscvStatusWord::new(0).with_mie(true));
    write_csr(&mut hart, 0x303, SSIP);
    write_csr(&mut hart, 0x304, SSIP);
    write_csr(&mut hart, 0x344, SSIP);
    hart.set_pc(0x7200);

    let record = hart.execute(addi(5, 0, 1)).unwrap();

    assert_eq!(record.pc(), 0x7200);
    assert_eq!(record.next_pc(), 0x7204);
    assert_eq!(hart.pc(), 0x7204);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(record.register_writes(), &[RegisterWrite::new(reg(5), 1)]);
    assert_eq!(hart.read(reg(5)), 1);
    assert_eq!(record.trap(), None);
}
