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

fn lanes_u32(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn e32_byte_mask(active_lanes: [bool; 4]) -> Vec<bool> {
    active_lanes
        .into_iter()
        .flat_map(|active| [active; 4])
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
fn hart_rejects_masked_unit_stride_vector_memory_outside_e32_m1_slice() {
    assert_masked_memory_trap(0xd1, vector_unit_stride_load_type(false, 0b110, 14, 2)); // e32,m2
    assert_masked_memory_trap(0xc8, vector_unit_stride_load_type(false, 0b101, 14, 2)); // e16,m1
    assert_masked_memory_trap(0xd1, vector_unit_stride_store_type(false, 0b110, 14, 2)); // e32,m2
    assert_masked_memory_trap(0xc8, vector_unit_stride_store_type(false, 0b101, 14, 2));
    // e16,m1
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
