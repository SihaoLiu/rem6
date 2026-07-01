use crate::RiscvFloatRoundingMode;

use super::{
    box_single, is_nan_double, is_signaling_nan_double, DEFAULT_NAN_SINGLE_BITS,
    FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID, FLOAT_FLAG_OVERFLOW, FLOAT_FLAG_UNDERFLOW,
    SINGLE_EXP_MASK,
};

pub(super) fn double_to_single_register_write(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    box_single(double_to_single_bits(value, rounding_mode))
}

pub(super) fn double_to_single_exception_flags(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    if is_signaling_nan_double(value) {
        return FLOAT_FLAG_INVALID;
    }
    if is_nan_double(value) {
        return 0;
    }

    let value = f64::from_bits(value);
    if !value.is_finite() {
        return 0;
    }

    let rounded = double_to_single_bits(value.to_bits(), rounding_mode);
    let rounded_value = f32::from_bits(rounded);
    let mut flags = 0;
    if f64::from(rounded_value) != value {
        flags |= FLOAT_FLAG_INEXACT;
    }
    if double_to_single_overflows(value, rounded_value, rounding_mode) {
        flags |= FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    if flags & FLOAT_FLAG_INEXACT != 0 && value != 0.0 && rounded & SINGLE_EXP_MASK == 0 {
        flags |= FLOAT_FLAG_UNDERFLOW;
    }
    flags
}

fn double_to_single_overflows(
    value: f64,
    rounded_value: f32,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    rounded_value.is_infinite() || directed_finite_max_overflows(value, rounding_mode)
}

fn directed_finite_max_overflows(value: f64, rounding_mode: RiscvFloatRoundingMode) -> bool {
    let single_exponent_overflow = f64::from_bits(0x47f0_0000_0000_0000);
    if value.abs() < single_exponent_overflow {
        return false;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundTowardZero => true,
        RiscvFloatRoundingMode::RoundDown => value.is_sign_positive(),
        RiscvFloatRoundingMode::RoundUp => value.is_sign_negative(),
        RiscvFloatRoundingMode::RoundNearestEven
        | RiscvFloatRoundingMode::RoundNearestMaxMagnitude
        | RiscvFloatRoundingMode::Dynamic => false,
    }
}

fn double_to_single_bits(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u32 {
    if is_nan_double(value) {
        return DEFAULT_NAN_SINGLE_BITS;
    }

    let exact = f64::from_bits(value);
    let nearest = exact as f32;
    if f64::from(nearest) == exact {
        return nearest.to_bits();
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest.to_bits(),
        RiscvFloatRoundingMode::RoundTowardZero => {
            if exact.is_sign_negative() {
                step_up_if_below(nearest, exact).to_bits()
            } else {
                step_down_if_above(nearest, exact).to_bits()
            }
        }
        RiscvFloatRoundingMode::RoundDown => step_down_if_above(nearest, exact).to_bits(),
        RiscvFloatRoundingMode::RoundUp => step_up_if_below(nearest, exact).to_bits(),
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_nearest_max_magnitude(exact, nearest).to_bits()
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn step_down_if_above(nearest: f32, exact: f64) -> f32 {
    if f64::from(nearest) > exact {
        nearest.next_down()
    } else {
        nearest
    }
}

fn step_up_if_below(nearest: f32, exact: f64) -> f32 {
    if f64::from(nearest) < exact {
        nearest.next_up()
    } else {
        nearest
    }
}

fn round_nearest_max_magnitude(exact: f64, nearest: f32) -> f32 {
    if !nearest.is_finite() {
        return nearest;
    }

    let nearest_value = f64::from(nearest);
    let (lower, upper) = if nearest_value < exact {
        (nearest, nearest.next_up())
    } else {
        (nearest.next_down(), nearest)
    };
    if !lower.is_finite() || !upper.is_finite() {
        return nearest;
    }

    let midpoint = (f64::from(lower) + f64::from(upper)) / 2.0;
    if exact == midpoint {
        if exact.is_sign_negative() {
            lower
        } else {
            upper
        }
    } else {
        nearest
    }
}
