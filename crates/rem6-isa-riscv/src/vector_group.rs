use crate::{MemoryWidth, RiscvHartState, VectorRegister, RISCV_VECTOR_REGISTER_BYTES};

pub(crate) const MAX_VECTOR_GROUP_REGISTERS: usize = 8;
pub(crate) const MAX_VECTOR_GROUP_BYTES: usize =
    RISCV_VECTOR_REGISTER_BYTES * MAX_VECTOR_GROUP_REGISTERS;

pub(crate) fn valid_register_group(register: VectorRegister, group_registers: usize) -> bool {
    let index = register.index() as usize;
    group_registers > 0
        && group_registers <= MAX_VECTOR_GROUP_REGISTERS
        && index.is_multiple_of(group_registers)
        && index + group_registers <= 32
}

pub(crate) struct VectorBinaryPlan {
    pub(crate) element_bytes: usize,
    pub(crate) group_registers: usize,
    pub(crate) active_bytes: usize,
}

impl VectorBinaryPlan {
    pub(crate) fn new(
        hart: &RiscvHartState,
        destination: VectorRegister,
        sources: &[VectorRegister],
    ) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let group_registers = config.register_group_registers()?;
        if !valid_register_group(destination, group_registers)
            || sources
                .iter()
                .any(|source| !valid_register_group(*source, group_registers))
        {
            return None;
        }

        let active_bytes = (config.vl() as usize).checked_mul(element_bytes)?;
        if active_bytes > group_registers * RISCV_VECTOR_REGISTER_BYTES {
            return None;
        }

        Some(Self {
            element_bytes,
            group_registers,
            active_bytes,
        })
    }

    pub(crate) fn active_element_count(&self) -> usize {
        self.active_bytes / self.element_bytes
    }
}

pub(crate) fn register_groups_overlap(
    left: VectorRegister,
    left_registers: usize,
    right: VectorRegister,
    right_registers: usize,
) -> bool {
    let left_start = left.index() as usize;
    let right_start = right.index() as usize;
    left_start < right_start + right_registers && right_start < left_start + left_registers
}

pub(crate) fn read_register_group(
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

pub(crate) fn write_register_group(
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

pub(crate) fn vector_register_at(base: VectorRegister, group_index: usize) -> VectorRegister {
    VectorRegister::from_field(u32::from(base.index()) + group_index as u32)
}

pub(crate) fn memory_width_from_element_bytes(element_bytes: usize) -> Option<MemoryWidth> {
    match element_bytes {
        1 => Some(MemoryWidth::Byte),
        2 => Some(MemoryWidth::Halfword),
        4 => Some(MemoryWidth::Word),
        8 => Some(MemoryWidth::Doubleword),
        _ => None,
    }
}

pub(crate) fn group_bytes_to_elements(
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

pub(crate) fn write_elements_to_group_bytes(
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

pub(crate) fn lane_bytes_to_u64(bytes: &[u8]) -> u64 {
    let mut lane = [0; 8];
    lane[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(lane)
}

pub(crate) fn lane_bytes_to_u128(bytes: &[u8]) -> u128 {
    let mut lane = [0; 16];
    lane[..bytes.len()].copy_from_slice(bytes);
    u128::from_le_bytes(lane)
}

pub(crate) fn write_u128_lane(bytes: &mut [u8], value: u128) {
    bytes.copy_from_slice(&value.to_le_bytes()[..bytes.len()]);
}

pub(crate) fn read_mask_bit(
    mask: &[u8; RISCV_VECTOR_REGISTER_BYTES],
    element_index: usize,
) -> bool {
    let byte_index = element_index / 8;
    let bit = 1_u8 << (element_index % 8);
    (mask[byte_index] & bit) != 0
}
