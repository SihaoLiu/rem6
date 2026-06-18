use crate::{
    float,
    vector_group::{
        read_mask_bit, read_register_group, valid_register_group, write_register_group,
        VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    FloatRegister, RiscvFloatRoundingMode, RiscvHartState, RiscvVectorFloatInstruction,
    RiscvVectorFloatMulAddMode, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn execute(hart: &mut RiscvHartState, instruction: RiscvVectorFloatInstruction) -> bool {
    match instruction {
        RiscvVectorFloatInstruction::AddVv { vd, vs1, vs2 } => {
            execute_arithmetic_vv(hart, vd, vs1, vs2, FloatBinaryOp::Add)
        }
        RiscvVectorFloatInstruction::AddVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::Add)
        }
        RiscvVectorFloatInstruction::SubVv { vd, vs1, vs2 } => {
            execute_arithmetic_vv(hart, vd, vs1, vs2, FloatBinaryOp::Sub)
        }
        RiscvVectorFloatInstruction::SubVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::Sub)
        }
        RiscvVectorFloatInstruction::MinVv { vd, vs1, vs2 } => {
            execute_minmax_vv(hart, vd, vs1, vs2, FloatMinMaxOp::Min)
        }
        RiscvVectorFloatInstruction::MinVf { vd, fs1, vs2 } => {
            execute_minmax_vf(hart, vd, fs1, vs2, FloatMinMaxOp::Min)
        }
        RiscvVectorFloatInstruction::MaxVv { vd, vs1, vs2 } => {
            execute_minmax_vv(hart, vd, vs1, vs2, FloatMinMaxOp::Max)
        }
        RiscvVectorFloatInstruction::MaxVf { vd, fs1, vs2 } => {
            execute_minmax_vf(hart, vd, fs1, vs2, FloatMinMaxOp::Max)
        }
        RiscvVectorFloatInstruction::SqrtV { vd, vs2 } => execute_sqrt_v(hart, vd, vs2),
        RiscvVectorFloatInstruction::ClassV { vd, vs2 } => execute_class_v(hart, vd, vs2),
        RiscvVectorFloatInstruction::MaskEqualVv { vd, vs1, vs2 } => {
            execute_mask_compare_vv(hart, vd, vs1, vs2, FloatMaskCompareOp::Equal)
        }
        RiscvVectorFloatInstruction::MaskEqualVf { vd, fs1, vs2 } => {
            execute_mask_compare_vf(hart, vd, fs1, vs2, FloatMaskCompareOp::Equal)
        }
        RiscvVectorFloatInstruction::MaskNotEqualVv { vd, vs1, vs2 } => {
            execute_mask_compare_vv(hart, vd, vs1, vs2, FloatMaskCompareOp::NotEqual)
        }
        RiscvVectorFloatInstruction::MaskNotEqualVf { vd, fs1, vs2 } => {
            execute_mask_compare_vf(hart, vd, fs1, vs2, FloatMaskCompareOp::NotEqual)
        }
        RiscvVectorFloatInstruction::MaskLessThanVv { vd, vs1, vs2 } => {
            execute_mask_compare_vv(hart, vd, vs1, vs2, FloatMaskCompareOp::LessThan)
        }
        RiscvVectorFloatInstruction::MaskLessThanVf { vd, fs1, vs2 } => {
            execute_mask_compare_vf(hart, vd, fs1, vs2, FloatMaskCompareOp::LessThan)
        }
        RiscvVectorFloatInstruction::MaskLessEqualVv { vd, vs1, vs2 } => {
            execute_mask_compare_vv(hart, vd, vs1, vs2, FloatMaskCompareOp::LessEqual)
        }
        RiscvVectorFloatInstruction::MaskLessEqualVf { vd, fs1, vs2 } => {
            execute_mask_compare_vf(hart, vd, fs1, vs2, FloatMaskCompareOp::LessEqual)
        }
        RiscvVectorFloatInstruction::ReverseSubVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::ReverseSub)
        }
        RiscvVectorFloatInstruction::DivVv { vd, vs1, vs2 } => {
            execute_arithmetic_vv(hart, vd, vs1, vs2, FloatBinaryOp::Div)
        }
        RiscvVectorFloatInstruction::DivVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::Div)
        }
        RiscvVectorFloatInstruction::ReverseDivVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::ReverseDiv)
        }
        RiscvVectorFloatInstruction::MulVv { vd, vs1, vs2 } => {
            execute_arithmetic_vv(hart, vd, vs1, vs2, FloatBinaryOp::Mul)
        }
        RiscvVectorFloatInstruction::MulVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::Mul)
        }
        RiscvVectorFloatInstruction::MulAddVv { vd, vs1, vs2, mode } => {
            execute_mul_add_vv(hart, vd, vs1, vs2, mode)
        }
        RiscvVectorFloatInstruction::MulAddVf { vd, fs1, vs2, mode } => {
            execute_mul_add_vf(hart, vd, fs1, vs2, mode)
        }
        RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV { vd, vs2 } => {
            execute_int_to_float_v(hart, vd, vs2, VectorIntToFloatOp::Unsigned)
        }
        RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV { vd, vs2 } => {
            execute_int_to_float_v(hart, vd, vs2, VectorIntToFloatOp::Signed)
        }
        RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatV { vd, vs2 } => {
            execute_float_to_int_v(hart, vd, vs2, VectorFloatToIntOp::Unsigned)
        }
        RiscvVectorFloatInstruction::ConvertSignedIntFromFloatV { vd, vs2 } => {
            execute_float_to_int_v(hart, vd, vs2, VectorFloatToIntOp::Signed)
        }
        RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatTowardZeroV { vd, vs2 } => {
            execute_float_to_int_toward_zero_v(hart, vd, vs2, VectorFloatToIntOp::Unsigned)
        }
        RiscvVectorFloatInstruction::ConvertSignedIntFromFloatTowardZeroV { vd, vs2 } => {
            execute_float_to_int_toward_zero_v(hart, vd, vs2, VectorFloatToIntOp::Signed)
        }
        RiscvVectorFloatInstruction::SignInjectVv { vd, vs1, vs2 } => {
            execute_sign_inject_vv(hart, vd, vs1, vs2, FloatSignInjectOp::Inject)
        }
        RiscvVectorFloatInstruction::SignInjectVf { vd, fs1, vs2 } => {
            execute_sign_inject_vf(hart, vd, fs1, vs2, FloatSignInjectOp::Inject)
        }
        RiscvVectorFloatInstruction::SignInjectNegVv { vd, vs1, vs2 } => {
            execute_sign_inject_vv(hart, vd, vs1, vs2, FloatSignInjectOp::InjectNeg)
        }
        RiscvVectorFloatInstruction::SignInjectNegVf { vd, fs1, vs2 } => {
            execute_sign_inject_vf(hart, vd, fs1, vs2, FloatSignInjectOp::InjectNeg)
        }
        RiscvVectorFloatInstruction::SignInjectXorVv { vd, vs1, vs2 } => {
            execute_sign_inject_vv(hart, vd, vs1, vs2, FloatSignInjectOp::InjectXor)
        }
        RiscvVectorFloatInstruction::SignInjectXorVf { vd, fs1, vs2 } => {
            execute_sign_inject_vf(hart, vd, fs1, vs2, FloatSignInjectOp::InjectXor)
        }
        RiscvVectorFloatInstruction::MergeVf { vd, vs2, fs1 } => {
            execute_merge_vf(hart, vd, vs2, fs1)
        }
        RiscvVectorFloatInstruction::MoveVf { vd, fs1 } => execute_move_vf(hart, vd, fs1),
        RiscvVectorFloatInstruction::MoveFv { fd, vs2 } => execute_move_fv(hart, fd, vs2),
        RiscvVectorFloatInstruction::MoveSv { vd, fs1 } => execute_move_sv(hart, vd, fs1),
    }
}

#[derive(Clone, Copy)]
enum FloatBinaryOp {
    Add,
    Sub,
    ReverseSub,
    Div,
    ReverseDiv,
    Mul,
}

#[derive(Clone, Copy)]
enum FloatSignInjectOp {
    Inject,
    InjectNeg,
    InjectXor,
}

#[derive(Clone, Copy)]
enum FloatMinMaxOp {
    Min,
    Max,
}

#[derive(Clone, Copy)]
enum VectorIntToFloatOp {
    Unsigned,
    Signed,
}

#[derive(Clone, Copy)]
enum VectorFloatToIntOp {
    Unsigned,
    Signed,
}

#[derive(Clone, Copy)]
enum FloatMaskCompareOp {
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
}

struct VectorFloatMaskPlan {
    element_bytes: usize,
    group_registers: usize,
    active_elements: usize,
}

impl VectorFloatMaskPlan {
    fn new(hart: &RiscvHartState, sources: &[VectorRegister]) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let group_registers = config.register_group_registers()?;
        if sources
            .iter()
            .any(|source| !valid_register_group(*source, group_registers))
        {
            return None;
        }

        let active_elements = config.vl() as usize;
        let active_bytes = active_elements.checked_mul(element_bytes)?;
        let active_mask_bytes = active_elements.div_ceil(8);
        if active_bytes > group_registers * RISCV_VECTOR_REGISTER_BYTES
            || active_mask_bytes > RISCV_VECTOR_REGISTER_BYTES
        {
            return None;
        }

        Some(Self {
            element_bytes,
            group_registers,
            active_elements,
        })
    }
}

fn execute_arithmetic_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: FloatBinaryOp,
) -> bool {
    let Some(rounding_mode) = active_rounding_mode(hart) else {
        return false;
    };
    execute_binary_vv(hart, vd, vs1, vs2, operation, rounding_mode)
}

fn execute_arithmetic_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    fs1: FloatRegister,
    vs2: VectorRegister,
    operation: FloatBinaryOp,
) -> bool {
    let Some(rounding_mode) = active_rounding_mode(hart) else {
        return false;
    };
    execute_binary_vf(hart, vd, fs1, vs2, operation, rounding_mode)
}

fn active_rounding_mode(hart: &RiscvHartState) -> Option<RiscvFloatRoundingMode> {
    RiscvFloatRoundingMode::from_frm_bits(hart.float_status().frm() as u8)
}

fn execute_binary_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: FloatBinaryOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    if !apply_exact_lanes(&plan, &mut result, &left, &right, operation, rounding_mode) {
        return false;
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_binary_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    fs1: FloatRegister,
    vs2: VectorRegister,
    operation: FloatBinaryOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let scalar = f32::from_bits(float::single_register_bits(hart.read_float(fs1)));
    let mut result = read_register_group(hart, vd, plan.group_registers);
    if !apply_exact_scalar_lanes(&plan, &mut result, &left, scalar, operation, rounding_mode) {
        return false;
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_sign_inject_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: FloatSignInjectOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = u32::from_le_bytes(lane4(&left, offset));
        let rhs = u32::from_le_bytes(lane4(&right, offset));
        let value = sign_inject_single_bits(lhs, rhs, operation);
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_mul_add_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    mode: RiscvVectorFloatMulAddMode,
) -> bool {
    let Some(rounding_mode) = active_rounding_mode(hart) else {
        return false;
    };
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let multiplier = read_register_group(hart, vs2, plan.group_registers);
    let multiplicand = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    if !apply_exact_mul_add_lanes(
        &plan,
        &mut result,
        &multiplicand,
        &multiplier,
        mode,
        rounding_mode,
    ) {
        return false;
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_mul_add_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    fs1: FloatRegister,
    vs2: VectorRegister,
    mode: RiscvVectorFloatMulAddMode,
) -> bool {
    let Some(rounding_mode) = active_rounding_mode(hart) else {
        return false;
    };
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let scalar = float::single_register_bits(hart.read_float(fs1));
    let multiplier = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    if !apply_exact_mul_add_scalar_lanes(
        &plan,
        &mut result,
        scalar,
        &multiplier,
        mode,
        rounding_mode,
    ) {
        return false;
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_minmax_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: FloatMinMaxOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let mut exception_flags = 0;
    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = u32::from_le_bytes(lane4(&left, offset));
        let rhs = u32::from_le_bytes(lane4(&right, offset));
        exception_flags |= float::minmax_exception_flags_single_bits(lhs, rhs);
        let value = minmax_single_bits(lhs, rhs, operation);
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn execute_int_to_float_v(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    operation: VectorIntToFloatOp,
) -> bool {
    let Some(rounding_mode) = active_rounding_mode(hart) else {
        return false;
    };
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if !matches!(plan.element_bytes, 4 | 8) {
        return false;
    }

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let mut exception_flags = 0;
    match plan.element_bytes {
        4 => {
            for offset in (0..plan.active_bytes).step_by(4) {
                let value = u32::from_le_bytes(lane4(&source, offset));
                let (bits, flags) = match operation {
                    VectorIntToFloatOp::Unsigned => {
                        float::unsigned_word_to_single_bits(value, rounding_mode)
                    }
                    VectorIntToFloatOp::Signed => {
                        float::signed_word_to_single_bits(value, rounding_mode)
                    }
                };
                exception_flags |= flags;
                result[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
            }
        }
        8 => {
            for offset in (0..plan.active_bytes).step_by(8) {
                let value = u64::from_le_bytes(lane8(&source, offset));
                let (bits, flags) = match operation {
                    VectorIntToFloatOp::Unsigned => {
                        float::unsigned_doubleword_to_double_bits(value, rounding_mode)
                    }
                    VectorIntToFloatOp::Signed => {
                        float::signed_doubleword_to_double_bits(value, rounding_mode)
                    }
                };
                exception_flags |= flags;
                result[offset..offset + 8].copy_from_slice(&bits.to_le_bytes());
            }
        }
        _ => unreachable!("unsupported vector float conversion element width was rejected"),
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn execute_float_to_int_v(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    operation: VectorFloatToIntOp,
) -> bool {
    let Some(rounding_mode) = active_rounding_mode(hart) else {
        return false;
    };
    execute_float_to_int_v_with_rounding_mode(hart, vd, vs2, operation, rounding_mode)
}

fn execute_float_to_int_toward_zero_v(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    operation: VectorFloatToIntOp,
) -> bool {
    execute_float_to_int_v_with_rounding_mode(
        hart,
        vd,
        vs2,
        operation,
        RiscvFloatRoundingMode::RoundTowardZero,
    )
}

fn execute_float_to_int_v_with_rounding_mode(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    operation: VectorFloatToIntOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if !matches!(plan.element_bytes, 4 | 8) {
        return false;
    }

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let mut exception_flags = 0;
    match plan.element_bytes {
        4 => {
            for offset in (0..plan.active_bytes).step_by(4) {
                let value = u32::from_le_bytes(lane4(&source, offset));
                let (bits, flags) = match operation {
                    VectorFloatToIntOp::Unsigned => {
                        float::single_to_unsigned_word_bits(value, rounding_mode)
                    }
                    VectorFloatToIntOp::Signed => {
                        float::single_to_signed_word_bits(value, rounding_mode)
                    }
                };
                exception_flags |= flags;
                result[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
            }
        }
        8 => {
            for offset in (0..plan.active_bytes).step_by(8) {
                let value = u64::from_le_bytes(lane8(&source, offset));
                let (bits, flags) = match operation {
                    VectorFloatToIntOp::Unsigned => {
                        float::double_to_unsigned_doubleword_bits(value, rounding_mode)
                    }
                    VectorFloatToIntOp::Signed => {
                        float::double_to_signed_doubleword_bits(value, rounding_mode)
                    }
                };
                exception_flags |= flags;
                result[offset..offset + 8].copy_from_slice(&bits.to_le_bytes());
            }
        }
        _ => unreachable!("unsupported vector float conversion element width was rejected"),
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn execute_minmax_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    fs1: FloatRegister,
    vs2: VectorRegister,
    operation: FloatMinMaxOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let left = read_register_group(hart, vs2, plan.group_registers);
    let scalar = float::single_register_bits(hart.read_float(fs1));
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let mut exception_flags = 0;
    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = u32::from_le_bytes(lane4(&left, offset));
        exception_flags |= float::minmax_exception_flags_single_bits(lhs, scalar);
        let value = minmax_single_bits(lhs, scalar, operation);
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn execute_sqrt_v(hart: &mut RiscvHartState, vd: VectorRegister, vs2: VectorRegister) -> bool {
    if active_rounding_mode(hart).is_none() {
        return false;
    }
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let mut exception_flags = 0;
    for offset in (0..plan.active_bytes).step_by(4) {
        let value = u32::from_le_bytes(lane4(&source, offset));
        let lane_flags = float::sqrt_exception_flags_single_bits(value);
        if lane_flags == 0
            && !value_is_nan(value)
            && !value_is_positive_infinity(value)
            && !float::sqrt_single_rounding_insensitive_bits(value)
        {
            return false;
        }
        exception_flags |= lane_flags;
        let sqrt = float::sqrt_single_bits(value);
        result[offset..offset + 4].copy_from_slice(&sqrt.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn execute_class_v(hart: &mut RiscvHartState, vd: VectorRegister, vs2: VectorRegister) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    for offset in (0..plan.active_bytes).step_by(4) {
        let value = u32::from_le_bytes(lane4(&source, offset));
        let class = float::class_single_bits(value);
        result[offset..offset + 4].copy_from_slice(&class.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_move_vf(hart: &mut RiscvHartState, vd: VectorRegister, fs1: FloatRegister) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let scalar = float::single_register_bits(hart.read_float(fs1));
    let mut result = read_register_group(hart, vd, plan.group_registers);
    for offset in (0..plan.active_bytes).step_by(4) {
        result[offset..offset + 4].copy_from_slice(&scalar.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_merge_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    fs1: FloatRegister,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4
        || register_group_overlaps_v0(vd, plan.group_registers)
        || register_group_overlaps_v0(vs2, plan.group_registers)
    {
        return false;
    }

    let scalar = float::single_register_bits(hart.read_float(fs1));
    let scalar_bytes = scalar.to_le_bytes();
    let mask = hart.read_vector(VectorRegister::from_field(0));
    let fallback = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    for element_index in 0..plan.active_element_count() {
        let offset = element_index * 4;
        if read_mask_bit(&mask, element_index) {
            result[offset..offset + 4].copy_from_slice(&scalar_bytes);
        } else {
            result[offset..offset + 4].copy_from_slice(&fallback[offset..offset + 4]);
        }
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_move_fv(hart: &mut RiscvHartState, fd: FloatRegister, vs2: VectorRegister) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    if element_bytes != 4 {
        return false;
    }

    let source = hart.read_vector(vs2);
    let value = u32::from_le_bytes([source[0], source[1], source[2], source[3]]);
    hart.write_float(fd, box_single_bits(value));
    true
}

fn execute_move_sv(hart: &mut RiscvHartState, vd: VectorRegister, fs1: FloatRegister) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    if element_bytes != 4 {
        return false;
    }
    if config.vl() == 0 {
        return true;
    }

    let scalar = float::single_register_bits(hart.read_float(fs1));
    let mut destination = hart.read_vector(vd);
    destination[..4].copy_from_slice(&scalar.to_le_bytes());
    hart.write_vector(vd, destination);
    true
}

fn execute_mask_compare_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: FloatMaskCompareOp,
) -> bool {
    let Some(plan) = VectorFloatMaskPlan::new(hart, &[vs2, vs1]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut mask = hart.read_vector(vd);
    let mut exception_flags = 0;
    for element_index in 0..plan.active_elements {
        let offset = element_index * plan.element_bytes;
        let lhs = u32::from_le_bytes(lane4(&left, offset));
        let rhs = u32::from_le_bytes(lane4(&right, offset));
        exception_flags |= mask_compare_exception_flags(lhs, rhs, operation);
        write_mask_bit(&mut mask, element_index, mask_compare(lhs, rhs, operation));
    }
    hart.write_vector(vd, mask);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn execute_mask_compare_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    fs1: FloatRegister,
    vs2: VectorRegister,
    operation: FloatMaskCompareOp,
) -> bool {
    let Some(plan) = VectorFloatMaskPlan::new(hart, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let left = read_register_group(hart, vs2, plan.group_registers);
    let scalar = float::single_register_bits(hart.read_float(fs1));
    let mut mask = hart.read_vector(vd);
    let mut exception_flags = 0;
    for element_index in 0..plan.active_elements {
        let offset = element_index * plan.element_bytes;
        let lhs = u32::from_le_bytes(lane4(&left, offset));
        exception_flags |= mask_compare_exception_flags(lhs, scalar, operation);
        write_mask_bit(
            &mut mask,
            element_index,
            mask_compare(lhs, scalar, operation),
        );
    }
    hart.write_vector(vd, mask);
    hart.raise_float_exception_flags(exception_flags);
    true
}

fn mask_compare(lhs: u32, rhs: u32, operation: FloatMaskCompareOp) -> bool {
    match operation {
        FloatMaskCompareOp::Equal => float::equal_single_bits(lhs, rhs),
        FloatMaskCompareOp::NotEqual => !float::equal_single_bits(lhs, rhs),
        FloatMaskCompareOp::LessThan => float::less_than_single_bits(lhs, rhs),
        FloatMaskCompareOp::LessEqual => float::less_or_equal_single_bits(lhs, rhs),
    }
}

fn mask_compare_exception_flags(lhs: u32, rhs: u32, operation: FloatMaskCompareOp) -> u64 {
    match operation {
        FloatMaskCompareOp::Equal | FloatMaskCompareOp::NotEqual => {
            float::quiet_compare_exception_flags_single_bits(lhs, rhs)
        }
        FloatMaskCompareOp::LessThan | FloatMaskCompareOp::LessEqual => {
            float::signaling_compare_exception_flags_single_bits(lhs, rhs)
        }
    }
}

fn execute_sign_inject_vf(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    fs1: FloatRegister,
    vs2: VectorRegister,
    operation: FloatSignInjectOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if plan.element_bytes != 4 {
        return false;
    }

    let left = read_register_group(hart, vs2, plan.group_registers);
    let scalar = float::single_register_bits(hart.read_float(fs1));
    let mut result = read_register_group(hart, vd, plan.group_registers);
    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = u32::from_le_bytes(lane4(&left, offset));
        let value = sign_inject_single_bits(lhs, scalar, operation);
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn apply_exact_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right: &[u8; MAX_VECTOR_GROUP_BYTES],
    operation: FloatBinaryOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    match plan.element_bytes {
        4 => {
            for offset in (0..plan.active_bytes).step_by(4) {
                let lhs = f32::from_bits(u32::from_le_bytes(lane4(left, offset)));
                let rhs = f32::from_bits(u32::from_le_bytes(lane4(right, offset)));
                let Some(value) = exact_f32_binary(lhs, rhs, operation, rounding_mode) else {
                    return false;
                };
                result[offset..offset + 4].copy_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        8 => {
            for offset in (0..plan.active_bytes).step_by(8) {
                let lhs = u64::from_le_bytes(lane8(left, offset));
                let rhs = u64::from_le_bytes(lane8(right, offset));
                let Some(value) = exact_f64_binary(lhs, rhs, operation, rounding_mode) else {
                    return false;
                };
                result[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        _ => return false,
    }
    true
}

fn apply_exact_scalar_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: f32,
    operation: FloatBinaryOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    if plan.element_bytes != 4 {
        return false;
    }

    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = f32::from_bits(u32::from_le_bytes(lane4(left, offset)));
        let (left, right, lane_operation) = match operation {
            FloatBinaryOp::ReverseSub => (scalar, lhs, FloatBinaryOp::Sub),
            FloatBinaryOp::ReverseDiv => (scalar, lhs, FloatBinaryOp::Div),
            _ => (lhs, scalar, operation),
        };
        let value = exact_f32_binary(left, right, lane_operation, rounding_mode);
        let Some(value) = value else {
            return false;
        };
        result[offset..offset + 4].copy_from_slice(&value.to_bits().to_le_bytes());
    }
    true
}

fn exact_f32_binary(
    lhs: f32,
    rhs: f32,
    operation: FloatBinaryOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> Option<f32> {
    if !lhs.is_finite() || !rhs.is_finite() {
        return None;
    }
    if matches!(
        operation,
        FloatBinaryOp::Add | FloatBinaryOp::Sub | FloatBinaryOp::ReverseSub
    ) {
        let bits = float::exact_finite_single_add_sub_bits(
            lhs.to_bits(),
            rhs.to_bits(),
            rounding_mode,
            matches!(operation, FloatBinaryOp::Sub | FloatBinaryOp::ReverseSub),
        )?;
        return Some(f32::from_bits(bits));
    }
    let exact = match operation {
        FloatBinaryOp::Div => f64::from(lhs) / f64::from(rhs),
        FloatBinaryOp::Mul => f64::from(lhs) * f64::from(rhs),
        FloatBinaryOp::Add | FloatBinaryOp::Sub | FloatBinaryOp::ReverseSub => {
            unreachable!("handled above")
        }
        FloatBinaryOp::ReverseDiv => unreachable!("converted to div before dispatch"),
    };
    let rounded = exact as f32;
    (rounded.is_finite() && f64::from(rounded) == exact).then_some(rounded)
}

fn exact_f64_binary(
    lhs: u64,
    rhs: u64,
    operation: FloatBinaryOp,
    rounding_mode: RiscvFloatRoundingMode,
) -> Option<u64> {
    match operation {
        FloatBinaryOp::Add | FloatBinaryOp::Sub => float::exact_finite_double_add_sub_bits(
            lhs,
            rhs,
            rounding_mode,
            matches!(operation, FloatBinaryOp::Sub),
        ),
        FloatBinaryOp::Mul => float::exact_finite_double_mul_bits(lhs, rhs),
        FloatBinaryOp::Div => None,
        FloatBinaryOp::ReverseSub | FloatBinaryOp::ReverseDiv => {
            unreachable!("reverse operations are scalar-only")
        }
    }
}

fn apply_exact_mul_add_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    multiplicand: &[u8; MAX_VECTOR_GROUP_BYTES],
    multiplier: &[u8; MAX_VECTOR_GROUP_BYTES],
    mode: RiscvVectorFloatMulAddMode,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = mul_add_multiplicand_bits(u32::from_le_bytes(lane4(multiplicand, offset)), mode);
        let rhs = u32::from_le_bytes(lane4(multiplier, offset));
        let addend = mul_add_accumulator_bits(u32::from_le_bytes(lane4(result, offset)), mode);
        let Some(value) = float::exact_finite_single_mul_add_bits(lhs, rhs, addend, rounding_mode)
        else {
            return false;
        };
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    true
}

fn apply_exact_mul_add_scalar_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u32,
    multiplier: &[u8; MAX_VECTOR_GROUP_BYTES],
    mode: RiscvVectorFloatMulAddMode,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    let scalar = mul_add_multiplicand_bits(scalar, mode);
    for offset in (0..plan.active_bytes).step_by(4) {
        let rhs = u32::from_le_bytes(lane4(multiplier, offset));
        let addend = mul_add_accumulator_bits(u32::from_le_bytes(lane4(result, offset)), mode);
        let Some(value) =
            float::exact_finite_single_mul_add_bits(scalar, rhs, addend, rounding_mode)
        else {
            return false;
        };
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    true
}

fn mul_add_multiplicand_bits(value: u32, mode: RiscvVectorFloatMulAddMode) -> u32 {
    if mode.negates_product() {
        flip_single_sign(value)
    } else {
        value
    }
}

fn mul_add_accumulator_bits(value: u32, mode: RiscvVectorFloatMulAddMode) -> u32 {
    if mode.negates_accumulator() {
        flip_single_sign(value)
    } else {
        value
    }
}

fn flip_single_sign(value: u32) -> u32 {
    value ^ 0x8000_0000
}

fn sign_inject_single_bits(lhs: u32, rhs: u32, operation: FloatSignInjectOp) -> u32 {
    const SIGN_BIT: u32 = 0x8000_0000;
    let rhs_sign = rhs & SIGN_BIT;
    let sign = match operation {
        FloatSignInjectOp::Inject => rhs_sign,
        FloatSignInjectOp::InjectNeg => (!rhs_sign) & SIGN_BIT,
        FloatSignInjectOp::InjectXor => (lhs ^ rhs) & SIGN_BIT,
    };
    (lhs & !SIGN_BIT) | sign
}

fn minmax_single_bits(lhs: u32, rhs: u32, operation: FloatMinMaxOp) -> u32 {
    match operation {
        FloatMinMaxOp::Min => float::min_single_bits(lhs, rhs),
        FloatMinMaxOp::Max => float::max_single_bits(lhs, rhs),
    }
}

fn value_is_nan(value: u32) -> bool {
    value & 0x7f80_0000 == 0x7f80_0000 && value & 0x007f_ffff != 0
}

fn value_is_positive_infinity(value: u32) -> bool {
    value == 0x7f80_0000
}

fn box_single_bits(value: u32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(value)
}

fn register_group_overlaps_v0(register: VectorRegister, group_registers: usize) -> bool {
    register.index() == 0 && group_registers > 0
}

fn write_mask_bit(mask: &mut [u8; RISCV_VECTOR_REGISTER_BYTES], element_index: usize, value: bool) {
    let byte_index = element_index / 8;
    let bit = 1_u8 << (element_index % 8);
    if value {
        mask[byte_index] |= bit;
    } else {
        mask[byte_index] &= !bit;
    }
}

fn lane4(bytes: &[u8; MAX_VECTOR_GROUP_BYTES], offset: usize) -> [u8; 4] {
    [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]
}

fn lane8(bytes: &[u8; MAX_VECTOR_GROUP_BYTES], offset: usize) -> [u8; 8] {
    [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]
}
