use std::process::Command;

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_delays_architectural_visibility_until_scheduled_commit_stage() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-scheduled-commit", &elf);

    for (memory_system, cores, completed_tick_limit) in [
        ("direct", 1, 120),
        ("direct", 2, 120),
        ("cache-fabric-dram", 1, 600),
    ] {
        let completed = run_pipeline_timing(&path, cores, completed_tick_limit, memory_system);
        let first_fetch_response_tick = completed
            .pointer("/debug/memory_trace")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("fetch"))
            .filter(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
            })
            .filter_map(|record| record.pointer("/tick").and_then(Value::as_u64))
            .min()
            .expect("completed run should expose the first fetch response");
        let commit_ready_tick = first_fetch_response_tick + 4;

        let before_commit = run_pipeline_timing(&path, cores, commit_ready_tick, memory_system);
        assert_eq!(
            before_commit
                .pointer("/simulation/status")
                .and_then(Value::as_str),
            Some("stopped_at_tick_limit")
        );
        assert_eq!(
            before_commit
                .pointer("/simulation/final_tick")
                .and_then(Value::as_u64),
            Some(commit_ready_tick)
        );
        for cpu in 0..cores {
            let core = &before_commit["cores"][cpu];
            assert_eq!(core["committed_instructions"].as_u64(), Some(0));
            assert_eq!(core.pointer("/registers/x5"), None);
            assert_eq!(
                core.pointer("/in_order_pipeline/stage_in_flight/commit")
                    .and_then(Value::as_u64),
                Some(1)
            );
            assert_eq!(
                core.pointer("/in_order_pipeline/stage_retired/commit")
                    .and_then(Value::as_u64),
                Some(0)
            );
        }

        let after_commit = run_pipeline_timing(&path, cores, commit_ready_tick + 1, memory_system);
        for cpu in 0..cores {
            let core = &after_commit["cores"][cpu];
            assert_eq!(core["committed_instructions"].as_u64(), Some(1));
            assert_eq!(
                core.pointer("/registers/x5").and_then(Value::as_str),
                Some("0x7")
            );
            assert_eq!(
                core.pointer("/in_order_pipeline/stage_retired/commit")
                    .and_then(Value::as_u64),
                Some(1)
            );
        }
    }
}

fn run_pipeline_timing(
    path: &std::path::Path,
    cores: usize,
    max_tick: u64,
    memory_system: &str,
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
            "--memory-system",
            memory_system,
            "--cores",
            &cores.to_string(),
            "--debug-flags",
            "Memory",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
