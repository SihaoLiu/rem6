use rem6_isa_riscv::{
    Register, RegisterWrite, RiscvError, RiscvHartState, RiscvInstruction, RiscvTrap,
    RiscvTrapKind, RiscvVectorAveragingInstruction, RiscvVectorConfig,
    RiscvVectorFixedPointShiftInstruction, RiscvVectorFixedPointState,
    RiscvVectorFixedRoundingMode, RiscvVectorGatherInstruction, RiscvVectorMaskIndexInstruction,
    RiscvVectorMaskMode, RiscvVectorMaskPrefixInstruction, RiscvVectorMaskReductionInstruction,
    RiscvVectorReductionInstruction, RiscvVectorSaturatingInstruction, RiscvVectorSlideInstruction,
    RiscvVectorWideningIntegerInstruction, VectorRegister,
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

fn vslideup_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001110, vs2, rs1, vd)
}

fn vslidedown_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001111, vs2, rs1, vd)
}

fn vslideup_vi_type(vs2: u8, offset: u8, vd: u8) -> u32 {
    vector_vi_type(0b001110, vs2, offset as i8, vd)
}

fn vslidedown_vi_type(vs2: u8, offset: u8, vd: u8) -> u32 {
    vector_vi_type(0b001111, vs2, offset as i8, vd)
}

fn vslide1up_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b001110, vs2, rs1, vd)
}

fn vslide1down_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b001111, vs2, rs1, vd)
}

fn vrgather_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b001100, vs2, vs1, vd)
}

fn vrgather_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b001100, vs2, rs1, vd)
}

fn vrgather_vi_type(vs2: u8, imm: u8, vd: u8) -> u32 {
    vector_vi_type(0b001100, vs2, imm as i8, vd)
}

fn vcpop_m_type(vs2: u8, rd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x10, vs2, 0x10, rd, mask)
}

fn vfirst_m_type(vs2: u8, rd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x10, vs2, 0x11, rd, mask)
}

fn vmsbf_m_type(vs2: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x14, vs2, 0x01, vd, mask)
}

fn vmsof_m_type(vs2: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x14, vs2, 0x02, vd, mask)
}

fn vmsif_m_type(vs2: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x14, vs2, 0x03, vd, mask)
}

fn viota_m_type(vs2: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x14, vs2, 0x10, vd, mask)
}

fn vid_v_type(vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_mask_reduction_type(0x14, 0, 0x11, vd, mask)
}

fn vector_mask_reduction_type(
    funct6: u32,
    vs2: u8,
    vs1: u8,
    rd: u8,
    mask: RiscvVectorMaskMode,
) -> u32 {
    (funct6 << 26)
        | (u32::from(matches!(mask, RiscvVectorMaskMode::Unmasked)) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(rd) << 7)
        | 0x57
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

fn vector_widening_vv_type(
    funct6: u32,
    vs2: u8,
    vs1: u8,
    vd: u8,
    mask: RiscvVectorMaskMode,
) -> u32 {
    (funct6 << 26)
        | (u32::from(matches!(mask, RiscvVectorMaskMode::Unmasked)) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_widening_vx_type(
    funct6: u32,
    vs2: u8,
    rs1: u8,
    vd: u8,
    mask: RiscvVectorMaskMode,
) -> u32 {
    (funct6 << 26)
        | (u32::from(matches!(mask, RiscvVectorMaskMode::Unmasked)) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b110 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_reduction_type(funct6: u32, vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    (funct6 << 26)
        | (u32::from(matches!(mask, RiscvVectorMaskMode::Unmasked)) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_widening_reduction_type(
    funct6: u32,
    vs2: u8,
    vs1: u8,
    vd: u8,
    mask: RiscvVectorMaskMode,
) -> u32 {
    (funct6 << 26)
        | (u32::from(matches!(mask, RiscvVectorMaskMode::Unmasked)) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
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

fn vssrl_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b101010, vs2, vs1, vd)
}

fn vssrl_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b101010, vs2, rs1, vd)
}

fn vssrl_vi_type(vs2: u8, shamt: u8, vd: u8) -> u32 {
    vector_vi_type(0b101010, vs2, shamt as i8, vd)
}

fn vssra_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b101011, vs2, vs1, vd)
}

fn vssra_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b101011, vs2, rs1, vd)
}

fn vssra_vi_type(vs2: u8, shamt: u8, vd: u8) -> u32 {
    vector_vi_type(0b101011, vs2, shamt as i8, vd)
}

fn vsmul_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100111, vs2, vs1, vd)
}

fn vsmul_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b100111, vs2, rs1, vd)
}

fn vredsum_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000000, vs2, vs1, vd, mask)
}

fn vredand_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000001, vs2, vs1, vd, mask)
}

fn vredor_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000010, vs2, vs1, vd, mask)
}

fn vredxor_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000011, vs2, vs1, vd, mask)
}

fn vredminu_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000100, vs2, vs1, vd, mask)
}

fn vredmin_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000101, vs2, vs1, vd, mask)
}

fn vredmaxu_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000110, vs2, vs1, vd, mask)
}

fn vredmax_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_reduction_type(0b000111, vs2, vs1, vd, mask)
}

fn vwredsumu_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_reduction_type(0b110000, vs2, vs1, vd, mask)
}

fn vwredsum_vs_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_reduction_type(0b110001, vs2, vs1, vd, mask)
}

fn vwaddu_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110000, vs2, vs1, vd, mask)
}

fn vwadd_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110001, vs2, vs1, vd, mask)
}

fn vwsubu_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110010, vs2, vs1, vd, mask)
}

fn vwsub_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110011, vs2, vs1, vd, mask)
}

fn vwaddu_wv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110100, vs2, vs1, vd, mask)
}

fn vwadd_wv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110101, vs2, vs1, vd, mask)
}

fn vwsubu_wv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110110, vs2, vs1, vd, mask)
}

fn vwsub_wv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b110111, vs2, vs1, vd, mask)
}

fn vwaddu_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110000, vs2, rs1, vd, mask)
}

fn vwadd_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110001, vs2, rs1, vd, mask)
}

fn vwsubu_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110010, vs2, rs1, vd, mask)
}

fn vwsub_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110011, vs2, rs1, vd, mask)
}

fn vwaddu_wx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110100, vs2, rs1, vd, mask)
}

fn vwadd_wx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110101, vs2, rs1, vd, mask)
}

fn vwsubu_wx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110110, vs2, rs1, vd, mask)
}

fn vwsub_wx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b110111, vs2, rs1, vd, mask)
}

fn vwmulu_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b111000, vs2, vs1, vd, mask)
}

fn vwmulsu_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b111010, vs2, vs1, vd, mask)
}

fn vwmul_vv_type(vs2: u8, vs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vv_type(0b111011, vs2, vs1, vd, mask)
}

fn vwmulu_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b111000, vs2, rs1, vd, mask)
}

fn vwmulsu_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b111010, vs2, rs1, vd, mask)
}

fn vwmul_vx_type(vs2: u8, rs1: u8, vd: u8, mask: RiscvVectorMaskMode) -> u32 {
    vector_widening_vx_type(0b111011, vs2, rs1, vd, mask)
}

fn vaaddu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b001000, vs2, vs1, vd)
}

fn vaaddu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b001000, vs2, rs1, vd)
}

fn vaadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b001001, vs2, vs1, vd)
}

fn vaadd_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b001001, vs2, rs1, vd)
}

fn vasubu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b001010, vs2, vs1, vd)
}

fn vasubu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b001010, vs2, rs1, vd)
}

fn vasub_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(0b001011, vs2, vs1, vd)
}

fn vasub_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(0b001011, vs2, rs1, vd)
}

fn vsaddu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100000, vs2, vs1, vd)
}

fn vsaddu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b100000, vs2, rs1, vd)
}

fn vsaddu_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b100000, vs2, imm, vd)
}

fn vsadd_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100001, vs2, vs1, vd)
}

fn vsadd_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b100001, vs2, rs1, vd)
}

fn vsadd_vi_type(vs2: u8, imm: i8, vd: u8) -> u32 {
    vector_vi_type(0b100001, vs2, imm, vd)
}

fn vssubu_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100010, vs2, vs1, vd)
}

fn vssubu_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b100010, vs2, rs1, vd)
}

fn vssub_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(0b100011, vs2, vs1, vd)
}

fn vssub_vx_type(vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b100011, vs2, rs1, vd)
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

fn lanes_u64(lanes: [u64; 2]) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (index, lane) in lanes.into_iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&lane.to_le_bytes());
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

fn mask_bytes(mask: u8) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = mask;
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
fn decoder_accepts_unmasked_vector_slide_vx_and_vi() {
    assert_eq!(vslideup_vx_type(5, 6, 4), 0x3a53_4257);
    assert_eq!(vslidedown_vx_type(5, 6, 4), 0x3e53_4257);
    assert_eq!(vslideup_vi_type(5, 2, 4), 0x3a51_3257);
    assert_eq!(vslidedown_vi_type(5, 6, 4), 0x3e53_3257);
    assert_eq!(
        RiscvInstruction::decode(vslideup_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::UpVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vslidedown_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::DownVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vslideup_vi_type(5, 2, 4)).unwrap(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::UpVi {
            vd: vreg(4),
            vs2: vreg(5),
            offset: 2,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vslidedown_vi_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::DownVi {
            vd: vreg(4),
            vs2: vreg(5),
            offset: 6,
        })
    );
}

#[test]
fn decoder_accepts_unmasked_vector_slide1_vx() {
    assert_eq!(vslide1up_vx_type(5, 6, 4), 0x3a53_6257);
    assert_eq!(vslide1down_vx_type(5, 6, 4), 0x3e53_6257);
    assert_eq!(
        RiscvInstruction::decode(vslide1up_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::OneUpVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vslide1down_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::OneDownVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );
}

#[test]
fn decoder_accepts_unmasked_vector_gather_vv_vx_and_vi() {
    assert_eq!(vrgather_vv_type(5, 6, 4), 0x3253_0257);
    assert_eq!(vrgather_vx_type(5, 6, 4), 0x3253_4257);
    assert_eq!(vrgather_vi_type(5, 2, 4), 0x3251_3257);
    assert_eq!(
        RiscvInstruction::decode(vrgather_vv_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorGather(RiscvVectorGatherInstruction::Vv {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vrgather_vx_type(5, 6, 4)).unwrap(),
        RiscvInstruction::VectorGather(RiscvVectorGatherInstruction::Vx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vrgather_vi_type(5, 2, 4)).unwrap(),
        RiscvInstruction::VectorGather(RiscvVectorGatherInstruction::Vi {
            vd: vreg(4),
            vs2: vreg(5),
            index: 2,
        })
    );
}

#[test]
fn decoder_accepts_vector_mask_population_and_first_set_reductions() {
    assert_eq!(
        vcpop_m_type(6, 5, RiscvVectorMaskMode::Unmasked),
        0x4268_22d7
    );
    assert_eq!(vcpop_m_type(6, 5, RiscvVectorMaskMode::Masked), 0x4068_22d7);
    assert_eq!(
        vfirst_m_type(7, 8, RiscvVectorMaskMode::Unmasked),
        0x4278_a457
    );
    assert_eq!(
        RiscvInstruction::decode(vcpop_m_type(6, 5, RiscvVectorMaskMode::Unmasked)).unwrap(),
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::PopCount {
            rd: reg(5),
            vs2: vreg(6),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vcpop_m_type(6, 5, RiscvVectorMaskMode::Masked)).unwrap(),
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::PopCount {
            rd: reg(5),
            vs2: vreg(6),
            mask: RiscvVectorMaskMode::Masked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vfirst_m_type(7, 8, RiscvVectorMaskMode::Unmasked)).unwrap(),
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::FirstSet {
            rd: reg(8),
            vs2: vreg(7),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
}

#[test]
fn decoder_accepts_vector_mask_prefix_operations() {
    assert_eq!(
        vmsbf_m_type(6, 5, RiscvVectorMaskMode::Unmasked),
        0x5260_a2d7
    );
    assert_eq!(vmsbf_m_type(6, 5, RiscvVectorMaskMode::Masked), 0x5060_a2d7);
    assert_eq!(
        vmsof_m_type(7, 8, RiscvVectorMaskMode::Unmasked),
        0x5271_2457
    );
    assert_eq!(vmsif_m_type(7, 9, RiscvVectorMaskMode::Masked), 0x5071_a4d7);
    assert_eq!(
        RiscvInstruction::decode(vmsbf_m_type(6, 5, RiscvVectorMaskMode::Unmasked)).unwrap(),
        RiscvInstruction::VectorMaskPrefix(RiscvVectorMaskPrefixInstruction::BeforeFirst {
            vd: vreg(5),
            vs2: vreg(6),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vmsof_m_type(7, 8, RiscvVectorMaskMode::Unmasked)).unwrap(),
        RiscvInstruction::VectorMaskPrefix(RiscvVectorMaskPrefixInstruction::OnlyFirst {
            vd: vreg(8),
            vs2: vreg(7),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vmsif_m_type(7, 9, RiscvVectorMaskMode::Masked)).unwrap(),
        RiscvInstruction::VectorMaskPrefix(RiscvVectorMaskPrefixInstruction::IncludingFirst {
            vd: vreg(9),
            vs2: vreg(7),
            mask: RiscvVectorMaskMode::Masked,
        })
    );
}

#[test]
fn decoder_accepts_vector_mask_index_operations() {
    assert_eq!(
        viota_m_type(6, 5, RiscvVectorMaskMode::Unmasked),
        0x5268_22d7
    );
    assert_eq!(viota_m_type(7, 8, RiscvVectorMaskMode::Masked), 0x5078_2457);
    assert_eq!(vid_v_type(9, RiscvVectorMaskMode::Masked), 0x5008_a4d7);
    assert_eq!(
        RiscvInstruction::decode(viota_m_type(6, 5, RiscvVectorMaskMode::Unmasked)).unwrap(),
        RiscvInstruction::VectorMaskIndex(RiscvVectorMaskIndexInstruction::Iota {
            vd: vreg(5),
            vs2: vreg(6),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(viota_m_type(7, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        RiscvInstruction::VectorMaskIndex(RiscvVectorMaskIndexInstruction::Iota {
            vd: vreg(8),
            vs2: vreg(7),
            mask: RiscvVectorMaskMode::Masked,
        })
    );
    assert_eq!(
        RiscvInstruction::decode(vid_v_type(9, RiscvVectorMaskMode::Masked)).unwrap(),
        RiscvInstruction::VectorMaskIndex(RiscvVectorMaskIndexInstruction::Id {
            vd: vreg(9),
            mask: RiscvVectorMaskMode::Masked,
        })
    );

    let reserved_vid_vs2 =
        vector_mask_reduction_type(0x14, 3, 0x11, 9, RiscvVectorMaskMode::Masked);
    assert_eq!(
        RiscvInstruction::decode(reserved_vid_vs2),
        Err(RiscvError::UnknownEncoding {
            raw: reserved_vid_vs2,
        })
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
fn decoder_accepts_unmasked_vector_fixed_point_shift_operations() {
    assert_eq!(vssrl_vv_type(4, 5, 3), 0xaa42_81d7);
    assert_eq!(vssrl_vx_type(4, 5, 3), 0xaa42_c1d7);
    assert_eq!(vssrl_vi_type(4, 5, 3), 0xaa42_b1d7);
    assert_eq!(vssra_vv_type(4, 5, 3), 0xae42_81d7);
    assert_eq!(vssra_vx_type(4, 5, 3), 0xae42_c1d7);
    assert_eq!(vssra_vi_type(4, 5, 3), 0xae42_b1d7);

    assert_eq!(
        RiscvInstruction::decode(vssrl_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_logical_vv(
                vreg(3),
                vreg(4),
                vreg(5),
            ),
        )
    );
    assert_eq!(
        RiscvInstruction::decode(vssrl_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_logical_vx(vreg(3), vreg(4), reg(5),),
        )
    );
    assert_eq!(
        RiscvInstruction::decode(vssrl_vi_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_logical_vi(vreg(3), vreg(4), 5,),
        )
    );
    assert_eq!(
        RiscvInstruction::decode(vssra_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_arithmetic_vv(
                vreg(3),
                vreg(4),
                vreg(5),
            ),
        )
    );
    assert_eq!(
        RiscvInstruction::decode(vssra_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_arithmetic_vx(
                vreg(3),
                vreg(4),
                reg(5),
            ),
        )
    );
    assert_eq!(
        RiscvInstruction::decode(vssra_vi_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_arithmetic_vi(vreg(3), vreg(4), 5,),
        )
    );
}

#[test]
fn decoder_accepts_unmasked_vector_signed_fractional_multiply_operations() {
    assert_eq!(vsmul_vv_type(4, 5, 3), 0x9e42_81d7);
    assert_eq!(vsmul_vx_type(4, 5, 3), 0x9e42_c1d7);

    assert_eq!(
        RiscvInstruction::decode(vsmul_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::mul_signed_fractional_vv(vreg(3), vreg(4), vreg(5),),
        )
    );
    assert_eq!(
        RiscvInstruction::decode(vsmul_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::mul_signed_fractional_vx(vreg(3), vreg(4), reg(5),),
        )
    );
}

#[test]
fn decoder_accepts_vector_integer_reduction_operations() {
    assert_eq!(
        vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
        0x0242_a1d7
    );
    assert_eq!(
        vredmax_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked),
        0x1c42_a1d7
    );

    let cases = [
        (
            vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::sum(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredand_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::and(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredor_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::or(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredxor_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::xor(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredminu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::min_unsigned(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredmin_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::min_signed(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredmaxu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            RiscvVectorReductionInstruction::max_unsigned(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vredmax_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked),
            RiscvVectorReductionInstruction::max_signed(
                vreg(3),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(
            RiscvInstruction::decode(raw).unwrap(),
            RiscvInstruction::VectorReduction(expected)
        );
    }
}

#[test]
fn decoder_accepts_vector_integer_widening_reduction_operations() {
    assert_eq!(
        vwredsumu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
        0xc242_81d7
    );
    assert_eq!(
        vwredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked),
        0xc442_81d7
    );
    assert_eq!(
        RiscvInstruction::decode(vwredsumu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked,))
            .unwrap(),
        RiscvInstruction::VectorReduction(RiscvVectorReductionInstruction::widening_sum_unsigned(
            vreg(3),
            vreg(4),
            vreg(5),
            RiscvVectorMaskMode::Unmasked,
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vwredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked,)).unwrap(),
        RiscvInstruction::VectorReduction(RiscvVectorReductionInstruction::widening_sum_signed(
            vreg(3),
            vreg(4),
            vreg(5),
            RiscvVectorMaskMode::Masked,
        ))
    );
}

#[test]
fn decoder_accepts_vector_integer_widening_add_sub_operations() {
    assert_eq!(
        vwaddu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
        0xc242_a157
    );
    assert_eq!(
        vwadd_vx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
        0xc642_e157
    );
    assert_eq!(
        vwsubu_vv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
        0xc842_a157
    );
    assert_eq!(
        vwsub_vx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
        0xcc42_e157
    );
    assert_eq!(
        vwaddu_wv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
        0xd242_a157
    );
    assert_eq!(
        vwadd_wx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
        0xd442_e157
    );
    assert_eq!(
        vwsubu_wv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
        0xda42_a157
    );
    assert_eq!(
        vwsub_wx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
        0xdc42_e157
    );

    let cases = [
        (
            vwaddu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::add_unsigned_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwadd_vv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::add_signed_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwsub_vv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::sub_signed_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwaddu_vx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::add_unsigned_vx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwsubu_vx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::sub_unsigned_vx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwsub_vx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::sub_signed_vx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwaddu_wv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::add_unsigned_wv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwadd_wv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::add_signed_wv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwsubu_wv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::sub_unsigned_wv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwsub_wv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::sub_signed_wv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwaddu_wx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::add_unsigned_wx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwadd_wx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::add_signed_wx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwsubu_wx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::sub_unsigned_wx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwsub_wx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::sub_signed_wx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(
            RiscvInstruction::decode(raw).unwrap(),
            RiscvInstruction::VectorWideningInteger(expected)
        );
    }
}

#[test]
fn decoder_accepts_vector_integer_widening_multiply_operations() {
    assert_eq!(
        vwmulu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
        0xe242_a157
    );
    assert_eq!(
        vwmulsu_vx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
        0xe842_e157
    );
    assert_eq!(
        vwmul_vv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
        0xec42_a157
    );
    assert_eq!(
        vwmul_vx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
        0xee42_e157
    );

    let cases = [
        (
            vwmulu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::multiply_unsigned_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwmulsu_vv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::multiply_signed_unsigned_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwmul_vv_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::multiply_signed_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwmulu_vx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::multiply_unsigned_vx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
        (
            vwmulsu_vx_type(4, 5, 2, RiscvVectorMaskMode::Masked),
            RiscvVectorWideningIntegerInstruction::multiply_signed_unsigned_vx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Masked,
            ),
        ),
        (
            vwmul_vx_type(4, 5, 2, RiscvVectorMaskMode::Unmasked),
            RiscvVectorWideningIntegerInstruction::multiply_signed_vx(
                vreg(2),
                vreg(4),
                reg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        ),
    ];

    for (raw, expected) in cases {
        assert_eq!(
            RiscvInstruction::decode(raw).unwrap(),
            RiscvInstruction::VectorWideningInteger(expected)
        );
    }
}

#[test]
fn decoder_accepts_unmasked_vector_averaging_add_sub_operations() {
    assert_eq!(vaaddu_vv_type(4, 5, 3), 0x2242_a1d7);
    assert_eq!(vaadd_vv_type(4, 5, 3), 0x2642_a1d7);
    assert_eq!(vasubu_vv_type(4, 5, 3), 0x2a42_a1d7);
    assert_eq!(vasub_vv_type(4, 5, 3), 0x2e42_a1d7);
    assert_eq!(vaaddu_vx_type(4, 5, 3), 0x2242_e1d7);
    assert_eq!(vaadd_vx_type(4, 5, 3), 0x2642_e1d7);
    assert_eq!(vasubu_vx_type(4, 5, 3), 0x2a42_e1d7);
    assert_eq!(vasub_vx_type(4, 5, 3), 0x2e42_e1d7);

    assert_eq!(
        RiscvInstruction::decode(vaaddu_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vaadd_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vasubu_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vasub_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vaaddu_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vaadd_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_signed_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vasubu_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vasub_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_signed_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
}

#[test]
fn decoder_rejects_masked_vector_averaging_add_sub_operations() {
    for raw in [
        vaaddu_vv_type(4, 5, 3) & !(1 << 25),
        vaadd_vx_type(4, 5, 3) & !(1 << 25),
        vasubu_vv_type(4, 5, 3) & !(1 << 25),
        vasub_vx_type(4, 5, 3) & !(1 << 25),
    ] {
        assert_eq!(
            RiscvInstruction::decode(raw),
            Err(RiscvError::UnknownEncoding { raw })
        );
    }
}

#[test]
fn decoder_accepts_unmasked_vector_saturating_add_sub_operations() {
    assert_eq!(vsaddu_vv_type(4, 5, 3), 0x8242_81d7);
    assert_eq!(vsadd_vv_type(4, 5, 3), 0x8642_81d7);
    assert_eq!(vssubu_vv_type(4, 5, 3), 0x8a42_81d7);
    assert_eq!(vssub_vv_type(4, 5, 3), 0x8e42_81d7);
    assert_eq!(vsaddu_vx_type(4, 5, 3), 0x8242_c1d7);
    assert_eq!(vsadd_vx_type(4, 5, 3), 0x8642_c1d7);
    assert_eq!(vssubu_vx_type(4, 5, 3), 0x8a42_c1d7);
    assert_eq!(vssub_vx_type(4, 5, 3), 0x8e42_c1d7);
    assert_eq!(vsaddu_vi_type(4, 5, 3), 0x8242_b1d7);
    assert_eq!(vsadd_vi_type(4, 5, 3), 0x8642_b1d7);

    assert_eq!(
        RiscvInstruction::decode(vsaddu_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vsadd_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vssubu_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vssub_vv_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vsaddu_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vsadd_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_signed_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vssubu_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vssub_vx_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_signed_vx(
            vreg(3),
            vreg(4),
            reg(5),
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vsaddu_vi_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_unsigned_vi(
            vreg(3),
            vreg(4),
            5,
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(vsadd_vi_type(4, 5, 3)).unwrap(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_signed_vi(
            vreg(3),
            vreg(4),
            5,
        ))
    );
}

#[test]
fn decoder_rejects_masked_and_unsupported_vector_saturating_forms() {
    for raw in [
        vsaddu_vv_type(4, 5, 3) & !(1 << 25),
        vsadd_vx_type(4, 5, 3) & !(1 << 25),
        vsmul_vv_type(4, 5, 3) & !(1 << 25),
        vsmul_vx_type(4, 5, 3) & !(1 << 25),
        vsaddu_vi_type(4, 5, 3) & !(1 << 25),
        vector_vi_type(0b100010, 4, 5, 3),
        vector_vi_type(0b100011, 4, 5, 3),
    ] {
        assert_eq!(
            RiscvInstruction::decode(raw),
            Err(RiscvError::UnknownEncoding { raw })
        );
    }
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
fn hart_executes_vector_slide_vx_and_vi_for_active_lanes() {
    let mut up_vx = RiscvHartState::new(0x808c);
    up_vx.set_vector_config(RiscvVectorConfig::new(6, 0xc8));
    up_vx.write(reg(6), 2);
    up_vx.write_vector(vreg(5), bytes_with_u16([10, 11, 12, 13, 14, 15, 16, 17]));
    up_vx.write_vector(vreg(4), bytes_with_u16([0xaaa0; 8]));

    let up_vx_record = up_vx
        .execute(RiscvInstruction::decode(vslideup_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        up_vx.read_vector(vreg(4)),
        bytes_with_u16([0xaaa0, 0xaaa0, 10, 11, 12, 13, 0xaaa0, 0xaaa0])
    );
    assert_eq!(
        up_vx_record.instruction(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::UpVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );

    let mut down_vx = RiscvHartState::new(0x8090);
    down_vx.set_vector_config(RiscvVectorConfig::new(5, 0xc8));
    down_vx.write(reg(6), 3);
    down_vx.write_vector(vreg(5), bytes_with_u16([20, 21, 22, 23, 24, 25, 26, 27]));
    down_vx.write_vector(vreg(4), bytes_with_u16([0xbbbb; 8]));

    down_vx
        .execute(RiscvInstruction::decode(vslidedown_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        down_vx.read_vector(vreg(4)),
        bytes_with_u16([23, 24, 25, 26, 27, 0xbbbb, 0xbbbb, 0xbbbb])
    );

    let mut up_vi = RiscvHartState::new(0x8094);
    up_vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    up_vi.write_vector(vreg(5), bytes_with_u16([30, 31, 32, 33, 34, 35, 36, 37]));
    up_vi.write_vector(vreg(4), bytes_with_u16([0xcccc; 8]));

    up_vi
        .execute(RiscvInstruction::decode(vslideup_vi_type(5, 1, 4)).unwrap())
        .unwrap();

    assert_eq!(
        up_vi.read_vector(vreg(4)),
        bytes_with_u16([0xcccc, 30, 31, 32, 0xcccc, 0xcccc, 0xcccc, 0xcccc])
    );

    let mut down_vi = RiscvHartState::new(0x8098);
    down_vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    down_vi.write_vector(vreg(5), bytes_with_u16([40, 41, 42, 43, 44, 45, 46, 47]));
    down_vi.write_vector(vreg(4), bytes_with_u16([0xdddd; 8]));

    down_vi
        .execute(RiscvInstruction::decode(vslidedown_vi_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        down_vi.read_vector(vreg(4)),
        bytes_with_u16([46, 47, 0, 0, 0xdddd, 0xdddd, 0xdddd, 0xdddd])
    );
}

#[test]
fn hart_executes_vector_slide1_vx_for_active_lanes() {
    let mut up = RiscvHartState::new(0x809c);
    up.set_vector_config(RiscvVectorConfig::new(5, 0xc8));
    up.write(reg(6), 0x1234);
    up.write_vector(vreg(5), bytes_with_u16([10, 11, 12, 13, 14, 15, 16, 17]));
    up.write_vector(vreg(4), bytes_with_u16([0xaaaa; 8]));

    let up_record = up
        .execute(RiscvInstruction::decode(vslide1up_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        up.read_vector(vreg(4)),
        bytes_with_u16([0x1234, 10, 11, 12, 13, 0xaaaa, 0xaaaa, 0xaaaa])
    );
    assert_eq!(
        up_record.instruction(),
        RiscvInstruction::VectorSlide(RiscvVectorSlideInstruction::OneUpVx {
            vd: vreg(4),
            vs2: vreg(5),
            rs1: reg(6),
        })
    );

    let mut down = RiscvHartState::new(0x80a0);
    down.set_vector_config(RiscvVectorConfig::new(5, 0xc8));
    down.write(reg(6), 0x5678);
    down.write_vector(vreg(5), bytes_with_u16([20, 21, 22, 23, 24, 25, 26, 27]));
    down.write_vector(vreg(4), bytes_with_u16([0xbbbb; 8]));

    down.execute(RiscvInstruction::decode(vslide1down_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        down.read_vector(vreg(4)),
        bytes_with_u16([21, 22, 23, 24, 0x5678, 0xbbbb, 0xbbbb, 0xbbbb])
    );
}

#[test]
fn hart_executes_vector_gather_vv_vx_and_vi_for_active_lanes() {
    let mut vv = RiscvHartState::new(0x80a4);
    vv.set_vector_config(RiscvVectorConfig::new(6, 0xc8));
    vv.write_vector(
        vreg(5),
        bytes_with_u16([100, 101, 102, 103, 104, 105, 106, 107]),
    );
    vv.write_vector(vreg(6), bytes_with_u16([4, 0, 7, 8, 2, 1, 3, 3]));
    vv.write_vector(vreg(4), bytes_with_u16([0xaaaa; 8]));

    let vv_record = vv
        .execute(RiscvInstruction::decode(vrgather_vv_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        vv.read_vector(vreg(4)),
        bytes_with_u16([104, 100, 107, 0, 102, 101, 0xaaaa, 0xaaaa])
    );
    assert_eq!(
        vv_record.instruction(),
        RiscvInstruction::VectorGather(RiscvVectorGatherInstruction::Vv {
            vd: vreg(4),
            vs2: vreg(5),
            vs1: vreg(6),
        })
    );

    let mut vx = RiscvHartState::new(0x80a8);
    vx.set_vector_config(RiscvVectorConfig::new(5, 0xc8));
    vx.write(reg(6), 3);
    vx.write_vector(
        vreg(5),
        bytes_with_u16([200, 201, 202, 203, 204, 205, 206, 207]),
    );
    vx.write_vector(vreg(4), bytes_with_u16([0xbbbb; 8]));

    vx.execute(RiscvInstruction::decode(vrgather_vx_type(5, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        vx.read_vector(vreg(4)),
        bytes_with_u16([203, 203, 203, 203, 203, 0xbbbb, 0xbbbb, 0xbbbb])
    );

    let mut vi = RiscvHartState::new(0x80ac);
    vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vi.write_vector(
        vreg(5),
        bytes_with_u16([300, 301, 302, 303, 304, 305, 306, 307]),
    );
    vi.write_vector(vreg(4), bytes_with_u16([0xcccc; 8]));

    vi.execute(RiscvInstruction::decode(vrgather_vi_type(5, 9, 4)).unwrap())
        .unwrap();

    assert_eq!(
        vi.read_vector(vreg(4)),
        bytes_with_u16([0, 0, 0, 0, 0xcccc, 0xcccc, 0xcccc, 0xcccc])
    );
}

#[test]
fn hart_executes_vector_mask_population_and_first_set_reductions() {
    let mut unmasked = RiscvHartState::new(0x80b0);
    unmasked.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    unmasked.write_vector(vreg(6), mask_bytes(0b1011_0101));

    let pop_record = unmasked
        .execute(
            RiscvInstruction::decode(vcpop_m_type(6, 5, RiscvVectorMaskMode::Unmasked)).unwrap(),
        )
        .unwrap();

    assert_eq!(unmasked.read(reg(5)), 4);
    assert_eq!(
        pop_record.register_writes(),
        &[RegisterWrite::new(reg(5), 4)]
    );
    assert_eq!(
        pop_record.instruction(),
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::PopCount {
            rd: reg(5),
            vs2: vreg(6),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );

    let mut masked = RiscvHartState::new(0x80b4);
    masked.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    masked.write_vector(vreg(0), mask_bytes(0b0011_0100));
    masked.write_vector(vreg(6), mask_bytes(0b1011_0101));

    masked
        .execute(RiscvInstruction::decode(vcpop_m_type(6, 5, RiscvVectorMaskMode::Masked)).unwrap())
        .unwrap();

    assert_eq!(masked.read(reg(5)), 3);

    let mut first = RiscvHartState::new(0x80b8);
    first.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    first.write_vector(vreg(0), mask_bytes(0b0011_0100));
    first.write_vector(vreg(7), mask_bytes(0b1011_0001));

    let first_record = first
        .execute(
            RiscvInstruction::decode(vfirst_m_type(7, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();

    assert_eq!(first.read(reg(8)), 4);
    assert_eq!(
        first_record.register_writes(),
        &[RegisterWrite::new(reg(8), 4)]
    );

    first.write_vector(vreg(7), mask_bytes(0b1000_0001));
    first
        .execute(
            RiscvInstruction::decode(vfirst_m_type(7, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();

    assert_eq!(first.read(reg(8)), u64::MAX);
}

#[test]
fn hart_executes_vector_mask_prefix_operations() {
    let mut unmasked = RiscvHartState::new(0x80bc);
    unmasked.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    unmasked.write_vector(vreg(7), mask_bytes(0b0011_0000));
    unmasked.write_vector(vreg(4), mask_bytes(0b1000_0000));
    unmasked.write_vector(vreg(5), mask_bytes(0b1000_0000));
    unmasked.write_vector(vreg(6), mask_bytes(0b1000_0000));

    let before_record = unmasked
        .execute(
            RiscvInstruction::decode(vmsbf_m_type(7, 4, RiscvVectorMaskMode::Unmasked)).unwrap(),
        )
        .unwrap();
    unmasked
        .execute(
            RiscvInstruction::decode(vmsof_m_type(7, 5, RiscvVectorMaskMode::Unmasked)).unwrap(),
        )
        .unwrap();
    unmasked
        .execute(
            RiscvInstruction::decode(vmsif_m_type(7, 6, RiscvVectorMaskMode::Unmasked)).unwrap(),
        )
        .unwrap();

    assert_eq!(unmasked.read_vector(vreg(4)), mask_bytes(0b1000_1111));
    assert_eq!(unmasked.read_vector(vreg(5)), mask_bytes(0b1001_0000));
    assert_eq!(unmasked.read_vector(vreg(6)), mask_bytes(0b1001_1111));
    assert_eq!(
        before_record.instruction(),
        RiscvInstruction::VectorMaskPrefix(RiscvVectorMaskPrefixInstruction::BeforeFirst {
            vd: vreg(4),
            vs2: vreg(7),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );

    let mut masked = RiscvHartState::new(0x80c0);
    masked.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    masked.write_vector(vreg(0), mask_bytes(0b0110_1010));
    masked.write_vector(vreg(7), mask_bytes(0b0011_0000));
    masked.write_vector(vreg(4), mask_bytes(0b1000_0101));
    masked.write_vector(vreg(5), mask_bytes(0b1000_0101));
    masked.write_vector(vreg(6), mask_bytes(0b1000_0101));

    masked
        .execute(RiscvInstruction::decode(vmsbf_m_type(7, 4, RiscvVectorMaskMode::Masked)).unwrap())
        .unwrap();
    masked
        .execute(RiscvInstruction::decode(vmsof_m_type(7, 5, RiscvVectorMaskMode::Masked)).unwrap())
        .unwrap();
    masked
        .execute(RiscvInstruction::decode(vmsif_m_type(7, 6, RiscvVectorMaskMode::Masked)).unwrap())
        .unwrap();

    assert_eq!(masked.read_vector(vreg(4)), mask_bytes(0b1000_1111));
    assert_eq!(masked.read_vector(vreg(5)), mask_bytes(0b1010_0101));
    assert_eq!(masked.read_vector(vreg(6)), mask_bytes(0b1010_1111));
}

#[test]
fn hart_executes_vector_mask_index_operations() {
    let mut unmasked = RiscvHartState::new(0x80c4);
    unmasked.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    unmasked.write_vector(vreg(7), mask_bytes(0b0101_1010));
    unmasked.write_vector(vreg(8), bytes_with_u16([0xaaaa; 8]));
    unmasked.write_vector(vreg(9), bytes_with_u16([0xbbbb; 8]));

    let iota_record = unmasked
        .execute(
            RiscvInstruction::decode(viota_m_type(7, 8, RiscvVectorMaskMode::Unmasked)).unwrap(),
        )
        .unwrap();
    unmasked
        .execute(RiscvInstruction::decode(vid_v_type(9, RiscvVectorMaskMode::Unmasked)).unwrap())
        .unwrap();

    assert_eq!(
        unmasked.read_vector(vreg(8)),
        bytes_with_u16([0, 0, 1, 1, 2, 3, 3, 0xaaaa])
    );
    assert_eq!(
        unmasked.read_vector(vreg(9)),
        bytes_with_u16([0, 1, 2, 3, 4, 5, 6, 0xbbbb])
    );
    assert_eq!(
        iota_record.instruction(),
        RiscvInstruction::VectorMaskIndex(RiscvVectorMaskIndexInstruction::Iota {
            vd: vreg(8),
            vs2: vreg(7),
            mask: RiscvVectorMaskMode::Unmasked,
        })
    );

    let mut masked = RiscvHartState::new(0x80c8);
    masked.set_vector_config(RiscvVectorConfig::new(7, 0xc8));
    masked.write_vector(vreg(0), mask_bytes(0b0110_1010));
    masked.write_vector(vreg(7), mask_bytes(0b0011_0000));
    masked.write_vector(vreg(8), bytes_with_u16([10, 11, 12, 13, 14, 15, 16, 17]));
    masked.write_vector(vreg(9), bytes_with_u16([20, 21, 22, 23, 24, 25, 26, 27]));

    masked
        .execute(RiscvInstruction::decode(viota_m_type(7, 8, RiscvVectorMaskMode::Masked)).unwrap())
        .unwrap();
    masked
        .execute(RiscvInstruction::decode(vid_v_type(9, RiscvVectorMaskMode::Masked)).unwrap())
        .unwrap();

    assert_eq!(
        masked.read_vector(vreg(8)),
        bytes_with_u16([10, 0, 12, 0, 14, 0, 1, 17])
    );
    assert_eq!(
        masked.read_vector(vreg(9)),
        bytes_with_u16([20, 1, 22, 3, 24, 5, 6, 27])
    );
}

#[test]
fn hart_traps_viota_when_destination_overlaps_source_mask() {
    let mut hart = RiscvHartState::new(0x80cc);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xc9));
    hart.write_vector(vreg(7), mask_bytes(0b0000_1010));
    hart.write_vector(vreg(6), bytes_with_u16([0xaaaa; 8]));
    hart.write_vector(vreg(7), bytes_with_u16([0xbbbb; 8]));

    let record = hart
        .execute(
            RiscvInstruction::decode(viota_m_type(7, 6, RiscvVectorMaskMode::Unmasked)).unwrap(),
        )
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x80cc))
    );
    assert_eq!(hart.read_vector(vreg(6)), bytes_with_u16([0xaaaa; 8]));
    assert_eq!(hart.read_vector(vreg(7)), bytes_with_u16([0xbbbb; 8]));
}

#[test]
fn hart_executes_vslidedown_with_fractional_lmul_vlmax_zero_fill() {
    let mut hart = RiscvHartState::new(0x809c);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xcf));
    hart.write_vector(vreg(5), bytes_with_u16([10, 11, 12, 13, 14, 15, 16, 17]));
    hart.write_vector(vreg(4), bytes_with_u16([0xeeee; 8]));

    hart.execute(RiscvInstruction::decode(vslidedown_vi_type(5, 2, 4)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(4)),
        bytes_with_u16([12, 13, 0, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
}

#[test]
fn hart_traps_vslideup_when_destination_overlaps_source() {
    let mut hart = RiscvHartState::new(0x80a0);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    hart.write(reg(6), 1);
    hart.write_vector(vreg(4), bytes_with_u16([10, 11, 12, 13, 14, 15, 16, 17]));

    let record = hart
        .execute(RiscvInstruction::decode(vslideup_vx_type(4, 6, 4)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x80a0))
    );
    assert_eq!(
        hart.read_vector(vreg(4)),
        bytes_with_u16([10, 11, 12, 13, 14, 15, 16, 17])
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
fn hart_executes_vector_fixed_point_shift_vv_vx_and_vi_forms() {
    let mut vv = RiscvHartState::new(0x80e8);
    vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vv.write_vector(
        vreg(4),
        bytes_with_u16([5, 0x01ff, 4, 0xffff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    vv.write_vector(vreg(5), bytes_with_u16([1, 2, 17, 15, 0, 0, 0, 0]));
    vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let vv_record = vv
        .execute(RiscvInstruction::decode(vssrl_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        vv.read_vector(vreg(3)),
        bytes_with_u16([3, 0x80, 2, 2, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vv_record.instruction(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_logical_vv(
                vreg(3),
                vreg(4),
                vreg(5),
            ),
        )
    );
    assert!(!vv.vector_fixed_point().vxsat());

    let mut vx = RiscvHartState::new(0x80ec);
    vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vx.set_vector_fixed_point(RiscvVectorFixedPointState::new(
        RiscvVectorFixedRoundingMode::RoundNearestEven,
    ));
    vx.write(reg(6), 1);
    vx.write_vector(
        vreg(4),
        bytes_with_u16([5, 7, 0xfffb, 0xfff9, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let vx_record = vx
        .execute(RiscvInstruction::decode(vssra_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        vx.read_vector(vreg(3)),
        bytes_with_u16([2, 4, 0xfffe, 0xfffc, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vx_record.instruction(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_arithmetic_vx(
                vreg(3),
                vreg(4),
                reg(6),
            ),
        )
    );
    assert!(!vx.vector_fixed_point().vxsat());

    let mut vi = RiscvHartState::new(0x80f0);
    vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    let mut vi_fixed = RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundToOdd);
    vi_fixed.write_vxsat_bit(true);
    vi.set_vector_fixed_point(vi_fixed);
    vi.write_vector(
        vreg(4),
        bytes_with_u16([4, 5, 6, 7, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    vi.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let vi_record = vi
        .execute(RiscvInstruction::decode(vssrl_vi_type(4, 1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        vi.read_vector(vreg(3)),
        bytes_with_u16([2, 3, 3, 3, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vi_record.instruction(),
        RiscvInstruction::VectorFixedPointShift(
            RiscvVectorFixedPointShiftInstruction::shift_right_logical_vi(vreg(3), vreg(4), 1),
        )
    );
    assert!(vi.vector_fixed_point().vxsat());
}

#[test]
fn hart_executes_vector_signed_fractional_multiply_vv_and_vx_forms() {
    let mut vv = RiscvHartState::new(0x8144);
    vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    vv.write_vector(
        vreg(4),
        bytes_with_u16([
            0x4000, 0x4000, 0x7fff, 0x8000, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    vv.write_vector(
        vreg(5),
        bytes_with_u16([0x4000, 0xc000, 0x7fff, 0x8000, 0, 0, 0, 0]),
    );
    vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let vv_record = vv
        .execute(RiscvInstruction::decode(vsmul_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        vv.read_vector(vreg(3)),
        bytes_with_u16([0x2000, 0xe000, 0x7ffe, 0x7fff, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vv_record.instruction(),
        RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::mul_signed_fractional_vv(vreg(3), vreg(4), vreg(5),),
        )
    );
    assert!(vv.vector_fixed_point().vxsat());

    let mut vx = RiscvHartState::new(0x8148);
    vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    let mut fixed = RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundDown);
    fixed.write_vxsat_bit(true);
    vx.set_vector_fixed_point(fixed);
    vx.write(reg(6), 1);
    vx.write_vector(
        vreg(4),
        bytes_with_u16([0x4000, 0x4001, 0x7fff, 2, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let vx_record = vx
        .execute(RiscvInstruction::decode(vsmul_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        vx.read_vector(vreg(3)),
        bytes_with_u16([0, 0, 0, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        vx_record.instruction(),
        RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::mul_signed_fractional_vx(vreg(3), vreg(4), reg(6),),
        )
    );
    assert!(vx.vector_fixed_point().vxsat());
}

#[test]
fn hart_executes_vector_integer_reductions() {
    let cases = [
        (
            vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [1, 2, 3, 4, 0, 0, 0, 0],
            [10, 0, 0, 0, 0, 0, 0, 0],
            20,
        ),
        (
            vredand_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0x0fff, 0xf0ff, 0xffff, 0xaaaa, 0, 0, 0, 0],
            [0xff0f, 0, 0, 0, 0, 0, 0, 0],
            0x000a,
        ),
        (
            vredor_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [1, 0x20, 0x300, 0x4000, 0, 0, 0, 0],
            [0x1000, 0, 0, 0, 0, 0, 0, 0],
            0x5321,
        ),
        (
            vredxor_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0x00ff, 0x0f0f, 0xf000, 0xaaaa, 0, 0, 0, 0],
            [0xffff, 0, 0, 0, 0, 0, 0, 0],
            0xaaa5,
        ),
        (
            vredminu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0xffff, 0x7fff, 0x9000, 1, 0, 0, 0, 0],
            [0x8000, 0, 0, 0, 0, 0, 0, 0],
            1,
        ),
        (
            vredmin_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0xffff, 0x8000, 2, 0x7ffe, 0, 0, 0, 0],
            [0x7fff, 0, 0, 0, 0, 0, 0, 0],
            0x8000,
        ),
        (
            vredmaxu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [2, 0xffff, 0x8000, 0x7fff, 0, 0, 0, 0],
            [1, 0, 0, 0, 0, 0, 0, 0],
            0xffff,
        ),
        (
            vredmax_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0xffff, 0x7fff, 1, 0x8001, 0, 0, 0, 0],
            [0x8000, 0, 0, 0, 0, 0, 0, 0],
            0x7fff,
        ),
    ];

    for (raw, source, seed, expected) in cases {
        let mut hart = RiscvHartState::new(0x8160);
        hart.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
        hart.write_vector(vreg(4), bytes_with_u16(source));
        hart.write_vector(vreg(5), bytes_with_u16(seed));
        hart.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
        hart.execute(RiscvInstruction::decode(raw).unwrap())
            .unwrap();
        assert_eq!(
            hart.read_vector(vreg(3)),
            bytes_with_u16([expected, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee,])
        );
    }

    let mut wrapping_sum = RiscvHartState::new(0x8161);
    wrapping_sum.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    wrapping_sum.write_vector(vreg(4), bytes_with_u16([1, 2, 0, 0, 0, 0, 0, 0]));
    wrapping_sum.write_vector(vreg(5), bytes_with_u16([0xfffe, 0, 0, 0, 0, 0, 0, 0]));
    wrapping_sum.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    wrapping_sum
        .execute(
            RiscvInstruction::decode(vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        wrapping_sum.read_vector(vreg(3)),
        bytes_with_u16([1, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );

    let mut vl_zero = RiscvHartState::new(0x8162);
    vl_zero.set_vector_config(RiscvVectorConfig::new(0, 0xc8));
    vl_zero.write_vector(vreg(4), bytes_with_u16([1, 2, 3, 4, 0, 0, 0, 0]));
    vl_zero.write_vector(vreg(5), bytes_with_u16([10, 0, 0, 0, 0, 0, 0, 0]));
    vl_zero.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    vl_zero
        .execute(
            RiscvInstruction::decode(vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(vl_zero.read_vector(vreg(3)), bytes_with_u16([0xeeee; 8]));

    let mut all_masked = RiscvHartState::new(0x8163);
    all_masked.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    all_masked.write_vector(vreg(0), [0; 16]);
    all_masked.write_vector(vreg(4), bytes_with_u16([1, 2, 3, 4, 0, 0, 0, 0]));
    all_masked.write_vector(vreg(5), bytes_with_u16([0x1234, 0, 0, 0, 0, 0, 0, 0]));
    all_masked.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    all_masked
        .execute(
            RiscvInstruction::decode(vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        all_masked.read_vector(vreg(3)),
        bytes_with_u16([0x1234, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );

    let mut masked = RiscvHartState::new(0x8164);
    masked.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    masked.write_vector(
        vreg(0),
        [0b0000_0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    masked.write_vector(vreg(4), bytes_with_u16([1, 100, 3, 100, 0, 0, 0, 0]));
    masked.write_vector(vreg(5), bytes_with_u16([10, 0, 0, 0, 0, 0, 0, 0]));
    masked.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let record = masked
        .execute(
            RiscvInstruction::decode(vredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorReduction(RiscvVectorReductionInstruction::sum(
            vreg(3),
            vreg(4),
            vreg(5),
            RiscvVectorMaskMode::Masked,
        ))
    );
    assert_eq!(
        masked.read_vector(vreg(3)),
        bytes_with_u16([14, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
}

#[test]
fn hart_executes_vector_integer_widening_reductions() {
    let cases = [
        (
            vwredsumu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0xffff, 1, 2, 3, 0, 0, 0, 0],
            4,
            0x0001_0009,
        ),
        (
            vwredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0xffff, 0xfffe, 3, 4, 0, 0, 0, 0],
            5,
            9,
        ),
        (
            vwredsumu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked),
            [0xffff, 1, 0, 0, 0, 0, 0, 0],
            0xffff_ffff,
            0x0000_ffff,
        ),
    ];

    for (raw, source, seed, expected) in cases {
        let mut hart = RiscvHartState::new(0x8168);
        hart.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
        hart.write_vector(vreg(4), bytes_with_u16(source));
        hart.write_vector(vreg(5), lanes_u32([seed, 0, 0, 0]));
        hart.write_vector(vreg(3), lanes_u32([0xeeee_eeee; 4]));
        hart.execute(RiscvInstruction::decode(raw).unwrap())
            .unwrap();
        assert_eq!(
            hart.read_vector(vreg(3)),
            lanes_u32([expected, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee])
        );
    }

    let mut vl_zero = RiscvHartState::new(0x8169);
    vl_zero.set_vector_config(RiscvVectorConfig::new(0, 0xc8));
    vl_zero.write_vector(vreg(4), bytes_with_u16([1, 2, 3, 4, 0, 0, 0, 0]));
    vl_zero.write_vector(vreg(5), lanes_u32([10, 0, 0, 0]));
    vl_zero.write_vector(vreg(3), lanes_u32([0xeeee_eeee; 4]));
    vl_zero
        .execute(
            RiscvInstruction::decode(vwredsumu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(vl_zero.read_vector(vreg(3)), lanes_u32([0xeeee_eeee; 4]));

    let mut masked = RiscvHartState::new(0x816a);
    masked.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    masked.write_vector(vreg(0), mask_bytes(0b0101));
    masked.write_vector(vreg(4), bytes_with_u16([0xffff, 0xfffe, 3, 4, 0, 0, 0, 0]));
    masked.write_vector(vreg(5), lanes_u32([10, 0, 0, 0]));
    masked.write_vector(vreg(3), lanes_u32([0xeeee_eeee; 4]));
    let record = masked
        .execute(
            RiscvInstruction::decode(vwredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Masked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorReduction(RiscvVectorReductionInstruction::widening_sum_signed(
            vreg(3),
            vreg(4),
            vreg(5),
            RiscvVectorMaskMode::Masked,
        ))
    );
    assert_eq!(
        masked.read_vector(vreg(3)),
        lanes_u32([12, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee])
    );

    let mut signed_e64 = RiscvHartState::new(0x816b);
    signed_e64.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    signed_e64.write_vector(vreg(4), lanes_u64([u64::MAX, 2]));
    signed_e64.write_vector(vreg(5), 0_u128.to_le_bytes());
    signed_e64.write_vector(vreg(3), u128::MAX.to_le_bytes());
    signed_e64
        .execute(
            RiscvInstruction::decode(vwredsum_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(signed_e64.read_vector(vreg(3)), 1_u128.to_le_bytes());

    let mut wrapping_e64 = RiscvHartState::new(0x816c);
    wrapping_e64.set_vector_config(RiscvVectorConfig::new(2, 0xd8));
    wrapping_e64.write_vector(vreg(4), lanes_u64([1, 1]));
    wrapping_e64.write_vector(vreg(5), u128::MAX.to_le_bytes());
    wrapping_e64.write_vector(vreg(3), 0_u128.to_le_bytes());
    wrapping_e64
        .execute(
            RiscvInstruction::decode(vwredsumu_vs_type(4, 5, 3, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(wrapping_e64.read_vector(vreg(3)), 1_u128.to_le_bytes());
}

#[test]
fn hart_executes_vector_integer_widening_add_sub_vv_and_vx_forms() {
    let mut add_unsigned_vv = RiscvHartState::new(0x8170);
    add_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_unsigned_vv.write_vector(vreg(4), bytes_with_u16([0xffff, 1, 2, 3, 0, 0, 0, 0]));
    add_unsigned_vv.write_vector(vreg(5), bytes_with_u16([1, 2, 3, 4, 0, 0, 0, 0]));
    add_unsigned_vv.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    add_unsigned_vv.write_vector(vreg(3), lanes_u32([0xdddd_dddd; 4]));
    let add_unsigned_vv_record = add_unsigned_vv
        .execute(
            RiscvInstruction::decode(vwaddu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        add_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorWideningInteger(
            RiscvVectorWideningIntegerInstruction::add_unsigned_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        )
    );
    assert_eq!(
        add_unsigned_vv.read_vector(vreg(2)),
        lanes_u32([0x0001_0000, 3, 5, 7])
    );
    assert_eq!(
        add_unsigned_vv.read_vector(vreg(3)),
        lanes_u32([0xdddd_dddd; 4])
    );

    let mut add_signed_vx = RiscvHartState::new(0x8171);
    add_signed_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_vx.write(reg(6), 0xffff);
    add_signed_vx.write_vector(vreg(4), bytes_with_u16([0, 1, 0x8000, 0xffff, 0, 0, 0, 0]));
    add_signed_vx.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    add_signed_vx.write_vector(vreg(3), lanes_u32([0xdddd_dddd; 4]));
    let add_signed_vx_record = add_signed_vx
        .execute(
            RiscvInstruction::decode(vwadd_vx_type(4, 6, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        add_signed_vx_record.instruction(),
        RiscvInstruction::VectorWideningInteger(
            RiscvVectorWideningIntegerInstruction::add_signed_vx(
                vreg(2),
                vreg(4),
                reg(6),
                RiscvVectorMaskMode::Unmasked,
            ),
        )
    );
    assert_eq!(
        add_signed_vx.read_vector(vreg(2)),
        lanes_u32([0xffff_ffff, 0, 0xffff_7fff, 0xffff_fffe])
    );

    let mut sub_unsigned_vv = RiscvHartState::new(0x8172);
    sub_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_unsigned_vv.write_vector(vreg(4), bytes_with_u16([0, 5, 0xffff, 1, 0, 0, 0, 0]));
    sub_unsigned_vv.write_vector(vreg(5), bytes_with_u16([1, 2, 0xffff, 2, 0, 0, 0, 0]));
    sub_unsigned_vv.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    sub_unsigned_vv
        .execute(
            RiscvInstruction::decode(vwsubu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        sub_unsigned_vv.read_vector(vreg(2)),
        lanes_u32([0xffff_ffff, 3, 0, 0xffff_ffff])
    );

    let mut sub_signed_vx_masked = RiscvHartState::new(0x8173);
    sub_signed_vx_masked.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_signed_vx_masked.write(reg(6), 1);
    sub_signed_vx_masked.write_vector(vreg(0), mask_bytes(0b0101));
    sub_signed_vx_masked.write_vector(
        vreg(4),
        bytes_with_u16([0, 0x1234, 0x8000, 0x5678, 0, 0, 0, 0]),
    );
    sub_signed_vx_masked.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    sub_signed_vx_masked
        .execute(
            RiscvInstruction::decode(vwsub_vx_type(4, 6, 2, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        sub_signed_vx_masked.read_vector(vreg(2)),
        lanes_u32([0xffff_ffff, 0xeeee_eeee, 0xffff_7fff, 0xeeee_eeee])
    );
}

#[test]
fn hart_executes_vector_integer_widening_add_sub_wv_and_wx_forms() {
    let mut add_unsigned_wv = RiscvHartState::new(0x8184);
    add_unsigned_wv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_unsigned_wv.write_vector(vreg(4), lanes_u32([0xffff_ffff, 5, 0x8000_0000, 7]));
    add_unsigned_wv.write_vector(vreg(5), lanes_u32([0xcccc_cccc; 4]));
    add_unsigned_wv.write_vector(vreg(6), bytes_with_u16([1, 2, 3, 4, 0, 0, 0, 0]));
    add_unsigned_wv.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    add_unsigned_wv.write_vector(vreg(3), lanes_u32([0xdddd_dddd; 4]));
    let add_unsigned_wv_record = add_unsigned_wv
        .execute(
            RiscvInstruction::decode(vwaddu_wv_type(4, 6, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        add_unsigned_wv_record.instruction(),
        RiscvInstruction::VectorWideningInteger(
            RiscvVectorWideningIntegerInstruction::add_unsigned_wv(
                vreg(2),
                vreg(4),
                vreg(6),
                RiscvVectorMaskMode::Unmasked,
            ),
        )
    );
    assert_eq!(
        add_unsigned_wv.read_vector(vreg(2)),
        lanes_u32([0, 7, 0x8000_0003, 11])
    );
    assert_eq!(
        add_unsigned_wv.read_vector(vreg(3)),
        lanes_u32([0xdddd_dddd; 4])
    );

    let mut add_signed_wv_masked = RiscvHartState::new(0x8188);
    add_signed_wv_masked.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_wv_masked.write_vector(vreg(0), mask_bytes(0b1011));
    add_signed_wv_masked.write_vector(vreg(10), lanes_u32([0, 1, 0x8000_0000, 0xffff_0000]));
    add_signed_wv_masked.write_vector(vreg(11), lanes_u32([0xbbbb_bbbb; 4]));
    add_signed_wv_masked.write_vector(vreg(12), bytes_with_u16([0xffff, 1, 0x8000, 2, 0, 0, 0, 0]));
    add_signed_wv_masked.write_vector(vreg(8), lanes_u32([0xeeee_eeee; 4]));
    add_signed_wv_masked
        .execute(
            RiscvInstruction::decode(vwadd_wv_type(10, 12, 8, RiscvVectorMaskMode::Masked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        add_signed_wv_masked.read_vector(vreg(8)),
        lanes_u32([0xffff_ffff, 2, 0xeeee_eeee, 0xffff_0002])
    );

    let mut sub_unsigned_wx = RiscvHartState::new(0x818c);
    sub_unsigned_wx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_unsigned_wx.write(reg(6), 0xffff);
    sub_unsigned_wx.write_vector(vreg(4), lanes_u32([0, 5, 0xffff_ffff, 1]));
    sub_unsigned_wx.write_vector(vreg(5), lanes_u32([0xcccc_cccc; 4]));
    sub_unsigned_wx.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    sub_unsigned_wx
        .execute(
            RiscvInstruction::decode(vwsubu_wx_type(4, 6, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        sub_unsigned_wx.read_vector(vreg(2)),
        lanes_u32([0xffff_0001, 0xffff_0006, 0xffff_0000, 0xffff_0002])
    );

    let mut sub_signed_wx = RiscvHartState::new(0x8190);
    sub_signed_wx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_signed_wx.write(reg(6), 0xffff);
    sub_signed_wx.write_vector(vreg(4), lanes_u32([0, 1, 0x8000_0000, 0xffff_ffff]));
    sub_signed_wx.write_vector(vreg(5), lanes_u32([0xcccc_cccc; 4]));
    sub_signed_wx.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    sub_signed_wx
        .execute(
            RiscvInstruction::decode(vwsub_wx_type(4, 6, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        sub_signed_wx.read_vector(vreg(2)),
        lanes_u32([1, 2, 0x8000_0001, 0])
    );
}

#[test]
fn hart_executes_vector_integer_widening_multiply_vv_and_vx_forms() {
    let mut unsigned_vv = RiscvHartState::new(0x81b4);
    unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    unsigned_vv.write_vector(vreg(4), bytes_with_u16([0xffff, 2, 3, 4, 0, 0, 0, 0]));
    unsigned_vv.write_vector(vreg(5), bytes_with_u16([2, 0xffff, 3, 4, 0, 0, 0, 0]));
    unsigned_vv.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    unsigned_vv.write_vector(vreg(3), lanes_u32([0xdddd_dddd; 4]));
    let unsigned_vv_record = unsigned_vv
        .execute(
            RiscvInstruction::decode(vwmulu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        unsigned_vv_record.instruction(),
        RiscvInstruction::VectorWideningInteger(
            RiscvVectorWideningIntegerInstruction::multiply_unsigned_vv(
                vreg(2),
                vreg(4),
                vreg(5),
                RiscvVectorMaskMode::Unmasked,
            ),
        )
    );
    assert_eq!(
        unsigned_vv.read_vector(vreg(2)),
        lanes_u32([0x0001_fffe, 0x0001_fffe, 9, 16])
    );
    assert_eq!(
        unsigned_vv.read_vector(vreg(3)),
        lanes_u32([0xdddd_dddd; 4])
    );

    let mut signed_vx = RiscvHartState::new(0x81b8);
    signed_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    signed_vx.write(reg(6), 0xfffe);
    signed_vx.write_vector(vreg(4), bytes_with_u16([0, 1, 0x8000, 0xffff, 0, 0, 0, 0]));
    signed_vx.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    signed_vx
        .execute(
            RiscvInstruction::decode(vwmul_vx_type(4, 6, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        signed_vx.read_vector(vreg(2)),
        lanes_u32([0, 0xffff_fffe, 0x0001_0000, 2])
    );

    let mut signed_unsigned_vv = RiscvHartState::new(0x81bc);
    signed_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    signed_unsigned_vv.write_vector(
        vreg(4),
        bytes_with_u16([0xffff, 0x8000, 2, 0x7fff, 0, 0, 0, 0]),
    );
    signed_unsigned_vv.write_vector(vreg(5), bytes_with_u16([2, 3, 0xffff, 2, 0, 0, 0, 0]));
    signed_unsigned_vv.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    signed_unsigned_vv
        .execute(
            RiscvInstruction::decode(vwmulsu_vv_type(4, 5, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        signed_unsigned_vv.read_vector(vreg(2)),
        lanes_u32([0xffff_fffe, 0xfffe_8000, 0x0001_fffe, 0x0000_fffe])
    );

    let mut masked_signed_unsigned_vx = RiscvHartState::new(0x81c0);
    masked_signed_unsigned_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    masked_signed_unsigned_vx.write(reg(6), 3);
    masked_signed_unsigned_vx.write_vector(vreg(0), mask_bytes(0b0101));
    masked_signed_unsigned_vx
        .write_vector(vreg(4), bytes_with_u16([1, 2, 0xffff, 0x8000, 0, 0, 0, 0]));
    masked_signed_unsigned_vx.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    masked_signed_unsigned_vx
        .execute(
            RiscvInstruction::decode(vwmulsu_vx_type(4, 6, 2, RiscvVectorMaskMode::Masked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        masked_signed_unsigned_vx.read_vector(vreg(2)),
        lanes_u32([3, 0xeeee_eeee, 0xffff_fffd, 0xeeee_eeee])
    );
}

#[test]
fn hart_traps_vector_integer_widening_add_sub_reserved_register_groups() {
    let mut low_overlap = RiscvHartState::new(0x8174);
    low_overlap.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let low_overlap_record = low_overlap
        .execute(
            RiscvInstruction::decode(vwaddu_vv_type(2, 4, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        low_overlap_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8174))
    );

    let mut high_overlap = RiscvHartState::new(0x8178);
    high_overlap.set_vector_config(RiscvVectorConfig::new(2, 0xc8));
    high_overlap.write_vector(vreg(3), bytes_with_u16([1, 2, 0, 0, 0, 0, 0, 0]));
    high_overlap.write_vector(vreg(4), bytes_with_u16([3, 4, 0, 0, 0, 0, 0, 0]));
    high_overlap.write_vector(vreg(2), lanes_u32([0xeeee_eeee; 4]));
    high_overlap
        .execute(
            RiscvInstruction::decode(vwaddu_vv_type(3, 4, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        high_overlap.read_vector(vreg(2)),
        lanes_u32([4, 6, 0xeeee_eeee, 0xeeee_eeee])
    );

    let mut mask_overlap = RiscvHartState::new(0x817c);
    mask_overlap.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let mask_overlap_record = mask_overlap
        .execute(
            RiscvInstruction::decode(vwadd_vv_type(4, 5, 0, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        mask_overlap_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x817c))
    );

    let mut emul_overflow = RiscvHartState::new(0x8180);
    emul_overflow.set_vector_config(RiscvVectorConfig::new(1, 0xcb));
    let emul_overflow_record = emul_overflow
        .execute(
            RiscvInstruction::decode(vwaddu_vv_type(8, 16, 0, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        emul_overflow_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8180))
    );

    let mut wide_source_overlap = RiscvHartState::new(0x8194);
    wide_source_overlap.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    wide_source_overlap.write_vector(vreg(2), lanes_u32([1, 2, 3, 4]));
    wide_source_overlap.write_vector(vreg(3), lanes_u32([0xdddd_dddd; 4]));
    wide_source_overlap.write_vector(vreg(4), bytes_with_u16([5, 6, 7, 8, 0, 0, 0, 0]));
    wide_source_overlap
        .execute(
            RiscvInstruction::decode(vwaddu_wv_type(2, 4, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        wide_source_overlap.read_vector(vreg(2)),
        lanes_u32([6, 8, 10, 12])
    );
    assert_eq!(
        wide_source_overlap.read_vector(vreg(3)),
        lanes_u32([0xdddd_dddd; 4])
    );

    let mut narrow_low_overlap = RiscvHartState::new(0x8198);
    narrow_low_overlap.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let narrow_low_overlap_record = narrow_low_overlap
        .execute(
            RiscvInstruction::decode(vwaddu_wv_type(4, 2, 2, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        narrow_low_overlap_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8198))
    );

    let mut unaligned_wide_source = RiscvHartState::new(0x819c);
    unaligned_wide_source.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let unaligned_wide_source_record = unaligned_wide_source
        .execute(
            RiscvInstruction::decode(vwaddu_wx_type(3, 4, 8, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        unaligned_wide_source_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x819c))
    );

    let mut mixed_eew_source_overlap = RiscvHartState::new(0x81a0);
    mixed_eew_source_overlap.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let mixed_eew_source_overlap_record = mixed_eew_source_overlap
        .execute(
            RiscvInstruction::decode(vwaddu_wv_type(2, 3, 8, RiscvVectorMaskMode::Unmasked))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        mixed_eew_source_overlap_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x81a0))
    );

    let mut masked_narrow_source_v0 = RiscvHartState::new(0x81a4);
    masked_narrow_source_v0.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let masked_narrow_source_v0_record = masked_narrow_source_v0
        .execute(
            RiscvInstruction::decode(vwadd_vv_type(0, 4, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        masked_narrow_source_v0_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x81a4))
    );

    let mut masked_wide_source_v0 = RiscvHartState::new(0x81a8);
    masked_wide_source_v0.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let masked_wide_source_v0_record = masked_wide_source_v0
        .execute(
            RiscvInstruction::decode(vwaddu_wv_type(0, 4, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        masked_wide_source_v0_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x81a8))
    );

    let mut masked_second_source_v0 = RiscvHartState::new(0x81ac);
    masked_second_source_v0.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let masked_second_source_v0_record = masked_second_source_v0
        .execute(
            RiscvInstruction::decode(vwsub_wv_type(2, 0, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        masked_second_source_v0_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x81ac))
    );

    let mut masked_scalar_wide_source_v0 = RiscvHartState::new(0x81b0);
    masked_scalar_wide_source_v0.set_vector_config(RiscvVectorConfig::new(1, 0xc8));
    let masked_scalar_wide_source_v0_record = masked_scalar_wide_source_v0
        .execute(
            RiscvInstruction::decode(vwsubu_wx_type(0, 4, 8, RiscvVectorMaskMode::Masked)).unwrap(),
        )
        .unwrap();
    assert_eq!(
        masked_scalar_wide_source_v0_record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x81b0))
    );
}

#[test]
fn hart_executes_vector_averaging_add_sub_vv_and_vx_forms() {
    let mut add_unsigned_vv = RiscvHartState::new(0x8124);
    add_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_unsigned_vv.write_vector(
        vreg(4),
        bytes_with_u16([5, 6, 7, 8, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_unsigned_vv.write_vector(vreg(5), bytes_with_u16([0, 1, 2, 3, 0, 0, 0, 0]));
    add_unsigned_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_unsigned_vv_record = add_unsigned_vv
        .execute(RiscvInstruction::decode(vaaddu_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_unsigned_vv.read_vector(vreg(3)),
        bytes_with_u16([3, 4, 5, 6, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert!(!add_unsigned_vv.vector_fixed_point().vxsat());

    let mut add_unsigned_vx = RiscvHartState::new(0x8128);
    add_unsigned_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    let mut preserved_fixed =
        RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundDown);
    preserved_fixed.write_vxsat_bit(true);
    add_unsigned_vx.set_vector_fixed_point(preserved_fixed);
    add_unsigned_vx.write(reg(6), 1);
    add_unsigned_vx.write_vector(
        vreg(4),
        bytes_with_u16([5, 6, 7, 8, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_unsigned_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_unsigned_vx_record = add_unsigned_vx
        .execute(RiscvInstruction::decode(vaaddu_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_unsigned_vx.read_vector(vreg(3)),
        bytes_with_u16([3, 3, 4, 4, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_unsigned_vx_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );
    assert!(add_unsigned_vx.vector_fixed_point().vxsat());

    let mut add_signed_vv = RiscvHartState::new(0x812c);
    add_signed_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_vv.write_vector(
        vreg(4),
        bytes_with_u16([2, 0xfffc, 6, 0xfff8, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_signed_vv.write_vector(vreg(5), bytes_with_u16([4, 2, 0xfffe, 0xfffc, 0, 0, 0, 0]));
    add_signed_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_signed_vv_record = add_signed_vv
        .execute(RiscvInstruction::decode(vaadd_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_signed_vv.read_vector(vreg(3)),
        bytes_with_u16([3, 0xffff, 2, 0xfffa, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_signed_vv_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );

    let mut add_signed_vx = RiscvHartState::new(0x8130);
    add_signed_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_vx.write(reg(6), 1);
    add_signed_vx.write_vector(
        vreg(4),
        bytes_with_u16([5, 7, 0xfffb, 0xfff9, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_signed_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_signed_vx_record = add_signed_vx
        .execute(RiscvInstruction::decode(vaadd_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_signed_vx.read_vector(vreg(3)),
        bytes_with_u16([3, 4, 0xfffe, 0xfffd, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_signed_vx_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::add_signed_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );

    let mut sub_unsigned_vv = RiscvHartState::new(0x8134);
    sub_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_unsigned_vv.write_vector(
        vreg(4),
        bytes_with_u16([9, 8, 0, 6, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_unsigned_vv.write_vector(vreg(5), bytes_with_u16([2, 1, 1, 3, 0, 0, 0, 0]));
    sub_unsigned_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_unsigned_vv_record = sub_unsigned_vv
        .execute(RiscvInstruction::decode(vasubu_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_unsigned_vv.read_vector(vreg(3)),
        bytes_with_u16([4, 4, 0x8000, 2, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );

    let mut sub_unsigned_vx = RiscvHartState::new(0x8138);
    sub_unsigned_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_unsigned_vx.set_vector_fixed_point(RiscvVectorFixedPointState::new(
        RiscvVectorFixedRoundingMode::RoundDown,
    ));
    sub_unsigned_vx.write(reg(6), 3);
    sub_unsigned_vx.write_vector(
        vreg(4),
        bytes_with_u16([9, 8, 1, 6, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_unsigned_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_unsigned_vx_record = sub_unsigned_vx
        .execute(RiscvInstruction::decode(vasubu_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_unsigned_vx.read_vector(vreg(3)),
        bytes_with_u16([3, 2, 0x7fff, 1, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_unsigned_vx_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );

    let mut sub_signed_vv = RiscvHartState::new(0x813c);
    sub_signed_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_signed_vv.write_vector(
        vreg(4),
        bytes_with_u16([8, 0xfff8, 4, 0x7fff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_signed_vv.write_vector(vreg(5), bytes_with_u16([2, 2, 0xfffe, 0x8000, 0, 0, 0, 0]));
    sub_signed_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_signed_vv_record = sub_signed_vv
        .execute(RiscvInstruction::decode(vasub_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_signed_vv.read_vector(vreg(3)),
        bytes_with_u16([3, 0xfffb, 3, 0x8000, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_signed_vv_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );

    let mut sub_signed_vx = RiscvHartState::new(0x8140);
    sub_signed_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_signed_vx.write(reg(6), 2);
    sub_signed_vx.write_vector(
        vreg(4),
        bytes_with_u16([8, 0xfff8, 4, 0xfffc, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_signed_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_signed_vx_record = sub_signed_vx
        .execute(RiscvInstruction::decode(vasub_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_signed_vx.read_vector(vreg(3)),
        bytes_with_u16([3, 0xfffb, 1, 0xfffd, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_signed_vx_record.instruction(),
        RiscvInstruction::VectorAveraging(RiscvVectorAveragingInstruction::sub_signed_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );
}

#[test]
fn hart_executes_vector_saturating_add_sub_vv_vx_and_vi_forms() {
    let mut add_unsigned_vv = RiscvHartState::new(0x80f4);
    add_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_unsigned_vv.write_vector(
        vreg(4),
        bytes_with_u16([0xfffe, 10, 0x8000, 0xffff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_unsigned_vv.write_vector(vreg(5), bytes_with_u16([5, 20, 0x7fff, 0, 0, 0, 0, 0]));
    add_unsigned_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_unsigned_vv_record = add_unsigned_vv
        .execute(RiscvInstruction::decode(vsaddu_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_unsigned_vv.read_vector(vreg(3)),
        bytes_with_u16([0xffff, 30, 0xffff, 0xffff, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert!(add_unsigned_vv.vector_fixed_point().vxsat());

    let mut add_unsigned_vx = RiscvHartState::new(0x8110);
    add_unsigned_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_unsigned_vx.write(reg(6), 1);
    add_unsigned_vx.write_vector(
        vreg(4),
        bytes_with_u16([1, 2, 3, 4, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_unsigned_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_unsigned_vx_record = add_unsigned_vx
        .execute(RiscvInstruction::decode(vsaddu_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_unsigned_vx.read_vector(vreg(3)),
        bytes_with_u16([2, 3, 4, 5, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_unsigned_vx_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );
    assert!(!add_unsigned_vx.vector_fixed_point().vxsat());

    let mut add_unsigned_vx_preserves_vxsat = RiscvHartState::new(0x8114);
    add_unsigned_vx_preserves_vxsat.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    let mut preserved_fixed =
        RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundNearestEven);
    preserved_fixed.write_vxsat_bit(true);
    add_unsigned_vx_preserves_vxsat.set_vector_fixed_point(preserved_fixed);
    add_unsigned_vx_preserves_vxsat.write(reg(6), 1);
    add_unsigned_vx_preserves_vxsat.write_vector(
        vreg(4),
        bytes_with_u16([1, 2, 3, 4, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_unsigned_vx_preserves_vxsat.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    add_unsigned_vx_preserves_vxsat
        .execute(RiscvInstruction::decode(vsaddu_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_unsigned_vx_preserves_vxsat.read_vector(vreg(3)),
        bytes_with_u16([2, 3, 4, 5, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert!(add_unsigned_vx_preserves_vxsat.vector_fixed_point().vxsat());

    let mut add_signed_vv = RiscvHartState::new(0x8118);
    add_signed_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_vv.write_vector(
        vreg(4),
        bytes_with_u16([0x7fff, 0x7ffe, 0x8000, 0, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_signed_vv.write_vector(vreg(5), bytes_with_u16([1, 1, 0xffff, 0xffff, 0, 0, 0, 0]));
    add_signed_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_signed_vv_record = add_signed_vv
        .execute(RiscvInstruction::decode(vsadd_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_signed_vv.read_vector(vreg(3)),
        bytes_with_u16([0x7fff, 0x7fff, 0x8000, 0xffff, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_signed_vv_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert!(add_signed_vv.vector_fixed_point().vxsat());

    let mut add_signed_vx = RiscvHartState::new(0x80f8);
    add_signed_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_vx.write(reg(6), 1);
    add_signed_vx.write_vector(
        vreg(4),
        bytes_with_u16([
            0x7fff, 0x7ffe, 0x8000, 0xffff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd,
        ]),
    );
    add_signed_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_signed_vx_record = add_signed_vx
        .execute(RiscvInstruction::decode(vsadd_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_signed_vx.read_vector(vreg(3)),
        bytes_with_u16([0x7fff, 0x7fff, 0x8001, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_signed_vx_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_signed_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );
    assert!(add_signed_vx.vector_fixed_point().vxsat());

    let mut sub_unsigned_vv = RiscvHartState::new(0x811c);
    sub_unsigned_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_unsigned_vv.write_vector(
        vreg(4),
        bytes_with_u16([0, 1, 10, 0xffff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_unsigned_vv.write_vector(vreg(5), bytes_with_u16([1, 1, 2, 0xffff, 0, 0, 0, 0]));
    sub_unsigned_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_unsigned_vv_record = sub_unsigned_vv
        .execute(RiscvInstruction::decode(vssubu_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_unsigned_vv.read_vector(vreg(3)),
        bytes_with_u16([0, 0, 8, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_unsigned_vv_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_unsigned_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert!(sub_unsigned_vv.vector_fixed_point().vxsat());

    let mut sub_unsigned_vx = RiscvHartState::new(0x80fc);
    sub_unsigned_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_unsigned_vx.write(reg(6), 2);
    sub_unsigned_vx.write_vector(
        vreg(4),
        bytes_with_u16([0, 1, 10, 0xffff, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_unsigned_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_unsigned_vx_record = sub_unsigned_vx
        .execute(RiscvInstruction::decode(vssubu_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_unsigned_vx.read_vector(vreg(3)),
        bytes_with_u16([0, 0, 8, 0xfffd, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_unsigned_vx_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_unsigned_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );
    assert!(sub_unsigned_vx.vector_fixed_point().vxsat());

    let mut sub_signed_vv = RiscvHartState::new(0x8100);
    sub_signed_vv.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_signed_vv.write_vector(
        vreg(4),
        bytes_with_u16([0x8000, 0xfffc, 0x7fff, 5, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_signed_vv.write_vector(vreg(5), bytes_with_u16([1, 0xfff6, 0xffff, 20, 0, 0, 0, 0]));
    sub_signed_vv.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_signed_vv_record = sub_signed_vv
        .execute(RiscvInstruction::decode(vssub_vv_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_signed_vv.read_vector(vreg(3)),
        bytes_with_u16([0x8000, 6, 0x7fff, 0xfff1, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_signed_vv_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_signed_vv(
            vreg(3),
            vreg(4),
            vreg(5),
        ))
    );
    assert!(sub_signed_vv.vector_fixed_point().vxsat());

    let mut sub_signed_vx = RiscvHartState::new(0x8120);
    sub_signed_vx.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    sub_signed_vx.write(reg(6), 1);
    sub_signed_vx.write_vector(
        vreg(4),
        bytes_with_u16([0x8000, 0x8001, 0, 1, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    sub_signed_vx.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let sub_signed_vx_record = sub_signed_vx
        .execute(RiscvInstruction::decode(vssub_vx_type(4, 6, 3)).unwrap())
        .unwrap();
    assert_eq!(
        sub_signed_vx.read_vector(vreg(3)),
        bytes_with_u16([0x8000, 0x8000, 0xffff, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        sub_signed_vx_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::sub_signed_vx(
            vreg(3),
            vreg(4),
            reg(6),
        ))
    );
    assert!(sub_signed_vx.vector_fixed_point().vxsat());

    let mut add_unsigned_vi = RiscvHartState::new(0x8104);
    add_unsigned_vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_unsigned_vi.write_vector(
        vreg(4),
        bytes_with_u16([0xfffc, 2, 0, 10, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_unsigned_vi.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_unsigned_vi_record = add_unsigned_vi
        .execute(RiscvInstruction::decode(vsaddu_vi_type(4, 5, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_unsigned_vi.read_vector(vreg(3)),
        bytes_with_u16([0xffff, 7, 5, 15, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_unsigned_vi_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_unsigned_vi(
            vreg(3),
            vreg(4),
            5,
        ))
    );
    assert!(add_unsigned_vi.vector_fixed_point().vxsat());

    let mut add_signed_vi = RiscvHartState::new(0x8108);
    add_signed_vi.set_vector_config(RiscvVectorConfig::new(4, 0xc8));
    add_signed_vi.write_vector(
        vreg(4),
        bytes_with_u16([0x8000, 0x8001, 0, 1, 0xaaaa, 0xbbbb, 0xcccc, 0xdddd]),
    );
    add_signed_vi.write_vector(vreg(3), bytes_with_u16([0xeeee; 8]));
    let add_signed_vi_record = add_signed_vi
        .execute(RiscvInstruction::decode(vsadd_vi_type(4, -1, 3)).unwrap())
        .unwrap();
    assert_eq!(
        add_signed_vi.read_vector(vreg(3)),
        bytes_with_u16([0x8000, 0x8000, 0xffff, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee])
    );
    assert_eq!(
        add_signed_vi_record.instruction(),
        RiscvInstruction::VectorSaturating(RiscvVectorSaturatingInstruction::add_signed_vi(
            vreg(3),
            vreg(4),
            -1,
        ))
    );
    assert!(add_signed_vi.vector_fixed_point().vxsat());
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
