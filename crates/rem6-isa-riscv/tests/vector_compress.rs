use rem6_isa_riscv::{
    MemoryWidth, RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind,
    RiscvVectorCompressPlan, RiscvVectorConfig, RiscvVectorElements, RiscvVectorTailPolicy,
    VectorRegister,
};

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vcompress_vm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (0b010111 << 26)
        | (1 << 25)
        | ((vs2 as u32) << 20)
        | ((vs1 as u32) << 15)
        | (0x2 << 12)
        | ((vd as u32) << 7)
        | 0x57
}

#[test]
fn decoder_accepts_vcompress_vm() {
    assert_eq!(vcompress_vm_type(4, 5, 3), 0x5e42_a1d7);
    assert_eq!(
        RiscvInstruction::decode(vcompress_vm_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorCompressVm(vreg(3), vreg(4), vreg(5))
    );
}

#[test]
fn vcompress_tail_undisturbed_preserves_elements_after_compressed_count() {
    let destination = RiscvVectorElements::new(
        MemoryWidth::Byte,
        vec![
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x70, 0xe8, 0x41, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f,
        ],
    )
    .unwrap();
    let source = RiscvVectorElements::new(
        MemoryWidth::Byte,
        vec![
            0xf0, 0xe8, 0x41, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
    )
    .unwrap();
    let mask = vec![
        false, false, false, false, false, false, false, false, true, true, true, true, true, true,
        true, true,
    ];

    let result = RiscvVectorCompressPlan::new(16, RiscvVectorTailPolicy::Undisturbed)
        .execute(&destination, &source, &mask)
        .unwrap();

    assert_eq!(result.compressed_count(), 8);
    assert_eq!(
        result.elements().as_slice(),
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x70, 0xe8, 0x41, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f
        ]
    );
}

#[test]
fn hart_executes_vcompress_vm_for_active_u8_lanes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_vector_config(RiscvVectorConfig::new(6, 0x80));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(
        vreg(4),
        [
            10, 20, 30, 40, 50, 60, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    hart.write_vector(
        vreg(5),
        [0b0010_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    hart.execute(RiscvInstruction::decode(vcompress_vm_type(4, 5, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [
            10, 30, 60, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee
        ]
    );
}

#[test]
fn hart_executes_vcompress_vm_with_tail_agnostic_policy() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xc0));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(
        vreg(4),
        [
            10, 20, 30, 40, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        ],
    );
    hart.write_vector(
        vreg(5),
        [0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    hart.execute(RiscvInstruction::decode(vcompress_vm_type(4, 5, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [
            10, 30, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff
        ]
    );
}

#[test]
fn hart_traps_vcompress_vm_when_destination_overlaps_source_or_mask() {
    assert_vcompress_overlap_trap(vcompress_vm_type(4, 5, 4));
    assert_vcompress_overlap_trap(vcompress_vm_type(4, 3, 3));
    assert_vcompress_overlap_trap(vcompress_vm_type(4, 4, 3));
}

fn assert_vcompress_overlap_trap(raw: u32) {
    let mut hart = RiscvHartState::new(0x8200);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xc0));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(vreg(4), [0xcc; 16]);
    hart.write_vector(vreg(5), [0x0f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    let record = hart
        .execute(RiscvInstruction::decode(raw).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8200))
    );
    assert_eq!(hart.read_vector(vreg(3)), [0xee; 16]);
    assert_eq!(hart.read_vector(vreg(4)), [0xcc; 16]);
}

#[test]
fn vcompress_tail_agnostic_uses_deterministic_ones_after_compressed_count() {
    let destination =
        RiscvVectorElements::new(MemoryWidth::Byte, vec![0xaa, 0xbb, 0xcc, 0xdd]).unwrap();
    let source = RiscvVectorElements::new(MemoryWidth::Byte, vec![0x10, 0x20, 0x30, 0x40]).unwrap();
    let mask = vec![true, false, true, false];

    let result = RiscvVectorCompressPlan::new(4, RiscvVectorTailPolicy::AgnosticAllOnes)
        .execute(&destination, &source, &mask)
        .unwrap();

    assert_eq!(result.compressed_count(), 2);
    assert_eq!(result.elements().as_slice(), &[0x10, 0x30, 0xff, 0xff]);
}
