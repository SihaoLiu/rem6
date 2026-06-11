use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, Immediate, MemoryAccessKind, MemoryWidth, Register,
    RiscvExecutionRecord, RiscvHartState, RiscvInstruction,
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

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

#[test]
fn decoder_accepts_rv64d_load_store_and_add() {
    let cases = [
        (
            i_type(24, 2, 0x3, 5, 0x07),
            RiscvInstruction::FloatLoad {
                rd: freg(5),
                rs1: reg(2),
                offset: Immediate::new(24),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            s_type(-16, 6, 3, 0x3, 0x27),
            RiscvInstruction::FloatStore {
                rs1: reg(3),
                rs2: freg(6),
                offset: Immediate::new(-16),
                width: MemoryWidth::Doubleword,
            },
        ),
        (
            r_type(0x01, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatAddD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn decoder_accepts_rv64d_arithmetic_sign_and_compare_operations() {
    let cases = [
        (
            r_type(0x05, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatSubD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x09, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMulD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x0d, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatDivD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x11, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatSignInjectD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x11, 3, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatSignInjectNegD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x11, 3, 2, 0x2, 5, 0x53),
            RiscvInstruction::FloatSignInjectXorD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x51, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatLessOrEqualD {
                rd: reg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x51, 3, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatLessThanD {
                rd: reg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x51, 3, 2, 0x2, 5, 0x53),
            RiscvInstruction::FloatEqualD {
                rd: reg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn hart_executes_faddd_and_records_float_write() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(0), 1.5f64.to_bits());
    hart.write_float(freg(2), 2.25f64.to_bits());

    let record = hart
        .execute(RiscvInstruction::FloatAddD {
            rd: freg(0),
            rs1: freg(0),
            rs2: freg(2),
        })
        .unwrap();

    let expected = 3.75f64.to_bits();
    assert_eq!(hart.read_float(freg(0)), expected);
    assert_eq!(record.pc(), 0x8000);
    assert_eq!(record.next_pc(), 0x8004);
    assert_eq!(
        record.float_register_writes(),
        &[FloatRegisterWrite::new(freg(0), expected)]
    );
    assert_eq!(record.register_writes(), &[]);
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_executes_rv64d_rne_arithmetic_and_records_float_writes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(0), 9.0f64.to_bits());
    hart.write_float(freg(2), 2.0f64.to_bits());

    let sub = hart
        .execute(RiscvInstruction::FloatSubD {
            rd: freg(1),
            rs1: freg(0),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(1)), 7.0f64.to_bits());
    assert_eq!(
        sub.float_register_writes(),
        &[FloatRegisterWrite::new(freg(1), 7.0f64.to_bits())]
    );

    let mul = hart
        .execute(RiscvInstruction::FloatMulD {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(3)), 14.0f64.to_bits());
    assert_eq!(
        mul.float_register_writes(),
        &[FloatRegisterWrite::new(freg(3), 14.0f64.to_bits())]
    );

    let div = hart
        .execute(RiscvInstruction::FloatDivD {
            rd: freg(4),
            rs1: freg(3),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(4)), 7.0f64.to_bits());
    assert_eq!(
        div.float_register_writes(),
        &[FloatRegisterWrite::new(freg(4), 7.0f64.to_bits())]
    );
}

#[test]
fn hart_executes_rv64d_sign_injection_with_raw_bits() {
    let mut hart = RiscvHartState::new(0x8000);
    let positive = 1.25f64.to_bits();
    let negative = (-2.5f64).to_bits();
    hart.write_float(freg(1), positive);
    hart.write_float(freg(2), negative);

    let sign = hart
        .execute(RiscvInstruction::FloatSignInjectD {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(3)), positive | (1 << 63));
    assert_eq!(
        sign.float_register_writes(),
        &[FloatRegisterWrite::new(freg(3), positive | (1 << 63))]
    );

    hart.execute(RiscvInstruction::FloatSignInjectNegD {
        rd: freg(4),
        rs1: freg(3),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), positive);

    hart.execute(RiscvInstruction::FloatSignInjectXorD {
        rd: freg(5),
        rs1: freg(3),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(5)), positive);
}

#[test]
fn hart_executes_rv64d_comparisons_and_records_integer_writes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 2.0f64.to_bits());
    hart.write_float(freg(2), 3.0f64.to_bits());
    hart.write_float(freg(3), 2.0f64.to_bits());

    let less_or_equal = hart
        .execute(RiscvInstruction::FloatLessOrEqualD {
            rd: reg(5),
            rs1: freg(1),
            rs2: freg(3),
        })
        .unwrap();
    assert_eq!(hart.read(reg(5)), 1);
    assert_eq!(less_or_equal.register_writes()[0].value(), 1);
    assert_eq!(less_or_equal.float_register_writes(), &[]);

    let less_than = hart
        .execute(RiscvInstruction::FloatLessThanD {
            rd: reg(6),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read(reg(6)), 1);
    assert_eq!(less_than.register_writes()[0].value(), 1);

    let equal = hart
        .execute(RiscvInstruction::FloatEqualD {
            rd: reg(7),
            rs1: freg(1),
            rs2: freg(3),
        })
        .unwrap();
    assert_eq!(hart.read(reg(7)), 1);
    assert_eq!(equal.register_writes()[0].value(), 1);
}

#[test]
fn hart_rv64d_comparisons_return_false_for_nan_without_fflags() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), f64::NAN.to_bits());
    hart.write_float(freg(2), 1.0f64.to_bits());

    for instruction in [
        RiscvInstruction::FloatLessOrEqualD {
            rd: reg(5),
            rs1: freg(1),
            rs2: freg(2),
        },
        RiscvInstruction::FloatLessThanD {
            rd: reg(6),
            rs1: freg(1),
            rs2: freg(2),
        },
        RiscvInstruction::FloatEqualD {
            rd: reg(7),
            rs1: freg(1),
            rs2: freg(2),
        },
    ] {
        let record = hart.execute(instruction).unwrap();
        assert_eq!(record.register_writes()[0].value(), 0);
    }
}

#[test]
fn hart_reports_float_load_store_memory_accesses() {
    let mut hart = RiscvHartState::new(0x9000);
    hart.write(reg(2), 0x8000);
    hart.write_float(freg(0), 1.25f64.to_bits());

    let load = hart
        .execute(RiscvInstruction::FloatLoad {
            rd: freg(0),
            rs1: reg(2),
            offset: Immediate::new(32),
            width: MemoryWidth::Doubleword,
        })
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::FloatLoad {
            rd: freg(0),
            address: 0x8020,
            width: MemoryWidth::Doubleword,
        })
    );

    let store = hart
        .execute(RiscvInstruction::FloatStore {
            rs1: reg(2),
            rs2: freg(0),
            offset: Immediate::new(-8),
            width: MemoryWidth::Doubleword,
        })
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::FloatStore {
            address: 0x7ff8,
            width: MemoryWidth::Doubleword,
            value: 1.25f64.to_bits(),
        })
    );
}

#[test]
fn execution_records_default_to_no_float_writes() {
    let record = RiscvExecutionRecord::new(
        RiscvInstruction::Addi {
            rd: reg(1),
            rs1: reg(0),
            imm: Immediate::new(1),
        },
        0x8000,
        0x8004,
        Vec::new(),
        None,
    );

    assert_eq!(record.float_register_writes(), &[]);
}
