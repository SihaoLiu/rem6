use std::process::Command;

use serde_json::Value;

use crate::support::*;

const M5_EXIT: u32 = 0x21;
const M5_RESET_STATS: u32 = 0x40;
const M5_DUMP_STATS: u32 = 0x41;
const M5_WORK_BEGIN: u32 = 0x5a;
const M5_WORK_END: u32 = 0x5b;

#[test]
fn rem6_run_riscv_se_exposes_m5_roi_and_stat_hooks_as_run_stats() {
    let program = riscv64_program(&[
        i_type(21, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        m5op(M5_RESET_STATS),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(2, 5, 0x0, 5, 0x13),
        m5op(M5_DUMP_STATS),
        i_type(21, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-m5-roi-stat-hooks", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/riscv_boot/se").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        json.pointer("/riscv_unknown_syscalls")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/total_action_count")
            .and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        host_actions
            .pointer("/roi_begin_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/roi_end_count")
            .and_then(Value::as_u64),
        Some(1)
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
    assert_work_marker(host_actions, "roi_begin", 0, 21, 3);
    assert_work_marker(host_actions, "roi_end", 0, 21, 3);

    assert_stat(&stdout, "sim.host_actions.total", "Count", 7, "monotonic");
    assert_stat(
        &stdout,
        "sim.host_actions.roi_begin",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(&stdout, "sim.host_actions.roi_end", "Count", 1, "monotonic");
    assert_stat(
        &stdout,
        "sim.host_actions.stats_resets",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.host_actions.stats_dumps",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(&stdout, "sim.host_actions.stops", "Count", 1, "monotonic");
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
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
