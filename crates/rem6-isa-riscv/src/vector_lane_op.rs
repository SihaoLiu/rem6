use crate::{
    vector::RiscvVectorFixedRoundingMode,
    vector_fixed_point_shift::{round_signed, round_unsigned},
};

#[derive(Clone, Copy)]
pub(crate) enum LaneBinaryOp {
    Add,
    Sub,
    ReverseSub,
    MinUnsigned,
    MinSigned,
    MaxUnsigned,
    MaxSigned,
    MultiplyLow,
    MultiplyHighUnsigned,
    MultiplyHighSignedUnsigned,
    MultiplyHighSigned,
    DivideUnsigned,
    DivideSigned,
    RemainderUnsigned,
    RemainderSigned,
    And,
    Or,
    Xor,
    ShiftLeftLogical,
    ShiftRightLogical,
    ShiftRightArithmetic,
    ScalingShiftRightLogical {
        rounding_mode: RiscvVectorFixedRoundingMode,
    },
    ScalingShiftRightArithmetic {
        rounding_mode: RiscvVectorFixedRoundingMode,
    },
}

impl LaneBinaryOp {
    pub(crate) fn apply_u8(self, left: u8, right: u8) -> u8 {
        let shift = shift_amount(u64::from(right), 8);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::ReverseSub => right.wrapping_sub(left),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i8).min(right as i8) as u8,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i8).max(right as i8) as u8,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u8::BITS) as u8
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i8), u128::from(right), u8::BITS)
                    as u8
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i8), i128::from(right as i8), u8::BITS)
                    as u8
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u8::MAX.into()) as u8
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i8),
                i128::from(right as i8),
                i128::from(i8::MIN),
            ) as u8,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u8
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i8),
                i128::from(right as i8),
                i128::from(i8::MIN),
            ) as u8,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i8) >> shift) as u8,
            Self::ScalingShiftRightLogical { rounding_mode } => {
                scaling_shift_right_unsigned(u128::from(left), shift, rounding_mode) as u8
            }
            Self::ScalingShiftRightArithmetic { rounding_mode } => {
                scaling_shift_right_signed(i128::from(left as i8), shift, rounding_mode) as u8
            }
        }
    }

    pub(crate) fn apply_u16(self, left: u16, right: u16) -> u16 {
        let shift = shift_amount(u64::from(right), 16);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::ReverseSub => right.wrapping_sub(left),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i16).min(right as i16) as u16,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i16).max(right as i16) as u16,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u16::BITS) as u16
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i16), u128::from(right), u16::BITS)
                    as u16
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i16), i128::from(right as i16), u16::BITS)
                    as u16
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u16::MAX.into()) as u16
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i16),
                i128::from(right as i16),
                i128::from(i16::MIN),
            ) as u16,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u16
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i16),
                i128::from(right as i16),
                i128::from(i16::MIN),
            ) as u16,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i16) >> shift) as u16,
            Self::ScalingShiftRightLogical { rounding_mode } => {
                scaling_shift_right_unsigned(u128::from(left), shift, rounding_mode) as u16
            }
            Self::ScalingShiftRightArithmetic { rounding_mode } => {
                scaling_shift_right_signed(i128::from(left as i16), shift, rounding_mode) as u16
            }
        }
    }

    pub(crate) fn apply_u32(self, left: u32, right: u32) -> u32 {
        let shift = shift_amount(u64::from(right), 32);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::ReverseSub => right.wrapping_sub(left),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i32).min(right as i32) as u32,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i32).max(right as i32) as u32,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u32::BITS) as u32
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i32), u128::from(right), u32::BITS)
                    as u32
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i32), i128::from(right as i32), u32::BITS)
                    as u32
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u32::MAX.into()) as u32
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i32),
                i128::from(right as i32),
                i128::from(i32::MIN),
            ) as u32,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u32
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i32),
                i128::from(right as i32),
                i128::from(i32::MIN),
            ) as u32,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i32) >> shift) as u32,
            Self::ScalingShiftRightLogical { rounding_mode } => {
                scaling_shift_right_unsigned(u128::from(left), shift, rounding_mode) as u32
            }
            Self::ScalingShiftRightArithmetic { rounding_mode } => {
                scaling_shift_right_signed(i128::from(left as i32), shift, rounding_mode) as u32
            }
        }
    }

    pub(crate) fn apply_u64(self, left: u64, right: u64) -> u64 {
        let shift = shift_amount(right, 64);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::ReverseSub => right.wrapping_sub(left),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i64).min(right as i64) as u64,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i64).max(right as i64) as u64,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u64::BITS) as u64
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i64), u128::from(right), u64::BITS)
                    as u64
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i64), i128::from(right as i64), u64::BITS)
                    as u64
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u64::MAX.into()) as u64
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i64),
                i128::from(right as i64),
                i128::from(i64::MIN),
            ) as u64,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u64
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i64),
                i128::from(right as i64),
                i128::from(i64::MIN),
            ) as u64,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i64) >> shift) as u64,
            Self::ScalingShiftRightLogical { rounding_mode } => {
                scaling_shift_right_unsigned(u128::from(left), shift, rounding_mode) as u64
            }
            Self::ScalingShiftRightArithmetic { rounding_mode } => {
                scaling_shift_right_signed(i128::from(left as i64), shift, rounding_mode) as u64
            }
        }
    }
}

fn scaling_shift_right_unsigned(
    value: u128,
    shift: u32,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> u128 {
    round_unsigned(value, shift, rounding_mode)
        .expect("single-width unsigned vector scaling shift cannot overflow")
        >> shift
}

fn scaling_shift_right_signed(
    value: i128,
    shift: u32,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> u128 {
    (round_signed(value, shift, rounding_mode)
        .expect("single-width signed vector scaling shift cannot overflow")
        >> shift) as u128
}

fn multiply_high_unsigned(left: u128, right: u128, element_bits: u32) -> u128 {
    (left * right) >> element_bits
}

fn multiply_high_signed(left: i128, right: i128, element_bits: u32) -> u128 {
    ((left * right) >> element_bits) as u128
}

fn multiply_high_signed_unsigned(left: i128, right: u128, element_bits: u32) -> u128 {
    ((left * right as i128) >> element_bits) as u128
}

fn divide_unsigned(left: u128, right: u128, division_by_zero_result: u128) -> u128 {
    if right == 0 {
        division_by_zero_result
    } else {
        left / right
    }
}

fn divide_signed(left: i128, right: i128, min_value: i128) -> u128 {
    if right == 0 {
        u128::MAX
    } else if left == min_value && right == -1 {
        min_value as u128
    } else {
        (left / right) as u128
    }
}

fn remainder_unsigned(left: u128, right: u128) -> u128 {
    if right == 0 {
        left
    } else {
        left % right
    }
}

fn remainder_signed(left: i128, right: i128, min_value: i128) -> u128 {
    if right == 0 {
        left as u128
    } else if left == min_value && right == -1 {
        0
    } else {
        (left % right) as u128
    }
}

fn shift_amount(raw: u64, element_bits: u32) -> u32 {
    (raw & u64::from(element_bits - 1)) as u32
}
