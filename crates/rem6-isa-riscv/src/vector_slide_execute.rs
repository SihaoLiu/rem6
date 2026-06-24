use crate::{
    vector_group::{
        read_register_group, register_groups_overlap, write_register_group, VectorBinaryPlan,
        MAX_VECTOR_GROUP_BYTES,
    },
    RiscvHartState, RiscvVectorConfig, RiscvVectorSlideInstruction, VectorRegister,
};

pub(crate) fn execute(hart: &mut RiscvHartState, instruction: RiscvVectorSlideInstruction) -> bool {
    match instruction {
        RiscvVectorSlideInstruction::UpVx { vd, vs2, rs1 } => {
            execute_slide_up(hart, vd, vs2, scalar_offset(hart.read(rs1)))
        }
        RiscvVectorSlideInstruction::DownVx { vd, vs2, rs1 } => {
            execute_slide_down(hart, vd, vs2, scalar_offset(hart.read(rs1)))
        }
        RiscvVectorSlideInstruction::UpVi { vd, vs2, offset } => {
            execute_slide_up(hart, vd, vs2, usize::from(offset))
        }
        RiscvVectorSlideInstruction::DownVi { vd, vs2, offset } => {
            execute_slide_down(hart, vd, vs2, usize::from(offset))
        }
    }
}

fn execute_slide_up(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    offset: usize,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if register_groups_overlap(vd, plan.group_registers, vs2, plan.group_registers) {
        return false;
    }

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_slide_up(&plan, &mut result, &source, offset);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_slide_down(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    offset: usize,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let Some(vlmax) = RiscvVectorConfig::vlmax(hart.vector_config().vtype()) else {
        return false;
    };

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_slide_down(&plan, &mut result, &source, offset, vlmax as usize);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn apply_slide_up(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
    offset: usize,
) {
    for element_index in offset..plan.active_element_count() {
        let source_index = element_index - offset;
        copy_lane(plan, result, element_index, source, source_index);
    }
}

fn apply_slide_down(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
    offset: usize,
    vlmax: usize,
) {
    for element_index in 0..plan.active_element_count() {
        if let Some(source_index) = element_index
            .checked_add(offset)
            .filter(|source_index| *source_index < vlmax)
        {
            copy_lane(plan, result, element_index, source, source_index);
        } else {
            zero_lane(plan, result, element_index);
        }
    }
}

fn copy_lane(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    destination_index: usize,
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
    source_index: usize,
) {
    let destination_offset = destination_index * plan.element_bytes;
    let source_offset = source_index * plan.element_bytes;
    result[destination_offset..destination_offset + plan.element_bytes]
        .copy_from_slice(&source[source_offset..source_offset + plan.element_bytes]);
}

fn zero_lane(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    element_index: usize,
) {
    let offset = element_index * plan.element_bytes;
    result[offset..offset + plan.element_bytes].fill(0);
}

fn scalar_offset(offset: u64) -> usize {
    usize::try_from(offset).unwrap_or(usize::MAX)
}
