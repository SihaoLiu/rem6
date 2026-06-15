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
fn hart_executes_rv64f_single_add_directed_rounding_when_inexact() {
    let half_ulp = f32::from_bits(0x3380_0000);
    let next_after_one = f32::from_bits(1.0_f32.to_bits() + 1);
    let mut hart = RiscvHartState::new(0x8780);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(half_ulp));
    let static_round_up = hart
        .execute(RiscvInstruction::FloatAddS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::RoundUp,
        })
        .unwrap();
    assert_eq!(static_round_up.trap(), None);
    assert_eq!(hart.read_float(freg(3)), f32_box(next_after_one));
    assert_eq!(hart.float_status().fflags(), FLOAT_FLAG_INEXACT);

    let mut dynamic_hart = RiscvHartState::new(0x87c0);
    dynamic_hart.write(reg(10), 2 << 5);
    dynamic_hart
        .execute(RiscvInstruction::decode(csr_write_type(FCSR_CSR, 10, 0)).unwrap())
        .unwrap();
    dynamic_hart.write_float(freg(1), f32_box(1.0));
    dynamic_hart.write_float(freg(2), f32_box(half_ulp));
    let dynamic_round_down = dynamic_hart
        .execute(RiscvInstruction::FloatAddS {
            rd: freg(3),
            rs1: freg(1),
            rs2: freg(2),
            rounding_mode: RiscvFloatRoundingMode::Dynamic,
        })
        .unwrap();
    assert_eq!(dynamic_round_down.trap(), None);
    assert_eq!(dynamic_hart.read_float(freg(3)), f32_box(1.0));
    assert_eq!(dynamic_hart.float_status().fflags(), FLOAT_FLAG_INEXACT);
}

#[test]
fn hart_executes_rv64f_single_add_round_down_exact_cancellation_to_negative_zero() {
    let mut hart = RiscvHartState::new(0x87d0);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(-1.0));
    let record = hart
        .execute(RiscvInstruction::FloatAddS {
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
fn hart_rv64f_single_add_raises_invalid_for_nan_and_infinity_cases() {
    let mut hart = RiscvHartState::new(0x87e0);
    hart.write_float(freg(1), box_single(0x7f80_0001));
    hart.write_float(freg(2), f32_box(1.0));
    hart.execute(RiscvInstruction::FloatAddS {
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
    hart.write_float(freg(5), f32_box(f32::NEG_INFINITY));
    hart.execute(RiscvInstruction::FloatAddS {
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
fn hart_rv64f_single_add_sets_inexact_beyond_directed_rounding_window() {
    let mut hart = RiscvHartState::new(0x87f0);
    hart.write_float(freg(1), f32_box(1.0));
    hart.write_float(freg(2), f32_box(f32::from_bits(0x3080_0000)));
    hart.execute(RiscvInstruction::FloatAddS {
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
fn hart_rv64f_single_add_raises_overflow_and_inexact_for_overflowing_sum() {
    let max_finite = f32::from_bits(0x7f7f_ffff);
    let expected_flags = FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    let mut hart = RiscvHartState::new(0x87f8);
    hart.write_float(freg(1), f32_box(max_finite));
    hart.write_float(freg(2), f32_box(max_finite));
    hart.execute(RiscvInstruction::FloatAddS {
        rd: freg(3),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(3)), f32_box(f32::INFINITY));
    assert_eq!(hart.float_status().fflags(), expected_flags);

    hart.set_float_status(rem6_isa_riscv::RiscvFloatStatus::new(0));
    hart.execute(RiscvInstruction::FloatAddS {
        rd: freg(4),
        rs1: freg(1),
        rs2: freg(2),
        rounding_mode: RiscvFloatRoundingMode::RoundTowardZero,
    })
    .unwrap();
    assert_eq!(hart.read_float(freg(4)), f32_box(max_finite));
    assert_eq!(hart.float_status().fflags(), expected_flags);
}
