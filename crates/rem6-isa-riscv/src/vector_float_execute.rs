use crate::{
    float,
    vector_group::{
        read_register_group, write_register_group, VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    FloatRegister, RiscvFloatRoundingMode, RiscvHartState, RiscvVectorFloatInstruction,
    VectorRegister,
};

pub(crate) fn execute(hart: &mut RiscvHartState, instruction: RiscvVectorFloatInstruction) -> bool {
    let Some(rounding_mode) =
        RiscvFloatRoundingMode::from_frm_bits(hart.float_status().frm() as u8)
    else {
        return false;
    };

    match instruction {
        RiscvVectorFloatInstruction::AddVv { vd, vs1, vs2 } => {
            execute_binary_vv(hart, vd, vs1, vs2, FloatBinaryOp::Add, rounding_mode)
        }
        RiscvVectorFloatInstruction::AddVf { vd, fs1, vs2 } => {
            execute_binary_vf(hart, vd, fs1, vs2, FloatBinaryOp::Add, rounding_mode)
        }
        RiscvVectorFloatInstruction::SubVv { vd, vs1, vs2 } => {
            execute_binary_vv(hart, vd, vs1, vs2, FloatBinaryOp::Sub, rounding_mode)
        }
        RiscvVectorFloatInstruction::MulVv { vd, vs1, vs2 } => {
            execute_binary_vv(hart, vd, vs1, vs2, FloatBinaryOp::Mul, rounding_mode)
        }
    }
}

#[derive(Clone, Copy)]
enum FloatBinaryOp {
    Add,
    Sub,
    Mul,
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
        let Some(value) = exact_f32_binary(lhs, scalar, operation, rounding_mode) else {
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
    if matches!(operation, FloatBinaryOp::Add | FloatBinaryOp::Sub) {
        let bits = float::exact_finite_single_add_sub_bits(
            lhs.to_bits(),
            rhs.to_bits(),
            rounding_mode,
            matches!(operation, FloatBinaryOp::Sub),
        )?;
        return Some(f32::from_bits(bits));
    }
    let exact = match operation {
        FloatBinaryOp::Mul => f64::from(lhs) * f64::from(rhs),
        FloatBinaryOp::Add | FloatBinaryOp::Sub => unreachable!("handled above"),
    };
    let rounded = exact as f32;
    (rounded.is_finite() && f64::from(rounded) == exact).then_some(rounded)
}

fn lane4(bytes: &[u8; MAX_VECTOR_GROUP_BYTES], offset: usize) -> [u8; 4] {
    [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]
}
