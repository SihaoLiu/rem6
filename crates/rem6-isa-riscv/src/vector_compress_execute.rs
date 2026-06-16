use crate::{
    vector_group::{
        group_bytes_to_elements, memory_width_from_element_bytes, read_mask_bit,
        read_register_group, register_groups_overlap, valid_register_group,
        write_elements_to_group_bytes, write_register_group,
    },
    RiscvHartState, RiscvVectorCompressPlan, RiscvVectorConfig, RiscvVectorElements,
    RiscvVectorTailPolicy, VectorRegister,
};

pub(crate) fn execute_vector_compress_vm(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    let Some(group_registers) = config.register_group_registers() else {
        return false;
    };
    let Some(element_count) = RiscvVectorConfig::vlmax(config.vtype()) else {
        return false;
    };
    let Some(width) = memory_width_from_element_bytes(element_bytes) else {
        return false;
    };
    if !valid_register_group(vd, group_registers)
        || !valid_register_group(vs2, group_registers)
        || register_groups_overlap(vd, group_registers, vs2, group_registers)
        || register_groups_overlap(vd, group_registers, vs1, 1)
        || register_groups_overlap(vs2, group_registers, vs1, 1)
    {
        return false;
    }

    let mut destination_bytes = read_register_group(hart, vd, group_registers);
    let source_bytes = read_register_group(hart, vs2, group_registers);
    let mask = hart.read_vector(vs1);
    let element_count = element_count as usize;
    let Ok(destination) = RiscvVectorElements::new(
        width,
        group_bytes_to_elements(&destination_bytes, element_bytes, element_count),
    ) else {
        return false;
    };
    let Ok(source) = RiscvVectorElements::new(
        width,
        group_bytes_to_elements(&source_bytes, element_bytes, element_count),
    ) else {
        return false;
    };
    let mask = (0..element_count)
        .map(|element_index| read_mask_bit(&mask, element_index))
        .collect::<Vec<_>>();

    let Ok(result) = RiscvVectorCompressPlan::new(config.vl() as usize, vector_tail_policy(config))
        .execute(&destination, &source, &mask)
    else {
        return false;
    };
    write_elements_to_group_bytes(
        &mut destination_bytes,
        element_bytes,
        result.elements().as_slice(),
    );
    write_register_group(hart, vd, group_registers, &destination_bytes);
    true
}

fn vector_tail_policy(config: RiscvVectorConfig) -> RiscvVectorTailPolicy {
    if config.vtype() & (1 << 6) != 0 {
        RiscvVectorTailPolicy::AgnosticAllOnes
    } else {
        RiscvVectorTailPolicy::Undisturbed
    }
}
