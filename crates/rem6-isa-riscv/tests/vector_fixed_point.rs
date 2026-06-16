use rem6_isa_riscv::{
    MemoryWidth, Register, RiscvCsrOp, RiscvHartState, RiscvInstruction, RiscvTrap, RiscvTrapKind,
    RiscvVectorConfig, RiscvVectorFixedPointCsr, RiscvVectorFixedPointCsrInstruction,
    RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode, RiscvVectorNarrowClipPlan,
    VectorRegister,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn csr_type(csr: u16, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (u32::from(csr) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
}

fn vnclipu_wi_type(vs2: u8, imm: u8, vd: u8) -> u32 {
    (0b101110 << 26)
        | (1 << 25)
        | ((vs2 as u32) << 20)
        | (u32::from(imm & 0x1f) << 15)
        | (0x3 << 12)
        | ((vd as u32) << 7)
        | 0x57
}

#[test]
fn vector_fixed_point_state_masks_vcsr_alias_bits() {
    let mut state = RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundDown);

    assert_eq!(state.vcsr_bits(), 0b100);
    assert!(!state.vxsat());

    state.write_vcsr_bits(0b1111);
    assert_eq!(
        state.rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundToOdd
    );
    assert!(state.vxsat());
    assert_eq!(state.vcsr_bits(), 0b111);

    state.write_vxsat_bit(false);
    assert_eq!(state.vcsr_bits(), 0b110);

    state.write_vxrm_bits(0b1001);
    assert_eq!(
        state.rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundNearestEven
    );
    assert_eq!(state.vcsr_bits(), 0b010);
}

#[test]
fn vector_narrow_clip_uses_all_vxrm_rounding_modes() {
    let plan = RiscvVectorNarrowClipPlan::unsigned(MemoryWidth::Byte);

    assert_eq!(
        plan.execute_unsigned(5, 1, RiscvVectorFixedRoundingMode::RoundNearestUp)
            .unwrap()
            .value(),
        3
    );
    assert_eq!(
        plan.execute_unsigned(5, 1, RiscvVectorFixedRoundingMode::RoundNearestEven)
            .unwrap()
            .value(),
        2
    );
    assert_eq!(
        plan.execute_unsigned(3, 1, RiscvVectorFixedRoundingMode::RoundNearestEven)
            .unwrap()
            .value(),
        2
    );
    assert_eq!(
        plan.execute_unsigned(5, 1, RiscvVectorFixedRoundingMode::RoundDown)
            .unwrap()
            .value(),
        2
    );
    assert_eq!(
        plan.execute_unsigned(5, 1, RiscvVectorFixedRoundingMode::RoundToOdd)
            .unwrap()
            .value(),
        3
    );
    assert_eq!(
        plan.execute_unsigned(4, 1, RiscvVectorFixedRoundingMode::RoundToOdd)
            .unwrap()
            .value(),
        2
    );
}

#[test]
fn vector_narrow_clip_records_saturation_without_extra_micro_op() {
    let mut unsigned_state =
        RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundNearestUp);
    let unsigned_plan = RiscvVectorNarrowClipPlan::unsigned(MemoryWidth::Byte);
    let unsigned_result = unsigned_plan
        .execute_unsigned(0x1ff, 1, unsigned_state.rounding_mode())
        .unwrap();

    assert_eq!(unsigned_result.value(), 0xff);
    assert!(unsigned_result.saturated());
    unsigned_state.apply_narrow_clip_result(unsigned_result);
    assert!(unsigned_state.vxsat());

    let mut signed_state = RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundDown);
    let signed_plan = RiscvVectorNarrowClipPlan::signed(MemoryWidth::Byte);
    let signed_result = signed_plan
        .execute_signed(-257, 1, signed_state.rounding_mode())
        .unwrap();

    assert_eq!(signed_result.value(), -128);
    assert!(signed_result.saturated());
    signed_state.apply_narrow_clip_result(signed_result);
    assert!(signed_state.vxsat());
}

#[test]
fn decoder_accepts_unmasked_vnclipu_wi() {
    assert_eq!(vnclipu_wi_type(4, 1, 3), 0xba40_b1d7);
    assert_eq!(
        RiscvInstruction::decode(vnclipu_wi_type(4, 1, 3)).unwrap(),
        RiscvInstruction::VectorNarrowClipUnsignedWi(vreg(3), vreg(4), 1)
    );
}

#[test]
fn decoder_accepts_vector_fixed_point_csrs() {
    assert_eq!(
        RiscvInstruction::decode(csr_type(0x009, 0, 0x2, 5)).unwrap(),
        RiscvInstruction::VectorFixedPointCsr(RiscvVectorFixedPointCsrInstruction::read(
            reg(5),
            RiscvVectorFixedPointCsr::Vxsat
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(csr_type(0x00a, 1, 0x1, 6)).unwrap(),
        RiscvInstruction::VectorFixedPointCsr(RiscvVectorFixedPointCsrInstruction::register(
            reg(6),
            RiscvVectorFixedPointCsr::Vxrm,
            RiscvCsrOp::Write,
            reg(1)
        ))
    );
    assert_eq!(
        RiscvInstruction::decode(csr_type(0x00f, 0b100, 0x5, 7)).unwrap(),
        RiscvInstruction::VectorFixedPointCsr(RiscvVectorFixedPointCsrInstruction::immediate(
            reg(7),
            RiscvVectorFixedPointCsr::Vcsr,
            RiscvCsrOp::Write,
            0b100
        ))
    );
}

#[test]
fn hart_executes_vnclipu_wi_with_default_round_nearest_up() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x80));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(
        vreg(4),
        [
            5, 0, 0xff, 0x01, 4, 0, 6, 0, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        ],
    );
    hart.write_vector(vreg(5), [0xbb; 16]);

    hart.execute(RiscvInstruction::decode(vnclipu_wi_type(4, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [3, 0xff, 2, 3, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee]
    );
    assert!(hart.vector_fixed_point().vxsat());
}

#[test]
fn hart_executes_vnclipu_wi_with_low_source_overlap() {
    let mut hart = RiscvHartState::new(0x8100);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x80));
    hart.write_vector(
        vreg(4),
        [
            5, 0, 0xff, 0x01, 4, 0, 6, 0, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        ],
    );
    hart.write_vector(vreg(5), [0xbb; 16]);

    hart.execute(RiscvInstruction::decode(vnclipu_wi_type(4, 1, 4)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(4)),
        [3, 0xff, 2, 3, 4, 0, 6, 0, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa]
    );
    assert!(hart.vector_fixed_point().vxsat());
}

#[test]
fn hart_traps_vnclipu_wi_with_upper_source_overlap() {
    let mut hart = RiscvHartState::new(0x8110);
    hart.set_vector_config(RiscvVectorConfig::new(4, 0x80));

    let record = hart
        .execute(RiscvInstruction::decode(vnclipu_wi_type(4, 1, 5)).unwrap())
        .unwrap();

    assert_eq!(
        record.trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8110))
    );
}

#[test]
fn hart_executes_vnclipu_wi_with_fractional_lmul_source_register() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.set_vector_config(RiscvVectorConfig::new(8, 0x87));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(
        vreg(5),
        [5, 0, 0xff, 0x01, 4, 0, 6, 0, 7, 0, 8, 0, 9, 0, 10, 0],
    );

    hart.execute(RiscvInstruction::decode(vnclipu_wi_type(5, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [3, 0xff, 2, 3, 4, 4, 5, 5, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee]
    );
    assert!(hart.vector_fixed_point().vxsat());
}

#[test]
fn hart_vnclipu_wi_saturates_after_max_width_rounding_overflow() {
    let mut hart = RiscvHartState::new(0x8120);
    hart.set_vector_config(RiscvVectorConfig::new(1, 0x98));
    hart.write_vector(vreg(3), [0xee; 16]);
    hart.write_vector(vreg(4), [0xff; 16]);
    hart.write_vector(vreg(5), [0; 16]);

    hart.execute(RiscvInstruction::decode(vnclipu_wi_type(4, 1, 3)).unwrap())
        .unwrap();

    assert_eq!(
        hart.read_vector(vreg(3)),
        [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xee, 0xee, 0xee, 0xee, 0xee, 0xee,
            0xee, 0xee
        ]
    );
    assert!(hart.vector_fixed_point().vxsat());
}

#[test]
fn hart_executes_vector_fixed_point_csr_read_modify_write_operations() {
    let mut hart = RiscvHartState::new(0x8130);
    hart.write(reg(1), 0b11);
    hart.write(reg(2), 0b1);

    hart.execute(RiscvInstruction::decode(csr_type(0x00a, 1, 0x1, 5)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(5)), 0);
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundToOdd
    );

    hart.execute(RiscvInstruction::decode(csr_type(0x009, 2, 0x2, 6)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(6)), 0);
    assert!(hart.vector_fixed_point().vxsat());

    hart.execute(RiscvInstruction::decode(csr_type(0x00f, 0, 0x2, 7)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(7)), 0b111);

    hart.execute(RiscvInstruction::decode(csr_type(0x009, 2, 0x3, 8)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(8)), 1);
    assert!(!hart.vector_fixed_point().vxsat());

    hart.execute(RiscvInstruction::decode(csr_type(0x00f, 0b100, 0x5, 9)).unwrap())
        .unwrap();
    assert_eq!(hart.read(reg(9)), 0b110);
    assert_eq!(
        hart.vector_fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundDown
    );
    assert!(!hart.vector_fixed_point().vxsat());
}
