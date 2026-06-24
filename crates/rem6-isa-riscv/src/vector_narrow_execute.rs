use crate::{
    vector_group::{
        lane_bytes_to_u128, memory_width_from_element_bytes, read_register_group,
        register_groups_overlap, valid_register_group, write_register_group, write_u128_lane,
    },
    RiscvHartState, RiscvVectorConfig, RiscvVectorNarrowClipPlan, RiscvVectorNarrowInstruction,
    RiscvVectorNarrowOperation, VectorRegister,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorNarrowInstruction,
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
        || !valid_register_group(instruction.vd(), destination_registers)
        || !valid_register_group(instruction.vs2(), source_registers)
        || instruction
            .vs1()
            .is_some_and(|vs1| !valid_register_group(vs1, destination_registers))
        || !narrowing_overlap_allowed(
            instruction.vd(),
            destination_registers,
            instruction.vs2(),
            source_registers,
        )
    {
        return false;
    }

    let mut destination_bytes = read_register_group(hart, instruction.vd(), destination_registers);
    let source_bytes = read_register_group(hart, instruction.vs2(), source_registers);
    let shift_bytes = instruction
        .vs1()
        .map(|vs1| read_register_group(hart, vs1, destination_registers));
    let mut fixed = hart.vector_fixed_point();

    for element_index in 0..vl {
        let source_offset = element_index * source_element_bytes;
        let destination_offset = element_index * element_bytes;
        let source =
            lane_bytes_to_u128(&source_bytes[source_offset..source_offset + source_element_bytes]);
        let shift = narrow_shift_amount(
            shift_value(
                instruction,
                shift_bytes.as_ref().map(|bytes| bytes.as_slice()),
                element_index,
                element_bytes,
            ),
            source_element_bytes,
        );
        let value = match instruction.operation() {
            RiscvVectorNarrowOperation::ShiftRightLogical => source >> shift,
            RiscvVectorNarrowOperation::ShiftRightArithmetic => {
                (sign_extend(source, source_element_bytes * 8) >> shift) as u128
            }
            RiscvVectorNarrowOperation::ClipUnsigned => {
                let plan = RiscvVectorNarrowClipPlan::unsigned(width);
                let Ok(result) = plan.execute_unsigned(source, shift, fixed.rounding_mode()) else {
                    return false;
                };
                fixed.apply_narrow_clip_result(result);
                result.value() as u128
            }
            RiscvVectorNarrowOperation::ClipSigned => {
                let plan = RiscvVectorNarrowClipPlan::signed(width);
                let Ok(result) = plan.execute_signed(
                    sign_extend(source, source_element_bytes * 8),
                    shift,
                    fixed.rounding_mode(),
                ) else {
                    return false;
                };
                fixed.apply_narrow_clip_result(result);
                result.value() as u128
            }
        };
        write_u128_lane(
            &mut destination_bytes[destination_offset..destination_offset + element_bytes],
            value,
        );
    }

    hart.set_vector_fixed_point(fixed);
    write_register_group(
        hart,
        instruction.vd(),
        destination_registers,
        &destination_bytes,
    );
    true
}

fn sign_extend(value: u128, bits: usize) -> i128 {
    let shift = 128 - bits;
    ((value << shift) as i128) >> shift
}

fn shift_value(
    instruction: RiscvVectorNarrowInstruction,
    shift_bytes: Option<&[u8]>,
    element_index: usize,
    element_bytes: usize,
) -> u128 {
    if let Some(shift) = instruction.immediate_shift() {
        return u128::from(shift);
    }
    let offset = element_index * element_bytes;
    lane_bytes_to_u128(
        &shift_bytes.expect("validated vector narrow shift source")[offset..offset + element_bytes],
    )
}

fn narrow_shift_amount(shift: u128, source_element_bytes: usize) -> u32 {
    let source_bits = (source_element_bytes * 8) as u32;
    (shift & u128::from(source_bits - 1)) as u32
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
