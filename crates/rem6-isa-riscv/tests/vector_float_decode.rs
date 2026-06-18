use rem6_isa_riscv::{
    FloatRegister, RiscvError, RiscvInstruction, RiscvVectorFloatInstruction, VectorRegister,
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

fn vector_float_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
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
