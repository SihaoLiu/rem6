use rem6_isa_riscv::{
    Register, RiscvHartState, RiscvInstruction, RiscvVectorConfig, VectorRegister,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vector_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_vx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_vi_type(funct6: u32, vs2: u8, imm: i8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vmseq_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011000, vs2, vs1, vd)
}

fn vmseq_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011000, vs2, rs1, vd)
}

fn vmseq_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011000, vs2, imm, vd)
}

fn vmsne_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011001, vs2, vs1, vd)
}

fn vmsne_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011001, vs2, rs1, vd)
}

fn vmsne_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011001, vs2, imm, vd)
}

fn vmsltu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011010, vs2, vs1, vd)
}

fn vmsltu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011010, vs2, rs1, vd)
}

fn vmslt_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011011, vs2, vs1, vd)
}

fn vmslt_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011011, vs2, rs1, vd)
}

fn vmsleu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011100, vs2, vs1, vd)
}

fn vmsleu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011100, vs2, rs1, vd)
}

fn vmsleu_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011100, vs2, imm, vd)
}

fn vmsle_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b011101, vs2, vs1, vd)
}

fn vmsle_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011101, vs2, rs1, vd)
}

fn vmsle_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011101, vs2, imm, vd)
}

fn vmsgtu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011110, vs2, rs1, vd)
}

fn vmsgtu_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011110, vs2, imm, vd)
}

fn vmsgt_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b011111, vs2, rs1, vd)
}

fn vmsgt_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b011111, vs2, imm, vd)
}

fn lanes_u32(lanes: [u32; 4]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn bytes_with_u16(lanes: [u16; 8]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 2..index * 2 + 2].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

fn ordered_mask_fixture(scalar: u64) -> RiscvHartState {
    let mut hart = RiscvHartState::new(0x9000);
    hart.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    hart.write(reg(5), scalar);
    hart.write_vector(
        vreg(2),
        [
            0, 1, 2, 127, 128, 255, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    hart.write_vector(
        vreg(3),
        [
            1, 1, 0, 127, 0, 254, 0xbb, 0xbb, 0xbb, 0xbb, 0, 0, 0, 0, 0, 0,
        ],
    );
    hart.write_vector(vreg(4), [0; 16]);
    hart
}

fn assert_mask_byte(hart: &RiscvHartState, register: VectorRegister, expected: u8) {
    let mut expected_mask = [0; 16];
    expected_mask[0] = expected;
    assert_eq!(hart.read_vector(register), expected_mask);
}

#[test]
fn decoder_accepts_unmasked_vector_mask_compare_operations() {
    assert_eq!(vmseq_vv_type(7, 8, 6), 0x6274_0357);
    assert_eq!(
        RiscvInstruction::decode(vmseq_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaskEqualVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    assert_eq!(vmseq_vx_type(5, 6, 4), 0x6253_4257);
    assert_eq!(
        RiscvInstruction::decode(vmseq_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskEqualVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmseq_vi_type(2, -1, 3), 0x622f_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vmseq_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorMaskEqualVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );

    assert_eq!(vmsne_vv_type(7, 8, 6), 0x6674_0357);
    assert_eq!(
        RiscvInstruction::decode(vmsne_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaskNotEqualVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    assert_eq!(vmsne_vx_type(5, 6, 4), 0x6653_4257);
    assert_eq!(
        RiscvInstruction::decode(vmsne_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskNotEqualVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmsne_vi_type(2, -1, 3), 0x662f_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vmsne_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorMaskNotEqualVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vector_ordered_mask_compare_operations() {
    assert_eq!(vmsltu_vv_type(7, 8, 6), 0x6a74_0357);
    assert_eq!(
        RiscvInstruction::decode(vmsltu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaskLessUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    assert_eq!(vmsltu_vx_type(5, 6, 4), 0x6a53_4257);
    assert_eq!(
        RiscvInstruction::decode(vmsltu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskLessUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmslt_vv_type(7, 8, 6), 0x6e74_0357);
    assert_eq!(
        RiscvInstruction::decode(vmslt_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaskLessSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    assert_eq!(vmslt_vx_type(5, 6, 4), 0x6e53_4257);
    assert_eq!(
        RiscvInstruction::decode(vmslt_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskLessSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmsleu_vv_type(7, 8, 6), 0x7274_0357);
    assert_eq!(
        RiscvInstruction::decode(vmsleu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaskLessEqualUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    assert_eq!(vmsleu_vx_type(5, 6, 4), 0x7253_4257);
    assert_eq!(
        RiscvInstruction::decode(vmsleu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskLessEqualUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmsleu_vi_type(2, -1, 3), 0x722f_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vmsleu_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorMaskLessEqualUnsignedVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );

    assert_eq!(vmsle_vv_type(7, 8, 6), 0x7674_0357);
    assert_eq!(
        RiscvInstruction::decode(vmsle_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaskLessEqualSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    assert_eq!(vmsle_vx_type(5, 6, 4), 0x7653_4257);
    assert_eq!(
        RiscvInstruction::decode(vmsle_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskLessEqualSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmsle_vi_type(2, -1, 3), 0x762f_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vmsle_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorMaskLessEqualSignedVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );

    assert_eq!(vmsgtu_vx_type(5, 6, 4), 0x7a53_4257);
    assert_eq!(
        RiscvInstruction::decode(vmsgtu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskGreaterUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmsgtu_vi_type(2, -1, 3), 0x7a2f_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vmsgtu_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorMaskGreaterUnsignedVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );

    assert_eq!(vmsgt_vx_type(5, 6, 4), 0x7e53_4257);
    assert_eq!(
        RiscvInstruction::decode(vmsgt_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaskGreaterSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    assert_eq!(vmsgt_vi_type(2, -1, 3), 0x7e2f_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vmsgt_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorMaskGreaterSignedVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );
}

#[test]
fn hart_executes_vector_mask_compare_operations() {
    let mut vv = RiscvHartState::new(0x8000);
    vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    vv.write_vector(vreg(7), lanes_u32([1, 2, 3, 4]));
    vv.write_vector(vreg(8), lanes_u32([1, 9, 3, 4]));
    vv.write_vector(vreg(6), [0xf8; 16]);

    let record = vv
        .execute(RiscvInstruction::decode(vmseq_vv_type(7, 8, 6)).unwrap())
        .unwrap();

    let mut expected = [0xf8; 16];
    expected[0] = 0xfd;
    assert_eq!(vv.read_vector(vreg(6)), expected);
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorMaskEqualVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut ne_vv = RiscvHartState::new(0x8010);
    ne_vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    ne_vv.write_vector(vreg(7), lanes_u32([1, 2, 3, 4]));
    ne_vv.write_vector(vreg(8), lanes_u32([1, 9, 3, 4]));
    ne_vv.write_vector(vreg(6), [0xf8; 16]);

    ne_vv
        .execute(RiscvInstruction::decode(vmsne_vv_type(7, 8, 6)).unwrap())
        .unwrap();

    let mut expected = [0xf8; 16];
    expected[0] = 0xfa;
    assert_eq!(ne_vv.read_vector(vreg(6)), expected);

    let mut eq_vx = RiscvHartState::new(0x8020);
    eq_vx.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    eq_vx.write(reg(5), 0x1ff);
    eq_vx.write_vector(
        vreg(2),
        [
            0xff, 0, 1, 0xff, 2, 3, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    eq_vx.write_vector(vreg(3), [0; 16]);

    eq_vx
        .execute(RiscvInstruction::decode(vmseq_vx_type(2, 5, 3)).unwrap())
        .unwrap();

    let mut expected = [0; 16];
    expected[0] = 0x09;
    assert_eq!(eq_vx.read_vector(vreg(3)), expected);

    let mut ne_vx = RiscvHartState::new(0x8030);
    ne_vx.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    ne_vx.write(reg(5), 0x1ff);
    ne_vx.write_vector(
        vreg(2),
        [
            0xff, 0, 1, 0xff, 2, 3, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    ne_vx.write_vector(vreg(3), [0; 16]);

    ne_vx
        .execute(RiscvInstruction::decode(vmsne_vx_type(2, 5, 3)).unwrap())
        .unwrap();

    let mut expected = [0; 16];
    expected[0] = 0x36;
    assert_eq!(ne_vx.read_vector(vreg(3)), expected);

    let mut eq_vi = RiscvHartState::new(0x8040);
    eq_vi.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    eq_vi.write_vector(
        vreg(2),
        [
            0xff, 0, 1, 0xff, 2, 3, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    eq_vi.write_vector(vreg(3), [0; 16]);

    eq_vi
        .execute(RiscvInstruction::decode(vmseq_vi_type(2, -1, 3)).unwrap())
        .unwrap();

    let mut expected = [0; 16];
    expected[0] = 0x09;
    assert_eq!(eq_vi.read_vector(vreg(3)), expected);

    let mut ne_vi = RiscvHartState::new(0x8050);
    ne_vi.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    ne_vi.write_vector(
        vreg(2),
        [
            0xff, 0, 1, 0xff, 2, 3, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    ne_vi.write_vector(vreg(3), [0; 16]);

    ne_vi
        .execute(RiscvInstruction::decode(vmsne_vi_type(2, -1, 3)).unwrap())
        .unwrap();

    let mut expected = [0; 16];
    expected[0] = 0x36;
    assert_eq!(ne_vi.read_vector(vreg(3)), expected);
}

#[test]
fn hart_executes_mask_compare_from_lmul2_sources_into_single_mask_register() {
    let mut hart = RiscvHartState::new(0x8060);
    hart.set_vector_config(RiscvVectorConfig::new(6, 0xd1));
    hart.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));
    hart.write_vector(vreg(3), lanes_u32([5, 6, 7, 8]));
    hart.write_vector(vreg(4), lanes_u32([1, 9, 3, 0]));
    hart.write_vector(vreg(5), lanes_u32([5, 0, 7, 8]));
    hart.write_vector(vreg(7), [0x80; 16]);

    let record = hart
        .execute(RiscvInstruction::decode(vmseq_vv_type(2, 4, 7)).unwrap())
        .unwrap();

    let mut expected = [0x80; 16];
    expected[0] = 0x95;
    assert_eq!(hart.read_vector(vreg(7)), expected);
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorMaskEqualVv {
            vd: vreg(7),
            vs1: vreg(4),
            vs2: vreg(2),
        }
    );
}

#[test]
fn hart_sign_extends_mask_compare_immediates_for_wider_element_widths() {
    let mut eq = RiscvHartState::new(0x8070);
    eq.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    eq.write_vector(
        vreg(2),
        bytes_with_u16([0xffff, 0x00ff, 0xffff, 0, 0xaaaa, 0xaaaa, 0xaaaa, 0xaaaa]),
    );
    eq.write_vector(vreg(3), [0; 16]);

    eq.execute(RiscvInstruction::decode(vmseq_vi_type(2, -1, 3)).unwrap())
        .unwrap();

    let mut expected = [0; 16];
    expected[0] = 0x05;
    assert_eq!(eq.read_vector(vreg(3)), expected);

    let mut ne = RiscvHartState::new(0x8080);
    ne.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    ne.write_vector(
        vreg(2),
        bytes_with_u16([0xffff, 0x00ff, 0xffff, 0, 0xaaaa, 0xaaaa, 0xaaaa, 0xaaaa]),
    );
    ne.write_vector(vreg(3), [0; 16]);

    ne.execute(RiscvInstruction::decode(vmsne_vi_type(2, -1, 3)).unwrap())
        .unwrap();

    let mut expected = [0; 16];
    expected[0] = 0x0a;
    assert_eq!(ne.read_vector(vreg(3)), expected);
}

#[test]
fn hart_executes_vector_ordered_mask_compare_operations() {
    let mut less_unsigned_vv = ordered_mask_fixture(0);
    less_unsigned_vv
        .execute(RiscvInstruction::decode(vmsltu_vv_type(2, 3, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_unsigned_vv, vreg(4), 0x01);

    let mut less_unsigned_vx = ordered_mask_fixture(1);
    less_unsigned_vx
        .execute(RiscvInstruction::decode(vmsltu_vx_type(2, 5, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_unsigned_vx, vreg(4), 0x01);

    let mut less_signed_vv = ordered_mask_fixture(0);
    less_signed_vv
        .execute(RiscvInstruction::decode(vmslt_vv_type(2, 3, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_signed_vv, vreg(4), 0x11);

    let mut less_signed_vx = ordered_mask_fixture((-1_i64) as u64);
    less_signed_vx
        .execute(RiscvInstruction::decode(vmslt_vx_type(2, 5, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_signed_vx, vreg(4), 0x10);

    let mut less_equal_unsigned_vv = ordered_mask_fixture(0);
    less_equal_unsigned_vv
        .execute(RiscvInstruction::decode(vmsleu_vv_type(2, 3, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_equal_unsigned_vv, vreg(4), 0x0b);

    let mut less_equal_unsigned_vx = ordered_mask_fixture(1);
    less_equal_unsigned_vx
        .execute(RiscvInstruction::decode(vmsleu_vx_type(2, 5, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_equal_unsigned_vx, vreg(4), 0x03);

    let mut less_equal_unsigned_vi = ordered_mask_fixture(0);
    less_equal_unsigned_vi
        .execute(RiscvInstruction::decode(vmsleu_vi_type(2, -1, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_equal_unsigned_vi, vreg(4), 0x3f);

    let mut less_equal_signed_vv = ordered_mask_fixture(0);
    less_equal_signed_vv
        .execute(RiscvInstruction::decode(vmsle_vv_type(2, 3, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_equal_signed_vv, vreg(4), 0x1b);

    let mut less_equal_signed_vx = ordered_mask_fixture((-1_i64) as u64);
    less_equal_signed_vx
        .execute(RiscvInstruction::decode(vmsle_vx_type(2, 5, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_equal_signed_vx, vreg(4), 0x30);

    let mut less_equal_signed_vi = ordered_mask_fixture(0);
    less_equal_signed_vi
        .execute(RiscvInstruction::decode(vmsle_vi_type(2, -1, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&less_equal_signed_vi, vreg(4), 0x30);

    let mut greater_unsigned_vx = ordered_mask_fixture(1);
    greater_unsigned_vx
        .execute(RiscvInstruction::decode(vmsgtu_vx_type(2, 5, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&greater_unsigned_vx, vreg(4), 0x3c);

    let mut greater_unsigned_vi = ordered_mask_fixture(0);
    greater_unsigned_vi
        .execute(RiscvInstruction::decode(vmsgtu_vi_type(2, -1, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&greater_unsigned_vi, vreg(4), 0x00);

    let mut greater_signed_vx = ordered_mask_fixture((-1_i64) as u64);
    greater_signed_vx
        .execute(RiscvInstruction::decode(vmsgt_vx_type(2, 5, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&greater_signed_vx, vreg(4), 0x0f);

    let mut greater_signed_vi = ordered_mask_fixture(0);
    greater_signed_vi
        .execute(RiscvInstruction::decode(vmsgt_vi_type(2, -1, 4)).unwrap())
        .unwrap();
    assert_mask_byte(&greater_signed_vi, vreg(4), 0x0f);
}
