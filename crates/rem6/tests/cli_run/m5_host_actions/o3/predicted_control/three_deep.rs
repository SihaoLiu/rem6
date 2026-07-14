use super::window_support::{
    assert_no_data_address, control_window_command, resident_rob_pcs, run_control_window_json,
};
use super::*;

const LOAD_PC: &str = "0x80000028";
const OUTER_BRANCH_PC: &str = "0x8000002c";
const MIDDLE_BRANCH_PC: &str = "0x80000030";
const INNER_BRANCH_PC: &str = "0x80000034";
const DESCENDANT_PC: &str = "0x80000038";
const WRONG_STORE_PC: &str = "0x8000003c";
const FALLTHROUGH_MARKER_PC: &str = "0x80000040";
const INNER_TARGET_PC: &str = "0x80000044";
const MIDDLE_TARGET_PC: &str = "0x80000048";
const OUTER_TARGET_PC: &str = "0x8000004c";
const DATA_ADDRESS: &str = "0x80000100";
const WRONG_STORE_ADDRESS: &str = "0x80000108";

#[derive(Clone, Copy, Debug, Default)]
struct ThreeDeepControlOptions {
    outer_taken: bool,
    middle_taken: bool,
    inner_taken: bool,
    dependent_inner: bool,
}

#[test]
fn rem6_run_o3_three_deep_mixed_controls_commit_direct() {
    let path = three_deep_control_binary(
        "o3-three-deep-mixed-control-direct",
        ThreeDeepControlOptions::default(),
    );
    let completed = run_three_deep_control_json(&path, "direct", 2_500, "detailed", &[]);

    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000000f00000000000000")
    );
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let middle = event_at_pc(&completed, MIDDLE_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    for branch in [outer, middle, inner] {
        assert!(event_u64(branch, "issue_tick") < response_tick);
        assert_correct_not_taken(branch);
    }
    assert!([load, outer, middle, inner]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));

    let live_tick = event_u64(inner, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_three_deep_control_json(&path, "direct", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, OUTER_BRANCH_PC, MIDDLE_BRANCH_PC, INNER_BRANCH_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        resident
            .pointer("/cores/0/branch_predictor/lookups/direct_conditional")
            .and_then(Value::as_u64),
        Some(3)
    );

    assert_fallthrough_state(&completed);
    assert_store_at_wrong_path_address(&completed);
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
    assert_drained_runtime(&completed);
}

#[test]
fn rem6_run_o3_three_deep_control_requires_branch_lookahead_three() {
    let path = three_deep_control_binary(
        "o3-three-deep-lookahead-two",
        ThreeDeepControlOptions::default(),
    );
    let completed =
        run_three_deep_control_json_with_lookahead(&path, "direct", 2_500, "detailed", 2, &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let middle = event_at_pc(&completed, MIDDLE_BRANCH_PC);
    event_at_pc(&completed, INNER_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(middle, "issue_tick") < response_tick);

    let live_tick = event_u64(middle, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident =
        run_three_deep_control_json_with_lookahead(&path, "direct", live_tick, "detailed", 2, &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, OUTER_BRANCH_PC, MIDDLE_BRANCH_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/branch_predictor/lookups/direct_conditional")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_fallthrough_state(&completed);
    assert_drained_runtime(&completed);
}

#[test]
fn rem6_run_o3_three_deep_outer_misprediction_discards_younger_controls_cache_fabric_dram() {
    let path = three_deep_control_binary(
        "o3-three-deep-outer-mispredict",
        ThreeDeepControlOptions {
            outer_taken: true,
            ..ThreeDeepControlOptions::default()
        },
    );
    let completed = run_three_deep_control_json(&path, "cache-fabric-dram", 3_000, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    assert!(event_u64(outer, "issue_tick") < event_u64(load, "lsq_data_response_tick"));
    assert_mispredicted_taken(outer);

    for pc in [
        MIDDLE_BRANCH_PC,
        INNER_BRANCH_PC,
        DESCENDANT_PC,
        WRONG_STORE_PC,
        FALLTHROUGH_MARKER_PC,
        INNER_TARGET_PC,
        MIDDLE_TARGET_PC,
    ] {
        assert!(event_at_pc_if_present(&completed, pc).is_none(), "{pc}");
    }
    event_at_pc(&completed, OUTER_TARGET_PC);
    assert_marker_state(&completed, [0, 0, 0, 4]);
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_drained_runtime(&completed);

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
            "hierarchy-backed three-deep control should expose {pointer}: {completed}"
        );
    }
}

#[test]
fn rem6_run_o3_three_deep_middle_misprediction_preserves_outer_control_direct() {
    let path = three_deep_control_binary(
        "o3-three-deep-middle-mispredict",
        ThreeDeepControlOptions {
            middle_taken: true,
            ..ThreeDeepControlOptions::default()
        },
    );
    let completed = run_three_deep_control_json(&path, "direct", 2_500, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let middle = event_at_pc(&completed, MIDDLE_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(middle, "issue_tick") < response_tick);
    assert_correct_not_taken(outer);
    assert_mispredicted_taken(middle);

    for pc in [
        INNER_BRANCH_PC,
        DESCENDANT_PC,
        WRONG_STORE_PC,
        FALLTHROUGH_MARKER_PC,
        INNER_TARGET_PC,
    ] {
        assert!(event_at_pc_if_present(&completed, pc).is_none(), "{pc}");
    }
    event_at_pc(&completed, MIDDLE_TARGET_PC);
    event_at_pc(&completed, OUTER_TARGET_PC);
    assert_marker_state(&completed, [0, 0, 3, 4]);
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_drained_runtime(&completed);
}

#[test]
fn rem6_run_o3_three_deep_inner_misprediction_preserves_older_controls_direct() {
    let path = three_deep_control_binary(
        "o3-three-deep-inner-mispredict",
        ThreeDeepControlOptions {
            inner_taken: true,
            ..ThreeDeepControlOptions::default()
        },
    );
    let completed = run_three_deep_control_json(&path, "direct", 2_500, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let middle = event_at_pc(&completed, MIDDLE_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    for branch in [outer, middle, inner] {
        assert!(event_u64(branch, "issue_tick") < response_tick);
    }
    assert_correct_not_taken(outer);
    assert_correct_not_taken(middle);
    assert_mispredicted_taken(inner);

    for pc in [DESCENDANT_PC, WRONG_STORE_PC, FALLTHROUGH_MARKER_PC] {
        assert!(event_at_pc_if_present(&completed, pc).is_none(), "{pc}");
    }
    event_at_pc(&completed, INNER_TARGET_PC);
    event_at_pc(&completed, MIDDLE_TARGET_PC);
    event_at_pc(&completed, OUTER_TARGET_PC);
    assert_marker_state(&completed, [0, 2, 3, 4]);
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
    assert_drained_runtime(&completed);
}

#[test]
fn rem6_run_o3_three_deep_load_dependent_inner_control_is_terminal() {
    let path = three_deep_control_binary(
        "o3-three-deep-dependent-inner",
        ThreeDeepControlOptions {
            dependent_inner: true,
            ..ThreeDeepControlOptions::default()
        },
    );
    let completed = run_three_deep_control_json(&path, "direct", 2_500, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let middle = event_at_pc(&completed, MIDDLE_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    let descendant = event_at_pc(&completed, DESCENDANT_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(middle, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") >= response_tick);
    assert!(event_u64(descendant, "issue_tick") >= response_tick);

    let live_tick = event_u64(middle, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_three_deep_control_json(&path, "direct", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        [LOAD_PC, OUTER_BRANCH_PC, MIDDLE_BRANCH_PC, INNER_BRANCH_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        resident
            .pointer("/cores/0/branch_predictor/lookups/direct_conditional")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_fallthrough_state(&completed);
    assert_drained_runtime(&completed);
}

#[test]
fn rem6_run_host_switch_transfers_o3_three_deep_controls() {
    let path = three_deep_control_binary(
        "o3-three-deep-control-switch",
        ThreeDeepControlOptions::default(),
    );
    let baseline = run_three_deep_control_json(&path, "direct", 2_500, "detailed", &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(event_at_pc(&baseline, INNER_BRANCH_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_three_deep_control_json(
        &path,
        "direct",
        2_500,
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
        .unwrap_or_else(|| panic!("missing three-deep timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("three-deep state transfer");
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

    for pc in [LOAD_PC, OUTER_BRANCH_PC, MIDDLE_BRANCH_PC, INNER_BRANCH_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "three-deep transfer must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
    let execution_modes = switched
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing final execution-mode authority: {switched}"));
    assert_eq!(execution_modes.len(), 1);
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some("timing")
    );
    assert_fallthrough_architectural_state(&switched);
    assert_store_at_wrong_path_address(&switched);
    assert_drained_runtime(&switched);
}

#[test]
fn rem6_run_o3_three_deep_control_checkpoint_boundary() {
    let path = three_deep_control_binary(
        "o3-three-deep-control-checkpoint",
        ThreeDeepControlOptions::default(),
    );
    let baseline = run_three_deep_control_json(&path, "direct", 2_500, "detailed", &[]);
    let live_tick = event_u64(event_at_pc(&baseline, INNER_BRANCH_PC), "issue_tick") + 1;
    let live_arg = format!("{live_tick}:three-deep-control-live");
    let mut live_command = three_deep_control_command(&path, "direct", 2_500, "detailed");
    live_command.args(["--host-checkpoint", &live_arg]);
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live three-deep checkpoint should fail closed: {stderr}"
    );

    let checkpoint_tick = event_u64(event_at_pc(&baseline, OUTER_TARGET_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:three-deep-control-drained");
    let restore_arg = format!("{restore_tick}:three-deep-control-drained");
    let restored = run_three_deep_control_json(
        &path,
        "direct",
        2_500,
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
        .expect("drained three-deep checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    assert!(checkpoint_component_chunks(cpu0).iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained three-deep O3 runtime checkpoint");
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
    assert_fallthrough_architectural_state(&restored);
    assert_drained_runtime(&restored);
}

#[test]
fn rem6_run_timing_suppresses_o3_three_deep_controls() {
    let path = three_deep_control_binary(
        "o3-three-deep-control-timing",
        ThreeDeepControlOptions::default(),
    );
    let timing = run_three_deep_control_json(&path, "direct", 2_500, "timing", &[]);

    assert_eq!(
        timing.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000000f00000000000000")
    );
    assert_fallthrough_architectural_state(&timing);
    assert_store_at_wrong_path_address(&timing);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing three-deep stats")
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
        "timing mode leaked three-deep O3 stats: {unexpected:?}"
    );
}

fn three_deep_control_binary(name: &str, options: ThreeDeepControlOptions) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let inner_rs1 = if options.dependent_inner { 12 } else { 9 };
    let inner_rhs = if options.dependent_inner {
        43
    } else if options.inner_taken {
        4
    } else {
        6
    };
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(if options.outer_taken { 2 } else { 1 }, 0, 0x0, 6, 0x13),
        i_type(3, 0, 0x0, 7, 0x13),
        i_type(if options.middle_taken { 4 } else { 2 }, 0, 0x0, 8, 0x13),
        i_type(5, 0, 0x0, 9, 0x13),
        i_type(inner_rhs, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(32, 6, 5, 0b001),
        b_type(24, 8, 7, 0b100),
        b_type(16, 11, inner_rs1, 0b111),
        r_type(0x01, 9, 7, 0x0, 13, 0x33),
        s_type(8, 13, 10, 0b010),
        i_type(1, 0, 0x0, 14, 0x13),
        i_type(2, 0, 0x0, 15, 0x13),
        i_type(3, 0, 0x0, 16, 0x13),
        i_type(4, 0, 0x0, 17, 0x13),
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

fn three_deep_control_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
) -> Command {
    control_window_command(
        path,
        memory_system,
        max_tick,
        execution_mode,
        3,
        DATA_ADDRESS,
        16,
    )
}

fn run_three_deep_control_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    run_three_deep_control_json_with_lookahead(
        path,
        memory_system,
        max_tick,
        execution_mode,
        3,
        extra_args,
    )
}

fn run_three_deep_control_json_with_lookahead(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    extra_args: &[&str],
) -> Value {
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
        DATA_ADDRESS,
        16,
        extra_args,
    )
}

fn assert_correct_not_taken(event: &Value) {
    assert_eq!(
        event
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        event
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        event
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );
}

fn assert_mispredicted_taken(event: &Value) {
    assert_eq!(
        event
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        event
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        event
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
}

fn assert_fallthrough_state(json: &Value) {
    for pc in [
        DESCENDANT_PC,
        WRONG_STORE_PC,
        FALLTHROUGH_MARKER_PC,
        INNER_TARGET_PC,
        MIDDLE_TARGET_PC,
        OUTER_TARGET_PC,
    ] {
        event_at_pc(json, pc);
    }
    assert_fallthrough_architectural_state(json);
}

fn assert_fallthrough_architectural_state(json: &Value) {
    assert_eq!(register_value(json, "x12"), 42);
    assert_eq!(register_value(json, "x13"), 15);
    assert_marker_state(json, [1, 2, 3, 4]);
}

fn assert_marker_state(json: &Value, expected: [u64; 4]) {
    for (register, value) in ["x14", "x15", "x16", "x17"].into_iter().zip(expected) {
        assert_eq!(register_value(json, register), value, "{register}: {json}");
    }
}

fn assert_store_at_wrong_path_address(json: &Value) {
    assert!(json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records.iter().any(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("store")
                && record.pointer("/address").and_then(Value::as_str) == Some(WRONG_STORE_ADDRESS)
        })));
}

fn assert_drained_runtime(json: &Value) {
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}
