use super::super::lsq_fu_branch::{
    assert_completed_mixed_branch_window, event_at_pc, event_at_pc_if_present, event_u64,
    mixed_load_alu_branch_binary, run_mixed_branch_json, BRANCH_PC, FIRST_ALU_PC, LOAD_PC,
    SECOND_ALU_PC, WRONG_STORE_PC,
};
use super::*;

#[test]
fn rem6_run_host_switch_transfers_o3_mixed_load_alu_branch_until_squash() {
    let path = mixed_load_alu_branch_binary("host-switch-o3-mixed-load-alu-branch");
    let baseline = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);
    assert_completed_mixed_branch_window(&baseline);

    let load = event_at_pc(&baseline, LOAD_PC);
    let load_issue = event_u64(load, "issue_tick");
    let load_response = event_u64(load, "lsq_data_response_tick");
    let switch_tick = load_issue + (load_response - load_issue) / 2;
    assert!(load_issue < switch_tick && switch_tick < load_response);

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let json = run_mixed_branch_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &["--host-switch-cpu-mode", &switch_arg],
    );

    assert_completed_mixed_branch_window(&json);
    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .expect("execution-mode switches");
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .expect("detailed-to-timing switch");
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("timing switch action tick");
    assert!(load_issue < timing_action_tick && timing_action_tick < load_response);
    assert!(timing_action_tick >= switch_tick);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("mixed-window state transfer");
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1)
    );

    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        handoff.pointer("/resident_rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        handoff
            .pointer("/outstanding_requests")
            .and_then(Value::as_u64),
        Some(1)
    );

    for pc in [LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, BRANCH_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "handoff must preserve {field} for {pc}: {actual}"
            );
        }
    }
    assert!(event_at_pc_if_present(&json, WRONG_STORE_PC).is_none());
}
