use rem6_isa_riscv::{
    RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind, RiscvVectorConfig, VectorRegister,
};

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vmand_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011001, vs2, vs1, vd)
}

fn vmnand_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011101, vs2, vs1, vd)
}

fn vmandn_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011000, vs2, vs1, vd)
}

fn vmxor_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011011, vs2, vs1, vd)
}

fn vmor_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011010, vs2, vs1, vd)
}

fn vmnor_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011110, vs2, vs1, vd)
}

fn vmorn_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011100, vs2, vs1, vd)
}

fn vmxnor_mm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b011111, vs2, vs1, vd)
}

#[test]
fn decoder_accepts_unmasked_vector_mask_logical_operations() {
    assert_eq!(vmand_mm_type(5, 6, 4), 0x6653_2257);
    assert_eq!(vmnand_mm_type(5, 6, 4), 0x7653_2257);
    assert_eq!(vmandn_mm_type(5, 6, 4), 0x6253_2257);
    assert_eq!(vmxor_mm_type(5, 6, 4), 0x6e53_2257);
    assert_eq!(vmor_mm_type(5, 6, 4), 0x6a53_2257);
    assert_eq!(vmnor_mm_type(5, 6, 4), 0x7a53_2257);
    assert_eq!(vmorn_mm_type(5, 6, 4), 0x7253_2257);
    assert_eq!(vmxnor_mm_type(5, 6, 4), 0x7e53_2257);

    assert_eq!(
        RiscvInstruction::decode(vmand_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskAndMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmnand_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskNandMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmandn_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskAndNotMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmxor_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskXorMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmor_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskOrMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmnor_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskNorMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmorn_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskOrNotMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmxnor_mm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskXnorMm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
}

#[test]
fn hart_executes_vector_mask_logical_operations_and_preserves_tail_bits() {
    let cases = [
        (vmand_mm_type(5, 6, 4), 0xc8),
        (vmnand_mm_type(5, 6, 4), 0xf7),
        (vmandn_mm_type(5, 6, 4), 0xc2),
        (vmxor_mm_type(5, 6, 4), 0xe6),
        (vmor_mm_type(5, 6, 4), 0xee),
        (vmnor_mm_type(5, 6, 4), 0xd1),
        (vmorn_mm_type(5, 6, 4), 0xdb),
        (vmxnor_mm_type(5, 6, 4), 0xd9),
    ];

    for (raw, expected_first_byte) in cases {
        let mut hart = RiscvHartState::new(0x9000);
        hart.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
        hart.write_vector(
            vreg(5),
            [0xca, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        hart.write_vector(
            vreg(6),
            [0xac, 0xbb, 0xbb, 0xbb, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        hart.write_vector(
            vreg(4),
            [0xc0, 0xee, 0xee, 0xee, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );

        hart.execute(RiscvInstruction::decode(raw).unwrap())
            .unwrap();

        assert_eq!(
            hart.read_vector(vreg(4)),
            [
                expected_first_byte,
                0xee,
                0xee,
                0xee,
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
                0
            ]
        );
    }
}

#[test]
fn hart_traps_vector_mask_logical_when_vl_exceeds_vlmax() {
    let mut hart = RiscvHartState::new(0x9100);
    hart.set_vector_config(RiscvVectorConfig::new(3, 0xd8));
    hart.write_vector(vreg(5), [0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(6), [0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(
        vreg(4),
        [0x80, 0xee, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );

    let record = hart
        .execute(RiscvInstruction::decode(vmand_mm_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x9100))
    );
    assert_eq!(
        hart.read_vector(vreg(4)),
        [0x80, 0xee, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}
