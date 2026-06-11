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

fn box_single(bits: u32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(bits)
}

fn f32_box(value: f32) -> u64 {
    box_single(value.to_bits())
}

#[test]
fn decoder_accepts_rv64f_load_store_and_arithmetic_operations() {
    let cases = [
        (
            i_type(24, 2, 0x2, 5, 0x07),
            RiscvInstruction::FloatLoad {
                rd: freg(5),
                rs1: reg(2),
                offset: Immediate::new(24),
                width: MemoryWidth::Word,
            },
        ),
        (
            s_type(-16, 6, 3, 0x2, 0x27),
            RiscvInstruction::FloatStore {
                rs1: reg(3),
                rs2: freg(6),
                offset: Immediate::new(-16),
                width: MemoryWidth::Word,
            },
        ),
        (
            r_type(0x00, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatAddS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x04, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatSubS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x08, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMulS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x0c, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatDivS {
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
fn decoder_accepts_rv64f_sign_compare_minmax_class_and_moves() {
    let cases = [
        (
            r_type(0x2c, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatSqrtS {
                rd: freg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x10, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatSignInjectS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x10, 3, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatSignInjectNegS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x10, 3, 2, 0x2, 5, 0x53),
            RiscvInstruction::FloatSignInjectXorS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x14, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMinS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x14, 3, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatMaxS {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x50, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatLessOrEqualS {
                rd: reg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x50, 3, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatLessThanS {
                rd: reg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x50, 3, 2, 0x2, 5, 0x53),
            RiscvInstruction::FloatEqualS {
                rd: reg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x70, 0, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatClassS {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x70, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMoveXFromS {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x78, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMoveSFromX {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn decoder_accepts_rv64f_single_integer_conversions() {
    let cases = [
        (
            r_type(0x68, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertSFromW {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x68, 1, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertSFromWu {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x68, 2, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertSFromL {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x68, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertSFromLu {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x60, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertWFromS {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x60, 1, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertWuFromS {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x60, 2, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertLFromS {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x60, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertLuFromS {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn hart_executes_rv64f_rne_arithmetic_and_records_nan_boxed_writes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(0), f32_box(9.0));
    hart.write_float(freg(2), f32_box(2.0));

    let sub = hart
        .execute(RiscvInstruction::FloatSubS {
            rd: freg(1),
            rs1: freg(0),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(1)), f32_box(7.0));
    assert_eq!(
        sub.float_register_writes(),
        &[FloatRegisterWrite::new(freg(1), f32_box(7.0))]
    );

    hart.execute(RiscvInstruction::FloatMulS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), f32_box(14.0));

    hart.execute(RiscvInstruction::FloatDivS {
        rd: freg(4),
        rs1: freg(3),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), f32_box(7.0));

    let add = hart
        .execute(RiscvInstruction::FloatAddS {
            rd: freg(5),
            rs1: freg(0),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(5)), f32_box(11.0));
    assert_eq!(add.register_writes(), &[]);
}

#[test]
fn hart_executes_rv64f_sqrt_and_treats_unboxed_inputs_as_nan() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), f32_box(144.0));

    let sqrt = hart
        .execute(RiscvInstruction::FloatSqrtS {
            rd: freg(2),
            rs1: freg(1),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(2)), f32_box(12.0));
    assert_eq!(
        sqrt.float_register_writes(),
        &[FloatRegisterWrite::new(freg(2), f32_box(12.0))]
    );

    hart.write_float(freg(3), f32_box(-1.0));
    hart.execute(RiscvInstruction::FloatSqrtS {
        rd: freg(4),
        rs1: freg(3),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), box_single(0x7fc0_0000));

    hart.write_float(freg(5), 1.0f32.to_bits().into());
    hart.execute(RiscvInstruction::FloatAddS {
        rd: freg(6),
        rs1: freg(5),
        rs2: freg(1),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(6)), box_single(0x7fc0_0000));
}

#[test]
fn hart_rv64f_sign_injection_treats_unboxed_source_as_nan() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 1.0f32.to_bits().into());
    hart.write_float(freg(2), f32_box(-2.0));

    hart.execute(RiscvInstruction::FloatSignInjectS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), box_single(0xffc0_0000));

    hart.execute(RiscvInstruction::FloatSignInjectNegS {
        rd: freg(4),
        rs1: freg(1),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), box_single(0x7fc0_0000));

    hart.execute(RiscvInstruction::FloatSignInjectXorS {
        rd: freg(5),
        rs1: freg(1),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(5)), box_single(0xffc0_0000));
}

#[test]
fn hart_executes_rv64f_sign_injection_minmax_and_comparisons() {
    let mut hart = RiscvHartState::new(0x8000);
    let positive = f32_box(1.25);
    let negative = f32_box(-2.5);
    hart.write_float(freg(1), positive);
    hart.write_float(freg(2), negative);

    hart.execute(RiscvInstruction::FloatSignInjectS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(
        hart.read_float(freg(3)),
        box_single(1.25f32.to_bits() | (1 << 31))
    );

    hart.execute(RiscvInstruction::FloatSignInjectNegS {
        rd: freg(4),
        rs1: freg(3),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), positive);

    hart.execute(RiscvInstruction::FloatSignInjectXorS {
        rd: freg(5),
        rs1: freg(3),
        rs2: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(5)), positive);

    hart.write_float(freg(6), f32_box(-0.0));
    hart.write_float(freg(7), f32_box(0.0));
    hart.execute(RiscvInstruction::FloatMinS {
        rd: freg(8),
        rs1: freg(6),
        rs2: freg(7),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(8)), f32_box(-0.0));
    hart.execute(RiscvInstruction::FloatMaxS {
        rd: freg(9),
        rs1: freg(6),
        rs2: freg(7),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(9)), f32_box(0.0));

    let less = hart
        .execute(RiscvInstruction::FloatLessThanS {
            rd: reg(5),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(less.register_writes()[0].value(), 0);
    hart.execute(RiscvInstruction::FloatLessOrEqualS {
        rd: reg(6),
        rs1: freg(2),
        rs2: freg(1),
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), 1);
    hart.execute(RiscvInstruction::FloatEqualS {
        rd: reg(7),
        rs1: freg(1),
        rs2: freg(1),
    })
    .unwrap();
    assert_eq!(hart.read(reg(7)), 1);
}

#[test]
fn hart_executes_rv64f_classification_and_raw_moves() {
    let mut hart = RiscvHartState::new(0x8000);
    let cases = [
        (f32::NEG_INFINITY.to_bits(), 1 << 0),
        ((-1.0f32).to_bits(), 1 << 1),
        (0x8000_0001, 1 << 2),
        ((-0.0f32).to_bits(), 1 << 3),
        (0.0f32.to_bits(), 1 << 4),
        (0x0000_0001, 1 << 5),
        (1.0f32.to_bits(), 1 << 6),
        (f32::INFINITY.to_bits(), 1 << 7),
        (0x7f80_0001, 1 << 8),
        (f32::NAN.to_bits(), 1 << 9),
    ];

    for (bits, expected) in cases {
        hart.write_float(freg(1), box_single(bits));
        let record = hart
            .execute(RiscvInstruction::FloatClassS {
                rd: reg(5),
                rs1: freg(1),
            })
            .unwrap();
        assert_eq!(record.register_writes()[0].value(), expected);
    }

    hart.write_float(freg(2), box_single(0x8000_0001));
    hart.execute(RiscvInstruction::FloatMoveXFromS {
        rd: reg(6),
        rs1: freg(2),
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), 0xffff_ffff_8000_0001);

    hart.write(reg(7), 0x1234_5678_7fc0_1234);
    let to_float = hart
        .execute(RiscvInstruction::FloatMoveSFromX {
            rd: freg(8),
            rs1: reg(7),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(8)), box_single(0x7fc0_1234));
    assert_eq!(
        to_float.float_register_writes(),
        &[FloatRegisterWrite::new(freg(8), box_single(0x7fc0_1234))]
    );
}

#[test]
fn hart_executes_rv64f_integer_to_single_conversions() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write(reg(1), 0xffff_fffe);

    hart.execute(RiscvInstruction::FloatConvertSFromW {
        rd: freg(2),
        rs1: reg(1),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(2)), f32_box(-2.0));

    hart.execute(RiscvInstruction::FloatConvertSFromWu {
        rd: freg(3),
        rs1: reg(1),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), f32_box(4_294_967_294.0));

    hart.write(reg(4), (-9i64) as u64);
    hart.execute(RiscvInstruction::FloatConvertSFromL {
        rd: freg(5),
        rs1: reg(4),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(5)), f32_box(-9.0));

    hart.write(reg(6), 1_u64 << 63);
    let unsigned = hart
        .execute(RiscvInstruction::FloatConvertSFromLu {
            rd: freg(7),
            rs1: reg(6),
        })
        .unwrap();
    assert_eq!(
        hart.read_float(freg(7)),
        f32_box(9_223_372_036_854_775_808.0)
    );
    assert_eq!(
        unsigned.float_register_writes(),
        &[FloatRegisterWrite::new(
            freg(7),
            f32_box(9_223_372_036_854_775_808.0)
        )]
    );
    assert_eq!(unsigned.register_writes(), &[]);
}

#[test]
fn hart_executes_rv64f_single_to_integer_conversions_with_rne() {
    let mut hart = RiscvHartState::new(0x8000);

    hart.write_float(freg(1), f32_box(2.5));
    let even_down = hart
        .execute(RiscvInstruction::FloatConvertWFromS {
            rd: reg(2),
            rs1: freg(1),
        })
        .unwrap();
    assert_eq!(hart.read(reg(2)), 2);
    assert_eq!(even_down.register_writes()[0].value(), 2);
    assert_eq!(even_down.float_register_writes(), &[]);

    hart.write_float(freg(3), f32_box(3.5));
    hart.execute(RiscvInstruction::FloatConvertWFromS {
        rd: reg(4),
        rs1: freg(3),
    })
    .unwrap();
    assert_eq!(hart.read(reg(4)), 4);

    hart.write_float(freg(5), f32_box(-2.5));
    hart.execute(RiscvInstruction::FloatConvertWFromS {
        rd: reg(6),
        rs1: freg(5),
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), (-2i64) as u64);

    hart.write_float(freg(7), f32_box(4_294_967_295.0));
    hart.execute(RiscvInstruction::FloatConvertWuFromS {
        rd: reg(8),
        rs1: freg(7),
    })
    .unwrap();
    assert_eq!(hart.read(reg(8)), u64::MAX);

    hart.write_float(freg(9), f32_box(-9.5));
    hart.execute(RiscvInstruction::FloatConvertLFromS {
        rd: reg(10),
        rs1: freg(9),
    })
    .unwrap();
    assert_eq!(hart.read(reg(10)), (-10i64) as u64);

    hart.write_float(freg(11), f32_box(9.5));
    hart.execute(RiscvInstruction::FloatConvertLuFromS {
        rd: reg(12),
        rs1: freg(11),
    })
    .unwrap();
    assert_eq!(hart.read(reg(12)), 10);
}

#[test]
fn hart_saturates_rv64f_single_to_integer_invalid_results_without_fflags() {
    let mut hart = RiscvHartState::new(0x8000);

    hart.write_float(freg(1), box_single(0x7fc0_0000));
    hart.execute(RiscvInstruction::FloatConvertWFromS {
        rd: reg(2),
        rs1: freg(1),
    })
    .unwrap();
    assert_eq!(hart.read(reg(2)), i32::MAX as u64);

    hart.write_float(freg(3), f32_box(f32::NEG_INFINITY));
    hart.execute(RiscvInstruction::FloatConvertWFromS {
        rd: reg(4),
        rs1: freg(3),
    })
    .unwrap();
    assert_eq!(hart.read(reg(4)), i32::MIN as i64 as u64);

    hart.write_float(freg(5), f32_box(-1.0));
    hart.execute(RiscvInstruction::FloatConvertWuFromS {
        rd: reg(6),
        rs1: freg(5),
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), 0);

    hart.write_float(freg(7), f32_box(f32::INFINITY));
    hart.execute(RiscvInstruction::FloatConvertLuFromS {
        rd: reg(8),
        rs1: freg(7),
    })
    .unwrap();
    assert_eq!(hart.read(reg(8)), u64::MAX);

    hart.write_float(freg(9), f32_box(f32::NEG_INFINITY));
    hart.execute(RiscvInstruction::FloatConvertLFromS {
        rd: reg(10),
        rs1: freg(9),
    })
    .unwrap();
    assert_eq!(hart.read(reg(10)), i64::MIN as u64);

    hart.write_float(freg(11), 1.0f32.to_bits().into());
    hart.execute(RiscvInstruction::FloatConvertWFromS {
        rd: reg(12),
        rs1: freg(11),
    })
    .unwrap();
    assert_eq!(hart.read(reg(12)), i32::MAX as u64);
}

#[test]
fn hart_reports_rv64f_load_store_memory_accesses() {
    let mut hart = RiscvHartState::new(0x9000);
    hart.write(reg(2), 0x8000);
    hart.write_float(freg(0), f32_box(1.25));

    let load = hart
        .execute(RiscvInstruction::FloatLoad {
            rd: freg(0),
            rs1: reg(2),
            offset: Immediate::new(32),
            width: MemoryWidth::Word,
        })
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::FloatLoad {
            rd: freg(0),
            address: 0x8020,
            width: MemoryWidth::Word,
        })
    );

    let store = hart
        .execute(RiscvInstruction::FloatStore {
            rs1: reg(2),
            rs2: freg(0),
            offset: Immediate::new(-8),
            width: MemoryWidth::Word,
        })
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::FloatStore {
            address: 0x7ff8,
            width: MemoryWidth::Word,
            value: f32_box(1.25),
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
