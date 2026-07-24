use super::*;

#[path = "scoped_issue/general_iq.rs"]
mod general_iq;

const LOAD_PC: &str = "0x80000030";
const BRANCH_PC: &str = "0x80000034";
const SECOND_ROW_PC: &str = "0x80000038";
const THIRD_ROW_PC: &str = "0x8000003c";
const DUMP_STATS_PC: &str = "0x80000054";

const CROSS_RESOURCE_RESULTS: &str = "2a000000070000004d00000012000000";
const SAME_MULTIPLY_RESULTS: &str = "2a000000070000004d00000022000000";
const FU_HEAD_PC: &str = "0x8000000c";
const FU_INDEPENDENT_PC: &str = "0x80000010";
const FU_DEPENDENT_PC: &str = "0x80000014";
const FU_DEPENDENT_RESULTS: &str = "0c000000050000000100000000000000";
const GENERAL_IQ_LOAD_PC: &str = "0x80000010";
const GENERAL_IQ_PRODUCER_PC: &str = "0x80000014";
const GENERAL_IQ_BLOCKED_PC: &str = "0x80000018";
const GENERAL_IQ_ALU_PC: &str = "0x8000001c";
const GENERAL_IQ_MUL_PC: &str = "0x80000020";
const GENERAL_IQ_RESULTS: &str = "0900000001000000050000004c020000";

const SCOPED_ISSUE_STATS: [(&str, &str, &str); 5] = [
    ("cycles", "issue_cycles", "Cycle"),
    ("issued_rows", "issued_rows", "Count"),
    (
        "resource_blocked_row_cycles",
        "resource_blocked_row_cycles",
        "Cycle",
    ),
    (
        "dependency_blocked_row_cycles",
        "dependency_blocked_row_cycles",
        "Cycle",
    ),
    ("max_rows_per_cycle", "max_rows_per_cycle", "Count"),
];

const ISSUE_QUEUE_STATS: [(&str, &str); 9] = [
    ("enqueued_rows", "enqueued_rows"),
    ("service_turns", "service_turns"),
    ("wake_requests", "wake_requests"),
    ("current_occupancy", "current_occupancy"),
    ("peak_occupancy", "peak_occupancy"),
    (
        "issued_by_class/scalar_integer",
        "issued_by_class.scalar_integer",
    ),
    (
        "issued_by_class/integer_mul_div",
        "issued_by_class.integer_mul_div",
    ),
    ("issued_by_class/memory_agu", "issued_by_class.memory_agu"),
    ("issued_by_class/control", "issued_by_class.control"),
];

#[test]
fn core_summary_json_o3_issue_queue() {
    let path = scoped_issue_binary("o3-issue-queue-json", ScopedIssueCase::CrossResource);
    let json = scoped_issue_json(&path, "direct", 2, 1_500);
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .unwrap_or_else(|| panic!("missing O3 issue queue telemetry: {json}"));

    for (json_field, _) in ISSUE_QUEUE_STATS {
        queue_u64(queue, json_field);
    }
    assert!(queue_u64(queue, "enqueued_rows") > 0);
    assert!(queue_u64(queue, "service_turns") > 0);
    assert!(queue_u64(queue, "wake_requests") > 0);
    assert_eq!(queue_u64(queue, "current_occupancy"), 0);
    assert!(queue_u64(queue, "peak_occupancy") > 0);
    let issued_by_class = ["scalar_integer", "integer_mul_div", "memory_agu", "control"]
        .into_iter()
        .map(|issue_class| queue_u64(queue, &format!("issued_by_class/{issue_class}")))
        .sum::<u64>();
    assert!(issued_by_class > 0);
}

#[test]
fn stats_output_o3_runtime_issue_queue() {
    let path = scoped_issue_binary("o3-issue-queue-stats", ScopedIssueCase::CrossResource);
    let json = scoped_issue_json(&path, "direct", 2, 1_500);
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .unwrap_or_else(|| panic!("missing O3 issue queue telemetry: {json}"));

    for (json_field, stat_field) in ISSUE_QUEUE_STATS {
        assert_json_stat(
            &json,
            &format!("sim.cpu0.o3.issue_queue.{stat_field}"),
            "Count",
            queue_u64(queue, json_field),
            "resettable",
        );
    }

    let output = scoped_issue_command_with_stats_format(&path, "direct", 2, 1_500, "text")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for (json_field, stat_field) in ISSUE_QUEUE_STATS {
        let stat_path = format!("sim.cpu0.o3.issue_queue.{stat_field}");
        assert_text_resettable_count_stat(&stdout, &stat_path, queue_u64(queue, json_field));
        assert_text_stat_occurs_once(&stdout, &stat_path);
    }
}

#[test]
fn rem6_run_o3_scoped_issue_width_one_serializes_direct_window() {
    let path = scoped_issue_binary("o3-scoped-issue-width-one", ScopedIssueCase::CrossResource);
    let json = scoped_issue_json(&path, "direct", 1, 1_500);

    assert_completed_scoped_issue(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    let load_issue = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(
        event_u64(event_at_pc(&json, BRANCH_PC), "issue_tick"),
        load_issue + 1
    );
    assert_eq!(
        event_u64(event_at_pc(&json, SECOND_ROW_PC), "issue_tick"),
        load_issue + 2
    );
    assert_eq!(
        event_u64(event_at_pc(&json, THIRD_ROW_PC), "issue_tick"),
        load_issue + 3
    );
    let issue = scoped_issue_artifact(&json);
    assert!(
        issue_u64(issue, "cycles") > 0,
        "width-one fixture should record issue arbitration cycles: {issue}"
    );
    assert_eq!(issue_u64(issue, "issued_rows"), 3);
    assert!(
        issue_u64(issue, "resource_blocked_row_cycles") > 0,
        "width-one fixture should record width/resource pressure: {issue}"
    );
    assert_eq!(issue_u64(issue, "dependency_blocked_row_cycles"), 0);
    assert_eq!(issue_u64(issue, "max_rows_per_cycle"), 1);
    assert_scoped_issue_native_stats(&json, issue);
}

#[test]
fn rem6_run_o3_scoped_issue_text_stats_expose_arbitration_counters() {
    let path = scoped_issue_binary("o3-scoped-issue-text-stats", ScopedIssueCase::CrossResource);
    let json = scoped_issue_json(&path, "direct", 1, 1_500);

    assert_completed_scoped_issue(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    let issue = scoped_issue_artifact(&json);
    assert!(
        issue_u64(issue, "cycles") > 0,
        "width-one fixture should record positive issue cycles: {issue}"
    );
    assert_eq!(issue_u64(issue, "issued_rows"), 3);
    assert!(
        issue_u64(issue, "resource_blocked_row_cycles") > 0,
        "width-one fixture should record resource-blocked row cycles: {issue}"
    );
    assert_eq!(issue_u64(issue, "dependency_blocked_row_cycles"), 0);
    assert_eq!(issue_u64(issue, "max_rows_per_cycle"), 1);

    let output = scoped_issue_command_with_stats_format(&path, "direct", 1, 1_500, "text")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    for (json_field, stat_field, unit) in SCOPED_ISSUE_STATS {
        let path = format!("sim.cpu0.o3.{stat_field}");
        let value = issue_u64(issue, json_field);
        match unit {
            "Cycle" => assert_text_cycle_stat(&stdout, &path, value),
            "Count" => assert_text_count_stat(&stdout, &path, value),
            _ => panic!("unexpected scoped issue stat unit {unit} for {path}"),
        }
        assert_text_stat_occurs_once(&stdout, &path);
    }
}

#[test]
fn rem6_run_o3_scoped_issue_width_two_coissues_cross_resource_rows() {
    let path = scoped_issue_binary(
        "o3-scoped-issue-width-two-cross",
        ScopedIssueCase::CrossResource,
    );
    let json = scoped_issue_json(&path, "direct", 2, 1_500);

    assert_completed_scoped_issue(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    let load_issue = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(
        event_u64(event_at_pc(&json, BRANCH_PC), "issue_tick"),
        load_issue
    );
    assert_eq!(
        event_u64(event_at_pc(&json, SECOND_ROW_PC), "issue_tick"),
        load_issue + 1
    );
    assert_eq!(
        event_u64(event_at_pc(&json, THIRD_ROW_PC), "issue_tick"),
        load_issue + 1
    );
    let issue = scoped_issue_artifact(&json);
    assert_eq!(issue_u64(issue, "issued_rows"), 3);
    assert_eq!(issue_u64(issue, "dependency_blocked_row_cycles"), 0);
    assert_eq!(issue_u64(issue, "max_rows_per_cycle"), 2);
    assert!(
        issue_u64(issue, "cycles") >= 2,
        "width-two cross-resource fixture should still span multiple issue cycles: {issue}"
    );
    assert_scoped_issue_native_stats(&json, issue);
}

#[test]
fn rem6_run_o3_scoped_issue_serializes_same_multiply_resource() {
    let path = scoped_issue_binary(
        "o3-scoped-issue-same-multiply",
        ScopedIssueCase::SameMultiply,
    );
    let json = scoped_issue_json(&path, "cache-fabric-dram", 2, 1_500);

    assert_completed_scoped_issue(
        &json,
        SAME_MULTIPLY_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x22"),
        ],
    );
    let load_issue = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(
        event_u64(event_at_pc(&json, BRANCH_PC), "issue_tick"),
        load_issue
    );
    assert_eq!(
        event_u64(event_at_pc(&json, SECOND_ROW_PC), "issue_tick"),
        load_issue + 1
    );
    assert_eq!(
        event_u64(event_at_pc(&json, THIRD_ROW_PC), "issue_tick"),
        load_issue + 2
    );
    assert_memory_hierarchy_activity(&json);
    let issue = scoped_issue_artifact(&json);
    assert_eq!(issue_u64(issue, "issued_rows"), 3);
    assert_eq!(issue_u64(issue, "dependency_blocked_row_cycles"), 0);
    assert!(
        issue_u64(issue, "resource_blocked_row_cycles") > 0,
        "same-MUL fixture should expose resource contention: {issue}"
    );
    assert!(
        issue_u64(issue, "max_rows_per_cycle") <= 2,
        "configured width two must bound same-MUL issue rows: {issue}"
    );
    assert_scoped_issue_native_stats(&json, issue);
}

#[test]
fn rem6_run_o3_scoped_issue_dependency_waits_for_multiply() {
    let path = scoped_issue_fu_head_binary("o3-scoped-issue-dependent-fu-head");
    let json = scoped_issue_fu_json(&path, "direct", 1, 1_500);

    assert_final_witness(
        &json,
        FU_DEPENDENT_RESULTS,
        [("x3", "0xc"), ("x4", "0x5"), ("x5", "0x1")],
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        3,
        "monotonic",
    );
    let multiply = event_at_pc(&json, FU_HEAD_PC);
    let independent = event_at_pc(&json, FU_INDEPENDENT_PC);
    let dependent = event_at_pc(&json, FU_DEPENDENT_PC);
    assert_eq!(
        event_u64(independent, "issue_tick"),
        event_u64(multiply, "issue_tick") + 2,
        "the fetched younger row must not inherit a phantom head reservation: multiply={multiply}, independent={independent}"
    );
    assert!(
        event_u64(dependent, "issue_tick") >= event_u64(multiply, "writeback_tick"),
        "dependent ADDI must wait for IntMult writeback: multiply={multiply}, dependent={dependent}"
    );
    assert!(
        event_u64(independent, "issue_tick") < event_u64(dependent, "issue_tick"),
        "independent branch should issue before the blocked dependent row: independent={independent}, dependent={dependent}"
    );
}

#[test]
fn rem6_run_o3_scoped_issue_stats_dump_exposes_arbitration_counters() {
    let path = scoped_issue_stats_dump_binary("o3-scoped-issue-stats-dump");
    let json = scoped_issue_json(&path, "direct", 1, 1_500);

    assert_completed_scoped_issue(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    let dump_event = event_at_pc(&json, DUMP_STATS_PC);
    assert_eq!(
        dump_event.pointer("/system_event").and_then(Value::as_bool),
        Some(true),
        "fixture should execute a real m5_dump_stats row: {dump_event}"
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "scoped-issue dump fixture should deliver one m5_dump_stats action: {host_actions}"
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing scoped-issue stats dump action: {host_actions}"));
    assert_stats_dump_after_scoped_issue_activity(&json, dump, dump_event);
    let issue = scoped_issue_artifact(&json);
    assert_scoped_issue_stats_dump(dump, issue);
}

#[test]
fn rem6_run_timing_suppresses_o3_scoped_issue_surface() {
    let path = scoped_issue_binary("o3-scoped-issue-timing", ScopedIssueCase::CrossResource);
    let json = scoped_issue_json_with_mode(&path, "direct", 1, 1_500, "timing");

    assert_final_witness(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    assert_scoped_issue_surface_absent(&json);
}

#[test]
fn rem6_run_o3_scoped_issue_checkpoint_boundary() {
    let path = scoped_issue_binary("o3-scoped-issue-checkpoint", ScopedIssueCase::CrossResource);
    let baseline = scoped_issue_json(&path, "direct", 1, 1_500);
    let live_tick = event_u64(event_at_pc(&baseline, THIRD_ROW_PC), "issue_tick") + 1;
    let live_arg = format!("{live_tick}:scoped-issue-live");
    let mut live_command = scoped_issue_command(&path, "direct", 1, 1_500);
    live_command.args(["--host-checkpoint", &live_arg]);
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live scoped-issue checkpoint should fail closed: {stderr}"
    );

    let checkpoint_tick = event_u64(event_at_pc(&baseline, LOAD_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:scoped-issue-drained");
    let restore_arg = format!("{restore_tick}:scoped-issue-drained");
    let restored = scoped_issue_json_with_args(
        &path,
        "direct",
        1,
        1_500,
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
    assert_completed_scoped_issue(
        &restored,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
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
        .expect("drained scoped-issue checkpoint");
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("restored scoped-issue checkpoint");
    let captured_runtime = scoped_issue_checkpoint_runtime(checkpoint);
    let restored_runtime = scoped_issue_checkpoint_runtime(restore);
    assert_eq!(
        captured_runtime
            .pointer("/checkpoint_version")
            .and_then(Value::as_u64),
        Some(23)
    );
    for field in ["snapshot_rob_entries", "snapshot_lsq_entries"] {
        assert_eq!(
            captured_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(0),
            "drained scoped-issue checkpoint should expose zero {field}: {captured_runtime}"
        );
    }
    assert_eq!(
        captured_runtime
            .pointer("/stats_issued_rows")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert!(captured_runtime
        .pointer("/stats_issue_cycles")
        .and_then(Value::as_u64)
        .is_some_and(|cycles| cycles >= 3));
    assert!(captured_runtime
        .pointer("/stats_resource_blocked_row_cycles")
        .and_then(Value::as_u64)
        .is_some_and(|rows| rows > 0));
    assert_eq!(
        captured_runtime
            .pointer("/stats_dependency_blocked_row_cycles")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        captured_runtime
            .pointer("/stats_max_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(1)
    );
    for field in [
        "checkpoint_version",
        "snapshot_rob_entries",
        "snapshot_lsq_entries",
        "stats_issue_cycles",
        "stats_issued_rows",
        "stats_resource_blocked_row_cycles",
        "stats_dependency_blocked_row_cycles",
        "stats_max_rows_per_cycle",
    ] {
        assert_eq!(
            restored_runtime.pointer(&format!("/{field}")),
            captured_runtime.pointer(&format!("/{field}")),
            "restored scoped-issue checkpoint must preserve {field}"
        );
    }
}

#[test]
fn rem6_run_host_switch_preserves_o3_scoped_issue_ticks() {
    let path = scoped_issue_binary(
        "o3-scoped-issue-host-switch",
        ScopedIssueCase::CrossResource,
    );
    let baseline = scoped_issue_json(&path, "direct", 2, 1_500);
    assert_completed_scoped_issue(
        &baseline,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );

    let baseline_load = event_at_pc(&baseline, LOAD_PC);
    let baseline_first_younger = event_at_pc(&baseline, BRANCH_PC);
    let first_younger_issue = event_u64(baseline_first_younger, "issue_tick");
    let requested_switch_tick = first_younger_issue + 1;

    let switch_arg = format!("{requested_switch_tick}:cpu0:timing");
    let switched = scoped_issue_json_with_args(
        &path,
        "direct",
        2,
        1_500,
        &["--host-switch-cpu-mode", &switch_arg],
    );
    assert_completed_scoped_issue_with_max_lsq(
        &switched,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
        1,
    );

    for pc in [LOAD_PC, BRANCH_PC, SECOND_ROW_PC, THIRD_ROW_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "host switch must preserve {field} for scoped row {pc}: expected={expected} actual={actual}"
            );
        }
    }

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
        .unwrap_or_else(|| panic!("missing scoped-issue detailed-to-timing switch: {switched}"));
    let switch_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("timing switch should expose its real tick: {timing_switch}"));
    assert!(
        switch_tick >= requested_switch_tick,
        "real host switch tick must not precede the requested scoped-issue tick: requested={requested_switch_tick}, switch={timing_switch}"
    );
    assert!(
        switch_tick > first_younger_issue,
        "host switch must occur after the first younger row issues: switch_tick={switch_tick}, row={baseline_first_younger}"
    );
    assert!(
        switch_tick < event_u64(baseline_load, "lsq_data_response_tick"),
        "host switch must precede the delayed load response: switch_tick={switch_tick}, load={baseline_load}"
    );
    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing scoped-issue timing switch transfer: {timing_switch}"));
    let runtime = scoped_issue_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false),
        "scoped-issue transfer runtime chunk should decode cleanly: {runtime}"
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4),
        "scoped-issue switch must capture the load and three younger rows: {runtime}"
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1),
        "scoped-issue switch must capture the outstanding load: {runtime}"
    );
    let handoff = scoped_issue_transfer_live_data_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(7),
        "scoped-issue live-data handoff schema should remain unchanged: {handoff}"
    );
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3),
        "scoped-issue live-data handoff should retain the three younger rows: {handoff}"
    );

    let transfer_oracle = scoped_issue_json(&path, "direct", 2, switch_tick);
    let source_issue = scoped_issue_artifact(&transfer_oracle);
    for (json_field, chunk_field, unit) in [
        ("cycles", "stats_issue_cycles", "Cycle"),
        ("issued_rows", "stats_issued_rows", "Count"),
        (
            "resource_blocked_row_cycles",
            "stats_resource_blocked_row_cycles",
            "Cycle",
        ),
        (
            "dependency_blocked_row_cycles",
            "stats_dependency_blocked_row_cycles",
            "Cycle",
        ),
        ("max_rows_per_cycle", "stats_max_rows_per_cycle", "Count"),
    ] {
        let value = issue_u64(source_issue, json_field);
        assert_eq!(
            runtime
                .pointer(&format!("/{chunk_field}"))
                .and_then(Value::as_u64),
            Some(value),
            "decoded scoped-issue transfer field {chunk_field} must match source detailed O3RuntimeStats field {json_field}: runtime={runtime}, source={source_issue}"
        );
        assert_json_stat(
            &switched,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{chunk_field}"
            ),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            &switched,
            &format!(
                "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu0.component.cpu0.chunk.o3_runtime_state.o3_runtime.{chunk_field}"
            ),
            unit,
            value,
            "monotonic",
        );
    }
}

fn scoped_issue_transfer_o3_runtime_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    scoped_issue_transfer_component(transfer, component)
        .pointer("/chunks")
        .and_then(Value::as_array)
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap_or_else(|| panic!("missing decoded scoped-issue O3 runtime transfer: {transfer}"))
}

fn scoped_issue_transfer_live_data_handoff_chunk<'a>(
    transfer: &'a Value,
    component: &str,
) -> &'a Value {
    scoped_issue_transfer_component(transfer, component)
        .pointer("/chunks")
        .and_then(Value::as_array)
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-live-data-handoff")
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_live_data_handoff"))
        .unwrap_or_else(|| {
            panic!("missing decoded scoped-issue live-data handoff transfer: {transfer}")
        })
}

fn scoped_issue_transfer_component<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .unwrap_or_else(|| panic!("missing transfer component {component}: {transfer}"))
}

fn scoped_issue_checkpoint_runtime(checkpoint: &Value) -> &Value {
    let cpu0 = scoped_issue_checkpoint_component(checkpoint, "cpu0");
    assert!(scoped_issue_checkpoint_component_chunks(cpu0)
        .iter()
        .all(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
        }));
    scoped_issue_checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap_or_else(|| panic!("missing decoded scoped-issue O3 runtime checkpoint: {cpu0}"))
}

fn scoped_issue_checkpoint_component<'a>(checkpoint: &'a Value, component: &str) -> &'a Value {
    checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .unwrap_or_else(|| panic!("missing checkpoint component {component}: {checkpoint}"))
}

fn scoped_issue_checkpoint_component_chunks(component: &Value) -> &[Value] {
    component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing checkpoint chunks: {component}"))
}

fn assert_completed_scoped_issue(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); 4],
) {
    assert_completed_scoped_issue_with_max_lsq(json, expected_memory, expected_registers, 3);
}

fn assert_completed_scoped_issue_with_max_lsq(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); 4],
    expected_max_lsq: u64,
) {
    assert_final_witness(json, expected_memory, expected_registers);
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        expected_max_lsq,
        "monotonic",
    );
}

fn assert_final_witness<const N: usize>(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); N],
) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(expected_memory),
        "final memory witness should match fixture semantics: {json}"
    );
    for (register, value) in expected_registers {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should match fixture semantics: {json}"
        );
    }
}

fn assert_memory_hierarchy_activity(json: &Value) {
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed scoped issue run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
    }
}

fn scoped_issue_artifact(json: &Value) -> &Value {
    json.pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing scoped issue arbitration JSON: {json}"))
}

fn issue_u64(issue: &Value, field: &str) -> u64 {
    issue
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("scoped issue JSON should expose {field}: {issue}"))
}

fn queue_u64(queue: &Value, field: &str) -> u64 {
    queue
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 issue queue JSON should expose {field}: {queue}"))
}

fn assert_scoped_issue_native_stats(json: &Value, issue: &Value) {
    for (json_field, stat_field, unit) in SCOPED_ISSUE_STATS {
        assert_json_stat(
            json,
            &format!("sim.cpu0.o3.{stat_field}"),
            unit,
            issue_u64(issue, json_field),
            "monotonic",
        );
    }
}

fn assert_scoped_issue_stats_dump(dump: &Value, issue: &Value) {
    for (json_field, stat_field, unit) in SCOPED_ISSUE_STATS {
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu0.o3.{stat_field}"),
            "counter",
            unit,
            issue_u64(issue, json_field),
            "resettable",
        );
    }
}

fn assert_stats_dump_after_scoped_issue_activity(json: &Value, dump: &Value, dump_event: &Value) {
    let dump_issue_tick = event_u64(dump_event, "issue_tick");
    let dump_action_tick = dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("stats dump action should expose tick: {dump}"));
    assert!(
        dump_action_tick >= dump_issue_tick,
        "stats dump action must not precede its O3 issue boundary: dump={dump}, event={dump_event}"
    );
    for pc in [LOAD_PC, BRANCH_PC, SECOND_ROW_PC, THIRD_ROW_PC] {
        let event = event_at_pc(json, pc);
        assert!(
            event_u64(event, "issue_tick") < dump_issue_tick,
            "scoped issue row {pc} must issue before m5_dump_stats: row={event}, dump={dump_event}"
        );
    }
    for event in o3_trace_events(json)
        .iter()
        .filter(|event| event.pointer("/system_event").and_then(Value::as_bool) != Some(true))
    {
        assert!(
            event_u64(event, "issue_tick") < dump_issue_tick,
            "no non-system O3 row may issue at or after m5_dump_stats before comparing dump samples to final issue stats: row={event}, dump={dump_event}"
        );
    }
}

fn assert_scoped_issue_surface_absent(json: &Value) {
    assert!(
        json.pointer("/cores/0/o3_runtime/issue").is_none(),
        "timing mode should not expose detailed O3 scoped issue JSON: {json}"
    );
    for (_, stat_field, _) in SCOPED_ISSUE_STATS {
        assert_json_stat_absent(json, &format!("sim.cpu0.o3.{stat_field}"));
    }
}

fn scoped_issue_json(path: &Path, memory_system: &str, issue_width: usize, max_tick: u64) -> Value {
    scoped_issue_json_with_args(path, memory_system, issue_width, max_tick, &[])
}

fn scoped_issue_json_with_args(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
    extra_args: &[&str],
) -> Value {
    let mut command = scoped_issue_command(path, memory_system, issue_width, max_tick);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn scoped_issue_json_with_mode(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
    switch_mode: &str,
) -> Value {
    let output =
        scoped_issue_command_with_mode(path, memory_system, issue_width, max_tick, switch_mode)
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn scoped_issue_command(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
) -> Command {
    scoped_issue_command_with_stats_format(path, memory_system, issue_width, max_tick, "json")
}

fn scoped_issue_command_with_stats_format(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
    stats_format: &str,
) -> Command {
    scoped_issue_command_with_mode_and_stats_format(
        path,
        memory_system,
        issue_width,
        max_tick,
        "detailed",
        stats_format,
    )
}

fn scoped_issue_command_with_mode(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
    switch_mode: &str,
) -> Command {
    scoped_issue_command_with_mode_and_stats_format(
        path,
        memory_system,
        issue_width,
        max_tick,
        switch_mode,
        "json",
    )
}

fn scoped_issue_command_with_mode_and_stats_format(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
    switch_mode: &str,
    stats_format: &str,
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
        stats_format,
        "--execute",
    ]);
    if stats_format == "json" {
        command.args(["--debug-flags", "O3,Data,Fetch,Memory,HostAction"]);
    }
    command.args([
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--riscv-o3-issue-width",
        &issue_width.to_string(),
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        switch_mode,
        "--dump-memory",
        "0x800000a0:16",
    ]);
    command
}

fn scoped_issue_fu_json(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
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
            "--riscv-o3-issue-width",
            &issue_width.to_string(),
            "--memory-system",
            memory_system,
            "--m5-switch-cpu-mode",
            "detailed",
            "--dump-memory",
            "0x800000a0:16",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn general_iq_oldest_ready_json(path: &Path, issue_width: usize, max_tick: u64) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
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
            "--riscv-execution-mode",
            "detailed",
            "--riscv-o3-scalar-memory-depth",
            "1",
            "--riscv-o3-scalar-live-window-depth",
            "5",
            "--riscv-o3-issue-width",
            &issue_width.to_string(),
            "--riscv-o3-writeback-width",
            "4",
            "--memory-system",
            "direct",
            "--memory-route-delay",
            "80",
            "--dump-memory",
            "0x80000060:16",
            "--debug-flags",
            "O3,Data,Fetch,Memory,HostAction",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn o3_trace_events(json: &Value) -> &[Value] {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("O3 trace should expose events: {json}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    o3_trace_events(json)
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedIssueCase {
    CrossResource,
    SameMultiply,
}

fn scoped_issue_binary(name: &str, case: ScopedIssueCase) -> std::path::PathBuf {
    scoped_issue_binary_with_dump(name, case, false)
}

fn scoped_issue_stats_dump_binary(name: &str) -> std::path::PathBuf {
    scoped_issue_binary_with_dump(name, ScopedIssueCase::CrossResource, true)
}

fn scoped_issue_binary_with_dump(
    name: &str,
    case: ScopedIssueCase,
    dump_stats: bool,
) -> std::path::PathBuf {
    let data_start = 160_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(5, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        i_type(11, 0, 0x0, 3, 0x13),
        i_type(17, 0, 0x0, 4, 0x13),
        i_type(2, 0, 0x0, 5, 0x13),
        i_type(1, 0, 0x0, 6, 0x13),
        i_type(2, 0, 0x0, 7, 0x13),
        i_type(7, 0, 0x0, 13, 0x13),
        i_type(18, 0, 0x0, 15, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(8, 7, 6, 0b000),
    ]);
    match case {
        ScopedIssueCase::CrossResource => {
            words.extend([r_type(1, 3, 2, 0x0, 14, 0x33), i_type(1, 4, 0x0, 15, 0x13)])
        }
        ScopedIssueCase::SameMultiply => words.extend([
            r_type(1, 3, 2, 0x0, 14, 0x33),
            r_type(1, 5, 4, 0x0, 15, 0x33),
        ]),
    }
    words.extend([
        s_type(4, 13, 10, 0b010),
        s_type(8, 14, 10, 0b010),
        s_type(12, 15, 10, 0b010),
    ]);
    if dump_stats {
        words.extend([i_type(0, 0, 0x0, 10, 0x13), i_type(0, 0, 0x0, 11, 0x13)]);
        words.push(m5op(M5_DUMP_STATS));
    }
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn general_iq_oldest_ready_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![i_type(84, 0, 0x0, 1, 0x13), i_type(7, 0, 0x0, 2, 0x13)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b011, 5, 0x03),
        r_type(1, 2, 1, 0x4, 6, 0x33),
        i_type(-11, 6, 0x0, 7, 0x13),
        i_type(5, 0, 0x0, 8, 0x13),
        r_type(1, 2, 1, 0x0, 9, 0x33),
        s_type(4, 7, 10, 0b010),
        s_type(8, 8, 10, 0b010),
        s_type(12, 9, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0]);
    let program = riscv64_program(&words);
    temp_binary(name, &riscv64_elf(0x8000_0000, 0x8000_0000, &program))
}

fn scoped_issue_fu_head_binary(name: &str) -> std::path::PathBuf {
    let data_start = 160_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        r_type(1, 2, 1, 0x4, 3, 0x33),
        i_type(5, 0, 0x0, 4, 0x13),
        i_type(-11, 3, 0x0, 5, 0x13),
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13),
        s_type(0, 3, 12, 0b010),
        s_type(4, 4, 12, 0b010),
        s_type(8, 5, 12, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
