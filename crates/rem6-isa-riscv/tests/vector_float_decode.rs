use rem6_isa_riscv::{
    FloatRegister, RiscvError, RiscvInstruction, RiscvVectorFloatInstruction, VectorRegister,
};

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn vector_float_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(fs1) << 15)
        | (0b101 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vfmv_v_f_type(vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_vf_type(0x17, vs2, fs1, vd)
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
