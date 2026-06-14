use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, Immediate, MemoryAccessKind, MemoryWidth, Register,
    RiscvExecutionRecord, RiscvFloatRoundingMode, RiscvHartState, RiscvInstruction, RiscvTrap,
    RiscvTrapKind,
};

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn r4_type(rs3: u8, funct2: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (u32::from(rs3) << 27)
        | (funct2 << 25)
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

fn csr_type(csr: u16, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (u32::from(csr) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
}

fn csr_read_type(csr: u16, rd: u8) -> u32 {
    csr_type(csr, 0, 0x2, rd)
}

fn csr_write_type(csr: u16, rs1: u8, rd: u8) -> u32 {
    csr_type(csr, rs1, 0x1, rd)
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

const FFLAGS_CSR: u16 = 0x001;
const FLOAT_FLAG_INVALID: u64 = 1 << 4;
const FLOAT_FLAG_DIVIDE_BY_ZERO: u64 = 1 << 3;
const FLOAT_FLAG_INEXACT: u64 = 1 << 0;

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
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
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
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r_type(0x09, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMulD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r_type(0x0d, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatDivD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
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
fn decoder_accepts_rv64d_arithmetic_static_and_dynamic_rounding_modes() {
    assert_eq!(
        RiscvInstruction::decode(r_type(0x01, 3, 2, 0x7, 5, 0x53)).unwrap(),
        RiscvInstruction::FloatAddD {
            rd: freg(5),
            rs1: freg(2),
            rs2: freg(3),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x01, 3, 2, 0x1, 5, 0x53)).unwrap(),
        RiscvInstruction::FloatAddD {
            rd: freg(5),
            rs1: freg(2),
            rs2: freg(3),
            rounding_mode: RiscvFloatRoundingMode::RoundTowardZero,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x2d, 0, 2, 0x7, 5, 0x53)).unwrap(),
        RiscvInstruction::FloatSqrtD {
            rd: freg(5),
            rs1: freg(2),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x2d, 0, 2, 0x2, 5, 0x53)).unwrap(),
        RiscvInstruction::FloatSqrtD {
            rd: freg(5),
            rs1: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r4_type(4, 0x1, 3, 2, 0x7, 5, 0x43)).unwrap(),
        RiscvInstruction::FloatMultiplyAddD {
            rd: freg(5),
            rs1: freg(2),
            rs2: freg(3),
            rs3: freg(4),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r4_type(4, 0x1, 3, 2, 0x4, 5, 0x43)).unwrap(),
        RiscvInstruction::FloatMultiplyAddD {
            rd: freg(5),
            rs1: freg(2),
            rs2: freg(3),
            rs3: freg(4),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestMaxMagnitude,
        }
    );
    assert!(RiscvInstruction::decode(r_type(0x01, 3, 2, 0x5, 5, 0x53)).is_err());
    assert!(RiscvInstruction::decode(r4_type(4, 0x1, 3, 2, 0x6, 5, 0x43)).is_err());
}

#[test]
fn decoder_accepts_rv64d_minmax_class_and_move_operations() {
    let cases = [
        (
            r_type(0x15, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMinD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x15, 3, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatMaxD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
            },
        ),
        (
            r_type(0x71, 0, 2, 0x1, 5, 0x53),
            RiscvInstruction::FloatClassD {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x71, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMoveXFromD {
                rd: reg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x79, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatMoveDFromX {
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
fn decoder_accepts_rv64d_sqrt_and_integer_to_double_conversions() {
    let cases = [
        (
            r_type(0x2d, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatSqrtD {
                rd: freg(5),
                rs1: freg(2),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r_type(0x69, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertDFromW {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x69, 1, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertDFromWu {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x69, 2, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertDFromL {
                rd: freg(5),
                rs1: reg(2),
            },
        ),
        (
            r_type(0x69, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertDFromLu {
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
fn decoder_accepts_rv64d_double_to_integer_conversions() {
    let cases = [
        (
            r_type(0x61, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertWFromD {
                rd: reg(5),
                rs1: freg(2),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r_type(0x61, 1, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertWuFromD {
                rd: reg(5),
                rs1: freg(2),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r_type(0x61, 2, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertLFromD {
                rd: reg(5),
                rs1: freg(2),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r_type(0x61, 3, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertLuFromD {
                rd: reg(5),
                rs1: freg(2),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn decoder_accepts_rv64d_float_to_integer_rounding_modes() {
    assert_eq!(
        RiscvInstruction::decode(r_type(0x61, 0, 2, 0x7, 5, 0x53)).unwrap(),
        RiscvInstruction::FloatConvertWFromD {
            rd: reg(5),
            rs1: freg(2),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(r_type(0x61, 0, 2, 0x2, 5, 0x53)).unwrap(),
        RiscvInstruction::FloatConvertWFromD {
            rd: reg(5),
            rs1: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        }
    );
}

#[test]
fn decoder_rejects_rv64d_float_to_integer_reserved_rounding_modes() {
    assert!(RiscvInstruction::decode(r_type(0x61, 0, 2, 0x5, 5, 0x53)).is_err());
    assert!(RiscvInstruction::decode(r_type(0x61, 0, 2, 0x6, 5, 0x53)).is_err());
}

#[test]
fn decoder_accepts_rv64d_single_double_conversions() {
    let cases = [
        (
            r_type(0x20, 1, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertSFromD {
                rd: freg(5),
                rs1: freg(2),
            },
        ),
        (
            r_type(0x21, 0, 2, 0x0, 5, 0x53),
            RiscvInstruction::FloatConvertDFromS {
                rd: freg(5),
                rs1: freg(2),
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn decoder_accepts_rv64d_fused_multiply_add_operations() {
    let cases = [
        (
            r4_type(4, 1, 3, 2, 0x0, 5, 0x43),
            RiscvInstruction::FloatMultiplyAddD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
                rs3: freg(4),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r4_type(4, 1, 3, 2, 0x0, 5, 0x47),
            RiscvInstruction::FloatMultiplySubtractD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
                rs3: freg(4),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r4_type(4, 1, 3, 2, 0x0, 5, 0x4b),
            RiscvInstruction::FloatNegativeMultiplySubtractD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
                rs3: freg(4),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
        (
            r4_type(4, 1, 3, 2, 0x0, 5, 0x4f),
            RiscvInstruction::FloatNegativeMultiplyAddD {
                rd: freg(5),
                rs1: freg(2),
                rs2: freg(3),
                rs3: freg(4),
                rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
            },
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(RiscvInstruction::decode(raw).unwrap(), expected);
    }
}

#[test]
fn hart_accrues_rv64d_divide_exception_flags() {
    let mut hart = RiscvHartState::new(0x2100);
    hart.write_float(freg(1), 1.0f64.to_bits());
    hart.write_float(freg(2), 0.0f64.to_bits());

    hart.execute(RiscvInstruction::FloatDivD {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();

    assert_eq!(hart.read_float(freg(3)), f64::INFINITY.to_bits());
    hart.execute(RiscvInstruction::decode(csr_read_type(FFLAGS_CSR, 5)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), FLOAT_FLAG_DIVIDE_BY_ZERO);

    hart.write(reg(10), 0);
    hart.execute(RiscvInstruction::decode(csr_write_type(FFLAGS_CSR, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 0.0f64.to_bits());
    hart.write_float(freg(2), 0.0f64.to_bits());
    hart.execute(RiscvInstruction::FloatDivD {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();

    assert!(f64::from_bits(hart.read_float(freg(3))).is_nan());
    hart.execute(RiscvInstruction::decode(csr_read_type(FFLAGS_CSR, 6)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(6)), FLOAT_FLAG_INVALID);
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
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
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
fn hart_executes_rv64d_fused_multiply_add_operations() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 1.5f64.to_bits());
    hart.write_float(freg(2), 2.0f64.to_bits());
    hart.write_float(freg(3), (-0.25f64).to_bits());

    let fused = hart
        .execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(4)), 2.75f64.to_bits());
    assert_eq!(
        fused.float_register_writes(),
        &[FloatRegisterWrite::new(freg(4), 2.75f64.to_bits())]
    );

    hart.execute(RiscvInstruction::FloatMultiplySubtractD {
        rd: freg(5),
        rs1: freg(1),
        rs2: freg(2),
        rs3: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(5)), 3.25f64.to_bits());

    hart.execute(RiscvInstruction::FloatNegativeMultiplySubtractD {
        rd: freg(6),
        rs1: freg(1),
        rs2: freg(2),
        rs3: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(6)), (-3.25f64).to_bits());

    hart.execute(RiscvInstruction::FloatNegativeMultiplyAddD {
        rd: freg(7),
        rs1: freg(1),
        rs2: freg(2),
        rs3: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(7)), (-2.75f64).to_bits());
}

#[test]
fn hart_rv64d_fused_multiply_add_raises_invalid_for_signaling_nan_only() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 0x7ff0_0000_0000_0001);
    hart.write_float(freg(2), 2.0f64.to_bits());
    hart.write_float(freg(3), 3.0f64.to_bits());

    hart.execute(RiscvInstruction::FloatMultiplyAddD {
        rd: freg(4),
        rs1: freg(1),
        rs2: freg(2),
        rs3: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();

    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.write(reg(10), 0);
    hart.execute(RiscvInstruction::decode(csr_write_type(FFLAGS_CSR, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 0x7ff8_0000_0000_0001);
    hart.execute(RiscvInstruction::FloatMultiplyAddD {
        rd: freg(4),
        rs1: freg(1),
        rs2: freg(2),
        rs3: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.float_status().fflags(), 0);
}

#[test]
fn hart_rv64d_fused_multiply_add_raises_invalid_for_infinity_times_zero() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), f64::INFINITY.to_bits());
    hart.write_float(freg(2), 0.0f64.to_bits());
    hart.write_float(freg(3), 0x7ff8_0000_0000_0001);

    hart.execute(RiscvInstruction::FloatMultiplyAddD {
        rd: freg(4),
        rs1: freg(1),
        rs2: freg(2),
        rs3: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();

    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);
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
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
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
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
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
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
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
fn hart_rv64d_comparisons_raise_invalid_for_nan_by_opcode() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 0x7ff8_0000_0000_0001);
    hart.write_float(freg(2), 1.0f64.to_bits());

    let less_than = hart
        .execute(RiscvInstruction::FloatLessThanD {
            rd: reg(5),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(less_than.register_writes()[0].value(), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.write(reg(10), 0);
    hart.execute(RiscvInstruction::decode(csr_write_type(FFLAGS_CSR, 10, 0)).unwrap())
        .unwrap();
    let less_or_equal = hart
        .execute(RiscvInstruction::FloatLessOrEqualD {
            rd: reg(6),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(less_or_equal.register_writes()[0].value(), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.write(reg(10), 0);
    hart.execute(RiscvInstruction::decode(csr_write_type(FFLAGS_CSR, 10, 0)).unwrap())
        .unwrap();
    let equal_quiet = hart
        .execute(RiscvInstruction::FloatEqualD {
            rd: reg(7),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(equal_quiet.register_writes()[0].value(), 0);
    assert_eq!(hart.float_status().fflags(), 0);

    hart.write_float(freg(1), 0x7ff0_0000_0000_0001);
    let equal_signaling = hart
        .execute(RiscvInstruction::FloatEqualD {
            rd: reg(8),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(equal_signaling.register_writes()[0].value(), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);
}

#[test]
fn hart_executes_rv64d_minmax_with_nan_and_signed_zero_rules() {
    let mut hart = RiscvHartState::new(0x8000);
    let neg_zero = (-0.0f64).to_bits();
    let pos_zero = 0.0f64.to_bits();
    hart.write_float(freg(1), neg_zero);
    hart.write_float(freg(2), pos_zero);

    let min_zero = hart
        .execute(RiscvInstruction::FloatMinD {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(3)), neg_zero);
    assert_eq!(
        min_zero.float_register_writes(),
        &[FloatRegisterWrite::new(freg(3), neg_zero)]
    );

    let max_zero = hart
        .execute(RiscvInstruction::FloatMaxD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(4)), pos_zero);
    assert_eq!(
        max_zero.float_register_writes(),
        &[FloatRegisterWrite::new(freg(4), pos_zero)]
    );

    let quiet_nan = f64::NAN.to_bits();
    hart.write_float(freg(5), quiet_nan);
    hart.write_float(freg(6), 12.5f64.to_bits());
    hart.execute(RiscvInstruction::FloatMinD {
        rd: freg(7),
        rs1: freg(5),
        rs2: freg(6),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(7)), 12.5f64.to_bits());

    hart.execute(RiscvInstruction::FloatMaxD {
        rd: freg(8),
        rs1: freg(5),
        rs2: freg(5),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(8)), 0x7ff8_0000_0000_0000);
}

#[test]
fn hart_rv64d_minmax_raise_invalid_for_signaling_nan_only() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 0x7ff0_0000_0000_0001);
    hart.write_float(freg(2), 4.0f64.to_bits());

    hart.execute(RiscvInstruction::FloatMinD {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
    })
    .unwrap();

    assert_eq!(hart.read_float(freg(3)), 4.0f64.to_bits());
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(4), 0x7ff8_0000_0000_0001);
    hart.execute(RiscvInstruction::FloatMaxD {
        rd: freg(5),
        rs1: freg(4),
        rs2: freg(2),
    })
    .unwrap();

    assert_eq!(hart.read_float(freg(5)), 4.0f64.to_bits());
    assert_eq!(hart.float_status().fflags(), 0);
}

#[test]
fn hart_executes_rv64d_classification_masks() {
    let mut hart = RiscvHartState::new(0x8000);
    let cases = [
        (f64::NEG_INFINITY.to_bits(), 1 << 0),
        ((-1.0f64).to_bits(), 1 << 1),
        (0x8000_0000_0000_0001, 1 << 2),
        ((-0.0f64).to_bits(), 1 << 3),
        (0.0f64.to_bits(), 1 << 4),
        (0x0000_0000_0000_0001, 1 << 5),
        (1.0f64.to_bits(), 1 << 6),
        (f64::INFINITY.to_bits(), 1 << 7),
        (0x7ff0_0000_0000_0001, 1 << 8),
        (f64::NAN.to_bits(), 1 << 9),
    ];

    for (bits, expected) in cases {
        hart.write_float(freg(1), bits);
        let record = hart
            .execute(RiscvInstruction::FloatClassD {
                rd: reg(5),
                rs1: freg(1),
            })
            .unwrap();
        assert_eq!(hart.read(reg(5)), expected);
        assert_eq!(record.register_writes()[0].value(), expected);
        assert_eq!(record.float_register_writes(), &[]);
    }
}

#[test]
fn hart_executes_rv64d_raw_integer_float_moves() {
    let mut hart = RiscvHartState::new(0x8000);
    let bits = 0x7ff8_0123_4567_89ab;
    hart.write_float(freg(1), bits);

    let from_float = hart
        .execute(RiscvInstruction::FloatMoveXFromD {
            rd: reg(5),
            rs1: freg(1),
        })
        .unwrap();
    assert_eq!(hart.read(reg(5)), bits);
    assert_eq!(from_float.register_writes()[0].value(), bits);
    assert_eq!(from_float.float_register_writes(), &[]);

    let moved_bits = 0xfff0_0000_0000_0001;
    hart.write(reg(6), moved_bits);
    let to_float = hart
        .execute(RiscvInstruction::FloatMoveDFromX {
            rd: freg(7),
            rs1: reg(6),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(7)), moved_bits);
    assert_eq!(
        to_float.float_register_writes(),
        &[FloatRegisterWrite::new(freg(7), moved_bits)]
    );
    assert_eq!(to_float.register_writes(), &[]);
}

#[test]
fn hart_executes_rv64d_sqrt_and_integer_to_double_conversions() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 144.0f64.to_bits());

    let sqrt = hart
        .execute(RiscvInstruction::FloatSqrtD {
            rd: freg(2),
            rs1: freg(1),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(2)), 12.0f64.to_bits());
    assert_eq!(
        sqrt.float_register_writes(),
        &[FloatRegisterWrite::new(freg(2), 12.0f64.to_bits())]
    );

    hart.write_float(freg(10), (-1.0f64).to_bits());
    let invalid_sqrt = hart
        .execute(RiscvInstruction::FloatSqrtD {
            rd: freg(11),
            rs1: freg(10),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(11)), 0x7ff8_0000_0000_0000);
    assert_eq!(
        invalid_sqrt.float_register_writes(),
        &[FloatRegisterWrite::new(freg(11), 0x7ff8_0000_0000_0000)]
    );

    hart.write_float(freg(12), 0x7ff0_0000_0000_0001);
    hart.execute(RiscvInstruction::FloatSqrtD {
        rd: freg(13),
        rs1: freg(12),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(13)), 0x7ff8_0000_0000_0000);

    hart.write(reg(3), 0xffff_fffe);
    hart.execute(RiscvInstruction::FloatConvertDFromW {
        rd: freg(4),
        rs1: reg(3),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), (-2.0f64).to_bits());

    hart.execute(RiscvInstruction::FloatConvertDFromWu {
        rd: freg(5),
        rs1: reg(3),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(5)), 4_294_967_294.0f64.to_bits());

    hart.write(reg(6), (-9i64) as u64);
    hart.execute(RiscvInstruction::FloatConvertDFromL {
        rd: freg(7),
        rs1: reg(6),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(7)), (-9.0f64).to_bits());

    hart.write(reg(8), 1u64 << 63);
    let unsigned = hart
        .execute(RiscvInstruction::FloatConvertDFromLu {
            rd: freg(9),
            rs1: reg(8),
        })
        .unwrap();
    assert_eq!(
        hart.read_float(freg(9)),
        9_223_372_036_854_775_808.0f64.to_bits()
    );
    assert_eq!(unsigned.register_writes(), &[]);

    hart.write(reg(10), u64::MAX);
    hart.execute(RiscvInstruction::FloatConvertDFromLu {
        rd: freg(11),
        rs1: reg(10),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(11)), 0x43f0_0000_0000_0000);
}

#[test]
fn hart_rv64d_sqrt_raises_invalid_for_negative_inputs_and_signaling_nan() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), (-4.0f64).to_bits());

    hart.execute(RiscvInstruction::FloatSqrtD {
        rd: freg(2),
        rs1: freg(1),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(2)), 0x7ff8_0000_0000_0000);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(3), (-0.0f64).to_bits());
    hart.execute(RiscvInstruction::FloatSqrtD {
        rd: freg(4),
        rs1: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), (-0.0f64).to_bits());
    assert_eq!(hart.float_status().fflags(), 0);

    hart.write_float(freg(5), 0x7ff0_0000_0000_0001);
    hart.execute(RiscvInstruction::FloatSqrtD {
        rd: freg(6),
        rs1: freg(5),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(6)), 0x7ff8_0000_0000_0000);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(7), 0x7ff8_0000_0000_0001);
    hart.execute(RiscvInstruction::FloatSqrtD {
        rd: freg(8),
        rs1: freg(7),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.float_status().fflags(), 0);
}

#[test]
fn hart_executes_rv64d_single_double_conversions_with_nan_boxing() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write_float(freg(1), 1.5f64.to_bits());
    let to_single = hart
        .execute(RiscvInstruction::FloatConvertSFromD {
            rd: freg(2),
            rs1: freg(1),
        })
        .unwrap();
    assert_eq!(hart.read_float(freg(2)), f32_box(1.5));
    assert_eq!(
        to_single.float_register_writes(),
        &[FloatRegisterWrite::new(freg(2), f32_box(1.5))]
    );

    let lower = f32::from_bits(0x3f80_0000);
    let upper = f32::from_bits(0x3f80_0001);
    hart.write_float(
        freg(3),
        ((f64::from(lower) + f64::from(upper)) / 2.0).to_bits(),
    );
    hart.execute(RiscvInstruction::FloatConvertSFromD {
        rd: freg(4),
        rs1: freg(3),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), f32_box(lower));

    hart.write_float(freg(5), f32_box(-2.25));
    hart.execute(RiscvInstruction::FloatConvertDFromS {
        rd: freg(6),
        rs1: freg(5),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(6)), (-2.25f64).to_bits());

    hart.write_float(freg(7), 1.0f32.to_bits().into());
    hart.execute(RiscvInstruction::FloatConvertDFromS {
        rd: freg(8),
        rs1: freg(7),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(8)), 0x7ff8_0000_0000_0000);

    hart.write_float(freg(9), f64::NAN.to_bits());
    hart.execute(RiscvInstruction::FloatConvertSFromD {
        rd: freg(10),
        rs1: freg(9),
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(10)), box_single(0x7fc0_0000));
}

#[test]
fn hart_executes_rv64d_double_to_integer_conversions_with_rne() {
    let mut hart = RiscvHartState::new(0x8000);

    hart.write_float(freg(1), 2.5f64.to_bits());
    let even_down = hart
        .execute(RiscvInstruction::FloatConvertWFromD {
            rd: reg(2),
            rs1: freg(1),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
        })
        .unwrap();
    assert_eq!(hart.read(reg(2)), 2);
    assert_eq!(even_down.register_writes()[0].value(), 2);
    assert_eq!(even_down.float_register_writes(), &[]);

    hart.write_float(freg(3), 3.5f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(4),
        rs1: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(4)), 4);

    hart.write_float(freg(5), (-2.5f64).to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(6),
        rs1: freg(5),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), (-2i64) as u64);

    hart.write_float(freg(7), 4_294_967_295.0f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertWuFromD {
        rd: reg(8),
        rs1: freg(7),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(8)), u64::MAX);

    hart.write_float(freg(9), (-9.5f64).to_bits());
    hart.execute(RiscvInstruction::FloatConvertLFromD {
        rd: reg(10),
        rs1: freg(9),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(10)), (-10i64) as u64);

    hart.write_float(freg(11), 9.5f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertLuFromD {
        rd: reg(12),
        rs1: freg(11),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(12)), 10);
}

#[test]
fn hart_executes_rv64d_double_to_integer_dynamic_and_static_rounding() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.write(reg(10), 1 << 5);
    hart.execute(RiscvInstruction::decode(csr_write_type(0x003, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 2.9f64.to_bits());

    hart.execute(RiscvInstruction::decode(r_type(0x61, 0, 1, 0x7, 5, 0x53)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 2);

    hart.write_float(freg(2), (-2.1f64).to_bits());
    hart.execute(RiscvInstruction::decode(r_type(0x61, 0, 2, 0x2, 6, 0x53)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(6)), (-3i64) as u64);

    hart.write_float(freg(3), 2.5f64.to_bits());
    hart.execute(RiscvInstruction::decode(r_type(0x61, 0, 3, 0x4, 7, 0x53)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(7)), 3);
}

#[test]
fn hart_rv64d_double_to_integer_valid_rounding_raises_inexact() {
    let mut hart = RiscvHartState::new(0x8000);

    hart.write_float(freg(1), 2.5f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(2),
        rs1: freg(1),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(2)), 2);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(3), (-0.4f64).to_bits());
    hart.execute(RiscvInstruction::FloatConvertWuFromD {
        rd: reg(4),
        rs1: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(4)), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(5), (-9.5f64).to_bits());
    hart.execute(RiscvInstruction::FloatConvertLFromD {
        rd: reg(6),
        rs1: freg(5),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), (-10i64) as u64);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(7), 9.5f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertLuFromD {
        rd: reg(8),
        rs1: freg(7),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(8)), 10);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(9), 2.0f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(10),
        rs1: freg(9),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(10)), 2);
    assert_eq!(hart.float_status().fflags(), 0);
}

#[test]
fn hart_traps_rv64d_dynamic_rounding_with_reserved_frm() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_machine_trap_vector(0x9000);
    hart.write(reg(10), (7 << 5) | FLOAT_FLAG_DIVIDE_BY_ZERO);
    hart.execute(RiscvInstruction::decode(csr_write_type(0x003, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), f64::NAN.to_bits());

    let record = hart
        .execute(RiscvInstruction::decode(r_type(0x61, 0, 1, 0x7, 5, 0x53)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8104))
    );
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_DIVIDE_BY_ZERO);
}

#[test]
fn hart_ignores_frm_for_rv64d_static_rounding() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.write(reg(10), 7 << 5);
    hart.execute(RiscvInstruction::decode(csr_write_type(0x003, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 2.1f64.to_bits());

    let record = hart
        .execute(RiscvInstruction::decode(r_type(0x61, 0, 1, 0x3, 5, 0x53)).unwrap())
        .unwrap();

    assert_eq!(record.trap(), None);
    assert_eq!(hart.read(reg(5)), 3);
}

#[test]
fn hart_executes_rv64d_arithmetic_dynamic_rounding_with_valid_frm() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.write(reg(10), 0);
    hart.execute(RiscvInstruction::decode(csr_write_type(0x003, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 1.25f64.to_bits());
    hart.write_float(freg(2), 2.5f64.to_bits());
    hart.write_float(freg(3), 3.0f64.to_bits());
    hart.write_float(freg(4), (-1.0f64).to_bits());

    let record = hart
        .execute(RiscvInstruction::decode(r_type(0x01, 2, 1, 0x7, 5, 0x53)).unwrap())
        .unwrap();

    assert_eq!(record.trap(), None);
    assert_eq!(hart.read_float(freg(5)), 3.75f64.to_bits());

    let fused = hart
        .execute(RiscvInstruction::decode(r4_type(4, 0x1, 2, 1, 0x7, 6, 0x43)).unwrap())
        .unwrap();
    assert_eq!(fused.trap(), None);
    assert_eq!(hart.read_float(freg(6)), 2.125f64.to_bits());

    let sqrt = hart
        .execute(RiscvInstruction::decode(r_type(0x2d, 0, 3, 0x7, 7, 0x53)).unwrap())
        .unwrap();
    assert_eq!(sqrt.trap(), None);
    assert_eq!(hart.read_float(freg(7)), 3.0f64.sqrt().to_bits());
}

#[test]
fn hart_executes_rv64d_static_rounding_when_result_is_rounding_insensitive() {
    let mut hart = RiscvHartState::new(0x8200);
    hart.write(reg(10), 7 << 5);
    hart.execute(RiscvInstruction::decode(csr_write_type(0x003, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 3.75f64.to_bits());
    hart.write_float(freg(2), 0.0f64.to_bits());
    hart.write_float(freg(3), 9.0f64.to_bits());
    hart.write_float(freg(4), 2.5f64.to_bits());
    hart.write_float(freg(5), 1.0f64.to_bits());
    hart.write_float(freg(6), 0.0f64.to_bits());

    let add = hart
        .execute(RiscvInstruction::decode(r_type(0x01, 2, 1, 0x1, 8, 0x53)).unwrap())
        .unwrap();
    assert_eq!(add.trap(), None);
    assert_eq!(hart.read_float(freg(8)), 3.75f64.to_bits());

    let sqrt = hart
        .execute(RiscvInstruction::decode(r_type(0x2d, 0, 3, 0x2, 9, 0x53)).unwrap())
        .unwrap();
    assert_eq!(sqrt.trap(), None);
    assert_eq!(hart.read_float(freg(9)), 3.0f64.to_bits());

    let fused = hart
        .execute(RiscvInstruction::decode(r4_type(6, 0x1, 5, 4, 0x4, 10, 0x43)).unwrap())
        .unwrap();
    assert_eq!(fused.trap(), None);
    assert_eq!(hart.read_float(freg(10)), 2.5f64.to_bits());
    assert_eq!(hart.float_status().fflags(), 0);
}

#[test]
fn hart_traps_rv64d_static_rounding_when_result_may_depend_on_rounding() {
    let mut sqrt_hart = RiscvHartState::new(0x8300);
    sqrt_hart.write_float(freg(1), 0x4690_0000_0000_0002);
    let sqrt = sqrt_hart
        .execute(RiscvInstruction::decode(r_type(0x2d, 0, 1, 0x2, 2, 0x53)).unwrap())
        .unwrap();
    assert!(sqrt.trap().is_some());
    assert_eq!(sqrt_hart.read_float(freg(2)), 0);

    let mut fused_hart = RiscvHartState::new(0x8400);
    fused_hart.write_float(freg(1), 1.0f64.to_bits());
    fused_hart.write_float(freg(2), 0.0f64.to_bits());
    fused_hart.write_float(freg(3), (-0.0f64).to_bits());
    let fused = fused_hart
        .execute(RiscvInstruction::decode(r4_type(3, 0x1, 2, 1, 0x2, 4, 0x43)).unwrap())
        .unwrap();
    assert!(fused.trap().is_some());
    assert_eq!(fused_hart.read_float(freg(4)), 0);

    let mut invalid_hart = RiscvHartState::new(0x8500);
    invalid_hart.write_float(freg(1), 0x7ff0_0000_0000_0001);
    invalid_hart.write_float(freg(2), 1.0f64.to_bits());
    let add = invalid_hart
        .execute(RiscvInstruction::decode(r_type(0x01, 2, 1, 0x1, 3, 0x53)).unwrap())
        .unwrap();
    assert!(add.trap().is_some());
    assert_eq!(invalid_hart.read_float(freg(3)), 0);
    assert_eq!(invalid_hart.float_status().fflags(), 0);
}

#[test]
fn hart_traps_rv64d_arithmetic_dynamic_rounding_with_reserved_frm() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_machine_trap_vector(0x9000);
    hart.write(reg(10), (7 << 5) | FLOAT_FLAG_DIVIDE_BY_ZERO);
    hart.execute(RiscvInstruction::decode(csr_write_type(0x003, 10, 0)).unwrap())
        .unwrap();
    hart.write_float(freg(1), 1.25f64.to_bits());
    hart.write_float(freg(2), 2.5f64.to_bits());
    hart.write_float(freg(3), 3.0f64.to_bits());

    let record = hart
        .execute(RiscvInstruction::decode(r_type(0x01, 2, 1, 0x7, 5, 0x53)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8104))
    );
    assert_eq!(hart.machine_trap_cause(), 2);
    assert_eq!(hart.read_float(freg(5)), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_DIVIDE_BY_ZERO);

    hart.set_pc(0x8100);
    let fused = hart
        .execute(RiscvInstruction::decode(r4_type(3, 0x1, 2, 1, 0x7, 6, 0x43)).unwrap())
        .unwrap();
    assert_eq!(
        fused.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8100))
    );
    assert_eq!(hart.read_float(freg(6)), 0);

    hart.set_pc(0x8100);
    let sqrt = hart
        .execute(RiscvInstruction::decode(r_type(0x2d, 0, 3, 0x7, 7, 0x53)).unwrap())
        .unwrap();
    assert_eq!(
        sqrt.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8100))
    );
    assert_eq!(hart.read_float(freg(7)), 0);
}

#[test]
fn hart_rv64d_double_to_integer_invalid_results_raise_invalid() {
    let mut hart = RiscvHartState::new(0x8000);

    hart.write_float(freg(1), f64::NAN.to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(2),
        rs1: freg(1),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(2)), i32::MAX as u64);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(3), (-1.0e40f64).to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(4),
        rs1: freg(3),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(4)), i32::MIN as i64 as u64);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(5), (-1.0f64).to_bits());
    hart.execute(RiscvInstruction::FloatConvertWuFromD {
        rd: reg(6),
        rs1: freg(5),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(6)), 0);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(7), f64::INFINITY.to_bits());
    hart.execute(RiscvInstruction::FloatConvertLuFromD {
        rd: reg(8),
        rs1: freg(7),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(8)), u64::MAX);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(9), f64::NEG_INFINITY.to_bits());
    hart.execute(RiscvInstruction::FloatConvertLFromD {
        rd: reg(10),
        rs1: freg(9),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(10)), i64::MIN as u64);
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(11), 2.0f64.to_bits());
    hart.execute(RiscvInstruction::FloatConvertWFromD {
        rd: reg(12),
        rs1: freg(11),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read(reg(12)), 2);
    assert_eq!(hart.float_status().fflags(), 0);
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
