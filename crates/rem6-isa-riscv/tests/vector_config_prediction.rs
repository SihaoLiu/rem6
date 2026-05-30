use rem6_isa_riscv::{
    RiscvBranchPredictionTarget, RiscvControlFlowSnapshot, RiscvControlFlowUpdate, RiscvHartState,
    RiscvVectorConfig, RiscvVectorConfigUpdate,
};

#[test]
fn branch_prediction_target_drops_copied_dynamic_vector_config() {
    let mut hart = RiscvHartState::new(0x8000);
    let current_config = RiscvVectorConfig::new(64, 0x0000_0000_0000_00d0);
    hart.set_vector_config(current_config);

    let stale_copied_state =
        RiscvControlFlowSnapshot::new(0x8400, RiscvVectorConfig::new(128, 0x0000_0000_0000_00f0));
    hart.apply_control_flow_update(RiscvControlFlowUpdate::branch_prediction(
        RiscvBranchPredictionTarget::from_copied_dynamic_state(stale_copied_state),
    ));

    assert_eq!(hart.pc(), 0x8400);
    assert_eq!(hart.vector_config(), current_config);
}

#[test]
fn explicit_vector_config_update_changes_vector_config() {
    let mut hart = RiscvHartState::new(0x9000);
    let next_config = RiscvVectorConfig::new(32, 0x0000_0000_0000_0078);

    hart.apply_control_flow_update(RiscvControlFlowUpdate::vector_config(
        RiscvVectorConfigUpdate::new(0x9004, next_config),
    ));

    assert_eq!(hart.pc(), 0x9004);
    assert_eq!(hart.vector_config(), next_config);
}
