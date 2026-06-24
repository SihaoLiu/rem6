use crate::{
    vector_group::{
        lane_bytes_to_u64, read_register_group, write_register_group, VectorBinaryPlan,
        MAX_VECTOR_GROUP_BYTES,
    },
    RiscvHartState, RiscvVectorConfig, RiscvVectorGatherInstruction, VectorRegister,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorGatherInstruction,
) -> bool {
    match instruction {
        RiscvVectorGatherInstruction::Vv { vd, vs2, vs1 } => {
            let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
                return false;
            };
            let Some(vlmax) = RiscvVectorConfig::vlmax(hart.vector_config().vtype()) else {
                return false;
            };
            let source = read_register_group(hart, vs2, plan.group_registers);
            let indices = read_register_group(hart, vs1, plan.group_registers);
            let mut result = read_register_group(hart, vd, plan.group_registers);
            apply_vector_gather(&plan, &mut result, &source, &indices, vlmax as usize);
            write_register_group(hart, vd, plan.group_registers, &result);
            true
        }
        RiscvVectorGatherInstruction::Vx { vd, vs2, rs1 } => {
            execute_scalar_gather(hart, vd, vs2, hart.read(rs1))
        }
        RiscvVectorGatherInstruction::Vi { vd, vs2, index } => {
            execute_scalar_gather(hart, vd, vs2, u64::from(index))
        }
    }
}

fn execute_scalar_gather(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    index: u64,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let Some(vlmax) = RiscvVectorConfig::vlmax(hart.vector_config().vtype()) else {
        return false;
    };

    let source = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_scalar_gather(&plan, &mut result, &source, index, vlmax as usize);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn apply_vector_gather(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
    indices: &[u8; MAX_VECTOR_GROUP_BYTES],
    vlmax: usize,
) {
    for element_index in 0..plan.active_element_count() {
        let index = read_lane(plan, indices, element_index);
        write_gathered_lane(plan, result, element_index, source, index, vlmax);
    }
}

fn apply_scalar_gather(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
    index: u64,
    vlmax: usize,
) {
    for element_index in 0..plan.active_element_count() {
        write_gathered_lane(plan, result, element_index, source, index, vlmax);
    }
}

fn write_gathered_lane(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    destination_index: usize,
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
    source_index: u64,
    vlmax: usize,
) {
    if let Ok(source_index) = usize::try_from(source_index) {
        if source_index < vlmax {
            copy_lane(plan, result, destination_index, source, source_index);
            return;
        }
    }
    zero_lane(plan, result, destination_index);
}

fn read_lane(
    plan: &VectorBinaryPlan,
    bytes: &[u8; MAX_VECTOR_GROUP_BYTES],
    element_index: usize,
) -> u64 {
    let offset = element_index * plan.element_bytes;
    lane_bytes_to_u64(&bytes[offset..offset + plan.element_bytes])
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
