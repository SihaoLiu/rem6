use rem6_isa_riscv::{
    Register, RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind, RiscvVectorConfig,
    RiscvVectorMaskMode, VectorRegister,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (1 << 25) | (u32::from(vs2) << 20) | (u32::from(vs1) << 15) | (u32::from(vd) << 7) | 0x57
}

fn vadd_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vadd_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vadd_masked_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (u32::from(vs2) << 20) | (u32::from(vs1) << 15) | (u32::from(vd) << 7) | 0x57
}

fn vadd_masked_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    (u32::from(vs2) << 20) | (u32::from(rs1) << 15) | (0b100 << 12) | (u32::from(vd) << 7) | 0x57
}

fn vadd_masked_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsub_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    (0b000010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vsub_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    (0b000010 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vrsub_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000011, vs2, rs1, vd)
}

fn vrsub_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b000011, vs2, imm, vd)
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

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_mvx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b110 << 12)
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

fn vand_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b001001, vs2, vs1, vd)
}

fn vand_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001001, vs2, rs1, vd)
}

fn vand_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b001001, vs2, imm, vd)
}

fn vor_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b001010, vs2, vs1, vd)
}

fn vor_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001010, vs2, rs1, vd)
}

fn vor_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b001010, vs2, imm, vd)
}

fn vxor_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b001011, vs2, vs1, vd)
}

fn vxor_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001011, vs2, rs1, vd)
}

fn vxor_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b001011, vs2, imm, vd)
}

fn vsll_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100101, vs2, vs1, vd)
}

fn vsll_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b100101, vs2, rs1, vd)
}

fn vsll_vi_type(vs2: u8, shamt: u8, vd: u8) -> u32 {
    vector_vi_type(0b100101, vs2, shamt as i8, vd)
}

fn vsrl_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b101000, vs2, vs1, vd)
}

fn vsrl_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b101000, vs2, rs1, vd)
}

fn vsrl_vi_type(vs2: u8, shamt: u8, vd: u8) -> u32 {
    vector_vi_type(0b101000, vs2, shamt as i8, vd)
}

fn vsra_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b101001, vs2, vs1, vd)
}

fn vsra_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b101001, vs2, rs1, vd)
}

fn vsra_vi_type(vs2: u8, shamt: u8, vd: u8) -> u32 {
    vector_vi_type(0b101001, vs2, shamt as i8, vd)
}

fn vminu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b000100, vs2, vs1, vd)
}

fn vminu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000100, vs2, rs1, vd)
}

fn vmin_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b000101, vs2, vs1, vd)
}

fn vmin_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000101, vs2, rs1, vd)
}

fn vmaxu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b000110, vs2, vs1, vd)
}

fn vmaxu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000110, vs2, rs1, vd)
}

fn vmax_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b000111, vs2, vs1, vd)
}

fn vmax_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b000111, vs2, rs1, vd)
}

fn vmul_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100101, vs2, vs1, vd)
}

fn vmul_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100101, vs2, rs1, vd)
}

fn vmulhu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100100, vs2, vs1, vd)
}

fn vmulhu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100100, vs2, rs1, vd)
}

fn vmulhsu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100110, vs2, vs1, vd)
}

fn vmulhsu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100110, vs2, rs1, vd)
}

fn vmulh_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100111, vs2, vs1, vd)
}

fn vmulh_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100111, vs2, rs1, vd)
}

fn vdivu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100000, vs2, vs1, vd)
}

fn vdivu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100000, vs2, rs1, vd)
}

fn vdiv_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100001, vs2, vs1, vd)
}

fn vdiv_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100001, vs2, rs1, vd)
}

fn vremu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100010, vs2, vs1, vd)
}

fn vremu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100010, vs2, rs1, vd)
}

fn vrem_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b100011, vs2, vs1, vd)
}

fn vrem_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b100011, vs2, rs1, vd)
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

fn bytes_with_u64(lanes: [u64; 2]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&lane.to_le_bytes());
    }
    bytes
}

#[test]
fn decoder_accepts_unmasked_vadd_vv() {
    assert_eq!(vadd_vv_type(2, 1, 3), 0x0220_81d7);
    assert_eq!(
        RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap(),
        RiscvInstruction::VectorAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vadd_vx_and_vi() {
    assert_eq!(vadd_vx_type(7, 8, 6), 0x0274_4357);
    assert_eq!(vadd_vi_type(5, 7, 4), 0x0253_b257);
    assert_eq!(
        RiscvInstruction::decode(vadd_vx_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorAddVx {
            vd: vreg(6),
            vs2: vreg(7),
            rs1: reg(8),
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vadd_vi_type(5, -1, 4)).unwrap(),
        RiscvInstruction::VectorAddVi {
            vd: vreg(4),
            vs2: vreg(5),
            imm: -1,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
}

#[test]
fn decoder_accepts_masked_vadd_forms() {
    assert_eq!(vadd_masked_vv_type(2, 1, 3), 0x0020_81d7);
    assert_eq!(vadd_masked_vx_type(7, 8, 6), 0x0074_4357);
    assert_eq!(vadd_masked_vi_type(5, -1, 4), 0x005f_b257);

    assert_eq!(
        RiscvInstruction::decode(vadd_masked_vv_type(2, 1, 3)).unwrap(),
        RiscvInstruction::VectorAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mask: RiscvVectorMaskMode::Masked,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vadd_masked_vx_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorAddVx {
            vd: vreg(6),
            vs2: vreg(7),
            rs1: reg(8),
            mask: RiscvVectorMaskMode::Masked,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vadd_masked_vi_type(5, -1, 4)).unwrap(),
        RiscvInstruction::VectorAddVi {
            vd: vreg(4),
            vs2: vreg(5),
            imm: -1,
            mask: RiscvVectorMaskMode::Masked,
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vsub_vv_and_vx() {
    assert_eq!(vsub_vv_type(7, 8, 6), 0x0a74_0357);
    assert_eq!(vsub_vx_type(5, 6, 4), 0x0a53_4257);
    assert_eq!(
        RiscvInstruction::decode(vsub_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorSubVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsub_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorSubVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vrsub_vx_and_vi() {
    assert_eq!(vrsub_vx_type(5, 6, 4), 0x0e53_4257);
    assert_eq!(vrsub_vi_type(5, -1, 4), 0x0e5f_b257);
    assert_eq!(
        RiscvInstruction::decode(vrsub_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorReverseSubVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vrsub_vi_type(5, -1, 4)).unwrap(),
        RiscvInstruction::VectorReverseSubVi {
            vd: vreg(4),
            vs2: vreg(5),
            imm: -1,
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vector_logical_operations() {
    assert_eq!(vand_vv_type(7, 8, 6), 0x2674_0357);
    assert_eq!(vand_vx_type(5, 6, 4), 0x2653_4257);
    assert_eq!(vand_vi_type(2, -1, 3), 0x262f_b1d7);
    assert_eq!(vor_vv_type(7, 8, 6), 0x2a74_0357);
    assert_eq!(vor_vx_type(5, 6, 4), 0x2a53_4257);
    assert_eq!(vor_vi_type(2, 7, 3), 0x2a23_b1d7);
    assert_eq!(vxor_vv_type(7, 8, 6), 0x2e74_0357);
    assert_eq!(vxor_vx_type(5, 6, 4), 0x2e53_4257);
    assert_eq!(vxor_vi_type(2, 15, 3), 0x2e27_b1d7);

    assert_eq!(
        RiscvInstruction::decode(vand_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorAndVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vand_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorAndVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vand_vi_type(2, -1, 3)).unwrap(),
        RiscvInstruction::VectorAndVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: -1,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vor_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorOrVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vor_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorOrVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vor_vi_type(2, 7, 3)).unwrap(),
        RiscvInstruction::VectorOrVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: 7,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vxor_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorXorVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vxor_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorXorVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vxor_vi_type(2, 15, 3)).unwrap(),
        RiscvInstruction::VectorXorVi {
            vd: vreg(3),
            vs2: vreg(2),
            imm: 15,
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vector_shift_operations() {
    assert_eq!(vsll_vv_type(7, 8, 6), 0x9674_0357);
    assert_eq!(vsll_vx_type(5, 6, 4), 0x9653_4257);
    assert_eq!(vsll_vi_type(2, 7, 3), 0x9623_b1d7);
    assert_eq!(vsrl_vv_type(7, 8, 6), 0xa274_0357);
    assert_eq!(vsrl_vx_type(5, 6, 4), 0xa253_4257);
    assert_eq!(vsrl_vi_type(2, 15, 3), 0xa227_b1d7);
    assert_eq!(vsra_vv_type(7, 8, 6), 0xa674_0357);
    assert_eq!(vsra_vx_type(5, 6, 4), 0xa653_4257);
    assert_eq!(vsra_vi_type(2, 31, 3), 0xa62f_b1d7);

    assert_eq!(
        RiscvInstruction::decode(vsll_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorShiftLeftLogicalVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsll_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorShiftLeftLogicalVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsll_vi_type(2, 7, 3)).unwrap(),
        RiscvInstruction::VectorShiftLeftLogicalVi {
            vd: vreg(3),
            vs2: vreg(2),
            shamt: 7,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsrl_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorShiftRightLogicalVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsrl_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorShiftRightLogicalVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsrl_vi_type(2, 15, 3)).unwrap(),
        RiscvInstruction::VectorShiftRightLogicalVi {
            vd: vreg(3),
            vs2: vreg(2),
            shamt: 15,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsra_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorShiftRightArithmeticVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsra_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorShiftRightArithmeticVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsra_vi_type(2, 31, 3)).unwrap(),
        RiscvInstruction::VectorShiftRightArithmeticVi {
            vd: vreg(3),
            vs2: vreg(2),
            shamt: 31,
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vector_minmax_operations() {
    assert_eq!(vminu_vv_type(7, 8, 6), 0x1274_0357);
    assert_eq!(vminu_vx_type(5, 6, 4), 0x1253_4257);
    assert_eq!(vmin_vv_type(7, 8, 6), 0x1674_0357);
    assert_eq!(vmin_vx_type(5, 6, 4), 0x1653_4257);
    assert_eq!(vmaxu_vv_type(7, 8, 6), 0x1a74_0357);
    assert_eq!(vmaxu_vx_type(5, 6, 4), 0x1a53_4257);
    assert_eq!(vmax_vv_type(7, 8, 6), 0x1e74_0357);
    assert_eq!(vmax_vx_type(5, 6, 4), 0x1e53_4257);

    assert_eq!(
        RiscvInstruction::decode(vminu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMinUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vminu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMinUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmin_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMinSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmin_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMinSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmaxu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaxUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmaxu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaxUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmax_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMaxSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmax_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMaxSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
}

#[test]
fn decoder_rejects_vector_minmax_immediate_forms() {
    assert!(RiscvInstruction::decode(vector_vi_type(0b000100, 2, 7, 3)).is_err());
    assert!(RiscvInstruction::decode(vector_vi_type(0b000101, 2, 7, 3)).is_err());
    assert!(RiscvInstruction::decode(vector_vi_type(0b000110, 2, 7, 3)).is_err());
    assert!(RiscvInstruction::decode(vector_vi_type(0b000111, 2, 7, 3)).is_err());
}

#[test]
fn decoder_accepts_unmasked_vector_multiply_operations() {
    assert_eq!(vmul_vv_type(7, 8, 6), 0x9674_2357);
    assert_eq!(vmul_vx_type(5, 6, 4), 0x9653_6257);
    assert_eq!(vmulhu_vv_type(7, 8, 6), 0x9274_2357);
    assert_eq!(vmulhu_vx_type(5, 6, 4), 0x9253_6257);
    assert_eq!(vmulhsu_vv_type(7, 8, 6), 0x9a74_2357);
    assert_eq!(vmulhsu_vx_type(5, 6, 4), 0x9a53_6257);
    assert_eq!(vmulh_vv_type(7, 8, 6), 0x9e74_2357);
    assert_eq!(vmulh_vx_type(5, 6, 4), 0x9e53_6257);

    assert_eq!(
        RiscvInstruction::decode(vmul_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMultiplyLowVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmul_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMultiplyLowVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmulhu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMultiplyHighUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmulhu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMultiplyHighUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmulhsu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmulhsu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmulh_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorMultiplyHighSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vmulh_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorMultiplyHighSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
}

#[test]
fn decoder_accepts_unmasked_vector_divide_remainder_operations() {
    assert_eq!(vdivu_vv_type(7, 8, 6), 0x8274_2357);
    assert_eq!(vdivu_vx_type(5, 6, 4), 0x8253_6257);
    assert_eq!(vdiv_vv_type(7, 8, 6), 0x8674_2357);
    assert_eq!(vdiv_vx_type(5, 6, 4), 0x8653_6257);
    assert_eq!(vremu_vv_type(7, 8, 6), 0x8a74_2357);
    assert_eq!(vremu_vx_type(5, 6, 4), 0x8a53_6257);
    assert_eq!(vrem_vv_type(7, 8, 6), 0x8e74_2357);
    assert_eq!(vrem_vx_type(5, 6, 4), 0x8e53_6257);

    assert_eq!(
        RiscvInstruction::decode(vdivu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorDivideUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vdivu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorDivideUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vdiv_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorDivideSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vdiv_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorDivideSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vremu_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorRemainderUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vremu_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorRemainderUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vrem_vv_type(7, 8, 6)).unwrap(),
        RiscvInstruction::VectorRemainderSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vrem_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorRemainderSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
}

#[test]
fn hart_executes_vadd_vv_for_active_u32_lanes() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write(reg(10), 3);
    hart.write_vector(vreg(1), lanes_u32([1, 2, u32::MAX, 40]));
    hart.write_vector(vreg(2), lanes_u32([10, 20, 2, 400]));
    hart.write_vector(
        vreg(3),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xdddd_dddd]),
    );

    hart.execute(RiscvInstruction::decode(vsetvli_type(0xd0, 10, 5)).unwrap())
        .unwrap();
    let record = hart
        .execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(3, 0xd0));
    assert_eq!(hart.pc(), 0x8008);
    assert_eq!(
        hart.read_vector(vreg(3)),
        lanes_u32([11, 22, 1, 0xdddd_dddd])
    );
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
}

#[test]
fn hart_executes_masked_vadd_vv_for_selected_u32_lanes() {
    let mut hart = RiscvHartState::new(0x8010);
    hart.write(reg(10), 4);
    hart.write_vector(
        vreg(0),
        [0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    hart.write_vector(vreg(1), lanes_u32([1, 2, 3, 4]));
    hart.write_vector(vreg(2), lanes_u32([10, 20, 30, 40]));
    hart.write_vector(
        vreg(3),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xaaaa_0003]),
    );

    hart.execute(RiscvInstruction::decode(vsetvli_type(0xd0, 10, 5)).unwrap())
        .unwrap();
    hart.execute(RiscvInstruction::decode(vadd_masked_vv_type(2, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        lanes_u32([11, 0xaaaa_0001, 33, 0xaaaa_0003])
    );
}

#[test]
fn hart_executes_vadd_vx_for_active_u32_lanes() {
    let mut hart = RiscvHartState::new(0x8050);
    hart.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    hart.write(reg(8), 10);
    hart.write_vector(vreg(2), lanes_u32([1, u32::MAX, 5, 40]));
    hart.write_vector(
        vreg(4),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xdddd_dddd]),
    );

    let record = hart
        .execute(RiscvInstruction::decode(vadd_vx_type(2, 8, 4)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(4)),
        lanes_u32([11, 9, 15, 0xdddd_dddd])
    );
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorAddVx {
            vd: vreg(4),
            vs2: vreg(2),
            rs1: reg(8),
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
}

#[test]
fn hart_executes_vadd_vi_with_signed_immediate() {
    let mut hart = RiscvHartState::new(0x8060);
    hart.set_vector_config(RiscvVectorConfig::new(3, 0xc8));
    hart.write_vector(
        vreg(5),
        bytes_with_u16([1, 0, u16::MAX, 40, 50, 60, 70, 80]),
    );
    hart.write_vector(vreg(6), bytes_with_u16([0xaaaa; 8]));

    let record = hart
        .execute(RiscvInstruction::decode(vadd_vi_type(5, -1, 6)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(6)),
        bytes_with_u16([
            0,
            u16::MAX,
            u16::MAX - 1,
            0xaaaa,
            0xaaaa,
            0xaaaa,
            0xaaaa,
            0xaaaa
        ])
    );
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorAddVi {
            vd: vreg(6),
            vs2: vreg(5),
            imm: -1,
            mask: RiscvVectorMaskMode::Unmasked,
        }
    );
}

#[test]
fn hart_executes_masked_vadd_vx_and_vi_for_selected_u32_lanes() {
    let mut hart = RiscvHartState::new(0x8068);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xd0));
    hart.write(reg(8), 10);
    hart.write_vector(
        vreg(0),
        [0b0000_1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    hart.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));
    hart.write_vector(vreg(4), lanes_u32([0xaaaa_0000; 4]));
    hart.write_vector(vreg(6), lanes_u32([0xbbbb_0000; 4]));

    hart.execute(RiscvInstruction::decode(vadd_masked_vx_type(2, 8, 4)).unwrap())
        .unwrap();
    hart.execute(RiscvInstruction::decode(vadd_masked_vi_type(2, -1, 6)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(4)),
        lanes_u32([0xaaaa_0000, 12, 0xaaaa_0000, 14])
    );
    assert_eq!(
        hart.read_vector(vreg(6)),
        lanes_u32([0xbbbb_0000, 1, 0xbbbb_0000, 3])
    );
}

#[test]
fn hart_executes_vsub_vv_for_active_u32_lanes() {
    let mut hart = RiscvHartState::new(0x8070);
    hart.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    hart.write_vector(vreg(7), lanes_u32([20, 0, 5, 40]));
    hart.write_vector(vreg(8), lanes_u32([3, 1, 7, 400]));
    hart.write_vector(
        vreg(6),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xdddd_dddd]),
    );

    let record = hart
        .execute(RiscvInstruction::decode(vsub_vv_type(7, 8, 6)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(6)),
        lanes_u32([17, u32::MAX, u32::MAX - 1, 0xdddd_dddd])
    );
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorSubVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
}

#[test]
fn hart_executes_vsub_vx_for_active_u16_lanes() {
    let mut hart = RiscvHartState::new(0x8080);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    hart.write(reg(6), 3);
    hart.write_vector(vreg(5), bytes_with_u16([10, 0, 3, 1, 50, 60, 70, 80]));
    hart.write_vector(vreg(4), bytes_with_u16([0xaaaa; 8]));

    let record = hart
        .execute(RiscvInstruction::decode(vsub_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(4)),
        bytes_with_u16([
            7,
            u16::MAX - 2,
            0,
            u16::MAX - 1,
            0xaaaa,
            0xaaaa,
            0xaaaa,
            0xaaaa
        ])
    );
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorSubVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );
}

#[test]
fn hart_executes_vrsub_vx_and_vi_for_active_lanes() {
    let mut vx = RiscvHartState::new(0x8088);
    vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vx.write(reg(6), 3);
    vx.write_vector(vreg(5), bytes_with_u16([10, 0, 3, 1, 50, 60, 70, 80]));
    vx.write_vector(vreg(4), bytes_with_u16([0xaaaa; 8]));

    let vx_record = vx
        .execute(RiscvInstruction::decode(vrsub_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        vx.read_vector(vreg(4)),
        bytes_with_u16([u16::MAX - 6, 3, 0, 2, 0xaaaa, 0xaaaa, 0xaaaa, 0xaaaa])
    );
    assert_eq!(
        vx_record.instruction(),
        RiscvInstruction::VectorReverseSubVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut vi = RiscvHartState::new(0x808c);
    vi.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    vi.write_vector(vreg(5), lanes_u32([0, 1, u32::MAX, u32::MAX - 1]));
    vi.write_vector(vreg(4), lanes_u32([0xaaaa_0000; 4]));

    let vi_record = vi
        .execute(RiscvInstruction::decode(vrsub_vi_type(5, -1, 4)).unwrap())
        .unwrap();

    assert_eq!(
        vi.read_vector(vreg(4)),
        lanes_u32([u32::MAX, u32::MAX - 1, 0, 0xaaaa_0000])
    );
    assert_eq!(
        vi_record.instruction(),
        RiscvInstruction::VectorReverseSubVi {
            vd: vreg(4),
            vs2: vreg(5),
            imm: -1,
        }
    );
}

#[test]
fn hart_executes_vector_logical_vv_vx_and_vi_forms() {
    let mut vv = RiscvHartState::new(0x8090);
    vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    vv.write_vector(
        vreg(2),
        lanes_u32([0b1100, 0xffff_0000, 0x1234_5678, 0xaaaa_aaaa]),
    );
    vv.write_vector(
        vreg(1),
        lanes_u32([0b1010, 0x00ff_00ff, 0xffff_0000, 0x5555_5555]),
    );
    vv.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let vv_record = vv
        .execute(RiscvInstruction::decode(vand_vv_type(2, 1, 4)).unwrap())
        .unwrap();
    assert_eq!(
        vv.read_vector(vreg(4)),
        lanes_u32([0b1000, 0x00ff_0000, 0x1234_0000, 0xdddd_dddd])
    );
    assert_eq!(
        vv_record.instruction(),
        RiscvInstruction::VectorAndVv {
            vd: vreg(4),
            vs1: vreg(1),
            vs2: vreg(2),
        }
    );

    let mut vx = RiscvHartState::new(0x80a0);
    vx.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    vx.write(reg(6), 0x00ff_000f);
    vx.write_vector(
        vreg(5),
        lanes_u32([0x1000_0000, 0, 0xf000_0000, 0xaaaa_aaaa]),
    );
    vx.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let vx_record = vx
        .execute(RiscvInstruction::decode(vor_vx_type(5, 6, 6)).unwrap())
        .unwrap();
    assert_eq!(
        vx.read_vector(vreg(6)),
        lanes_u32([0x10ff_000f, 0x00ff_000f, 0xf0ff_000f, 0xcccc_cccc])
    );
    assert_eq!(
        vx_record.instruction(),
        RiscvInstruction::VectorOrVx {
            vd: vreg(6),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut vi = RiscvHartState::new(0x80b0);
    vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vi.write_vector(
        vreg(7),
        bytes_with_u16([
            0x000f, 0x00f0, 0x0f00, 0xf000, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    vi.write_vector(vreg(8), bytes_with_u16([0xeeee; 8]));
    let vi_record = vi
        .execute(RiscvInstruction::decode(vxor_vi_type(7, -1, 8)).unwrap())
        .unwrap();
    assert_eq!(
        vi.read_vector(vreg(8)),
        bytes_with_u16([0xfff0, 0xff0f, 0xf0ff, 0x0fff, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vi_record.instruction(),
        RiscvInstruction::VectorXorVi {
            vd: vreg(8),
            vs2: vreg(7),
            imm: -1,
        }
    );
}

#[test]
fn hart_executes_vector_shift_vv_vx_and_vi_forms() {
    let mut vv = RiscvHartState::new(0x80c0);
    vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    vv.write_vector(vreg(7), lanes_u32([1, 0x8000_0000, 3, 0xaaaa_aaaa]));
    vv.write_vector(vreg(8), lanes_u32([1, 31, 32, 0]));
    vv.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let vv_record = vv
        .execute(RiscvInstruction::decode(vsll_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(vv.read_vector(vreg(6)), lanes_u32([2, 0, 3, 0xdddd_dddd]));
    assert_eq!(
        vv_record.instruction(),
        RiscvInstruction::VectorShiftLeftLogicalVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut vx = RiscvHartState::new(0x80d0);
    vx.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    vx.write(reg(6), 4);
    vx.write_vector(
        vreg(5),
        lanes_u32([0xffff_0000, 0x8000_0000, 0x0000_00f0, 0xaaaa_aaaa]),
    );
    vx.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let vx_record = vx
        .execute(RiscvInstruction::decode(vsrl_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        vx.read_vector(vreg(4)),
        lanes_u32([0x0fff_f000, 0x0800_0000, 0x0000_000f, 0xcccc_cccc])
    );
    assert_eq!(
        vx_record.instruction(),
        RiscvInstruction::VectorShiftRightLogicalVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut vi = RiscvHartState::new(0x80e0);
    vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vi.write_vector(
        vreg(2),
        bytes_with_u16([
            0x8000, 0x7ff0, 0xffff, 0x0008, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    vi.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let vi_record = vi
        .execute(RiscvInstruction::decode(vsra_vi_type(2, 3, 3)).unwrap())
        .unwrap();
    assert_eq!(
        vi.read_vector(vreg(3)),
        bytes_with_u16([0xf000, 0x0ffe, 0xffff, 0x0001, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vi_record.instruction(),
        RiscvInstruction::VectorShiftRightArithmeticVi {
            vd: vreg(3),
            vs2: vreg(2),
            shamt: 3,
        }
    );
}

#[test]
fn hart_executes_vector_minmax_vv_and_vx_forms() {
    let mut min_unsigned = RiscvHartState::new(0x80f0);
    min_unsigned.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    min_unsigned.write_vector(vreg(7), lanes_u32([1, u32::MAX, 0x8000_0000, 0xaaaa_aaaa]));
    min_unsigned.write_vector(vreg(8), lanes_u32([2, 7, 0x7fff_ffff, 0]));
    min_unsigned.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let min_unsigned_record = min_unsigned
        .execute(RiscvInstruction::decode(vminu_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        min_unsigned.read_vector(vreg(6)),
        lanes_u32([1, 7, 0x7fff_ffff, 0xdddd_dddd])
    );
    assert_eq!(
        min_unsigned_record.instruction(),
        RiscvInstruction::VectorMinUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut min_signed = RiscvHartState::new(0x8100);
    min_signed.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    min_signed.write(reg(6), u64::from(0x8001_u16));
    min_signed.write_vector(
        vreg(5),
        bytes_with_u16([
            0x0001, 0x7fff, 0x8000, 0xffff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    min_signed.write_vector(vreg(4), bytes_with_u16([0xeeee; 8]));
    let min_signed_record = min_signed
        .execute(RiscvInstruction::decode(vmin_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        min_signed.read_vector(vreg(4)),
        bytes_with_u16([0x8001, 0x8001, 0x8000, 0x8001, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        min_signed_record.instruction(),
        RiscvInstruction::VectorMinSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut max_unsigned = RiscvHartState::new(0x8110);
    max_unsigned.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    max_unsigned.write(reg(6), 0x7fff_ffff);
    max_unsigned.write_vector(vreg(5), lanes_u32([0, 0x8000_0000, u32::MAX, 0xaaaa_aaaa]));
    max_unsigned.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let max_unsigned_record = max_unsigned
        .execute(RiscvInstruction::decode(vmaxu_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        max_unsigned.read_vector(vreg(4)),
        lanes_u32([0x7fff_ffff, 0x8000_0000, u32::MAX, 0xcccc_cccc])
    );
    assert_eq!(
        max_unsigned_record.instruction(),
        RiscvInstruction::VectorMaxUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut max_signed = RiscvHartState::new(0x8120);
    max_signed.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    max_signed.write_vector(
        vreg(7),
        lanes_u32([0xffff_ffff, 0x8000_0000, 4, 0xaaaa_aaaa]),
    );
    max_signed.write_vector(vreg(8), lanes_u32([0, 0x7fff_ffff, 3, 0]));
    max_signed.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xbbbb_bbbb]));
    let max_signed_record = max_signed
        .execute(RiscvInstruction::decode(vmax_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        max_signed.read_vector(vreg(6)),
        lanes_u32([0, 0x7fff_ffff, 4, 0xbbbb_bbbb])
    );
    assert_eq!(
        max_signed_record.instruction(),
        RiscvInstruction::VectorMaxSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );
}

#[test]
fn hart_executes_vector_multiply_vv_and_vx_forms() {
    let mut low = RiscvHartState::new(0x8130);
    low.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    low.write_vector(vreg(7), lanes_u32([3, u32::MAX, 0x8000_0000, 0xaaaa_aaaa]));
    low.write_vector(vreg(8), lanes_u32([7, 2, 2, 0]));
    low.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let low_record = low
        .execute(RiscvInstruction::decode(vmul_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        low.read_vector(vreg(6)),
        lanes_u32([21, u32::MAX - 1, 0, 0xdddd_dddd])
    );
    assert_eq!(
        low_record.instruction(),
        RiscvInstruction::VectorMultiplyLowVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut high_unsigned = RiscvHartState::new(0x8140);
    high_unsigned.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    high_unsigned.write(reg(6), 0x0001_0000);
    high_unsigned.write_vector(
        vreg(5),
        lanes_u32([u32::MAX, 0x8000_0000, 0x0002_0000, 0xaaaa_aaaa]),
    );
    high_unsigned.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let high_unsigned_record = high_unsigned
        .execute(RiscvInstruction::decode(vmulhu_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        high_unsigned.read_vector(vreg(4)),
        lanes_u32([0x0000_ffff, 0x0000_8000, 2, 0xcccc_cccc])
    );
    assert_eq!(
        high_unsigned_record.instruction(),
        RiscvInstruction::VectorMultiplyHighUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut high_signed = RiscvHartState::new(0x8150);
    high_signed.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    high_signed.write(reg(6), u64::from(0xfffe_u16));
    high_signed.write_vector(
        vreg(5),
        bytes_with_u16([
            0x8000, 0x7fff, 0xffff, 0x0002, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    high_signed.write_vector(vreg(4), bytes_with_u16([0xeeee; 8]));
    let high_signed_record = high_signed
        .execute(RiscvInstruction::decode(vmulh_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        high_signed.read_vector(vreg(4)),
        bytes_with_u16([0x0001, 0xffff, 0x0000, 0xffff, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        high_signed_record.instruction(),
        RiscvInstruction::VectorMultiplyHighSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut high_signed_unsigned = RiscvHartState::new(0x8160);
    high_signed_unsigned.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    high_signed_unsigned.write_vector(
        vreg(7),
        bytes_with_u16([
            0xffff, 0x8000, 0x7fff, 0xfffe, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    high_signed_unsigned.write_vector(vreg(8), bytes_with_u16([2, 2, 2, 0xffff, 0, 0, 0, 0]));
    high_signed_unsigned.write_vector(vreg(6), bytes_with_u16([0xeeee; 8]));
    let high_signed_unsigned_record = high_signed_unsigned
        .execute(RiscvInstruction::decode(vmulhsu_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        high_signed_unsigned.read_vector(vreg(6)),
        bytes_with_u16([0xffff, 0xffff, 0x0000, 0xfffe, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        high_signed_unsigned_record.instruction(),
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut high_signed_unsigned_vx_e8 = RiscvHartState::new(0x8168);
    high_signed_unsigned_vx_e8.set_vector_config(RiscvVectorConfig::new(4, 0xc0));
    high_signed_unsigned_vx_e8.write(reg(6), 0x1ff);
    high_signed_unsigned_vx_e8.write_vector(
        vreg(5),
        [
            0xff, 0x80, 0x7f, 0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77,
        ],
    );
    high_signed_unsigned_vx_e8.write_vector(vreg(4), [0x99; 16]);
    let high_signed_unsigned_vx_e8_record = high_signed_unsigned_vx_e8
        .execute(RiscvInstruction::decode(vmulhsu_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        high_signed_unsigned_vx_e8.read_vector(vreg(4)),
        [
            0xff, 0x80, 0x7e, 0x01, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99,
        ]
    );
    assert_eq!(
        high_signed_unsigned_vx_e8_record.instruction(),
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut high_unsigned_e64 = RiscvHartState::new(0x8170);
    high_unsigned_e64.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    high_unsigned_e64.write_vector(vreg(7), bytes_with_u64([u64::MAX, 1_u64 << 63]));
    high_unsigned_e64.write_vector(vreg(8), bytes_with_u64([2, 2]));
    high_unsigned_e64.write_vector(vreg(6), bytes_with_u64([0, 0]));
    high_unsigned_e64
        .execute(RiscvInstruction::decode(vmulhu_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        high_unsigned_e64.read_vector(vreg(6)),
        bytes_with_u64([1, 1])
    );

    let mut high_signed_e64 = RiscvHartState::new(0x8180);
    high_signed_e64.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    high_signed_e64.write(reg(6), u64::MAX - 1);
    high_signed_e64.write_vector(vreg(5), bytes_with_u64([i64::MAX as u64, 1_u64 << 63]));
    high_signed_e64.write_vector(vreg(4), bytes_with_u64([0, 0]));
    high_signed_e64
        .execute(RiscvInstruction::decode(vmulh_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        high_signed_e64.read_vector(vreg(4)),
        bytes_with_u64([u64::MAX, 1])
    );

    let mut high_signed_unsigned_e64 = RiscvHartState::new(0x8190);
    high_signed_unsigned_e64.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    high_signed_unsigned_e64.write_vector(vreg(7), bytes_with_u64([u64::MAX - 1, i64::MAX as u64]));
    high_signed_unsigned_e64.write_vector(vreg(8), bytes_with_u64([u64::MAX, 2]));
    high_signed_unsigned_e64.write_vector(vreg(6), bytes_with_u64([0, 0]));
    high_signed_unsigned_e64
        .execute(RiscvInstruction::decode(vmulhsu_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        high_signed_unsigned_e64.read_vector(vreg(6)),
        bytes_with_u64([u64::MAX - 1, 0])
    );
}

#[test]
fn hart_executes_vector_divide_remainder_vv_and_vx_forms() {
    let mut div_unsigned_vv = RiscvHartState::new(0x81a0);
    div_unsigned_vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    div_unsigned_vv.write_vector(vreg(7), lanes_u32([20, 7, 0x8000_0000, 0xaaaa_aaaa]));
    div_unsigned_vv.write_vector(vreg(8), lanes_u32([3, 0, 2, 1]));
    div_unsigned_vv.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let div_unsigned_vv_record = div_unsigned_vv
        .execute(RiscvInstruction::decode(vdivu_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        div_unsigned_vv.read_vector(vreg(6)),
        lanes_u32([6, u32::MAX, 0x4000_0000, 0xdddd_dddd])
    );
    assert_eq!(
        div_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorDivideUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut div_unsigned_vx_e8 = RiscvHartState::new(0x81b0);
    div_unsigned_vx_e8.set_vector_config(RiscvVectorConfig::new(4, 0xc0));
    div_unsigned_vx_e8.write(reg(6), 0x102);
    div_unsigned_vx_e8.write_vector(
        vreg(5),
        [
            9, 0, 255, 4, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        ],
    );
    div_unsigned_vx_e8.write_vector(vreg(4), [0x99; 16]);
    div_unsigned_vx_e8
        .execute(RiscvInstruction::decode(vdivu_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        div_unsigned_vx_e8.read_vector(vreg(4)),
        [4, 0, 127, 2, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,]
    );

    let mut div_signed_vv = RiscvHartState::new(0x81c0);
    div_signed_vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    div_signed_vv.write_vector(
        vreg(7),
        lanes_u32([(-9_i32) as u32, 9, i32::MIN as u32, 0xaaaa_aaaa]),
    );
    div_signed_vv.write_vector(vreg(8), lanes_u32([2, 0, (-1_i32) as u32, 1]));
    div_signed_vv.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xdddd_dddd]));
    let div_signed_vv_record = div_signed_vv
        .execute(RiscvInstruction::decode(vdiv_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        div_signed_vv.read_vector(vreg(6)),
        lanes_u32([(-4_i32) as u32, u32::MAX, i32::MIN as u32, 0xdddd_dddd])
    );
    assert_eq!(
        div_signed_vv_record.instruction(),
        RiscvInstruction::VectorDivideSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut div_signed_vx = RiscvHartState::new(0x81d0);
    div_signed_vx.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    div_signed_vx.write(reg(6), (-2_i32) as u32 as u64);
    div_signed_vx.write_vector(
        vreg(5),
        lanes_u32([(-9_i32) as u32, 9, i32::MIN as u32, 0xaaaa_aaaa]),
    );
    div_signed_vx.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xcccc_cccc]));
    let div_signed_vx_record = div_signed_vx
        .execute(RiscvInstruction::decode(vdiv_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        div_signed_vx.read_vector(vreg(4)),
        lanes_u32([4, (-4_i32) as u32, 0x4000_0000, 0xcccc_cccc])
    );
    assert_eq!(
        div_signed_vx_record.instruction(),
        RiscvInstruction::VectorDivideSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut rem_unsigned_vv = RiscvHartState::new(0x81e0);
    rem_unsigned_vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    rem_unsigned_vv.write_vector(vreg(7), lanes_u32([20, 7, 0x8000_0001, 0xaaaa_aaaa]));
    rem_unsigned_vv.write_vector(vreg(8), lanes_u32([3, 0, 2, 1]));
    rem_unsigned_vv.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xbbbb_bbbb]));
    let rem_unsigned_vv_record = rem_unsigned_vv
        .execute(RiscvInstruction::decode(vremu_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        rem_unsigned_vv.read_vector(vreg(6)),
        lanes_u32([2, 7, 1, 0xbbbb_bbbb])
    );
    assert_eq!(
        rem_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorRemainderUnsignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut rem_unsigned_vx = RiscvHartState::new(0x81f0);
    rem_unsigned_vx.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    rem_unsigned_vx.write(reg(6), 3);
    rem_unsigned_vx.write_vector(vreg(5), lanes_u32([20, 7, 0x8000_0001, 0xaaaa_aaaa]));
    rem_unsigned_vx.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xbbbb_bbbb]));
    let rem_unsigned_vx_record = rem_unsigned_vx
        .execute(RiscvInstruction::decode(vremu_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        rem_unsigned_vx.read_vector(vreg(4)),
        lanes_u32([2, 1, 0, 0xbbbb_bbbb])
    );
    assert_eq!(
        rem_unsigned_vx_record.instruction(),
        RiscvInstruction::VectorRemainderUnsignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut rem_signed_vv = RiscvHartState::new(0x8200);
    rem_signed_vv.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    rem_signed_vv.write_vector(
        vreg(7),
        lanes_u32([(-9_i32) as u32, 9, i32::MIN as u32, 0xaaaa_aaaa]),
    );
    rem_signed_vv.write_vector(vreg(8), lanes_u32([2, 0, (-1_i32) as u32, 1]));
    rem_signed_vv.write_vector(vreg(6), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    let rem_signed_vv_record = rem_signed_vv
        .execute(RiscvInstruction::decode(vrem_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        rem_signed_vv.read_vector(vreg(6)),
        lanes_u32([(-1_i32) as u32, 9, 0, 0xeeee_eeee])
    );
    assert_eq!(
        rem_signed_vv_record.instruction(),
        RiscvInstruction::VectorRemainderSignedVv {
            vd: vreg(6),
            vs1: vreg(8),
            vs2: vreg(7),
        }
    );

    let mut rem_signed_vx = RiscvHartState::new(0x8210);
    rem_signed_vx.set_vector_config(RiscvVectorConfig::new(3, 0xd0));
    rem_signed_vx.write(reg(6), (-2_i32) as u32 as u64);
    rem_signed_vx.write_vector(
        vreg(5),
        lanes_u32([(-9_i32) as u32, 9, i32::MIN as u32, 0xaaaa_aaaa]),
    );
    rem_signed_vx.write_vector(vreg(4), lanes_u32([0, 0, 0, 0xeeee_eeee]));
    let rem_signed_vx_record = rem_signed_vx
        .execute(RiscvInstruction::decode(vrem_vx_type(5, 6, 4)).unwrap())
        .unwrap();
    assert_eq!(
        rem_signed_vx.read_vector(vreg(4)),
        lanes_u32([(-1_i32) as u32, 1, 0, 0xeeee_eeee])
    );
    assert_eq!(
        rem_signed_vx_record.instruction(),
        RiscvInstruction::VectorRemainderSignedVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        }
    );

    let mut div_signed_e8 = RiscvHartState::new(0x8220);
    div_signed_e8.set_vector_config(RiscvVectorConfig::new(4, 0xc0));
    div_signed_e8.write_vector(
        vreg(7),
        [
            0x80, 0x7f, 0xff, 0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77,
        ],
    );
    div_signed_e8.write_vector(
        vreg(8),
        [0xff, 0x00, 0x02, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    div_signed_e8.write_vector(vreg(6), [0x99; 16]);
    div_signed_e8
        .execute(RiscvInstruction::decode(vdiv_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        div_signed_e8.read_vector(vreg(6)),
        [
            0x80, 0xff, 0x00, 0xfe, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
            0x99, 0x99,
        ]
    );

    let mut rem_signed_e16 = RiscvHartState::new(0x8230);
    rem_signed_e16.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    rem_signed_e16.write_vector(
        vreg(7),
        bytes_with_u16([
            0x8000, 0x7fff, 0xffff, 0x0005, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    rem_signed_e16.write_vector(
        vreg(8),
        bytes_with_u16([0xffff, 0x0000, 0x0002, 0xfffe, 0, 0, 0, 0]),
    );
    rem_signed_e16.write_vector(vreg(6), bytes_with_u16([0xeeee; 8]));
    rem_signed_e16
        .execute(RiscvInstruction::decode(vrem_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        rem_signed_e16.read_vector(vreg(6)),
        bytes_with_u16([0x0000, 0x7fff, 0xffff, 0x0001, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );

    let mut div_signed_e64 = RiscvHartState::new(0x8240);
    div_signed_e64.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    div_signed_e64.write_vector(vreg(7), bytes_with_u64([i64::MIN as u64, 123]));
    div_signed_e64.write_vector(vreg(8), bytes_with_u64([u64::MAX, 0]));
    div_signed_e64.write_vector(vreg(6), bytes_with_u64([0, 0]));
    div_signed_e64
        .execute(RiscvInstruction::decode(vdiv_vv_type(7, 8, 6)).unwrap())
        .unwrap();
    assert_eq!(
        div_signed_e64.read_vector(vreg(6)),
        bytes_with_u64([i64::MIN as u64, u64::MAX])
    );
}

#[test]
fn hart_executes_vadd_vv_for_configured_element_widths() {
    let mut e8 = RiscvHartState::new(0x8100);
    e8.set_vector_config(RiscvVectorConfig::new(5, 0xc0));
    e8.write_vector(
        vreg(1),
        [255, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    );
    e8.write_vector(
        vreg(2),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    );
    e8.write_vector(vreg(3), [0xee; 16]);
    e8.execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        e8.read_vector(vreg(3)),
        [0, 3, 5, 7, 9, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee]
    );

    let mut e16 = RiscvHartState::new(0x8200);
    e16.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    e16.write_vector(
        vreg(1),
        bytes_with_u16([u16::MAX, 10, 30, 40, 50, 60, 70, 80]),
    );
    e16.write_vector(
        vreg(2),
        bytes_with_u16([2, 20, 300, 400, 500, 600, 700, 800]),
    );
    e16.write_vector(vreg(3), bytes_with_u16([0xbbbb; 8]));
    e16.execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        e16.read_vector(vreg(3)),
        bytes_with_u16([1, 30, 0xbbbb, 0xbbbb, 0xbbbb, 0xbbbb, 0xbbbb, 0xbbbb])
    );

    let mut e64 = RiscvHartState::new(0x8300);
    e64.set_vector_config(RiscvVectorConfig::new(1, 0xd8));
    e64.write_vector(vreg(1), bytes_with_u64([u64::MAX, 10]));
    e64.write_vector(vreg(2), bytes_with_u64([3, 20]));
    e64.write_vector(
        vreg(3),
        bytes_with_u64([0xaaaa_aaaa_aaaa_aaaa, 0xbbbb_bbbb_bbbb_bbbb]),
    );
    e64.execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        e64.read_vector(vreg(3)),
        bytes_with_u64([2, 0xbbbb_bbbb_bbbb_bbbb])
    );
}

#[test]
fn hart_executes_vadd_vv_across_lmul2_register_group() {
    let mut hart = RiscvHartState::new(0x8400);
    hart.set_vector_config(RiscvVectorConfig::new(6, 0xd1));
    hart.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));
    hart.write_vector(vreg(3), lanes_u32([5, 6, 7, 8]));
    hart.write_vector(vreg(4), lanes_u32([10, 20, 30, 40]));
    hart.write_vector(vreg(5), lanes_u32([50, 60, 70, 80]));
    hart.write_vector(
        vreg(6),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xaaaa_0003]),
    );
    hart.write_vector(
        vreg(7),
        lanes_u32([0xbbbb_0000, 0xbbbb_0001, 0xbbbb_0002, 0xbbbb_0003]),
    );

    hart.execute(RiscvInstruction::decode(vadd_vv_type(4, 2, 6)).unwrap())
        .unwrap();

    assert_eq!(hart.read_vector(vreg(6)), lanes_u32([11, 22, 33, 44]));
    assert_eq!(
        hart.read_vector(vreg(7)),
        lanes_u32([55, 66, 0xbbbb_0002, 0xbbbb_0003])
    );
}

#[test]
fn hart_executes_vsub_vv_across_lmul2_register_group() {
    let mut hart = RiscvHartState::new(0x8450);
    hart.set_vector_config(RiscvVectorConfig::new(6, 0xd1));
    hart.write_vector(vreg(2), lanes_u32([20, 0, 3, 4]));
    hart.write_vector(vreg(3), lanes_u32([5, 6, 7, 8]));
    hart.write_vector(vreg(4), lanes_u32([10, 1, 30, 40]));
    hart.write_vector(vreg(5), lanes_u32([50, 10, 9, 80]));
    hart.write_vector(
        vreg(6),
        lanes_u32([0xaaaa_0000, 0xaaaa_0001, 0xaaaa_0002, 0xaaaa_0003]),
    );
    hart.write_vector(
        vreg(7),
        lanes_u32([0xbbbb_0000, 0xbbbb_0001, 0xbbbb_0002, 0xbbbb_0003]),
    );

    hart.execute(RiscvInstruction::decode(vsub_vv_type(2, 4, 6)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(6)),
        lanes_u32([10, u32::MAX, u32::MAX - 26, u32::MAX - 35])
    );
    assert_eq!(
        hart.read_vector(vreg(7)),
        lanes_u32([u32::MAX - 44, u32::MAX - 3, 0xbbbb_0002, 0xbbbb_0003])
    );
}

#[test]
fn hart_traps_vadd_vv_for_unaligned_lmul2_register_group() {
    let mut hart = RiscvHartState::new(0x8500);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd1));
    hart.write_vector(
        vreg(3),
        lanes_u32([0xcccc_0000, 0xcccc_0001, 0xcccc_0002, 0xcccc_0003]),
    );

    let record = hart
        .execute(RiscvInstruction::decode(vadd_vv_type(4, 2, 3)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8500))
    );
    assert_eq!(
        hart.read_vector(vreg(3)),
        lanes_u32([0xcccc_0000, 0xcccc_0001, 0xcccc_0002, 0xcccc_0003])
    );
}

#[test]
fn hart_traps_masked_vadd_vv_when_destination_is_v0() {
    let mut hart = RiscvHartState::new(0x8510);
    hart.set_vector_config(RiscvVectorConfig::new(2, 0xd0));
    hart.write_vector(
        vreg(0),
        [0b0000_0011, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    hart.write_vector(vreg(1), lanes_u32([1, 2, 3, 4]));
    hart.write_vector(vreg(2), lanes_u32([10, 20, 30, 40]));

    let record = hart
        .execute(RiscvInstruction::decode(vadd_masked_vv_type(2, 1, 0)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8510))
    );
    assert_eq!(
        hart.read_vector(vreg(0)),
        [0b0000_0011, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn hart_traps_vadd_vv_when_vector_type_is_invalid() {
    let mut hart = RiscvHartState::new(0x9000);

    let record = hart
        .execute(RiscvInstruction::decode(vadd_vv_type(2, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x9000))
    );
    assert_eq!(hart.pc(), 0);
}
