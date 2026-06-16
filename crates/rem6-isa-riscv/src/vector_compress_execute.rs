use crate::{
    MemoryWidth, RiscvHartState, RiscvVectorCompressPlan, RiscvVectorConfig, RiscvVectorElements,
    RiscvVectorTailPolicy, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

const MAX_VECTOR_GROUP_REGISTERS: usize = 8;
const MAX_VECTOR_GROUP_BYTES: usize = RISCV_VECTOR_REGISTER_BYTES * MAX_VECTOR_GROUP_REGISTERS;

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

fn valid_register_group(register: VectorRegister, group_registers: usize) -> bool {
    let index = register.index() as usize;
    group_registers > 0
        && group_registers <= MAX_VECTOR_GROUP_REGISTERS
        && index.is_multiple_of(group_registers)
        && index + group_registers <= 32
}

fn register_groups_overlap(
    left: VectorRegister,
    left_registers: usize,
    right: VectorRegister,
    right_registers: usize,
) -> bool {
    let left_start = left.index() as usize;
    let right_start = right.index() as usize;
    left_start < right_start + right_registers && right_start < left_start + left_registers
}

fn read_register_group(
    hart: &RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
) -> [u8; MAX_VECTOR_GROUP_BYTES] {
    let mut bytes = [0; MAX_VECTOR_GROUP_BYTES];
    for group_index in 0..group_registers {
        let vector = hart.read_vector(vector_register_at(register, group_index));
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES].copy_from_slice(&vector);
    }
    bytes
}

fn write_register_group(
    hart: &mut RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
    bytes: &[u8; MAX_VECTOR_GROUP_BYTES],
) {
    for group_index in 0..group_registers {
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        let mut vector = [0; RISCV_VECTOR_REGISTER_BYTES];
        vector.copy_from_slice(&bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES]);
        hart.write_vector(vector_register_at(register, group_index), vector);
    }
}

fn vector_register_at(base: VectorRegister, group_index: usize) -> VectorRegister {
    VectorRegister::from_field(u32::from(base.index()) + group_index as u32)
}

fn memory_width_from_element_bytes(element_bytes: usize) -> Option<MemoryWidth> {
    match element_bytes {
        1 => Some(MemoryWidth::Byte),
        2 => Some(MemoryWidth::Halfword),
        4 => Some(MemoryWidth::Word),
        8 => Some(MemoryWidth::Doubleword),
        _ => None,
    }
}

fn vector_tail_policy(config: RiscvVectorConfig) -> RiscvVectorTailPolicy {
    if config.vtype() & (1 << 6) != 0 {
        RiscvVectorTailPolicy::AgnosticAllOnes
    } else {
        RiscvVectorTailPolicy::Undisturbed
    }
}

fn group_bytes_to_elements(
    bytes: &[u8; MAX_VECTOR_GROUP_BYTES],
    element_bytes: usize,
    element_count: usize,
) -> Vec<u64> {
    (0..element_count)
        .map(|element_index| {
            let offset = element_index * element_bytes;
            lane_bytes_to_u64(&bytes[offset..offset + element_bytes])
        })
        .collect()
}

fn write_elements_to_group_bytes(
    bytes: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    element_bytes: usize,
    elements: &[u64],
) {
    for (element_index, element) in elements.iter().copied().enumerate() {
        let offset = element_index * element_bytes;
        bytes[offset..offset + element_bytes]
            .copy_from_slice(&element.to_le_bytes()[..element_bytes]);
    }
}

fn lane_bytes_to_u64(bytes: &[u8]) -> u64 {
    let mut lane = [0; 8];
    lane[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(lane)
}

fn read_mask_bit(mask: &[u8; RISCV_VECTOR_REGISTER_BYTES], element_index: usize) -> bool {
    let byte_index = element_index / 8;
    let bit = 1_u8 << (element_index % 8);
    (mask[byte_index] & bit) != 0
}
