use rem6_isa_riscv::{
    FloatRegister, Register, RiscvHartState, RiscvVectorArchitecturalState, RiscvVectorConfig,
    RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES, RISCV_VECTOR_REGISTER_COUNT,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn patterned_register(seed: u8) -> [u8; RISCV_VECTOR_REGISTER_BYTES] {
    std::array::from_fn(|byte| seed.wrapping_add(byte as u8))
}

fn saturated_round_to_odd() -> RiscvVectorFixedPointState {
    let mut state = RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundToOdd);
    state.write_vxsat_bit(true);
    state
}

#[test]
fn vector_architectural_state_default_matches_reset_vector_state() {
    let state = RiscvVectorArchitecturalState::default();

    assert_eq!(
        state,
        RiscvHartState::new(0x8000).vector_architectural_state()
    );
    assert_eq!(state.config(), RiscvVectorConfig::invalid());
    assert_eq!(
        state.fixed_point().rounding_mode(),
        RiscvVectorFixedRoundingMode::RoundNearestUp
    );
    assert!(!state.fixed_point().vxsat());
    assert_eq!(RISCV_VECTOR_REGISTER_COUNT, 32);
    assert_eq!(state.registers().len(), 32);
    assert!(state
        .registers()
        .iter()
        .all(|register| *register == [0; RISCV_VECTOR_REGISTER_BYTES]));
}

#[test]
fn hart_snapshot_projects_complete_vector_architectural_state() {
    let config = RiscvVectorConfig::new(11, 0xd3);
    let fixed_point = saturated_round_to_odd();
    let mut expected_registers = [[0; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT];
    expected_registers[0] = patterned_register(0x10);
    expected_registers[17] = patterned_register(0x70);
    expected_registers[31] = patterned_register(0xf0);

    let mut hart = RiscvHartState::new(0x8000);
    hart.set_vector_config(config);
    hart.set_vector_fixed_point(fixed_point);
    hart.write_vector(vreg(0), expected_registers[0]);
    hart.write_vector(vreg(17), expected_registers[17]);
    hart.write_vector(vreg(31), expected_registers[31]);

    let snapshot = hart.vector_architectural_state();

    assert_eq!(snapshot.config().vl(), 11);
    assert_eq!(snapshot.config().vtype(), 0xd3);
    assert_eq!(snapshot.fixed_point().vxrm_bits(), 0b11);
    assert!(snapshot.fixed_point().vxsat());
    assert_eq!(snapshot.registers(), &expected_registers);
    assert_eq!(snapshot.register(vreg(0)), expected_registers[0]);
    assert_eq!(snapshot.register(vreg(17)), expected_registers[17]);
    assert_eq!(snapshot.register(vreg(31)), expected_registers[31]);
}

#[test]
fn hart_restore_replaces_vector_state_and_preserves_scalar_state() {
    let preserved_pc = 0x8123_4567;
    let preserved_integer = 0x1122_3344_5566_7788;
    let preserved_float = 0x8877_6655_4433_2211;
    let replacement_registers =
        std::array::from_fn(|index| patterned_register((index as u8).wrapping_mul(7)));
    let replacement = RiscvVectorArchitecturalState::new(
        RiscvVectorConfig::new(9, 0xc2),
        saturated_round_to_odd(),
        replacement_registers,
    );

    let mut hart = RiscvHartState::new(preserved_pc);
    hart.write(reg(12), preserved_integer);
    hart.write_float(freg(23), preserved_float);
    hart.set_vector_config(RiscvVectorConfig::new(1, 0));
    hart.set_vector_fixed_point(RiscvVectorFixedPointState::new(
        RiscvVectorFixedRoundingMode::RoundDown,
    ));
    for index in 0..RISCV_VECTOR_REGISTER_COUNT {
        hart.write_vector(vreg(index as u8), [0xff; RISCV_VECTOR_REGISTER_BYTES]);
    }

    hart.restore_vector_architectural_state(&replacement);

    assert_eq!(hart.vector_architectural_state(), replacement);
    assert_eq!(hart.pc(), preserved_pc);
    assert_eq!(hart.read(reg(12)), preserved_integer);
    assert_eq!(hart.read_float(freg(23)), preserved_float);
}
