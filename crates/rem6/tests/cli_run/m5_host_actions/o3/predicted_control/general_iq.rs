use super::*;

#[test]
fn rem6_run_o3_general_iq_control_release_orders_descendant() {
    let path = predicted_control_binary("o3-predicted-control-direct", false, false, false);
    let completed = run_predicted_control_json(&path, "direct", 1_500, "detailed", &[]);

    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x2a"),
        ("x14", "0x2d"),
        ("x15", "0x1"),
        ("x16", "0x2"),
    ] {
        assert_eq!(
            completed
                .pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "unexpected final {register}: {completed}"
        );
    }

    let load = event_at_pc(&completed, LOAD_PC);
    let branch = event_at_pc(&completed, BRANCH_PC);
    let multiply = event_at_pc(&completed, MUL_PC);
    let add = event_at_pc(&completed, ADD_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(branch, "issue_tick") < response_tick);
    assert!(event_u64(multiply, "issue_tick") < response_tick);
    assert_eq!(
        event_u64(add, "issue_tick"),
        event_u64(multiply, "writeback_tick")
    );
    assert!([load, branch, multiply, add]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
    assert_eq!(
        branch
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        branch
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );

    let live_tick = event_u64(add, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_predicted_control_json(&path, "direct", live_tick, "detailed", &[]);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing resident predicted-control ROB: {resident}"));
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LOAD_PC, BRANCH_PC, MUL_PC, ADD_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_switch_preserves_o3_general_iq_ticks() {
    let path = predicted_control_binary("o3-predicted-control-switch", false, false, false);
    let baseline = run_predicted_control_json(&path, "direct", 1_500, "detailed", &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, ADD_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_predicted_control_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &["--host-switch-cpu-mode", &switch_arg],
    );

    let timing_switch = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing predicted-control timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("predicted-control state transfer");
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
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
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        handoff.pointer("/resident_rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );

    for pc in [LOAD_PC, BRANCH_PC, MUL_PC, ADD_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "mode transfer must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
}

#[test]
fn rem6_run_o3_general_iq_checkpoint_boundary() {
    let path = predicted_control_binary("o3-predicted-control-checkpoint", false, false, false);
    let baseline = run_predicted_control_json(&path, "direct", 1_500, "detailed", &[]);
    let live_tick = event_u64(event_at_pc(&baseline, ADD_PC), "issue_tick") + 1;
    let checkpoint_arg = format!("{live_tick}:predicted-control-live");
    let mut live_command = predicted_control_command(&path, "direct", 1_500, "detailed");
    live_command.args(["--host-checkpoint", &checkpoint_arg]);
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live predicted-control checkpoint should fail closed: {stderr}"
    );

    let checkpoint_tick = event_u64(event_at_pc(&baseline, ADD_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:predicted-control-drained");
    let restore_arg = format!("{restore_tick}:predicted-control-drained");
    let restored = run_predicted_control_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("drained predicted-control checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    assert!(checkpoint_component_chunks(cpu0).iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained O3 runtime checkpoint");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(register_value(&restored, "x14"), 0x2d);
}

#[test]
fn rem6_run_timing_suppresses_o3_general_iq_surface() {
    let path = predicted_control_binary("o3-predicted-control-timing", false, false, false);
    let timing = run_predicted_control_json(&path, "direct", 1_500, "timing", &[]);

    assert_eq!(register_value(&timing, "x12"), 0x2a);
    assert_eq!(register_value(&timing, "x13"), 0x2a);
    assert_eq!(register_value(&timing, "x14"), 0x2d);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing predicted-control stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode leaked O3 stats: {unexpected:?}"
    );
}
