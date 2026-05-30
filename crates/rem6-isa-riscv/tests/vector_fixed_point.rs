use rem6_isa_riscv::{
    MemoryWidth, RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode,
    RiscvVectorNarrowClipPlan,
};

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
