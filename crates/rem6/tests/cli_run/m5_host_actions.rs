use std::process::Command;

use serde_json::Value;

use crate::support::*;

const M5_WORK_BEGIN: u32 = 0x5a;
const M5_WORK_END: u32 = 0x5b;
const M5_EXIT: u32 = 0x21;
const M5_FAIL: u32 = 0x22;
const M5_SUM: u32 = 0x23;
const M5_RESET_STATS: u32 = 0x40;
const M5_DUMP_STATS: u32 = 0x41;
const M5_DUMP_RESET_STATS: u32 = 0x42;
const M5_CHECKPOINT: u32 = 0x43;
const M5_SWITCH_CPU: u32 = 0x52;
const M5_HYPERCALL: u32 = 0x71;

#[test]
fn rem6_run_emits_m5_work_marker_host_actions_from_real_riscv_execution() {
    let program = riscv64_program(&[
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(11, 0, 0x0, 10, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        i_type(12, 0, 0x0, 10, 0x13),
        i_type(8, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(12, 0, 0x0, 10, 0x13),
        i_type(8, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("host_stop")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(9)
    );
    assert_eq!(
        host_actions
            .pointer("/roi_begin_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/roi_end_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_work_marker(host_actions, "roi_begin", 0, 11, 7);
    assert_work_marker(host_actions, "roi_end", 0, 11, 7);
    assert_work_marker(host_actions, "roi_begin", 1, 12, 8);
    assert_work_marker(host_actions, "roi_end", 1, 12, 8);
}

#[test]
fn rem6_run_emits_m5_hypercall_host_action_detail_from_real_riscv_execution() {
    let program = riscv64_program(&[
        i_type(0x321, 0, 0x0, 10, 0x13),
        m5op(M5_HYPERCALL),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-hypercall-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/guest_host_call_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    let call = host_actions
        .pointer("/guest_host_calls/0")
        .expect("missing guest-host-call detail");
    assert_eq!(
        call.pointer("/selector").and_then(Value::as_u64),
        Some(0x321)
    );
    assert_eq!(
        call.pointer("/argument_count").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        call.pointer("/payload_bytes").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        call.pointer("/response_status").and_then(Value::as_i64),
        Some(-1)
    );
    assert_eq!(
        call.pointer("/response_return_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        call.pointer("/response_payload_bytes")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert!(call.pointer("/tick").and_then(Value::as_u64).is_some());
}

#[test]
fn rem6_run_emits_m5_switch_cpu_command_from_real_riscv_execution() {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(77, 0, 0x0, 11, 0x13),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/injected_command_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_injected_command(host_actions, 0, "switchcpu");
}

#[test]
fn rem6_run_executes_m5_sum_return_value_from_real_riscv_execution() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 10, 0x13), // addi a0, x0, 1
        i_type(2, 0, 0x0, 11, 0x13), // addi a1, x0, 2
        i_type(3, 0, 0x0, 12, 0x13), // addi a2, x0, 3
        i_type(4, 0, 0x0, 13, 0x13), // addi a3, x0, 4
        i_type(5, 0, 0x0, 14, 0x13), // addi a4, x0, 5
        i_type(6, 0, 0x0, 15, 0x13), // addi a5, x0, 6
        m5op(M5_SUM),
        i_type(21, 0, 0x0, 5, 0x13), // addi t0, x0, 21
        b_type(12, 5, 10, 0x1),      // bne a0, t0, fail
        i_type(0, 0, 0x0, 10, 0x13), // addi a0, x0, 0
        m5op(M5_EXIT),
        i_type(99, 0, 0x0, 11, 0x13), // addi a1, x0, 99
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-sum-return-value", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("host_stop")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_run_emits_m5_stats_host_action_details_from_real_riscv_execution() {
    let program = riscv64_program(&[
        m5op(M5_RESET_STATS),
        m5op(M5_DUMP_STATS),
        m5op(M5_DUMP_RESET_STATS),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-stats-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_stats_reset(host_actions, 0, 0, 3, 1);
    assert_stats_dump(host_actions, 0, 0, 5, 1, 3);
    assert_stats_dump(host_actions, 1, 1, 7, 1, 3);
    assert_stats_reset(host_actions, 1, 1, 7, 2);
}

#[test]
fn rem6_run_repeats_m5_stats_host_actions_when_period_is_set_from_real_riscv_execution() {
    let mut words = vec![
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(4, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_RESET_STATS),
        i_type(18, 0, 0x0, 10, 0x13),
        m5op(M5_EXIT),
    ];
    words.extend(std::iter::repeat_n(i_type(0, 0, 0x0, 0, 0x13), 16));
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-periodic-stats-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );

    let reset_ticks = action_ticks(host_actions, "stats_resets");
    let dump_ticks = action_ticks(host_actions, "stats_dumps");
    assert_eq!(reset_ticks, vec![7, 11, 15, 19, 23, 27]);
    assert_eq!(dump_ticks, reset_ticks);
}

#[test]
fn rem6_run_emits_m5_checkpoint_host_action_detail_from_real_riscv_execution() {
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-checkpoint-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_checkpoint(host_actions, 0, "gem5-m5-checkpoint", 3, 3);
    assert_checkpoint_component_chunks(
        host_actions,
        0,
        0,
        "cpu0",
        &[
            "bimode-branch-predictor",
            "branch-predictor",
            "fregs",
            "gshare-branch-predictor",
            "hart-run-state",
            "in-order-pipeline",
            "multiperspective-perceptron",
            "o3-pending-state",
            "o3-runtime-state",
            "pc",
            "pmp",
            "tage-sc-l-branch-predictor",
            "tournament-branch-predictor",
            "xregs",
        ],
    );
    assert_checkpoint_component_chunks(host_actions, 0, 1, "memory0", &["store"]);
    assert_checkpoint_counts_match_nested_details(host_actions, 0);
}

#[test]
fn rem6_run_m5_store_checkpoint_chunk_checksum_tracks_live_memory_state() {
    let (baseline, after_store) =
        run_m5_checkpoint_memory_checksums("m5-store-checkpoint-live", false);

    assert_ne!(after_store, baseline);
}

#[test]
fn rem6_run_emits_m5_dram_checkpoint_host_action_detail_from_real_riscv_execution() {
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-dram-checkpoint-host-action", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--dram-memory",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_checkpoint(host_actions, 0, "gem5-m5-checkpoint", 11, 11);
    assert_checkpoint_component_chunks(
        host_actions,
        0,
        0,
        "cpu0",
        &[
            "bimode-branch-predictor",
            "branch-predictor",
            "fregs",
            "gshare-branch-predictor",
            "hart-run-state",
            "in-order-pipeline",
            "multiperspective-perceptron",
            "o3-pending-state",
            "o3-runtime-state",
            "pc",
            "pmp",
            "tage-sc-l-branch-predictor",
            "tournament-branch-predictor",
            "xregs",
        ],
    );
    assert_checkpoint_component_chunks(host_actions, 0, 1, "memory0", &["dram"]);
    assert_checkpoint_counts_match_nested_details(host_actions, 0);
}

#[test]
fn rem6_run_m5_dram_checkpoint_chunk_checksum_tracks_live_memory_state() {
    let (baseline, after_store) =
        run_m5_checkpoint_memory_checksums("m5-dram-checkpoint-live", true);

    assert_ne!(after_store, baseline);
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}

fn run_m5_checkpoint_memory_checksums(name: &str, dram_memory: bool) -> (String, String) {
    let words = [
        m5op(M5_CHECKPOINT),
        u_type(0, 2, 0x17),            // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13),   // addi x2, x2, data offset
        i_type(0x5a, 0, 0x0, 5, 0x13), // addi x5, x0, 0x5a
        s_type(0, 5, 2, 0x2),          // sw x5, 0(x2)
        m5op(M5_CHECKPOINT),
        m5op(M5_EXIT),
    ];
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);
    let mut args = vec![
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "120",
        "--stats-format",
        "json",
        "--execute",
    ];
    if dram_memory {
        args.push("--dram-memory");
    } else {
        args.extend(["--memory-system", "direct"]);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let chunk_name = if dram_memory { "dram" } else { "store" };
    (
        checkpoint_chunk_checksum(host_actions, 0, "memory0", chunk_name),
        checkpoint_chunk_checksum(host_actions, 1, "memory0", chunk_name),
    )
}

fn assert_work_marker(
    host_actions: &Value,
    field: &str,
    index: usize,
    work_id: u64,
    thread_id: u64,
) {
    let action = host_actions
        .pointer(&format!("/{field}/{index}"))
        .unwrap_or_else(|| panic!("missing host action {field}[{index}]"));
    assert_eq!(
        action.pointer("/work_id").and_then(Value::as_u64),
        Some(work_id)
    );
    assert_eq!(
        action.pointer("/thread_id").and_then(Value::as_u64),
        Some(thread_id)
    );
    assert!(action.pointer("/tick").and_then(Value::as_u64).is_some());
}

fn assert_injected_command(host_actions: &Value, index: usize, command: &str) {
    let action = host_actions
        .pointer(&format!("/injected_commands/{index}"))
        .unwrap_or_else(|| panic!("missing injected command action {index}"));
    assert_eq!(
        action.pointer("/command").and_then(Value::as_str),
        Some(command)
    );
    assert!(action.pointer("/tick").and_then(Value::as_u64).is_some());
    assert!(action.pointer("/event").and_then(Value::as_u64).is_some());
    assert!(action.pointer("/source").and_then(Value::as_u64).is_some());
}

fn assert_stats_reset(host_actions: &Value, index: usize, id: u64, tick: u64, epoch: u64) {
    let action = host_actions
        .pointer(&format!("/stats_resets/{index}"))
        .unwrap_or_else(|| panic!("missing stats reset action {index}"));
    assert_eq!(action.pointer("/id").and_then(Value::as_u64), Some(id));
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(tick),
        "stats reset action {index}: {action}"
    );
    assert_eq!(
        action.pointer("/epoch").and_then(Value::as_u64),
        Some(epoch)
    );
}

fn action_ticks(host_actions: &Value, field: &str) -> Vec<u64> {
    host_actions
        .pointer(&format!("/{field}"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing host action list {field}"))
        .iter()
        .map(|action| {
            action
                .pointer("/tick")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing host action tick in {field}: {action}"))
        })
        .collect()
}

fn assert_stats_dump(
    host_actions: &Value,
    index: usize,
    id: u64,
    tick: u64,
    epoch: u64,
    reset_tick: u64,
) {
    let action = host_actions
        .pointer(&format!("/stats_dumps/{index}"))
        .unwrap_or_else(|| panic!("missing stats dump action {index}"));
    assert_eq!(action.pointer("/id").and_then(Value::as_u64), Some(id));
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(tick),
        "stats dump action {index}: {action}"
    );
    assert_eq!(
        action.pointer("/epoch").and_then(Value::as_u64),
        Some(epoch)
    );
    assert_eq!(
        action.pointer("/reset_tick").and_then(Value::as_u64),
        Some(reset_tick),
        "stats dump action {index}: {action}"
    );
}

fn assert_checkpoint(
    host_actions: &Value,
    index: usize,
    label: &str,
    tick: u64,
    manifest_tick: u64,
) {
    let action = host_actions
        .pointer(&format!("/checkpoints/{index}"))
        .unwrap_or_else(|| panic!("missing checkpoint action {index}"));
    assert_eq!(
        action.pointer("/label").and_then(Value::as_str),
        Some(label)
    );
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(tick),
        "checkpoint action {index}: {action}"
    );
    assert_eq!(
        action.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(manifest_tick),
        "checkpoint action {index}: {action}"
    );
    assert!(action.pointer("/event").and_then(Value::as_u64).is_some());
    assert!(action.pointer("/source").and_then(Value::as_u64).is_some());
    assert!(
        action
            .pointer("/component_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "checkpoint action {index}: {action}"
    );
    assert!(
        action
            .pointer("/chunk_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "checkpoint action {index}: {action}"
    );
    assert!(
        action
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "checkpoint action {index}: {action}"
    );
}

fn assert_checkpoint_component_chunks(
    host_actions: &Value,
    checkpoint_index: usize,
    component_index: usize,
    component: &str,
    chunks: &[&str],
) {
    let component_summary = host_actions
        .pointer(&format!(
            "/checkpoints/{checkpoint_index}/components/{component_index}"
        ))
        .unwrap_or_else(|| {
            panic!("missing checkpoint component {checkpoint_index}/{component_index}")
        });
    assert_eq!(
        component_summary
            .pointer("/component")
            .and_then(Value::as_str),
        Some(component)
    );
    assert_eq!(
        component_summary
            .pointer("/chunk_count")
            .and_then(Value::as_u64),
        Some(chunks.len() as u64)
    );
    assert!(
        component_summary
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "checkpoint component {component_index}: {component_summary}"
    );
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        let chunk_summary = component_summary
            .pointer(&format!("/chunks/{chunk_index}"))
            .unwrap_or_else(|| panic!("missing checkpoint chunk {chunk_index}"));
        assert_eq!(
            chunk_summary.pointer("/name").and_then(Value::as_str),
            Some(*chunk)
        );
        assert!(
            chunk_summary
                .pointer("/payload_bytes")
                .and_then(Value::as_u64)
                .is_some_and(|bytes| bytes > 0),
            "checkpoint chunk {chunk_index}: {chunk_summary}"
        );
        assert!(
            chunk_summary
                .pointer("/payload_checksum")
                .and_then(Value::as_str)
                .is_some_and(|checksum| checksum.starts_with("0x") && checksum.len() == 18),
            "checkpoint chunk {chunk_index}: {chunk_summary}"
        );
    }
}

fn checkpoint_chunk_checksum(
    host_actions: &Value,
    checkpoint_index: usize,
    component: &str,
    chunk: &str,
) -> String {
    let components = host_actions
        .pointer(&format!("/checkpoints/{checkpoint_index}/components"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint components {checkpoint_index}"));
    let component_summary = components
        .iter()
        .find(|summary| summary.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing checkpoint component {component}"));
    let chunks = component_summary
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint chunks for {component}"));
    chunks
        .iter()
        .find(|summary| summary.pointer("/name").and_then(Value::as_str) == Some(chunk))
        .and_then(|summary| summary.pointer("/payload_checksum"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing checkpoint chunk checksum {component}/{chunk}"))
        .to_string()
}

fn assert_checkpoint_counts_match_nested_details(host_actions: &Value, checkpoint_index: usize) {
    let checkpoint = host_actions
        .pointer(&format!("/checkpoints/{checkpoint_index}"))
        .unwrap_or_else(|| panic!("missing checkpoint action {checkpoint_index}"));
    let components = checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoint components {checkpoint_index}"));
    let component_count = components.len() as u64;
    let chunk_count = components
        .iter()
        .map(|component| {
            component
                .pointer("/chunks")
                .and_then(Value::as_array)
                .map_or(0, |chunks| chunks.len() as u64)
        })
        .sum::<u64>();
    assert_eq!(
        checkpoint
            .pointer("/component_count")
            .and_then(Value::as_u64),
        Some(component_count),
        "checkpoint action {checkpoint_index}: {checkpoint}"
    );
    assert_eq!(
        checkpoint.pointer("/chunk_count").and_then(Value::as_u64),
        Some(chunk_count),
        "checkpoint action {checkpoint_index}: {checkpoint}"
    );
}
