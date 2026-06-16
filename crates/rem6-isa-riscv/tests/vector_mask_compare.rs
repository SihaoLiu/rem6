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
