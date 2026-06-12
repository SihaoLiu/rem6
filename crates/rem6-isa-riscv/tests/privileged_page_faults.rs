use rem6_isa_riscv::{
    Register, RiscvHartState, RiscvInstruction, RiscvPrivilegeMode, RiscvStatusWord, RiscvTrap,
    RiscvTrapKind,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn load_instruction() -> RiscvInstruction {
    RiscvInstruction::decode(i_type(0, 10, 0x3, 11, 0x03)).unwrap()
}

#[test]
fn hart_enters_machine_instruction_page_fault_with_faulting_address() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    hart.set_machine_trap_vector(0x9000);

    let record = hart.enter_synchronous_trap(
        load_instruction(),
        4,
        0x8000,
        RiscvTrapKind::InstructionPageFault {
            address: 0x0000_1234_5678_9abc,
        },
    );

    assert_eq!(record.pc(), 0x8000);
    assert_eq!(record.next_pc(), 0x9000);
    assert_eq!(hart.pc(), 0x9000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x8000);
    assert_eq!(hart.machine_trap_cause(), 12);
    assert_eq!(hart.machine_trap_value(), 0x0000_1234_5678_9abc);
    assert_eq!(hart.supervisor_trap_cause(), 0);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::InstructionPageFault {
                address: 0x0000_1234_5678_9abc,
            },
            0x8000,
        ))
    );
}

#[test]
fn hart_delegates_user_load_page_fault_with_faulting_address() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_supervisor_trap_vector(0xa001);
    hart.set_machine_trap_vector(0x9000);
    hart.set_machine_exception_delegation(1 << 13);
    hart.set_status(RiscvStatusWord::new(0).with_sie(true));

    let record = hart.enter_synchronous_trap(
        load_instruction(),
        4,
        0x8100,
        RiscvTrapKind::LoadPageFault {
            address: 0xffff_ffff_ffff_f000,
        },
    );

    assert_eq!(record.pc(), 0x8100);
    assert_eq!(record.next_pc(), 0xa000);
    assert_eq!(hart.pc(), 0xa000);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(hart.supervisor_exception_pc(), 0x8100);
    assert_eq!(hart.supervisor_trap_cause(), 13);
    assert_eq!(hart.supervisor_trap_value(), 0xffff_ffff_ffff_f000);
    assert_eq!(hart.machine_trap_cause(), 0);
    assert_eq!(hart.machine_trap_value(), 0);
    assert_eq!(hart.status().spp(), RiscvPrivilegeMode::User);
    assert!(hart.status().spie());
    assert!(!hart.status().sie());
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::LoadPageFault {
                address: 0xffff_ffff_ffff_f000,
            },
            0x8100,
        ))
    );
}

#[test]
fn hart_enters_machine_store_page_fault_with_faulting_address() {
    let mut hart = RiscvHartState::new(0x8200);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_machine_trap_vector(0x9100);
    hart.set_machine_exception_delegation(1 << 13);

    let record = hart.enter_synchronous_trap(
        load_instruction(),
        4,
        0x8200,
        RiscvTrapKind::StorePageFault {
            address: 0x4000_0000_0000_1000,
        },
    );

    assert_eq!(record.pc(), 0x8200);
    assert_eq!(record.next_pc(), 0x9100);
    assert_eq!(hart.pc(), 0x9100);
    assert_eq!(hart.privilege_mode(), RiscvPrivilegeMode::Machine);
    assert_eq!(hart.machine_exception_pc(), 0x8200);
    assert_eq!(hart.machine_trap_cause(), 15);
    assert_eq!(hart.machine_trap_value(), 0x4000_0000_0000_1000);
    assert_eq!(hart.supervisor_trap_cause(), 0);
    assert_eq!(hart.supervisor_trap_value(), 0);
    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::StorePageFault {
                address: 0x4000_0000_0000_1000,
            },
            0x8200,
        ))
    );
}

#[test]
fn page_fault_trap_entry_preserves_compressed_instruction_length() {
    let mut hart = RiscvHartState::new(0x8300);
    hart.set_privilege_mode(RiscvPrivilegeMode::User);
    hart.set_machine_trap_vector(0x9200);

    let record = hart.enter_synchronous_trap(
        RiscvInstruction::Addi {
            rd: reg(1),
            rs1: reg(0),
            imm: rem6_isa_riscv::Immediate::new(0),
        },
        2,
        0x8300,
        RiscvTrapKind::LoadPageFault { address: 0x8302 },
    );

    assert_eq!(record.instruction_bytes(), 2);
    assert_eq!(record.next_pc(), 0x9200);
    assert_eq!(hart.machine_trap_cause(), 13);
    assert_eq!(hart.machine_trap_value(), 0x8302);
}
