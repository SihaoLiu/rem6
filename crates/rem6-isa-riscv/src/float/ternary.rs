use crate::{FloatRegister, RiscvFloatRoundingMode, RiscvInstruction};

use super::{fused, register_rounding_mode_is_supported, sign_negate_double, sign_negate_single};

pub(crate) fn ternary_register_rounding_mode_is_supported(
    instruction: RiscvInstruction,
    frm: u64,
    lhs: u64,
    rhs: u64,
    addend: u64,
) -> bool {
    register_rounding_mode_is_supported(
        instruction,
        frm,
        ternary_result_is_rounding_insensitive(instruction, lhs, rhs, addend),
        ternary_rounding_mode_is_implemented(instruction, lhs, rhs, addend),
    )
}

fn ternary_rounding_mode_is_implemented(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    addend: u64,
) -> bool {
    let Some(dispatch) = fused_dispatch(instruction, lhs, rhs, addend) else {
        return false;
    };
    match dispatch.format {
        FusedFormat::Single => fused::single_directed_rounding_is_supported(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
        ),
        FusedFormat::Double => fused::double_directed_rounding_is_supported(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
        ),
    }
}

fn ternary_result_is_rounding_insensitive(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    addend: u64,
) -> bool {
    let Some(dispatch) = fused_dispatch(instruction, lhs, rhs, addend) else {
        return false;
    };
    match dispatch.format {
        FusedFormat::Single => fused_single_is_identity(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
        ),
        FusedFormat::Double => fused_double_is_identity(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
        ),
    }
}

fn fused_single_is_identity(lhs: u64, rhs: u64, addend: u64) -> bool {
    let lhs = f32::from_bits(super::unbox_single(lhs));
    let rhs = f32::from_bits(super::unbox_single(rhs));
    let addend = f32::from_bits(super::unbox_single(addend));
    lhs.is_finite()
        && rhs.is_finite()
        && addend.is_finite()
        && lhs != 0.0
        && rhs != 0.0
        && addend == 0.0
        && (lhs.abs() == 1.0 || rhs.abs() == 1.0)
}

fn fused_double_is_identity(lhs: u64, rhs: u64, addend: u64) -> bool {
    let lhs = f64::from_bits(lhs);
    let rhs = f64::from_bits(rhs);
    let addend = f64::from_bits(addend);
    lhs.is_finite()
        && rhs.is_finite()
        && addend.is_finite()
        && lhs != 0.0
        && rhs != 0.0
        && addend == 0.0
        && (lhs.abs() == 1.0 || rhs.abs() == 1.0)
}

pub(crate) fn float_register_write_ternary(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    addend: u64,
    frm: u64,
) -> (FloatRegister, u64) {
    let dispatch = fused_dispatch(instruction, lhs, rhs, addend)
        .expect("non-fused-float instruction dispatched to ternary float register write");
    let rounding_mode = dispatch
        .rounding_mode
        .resolve(frm)
        .expect("ternary float rounding mode is valid");
    let value = match dispatch.format {
        FusedFormat::Single => fused::single_register_write(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
            rounding_mode,
        ),
        FusedFormat::Double => fused::double_register_write(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
            rounding_mode,
        ),
    };
    (dispatch.rd, value)
}

#[derive(Clone, Copy)]
enum FusedFormat {
    Single,
    Double,
}

#[derive(Clone, Copy)]
struct FusedOperands {
    lhs: u64,
    rhs: u64,
    addend: u64,
}

#[derive(Clone, Copy)]
struct FusedDispatch {
    rd: FloatRegister,
    rounding_mode: RiscvFloatRoundingMode,
    format: FusedFormat,
    operands: FusedOperands,
}

fn fused_dispatch(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    addend: u64,
) -> Option<FusedDispatch> {
    let (rd, rounding_mode, format, operands) = match instruction {
        RiscvInstruction::FloatMultiplyAddS {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Single,
            FusedOperands { lhs, rhs, addend },
        ),
        RiscvInstruction::FloatMultiplySubtractS {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Single,
            FusedOperands {
                lhs,
                rhs,
                addend: sign_negate_single(addend),
            },
        ),
        RiscvInstruction::FloatNegativeMultiplySubtractS {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Single,
            FusedOperands {
                lhs: sign_negate_single(lhs),
                rhs,
                addend,
            },
        ),
        RiscvInstruction::FloatNegativeMultiplyAddS {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Single,
            FusedOperands {
                lhs: sign_negate_single(lhs),
                rhs,
                addend: sign_negate_single(addend),
            },
        ),
        RiscvInstruction::FloatMultiplyAddD {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Double,
            FusedOperands { lhs, rhs, addend },
        ),
        RiscvInstruction::FloatMultiplySubtractD {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Double,
            FusedOperands {
                lhs,
                rhs,
                addend: sign_negate_double(addend),
            },
        ),
        RiscvInstruction::FloatNegativeMultiplySubtractD {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Double,
            FusedOperands {
                lhs: sign_negate_double(lhs),
                rhs,
                addend,
            },
        ),
        RiscvInstruction::FloatNegativeMultiplyAddD {
            rd, rounding_mode, ..
        } => (
            rd,
            rounding_mode,
            FusedFormat::Double,
            FusedOperands {
                lhs: sign_negate_double(lhs),
                rhs,
                addend: sign_negate_double(addend),
            },
        ),
        _ => return None,
    };
    Some(FusedDispatch {
        rd,
        rounding_mode,
        format,
        operands,
    })
}

pub(crate) fn ternary_exception_flags(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    addend: u64,
    frm: u64,
) -> u64 {
    let Some(dispatch) = fused_dispatch(instruction, lhs, rhs, addend) else {
        return 0;
    };
    let rounding_mode = dispatch
        .rounding_mode
        .resolve(frm)
        .expect("ternary float rounding mode is valid");
    match dispatch.format {
        FusedFormat::Single => fused::single_exception_flags(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
            rounding_mode,
        ),
        FusedFormat::Double => fused::double_exception_flags(
            dispatch.operands.lhs,
            dispatch.operands.rhs,
            dispatch.operands.addend,
            rounding_mode,
        ),
    }
}
