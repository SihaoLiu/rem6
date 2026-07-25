use rem6_isa_riscv::{FloatRegister, RiscvFloatRoundingMode};

use super::*;

#[test]
fn live_issue_calendar_width_two_coissues_float_and_vector() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));

    let plan = calendar_plan(
        &runtime,
        20,
        [
            ready(1, O3IssueOpClass::Float),
            ready(2, O3IssueOpClass::Vector),
        ],
    );

    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![1, 2]);
    assert!(plan.resource_blocked().is_empty());
}

#[test]
fn live_issue_calendar_serializes_two_float_rows() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));

    let plan = calendar_plan(
        &runtime,
        20,
        [
            ready(1, O3IssueOpClass::Float),
            ready(2, O3IssueOpClass::Float),
        ],
    );

    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![1]);
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
}

#[test]
fn live_issue_calendar_rebuild_reserves_float_without_blocking_vector() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(3));
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, float_add_s(4, 1, 2)));

    let plan = calendar_plan(
        &runtime,
        20,
        [
            ready(2, O3IssueOpClass::Float),
            ready(3, O3IssueOpClass::Vector),
        ],
    );

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![3]);
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
}

fn float_add_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatAddS {
        rd: FloatRegister::new(rd).unwrap(),
        rs1: FloatRegister::new(rs1).unwrap(),
        rs2: FloatRegister::new(rs2).unwrap(),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}
