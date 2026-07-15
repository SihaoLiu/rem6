use super::window_support::{
    assert_branch_kind_and_link, assert_drained_control_runtime, assert_final_execution_mode,
    assert_hierarchy_activity, assert_link_rename_maps_to_call_destination, assert_no_data_address,
    assert_no_fetch_pc, assert_no_o3_stats, assert_ordered_commits, assert_pointer_u64_gt,
    assert_register_absent_or_zero, assert_stopped_by_host, control_window_command,
    resident_rob_pcs, run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const DIRECT_WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];
const DIRECT_LOAD_PC: &str = "0x80000010";
const DIRECT_CALL_PC: &str = "0x80000014";
const DIRECT_FALLTHROUGH_STORE_PC: &str = "0x80000018";
const DIRECT_TARGET_ADD_PC: &str = "0x80000020";
const DIRECT_TARGET_DEPENDENT_PC: &str = "0x80000024";
const DIRECT_RESULT_STORE_PC: &str = "0x80000028";
const FALLTHROUGH_STORE_ADDRESS: &str = "0x80000108";
const FALLTHROUGH_STORE_12_ADDRESS: &str = "0x8000010c";

#[test]
fn rem6_run_o3_link_kind_direct_call_commits_direct() {
    let path = direct_call_binary("o3-link-kind-direct-call", 0);
    let completed = run_link_kind_json(&path, "direct", 2_500, "detailed", &DIRECT_WIDTH_ARGS);

    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(register_value(&completed, "x1"), 0x8000_0018);
    assert_eq!(register_value(&completed, "x13"), 42);
    assert_eq!(register_value(&completed, "x14"), 45);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002d0000000000000000000000")
    );
    assert_no_data_address(&completed, FALLTHROUGH_STORE_ADDRESS);
    assert!(event_at_pc_if_present(&completed, DIRECT_FALLTHROUGH_STORE_PC).is_none());

    let load = event_at_pc(&completed, DIRECT_LOAD_PC);
    let call = event_at_pc(&completed, DIRECT_CALL_PC);
    let target_add = event_at_pc(&completed, DIRECT_TARGET_ADD_PC);
    let target_dependent = event_at_pc(&completed, DIRECT_TARGET_DEPENDENT_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_eq!(
        call.pointer("/branch_kind").and_then(Value::as_str),
        Some("call_direct")
    );
    assert_eq!(
        call.pointer("/branch_link_register_write")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_u64(call, "issue_tick") < response_tick);
    assert!(event_u64(call, "writeback_tick") <= event_u64(call, "commit_tick"));
    for event in [target_add, target_dependent] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!([load, call, target_add, target_dependent]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));

    assert_eq!(
        event_u64(call, "issue_tick"),
        event_u64(target_add, "issue_tick")
    );
    assert!(
        event_u64(target_add, "writeback_tick") > event_u64(call, "writeback_tick"),
        "width-one writeback should defer the first target ADDI after the linked call: call={call} target={target_add}"
    );
    assert_pointer_u64_gt(
        &completed,
        "/cores/0/o3_runtime/writeback_port/deferred_rows",
        0,
    );

    let live_tick = event_u64(target_dependent, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_link_kind_json(&path, "direct", live_tick, "detailed", &DIRECT_WIDTH_ARGS);
    assert_eq!(
        resident_rob_pcs(&resident),
        [
            DIRECT_LOAD_PC,
            DIRECT_CALL_PC,
            DIRECT_TARGET_ADD_PC,
            DIRECT_TARGET_DEPENDENT_PC,
        ]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(register_value(&resident, "x1"), 0x11);
    assert_link_rename_maps_to_call_destination(&resident, DIRECT_CALL_PC, 1);
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
    assert_pointer_u64_gt(&completed, "/cores/0/branch_predictor/ras/pushes", 0);
}

#[test]
fn rem6_run_o3_link_kind_indirect_call_commits_cache_fabric_dram() {
    let path = indirect_call_binary("o3-link-kind-indirect-call");
    let completed = run_link_kind_json(&path, "cache-fabric-dram", 3_000, "detailed", &[]);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0020);
    assert_eq!(register_value(&completed, "x13"), 42);
    assert_eq!(register_value(&completed, "x14"), 45);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002d0000000000000000000000")
    );
    assert_no_data_address(&completed, FALLTHROUGH_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x80000018");
    let call = event_at_pc(&completed, "0x8000001c");
    let target_add = event_at_pc(&completed, "0x8000002c");
    let target_dependent = event_at_pc(&completed, "0x80000030");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_indirect", true);
    for event in [call, target_add, target_dependent] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert_ordered_commits([load, call, target_add, target_dependent]);

    let live_tick = event_u64(target_dependent, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_link_kind_json(&path, "cache-fabric-dram", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x80000018", "0x8000001c", "0x8000002c", "0x80000030",]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_pointer_u64_gt(
        &completed,
        "/cores/0/branch_predictor/target_provider/indirect",
        0,
    );
    assert_pointer_u64_gt(&completed, "/cores/0/branch_predictor/ras/pushes", 0);
    assert_hierarchy_activity(&completed);
}

#[test]
fn rem6_run_o3_link_kind_return_uses_committed_ras_entry() {
    let path = return_ras_binary("o3-link-kind-return-ras");
    let completed = run_link_kind_json(&path, "direct", 2_500, "detailed", &[]);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0010);
    assert_eq!(register_value(&completed, "x13"), 42);
    assert_eq!(register_value(&completed, "x14"), 45);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002d0000000000000000000000")
    );
    assert_no_data_address(&completed, FALLTHROUGH_STORE_ADDRESS);
    assert!(event_at_pc_if_present(&completed, "0x8000002c").is_none());

    let load = event_at_pc(&completed, "0x80000024");
    let ret = event_at_pc(&completed, "0x80000028");
    let target_add = event_at_pc(&completed, "0x80000010");
    let target_dependent = event_at_pc(&completed, "0x80000014");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(ret, "return", false);
    for event in [ret, target_add, target_dependent] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert_ordered_commits([load, ret, target_add, target_dependent]);

    let live_tick = event_u64(target_dependent, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_link_kind_json(&path, "direct", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x80000024", "0x80000028", "0x80000010", "0x80000014",]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(register_value(&resident, "x1"), 0x8000_0010);
    for pointer in [
        "/cores/0/branch_predictor/ras/pushes",
        "/cores/0/branch_predictor/ras/pops",
        "/cores/0/branch_predictor/ras/used",
        "/cores/0/branch_predictor/ras/correct",
        "/cores/0/branch_predictor/target_provider/ras",
    ] {
        assert_pointer_u64_gt(&completed, pointer, 0);
    }
}

#[test]
fn rem6_run_o3_link_kind_older_branch_discards_linked_call() {
    let path = older_branch_discards_call_binary("o3-link-kind-older-branch");
    let completed = run_link_kind_json(&path, "cache-fabric-dram", 3_000, "detailed", &[]);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x11);
    assert_register_absent_or_zero(&completed, "x13");
    assert_eq!(register_value(&completed, "x15"), 0x33);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000330000000000000000000000")
    );
    assert_no_data_address(&completed, FALLTHROUGH_STORE_ADDRESS);
    assert_no_data_address(&completed, FALLTHROUGH_STORE_12_ADDRESS);
    let load = event_at_pc(&completed, "0x80000014");
    let branch = event_at_pc(&completed, "0x80000018");
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
        Some(true)
    );
    assert_eq!(
        branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.pointer("/branch_squash").and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, "0x8000001c").is_none());
    assert_pointer_u64_gt(&completed, "/cores/0/branch_predictor/ras/pushes", 0);
    assert_pointer_u64_gt(&completed, "/cores/0/branch_predictor/ras/squashes", 0);
    assert_hierarchy_activity(&completed);

    let response_tick = event_u64(load, "lsq_data_response_tick");
    let live_tick = event_u64(branch, "issue_tick") + 2;
    assert!(live_tick < response_tick);
    let resident = run_link_kind_json(&path, "cache-fabric-dram", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x80000014", "0x80000018", "0x8000001c", "0x80000028",]
    );
}

#[test]
fn rem6_run_o3_link_kind_live_target_sources_stay_terminal() {
    let load_target = load_produced_target_binary("o3-link-kind-load-produced-target");
    let load_completed = run_link_kind_json(&load_target, "direct", 2_500, "detailed", &[]);
    assert_stopped_by_host(&load_completed);
    assert_eq!(register_value(&load_completed, "x5"), 0x8000_0018);
    assert_eq!(register_value(&load_completed, "x13"), 42);
    assert_eq!(
        load_completed
            .pointer("/memory/0/hex")
            .and_then(Value::as_str),
        Some("200000802a0000000000000000000000")
    );
    assert_no_data_address(&load_completed, FALLTHROUGH_STORE_ADDRESS);
    let load = event_at_pc(&load_completed, "0x80000010");
    let call = event_at_pc(&load_completed, "0x80000014");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_indirect", true);
    assert!(event_u64(call, "issue_tick") >= response_tick);
    let live_tick = live_midpoint(load);
    let resident = run_link_kind_json(&load_target, "direct", live_tick, "detailed", &[]);
    assert_eq!(resident_rob_pcs(&resident), ["0x80000010", "0x80000014"]);
    assert_no_fetch_pc(&resident, "0x80000020");

    let alu_target = live_alu_target_binary("o3-link-kind-live-alu-target");
    let alu_completed = run_link_kind_json(&alu_target, "direct", 2_500, "detailed", &[]);
    assert_stopped_by_host(&alu_completed);
    assert_eq!(register_value(&alu_completed, "x5"), 0x8000_0020);
    assert_eq!(register_value(&alu_completed, "x13"), 42);
    assert_eq!(
        alu_completed
            .pointer("/memory/0/hex")
            .and_then(Value::as_str),
        Some("2a0000002a0000000000000000000000")
    );
    assert_no_data_address(&alu_completed, FALLTHROUGH_STORE_ADDRESS);
    let load = event_at_pc(&alu_completed, "0x80000010");
    let call = event_at_pc(&alu_completed, "0x8000001c");
    assert_branch_kind_and_link(call, "call_indirect", true);
    let live_tick = event_u64(call, "issue_tick") + 1;
    assert!(live_tick < event_u64(load, "lsq_data_response_tick"));
    assert!(event_u64(event_at_pc(&alu_completed, "0x80000018"), "issue_tick") < live_tick);
    let resident = run_link_kind_json(&alu_target, "direct", live_tick, "detailed", &[]);
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x80000010", "0x80000014", "0x80000018", "0x8000001c",]
    );
    assert_no_fetch_pc(&resident, "0x8000002c");
}

#[test]
fn rem6_run_host_switch_transfers_o3_link_kind_window() {
    let path = direct_call_binary("o3-link-kind-switch", 0);
    let baseline = run_link_kind_json(&path, "direct", 2_500, "detailed", &DIRECT_WIDTH_ARGS);
    let load = event_at_pc(&baseline, DIRECT_LOAD_PC);
    let switch_tick = event_u64(
        event_at_pc(&baseline, DIRECT_TARGET_DEPENDENT_PC),
        "issue_tick",
    ) + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let mut switch_args = DIRECT_WIDTH_ARGS.to_vec();
    switch_args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
    let switched = run_link_kind_json(&path, "direct", 2_500, "detailed", &switch_args);
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
        .unwrap_or_else(|| panic!("missing link-kind timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("link-kind state transfer");
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
    for pc in [
        DIRECT_LOAD_PC,
        DIRECT_CALL_PC,
        DIRECT_TARGET_ADD_PC,
        DIRECT_TARGET_DEPENDENT_PC,
    ] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "link-kind transfer must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
    assert_eq!(register_value(&switched, "x1"), 0x8000_0018);
    assert_eq!(
        switched.pointer("/memory/0/hex").and_then(Value::as_str),
        baseline.pointer("/memory/0/hex").and_then(Value::as_str)
    );
    assert_final_execution_mode(&switched, "timing");
    assert_drained_control_runtime(&switched);
}

#[test]
fn rem6_run_o3_link_kind_checkpoint_boundary() {
    let path = direct_call_binary("o3-link-kind-checkpoint", 8);
    let baseline = run_link_kind_json(&path, "direct", 2_500, "detailed", &DIRECT_WIDTH_ARGS);
    let live_tick = event_u64(
        event_at_pc(&baseline, DIRECT_TARGET_DEPENDENT_PC),
        "issue_tick",
    ) + 1;
    let live_arg = format!("{live_tick}:link-kind-live");
    let mut live_command =
        control_window_command(&path, "direct", 2_500, "detailed", 3, DATA_ADDRESS, 16);
    let mut live_args = DIRECT_WIDTH_ARGS.to_vec();
    live_args.extend(["--host-checkpoint", live_arg.as_str()]);
    live_command.args(live_args.iter().copied());
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live link-kind checkpoint should fail closed: {stderr}"
    );

    let checkpoint_tick = event_u64(
        event_at_pc(&baseline, DIRECT_RESULT_STORE_PC),
        "commit_tick",
    ) + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:link-kind-drained");
    let restore_arg = format!("{restore_tick}:link-kind-drained");
    let mut restore_args = DIRECT_WIDTH_ARGS.to_vec();
    restore_args.extend([
        "--host-checkpoint",
        checkpoint_arg.as_str(),
        "--host-restore-checkpoint",
        restore_arg.as_str(),
    ]);
    let restored = run_link_kind_json(&path, "direct", 2_500, "detailed", &restore_args);
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
        .expect("drained link-kind checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    assert!(checkpoint_component_chunks(cpu0).iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained link-kind O3 runtime checkpoint");
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
    assert_eq!(register_value(&restored, "x1"), 0x8000_0018);
    assert_eq!(
        restored.pointer("/memory/0/hex").and_then(Value::as_str),
        baseline.pointer("/memory/0/hex").and_then(Value::as_str)
    );
    assert_drained_control_runtime(&restored);
}

#[test]
fn rem6_run_timing_suppresses_o3_link_kind_window() {
    let path = direct_call_binary("o3-link-kind-timing", 0);
    let timing = run_link_kind_json(&path, "direct", 2_500, "timing", &[]);

    assert_stopped_by_host(&timing);
    assert_eq!(register_value(&timing, "x1"), 0x8000_0018);
    assert_eq!(register_value(&timing, "x13"), 42);
    assert_eq!(register_value(&timing, "x14"), 45);
    assert_eq!(
        timing.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002d0000000000000000000000")
    );
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert_no_o3_stats(&timing);
}

fn direct_call_binary(name: &str, exit_padding_words: usize) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0x11, 0, 0x0, 1, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(42, 0, 0x0, 13, 0x13),
        i_type(3, 13, 0x0, 14, 0x13),
        s_type(4, 14, 18, 0b010),
    ]);
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_link_kind_binary(name, words, [42, 0, 0, 0])
}

fn indirect_call_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0x2c - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(42, 0, 0x0, 13, 0x13),
        i_type(3, 13, 0x0, 14, 0x13),
        s_type(4, 14, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_link_kind_binary(name, words, [42, 0, 0, 0])
}

fn return_ras_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        j_type(24, 1),
        i_type(42, 0, 0x0, 13, 0x13),
        i_type(3, 13, 0x0, 14, 0x13),
        j_type(28, 0),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 1, 0x0, 0, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        s_type(4, 14, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_link_kind_binary(name, words, [42, 0, 0, 0])
}

fn older_branch_discards_call_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(0x11, 0, 0x0, 1, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        b_type(24, 7, 7, 0b000),
        j_type(12, 1),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(8, 13, 18, 0b010),
        i_type(0x33, 0, 0x0, 15, 0x13),
        s_type(4, 15, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_link_kind_binary(name, words, [42, 0, 0, 0])
}

fn load_produced_target_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b110, 11, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_link_kind_binary(name, words, [0x8000_0020, 0, 0, 0])
}

fn live_alu_target_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0x2c - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 5, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(42, 0, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_link_kind_binary(name, words, [42, 0, 0, 0])
}

fn finish_link_kind_binary(
    name: &str,
    mut words: Vec<u32>,
    data_words: [u32; 4],
) -> std::path::PathBuf {
    while words.len() * 4 < DATA_START as usize {
        words.push(0);
    }
    words.extend(data_words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn run_link_kind_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        3,
        DATA_ADDRESS,
        16,
        extra_args,
    )
}

fn live_midpoint(load: &Value) -> u64 {
    let issue_tick = event_u64(load, "issue_tick");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let midpoint = issue_tick + (response_tick - issue_tick) / 2;
    assert!(
        issue_tick < midpoint && midpoint < response_tick,
        "live midpoint must be strictly between issue and response: issue_tick={issue_tick} midpoint={midpoint} response_tick={response_tick} load={load}"
    );
    midpoint
}
