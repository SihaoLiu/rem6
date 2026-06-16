use crate::{RiscvHartState, VectorRegister, RISCV_VECTOR_REGISTER_BYTES};

const MAX_VECTOR_GROUP_REGISTERS: usize = 8;
const MAX_VECTOR_GROUP_BYTES: usize = RISCV_VECTOR_REGISTER_BYTES * MAX_VECTOR_GROUP_REGISTERS;

pub(crate) fn execute_vector_add_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    let Some(group_registers) = config.register_group_registers() else {
        return false;
    };
    if !valid_register_group(vd, group_registers)
        || !valid_register_group(vs1, group_registers)
        || !valid_register_group(vs2, group_registers)
    {
        return false;
    }

    let Some(active_bytes) = (config.vl() as usize).checked_mul(element_bytes) else {
        return false;
    };
    if active_bytes > group_registers * RISCV_VECTOR_REGISTER_BYTES {
        return false;
    }

    let left = read_register_group(hart, vs1, group_registers);
    let right = read_register_group(hart, vs2, group_registers);
    let mut result = read_register_group(hart, vd, group_registers);
    for offset in (0..active_bytes).step_by(element_bytes) {
        add_lane(
            &mut result[offset..offset + element_bytes],
            &left[offset..offset + element_bytes],
            &right[offset..offset + element_bytes],
        );
    }
    write_register_group(hart, vd, group_registers, &result);
    true
}

fn valid_register_group(register: VectorRegister, group_registers: usize) -> bool {
    let index = register.index() as usize;
    group_registers > 0
        && group_registers <= MAX_VECTOR_GROUP_REGISTERS
        && index.is_multiple_of(group_registers)
        && index + group_registers <= 32
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

fn add_lane(result: &mut [u8], left: &[u8], right: &[u8]) {
    match result.len() {
        1 => result[0] = left[0].wrapping_add(right[0]),
        2 => result.copy_from_slice(
            &u16::from_le_bytes([left[0], left[1]])
                .wrapping_add(u16::from_le_bytes([right[0], right[1]]))
                .to_le_bytes(),
        ),
        4 => result.copy_from_slice(
            &u32::from_le_bytes([left[0], left[1], left[2], left[3]])
                .wrapping_add(u32::from_le_bytes([right[0], right[1], right[2], right[3]]))
                .to_le_bytes(),
        ),
        8 => result.copy_from_slice(
            &u64::from_le_bytes([
                left[0], left[1], left[2], left[3], left[4], left[5], left[6], left[7],
            ])
            .wrapping_add(u64::from_le_bytes([
                right[0], right[1], right[2], right[3], right[4], right[5], right[6], right[7],
            ]))
            .to_le_bytes(),
        ),
        _ => unreachable!("validated vector element width"),
    }
}
