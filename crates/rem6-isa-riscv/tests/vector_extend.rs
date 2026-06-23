use rem6_isa_riscv::{
    RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind, RiscvVectorConfig,
    RiscvVectorExtensionFactor, RiscvVectorMaskMode, VectorRegister,
};

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vzext_vf2_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00110 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vzext_vf2_masked_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (u32::from(vs2) << 20)
        | (0b00110 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vzext_vf4_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00100 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vzext_vf8_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00010 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsext_vf2_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00111 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsext_vf4_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00101 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsext_vf8_type(vs2: u8, vd: u8) -> u32 {
    (0b010010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b00011 << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

#[test]
fn decoder_accepts_vector_extend_forms() {
    assert_eq!(vzext_vf2_type(4, 3), 0x4a43_21d7);
    assert_eq!(
        RiscvInstruction::decode(vzext_vf2_type(4, 3)).unwrap(),
        RiscvInstruction::VectorZeroExtend {
            vd: vreg(3),
            vs2: vreg(4),
            factor: RiscvVectorExtensionFactor::F2,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vzext_vf2_masked_type(4, 3)).unwrap(),
        RiscvInstruction::VectorZeroExtend {
            vd: vreg(3),
            vs2: vreg(4),
            factor: RiscvVectorExtensionFactor::F2,
            mask: RiscvVectorMaskMode::Masked,
        }
    );
    assert_eq!(vzext_vf4_type(6, 5), 0x4a62_22d7);
    assert_eq!(
        RiscvInstruction::decode(vzext_vf4_type(6, 5)).unwrap(),
        RiscvInstruction::VectorZeroExtend {
            vd: vreg(5),
            vs2: vreg(6),
            factor: RiscvVectorExtensionFactor::F4,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(vzext_vf8_type(8, 7), 0x4a81_23d7);
    assert_eq!(
        RiscvInstruction::decode(vzext_vf8_type(8, 7)).unwrap(),
        RiscvInstruction::VectorZeroExtend {
            vd: vreg(7),
            vs2: vreg(8),
            factor: RiscvVectorExtensionFactor::F8,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(vsext_vf2_type(10, 9), 0x4aa3_a4d7);
    assert_eq!(
        RiscvInstruction::decode(vsext_vf2_type(10, 9)).unwrap(),
        RiscvInstruction::VectorSignExtend {
            vd: vreg(9),
            vs2: vreg(10),
            factor: RiscvVectorExtensionFactor::F2,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(vsext_vf4_type(12, 11), 0x4ac2_a5d7);
    assert_eq!(
        RiscvInstruction::decode(vsext_vf4_type(12, 11)).unwrap(),
        RiscvInstruction::VectorSignExtend {
            vd: vreg(11),
            vs2: vreg(12),
            factor: RiscvVectorExtensionFactor::F4,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(vsext_vf8_type(14, 13), 0x4ae1_a6d7);
    assert_eq!(
        RiscvInstruction::decode(vsext_vf8_type(14, 13)).unwrap(),
        RiscvInstruction::VectorSignExtend {
            vd: vreg(13),
            vs2: vreg(14),
            factor: RiscvVectorExtensionFactor::F8,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
}

#[test]
fn hart_executes_vzext_vf2_from_u8_to_u16_lanes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x88));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(
        vreg(4),
        [
            0x7f, 0x80, 0xff, 0x01, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa,
        ],
    );

    hart.execute(RiscvInstruction::decode(vzext_vf2_type(4, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [
            0x7f, 0x00, 0x80, 0x00, 0xff, 0x00, 0x01, 0x00, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );
}

#[test]
fn hart_executes_vsext_vf2_from_i8_to_i16_lanes() {
    let mut hart = RiscvHartState::new(0x8040);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x88));
    hart.write_vector(vreg(9), [0xee; 16]);
    hart.write_vector(
        vreg(10),
        [
            0x7f, 0x80, 0xff, 0x01, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa,
        ],
    );

    hart.execute(RiscvInstruction::decode(vsext_vf2_type(10, 9)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(9)),
        [
            0x7f, 0x00, 0x80, 0xff, 0xff, 0xff, 0x01, 0x00, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );
}

#[test]
fn hart_executes_masked_vzext_vf2_from_u8_to_u16_lanes() {
    let mut hart = RiscvHartState::new(0x80c0);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x88));
    hart.write_vector(
        vreg(0),
        [0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(
        vreg(4),
        [
            0x7f, 0x80, 0xff, 0x01, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa,
        ],
    );

    hart.execute(RiscvInstruction::decode(vzext_vf2_masked_type(4, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [
            0x7f, 0x00, 0xee, 0xee, 0xff, 0x00, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );
}

#[test]
fn hart_traps_vzext_vf2_when_source_width_is_too_narrow() {
    let mut hart = RiscvHartState::new(0x8080);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x80));

    let record = hart
        .execute(RiscvInstruction::decode(vzext_vf2_type(4, 3)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8080))
    );
}

#[test]
fn hart_traps_vzext_vf2_when_source_overlaps_low_destination_part() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x89));

    let record = hart
        .execute(RiscvInstruction::decode(vzext_vf2_type(2, 2)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8100))
    );
}

#[test]
fn hart_executes_vzext_vf2_when_source_overlaps_high_destination_part() {
    let mut hart = RiscvHartState::new(0x8120);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x89));
    let source = [
        0x7f, 0x80, 0xff, 0x01, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        0xaa,
    ];
    hart.write_vector(vreg(2), [0xee; 16]);
    hart.write_vector(vreg(3), source);

    hart.execute(RiscvInstruction::decode(vzext_vf2_type(3, 2)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(2)),
        [
            0x7f, 0x00, 0x80, 0x00, 0xff, 0x00, 0x01, 0x00, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );
    assert_eq!(hart.read_vector(vreg(3)), source);
}

#[test]
fn hart_traps_vzext_vf2_when_fractional_source_emul_overlaps_destination() {
    let mut hart = RiscvHartState::new(0x8140);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x88));

    let record = hart
        .execute(RiscvInstruction::decode(vzext_vf2_type(4, 4)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8140))
    );
}
