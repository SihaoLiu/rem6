use crate::{
    vector_group::{
        lane_bytes_to_u128, memory_width_from_element_bytes, read_register_group,
        register_groups_overlap, valid_register_group, write_register_group, write_u128_lane,
    },
    RiscvHartState, RiscvVectorConfig, RiscvVectorNarrowClipPlan, VectorRegister,
};

pub(crate) fn execute_vector_narrow_clip_unsigned_wi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    shift: u8,
) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    let Some(destination_registers) = config.register_group_registers() else {
        return false;
    };
    let Some(source_registers) = widening_source_registers(config.vtype()) else {
        return false;
    };
    let source_element_bytes = element_bytes.saturating_mul(2);
    let Some(width) = memory_width_from_element_bytes(element_bytes) else {
        return false;
    };
    let Some(vlmax) = RiscvVectorConfig::vlmax(config.vtype()) else {
        return false;
    };
    let vl = config.vl() as usize;
    if vl > vlmax as usize
        || source_element_bytes > 16
        || !valid_register_group(vd, destination_registers)
        || !valid_register_group(vs2, source_registers)
        || !narrowing_overlap_allowed(vd, destination_registers, vs2, source_registers)
    {
        return false;
    }

    let mut destination_bytes = read_register_group(hart, vd, destination_registers);
    let source_bytes = read_register_group(hart, vs2, source_registers);
    let plan = RiscvVectorNarrowClipPlan::unsigned(width);
    let mut fixed = hart.vector_fixed_point();

    for element_index in 0..vl {
        let source_offset = element_index * source_element_bytes;
        let destination_offset = element_index * element_bytes;
        let source =
            lane_bytes_to_u128(&source_bytes[source_offset..source_offset + source_element_bytes]);
        let Ok(result) = plan.execute_unsigned(source, u32::from(shift), fixed.rounding_mode())
        else {
            return false;
        };
        fixed.apply_narrow_clip_result(result);
        write_u128_lane(
            &mut destination_bytes[destination_offset..destination_offset + element_bytes],
            result.value() as u128,
        );
    }

    hart.set_vector_fixed_point(fixed);
    write_register_group(hart, vd, destination_registers, &destination_bytes);
    true
}

fn widening_source_registers(vtype: u64) -> Option<usize> {
    match vtype & 0x7 {
        0 => Some(2),
        1 => Some(4),
        2 => Some(8),
        5..=7 => Some(1),
        _ => None,
    }
}

fn narrowing_overlap_allowed(
    vd: VectorRegister,
    destination_registers: usize,
    vs2: VectorRegister,
    source_registers: usize,
) -> bool {
    !register_groups_overlap(vd, destination_registers, vs2, source_registers)
        || vd.index() == vs2.index()
}
