use std::{collections::BTreeMap, process::Command};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_pipeline_debug_flag_attributes_data_wait_stage_cycles() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),        // sd x6, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&[0; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-debug-data-wait-stage-cycles", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "Pipeline",
            "--dump-memory",
            "0x80000020:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Pipeline".to_string())])
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));

    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let wait_records = trace
        .iter()
        .filter(|record| record.get("stall_cause").and_then(Value::as_str) == Some("data_wait"))
        .collect::<Vec<_>>();
    assert!(
        !wait_records.is_empty(),
        "data-wait run should emit data_wait stall records: {trace:?}"
    );
    let stall_cycles = wait_records
        .iter()
        .map(|record| json_record_u64(record, "stall_cycles"))
        .sum::<u64>();
    assert!(
        stall_cycles > 0,
        "data-wait stall records: {wait_records:?}"
    );

    let mut stage_blocked = BTreeMap::<String, u64>::new();
    let mut stage_blocked_cycles = BTreeMap::<String, u64>::new();
    for record in &wait_records {
        let stall_cycles = json_record_u64(record, "stall_cycles");
        let mut stage_present = BTreeMap::<String, bool>::new();
        for blocked in record_array(record, "resource_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing blocked instruction stage: {blocked}"));
            let stage = stat_path_segment(stage);
            *stage_blocked.entry(stage.clone()).or_default() += 1;
            stage_present.insert(stage, true);
        }
        for stage in stage_present.keys() {
            *stage_blocked_cycles.entry(stage.clone()).or_default() += stall_cycles;
        }
    }
    assert!(
        !stage_blocked_cycles.is_empty(),
        "data_wait stall records should preserve blocked stages: {wait_records:?}"
    );

    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.stall_cause.data_wait.stall_cycles",
        "Count",
        stall_cycles,
        "monotonic",
    );
    for (stage, blocked) in stage_blocked {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.stall_cause.data_wait.stage.{stage}.resource_blocked"
            ),
            "Count",
            blocked,
            "monotonic",
        );
    }
    for (stage, cycles) in stage_blocked_cycles {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.stall_cause.data_wait.stage.{stage}.resource_blocked_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
    }
    assert!(!stdout.contains(
        "sim.debug.pipeline_trace.stall_cause.fetch_wait.stage.execute.resource_blocked_cycles"
    ));
    assert!(!stdout.contains(
        "sim.debug.pipeline_trace.stall_cause.execute_wait.stage.execute.resource_blocked_cycles"
    ));
}

fn json_record_u64(record: &Value, field: &str) -> u64 {
    record
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric field {field} in record {record}"))
}

fn record_array<'a>(record: &'a Value, field: &str) -> &'a [Value] {
    record
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing array field {field} in record {record}"))
}
