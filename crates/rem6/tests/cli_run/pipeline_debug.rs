use std::{collections::BTreeMap, process::Command};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_stats_emit_in_order_stall_cause_stage_matrix_without_debug_flag() {
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
    let path = temp_binary("pipeline-stall-cause-stage-matrix-stats", &elf);

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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));

    let data_wait_cycles = json_stat_value(&json, "sim.cpu0.pipeline.in_order.data_wait_cycles");
    assert!(data_wait_cycles > 0, "{stdout}");
    let data_wait_stage_blocked =
        in_order_stall_cause_stage_metric_values(&json, "data_wait", "resource_blocked");
    let data_wait_stage_cycles =
        in_order_stall_cause_stage_metric_values(&json, "data_wait", "resource_blocked_cycles");
    assert_eq!(
        data_wait_stage_cycles.iter().sum::<u64>(),
        data_wait_cycles,
        "data-wait stage-cycle matrix should account for all data wait cycles: {data_wait_stage_cycles:?}"
    );
    assert!(
        data_wait_stage_blocked.iter().any(|blocked| *blocked > 0),
        "data-wait run should attribute at least one blocked stage: {data_wait_stage_blocked:?}"
    );
    assert!(
        data_wait_stage_cycles.iter().any(|cycles| *cycles > 0),
        "data-wait run should attribute at least one blocked stage cycle: {data_wait_stage_cycles:?}"
    );

    let fetch_wait_cycles = json_stat_value(&json, "sim.cpu0.pipeline.in_order.fetch_wait_cycles");
    let fetch_wait_stage_blocked =
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "resource_blocked");
    let fetch_wait_stage_cycles =
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "resource_blocked_cycles");
    if fetch_wait_cycles == 0 {
        assert_eq!(fetch_wait_stage_blocked, [0; 5]);
        assert_eq!(fetch_wait_stage_cycles, [0; 5]);
    } else {
        assert!(
            fetch_wait_stage_blocked.iter().any(|blocked| *blocked > 0),
            "fetch-wait run should attribute blocked stages when aggregate wait is nonzero: {fetch_wait_stage_blocked:?}"
        );
        assert!(
            fetch_wait_stage_cycles.iter().any(|cycles| *cycles > 0),
            "fetch-wait run should attribute stage cycles when aggregate wait is nonzero: {fetch_wait_stage_cycles:?}"
        );
    }

    let execute_wait_cycles =
        json_stat_value(&json, "sim.cpu0.pipeline.in_order.execute_wait_cycles");
    assert_eq!(execute_wait_cycles, 0);
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "resource_blocked"),
        [0; 5]
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "resource_blocked_cycles"),
        [0; 5]
    );
}

#[test]
fn rem6_run_stats_emit_in_order_stage_movement_matrix_without_debug_flag() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13),  // addi x5, x0, 1
        i_type(2, 0, 0x0, 6, 0x13),  // addi x6, x0, 2
        i_type(3, 0, 0x0, 7, 0x13),  // addi x7, x0, 3
        i_type(4, 0, 0x0, 28, 0x13), // addi x28, x0, 4
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-stage-movement-matrix-stats", &elf);

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
            "--riscv-in-order-width",
            "3",
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
    assert!(
        json.pointer("/debug/pipeline_trace").is_none(),
        "normal stats evidence should not require Pipeline debug trace: {stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"x6\":\"0x2\""));
    assert!(stdout.contains("\"x7\":\"0x3\""));
    assert!(stdout.contains("\"x28\":\"0x4\""));
    assert_eq!(
        json_stat_value(
            &json,
            "sim.cpu0.pipeline.in_order.branch_prediction_redirects"
        ),
        0
    );
    assert_eq!(
        json_stat_value(
            &json,
            "sim.cpu0.pipeline.in_order.branch_prediction_flushes"
        ),
        0
    );

    let stage_advanced = in_order_stage_metric_values(&json, "advanced");
    let stage_advanced_cycles = in_order_stage_metric_values(&json, "advanced_cycles");
    let stage_retired = in_order_stage_metric_values(&json, "retired");
    let stage_retired_cycles = in_order_stage_metric_values(&json, "retired_cycles");
    let aggregate_advanced = json_stat_value(&json, "sim.cpu0.pipeline.in_order.advanced");
    let aggregate_retired = json_stat_value(&json, "sim.cpu0.pipeline.in_order.retired");

    assert_eq!(
        stage_advanced.iter().sum::<u64>(),
        aggregate_advanced,
        "stage advanced matrix must account for aggregate movement: {stage_advanced:?}"
    );
    assert_eq!(
        stage_retired.iter().sum::<u64>(),
        aggregate_retired,
        "stage retired matrix must account for aggregate retired movement: {stage_retired:?}"
    );
    assert!(
        stage_advanced_cycles.iter().any(|cycles| *cycles > 0),
        "stage advanced cycles should be visible in normal stats: {stage_advanced_cycles:?}"
    );
    assert!(
        stage_retired_cycles.iter().any(|cycles| *cycles > 0),
        "stage retired cycles should be visible in normal stats: {stage_retired_cycles:?}"
    );
    assert!(
        stage_advanced
            .iter()
            .zip(stage_advanced_cycles.iter())
            .any(|(advanced, cycles)| advanced > cycles),
        "widened pipeline should distinguish movement counts from cycle presence: advanced={stage_advanced:?} cycles={stage_advanced_cycles:?}"
    );
    assert!(
        stage_retired[4] > 0,
        "retirement movement should be attributed to commit stage: {stage_retired:?}"
    );
}

#[test]
fn rem6_run_stats_emit_in_order_flush_cause_stage_matrix_without_debug_flag() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // target: addi x7, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-flush-cause-stage-matrix-stats", &elf);

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
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"x7\":\"0x7\""));
    assert!(
        !stdout.contains("\"x6\":\"0x9\""),
        "wrong-path instruction must be squashed: {stdout}"
    );

    let branch_prediction_flushes = json_stat_value(
        &json,
        "sim.cpu0.pipeline.in_order.branch_prediction_flushes",
    );
    let branch_prediction_flush_cycles = json_stat_value(
        &json,
        "sim.cpu0.pipeline.in_order.branch_prediction_flush_cycles",
    );
    assert!(branch_prediction_flushes > 0, "{stdout}");
    assert!(branch_prediction_flush_cycles > 0, "{stdout}");

    let stage_branch_prediction_flushed =
        in_order_stage_metric_values(&json, "branch_prediction_flushed");
    let stage_branch_prediction_flushed_cycles =
        in_order_stage_metric_values(&json, "branch_prediction_flushed_cycles");
    let flush_cause_branch_prediction_flushed =
        in_order_flush_cause_stage_metric_values(&json, "branch_prediction", "flushed");
    let flush_cause_branch_prediction_flushed_cycles =
        in_order_flush_cause_stage_metric_values(&json, "branch_prediction", "flushed_cycles");
    assert_eq!(
        flush_cause_branch_prediction_flushed,
        stage_branch_prediction_flushed
    );
    assert_eq!(
        flush_cause_branch_prediction_flushed_cycles,
        stage_branch_prediction_flushed_cycles
    );
    assert_eq!(
        flush_cause_branch_prediction_flushed.iter().sum::<u64>(),
        branch_prediction_flushes
    );
    assert!(
        flush_cause_branch_prediction_flushed_cycles
            .iter()
            .any(|cycles| *cycles > 0),
        "branch run should attribute branch-prediction flushed cycles by stage: {flush_cause_branch_prediction_flushed_cycles:?}"
    );
    assert!(
        flush_cause_branch_prediction_flushed_cycles
            .iter()
            .sum::<u64>()
            >= branch_prediction_flush_cycles
    );
    assert!(
        flush_cause_branch_prediction_flushed_cycles
            .iter()
            .sum::<u64>()
            <= branch_prediction_flushes
    );

    assert_eq!(
        in_order_flush_cause_stage_metric_values(&json, "trap_redirect", "flushed"),
        [0; 5]
    );
    assert_eq!(
        in_order_flush_cause_stage_metric_values(&json, "trap_redirect", "flushed_cycles"),
        [0; 5]
    );
}

#[test]
fn rem6_run_stats_emit_in_order_redirect_cause_stage_matrix_without_debug_flag() {
    let branch_program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // target: addi x7, x0, 7
        0x0000_0073,                // ecall
    ]);
    let branch_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &branch_program);
    let branch_path = temp_binary(
        "pipeline-redirect-cause-branch-stage-matrix-stats",
        &branch_elf,
    );

    let branch_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            branch_path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        branch_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&branch_output.stderr)
    );
    let branch_stdout = String::from_utf8(branch_output.stdout).unwrap();
    let branch_json: Value = serde_json::from_str(&branch_stdout).unwrap();
    assert!(branch_stdout.contains("\"x5\":\"0x1\""));
    assert!(branch_stdout.contains("\"x7\":\"0x7\""));
    assert!(
        !branch_stdout.contains("\"x6\":\"0x9\""),
        "wrong-path instruction must be squashed: {branch_stdout}"
    );
    let branch_redirects = json_stat_value(
        &branch_json,
        "sim.cpu0.pipeline.in_order.branch_prediction_redirects",
    );
    assert!(branch_redirects > 0, "{branch_stdout}");

    let branch_stage_flushed =
        in_order_stage_metric_values(&branch_json, "branch_prediction_flushed");
    let branch_stage_flushed_cycles =
        in_order_stage_metric_values(&branch_json, "branch_prediction_flushed_cycles");
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&branch_json, "branch_prediction", "flushed"),
        branch_stage_flushed
    );
    let branch_redirect_cause_cycles = in_order_redirect_cause_stage_metric_values(
        &branch_json,
        "branch_prediction",
        "flushed_cycles",
    );
    assert_eq!(branch_redirect_cause_cycles, branch_stage_flushed_cycles);
    assert!(
        branch_redirect_cause_cycles
            .iter()
            .any(|cycles| *cycles > 0),
        "branch run should attribute redirect-cause flushed cycles by stage: {branch_redirect_cause_cycles:?}"
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&branch_json, "trap_redirect", "flushed"),
        [0; 5]
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(
            &branch_json,
            "trap_redirect",
            "flushed_cycles"
        ),
        [0; 5]
    );

    let trap_program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        i_type(9, 0, 0x0, 6, 0x13), // wrong-path addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let trap_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &trap_program);
    let trap_path = temp_binary("pipeline-redirect-cause-trap-stage-matrix-stats", &trap_elf);

    let trap_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            trap_path.to_str().unwrap(),
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
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        trap_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&trap_output.stderr)
    );
    let trap_stdout = String::from_utf8(trap_output.stdout).unwrap();
    let trap_json: Value = serde_json::from_str(&trap_stdout).unwrap();
    assert_eq!(
        trap_json
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert!(
        !trap_stdout.contains("\"x6\":\"0x9\""),
        "wrong-path instruction must be squashed: {trap_stdout}"
    );
    assert_eq!(
        json_stat_value(
            &trap_json,
            "sim.cpu0.pipeline.in_order.branch_prediction_redirects"
        ),
        0
    );
    assert_eq!(
        json_stat_value(&trap_json, "sim.cpu0.pipeline.in_order.trap_redirects"),
        1
    );

    let trap_stage_flushed = in_order_stage_metric_values(&trap_json, "trap_redirect_flushed");
    let trap_stage_flushed_cycles =
        in_order_stage_metric_values(&trap_json, "trap_redirect_flushed_cycles");
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&trap_json, "trap_redirect", "flushed"),
        trap_stage_flushed
    );
    let trap_redirect_cause_cycles =
        in_order_redirect_cause_stage_metric_values(&trap_json, "trap_redirect", "flushed_cycles");
    assert_eq!(trap_redirect_cause_cycles, trap_stage_flushed_cycles);
    assert!(
        trap_redirect_cause_cycles
            .iter()
            .any(|cycles| *cycles > 0),
        "trap run should attribute redirect-cause flushed cycles by stage: {trap_redirect_cause_cycles:?}"
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&trap_json, "branch_prediction", "flushed"),
        [0; 5]
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(
            &trap_json,
            "branch_prediction",
            "flushed_cycles"
        ),
        [0; 5]
    );
}

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
            &format!("sim.debug.pipeline_trace.stage.{stage}.trap_redirect_flushed"),
            "Count",
            *flushed,
            "monotonic",
        );
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
            &format!("sim.debug.pipeline_trace.stage.{stage}.trap_redirect_flushed_cycles"),
            "Cycle",
            cycles,
            "monotonic",
        );
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
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.stage.commit.branch_prediction_flushed_cycles",
        "Cycle",
        0,
        "monotonic",
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
fn rem6_run_pipeline_debug_flag_attributes_advance_retire_stage_cycles() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13),  // addi x5, x0, 1
        i_type(2, 0, 0x0, 6, 0x13),  // addi x6, x0, 2
        i_type(3, 0, 0x0, 7, 0x13),  // addi x7, x0, 3
        i_type(4, 0, 0x0, 28, 0x13), // addi x28, x0, 4
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-debug-advance-retire-stage-cycles", &elf);

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
    let movement = stage_movement_counts(trace);
    assert!(
        movement.values().any(|summary| summary.advanced_cycles > 0),
        "widened pipeline run should emit advanced stage cycles: {trace:?}"
    );
    assert!(
        movement.values().any(|summary| summary.retired_cycles > 0),
        "widened pipeline run should emit retired stage cycles: {trace:?}"
    );
    assert!(
        movement
            .values()
            .any(|summary| summary.advanced > summary.advanced_cycles),
        "stage advancement counts should stay distinct from per-record cycle counts: {movement:?}"
    );

    for (stage, summary) in movement {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.advanced"),
            "Count",
            summary.advanced,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.advanced_cycles"),
            "Cycle",
            summary.advanced_cycles,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.retired"),
            "Count",
            summary.retired,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.retired_cycles"),
            "Cycle",
            summary.retired_cycles,
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
            &format!("sim.debug.pipeline_trace.stage.{stage}.branch_prediction_flushed"),
            "Count",
            *flushed,
            "monotonic",
        );
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
            &format!("sim.debug.pipeline_trace.stage.{stage}.branch_prediction_flushed_cycles"),
            "Cycle",
            *cycles,
            "monotonic",
        );
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
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.stage.commit.trap_redirect_flushed_cycles",
        "Cycle",
        0,
        "monotonic",
    );
}

fn json_record_u64(record: &Value, field: &str) -> u64 {
    record
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric field {field} in record {record}"))
}

fn json_record_bool(record: &Value, field: &str) -> bool {
    record
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("missing bool field {field} in record {record}"))
}

fn json_stat_value(json: &Value, path: &str) -> u64 {
    json.pointer("/stats")
        .and_then(Value::as_array)
        .expect("stats array")
        .iter()
        .find(|stat| stat.get("path").and_then(Value::as_str) == Some(path))
        .unwrap_or_else(|| panic!("missing stat path {path}"))
        .get("value")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric value for stat path {path}"))
}

fn in_order_stall_cause_stage_metric_values(json: &Value, cause: &str, metric: &str) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        json_stat_value(
            json,
            &format!("sim.cpu0.pipeline.in_order.stall_cause.{cause}.stage.{stage}.{metric}"),
        )
    })
}

fn in_order_flush_cause_stage_metric_values(json: &Value, cause: &str, metric: &str) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        json_stat_value(
            json,
            &format!("sim.cpu0.pipeline.in_order.flush_cause.{cause}.stage.{stage}.{metric}"),
        )
    })
}

fn in_order_redirect_cause_stage_metric_values(
    json: &Value,
    cause: &str,
    metric: &str,
) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        json_stat_value(
            json,
            &format!("sim.cpu0.pipeline.in_order.redirect_cause.{cause}.stage.{stage}.{metric}"),
        )
    })
}

fn in_order_stage_metric_values(json: &Value, metric: &str) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        json_stat_value(
            json,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.{metric}"),
        )
    })
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StageMovementSummary {
    advanced: u64,
    advanced_cycles: u64,
    retired: u64,
    retired_cycles: u64,
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

fn stage_movement_counts(trace: &[Value]) -> BTreeMap<String, StageMovementSummary> {
    let mut summaries = BTreeMap::<String, StageMovementSummary>::new();
    for record in trace {
        let mut advanced_stage_present = BTreeMap::<String, bool>::new();
        let mut retired_stage_present = BTreeMap::<String, bool>::new();
        for advanced in record_array(record, "advanced") {
            let stage = advanced
                .get("source_stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing advanced instruction source stage: {advanced}"));
            let stage = stat_path_segment(stage);
            summaries.entry(stage.clone()).or_default().advanced += 1;
            advanced_stage_present.insert(stage.clone(), true);
            if json_record_bool(advanced, "retires") {
                summaries.entry(stage.clone()).or_default().retired += 1;
                retired_stage_present.insert(stage, true);
            }
        }
        for stage in advanced_stage_present.keys() {
            summaries.entry(stage.clone()).or_default().advanced_cycles += 1;
        }
        for stage in retired_stage_present.keys() {
            summaries.entry(stage.clone()).or_default().retired_cycles += 1;
        }
    }
    summaries
}
