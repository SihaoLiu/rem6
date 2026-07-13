use std::process::Command;

use serde_json::Value;

use crate::support::*;

const M5_EXIT: u32 = 0x21;
const M5_CHECKPOINT: u32 = 0x43;

#[test]
fn rem6_run_repeats_m5_checkpoint_host_actions_when_period_is_set_from_real_riscv_execution() {
    let mut words = vec![
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(5, 0, 0x0, 11, 0x13),
        m5op(M5_CHECKPOINT),
        i_type(24, 0, 0x0, 10, 0x13),
        m5op(M5_EXIT),
    ];
    words.extend(std::iter::repeat_n(i_type(0, 0, 0x0, 0, 0x13), 24));
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-periodic-checkpoint-host-actions", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1)
    );

    let checkpoint_ticks = action_ticks(host_actions, "checkpoints");
    assert_eq!(checkpoint_ticks, vec![17, 22, 27, 32, 37, 42, 47, 52]);
    for checkpoint in host_actions
        .pointer("/checkpoints")
        .and_then(Value::as_array)
        .expect("missing checkpoints")
    {
        assert_eq!(
            checkpoint.pointer("/label").and_then(Value::as_str),
            Some("gem5-m5-checkpoint")
        );
        assert!(
            checkpoint
                .pointer("/payload_bytes")
                .and_then(Value::as_u64)
                .is_some_and(|bytes| bytes > 0),
            "checkpoint should carry a nonempty payload: {checkpoint}"
        );
    }
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
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
