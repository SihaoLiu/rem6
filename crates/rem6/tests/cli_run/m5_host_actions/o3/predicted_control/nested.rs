use super::*;

const LOAD_PC: &str = "0x80000024";
const OUTER_BRANCH_PC: &str = "0x80000028";
const INNER_BRANCH_PC: &str = "0x8000002c";
const DESCENDANT_PC: &str = "0x80000030";
const WRONG_STORE_PC: &str = "0x80000034";
const INNER_TARGET_PC: &str = "0x8000003c";
const OUTER_TARGET_PC: &str = "0x80000040";
const DATA_ADDRESS: &str = "0x800000c0";
const WRONG_STORE_ADDRESS: &str = "0x800000c8";

#[test]
fn rem6_run_o3_nested_controls_commit_direct() {
    let path = nested_control_binary("o3-nested-control-direct", false, false, false);
    let json = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000001200000000000000")
    );
    let load = event_at_pc(&json, LOAD_PC);
    let outer = event_at_pc(&json, OUTER_BRANCH_PC);
    let inner = event_at_pc(&json, INNER_BRANCH_PC);
    let descendant = event_at_pc(&json, DESCENDANT_PC);
    event_at_pc(&json, WRONG_STORE_PC);
    event_at_pc(&json, INNER_TARGET_PC);
    event_at_pc(&json, OUTER_TARGET_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") < response_tick);
    assert!(event_u64(descendant, "issue_tick") < response_tick);
    assert!([load, outer, inner, descendant]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
    for branch in [outer, inner] {
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
    }
    for (register, value) in [("x12", 42), ("x13", 18), ("x14", 1), ("x15", 2), ("x16", 3)] {
        assert_eq!(register_value(&json, register), value);
    }
    assert!(json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records.iter().any(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("store")
                && record.pointer("/address").and_then(Value::as_str) == Some(WRONG_STORE_ADDRESS)
        })));
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_outer_misprediction_discards_nested_control_cache_fabric_dram() {
    let path = nested_control_binary("o3-nested-outer-mispredict", true, false, false);
    let completed = run_nested_control_json(&path, "cache-fabric-dram", 2_500, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert_eq!(
        outer
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        outer
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        outer
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, INNER_BRANCH_PC).is_none());
    assert!(event_at_pc_if_present(&completed, DESCENDANT_PC).is_none());
    assert!(event_at_pc_if_present(&completed, WRONG_STORE_PC).is_none());
    assert!(event_at_pc_if_present(&completed, INNER_TARGET_PC).is_none());
    event_at_pc(&completed, OUTER_TARGET_PC);
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_eq!(register_value(&completed, "x14"), 0);
    assert_eq!(register_value(&completed, "x15"), 0);
    assert_eq!(register_value(&completed, "x16"), 3);
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let live_tick = event_u64(outer, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_nested_control_json(&path, "cache-fabric-dram", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC, DESCENDANT_PC]
    );

    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            completed
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed nested control should expose {pointer}: {completed}"
        );
    }
}

#[test]
fn rem6_run_o3_inner_misprediction_preserves_outer_control_direct() {
    let path = nested_control_binary("o3-nested-inner-mispredict", false, true, false);
    let completed = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);

    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") < response_tick);
    assert_eq!(
        outer
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        inner
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        inner
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        inner
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, DESCENDANT_PC).is_none());
    assert!(event_at_pc_if_present(&completed, WRONG_STORE_PC).is_none());
    event_at_pc(&completed, INNER_TARGET_PC);
    event_at_pc(&completed, OUTER_TARGET_PC);
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_eq!(register_value(&completed, "x14"), 0);
    assert_eq!(register_value(&completed, "x15"), 2);
    assert_eq!(register_value(&completed, "x16"), 3);
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.branch_event.squashes",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_load_dependent_inner_branch_suppresses_descendant() {
    let path = nested_control_binary("o3-nested-dependent-inner", false, false, true);
    let completed = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    let descendant = event_at_pc(&completed, DESCENDANT_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") >= response_tick);
    assert!(event_u64(descendant, "issue_tick") >= response_tick);

    let live_tick = event_u64(outer, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_nested_control_json(&path, "direct", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_run_o3_nested_control_requires_branch_lookahead_two() {
    let path = nested_control_binary("o3-nested-lookahead-one", false, false, false);
    let completed =
        run_nested_control_json_with_lookahead(&path, "direct", 2_000, "detailed", 1, &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    event_at_pc(&completed, INNER_BRANCH_PC);
    event_at_pc(&completed, DESCENDANT_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);

    let live_tick = event_u64(outer, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident =
        run_nested_control_json_with_lookahead(&path, "direct", live_tick, "detailed", 1, &[]);
    assert_eq!(resident_rob_pcs(&resident), [LOAD_PC, OUTER_BRANCH_PC]);
    assert_eq!(
        resident
            .pointer("/cores/0/branch_predictor/lookups/direct_conditional")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_run_host_switch_transfers_o3_nested_controls() {
    let path = nested_control_binary("o3-nested-control-switch", false, false, false);
    let baseline = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, DESCENDANT_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_nested_control_json(
        &path,
        "direct",
        2_000,
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
        .unwrap_or_else(|| panic!("missing nested-control timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("nested-control state transfer");
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
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );

    for pc in [LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC, DESCENDANT_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "nested transfer must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
}

#[test]
fn rem6_run_host_switch_preserves_load_dependent_inner_control_timing() {
    let path = nested_control_binary("o3-nested-dependent-inner-switch", false, false, true);
    let baseline = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, OUTER_BRANCH_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_nested_control_json(
        &path,
        "direct",
        2_000,
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
        .unwrap_or_else(|| panic!("missing dependent-inner timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("dependent-inner state transfer");
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1)
    );
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(2)
    );

    for pc in [LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "dependent-inner transfer must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
    assert_eq!(register_value(&switched, "x13"), 18);
    assert_eq!(register_value(&switched, "x14"), 1);
    assert_eq!(register_value(&switched, "x15"), 2);
    assert_eq!(register_value(&switched, "x16"), 3);
    assert_eq!(
        switched
            .pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        switched
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn rem6_run_host_switch_preserves_inner_misprediction_rollback() {
    let path = nested_control_binary("o3-nested-inner-mispredict-switch", false, true, false);
    let baseline = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, INNER_BRANCH_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_nested_control_json(
        &path,
        "direct",
        2_000,
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
        .unwrap_or_else(|| panic!("missing nested-misprediction timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("nested-misprediction state transfer");
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
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );

    for pc in [LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "switched rollback must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
    assert_eq!(
        event_at_pc(&switched, OUTER_BRANCH_PC)
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        event_at_pc(&switched, INNER_BRANCH_PC)
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&switched, DESCENDANT_PC).is_none());
    assert!(event_at_pc_if_present(&switched, WRONG_STORE_PC).is_none());
    assert_eq!(register_value(&switched, "x13"), 0);
    assert_eq!(register_value(&switched, "x14"), 0);
    assert_eq!(register_value(&switched, "x15"), 2);
    assert_eq!(register_value(&switched, "x16"), 3);
    assert_no_data_address(&switched, WRONG_STORE_ADDRESS);
    assert_eq!(
        switched
            .pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        switched
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn rem6_run_o3_nested_control_checkpoint_boundary() {
    let path = nested_control_binary("o3-nested-control-checkpoint", false, false, false);
    let baseline = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let live_tick = event_u64(event_at_pc(&baseline, DESCENDANT_PC), "issue_tick") + 1;
    let live_arg = format!("{live_tick}:nested-control-live");
    let mut live_command = nested_control_command(&path, "direct", 2_000, "detailed");
    live_command.args(["--host-checkpoint", &live_arg]);
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live nested checkpoint should fail closed: {stderr}"
    );

    let drained_tick = event_u64(event_at_pc(&baseline, OUTER_TARGET_PC), "commit_tick") + 1;
    let restore_tick = drained_tick + 1;
    let checkpoint_arg = format!("{drained_tick}:nested-control-drained");
    let restore_arg = format!("{restore_tick}:nested-control-drained");
    let restored = run_nested_control_json(
        &path,
        "direct",
        2_000,
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
        .expect("drained nested-control checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    assert!(checkpoint_component_chunks(cpu0).iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained nested O3 runtime checkpoint");
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
    assert_eq!(register_value(&restored, "x13"), 18);
}

#[test]
fn rem6_run_timing_suppresses_o3_nested_controls() {
    let path = nested_control_binary("o3-nested-control-timing", false, false, false);
    let timing = run_nested_control_json(&path, "direct", 2_000, "timing", &[]);

    assert_eq!(register_value(&timing, "x12"), 42);
    assert_eq!(register_value(&timing, "x13"), 18);
    assert_eq!(
        timing.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000001200000000000000")
    );
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing nested-control stats")
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
        "timing mode leaked nested-control O3 stats: {unexpected:?}"
    );
}

fn nested_control_binary(
    name: &str,
    outer_taken: bool,
    inner_taken: bool,
    dependent_inner: bool,
) -> std::path::PathBuf {
    let data_start = 192_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(if outer_taken { 1 } else { 2 }, 0, 0x0, 6, 0x13),
        i_type(3, 0, 0x0, 7, 0x13),
        i_type(if inner_taken { 3 } else { 4 }, 0, 0x0, 8, 0x13),
        i_type(6, 0, 0x0, 9, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(24, 6, 5, 0b000),
        if dependent_inner {
            b_type(16, 0, 12, 0b000)
        } else {
            b_type(16, 8, 7, 0b000)
        },
        r_type(0x01, 9, 7, 0x0, 13, 0x33),
        s_type(8, 13, 10, 0b010),
        i_type(1, 0, 0x0, 14, 0x13),
        i_type(2, 0, 0x0, 15, 0x13),
        i_type(3, 0, 0x0, 16, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn nested_control_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
) -> Command {
    nested_control_command_with_lookahead(path, memory_system, max_tick, execution_mode, 2)
}

fn nested_control_command_with_lookahead(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-branch-lookahead",
        &branch_lookahead.to_string(),
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        execution_mode,
        "--dump-memory",
        &format!("{DATA_ADDRESS}:16"),
    ]);
    command
}

fn run_nested_control_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = nested_control_command(path, memory_system, max_tick, execution_mode);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid nested-control JSON: {error}"))
}

fn run_nested_control_json_with_lookahead(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    extra_args: &[&str],
) -> Value {
    let mut command = nested_control_command_with_lookahead(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
    );
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode} lookahead={branch_lookahead}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid nested-control JSON: {error}"))
}

fn resident_rob_pcs(json: &Value) -> Vec<&str> {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing resident nested-control ROB: {json}"))
        .iter()
        .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
        .collect()
}

fn assert_no_data_address(json: &Value, address: &str) {
    assert!(
        json.pointer("/debug/data_trace")
            .and_then(Value::as_array)
            .is_some_and(|records| records.iter().all(|record| {
                record.pointer("/address").and_then(Value::as_str) != Some(address)
            })),
        "unexpected data access at {address}: {json}"
    );
    assert!(
        json.pointer("/debug/memory_trace")
            .and_then(Value::as_array)
            .is_some_and(|records| records.iter().all(|record| {
                record.pointer("/address").and_then(Value::as_str) != Some(address)
            })),
        "unexpected memory access at {address}: {json}"
    );
}
