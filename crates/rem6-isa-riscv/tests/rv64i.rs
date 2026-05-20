use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvError, RiscvExecutionRecord,
    RiscvHartState, RiscvInstruction,
};

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
}

fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
}

fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

#[test]
fn decoder_extracts_rv64i_fields_and_immediates() {
    assert_eq!(
        RiscvInstruction::decode(i_type(-1, 0, 0x0, 5, 0x13)).unwrap(),
        RiscvInstruction::Addi {
            rd: reg(5),
            rs1: reg(0),
            imm: Immediate::new(-1),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(u_type(0x1234_5000, 10, 0x37)).unwrap(),
        RiscvInstruction::Lui {
            rd: reg(10),
            imm: Immediate::new(0x1234_5000),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x20, 1, 2, 0x0, 3, 0x33)).unwrap(),
        RiscvInstruction::Sub {
            rd: reg(3),
            rs1: reg(2),
            rs2: reg(1),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(b_type(-8, 6, 5, 0x1)).unwrap(),
        RiscvInstruction::Bne {
            rs1: reg(5),
            rs2: reg(6),
            offset: Immediate::new(-8),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(j_type(2048, 1)).unwrap(),
        RiscvInstruction::Jal {
            rd: reg(1),
            offset: Immediate::new(2048),
        }
    );
}

#[test]
fn hart_executes_integer_register_operations_and_keeps_zero_readonly() {
    let mut hart = RiscvHartState::new(0x8000);

    let first = hart
        .execute(RiscvInstruction::decode(i_type(5, 0, 0x0, 1, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.pc(), 0x8004);
    assert_eq!(hart.read(reg(1)), 5);
    assert_eq!(
        first,
        RiscvExecutionRecord::new(
            RiscvInstruction::Addi {
                rd: reg(1),
                rs1: reg(0),
                imm: Immediate::new(5),
            },
            0x8000,
            0x8004,
            vec![rem6_isa_riscv::RegisterWrite::new(reg(1), 5)],
            None,
        )
    );

    let ignored = hart
        .execute(RiscvInstruction::decode(i_type(7, 1, 0x0, 0, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(0)), 0);
    assert_eq!(ignored.register_writes(), &[]);

    hart.execute(RiscvInstruction::decode(r_type(0, 1, 1, 0x0, 2, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(2)), 10);

    hart.execute(RiscvInstruction::decode(r_type(0x20, 1, 2, 0x0, 3, 0x33)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(3)), 5);

    hart.execute(RiscvInstruction::decode(i_type(-1, 0, 0x0, 4, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(4)), u64::MAX);
    hart.execute(RiscvInstruction::decode(i_type(1, 4, 0x0, 5, 0x13)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_executes_upper_immediate_jumps_and_branches() {
    let mut hart = RiscvHartState::new(0x1000);

    hart.execute(RiscvInstruction::decode(u_type(0x1234_5000, 4, 0x37)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(4)), 0x1234_5000);

    hart.execute(RiscvInstruction::decode(u_type(0x0001_0000, 5, 0x17)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 0x0001_1004);

    hart.write(reg(6), 9);
    hart.write(reg(7), 9);
    let taken = hart
        .execute(RiscvInstruction::decode(b_type(16, 7, 6, 0x0)).unwrap())
        .unwrap();
    assert_eq!(taken.next_pc(), 0x1018);

    let jump = hart
        .execute(RiscvInstruction::decode(j_type(-8, 1)).unwrap())
        .unwrap();
    assert_eq!(jump.next_pc(), 0x1010);
    assert_eq!(hart.read(reg(1)), 0x101c);

    hart.write(reg(8), 0x2003);
    let jalr = hart
        .execute(RiscvInstruction::decode(i_type(4, 8, 0x0, 1, 0x67)).unwrap())
        .unwrap();
    assert_eq!(jalr.next_pc(), 0x2006);
    assert_eq!(hart.read(reg(1)), 0x1014);
}

#[test]
fn hart_reports_memory_accesses_without_mutating_memory() {
    let mut hart = RiscvHartState::new(0x4000);
    hart.write(reg(2), 0x8000);
    hart.write(reg(3), 0x1122_3344_5566_7788);

    let load = hart
        .execute(RiscvInstruction::decode(i_type(24, 2, 0x3, 9, 0x03)).unwrap())
        .unwrap();
    assert_eq!(load.next_pc(), 0x4004);
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::Load {
            rd: reg(9),
            address: 0x8018,
            width: MemoryWidth::Doubleword,
            signed: true,
        })
    );
    assert_eq!(hart.read(reg(9)), 0);

    let store = hart
        .execute(RiscvInstruction::decode(s_type(-16, 3, 2, 0x2)).unwrap())
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::Store {
            address: 0x7ff0,
            width: MemoryWidth::Word,
            value: 0x1122_3344_5566_7788,
        })
    );
}

#[test]
fn decoder_rejects_compressed_and_unknown_encodings() {
    assert_eq!(
        RiscvInstruction::decode(0x0000_0001).unwrap_err(),
        RiscvError::CompressedNotSupported { raw: 0x0000_0001 }
    );
    assert_eq!(
        RiscvInstruction::decode(0xffff_ffff).unwrap_err(),
        RiscvError::UnknownEncoding { raw: 0xffff_ffff }
    );
    assert_eq!(
        Register::new(32).unwrap_err(),
        RiscvError::InvalidRegister { index: 32 }
    );
}
