use crate::{
    float,
    vector_group::{
        read_register_group, write_register_group, VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    FloatRegister, RiscvFloatRoundingMode, RiscvHartState, RiscvVectorFloatInstruction,
    VectorRegister,
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
        RiscvVectorFloatInstruction::ReverseSubVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::ReverseSub)
        }
        RiscvVectorFloatInstruction::MulVv { vd, vs1, vs2 } => {
            execute_arithmetic_vv(hart, vd, vs1, vs2, FloatBinaryOp::Mul)
        }
        RiscvVectorFloatInstruction::MulVf { vd, fs1, vs2 } => {
            execute_arithmetic_vf(hart, vd, fs1, vs2, FloatBinaryOp::Mul)
        }
        RiscvVectorFloatInstruction::SignInjectVf { vd, fs1, vs2 } => {
            execute_sign_inject_vf(hart, vd, fs1, vs2, FloatSignInjectOp::Inject)
        }
        RiscvVectorFloatInstruction::SignInjectNegVf { vd, fs1, vs2 } => {
            execute_sign_inject_vf(hart, vd, fs1, vs2, FloatSignInjectOp::InjectNeg)
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
    Mul,
}

#[derive(Clone, Copy)]
enum FloatSignInjectOp {
    Inject,
    InjectNeg,
    InjectXor,
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
        let value = if matches!(operation, FloatBinaryOp::ReverseSub) {
            exact_f32_binary(scalar, lhs, FloatBinaryOp::Sub, rounding_mode)
        } else {
            exact_f32_binary(lhs, scalar, operation, rounding_mode)
        };
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
        FloatBinaryOp::Mul => f64::from(lhs) * f64::from(rhs),
        FloatBinaryOp::Add | FloatBinaryOp::Sub | FloatBinaryOp::ReverseSub => {
            unreachable!("handled above")
        }
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

fn lane4(bytes: &[u8; MAX_VECTOR_GROUP_BYTES], offset: usize) -> [u8; 4] {
    [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]
}
