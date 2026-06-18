use crate::{
    float,
    vector_group::{
        read_register_group, valid_register_group, write_register_group, VectorBinaryPlan,
        MAX_VECTOR_GROUP_BYTES,
    },
    FloatRegister, RiscvFloatRoundingMode, RiscvHartState, RiscvVectorFloatInstruction,
    VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
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
    if plan.element_bytes != 4 {
        return false;
    }

    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = f32::from_bits(u32::from_le_bytes(lane4(left, offset)));
        let rhs = f32::from_bits(u32::from_le_bytes(lane4(right, offset)));
        let Some(value) = exact_f32_binary(lhs, rhs, operation, rounding_mode) else {
            return false;
        };
        result[offset..offset + 4].copy_from_slice(&value.to_bits().to_le_bytes());
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
