use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvHartState, RiscvInstruction, RiscvTrap,
    RiscvTrapKind, RiscvVectorConfig, RiscvVectorMaskMode, RiscvVectorMemoryInstruction,
    VectorRegister,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_store_type(vm_unmasked: bool, width: u32, rs1: u8, vs3: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn vector_strided_load_type(vm_unmasked: bool, width: u32, rs1: u8, rs2: u8, vd: u8) -> u32 {
    (0b10 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_strided_store_type(vm_unmasked: bool, width: u32, rs1: u8, rs2: u8, vs3: u8) -> u32 {
    (0b10 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn lanes_u32(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn lanes_u32x8(lanes: [u32; 8]) -> [u8; 32] {
    let mut bytes = [0; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn lanes_u32x16(lanes: [u32; 16]) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn lanes_u32x32(lanes: [u32; 32]) -> [u8; 128] {
    let mut bytes = [0; 128];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn e32_byte_mask(active_lanes: [bool; 4]) -> Vec<bool> {
    element_byte_mask(&active_lanes, 4)
}

fn element_byte_mask(active_lanes: &[bool], element_bytes: usize) -> Vec<bool> {
    active_lanes
        .iter()
        .copied()
        .flat_map(|active| vec![active; element_bytes])
        .collect()
}

#[test]
fn decoder_accepts_masked_vector_unit_stride_memory() {
    assert_eq!(
        vector_unit_stride_load_type(false, 0b110, 14, 2),
        0x0007_6107
    );
    assert_eq!(
        RiscvInstruction::decode(vector_unit_stride_load_type(false, 0b110, 14, 2)).unwrap(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride {
            vd: vreg(2),
            rs1: reg(14),
            width: MemoryWidth::Word,
            mask: RiscvVectorMaskMode::Masked,
        })
    );

    assert_eq!(
        RiscvInstruction::decode(vector_unit_stride_store_type(false, 0b110, 16, 2)).unwrap(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::StoreUnitStride {
            vs3: vreg(2),
            rs1: reg(16),
            width: MemoryWidth::Word,
            mask: RiscvVectorMaskMode::Masked,
        })
    );
}

#[test]
fn decoder_accepts_unmasked_vector_strided_memory() {
    assert_eq!(
        RiscvInstruction::decode(vector_strided_load_type(true, 0b110, 14, 12, 1)).unwrap(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadStrided {
            vd: vreg(1),
            rs1: reg(14),
            rs2: reg(12),
            width: MemoryWidth::Word,
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vector_strided_store_type(true, 0b110, 16, 12, 1)).unwrap(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::StoreStrided {
            vs3: vreg(1),
            rs1: reg(16),
            rs2: reg(12),
            width: MemoryWidth::Word,
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
}

#[test]
fn hart_builds_masked_unit_stride_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9010);
    hart.write_vector(
        vreg(0),
        [0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    hart.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_load_type(false, 0b110, 14, 2)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadUnitStride {
            vd: vreg(2),
            address: 0x9000,
            width: MemoryWidth::Word,
            byte_len: 16,
            byte_mask: Some(e32_byte_mask([true, false, true, false])),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_store_type(false, 0b110, 16, 2)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreUnitStride {
            address: 0x9010,
            width: MemoryWidth::Word,
            data: lanes_u32([1, 2, 3, 4]).to_vec(),
            byte_mask: Some(e32_byte_mask([true, false, true, false])),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_unmasked_strided_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8080);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(12), 12);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, 0b110, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            stride: 12,
            element_count: 2,
            span_len: 16,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(true, 0b110, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..4].copy_from_slice(&source[0..4]);
    data[12..16].copy_from_slice(&source[4..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Word,
            stride: 12,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(
                &[
                    true, true, true, true, false, false, false, false, false, false, false, false,
                    true, true, true, true,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_rejects_strided_vector_memory_outside_supported_slice() {
    assert_strided_memory_trap(8, 2);
    assert_strided_memory_trap(12, 3);
}

#[test]
fn hart_builds_masked_unit_stride_vector_memory_masks_for_all_m1_element_widths() {
    for (vtype, width_bits, width, active_lanes, expected_bytes) in [
        (
            0xc0,
            0b000,
            MemoryWidth::Byte,
            vec![true, false, true, false, true, false, true, false],
            8,
        ),
        (
            0xc8,
            0b101,
            MemoryWidth::Halfword,
            vec![true, false, true, false],
            8,
        ),
        (0xd8, 0b111, MemoryWidth::Doubleword, vec![true, false], 16),
    ] {
        let mut hart = RiscvHartState::new(0x8080);
        hart.set_vector_config(RiscvVectorConfig::new(active_lanes.len() as u32, vtype));
        hart.write(reg(14), 0x9000);
        hart.write_vector(
            vreg(0),
            [0b0101_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );

        let load = hart
            .execute(
                RiscvInstruction::decode(vector_unit_stride_load_type(false, width_bits, 14, 2))
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(
            load.memory_access(),
            Some(&MemoryAccessKind::VectorLoadUnitStride {
                vd: vreg(2),
                address: 0x9000,
                width,
                byte_len: expected_bytes,
                byte_mask: Some(element_byte_mask(&active_lanes, width.bytes())),
                group_registers: 1,
            })
        );
    }
}

#[test]
fn hart_builds_masked_unit_stride_vector_memory_masks_for_e32_m2_register_group() {
    let mut hart = RiscvHartState::new(0x80a0);
    hart.set_vector_config(RiscvVectorConfig::new(8, 0xd1));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0101_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let source = lanes_u32x8([1, 2, 3, 4, 5, 6, 7, 8]);
    hart.write_vector(vreg(2), source[..16].try_into().unwrap());
    hart.write_vector(vreg(3), source[16..].try_into().unwrap());
    let byte_mask = element_byte_mask(&[true, false, true, false, true, false, true, false], 4);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_load_type(false, 0b110, 14, 2)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadUnitStride {
            vd: vreg(2),
            address: 0x9000,
            width: MemoryWidth::Word,
            byte_len: 32,
            byte_mask: Some(byte_mask.clone()),
            group_registers: 2,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_store_type(false, 0b110, 16, 2)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreUnitStride {
            address: 0x9020,
            width: MemoryWidth::Word,
            data: source.to_vec(),
            byte_mask: Some(byte_mask),
            group_registers: 2,
        })
    );
}

#[test]
fn hart_builds_masked_unit_stride_vector_memory_masks_for_e32_m4_register_group() {
    let mut hart = RiscvHartState::new(0x80c0);
    hart.set_vector_config(RiscvVectorConfig::new(16, 0xd2));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9040);
    hart.write_vector(
        vreg(0),
        [
            0b0101_0101,
            0b0101_0101,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    );

    let source = lanes_u32x16([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    for group_index in 0..4 {
        let offset = group_index * 16;
        hart.write_vector(
            vreg(4 + group_index as u8),
            source[offset..offset + 16].try_into().unwrap(),
        );
    }
    let byte_mask = element_byte_mask(
        &[
            true, false, true, false, true, false, true, false, true, false, true, false, true,
            false, true, false,
        ],
        4,
    );

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_load_type(false, 0b110, 14, 4)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadUnitStride {
            vd: vreg(4),
            address: 0x9000,
            width: MemoryWidth::Word,
            byte_len: 64,
            byte_mask: Some(byte_mask.clone()),
            group_registers: 4,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_store_type(false, 0b110, 16, 4)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreUnitStride {
            address: 0x9040,
            width: MemoryWidth::Word,
            data: source.to_vec(),
            byte_mask: Some(byte_mask),
            group_registers: 4,
        })
    );
}

#[test]
fn hart_builds_masked_unit_stride_vector_memory_masks_for_e32_m8_register_group() {
    let mut hart = RiscvHartState::new(0x80e0);
    hart.set_vector_config(RiscvVectorConfig::new(32, 0xd3));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9080);
    hart.write_vector(
        vreg(0),
        [
            0b0101_0101,
            0b0101_0101,
            0b0101_0101,
            0b0101_0101,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    );

    let source = lanes_u32x32([
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ]);
    for group_index in 0..8 {
        let offset = group_index * 16;
        hart.write_vector(
            vreg(16 + group_index as u8),
            source[offset..offset + 16].try_into().unwrap(),
        );
    }
    let byte_mask = element_byte_mask(
        &[
            true, false, true, false, true, false, true, false, true, false, true, false, true,
            false, true, false, true, false, true, false, true, false, true, false, true, false,
            true, false, true, false, true, false,
        ],
        4,
    );

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_load_type(false, 0b110, 14, 16)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadUnitStride {
            vd: vreg(16),
            address: 0x9000,
            width: MemoryWidth::Word,
            byte_len: 128,
            byte_mask: Some(byte_mask.clone()),
            group_registers: 8,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_unit_stride_store_type(false, 0b110, 16, 16)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreUnitStride {
            address: 0x9080,
            width: MemoryWidth::Word,
            data: source.to_vec(),
            byte_mask: Some(byte_mask),
            group_registers: 8,
        })
    );
}

#[test]
fn hart_rejects_masked_unit_stride_vector_memory_outside_supported_lmul_slices() {
    assert_masked_memory_trap(0xd1, vector_unit_stride_load_type(false, 0b110, 14, 2)); // partial e32,m2
    assert_masked_memory_trap(0xd1, vector_unit_stride_store_type(false, 0b110, 14, 2));
    assert_masked_memory_trap(0xd2, vector_unit_stride_load_type(false, 0b110, 14, 4)); // e32,m4
    assert_masked_memory_trap(0xd2, vector_unit_stride_store_type(false, 0b110, 14, 4));
    assert_masked_memory_trap(0xd3, vector_unit_stride_load_type(false, 0b110, 14, 8)); // partial e32,m8
    assert_masked_memory_trap(0xd3, vector_unit_stride_store_type(false, 0b110, 14, 8));
}

#[test]
fn hart_rejects_masked_unit_stride_load_when_destination_overlaps_v0() {
    assert_masked_memory_trap(0xd0, vector_unit_stride_load_type(false, 0b110, 14, 0));
}

fn assert_masked_memory_trap(vtype: u64, raw: u32) {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_vector_config(RiscvVectorConfig::new(4, vtype));
    hart.write(reg(14), 0x9000);
    hart.write_vector(
        vreg(0),
        [0b0000_1111, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    hart.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));

    let record = hart
        .execute(RiscvInstruction::decode(raw).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8100))
    );
    assert_eq!(record.memory_access(), None);
}

fn assert_strided_memory_trap(stride: u64, vl: u32) {
    let mut hart = RiscvHartState::new(0x8140);
    hart.set_vector_config(RiscvVectorConfig::new(vl, 0xd0));
    hart.write(reg(12), stride);
    hart.write(reg(14), 0x9000);

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, 0b110, 14, 12, 1)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8140))
    );
    assert_eq!(record.memory_access(), None);
}
