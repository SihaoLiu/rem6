use rem6_isa_riscv::{Register, RiscvHartState, RiscvInstruction};

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

#[test]
fn decoder_accepts_rv64m_register_operations() {
    let cases = [
        (
            r_type(0x01, 3, 2, 0x0, 5, 0x33),
            RiscvInstruction::Mul {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x1, 5, 0x33),
            RiscvInstruction::Mulh {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x2, 5, 0x33),
            RiscvInstruction::Mulhsu {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x3, 5, 0x33),
            RiscvInstruction::Mulhu {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x4, 5, 0x33),
            RiscvInstruction::Div {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x5, 5, 0x33),
            RiscvInstruction::Divu {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x6, 5, 0x33),
            RiscvInstruction::Rem {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x7, 5, 0x33),
            RiscvInstruction::Remu {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn hart_executes_rv64m_multiplication_high_and_low_results() {
    let mut hart = RiscvHartState::new(0x1000);
    hart.write(reg(2), 0xffff_ffff_ffff_fffe);
    hart.write(reg(3), 3);

    let cases = [
        (0x0, 0xffff_ffff_ffff_fffa),
        (0x1, 0xffff_ffff_ffff_ffff),
        (0x2, 0xffff_ffff_ffff_ffff),
        (0x3, 0x0000_0000_0000_0002),
    ];

    for (funct3, expected) in cases {
        let instruction = RiscvInstruction::decode(r_type(0x01, 3, 2, funct3, 5, 0x33)).unwrap();
        let record = hart.execute(instruction).unwrap();

        assert_eq!(hart.read(reg(5)), expected);
        assert_eq!(record.memory_access(), None);
        assert_eq!(record.register_writes()[0].value(), expected);
    }
}

#[test]
fn hart_executes_mulhsu_with_high_unsigned_operand_bit() {
    let mut hart = RiscvHartState::new(0x1800);
    hart.write(reg(2), u64::MAX);
    hart.write(reg(3), u64::MAX);

    let instruction = RiscvInstruction::decode(r_type(0x01, 3, 2, 0x2, 5, 0x33)).unwrap();
    let record = hart.execute(instruction).unwrap();

    assert_eq!(hart.read(reg(5)), u64::MAX);
    assert_eq!(record.memory_access(), None);
    assert_eq!(record.register_writes()[0].value(), u64::MAX);
}

#[test]
fn hart_executes_rv64m_division_and_remainder_edge_cases() {
    let mut hart = RiscvHartState::new(0x2000);

    let cases = [
        (0x4, (-9i64) as u64, 2, (-4i64) as u64),
        (0x6, (-9i64) as u64, 2, (-1i64) as u64),
        (0x5, 9, 2, 4),
        (0x7, 9, 2, 1),
        (0x4, 0x1234, 0, u64::MAX),
        (0x5, 0x1234, 0, u64::MAX),
        (0x6, 0x1234, 0, 0x1234),
        (0x7, 0x1234, 0, 0x1234),
        (0x4, i64::MIN as u64, (-1i64) as u64, i64::MIN as u64),
        (0x6, i64::MIN as u64, (-1i64) as u64, 0),
    ];

    for (funct3, lhs, rhs, expected) in cases {
        hart.write(reg(2), lhs);
        hart.write(reg(3), rhs);
        let instruction = RiscvInstruction::decode(r_type(0x01, 3, 2, funct3, 5, 0x33)).unwrap();
        let record = hart.execute(instruction).unwrap();

        assert_eq!(hart.read(reg(5)), expected);
        assert_eq!(record.memory_access(), None);
        assert_eq!(record.register_writes()[0].value(), expected);
    }
}

#[test]
fn decoder_accepts_rv64m_word_register_operations() {
    let cases = [
        (
            r_type(0x01, 3, 2, 0x0, 5, 0x3b),
            RiscvInstruction::Mulw {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x4, 5, 0x3b),
            RiscvInstruction::Divw {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x5, 5, 0x3b),
            RiscvInstruction::Divuw {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x6, 5, 0x3b),
            RiscvInstruction::Remw {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
        (
            r_type(0x01, 3, 2, 0x7, 5, 0x3b),
            RiscvInstruction::Remuw {
                rd: reg(5),
                rs1: reg(2),
                rs2: reg(3),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn hart_executes_rv64m_word_multiply_with_sign_extended_low_word() {
    let mut hart = RiscvHartState::new(0x3000);
    hart.write(reg(2), 0x0000_0000_ffff_fffe);
    hart.write(reg(3), 3);

    let instruction = RiscvInstruction::decode(r_type(0x01, 3, 2, 0x0, 5, 0x3b)).unwrap();
    let record = hart.execute(instruction).unwrap();

    assert_eq!(hart.read(reg(5)), 0xffff_ffff_ffff_fffa);
    assert_eq!(record.memory_access(), None);
    assert_eq!(record.register_writes()[0].value(), 0xffff_ffff_ffff_fffa);
}

#[test]
fn hart_executes_rv64m_word_division_and_remainder_edge_cases() {
    let mut hart = RiscvHartState::new(0x4000);

    let cases = [
        (0x4, (-9i64) as u64, 2, (-4i64) as u64),
        (0x6, (-9i64) as u64, 2, (-1i64) as u64),
        (0x5, 0x0000_0000_ffff_fffe, 2, 0x0000_0000_7fff_ffff),
        (0x7, 0x0000_0000_ffff_fffe, 3, 2),
        (0x4, 0x1234_5678, 0, u64::MAX),
        (0x5, 0x1234_5678, 0, u64::MAX),
        (0x6, 0x0000_0000_8000_0001, 0, 0xffff_ffff_8000_0001),
        (0x7, 0x0000_0000_8000_0001, 0, 0xffff_ffff_8000_0001),
        (
            0x4,
            i32::MIN as u32 as u64,
            (-1i32) as u32 as u64,
            0xffff_ffff_8000_0000,
        ),
        (0x6, i32::MIN as u32 as u64, (-1i32) as u32 as u64, 0),
    ];

    for (funct3, lhs, rhs, expected) in cases {
        hart.write(reg(2), lhs);
        hart.write(reg(3), rhs);
        let instruction = RiscvInstruction::decode(r_type(0x01, 3, 2, funct3, 5, 0x3b)).unwrap();
        let record = hart.execute(instruction).unwrap();

        assert_eq!(hart.read(reg(5)), expected);
        assert_eq!(record.memory_access(), None);
        assert_eq!(record.register_writes()[0].value(), expected);
    }
}
