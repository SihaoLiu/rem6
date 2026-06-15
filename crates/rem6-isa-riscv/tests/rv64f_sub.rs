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

fn box_single(bits: u32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(bits)
}

fn f32_box(value: f32) -> u64 {
    box_single(value.to_bits())
}

const FCSR_CSR: u16 = 0x003;
const FLOAT_FLAG_INVALID: u64 = 1 << 4;
const FLOAT_FLAG_OVERFLOW: u64 = 1 << 2;
const FLOAT_FLAG_INEXACT: u64 = 1 << 0;

#[test]
fn hart_executes_rv64f_single_sub_directed_rounding_when_inexact() {
    let half_down_ulp = f32::from_bits(0x3300_0000);
    let next_before_one = f32::from_bits(1.0_f32.to_bits() - 1);
    let mut hart = RiscvHartState::new(0x9780);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(half_down_ulp));
    let static_round_down = hart
        .execute(RiscvInstruction::FloatSubS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        })
        .unwrap();
    assert_eq!(static_round_down.trap(), None);
    assert_eq!(hart.read_float(freg(3)), f32_box(next_before_one));
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    let mut dynamic_hart = RiscvHartState::new(0x97c0);
    dynamic_hart.write(reg(10), 3 << 5);
    dynamic_hart
        .execute(RiscvInstruction::decode(csr_write_type(FCSR_CSR, 10, 0)).unwrap())
        .unwrap();
    dynamic_hart.write_float(freg(1), f32_box(1.0));
    dynamic_hart.write_float(freg(2), f32_box(half_down_ulp));
    let dynamic_round_up = dynamic_hart
        .execute(RiscvInstruction::FloatSubS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        })
        .unwrap();
    assert_eq!(dynamic_round_up.trap(), None);
    assert_eq!(dynamic_hart.read_float(freg(3)), f32_box(1.0));
    assert_eq!(dynamic_hart.float_status().fflags(), FLOAT_FLAG_INEXACT);
}

#[test]
fn hart_executes_rv64f_single_sub_round_down_exact_cancellation_to_negative_zero() {
    let mut hart = RiscvHartState::new(0x97d0);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(1.0));
    let record = hart
        .execute(RiscvInstruction::FloatSubS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundDown,
        })
        .unwrap();
    assert_eq!(record.trap(), None);
    assert_eq!(hart.read_float(freg(3)), box_single(0x8000_0000));
    assert_eq!(hart.float_status().fflags(), 0);
}

#[test]
fn hart_executes_rv64f_single_sub_round_nearest_max_magnitude() {
    let half_down_ulp = f32::from_bits(0x3300_0000);
    let mut hart = RiscvHartState::new(0x97d8);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(half_down_ulp));
    let static_rmm = hart
        .execute(RiscvInstruction::FloatSubS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestMaxMagnitude,
        })
        .unwrap();
    assert_eq!(static_rmm.trap(), None);
    assert_eq!(hart.read_float(freg(3)), f32_box(1.0));
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    let mut dynamic_hart = RiscvHartState::new(0x97dc);
    dynamic_hart.write(reg(10), 4 << 5);
    dynamic_hart
        .execute(RiscvInstruction::decode(csr_write_type(FCSR_CSR, 10, 0)).unwrap())
        .unwrap();
    dynamic_hart.write_float(freg(1), f32_box(1.0));
    dynamic_hart.write_float(freg(2), f32_box(half_down_ulp));
    let dynamic_rmm = dynamic_hart
        .execute(RiscvInstruction::FloatSubS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        })
        .unwrap();
    assert_eq!(dynamic_rmm.trap(), None);
    assert_eq!(dynamic_hart.read_float(freg(3)), f32_box(1.0));
    assert_eq!(dynamic_hart.float_status().fflags(), FLOAT_FLAG_INEXACT);
}

#[test]
fn hart_rv64f_single_sub_raises_invalid_for_nan_and_infinity_cases() {
    let mut hart = RiscvHartState::new(0x97e0);
    hart.write_float(freg(1), box_single(0x7f80_0001));
    hart.write_float(freg(2), f32_box(1.0));
    hart.execute(RiscvInstruction::FloatSubS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), box_single(0x7fc0_0000));
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.write_float(freg(4), f32_box(f32::INFINITY));
    hart.write_float(freg(5), f32_box(f32::INFINITY));
    hart.execute(RiscvInstruction::FloatSubS {
        rd: freg(6),
        rs1: freg(4),
        rs2: freg(5),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(6)), box_single(0x7fc0_0000));
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INVALID);
}

#[test]
fn hart_rv64f_single_sub_raises_overflow_for_directed_boundary_difference() {
    let max_finite = f32::from_bits(0x7f7f_ffff);
    let quarter_ulp = f32::from_bits(0x7280_0000);
    let expected_flags = FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    let mut hart = RiscvHartState::new(0x97f4);
    hart.write_float(freg(1), f32_box(max_finite));
    hart.write_float(freg(2), f32_box(-quarter_ulp));
    hart.execute(RiscvInstruction::FloatSubS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundUp,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), f32_box(f32::INFINITY));
    assert_eq!(hart.float_status().fflags(), expected_flags);
}

#[test]
fn hart_rv64f_single_sub_sets_inexact_beyond_directed_rounding_window() {
    let mut hart = RiscvHartState::new(0x97f0);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(f32::from_bits(0x3080_0000)));
    hart.execute(RiscvInstruction::FloatSubS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), f32_box(1.0));
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);
}

#[test]
fn hart_rv64f_single_sub_raises_overflow_and_inexact_for_overflowing_difference() {
    let max_finite = f32::from_bits(0x7f7f_ffff);
    let expected_flags = FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    let mut hart = RiscvHartState::new(0x97f8);
    hart.write_float(freg(1), f32_box(max_finite));
    hart.write_float(freg(2), f32_box(-max_finite));
    hart.execute(RiscvInstruction::FloatSubS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), f32_box(f32::INFINITY));
    assert_eq!(hart.float_status().fflags(), expected_flags);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.execute(RiscvInstruction::FloatSubS {
        rd: freg(4),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundTowardZero,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), f32_box(max_finite));
    assert_eq!(hart.float_status().fflags(), expected_flags);
}
