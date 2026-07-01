use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatRoundingMode, RiscvHartState, RiscvInstruction,
};

fn csr_type(csr: u16, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (u32::from(csr) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
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

const FCSR_CSR: u16 = 0x003;
const FLOAT_FLAG_OVERFLOW: u64 = 1 << 2;
const FLOAT_FLAG_INEXACT: u64 = 1 << 0;

#[test]
fn hart_executes_rv64d_fmadd_directed_rounding_when_inexact() {
    let half_ulp = f64::from_bits(0x3ca0_0000_0000_0000);
    let next_after_one = f64::from_bits(1.0_f64.to_bits() + 1);
    let mut hart = RiscvHartState::new(0xb000);
    hart.write_float(freg(1), 1.0f64.to_bits());
    hart.write_float(freg(2), 1.0f64.to_bits());
    hart.write_float(freg(3), half_ulp.to_bits());
    let static_round_up = hart
        .execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode: RiscvFloatRoundingMode::RoundUp,
        })
        .unwrap();
    assert_eq!(static_round_up.trap(), None);
    assert_eq!(hart.read_float(freg(4)), next_after_one.to_bits());
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    let mut dynamic_hart = RiscvHartState::new(0xb040);
    dynamic_hart.write(reg(10), 2 << 5);
    dynamic_hart
        .execute(RiscvInstruction::decode(csr_write_type(FCSR_CSR, 10, 0)).unwrap())
        .unwrap();
    dynamic_hart.write_float(freg(1), 1.0f64.to_bits());
    dynamic_hart.write_float(freg(2), 1.0f64.to_bits());
    dynamic_hart.write_float(freg(3), half_ulp.to_bits());
    let dynamic_round_down = dynamic_hart
        .execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        })
        .unwrap();
    assert_eq!(dynamic_round_down.trap(), None);
    assert_eq!(dynamic_hart.read_float(freg(4)), 1.0f64.to_bits());
    assert_eq!(dynamic_hart.float_status().fflags(), FLOAT_FLAG_INEXACT);
}

#[test]
fn hart_executes_rv64d_fmadd_round_down_zero_signs() {
    let mut positive_zero_hart = RiscvHartState::new(0xb080);
    positive_zero_hart.write_float(freg(1), 0.0f64.to_bits());
    positive_zero_hart.write_float(freg(2), 0.0f64.to_bits());
    positive_zero_hart.write_float(freg(3), 0.0f64.to_bits());
    positive_zero_hart
        .execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        })
        .unwrap();
    assert_eq!(positive_zero_hart.read_float(freg(4)), 0.0f64.to_bits());
    assert_eq!(positive_zero_hart.float_status().fflags(), 0);

    let mut negative_zero_hart = RiscvHartState::new(0xb0c0);
    negative_zero_hart.write_float(freg(1), 1.0f64.to_bits());
    negative_zero_hart.write_float(freg(2), 0.0f64.to_bits());
    negative_zero_hart.write_float(freg(3), (-0.0f64).to_bits());
    negative_zero_hart
        .execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        })
        .unwrap();
    assert_eq!(negative_zero_hart.read_float(freg(4)), (-0.0f64).to_bits());
    assert_eq!(negative_zero_hart.float_status().fflags(), 0);
}

#[test]
fn hart_rv64d_fmadd_directed_boundary_flags_follow_rounded_result() {
    let max_finite = f64::from_bits(0x7fef_ffff_ffff_ffff);
    let half_ulp = f64::from_bits(0x7c90_0000_0000_0000);
    let finite_flags = FLOAT_FLAG_INEXACT;
    for rounding_mode in [
        RiscvFloatRoundingMode::RoundTowardZero,
        RiscvFloatRoundingMode::RoundDown,
    ] {
        let mut hart = RiscvHartState::new(0xb100);
        hart.write_float(freg(1), max_finite.to_bits());
        hart.write_float(freg(2), 1.0f64.to_bits());
        hart.write_float(freg(3), half_ulp.to_bits());
        hart.execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode,
        })
        .unwrap();
        assert_eq!(hart.read_float(freg(4)), max_finite.to_bits());
        assert_eq!(hart.float_status().fflags(), finite_flags);
    }

    let mut round_up_hart = RiscvHartState::new(0xb140);
    round_up_hart.write_float(freg(1), max_finite.to_bits());
    round_up_hart.write_float(freg(2), 1.0f64.to_bits());
    round_up_hart.write_float(freg(3), half_ulp.to_bits());
    round_up_hart
        .execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode: RiscvFloatRoundingMode::RoundUp,
        })
        .unwrap();
    assert_eq!(round_up_hart.read_float(freg(4)), f64::INFINITY.to_bits());
    assert_eq!(
        round_up_hart.float_status().fflags(),
        FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT
    );
}

#[test]
fn hart_rv64d_fmadd_raises_overflow_when_directed_result_is_max_finite() {
    let max_finite = f64::from_bits(0x7fef_ffff_ffff_ffff);
    let one_ulp = f64::from_bits(0x7ca0_0000_0000_0000);
    let expected_flags = FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;

    for rounding_mode in [
        RiscvFloatRoundingMode::RoundTowardZero,
        RiscvFloatRoundingMode::RoundDown,
    ] {
        let mut hart = RiscvHartState::new(0xb180);
        hart.write_float(freg(1), max_finite.to_bits());
        hart.write_float(freg(2), 1.0f64.to_bits());
        hart.write_float(freg(3), one_ulp.to_bits());
        hart.execute(RiscvInstruction::FloatMultiplyAddD {
            rd: freg(4),
            rs1: freg(1),
            rs2: freg(2),
            rs3: freg(3),
            rounding_mode,
        })
        .unwrap();

        assert_eq!(hart.read_float(freg(4)), max_finite.to_bits());
        assert_eq!(hart.float_status().fflags(), expected_flags);
    }
}

#[test]
fn hart_rv64d_fmadd_directed_rounding_when_product_native_overflows() {
    let max_finite = f64::from_bits(0x7fef_ffff_ffff_ffff);
    let expected_flags = FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    for (rounding_mode, expected_bits) in [
        (
            RiscvFloatRoundingMode::RoundTowardZero,
            max_finite.to_bits(),
        ),
        (RiscvFloatRoundingMode::RoundDown, max_finite.to_bits()),
        (RiscvFloatRoundingMode::RoundUp, f64::INFINITY.to_bits()),
        (
            RiscvFloatRoundingMode::RoundNearestMaxMagnitude,
            f64::INFINITY.to_bits(),
        ),
    ] {
        let mut hart = RiscvHartState::new(0xb1c0);
        hart.write_float(freg(1), max_finite.to_bits());
        hart.write_float(freg(2), max_finite.to_bits());
        hart.write_float(freg(3), 0x0000_0000_0000_0001);

        let fused = hart
            .execute(RiscvInstruction::FloatMultiplyAddD {
                rd: freg(4),
                rs1: freg(1),
                rs2: freg(2),
                rs3: freg(3),
                rounding_mode,
            })
            .unwrap();

        assert_eq!(fused.trap(), None);
        assert_eq!(hart.read_float(freg(4)), expected_bits);
        assert_eq!(hart.float_status().fflags(), expected_flags);
    }
}

#[test]
fn hart_rv64d_fmadd_directed_rounding_when_negative_product_native_overflows() {
    let negative_max_finite = f64::from_bits(0xffef_ffff_ffff_ffff);
    let expected_flags = FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    for (rounding_mode, expected_bits) in [
        (
            RiscvFloatRoundingMode::RoundTowardZero,
            negative_max_finite.to_bits(),
        ),
        (
            RiscvFloatRoundingMode::RoundDown,
            f64::NEG_INFINITY.to_bits(),
        ),
        (
            RiscvFloatRoundingMode::RoundUp,
            negative_max_finite.to_bits(),
        ),
        (
            RiscvFloatRoundingMode::RoundNearestMaxMagnitude,
            f64::NEG_INFINITY.to_bits(),
        ),
    ] {
        let mut hart = RiscvHartState::new(0xb200);
        hart.write_float(freg(1), negative_max_finite.to_bits());
        hart.write_float(freg(2), 0x7fef_ffff_ffff_ffff);
        hart.write_float(freg(3), 0x0000_0000_0000_0001);

        let fused = hart
            .execute(RiscvInstruction::FloatMultiplyAddD {
                rd: freg(4),
                rs1: freg(1),
                rs2: freg(2),
                rs3: freg(3),
                rounding_mode,
            })
            .unwrap();

        assert_eq!(fused.trap(), None);
        assert_eq!(hart.read_float(freg(4)), expected_bits);
        assert_eq!(hart.float_status().fflags(), expected_flags);
    }
}
