use rem6_isa_riscv::{
    Register, RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind, RiscvVectorConfig,
    VectorRegister,
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

fn vector_masked_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26) | (u32::from(vs2) << 20) | (u32::from(vs1) << 15) | (u32::from(vd) << 7) | 0x57
}

fn vector_masked_vx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_masked_vi_type(funct6: u32, vs2: u8, imm: i8, vd: u8) -> u32 {
    (funct6 << 26)
        | (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vmerge_vvm_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_masked_vv_type(0b010111, vs2, vs1, vd)
}

fn vmerge_vxm_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_masked_vx_type(0b010111, vs2, rs1, vd)
}

fn vmerge_vim_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_masked_vi_type(0b010111, vs2, imm, vd)
}

fn vmv_v_v_type(vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b010111, 0, vs1, vd)
}

fn vmv_v_x_type(rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b010111, 0, rs1, vd)
}

fn vmv_v_i_type(imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b010111, 0, imm, vd)
}

#[test]
fn decoder_accepts_vector_merge_and_move_operations() {
    assert_eq!(vmerge_vvm_type(5, 6, 4), 0x5c53_0257);
    assert_eq!(vmerge_vxm_type(5, 6, 4), 0x5c53_4257);
    assert_eq!(vmerge_vim_type(5, -3, 4), 0x5c5e_b257);
    assert_eq!(vmv_v_v_type(8, 7), 0x5e04_03d7);
    assert_eq!(vmv_v_x_type(7, 7), 0x5e03_c3d7);
    assert_eq!(vmv_v_i_type(-4, 7), 0x5e0e_33d7);

    assert_eq!(
        RiscvInstruction::decode(vmerge_vvm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMergeVvm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmerge_vxm_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMergeVxm {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmerge_vim_type(5, -3, 4)).unwrap(),
        RiscvInstruction::VectorMergeVim {
            vd: vreg(4),
            vs2: vreg(5),
            imm: -3,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmv_v_v_type(8, 7)).unwrap(),
        RiscvInstruction::VectorMoveVv {
            vd: vreg(7),
            vs1: vreg(8),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmv_v_x_type(7, 7)).unwrap(),
        RiscvInstruction::VectorMoveVx {
            vd: vreg(7),
            rs1: reg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmv_v_i_type(-4, 7)).unwrap(),
        RiscvInstruction::VectorMoveVi {
            vd: vreg(7),
            imm: -4,
        }
    );
}

#[test]
fn hart_executes_vector_merge_and_move_operations() {
    let mut merge_vv = RiscvHartState::new(0x8240);
    merge_vv.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    merge_vv.write_vector(vreg(0), [0x0b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    merge_vv.write_vector(
        vreg(5),
        [
            10, 20, 30, 40, 50, 60, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    merge_vv.write_vector(
        vreg(6),
        [1, 2, 3, 4, 5, 6, 0xbb, 0xbb, 0xbb, 0xbb, 0, 0, 0, 0, 0, 0],
    );
    merge_vv.write_vector(vreg(4), [0xee; 16]);
    let merge_vv_record = merge_vv
        .execute(RiscvInstruction::decode(vmerge_vvm_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        merge_vv_record.instruction(),
        RiscvInstruction::VectorMergeVvm {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        }
    );
    assert_eq!(
        merge_vv.read_vector(vreg(4)),
        [1, 2, 30, 4, 50, 60, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee]
    );

    let mut merge_vx = RiscvHartState::new(0x8250);
    merge_vx.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    merge_vx.write(reg(6), 0xab);
    merge_vx.write_vector(vreg(0), [0x0d, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    merge_vx.write_vector(
        vreg(5),
        [
            10, 20, 30, 40, 50, 60, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    merge_vx.write_vector(vreg(4), [0xee; 16]);
    merge_vx
        .execute(RiscvInstruction::decode(vmerge_vxm_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        merge_vx.read_vector(vreg(4)),
        [
            0xab, 20, 0xab, 0xab, 50, 60, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee,
        ]
    );

    let mut merge_vi = RiscvHartState::new(0x8260);
    merge_vi.set_vector_config(RiscvVectorConfig::new(6, 0xc0));
    merge_vi.write_vector(vreg(0), [0x12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    merge_vi.write_vector(
        vreg(5),
        [
            10, 20, 30, 40, 50, 60, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0,
        ],
    );
    merge_vi.write_vector(vreg(4), [0xee; 16]);
    merge_vi
        .execute(RiscvInstruction::decode(vmerge_vim_type(5, -3, 4)).unwrap())
        .unwrap();
    assert_eq!(
        merge_vi.read_vector(vreg(4)),
        [10, 0xfd, 30, 40, 0xfd, 60, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,]
    );

    let mut move_vv = RiscvHartState::new(0x8270);
    move_vv.set_vector_config(RiscvVectorConfig::new(5, 0xc0));
    move_vv.write_vector(
        vreg(8),
        [1, 2, 3, 4, 5, 6, 0xaa, 0xaa, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0],
    );
    move_vv.write_vector(vreg(7), [0xee; 16]);
    move_vv
        .execute(RiscvInstruction::decode(vmv_v_v_type(8, 7)).unwrap())
        .unwrap();
    assert_eq!(
        move_vv.read_vector(vreg(7)),
        [1, 2, 3, 4, 5, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee]
    );

    let mut move_vx = RiscvHartState::new(0x8280);
    move_vx.set_vector_config(RiscvVectorConfig::new(5, 0xc0));
    move_vx.write(reg(7), 0x44);
    move_vx.write_vector(vreg(7), [0xee; 16]);
    move_vx
        .execute(RiscvInstruction::decode(vmv_v_x_type(7, 7)).unwrap())
        .unwrap();
    assert_eq!(
        move_vx.read_vector(vreg(7)),
        [
            0x44, 0x44, 0x44, 0x44, 0x44, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );

    let mut move_vi = RiscvHartState::new(0x8290);
    move_vi.set_vector_config(RiscvVectorConfig::new(5, 0xc0));
    move_vi.write_vector(vreg(7), [0xee; 16]);
    move_vi
        .execute(RiscvInstruction::decode(vmv_v_i_type(-4, 7)).unwrap())
        .unwrap();
    assert_eq!(
        move_vi.read_vector(vreg(7)),
        [
            0xfc, 0xfc, 0xfc, 0xfc, 0xfc, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee,
        ]
    );
}

#[test]
fn hart_traps_vector_merge_when_mask_register_overlaps_sources_or_destination() {
    assert_merge_overlap_trap(vmerge_vvm_type(5, 6, 0));
    assert_merge_overlap_trap(vmerge_vvm_type(0, 6, 4));
    assert_merge_overlap_trap(vmerge_vvm_type(5, 0, 4));
    assert_merge_overlap_trap(vmerge_vxm_type(0, 6, 4));
    assert_merge_overlap_trap(vmerge_vim_type(0, -3, 4));
}

fn assert_merge_overlap_trap(raw: u32) {
    let mut hart = RiscvHartState::new(0x8800);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xc0));
    hart.write_vector(vreg(0), [0x0f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    hart.write_vector(vreg(4), [0xee; 16]);

    let record = hart
        .execute(RiscvInstruction::decode(raw).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8800))
    );
    assert_eq!(hart.read_vector(vreg(4)), [0xee; 16]);
}
