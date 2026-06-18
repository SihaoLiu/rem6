use rem6_isa_riscv::{
    FloatRegister, RiscvError, RiscvInstruction, RiscvVectorFloatInstruction,
    RiscvVectorFloatMulAddMode, VectorRegister,
};

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vector_float_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b001, vs2, vs1, vd)
}

fn vector_float_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b101, vs2, fs1, vd)
}

fn vector_float_masked_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_masked_type(funct6, 0b101, vs2, fs1, vd)
}

fn vector_float_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_float_masked_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vfmerge_vfm_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_masked_vf_type(0x17, vs2, fs1, vd)
}

fn vfmacc_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2c, vs2, vs1, vd)
}

fn vfmacc_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2c, vs2, fs1, vd)
}

fn vfnmacc_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2d, vs2, vs1, vd)
}

fn vfnmacc_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2d, vs2, fs1, vd)
}

fn vfmsac_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2e, vs2, vs1, vd)
}

fn vfmsac_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2e, vs2, fs1, vd)
}

fn vfnmsac_vv_type(vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x2f, vs2, vs1, vd)
}

fn vfnmsac_vf_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x2f, vs2, fs1, vd)
}

fn vfcvt_f_xu_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x02, vd)
}

fn vfcvt_f_x_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x03, vd)
}

fn vfcvt_xu_f_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x00, vd)
}

fn vfcvt_x_f_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x01, vd)
}

fn vfcvt_rtz_xu_f_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x06, vd)
}

fn vfcvt_rtz_x_f_v_type(vs2: u8, vd: u8) -> u32 {
    vector_float_vv_type(0x12, vs2, 0x07, vd)
}

fn vfmv_v_f_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x17, vs2, fs1, vd)
}

fn vfmv_f_s_type(vs2: u8, vs1: u8, fd: u8) -> u32 {
    vector_float_vv_type(0x10, vs2, vs1, fd)
}

fn vfmv_s_f_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x10, vs2, fs1, vd)
}

#[test]
fn decoder_accepts_vector_float_mul_add_modes() {
    assert_vector_float_mul_add_vv(
        vfmacc_vv_type(2, 1, 3),
        0xb220_91d7,
        RiscvVectorFloatMulAddMode::ProductPlusAccumulator,
    );
    assert_vector_float_mul_add_vf(
        vfmacc_vf_type(2, 1, 3),
        0xb220_d1d7,
        RiscvVectorFloatMulAddMode::ProductPlusAccumulator,
    );
    assert_vector_float_mul_add_vv(
        vfnmacc_vv_type(2, 1, 3),
        0xb620_91d7,
        RiscvVectorFloatMulAddMode::NegativeProductMinusAccumulator,
    );
    assert_vector_float_mul_add_vf(
        vfnmacc_vf_type(2, 1, 3),
        0xb620_d1d7,
        RiscvVectorFloatMulAddMode::NegativeProductMinusAccumulator,
    );
    assert_vector_float_mul_add_vv(
        vfmsac_vv_type(2, 1, 3),
        0xba20_91d7,
        RiscvVectorFloatMulAddMode::ProductMinusAccumulator,
    );
    assert_vector_float_mul_add_vf(
        vfmsac_vf_type(2, 1, 3),
        0xba20_d1d7,
        RiscvVectorFloatMulAddMode::ProductMinusAccumulator,
    );
    assert_vector_float_mul_add_vv(
        vfnmsac_vv_type(2, 1, 3),
        0xbe20_91d7,
        RiscvVectorFloatMulAddMode::NegativeProductPlusAccumulator,
    );
    assert_vector_float_mul_add_vf(
        vfnmsac_vf_type(2, 1, 3),
        0xbe20_d1d7,
        RiscvVectorFloatMulAddMode::NegativeProductPlusAccumulator,
    );

    for funct6 in 0x2c..=0x2f {
        let masked_vv = vector_float_masked_type(funct6, 0b001, 2, 1, 3);
        assert_eq!(
            RiscvInstruction::decode(masked_vv),
            Err(RiscvError::UnknownEncoding { raw: masked_vv })
        );

        let masked_vf = vector_float_masked_type(funct6, 0b101, 2, 1, 3);
        assert_eq!(
            RiscvInstruction::decode(masked_vf),
            Err(RiscvError::UnknownEncoding { raw: masked_vf })
        );
    }
}

fn assert_vector_float_mul_add_vv(raw: u32, expected_raw: u32, mode: RiscvVectorFloatMulAddMode) {
    assert_eq!(raw, expected_raw);
    assert_eq!(
        RiscvInstruction::decode(raw).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulAddVv {
            vd: vreg(3),
            vs1: vreg(1),
            vs2: vreg(2),
            mode,
        })
    );
}

fn assert_vector_float_mul_add_vf(raw: u32, expected_raw: u32, mode: RiscvVectorFloatMulAddMode) {
    assert_eq!(raw, expected_raw);
    assert_eq!(
        RiscvInstruction::decode(raw).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MulAddVf {
            vd: vreg(3),
            fs1: freg(1),
            vs2: vreg(2),
            mode,
        })
    );
}

#[test]
fn decoder_accepts_vector_float_from_integer_conversions() {
    let unsigned = vfcvt_f_xu_v_type(2, 3);
    assert_eq!(unsigned, 0x4a21_11d7);
    assert_eq!(
        RiscvInstruction::decode(unsigned).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        })
    );

    let signed = vfcvt_f_x_v_type(2, 3);
    assert_eq!(signed, 0x4a21_91d7);
    assert_eq!(
        RiscvInstruction::decode(signed).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV {
            vd: vreg(3),
            vs2: vreg(2),
        })
    );

    for vs1 in [0x02, 0x03] {
        let masked = vector_float_masked_type(0x12, 0b001, 2, vs1, 3);
        assert_eq!(
            RiscvInstruction::decode(masked),
            Err(RiscvError::UnknownEncoding { raw: masked })
        );
    }

    for unsupported_vs1 in [0x04, 0x1f] {
        let raw = vector_float_vv_type(0x12, 2, unsupported_vs1, 3);
        assert_eq!(
            RiscvInstruction::decode(raw),
            Err(RiscvError::UnknownEncoding { raw })
        );
    }

    let opfvf = vector_float_vf_type(0x12, 2, 0x02, 3);
    assert_eq!(
        RiscvInstruction::decode(opfvf),
        Err(RiscvError::UnknownEncoding { raw: opfvf })
    );
}

#[test]
fn decoder_accepts_vector_integer_from_float_conversions() {
    let unsigned = vfcvt_xu_f_v_type(2, 3);
    assert_eq!(unsigned, 0x4a20_11d7);
    assert_eq!(
        RiscvInstruction::decode(unsigned).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatV {
            vd: vreg(3),
            vs2: vreg(2),
        })
    );

    let signed = vfcvt_x_f_v_type(2, 3);
    assert_eq!(signed, 0x4a20_91d7);
    assert_eq!(
        RiscvInstruction::decode(signed).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::ConvertSignedIntFromFloatV {
            vd: vreg(3),
            vs2: vreg(2),
        })
    );

    let unsigned_rtz = vfcvt_rtz_xu_f_v_type(2, 3);
    assert_eq!(unsigned_rtz, 0x4a23_11d7);
    assert_eq!(
        RiscvInstruction::decode(unsigned_rtz).unwrap(),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatTowardZeroV {
                vd: vreg(3),
                vs2: vreg(2),
            }
        )
    );

    let signed_rtz = vfcvt_rtz_x_f_v_type(2, 3);
    assert_eq!(signed_rtz, 0x4a23_91d7);
    assert_eq!(
        RiscvInstruction::decode(signed_rtz).unwrap(),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::ConvertSignedIntFromFloatTowardZeroV {
                vd: vreg(3),
                vs2: vreg(2),
            }
        )
    );

    for vs1 in [0x00, 0x01, 0x06, 0x07] {
        let masked = vector_float_masked_type(0x12, 0b001, 2, vs1, 3);
        assert_eq!(
            RiscvInstruction::decode(masked),
            Err(RiscvError::UnknownEncoding { raw: masked })
        );
    }
}

#[test]
fn decoder_accepts_vfmerge_vfm_only_masked() {
    let valid = vfmerge_vfm_type(2, 1, 3);
    assert_eq!(valid, 0x5c20_d1d7);
    assert_eq!(
        RiscvInstruction::decode(valid).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MergeVf {
            vd: vreg(3),
            vs2: vreg(2),
            fs1: freg(1),
        })
    );

    let unmasked_move = vfmv_v_f_type(0, 1, 3);
    assert_eq!(
        RiscvInstruction::decode(unmasked_move).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveVf {
            vd: vreg(3),
            fs1: freg(1),
        })
    );
}

#[test]
fn decoder_accepts_vfmv_v_f_only_with_zero_vs2() {
    let valid = vfmv_v_f_type(0, 1, 3);
    assert_eq!(valid, 0x5e00_d1d7);
    assert_eq!(
        RiscvInstruction::decode(valid).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveVf {
            vd: vreg(3),
            fs1: freg(1),
        })
    );

    let reserved = vfmv_v_f_type(2, 1, 3);
    assert_eq!(
        RiscvInstruction::decode(reserved),
        Err(RiscvError::UnknownEncoding { raw: reserved })
    );
}

#[test]
fn decoder_accepts_vfmv_f_s_only_with_zero_vs1() {
    let valid = vfmv_f_s_type(2, 0, 3);
    assert_eq!(valid, 0x4220_11d7);
    assert_eq!(
        RiscvInstruction::decode(valid).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveFv {
            fd: freg(3),
            vs2: vreg(2),
        })
    );

    let reserved = vfmv_f_s_type(2, 1, 3);
    assert_eq!(
        RiscvInstruction::decode(reserved),
        Err(RiscvError::UnknownEncoding { raw: reserved })
    );
}

#[test]
fn decoder_accepts_vfmv_s_f_only_with_zero_vs2() {
    let valid = vfmv_s_f_type(0, 1, 3);
    assert_eq!(valid, 0x4200_d1d7);
    assert_eq!(
        RiscvInstruction::decode(valid).unwrap(),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::MoveSv {
            vd: vreg(3),
            fs1: freg(1),
        })
    );

    let reserved = vfmv_s_f_type(2, 1, 3);
    assert_eq!(
        RiscvInstruction::decode(reserved),
        Err(RiscvError::UnknownEncoding { raw: reserved })
    );
}
