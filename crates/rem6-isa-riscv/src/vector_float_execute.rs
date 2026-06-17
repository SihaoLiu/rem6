use crate::{
    vector_group::{
        read_register_group, write_register_group, VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    RiscvHartState, RiscvVectorFloatInstruction,
};

pub(crate) fn execute(hart: &mut RiscvHartState, instruction: RiscvVectorFloatInstruction) -> bool {
    if !float_rounding_mode_is_valid(hart.float_status().frm()) {
        return false;
    }

    match instruction {
        RiscvVectorFloatInstruction::AddVv { vd, vs1, vs2 } => {
            let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
                return false;
            };
            let left = read_register_group(hart, vs2, plan.group_registers);
            let right = read_register_group(hart, vs1, plan.group_registers);
            let mut result = read_register_group(hart, vd, plan.group_registers);
            if !apply_exact_add_lanes(&plan, &mut result, &left, &right) {
                return false;
            }
            write_register_group(hart, vd, plan.group_registers, &result);
            true
        }
    }
}

fn float_rounding_mode_is_valid(frm: u64) -> bool {
    frm <= 4
}

fn apply_exact_add_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right: &[u8; MAX_VECTOR_GROUP_BYTES],
) -> bool {
    if plan.element_bytes != 4 {
        return false;
    }

    for offset in (0..plan.active_bytes).step_by(4) {
        let lhs = f32::from_bits(u32::from_le_bytes(lane4(left, offset)));
        let rhs = f32::from_bits(u32::from_le_bytes(lane4(right, offset)));
        let Some(sum) = exact_f32_add(lhs, rhs) else {
            return false;
        };
        result[offset..offset + 4].copy_from_slice(&sum.to_bits().to_le_bytes());
    }
    true
}

fn exact_f32_add(lhs: f32, rhs: f32) -> Option<f32> {
    if !lhs.is_finite() || !rhs.is_finite() {
        return None;
    }
    let exact = f64::from(lhs) + f64::from(rhs);
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
