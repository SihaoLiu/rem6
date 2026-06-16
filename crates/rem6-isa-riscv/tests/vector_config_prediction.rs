use rem6_isa_riscv::{
    Register, RiscvBranchPredictionTarget, RiscvControlFlowSnapshot, RiscvControlFlowUpdate,
    RiscvHartState, RiscvInstruction, RiscvVectorConfig, RiscvVectorConfigUpdate,
};

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vsetivli_type(vtype: u32, avl: u8, rd: u8) -> u32 {
    (0b11 << 30)
        | (vtype << 20)
        | (u32::from(avl) << 15)
        | (0b111 << 12)
        | (u32::from(rd) << 7)
        | 0x57
}

fn vsetvl_type(rs2: u8, rs1: u8, rd: u8) -> u32 {
    (1 << 31)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b111 << 12)
        | (u32::from(rd) << 7)
        | 0x57
}

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

#[test]
fn decoder_accepts_vsetvli() {
    let decoded = RiscvInstruction::decode(vsetvli_type(0xd0, 10, 5)).unwrap();

    assert_eq!(
        decoded,
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );
}

#[test]
fn vector_config_vlmax_decodes_vsew_and_lmul_fields() {
    assert_eq!(RiscvVectorConfig::vlmax(0x10), Some(4));
    assert_eq!(RiscvVectorConfig::vlmax(0x02), Some(64));
    assert_eq!(
        RiscvVectorConfig::from_avl(0xd0, 5),
        RiscvVectorConfig::new(4, 0xd0)
    );
}

#[test]
fn decoder_accepts_vsetivli_and_vsetvl() {
    assert_eq!(
        RiscvInstruction::decode(vsetivli_type(0xc9, 7, 6)).unwrap(),
        RiscvInstruction::VectorSetIvli {
            rd: reg(6),
            avl: 7,
            vtype: 0xc9,
        }
    );
    assert_eq!(
        RiscvInstruction::decode(vsetvl_type(9, 8, 7)).unwrap(),
        RiscvInstruction::VectorSetVl {
            rd: reg(7),
            rs1: reg(8),
            rs2: reg(9),
        }
    );
}

#[test]
fn hart_executes_vsetvli_and_updates_vl_vtype() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write(reg(10), 5);

    let record = hart
        .execute(RiscvInstruction::decode(vsetvli_type(0xd0, 10, 5)).unwrap())
        .unwrap();

    assert_eq!(hart.pc(), 0x8004);
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(4, 0xd0));
    assert_eq!(hart.read(reg(5)), 4);
    assert_eq!(
        record.instruction(),
        RiscvInstruction::VectorSetVli {
            rd: reg(5),
            rs1: reg(10),
            vtype: 0xd0,
        }
    );
}

#[test]
fn hart_executes_vsetivli_and_vsetvl() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write(reg(8), 9);
    hart.write(reg(9), 0xd0);

    hart.execute(RiscvInstruction::decode(vsetivli_type(0xc9, 7, 6)).unwrap())
        .unwrap();
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(7, 0xc9));
    assert_eq!(hart.read(reg(6)), 7);

    hart.execute(RiscvInstruction::decode(vsetvl_type(9, 8, 7)).unwrap())
        .unwrap();
    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(4, 0xd0));
    assert_eq!(hart.read(reg(7)), 4);
}

#[test]
fn hart_vsetvli_marks_invalid_vtype_vill() {
    let mut hart = RiscvHartState::new(0x8000);
    hart.write(reg(10), 7);

    hart.execute(RiscvInstruction::decode(vsetvli_type(0x100, 10, 5)).unwrap())
        .unwrap();

    assert_eq!(hart.vector_config(), RiscvVectorConfig::invalid());
    assert_eq!(hart.read(reg(5)), 0);
}

#[test]
fn hart_vsetvli_rs1_zero_uses_vlmax() {
    let mut hart = RiscvHartState::new(0x8000);

    hart.execute(RiscvInstruction::decode(vsetvli_type(0x09, 0, 5)).unwrap())
        .unwrap();

    assert_eq!(hart.vector_config(), RiscvVectorConfig::new(16, 0x09));
    assert_eq!(hart.read(reg(5)), 16);
}
