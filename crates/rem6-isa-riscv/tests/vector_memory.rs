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

fn vector_indexed_unordered_load_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs2: u8,
    vd: u8,
) -> u32 {
    (0b01 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_indexed_unordered_store_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs2: u8,
    vs3: u8,
) -> u32 {
    (0b01 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn lanes_u16(lanes: [u16; 8]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 2..index * 2 + 2].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn lanes_u32(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn lanes_u64(lanes: [u64; 2]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&lane.to_le_bytes());
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
fn decoder_accepts_unmasked_vector_indexed_memory() {
    assert_eq!(
        RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
            .unwrap(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadIndexedUnordered {
            vd: vreg(1),
            rs1: reg(14),
            vs2: vreg(2),
            index_width: MemoryWidth::Word,
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
            .unwrap(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::StoreIndexedUnordered {
            vs3: vreg(1),
            rs1: reg(16),
            vs2: vreg(2),
            index_width: MemoryWidth::Word,
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
fn hart_builds_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8260);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 4, 0, 0]));
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 4],
            span_len: 8,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 8];
    data[0..4].copy_from_slice(&source[0..4]);
    data[4..8].copy_from_slice(&source[4..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 4],
            data,
            byte_mask: element_byte_mask(&[true, true, true, true, true, true, true, true,], 1,),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_sparse_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8270);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 12, 0, 0]));
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 12],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..4].copy_from_slice(&source[0..4]);
    data[12..16].copy_from_slice(&source[4..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 12],
            data,
            byte_mask: element_byte_mask(&[true, false, false, true], 4),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_leading_gap_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8290);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([4, 12, 0, 0]));
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![4, 12],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[4..8].copy_from_slice(&source[0..4]);
    data[12..16].copy_from_slice(&source[4..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![4, 12],
            data,
            byte_mask: element_byte_mask(&[false, true, false, true], 4),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_reversed_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x83c0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([12, 0, 0, 0]));
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![12, 0],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[12..16].copy_from_slice(&source[0..4]);
    data[0..4].copy_from_slice(&source[4..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![12, 0],
            data,
            byte_mask: element_byte_mask(&[true, false, false, true], 4),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_indexed_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8340);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 8]));
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 8],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 8],
            data: source.to_vec(),
            byte_mask: element_byte_mask(&[true; 16], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_sparse_indexed_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8350);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 24]));
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 24],
            span_len: 32,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 32];
    data[0..8].copy_from_slice(&source[0..8]);
    data[24..32].copy_from_slice(&source[8..16]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 24],
            data,
            byte_mask: element_byte_mask(&[true, false, false, true], 8),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_indexed_e8_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8420);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), [0, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Byte,
            offsets: vec![0, 15],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0] = source[0];
    data[15] = source[1];
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Byte,
            offsets: vec![0, 15],
            data,
            byte_mask: element_byte_mask(
                &[
                    true, false, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, true,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_indexed_e8_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8430);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), [0, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b000, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Byte,
            offsets: vec![0, 15],
            span_len: 1,
            byte_mask: Some(element_byte_mask(&[true, false], 1)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b000, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Byte,
            offsets: vec![0, 15],
            data: source[0..1].to_vec(),
            byte_mask: element_byte_mask(&[true], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8460);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u16([0, 15, 0, 0, 0, 0, 0, 0]));
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 15],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0] = source[0];
    data[15] = source[1];
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 15],
            data,
            byte_mask: element_byte_mask(
                &[
                    true, false, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, true,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8470);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u16([0, 15, 0, 0, 0, 0, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b101, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 15],
            span_len: 1,
            byte_mask: Some(element_byte_mask(&[true, false], 1)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b101, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 15],
            data: source[0..1].to_vec(),
            byte_mask: element_byte_mask(&[true], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8480);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 15, 0, 0]));
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 15],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0] = source[0];
    data[15] = source[1];
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 15],
            data,
            byte_mask: element_byte_mask(
                &[
                    true, false, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, true,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8490);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 15, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 15],
            span_len: 1,
            byte_mask: Some(element_byte_mask(&[true, false], 1)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Byte,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 15],
            data: source[0..1].to_vec(),
            byte_mask: element_byte_mask(&[true], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_indexed_e16_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8400);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u16([0, 14, 0, 0, 0, 0, 0, 0]));
    let source = lanes_u16([0xa1a2, 0xb1b2, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 14],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..2].copy_from_slice(&source[0..2]);
    data[14..16].copy_from_slice(&source[2..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 14],
            data,
            byte_mask: element_byte_mask(
                &[true, false, false, false, false, false, false, true],
                2,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_indexed_e16_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8410);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u16([0, 14, 0, 0, 0, 0, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u16([0xa1a2, 0xb1b2, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b101, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 14],
            span_len: 2,
            byte_mask: Some(element_byte_mask(&[true, false], 2)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b101, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Halfword,
            offsets: vec![0, 14],
            data: source[0..2].to_vec(),
            byte_mask: element_byte_mask(&[true, true], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8440);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 14, 0, 0]));
    let source = lanes_u16([0xa1a2, 0xb1b2, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 14],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..2].copy_from_slice(&source[0..2]);
    data[14..16].copy_from_slice(&source[2..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 14],
            data,
            byte_mask: element_byte_mask(
                &[true, false, false, false, false, false, false, true],
                2,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8450);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 14, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u16([0xa1a2, 0xb1b2, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 14],
            span_len: 2,
            byte_mask: Some(element_byte_mask(&[true, false], 2)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Halfword,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 14],
            data: source[0..2].to_vec(),
            byte_mask: element_byte_mask(&[true, true], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_indexed_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8350);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 8]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b111, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 8],
            span_len: 8,
            byte_mask: Some(element_byte_mask(&[true, false], 8)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 8],
            data: source[0..8].to_vec(),
            byte_mask: element_byte_mask(&[true; 8], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_sparse_indexed_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8370);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 24]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b111, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 24],
            span_len: 8,
            byte_mask: Some(element_byte_mask(&[true, false], 8)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            index_width: MemoryWidth::Doubleword,
            offsets: vec![0, 24],
            data: source[0..8].to_vec(),
            byte_mask: element_byte_mask(&[true; 8], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_sparse_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8280);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 12, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 12],
            span_len: 4,
            byte_mask: Some(element_byte_mask(&[true, false], 4)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 4];
    data[0..4].copy_from_slice(&source[0..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 12],
            data,
            byte_mask: element_byte_mask(&[true, true, true, true], 1),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_leading_gap_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x82b0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([4, 12, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![4, 12],
            span_len: 8,
            byte_mask: Some(element_byte_mask(&[true, false], 4)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 8];
    data[4..8].copy_from_slice(&source[0..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![4, 12],
            data,
            byte_mask: element_byte_mask(&[false, true], 4),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_reversed_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x83d0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([12, 0, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![12, 0],
            span_len: 16,
            byte_mask: Some(element_byte_mask(&[true, false], 4)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[12..16].copy_from_slice(&source[0..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![12, 0],
            data,
            byte_mask: element_byte_mask(&[false, false, false, true], 4),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_indexed_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x82d0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 4, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadIndexed {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 4],
            span_len: 4,
            byte_mask: Some(element_byte_mask(&[true, false], 4)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 4];
    data[0..4].copy_from_slice(&source[0..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreIndexed {
            address: 0x9020,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Word,
            offsets: vec![0, 4],
            data,
            byte_mask: element_byte_mask(&[true, true, true, true,], 1,),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_strided_e8_m1_stride15_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8260);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(12), 15);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, 0b000, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            stride: 15,
            element_count: 2,
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(true, 0b000, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0] = source[0];
    data[15] = source[1];
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Byte,
            stride: 15,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(
                &[
                    true, false, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, true,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_strided_e8_m1_stride15_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8270);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc0));
    hart.write(reg(12), 15);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = [0xa1, 0xb1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b000, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Byte,
            stride: 15,
            element_count: 2,
            span_len: 16,
            byte_mask: Some(element_byte_mask(&[true, false], 1)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b000, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0] = source[0];
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Byte,
            stride: 15,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(
                &[
                    true, false, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, false,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_strided_e16_m1_stride14_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8240);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    hart.write(reg(12), 14);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    let source = lanes_u16([0xa1a2, 0xb1b2, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, 0b101, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Halfword,
            stride: 14,
            element_count: 2,
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(true, 0b101, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..2].copy_from_slice(&source[0..2]);
    data[14..16].copy_from_slice(&source[2..4]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Halfword,
            stride: 14,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(
                &[true, false, false, false, false, false, false, true],
                2,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_strided_e16_m1_stride14_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8250);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    hart.write(reg(12), 14);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u16([0xa1a2, 0xb1b2, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b101, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Halfword,
            stride: 14,
            element_count: 2,
            span_len: 16,
            byte_mask: Some(element_byte_mask(&[true, false], 2)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b101, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..2].copy_from_slice(&source[0..2]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Halfword,
            stride: 14,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(
                &[true, false, false, false, false, false, false, false],
                2,
            ),
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
            byte_mask: None,
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
fn hart_builds_masked_strided_e32_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x8090);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(12), 12);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0, 0]);
    hart.write_vector(vreg(1), source);

    let compact_mask = element_byte_mask(&[true, false], 4);
    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b110, 14, 12, 1)).unwrap(),
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
            byte_mask: Some(compact_mask),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b110, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..4].copy_from_slice(&source[0..4]);
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
                    false, false, false, false,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_strided_e32_m1_stride6_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x81a0);
    hart.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    hart.write(reg(12), 6);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0]);
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
            stride: 6,
            element_count: 3,
            span_len: 16,
            byte_mask: None,
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
    data[6..10].copy_from_slice(&source[4..8]);
    data[12..16].copy_from_slice(&source[8..12]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Word,
            stride: 6,
            element_count: 3,
            data,
            byte_mask: element_byte_mask(
                &[
                    true, true, true, true, false, false, true, true, true, true, false, false,
                    true, true, true, true,
                ],
                1,
            ),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_strided_e32_m1_stride6_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x81b0);
    hart.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    hart.write(reg(12), 6);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u32([0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b110, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Word,
            stride: 6,
            element_count: 3,
            span_len: 16,
            byte_mask: Some(element_byte_mask(&[true, false, true], 4)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b110, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..4].copy_from_slice(&source[0..4]);
    data[12..16].copy_from_slice(&source[8..12]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Word,
            stride: 6,
            element_count: 3,
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
fn hart_builds_strided_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x81c0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(12), 8);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, 0b111, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            stride: 8,
            element_count: 2,
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(true, 0b111, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            stride: 8,
            element_count: 2,
            data: source.to_vec(),
            byte_mask: element_byte_mask(&[true, true], 8),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_sparse_strided_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x81e0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(12), 24);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, 0b111, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            span_len: 32,
            byte_mask: None,
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(true, 0b111, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 32];
    data[0..8].copy_from_slice(&source[0..8]);
    data[24..32].copy_from_slice(&source[8..16]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(&[true, false, false, true], 8),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_strided_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x81d0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(12), 8);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b111, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            stride: 8,
            element_count: 2,
            span_len: 16,
            byte_mask: Some(element_byte_mask(&[true, false], 8)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b111, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 16];
    data[0..8].copy_from_slice(&source[0..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            stride: 8,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(&[true, false], 8),
            group_registers: 1,
        })
    );
}

#[test]
fn hart_builds_masked_sparse_strided_e64_m1_vector_memory_accesses() {
    let mut hart = RiscvHartState::new(0x81f0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(12), 24);
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    let source = lanes_u64([0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8]);
    hart.write_vector(vreg(1), source);

    let load = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b111, 14, 12, 1)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        load.memory_access(),
        Some(&MemoryAccessKind::VectorLoadStrided {
            vd: vreg(1),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            span_len: 32,
            byte_mask: Some(element_byte_mask(&[true, false], 8)),
            group_registers: 1,
        })
    );

    let store = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b111, 16, 12, 1)).unwrap(),
        )
        .unwrap();
    let mut data = vec![0; 32];
    data[0..8].copy_from_slice(&source[0..8]);
    assert_eq!(
        store.memory_access(),
        Some(&MemoryAccessKind::VectorStoreStrided {
            address: 0x9020,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            data,
            byte_mask: element_byte_mask(&[true, false, false, false], 8),
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
fn hart_rejects_masked_fractional_lmul_strided_memory_outside_supported_slice() {
    let mut hart = RiscvHartState::new(0x8180);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd7));
    hart.write(reg(12), 12);
    hart.write(reg(14), 0x9000);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b110, 14, 12, 1)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8180))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(0x8190);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd7));
    hart.write(reg(12), 12);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b110, 16, 12, 1)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8190))
    );
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_rejects_masked_zero_vl_strided_memory_outside_supported_slice() {
    let mut hart = RiscvHartState::new(0x8160);
    hart.set_vector_config(RiscvVectorConfig::new(0, 0xd0));
    hart.write(reg(12), 8);
    hart.write(reg(14), 0x9000);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b110, 14, 12, 1)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8160))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(0x8170);
    hart.set_vector_config(RiscvVectorConfig::new(0, 0xd0));
    hart.write(reg(12), 8);
    hart.write(reg(16), 0x9020);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(false, 0b110, 16, 12, 1)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8170))
    );
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_rejects_unmasked_zero_vl_strided_memory_outside_supported_slice() {
    for (pc, vtype, width_bits, stride) in [
        (0x81e0, 0xc0, 0b000, 4),
        (0x8200, 0xc8, 0b101, 6),
        (0x8220, 0xd0, 0b110, 12),
        (0x8240, 0xd8, 0b111, 8),
    ] {
        assert_zero_vl_strided_memory_trap(pc, vtype, width_bits, stride);
    }
}

#[test]
fn hart_rejects_indexed_vector_memory_outside_supported_slice() {
    assert_indexed_memory_trap(0x8280, 0xd0, 0b110, 0, [0, 4, 0, 0]);
    assert_indexed_memory_trap(0x82a0, 0xc8, 0b101, 2, [0, 4, 0, 0]);
    assert_indexed_memory_trap(0x82c0, 0xd8, 0b111, 2, [0, 4, 0, 0]);
    assert_indexed_memory_trap(0x82e0, 0xd0, 0b110, 2, [0, 8, 0, 0]);
    assert_indexed_memory_trap(0x8300, 0xd0, 0b110, 3, [0, 4, 8, 0]);
}

#[test]
fn hart_rejects_masked_indexed_nonprefix_active_lanes_outside_supported_slice() {
    let mut hart = RiscvHartState::new(0x8320);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 4, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8320))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(0x8330);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 4, 0, 0]));
    hart.write_vector(
        vreg(0),
        [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b110, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8330))
    );
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_rejects_masked_indexed_e64_nonprefix_active_lanes_outside_supported_slice() {
    let mut hart = RiscvHartState::new(0x8360);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 8]));
    hart.write_vector(
        vreg(0),
        [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(false, 0b111, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8360))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(0x8370);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 8]));
    hart.write_vector(
        vreg(0),
        [0b0000_0010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(false, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8370))
    );
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_rejects_mixed_width_indexed_memory_outside_supported_slice() {
    let mut hart = RiscvHartState::new(0x8380);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32([0, 8, 0, 0]));

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8380))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(0x8390);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 4]));

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8390))
    );
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_rejects_e64_indexed_noncontiguous_offsets_outside_supported_slice() {
    let mut hart = RiscvHartState::new(0x83a0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 4]));

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x83a0))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(0x83b0);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u64([0, 4]));

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x83b0))
    );
    assert_eq!(record.memory_access(), None);
}

#[test]
fn hart_rejects_masked_strided_load_when_destination_overlaps_v0() {
    let mut hart = RiscvHartState::new(0x8150);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write(reg(12), 12);
    hart.write(reg(14), 0x9000);
    hart.write_vector(
        vreg(0),
        [0b0000_0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(false, 0b110, 14, 12, 0)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8150))
    );
    assert_eq!(record.memory_access(), None);
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

fn assert_zero_vl_strided_memory_trap(pc: u64, vtype: u64, width_bits: u32, stride: u64) {
    let mut hart = RiscvHartState::new(pc);
    hart.set_vector_config(RiscvVectorConfig::new(0, vtype));
    hart.write(reg(12), stride);
    hart.write(reg(14), 0x9000);

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_load_type(true, width_bits, 14, 12, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, pc))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(pc + 0x10);
    hart.set_vector_config(RiscvVectorConfig::new(0, vtype));
    hart.write(reg(12), stride);
    hart.write(reg(16), 0x9020);

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_strided_store_type(true, width_bits, 16, 12, 1))
                .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::IllegalInstruction,
            pc + 0x10
        ))
    );
    assert_eq!(record.memory_access(), None);
}

fn assert_indexed_memory_trap(pc: u64, vtype: u64, width_bits: u32, vl: u32, offsets: [u32; 4]) {
    let mut hart = RiscvHartState::new(pc);
    hart.set_vector_config(RiscvVectorConfig::new(vl, vtype));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32(offsets));

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_load_type(
                true, width_bits, 14, 2, 1,
            ))
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, pc))
    );
    assert_eq!(record.memory_access(), None);

    let mut hart = RiscvHartState::new(pc + 0x10);
    hart.set_vector_config(RiscvVectorConfig::new(vl, vtype));
    hart.write(reg(14), 0x9000);
    hart.write(reg(16), 0x9020);
    hart.write_vector(vreg(2), lanes_u32(offsets));

    let record = hart
        .execute(
            RiscvInstruction::decode(vector_indexed_unordered_store_type(
                true, width_bits, 16, 2, 1,
            ))
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(
            RiscvTrapKind::IllegalInstruction,
            pc + 0x10
        ))
    );
    assert_eq!(record.memory_access(), None);
}
