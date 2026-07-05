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
    let mut aggregate_stage_blocked = BTreeMap::<String, u64>::new();
    let mut aggregate_stage_blocked_cycles = BTreeMap::<String, u64>::new();
    for record in trace {
        let mut stage_present = BTreeMap::<String, bool>::new();
        for blocked in record_array(record, "resource_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing blocked instruction stage: {blocked}"));
            let stage = stat_path_segment(stage);
            *aggregate_stage_blocked.entry(stage.clone()).or_default() += 1;
            stage_present.insert(stage, true);
        }
        for stage in stage_present.keys() {
            *aggregate_stage_blocked_cycles
                .entry(stage.clone())
                .or_default() += 1;
        }
    }
    assert!(
        !aggregate_stage_blocked_cycles.is_empty(),
        "data-wait run should emit aggregate resource-blocked stage cycles: {trace:?}"
    );
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
    for (stage, blocked) in aggregate_stage_blocked {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.resource_blocked"),
            "Count",
            blocked,
            "monotonic",
        );
    }
    for (stage, cycles) in aggregate_stage_blocked_cycles {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.resource_blocked_cycles"),
            "Cycle",
            cycles,
            "monotonic",
        );
    }
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

#[test]
fn rem6_run_pipeline_debug_flag_attributes_trap_redirect_stage_cycles() {
    let program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        i_type(9, 0, 0x0, 6, 0x13), // wrong-path addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-debug-trap-redirect-stage-cycles", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-in-order-width",
            "3",
            "--debug-flags",
            "Pipeline",
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
    assert!(
        !stdout.contains("\"x6\":\"0x9\""),
        "wrong-path instruction must be squashed: {stdout}"
    );

    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let redirect_records = trace
        .iter()
        .filter(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("trap_redirect")
        })
        .collect::<Vec<_>>();
    assert!(
        !redirect_records.is_empty(),
        "trap run should emit trap_redirect records: {trace:?}"
    );

    let mut stage_flushed = BTreeMap::<String, u64>::new();
    let mut stage_flushed_cycles = BTreeMap::<String, u64>::new();
    for record in &redirect_records {
        let mut stage_present = BTreeMap::<String, bool>::new();
        for flushed in record_array(record, "flushed") {
            let stage = flushed
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
            let stage = stat_path_segment(stage);
            *stage_flushed.entry(stage.clone()).or_default() += 1;
            stage_present.insert(stage, true);
        }
        for stage in stage_present.keys() {
            *stage_flushed_cycles.entry(stage.clone()).or_default() += 1;
        }
    }
    assert!(
        !stage_flushed_cycles.is_empty(),
        "trap redirect records should preserve flushed stages: {redirect_records:?}"
    );
    for (stage, flushed) in &stage_flushed {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.redirect_cause.trap_redirect.stage.{stage}.flushed"),
            "Count",
            *flushed,
            "monotonic",
        );
    }
    for (stage, cycles) in stage_flushed_cycles {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.redirect_cause.trap_redirect.stage.{stage}.flushed_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
    }
    let branch_redirect_stage_flushed_cycles = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("stats array")
        .iter()
        .filter_map(|stat| stat.get("path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.debug.pipeline_trace.redirect_cause.branch_prediction.stage.")
                && path.ends_with(".flushed_cycles")
        })
        .collect::<Vec<_>>();
    assert!(
        branch_redirect_stage_flushed_cycles.is_empty(),
        "trap-only run should not emit branch_prediction redirect-cause stage flushed-cycle stats: {branch_redirect_stage_flushed_cycles:?}"
    );
}

#[test]
fn rem6_run_pipeline_debug_flag_attributes_in_flight_stage_cycles() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13),  // addi x5, x0, 1
        i_type(2, 0, 0x0, 6, 0x13),  // addi x6, x0, 2
        i_type(3, 0, 0x0, 7, 0x13),  // addi x7, x0, 3
        i_type(4, 0, 0x0, 28, 0x13), // addi x28, x0, 4
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-debug-in-flight-stage-cycles", &elf);

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
            "--riscv-in-order-width",
            "3",
            "--debug-flags",
            "Pipeline",
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
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"x6\":\"0x2\""));
    assert!(stdout.contains("\"x7\":\"0x3\""));
    assert!(stdout.contains("\"x28\":\"0x4\""));

    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let before = stage_in_flight_counts(trace, "before_in_flight");
    let after = stage_in_flight_counts(trace, "after_in_flight");
    assert!(
        before.values().any(|summary| summary.cycles > 0),
        "widened pipeline run should emit before-in-flight stage cycles: {trace:?}"
    );
    assert!(
        after.values().any(|summary| summary.cycles > 0),
        "widened pipeline run should emit after-in-flight stage cycles: {trace:?}"
    );
    assert!(
        before
            .values()
            .chain(after.values())
            .any(|summary| summary.count > summary.cycles),
        "stage in-flight instruction counts should stay distinct from per-record cycle counts: before={before:?} after={after:?}"
    );

    for (stage, summary) in before {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.before_in_flight"),
            "Count",
            summary.count,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.before_in_flight_cycles"),
            "Cycle",
            summary.cycles,
            "monotonic",
        );
    }
    for (stage, summary) in after {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.after_in_flight"),
            "Count",
            summary.count,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.after_in_flight_cycles"),
            "Cycle",
            summary.cycles,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_pipeline_debug_flag_attributes_branch_flush_stage_cycles() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // target: addi x7, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-debug-branch-flush-stage-cycles", &elf);

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
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "Pipeline",
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
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"x7\":\"0x7\""));
    assert!(
        !stdout.contains("\"x6\":\"0x9\""),
        "wrong-path instruction must be squashed: {stdout}"
    );

    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let flush_records = trace
        .iter()
        .filter(|record| {
            record.get("flush_cause").and_then(Value::as_str) == Some("branch_prediction")
        })
        .collect::<Vec<_>>();
    assert!(
        !flush_records.is_empty(),
        "branch run should emit branch_prediction flush records: {trace:?}"
    );

    let mut stage_flushed = BTreeMap::<String, u64>::new();
    let mut stage_flushed_cycles = BTreeMap::<String, u64>::new();
    for record in &flush_records {
        let mut stage_present = BTreeMap::<String, bool>::new();
        for flushed in record_array(record, "flushed") {
            let stage = flushed
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
            let stage = stat_path_segment(stage);
            *stage_flushed.entry(stage.clone()).or_default() += 1;
            stage_present.insert(stage, true);
        }
        for stage in stage_present.keys() {
            *stage_flushed_cycles.entry(stage.clone()).or_default() += 1;
        }
    }
    assert!(
        !stage_flushed_cycles.is_empty(),
        "branch flush records should preserve flushed stages: {flush_records:?}"
    );

    for (stage, flushed) in &stage_flushed {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.flush_cause.branch_prediction.stage.{stage}.flushed"
            ),
            "Count",
            *flushed,
            "monotonic",
        );
    }
    for (stage, cycles) in &stage_flushed_cycles {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.flushed_cycles"),
            "Cycle",
            *cycles,
            "monotonic",
        );
    }
    for (stage, cycles) in stage_flushed_cycles {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.flush_cause.branch_prediction.stage.{stage}.flushed_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
    }
    let trap_redirect_stage_flushed_cycles = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("stats array")
        .iter()
        .filter_map(|stat| stat.get("path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.debug.pipeline_trace.flush_cause.trap_redirect.stage.")
                && path.ends_with(".flushed_cycles")
        })
        .collect::<Vec<_>>();
    assert!(
        trap_redirect_stage_flushed_cycles.is_empty(),
        "branch-only run should not emit trap_redirect stage flushed-cycle stats: {trap_redirect_stage_flushed_cycles:?}"
    );
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StageInFlightSummary {
    count: u64,
    cycles: u64,
}

fn stage_in_flight_counts(trace: &[Value], field: &str) -> BTreeMap<String, StageInFlightSummary> {
    let mut summaries = BTreeMap::<String, StageInFlightSummary>::new();
    for record in trace {
        let mut stage_present = BTreeMap::<String, bool>::new();
        for instruction in record_array(record, field) {
            let stage = instruction
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing in-flight instruction stage: {instruction}"));
            let stage = stat_path_segment(stage);
            summaries.entry(stage.clone()).or_default().count += 1;
            stage_present.insert(stage, true);
        }
        for stage in stage_present.keys() {
            summaries.entry(stage.clone()).or_default().cycles += 1;
        }
    }
    summaries
}
