use std::{
    collections::{BTreeMap, BTreeSet},
    process::Command,
};

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
    assert_json_stat_ids_are_unique(&json);
    assert_in_order_pipeline_aliases(&json);
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
    let data_wait_stage_records =
        in_order_stall_cause_stage_metric_values(&json, "data_wait", "records");
    let data_wait_records = in_order_stall_cause_metric_value(&json, "data_wait", "records");
    assert!(data_wait_records > 0, "{stdout}");
    assert_eq!(
        in_order_artifact_stall_cause_metric_value(&json, "data_wait", "records"),
        data_wait_records
    );
    assert_in_order_stall_cause_alias(&json, "data_wait", "dataWait", "records", "records");
    assert_in_order_stall_cause_stage_aliases(&json, "data_wait", "dataWait", "records", "records");
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "data_wait",
        "dataWait",
        "resource_blocked",
        "resourceBlocked",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "data_wait",
        "dataWait",
        "resource_blocked_cycles",
        "resourceBlockedCycles",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "data_wait",
        "dataWait",
        "ordering_blocked",
        "orderingBlocked",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "data_wait",
        "dataWait",
        "ordering_blocked_cycles",
        "orderingBlockedCycles",
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(&json, "data_wait", "resource_blocked"),
        data_wait_stage_blocked
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "data_wait",
            "resource_blocked_cycles"
        ),
        data_wait_stage_cycles
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(&json, "data_wait", "records"),
        data_wait_stage_records
    );
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
    assert!(
        data_wait_stage_records.iter().any(|records| *records > 0),
        "data-wait run should attribute active stage records: {data_wait_stage_records:?}"
    );
    assert!(
        data_wait_records <= data_wait_stage_records.iter().sum::<u64>(),
        "data-wait aggregate records should not exceed stage records: records={data_wait_records} stage_records={data_wait_stage_records:?}"
    );
    assert!(
        data_wait_stage_records
            .iter()
            .zip(data_wait_stage_cycles.iter())
            .all(|(records, cycles)| records <= cycles),
        "data-wait record lanes should not exceed stalled cycle lanes: records={data_wait_stage_records:?} cycles={data_wait_stage_cycles:?}"
    );

    let fetch_wait_cycles = json_stat_value(&json, "sim.cpu0.pipeline.in_order.fetch_wait_cycles");
    let fetch_wait_stage_blocked =
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "resource_blocked");
    let fetch_wait_stage_cycles =
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "resource_blocked_cycles");
    let fetch_wait_stage_records =
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "records");
    let fetch_wait_records = in_order_stall_cause_metric_value(&json, "fetch_wait", "records");
    assert_eq!(
        in_order_artifact_stall_cause_metric_value(&json, "fetch_wait", "records"),
        fetch_wait_records
    );
    assert_in_order_stall_cause_alias(&json, "fetch_wait", "fetchWait", "records", "records");
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "fetch_wait",
        "fetchWait",
        "records",
        "records",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "fetch_wait",
        "fetchWait",
        "resource_blocked",
        "resourceBlocked",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "fetch_wait",
        "fetchWait",
        "resource_blocked_cycles",
        "resourceBlockedCycles",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "fetch_wait",
        "fetchWait",
        "ordering_blocked",
        "orderingBlocked",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "fetch_wait",
        "fetchWait",
        "ordering_blocked_cycles",
        "orderingBlockedCycles",
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(&json, "fetch_wait", "resource_blocked"),
        fetch_wait_stage_blocked
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "fetch_wait",
            "resource_blocked_cycles"
        ),
        fetch_wait_stage_cycles
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(&json, "fetch_wait", "records"),
        fetch_wait_stage_records
    );
    if fetch_wait_cycles == 0 {
        assert_eq!(fetch_wait_records, 0);
        assert_eq!(fetch_wait_stage_blocked, [0; 5]);
        assert_eq!(fetch_wait_stage_cycles, [0; 5]);
        assert_eq!(fetch_wait_stage_records, [0; 5]);
    } else {
        assert!(fetch_wait_records > 0);
        assert!(
            fetch_wait_stage_blocked.iter().any(|blocked| *blocked > 0),
            "fetch-wait run should attribute blocked stages when aggregate wait is nonzero: {fetch_wait_stage_blocked:?}"
        );
        assert!(
            fetch_wait_stage_cycles.iter().any(|cycles| *cycles > 0),
            "fetch-wait run should attribute stage cycles when aggregate wait is nonzero: {fetch_wait_stage_cycles:?}"
        );
        assert!(
            fetch_wait_stage_records.iter().any(|records| *records > 0),
            "fetch-wait run should attribute active stage records when aggregate wait is nonzero: {fetch_wait_stage_records:?}"
        );
    }

    let execute_wait_cycles =
        json_stat_value(&json, "sim.cpu0.pipeline.in_order.execute_wait_cycles");
    assert_eq!(execute_wait_cycles, 0);
    assert_eq!(
        in_order_stall_cause_metric_value(&json, "execute_wait", "records"),
        0
    );
    assert_eq!(
        in_order_artifact_stall_cause_metric_value(&json, "execute_wait", "records"),
        0
    );
    assert_in_order_stall_cause_alias(&json, "execute_wait", "executeWait", "records", "records");
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "resource_blocked"),
        [0; 5]
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "execute_wait",
        "executeWait",
        "resource_blocked",
        "resourceBlocked",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "execute_wait",
        "executeWait",
        "resource_blocked_cycles",
        "resourceBlockedCycles",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "execute_wait",
        "executeWait",
        "ordering_blocked",
        "orderingBlocked",
    );
    assert_in_order_stall_cause_stage_aliases(
        &json,
        "execute_wait",
        "executeWait",
        "ordering_blocked_cycles",
        "orderingBlockedCycles",
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "execute_wait",
            "resource_blocked"
        ),
        [0; 5]
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "resource_blocked_cycles"),
        [0; 5]
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "execute_wait",
            "resource_blocked_cycles"
        ),
        [0; 5]
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "records"),
        [0; 5]
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(&json, "execute_wait", "records"),
        [0; 5]
    );
}

#[test]
fn rem6_run_stats_emit_multicore_in_order_pipeline_aliases_after_existing_stage_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("multicore-pipeline-alias-id-order", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_json_stat_ids_are_unique(&json);
    for cpu in [0, 1] {
        assert_in_order_pipeline_aliases_for_cpu(
            &json,
            cpu,
            &format!("system.cpu{cpu}.pipeline.inOrder"),
        );
        assert_in_order_pipeline_stage_aliases_for_cpu(
            &json,
            cpu,
            &format!("system.cpu{cpu}.pipeline.inOrder"),
        );
    }
    assert_json_stat_id_after(
        &json,
        "system.cpu0.pipeline.inOrder.advanced",
        "system.cpu1.pipeline.inOrder.stallCause.executeWait.stage.commit.orderingBlockedCycles",
    );
    assert_json_stat_id_after(
        &json,
        "system.cpu0.pipeline.inOrder.stage.fetch1.occupiedCycles",
        "system.cpu1.pipeline.inOrder.branchSpeculationMaxPending",
    );
}

#[test]
fn rem6_run_pipeline_debug_stats_emit_multicore_cpu_scoped_stall_cause_matrices() {
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
    let path = temp_binary("pipeline-debug-multicore-cpu-stall-cause-matrix", &elf);

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
            "--cores",
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
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert_pipeline_summary_matches_trace(&json);

    let mut records_by_cpu = BTreeMap::<u64, u64>::new();
    let mut cycles_by_cpu_stage = BTreeMap::<(u64, String), u64>::new();
    let mut stage_resource_cycles_by_cpu_stage = BTreeMap::<(u64, String), u64>::new();
    let mut cycles_by_stage = BTreeMap::<String, u64>::new();
    for record in trace {
        let cpu = record
            .get("cpu")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing pipeline trace CPU: {record}"));
        let mut stage_present = BTreeSet::<String>::new();
        for blocked in record_array(record, "resource_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing blocked instruction stage: {blocked}"));
            stage_present.insert(stat_path_segment(stage));
        }
        for stage in &stage_present {
            *stage_resource_cycles_by_cpu_stage
                .entry((cpu, stage.clone()))
                .or_default() += 1;
        }
        if record.get("stall_cause").and_then(Value::as_str) != Some("data_wait") {
            continue;
        }
        *records_by_cpu.entry(cpu).or_default() += 1;
        let stall_cycles = json_record_u64(record, "stall_cycles");
        for stage in stage_present {
            *cycles_by_cpu_stage.entry((cpu, stage.clone())).or_default() += stall_cycles;
            *cycles_by_stage.entry(stage).or_default() += stall_cycles;
        }
    }

    for cpu in [0, 1] {
        let records = records_by_cpu
            .get(&cpu)
            .copied()
            .unwrap_or_else(|| panic!("missing CPU{cpu} data_wait records: {trace:?}"));
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.cpu.cpu{cpu}.stall_cause.data_wait.records"),
            "Count",
            records,
            "monotonic",
        );
        assert!(
            cycles_by_cpu_stage
                .keys()
                .any(|(sample_cpu, _)| *sample_cpu == cpu),
            "CPU{cpu} should expose data_wait stage-cycle debug stats: {cycles_by_cpu_stage:?}"
        );
    }

    for ((cpu, stage), cycles) in cycles_by_cpu_stage {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.cpu.cpu{cpu}.stall_cause.data_wait.stage.{stage}.resource_blocked_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.cpu.cpu{cpu}.stage.{stage}.resource_blocked_cycles"),
            "Cycle",
            *stage_resource_cycles_by_cpu_stage
                .get(&(cpu, stage.clone()))
                .unwrap_or_else(|| {
                    panic!(
                        "missing CPU{cpu} generic stage resource cycles for {stage}: {stage_resource_cycles_by_cpu_stage:?}"
                    )
                }),
            "monotonic",
        );
    }
    for (stage, cycles) in cycles_by_stage {
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
}

#[test]
fn rem6_run_pipeline_debug_stats_emit_multicore_cpu_scoped_flush_redirect_matrices() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // target: addi x7, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-debug-multicore-cpu-flush-redirect-matrix", &elf);

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
            "--cores",
            "2",
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
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert_pipeline_summary_matches_trace(&json);

    let mut flush_records_by_cpu = BTreeMap::<u64, u64>::new();
    let mut redirect_records_by_cpu = BTreeMap::<u64, u64>::new();
    let mut flush_cycles_by_cpu_stage = BTreeMap::<(u64, String), u64>::new();
    let mut redirect_cycles_by_cpu_stage = BTreeMap::<(u64, String), u64>::new();
    let mut stage_branch_cycles_by_cpu_stage = BTreeMap::<(u64, String), u64>::new();
    let mut flush_cycles_by_stage = BTreeMap::<String, u64>::new();
    let mut redirect_cycles_by_stage = BTreeMap::<String, u64>::new();
    for record in trace {
        let cpu = record
            .get("cpu")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing pipeline trace CPU: {record}"));
        let mut stage_present = BTreeSet::<String>::new();
        for flushed in record_array(record, "flushed") {
            let stage = flushed
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
            stage_present.insert(stat_path_segment(stage));
        }
        if record.get("flush_cause").and_then(Value::as_str) == Some("branch_prediction") {
            *flush_records_by_cpu.entry(cpu).or_default() += 1;
            for stage in &stage_present {
                *flush_cycles_by_cpu_stage
                    .entry((cpu, stage.clone()))
                    .or_default() += 1;
                *stage_branch_cycles_by_cpu_stage
                    .entry((cpu, stage.clone()))
                    .or_default() += 1;
                *flush_cycles_by_stage.entry(stage.clone()).or_default() += 1;
            }
        }
        if record.get("redirect_cause").and_then(Value::as_str) == Some("branch_prediction") {
            *redirect_records_by_cpu.entry(cpu).or_default() += 1;
            for stage in stage_present {
                *redirect_cycles_by_cpu_stage
                    .entry((cpu, stage.clone()))
                    .or_default() += 1;
                *redirect_cycles_by_stage.entry(stage).or_default() += 1;
            }
        }
    }

    for cpu in [0, 1] {
        let flush_records = flush_records_by_cpu
            .get(&cpu)
            .copied()
            .unwrap_or_else(|| panic!("missing CPU{cpu} branch flush records: {trace:?}"));
        let redirect_records = redirect_records_by_cpu
            .get(&cpu)
            .copied()
            .unwrap_or_else(|| panic!("missing CPU{cpu} branch redirect records: {trace:?}"));
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.cpu.cpu{cpu}.flush_cause.branch_prediction.records"),
            "Count",
            flush_records,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.cpu.cpu{cpu}.redirect_cause.branch_prediction.records"
            ),
            "Count",
            redirect_records,
            "monotonic",
        );
    }

    for ((cpu, stage), cycles) in flush_cycles_by_cpu_stage {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.cpu.cpu{cpu}.flush_cause.branch_prediction.stage.{stage}.flushed_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.cpu.cpu{cpu}.stage.{stage}.branch_prediction_flushed_cycles"
            ),
            "Cycle",
            *stage_branch_cycles_by_cpu_stage
                .get(&(cpu, stage.clone()))
                .unwrap_or_else(|| {
                    panic!(
                        "missing CPU{cpu} generic branch-flush stage cycles for {stage}: {stage_branch_cycles_by_cpu_stage:?}"
                    )
                }),
            "monotonic",
        );
    }
    for ((cpu, stage), cycles) in redirect_cycles_by_cpu_stage {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.cpu.cpu{cpu}.redirect_cause.branch_prediction.stage.{stage}.flushed_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
    }
    for (stage, cycles) in flush_cycles_by_stage {
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
    for (stage, cycles) in redirect_cycles_by_stage {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.redirect_cause.branch_prediction.stage.{stage}.flushed_cycles"
            ),
            "Cycle",
            cycles,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_stats_emit_in_order_execute_wait_ordering_stage_matrix_without_debug_flag() {
    let program = riscv64_program(&[
        i_type(97, 0, 0x0, 11, 0x13),        // addi x11, x0, 97
        i_type(3, 0, 0x0, 12, 0x13),         // addi x12, x0, 3
        r_type(0x01, 12, 11, 0x4, 10, 0x33), // div x10, x11, x12
        i_type(1, 0, 0x0, 8, 0x13),          // addi x8, x0, 1
        i_type(2, 0, 0x0, 9, 0x13),          // addi x9, x0, 2
        0x0000_0073,                         // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-execute-wait-ordering-stage-matrix-stats", &elf);

    let json_stdout = run_execute_wait_ordering_program(&path, "json");
    let json: Value = serde_json::from_str(&json_stdout).unwrap();
    assert!(
        json.pointer("/debug/pipeline_trace").is_none(),
        "normal stats evidence should not require Pipeline debug trace: {json_stdout}"
    );
    assert!(json_stdout.contains("\"x10\":\"0x20\""));
    assert!(json_stdout.contains("\"x8\":\"0x1\""));
    assert!(json_stdout.contains("\"x9\":\"0x2\""));
    assert_eq!(
        json_stat_value(
            &json,
            "sim.cpu0.pipeline.in_order.branch_prediction_redirects"
        ),
        0
    );
    let debug_stdout =
        run_execute_wait_ordering_program_with_debug(&path, "json", Some("Pipeline"));
    let debug_json: Value = serde_json::from_str(&debug_stdout).unwrap();
    assert_pipeline_summary_matches_trace(&debug_json);
    let execute_wait_summary_ordering_cycles = ["fetch1", "fetch2", "decode", "execute", "commit"]
        .map(|stage| {
            pipeline_summary_path_u64(
                &debug_json,
                &format!(
                    "/debug/pipeline_summary/stall_cause/execute_wait/stage/{stage}/ordering_blocked_cycles"
                ),
            )
        });
    assert!(
        execute_wait_summary_ordering_cycles
            .iter()
            .any(|cycles| *cycles > 0),
        "Pipeline debug summary should preserve execute_wait ordering-blocked stage cycles: {debug_stdout}"
    );

    let execute_wait_cycles =
        json_stat_value(&json, "sim.cpu0.pipeline.in_order.execute_wait_cycles");
    assert!(execute_wait_cycles > 0, "{json_stdout}");
    let execute_wait_resource =
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "resource_blocked");
    let execute_wait_resource_cycles =
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "resource_blocked_cycles");
    let execute_wait_ordering =
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "ordering_blocked");
    let execute_wait_ordering_cycles =
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "ordering_blocked_cycles");
    let execute_wait_records =
        in_order_stall_cause_stage_metric_values(&json, "execute_wait", "records");
    let execute_wait_record_count =
        in_order_stall_cause_metric_value(&json, "execute_wait", "records");
    assert!(execute_wait_record_count > 0, "{json_stdout}");
    assert_eq!(
        in_order_artifact_stall_cause_metric_value(&json, "execute_wait", "records"),
        execute_wait_record_count
    );
    assert_in_order_stall_cause_alias(&json, "execute_wait", "executeWait", "records", "records");
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "execute_wait",
            "resource_blocked"
        ),
        execute_wait_resource
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "execute_wait",
            "resource_blocked_cycles"
        ),
        execute_wait_resource_cycles
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "execute_wait",
            "ordering_blocked"
        ),
        execute_wait_ordering
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(
            &json,
            "execute_wait",
            "ordering_blocked_cycles"
        ),
        execute_wait_ordering_cycles
    );
    assert_eq!(
        in_order_artifact_stall_cause_stage_metric_values(&json, "execute_wait", "records"),
        execute_wait_records
    );
    assert!(
        execute_wait_resource_cycles.iter().any(|cycles| *cycles > 0),
        "execute-wait run should attribute resource-blocked stage cycles: {execute_wait_resource_cycles:?}"
    );
    assert!(
        execute_wait_ordering.iter().any(|blocked| *blocked > 0),
        "execute-wait run should attribute younger ordering-blocked instructions: {execute_wait_ordering:?}"
    );
    assert!(
        execute_wait_ordering_cycles.iter().any(|cycles| *cycles > 0),
        "execute-wait run should attribute younger ordering-blocked stage cycles: {execute_wait_ordering_cycles:?}"
    );
    assert!(
        execute_wait_records.iter().any(|records| *records > 0),
        "execute-wait run should attribute active stage records: {execute_wait_records:?}"
    );
    assert!(
        execute_wait_record_count <= execute_wait_records.iter().sum::<u64>(),
        "execute-wait aggregate records should not exceed stage records: records={execute_wait_record_count} stage_records={execute_wait_records:?}"
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "data_wait", "ordering_blocked"),
        [0; 5]
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "data_wait", "ordering_blocked_cycles"),
        [0; 5]
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "ordering_blocked"),
        [0; 5]
    );
    assert_eq!(
        in_order_stall_cause_stage_metric_values(&json, "fetch_wait", "ordering_blocked_cycles"),
        [0; 5]
    );

    let text_stdout = run_execute_wait_ordering_program(&path, "text");
    for (stage, ordering_blocked) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .into_iter()
        .zip(execute_wait_ordering)
    {
        assert_eq!(
            text_stat_value(
                &text_stdout,
                &format!(
                    "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.{stage}.orderingBlocked"
                ),
            ),
            ordering_blocked
        );
    }
    for (stage, ordering_blocked_cycles) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .into_iter()
        .zip(execute_wait_ordering_cycles)
    {
        assert_eq!(
            text_stat_value(
                &text_stdout,
                &format!(
                    "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.{stage}.orderingBlockedCycles"
                ),
            ),
            ordering_blocked_cycles
        );
    }
    for (stage, records) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .into_iter()
        .zip(execute_wait_records)
    {
        assert_eq!(
            text_stat_value(
                &text_stdout,
                &format!(
                    "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.{stage}.records"
                ),
            ),
            records
        );
    }
    assert_eq!(
        text_stat_value(
            &text_stdout,
            "system.cpu.pipeline.inOrder.stallCause.executeWait.records"
        ),
        execute_wait_record_count
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
    assert_in_order_pipeline_stage_aliases(&json);
    assert_eq!(
        in_order_artifact_stage_metric_values(&json, "advanced"),
        stage_advanced
    );
    assert_eq!(
        in_order_artifact_stage_metric_values(&json, "advanced_cycles"),
        stage_advanced_cycles
    );
    assert_eq!(
        in_order_artifact_stage_metric_values(&json, "retired"),
        stage_retired
    );
    assert_eq!(
        in_order_artifact_stage_metric_values(&json, "retired_cycles"),
        stage_retired_cycles
    );
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
    let flush_cause_branch_prediction_records =
        in_order_flush_cause_stage_metric_values(&json, "branch_prediction", "records");
    let flush_cause_branch_prediction_record_count =
        in_order_cause_metric_value(&json, "flush_cause", "branch_prediction", "records");
    assert!(flush_cause_branch_prediction_record_count > 0, "{stdout}");
    assert_eq!(
        in_order_artifact_cause_metric_value(&json, "flush_cause", "branch_prediction", "records"),
        flush_cause_branch_prediction_record_count
    );
    assert_in_order_cause_alias(
        &json,
        "flush_cause",
        "flushCause",
        "branch_prediction",
        "branchPrediction",
        "records",
        "records",
    );
    assert_in_order_cause_stage_alias_family(&json, "flush_cause", "flushCause");
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &json,
            "flush_cause",
            "branch_prediction",
            "flushed"
        ),
        flush_cause_branch_prediction_flushed
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &json,
            "flush_cause",
            "branch_prediction",
            "flushed_cycles"
        ),
        flush_cause_branch_prediction_flushed_cycles
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &json,
            "flush_cause",
            "branch_prediction",
            "records"
        ),
        flush_cause_branch_prediction_records
    );
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
    assert!(
        flush_cause_branch_prediction_records
            .iter()
            .any(|records| *records > 0),
        "branch run should attribute branch-prediction active records by stage: {flush_cause_branch_prediction_records:?}"
    );
    assert!(
        flush_cause_branch_prediction_record_count
            <= flush_cause_branch_prediction_records.iter().sum::<u64>(),
        "branch aggregate flush-cause records should not exceed stage records: records={flush_cause_branch_prediction_record_count} stage_records={flush_cause_branch_prediction_records:?}"
    );
    assert!(
        flush_cause_branch_prediction_records
            .iter()
            .zip(flush_cause_branch_prediction_flushed_cycles.iter())
            .all(|(records, cycles)| records <= cycles),
        "branch run record lanes should not exceed flushed cycle lanes: records={flush_cause_branch_prediction_records:?} cycles={flush_cause_branch_prediction_flushed_cycles:?}"
    );

    assert_eq!(
        in_order_flush_cause_stage_metric_values(&json, "trap_redirect", "flushed"),
        [0; 5]
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &json,
            "flush_cause",
            "trap_redirect",
            "flushed"
        ),
        [0; 5]
    );
    assert_eq!(
        in_order_flush_cause_stage_metric_values(&json, "trap_redirect", "flushed_cycles"),
        [0; 5]
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &json,
            "flush_cause",
            "trap_redirect",
            "flushed_cycles"
        ),
        [0; 5]
    );
    assert_eq!(
        in_order_flush_cause_stage_metric_values(&json, "trap_redirect", "records"),
        [0; 5]
    );
    assert_eq!(
        in_order_cause_metric_value(&json, "flush_cause", "trap_redirect", "records"),
        0
    );
    assert_eq!(
        in_order_artifact_cause_metric_value(&json, "flush_cause", "trap_redirect", "records"),
        0
    );
    assert_in_order_cause_alias(
        &json,
        "flush_cause",
        "flushCause",
        "trap_redirect",
        "trapRedirect",
        "records",
        "records",
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &json,
            "flush_cause",
            "trap_redirect",
            "records"
        ),
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
    assert_in_order_cause_stage_alias_family(&branch_json, "redirect_cause", "redirectCause");
    let branch_redirect_cause_flushed =
        in_order_redirect_cause_stage_metric_values(&branch_json, "branch_prediction", "flushed");
    let branch_redirect_cause_records =
        in_order_redirect_cause_stage_metric_values(&branch_json, "branch_prediction", "records");
    let branch_redirect_cause_record_count = in_order_cause_metric_value(
        &branch_json,
        "redirect_cause",
        "branch_prediction",
        "records",
    );
    assert_eq!(branch_redirect_cause_record_count, branch_redirects);
    assert_eq!(
        in_order_artifact_cause_metric_value(
            &branch_json,
            "redirect_cause",
            "branch_prediction",
            "records",
        ),
        branch_redirect_cause_record_count
    );
    assert_in_order_cause_alias(
        &branch_json,
        "redirect_cause",
        "redirectCause",
        "branch_prediction",
        "branchPrediction",
        "records",
        "records",
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &branch_json,
            "redirect_cause",
            "branch_prediction",
            "flushed"
        ),
        branch_redirect_cause_flushed
    );
    assert_eq!(branch_redirect_cause_flushed, branch_stage_flushed);
    let branch_redirect_cause_cycles = in_order_redirect_cause_stage_metric_values(
        &branch_json,
        "branch_prediction",
        "flushed_cycles",
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &branch_json,
            "redirect_cause",
            "branch_prediction",
            "flushed_cycles"
        ),
        branch_redirect_cause_cycles
    );
    assert_eq!(branch_redirect_cause_cycles, branch_stage_flushed_cycles);
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &branch_json,
            "redirect_cause",
            "branch_prediction",
            "records"
        ),
        branch_redirect_cause_records
    );
    assert!(
        branch_redirect_cause_cycles
            .iter()
            .any(|cycles| *cycles > 0),
        "branch run should attribute redirect-cause flushed cycles by stage: {branch_redirect_cause_cycles:?}"
    );
    assert!(
        branch_redirect_cause_records
            .iter()
            .any(|records| *records > 0),
        "branch run should attribute redirect-cause active records by stage: {branch_redirect_cause_records:?}"
    );
    assert!(
        branch_redirect_cause_record_count <= branch_redirect_cause_records.iter().sum::<u64>(),
        "branch aggregate redirect-cause records should not exceed stage records: records={branch_redirect_cause_record_count} stage_records={branch_redirect_cause_records:?}"
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&branch_json, "trap_redirect", "flushed"),
        [0; 5]
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &branch_json,
            "redirect_cause",
            "trap_redirect",
            "flushed"
        ),
        [0; 5]
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&branch_json, "trap_redirect", "records"),
        [0; 5]
    );
    let branch_trap_redirect_record_count =
        in_order_cause_metric_value(&branch_json, "redirect_cause", "trap_redirect", "records");
    assert_eq!(
        branch_trap_redirect_record_count,
        json_stat_value(&branch_json, "sim.cpu0.pipeline.in_order.trap_redirects")
    );
    assert_eq!(
        in_order_artifact_cause_metric_value(
            &branch_json,
            "redirect_cause",
            "trap_redirect",
            "records",
        ),
        branch_trap_redirect_record_count
    );
    assert_in_order_cause_alias(
        &branch_json,
        "redirect_cause",
        "redirectCause",
        "trap_redirect",
        "trapRedirect",
        "records",
        "records",
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &branch_json,
            "redirect_cause",
            "trap_redirect",
            "records"
        ),
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
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &branch_json,
            "redirect_cause",
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
    assert_in_order_cause_stage_alias_family(&trap_json, "redirect_cause", "redirectCause");
    let trap_redirect_cause_flushed =
        in_order_redirect_cause_stage_metric_values(&trap_json, "trap_redirect", "flushed");
    let trap_redirect_cause_records =
        in_order_redirect_cause_stage_metric_values(&trap_json, "trap_redirect", "records");
    let trap_redirect_cause_record_count =
        in_order_cause_metric_value(&trap_json, "redirect_cause", "trap_redirect", "records");
    assert_eq!(trap_redirect_cause_record_count, 1);
    assert_eq!(
        in_order_artifact_cause_metric_value(
            &trap_json,
            "redirect_cause",
            "trap_redirect",
            "records",
        ),
        trap_redirect_cause_record_count
    );
    assert_in_order_cause_alias(
        &trap_json,
        "redirect_cause",
        "redirectCause",
        "trap_redirect",
        "trapRedirect",
        "records",
        "records",
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &trap_json,
            "redirect_cause",
            "trap_redirect",
            "flushed"
        ),
        trap_redirect_cause_flushed
    );
    assert_eq!(trap_redirect_cause_flushed, trap_stage_flushed);
    let trap_redirect_cause_cycles =
        in_order_redirect_cause_stage_metric_values(&trap_json, "trap_redirect", "flushed_cycles");
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &trap_json,
            "redirect_cause",
            "trap_redirect",
            "flushed_cycles"
        ),
        trap_redirect_cause_cycles
    );
    assert_eq!(trap_redirect_cause_cycles, trap_stage_flushed_cycles);
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &trap_json,
            "redirect_cause",
            "trap_redirect",
            "records"
        ),
        trap_redirect_cause_records
    );
    assert!(
        trap_redirect_cause_cycles
            .iter()
            .any(|cycles| *cycles > 0),
        "trap run should attribute redirect-cause flushed cycles by stage: {trap_redirect_cause_cycles:?}"
    );
    assert!(
        trap_redirect_cause_records.iter().any(|records| *records > 0),
        "trap run should attribute redirect-cause active records by stage: {trap_redirect_cause_records:?}"
    );
    assert!(
        trap_redirect_cause_record_count <= trap_redirect_cause_records.iter().sum::<u64>(),
        "trap aggregate redirect-cause records should not exceed stage records: records={trap_redirect_cause_record_count} stage_records={trap_redirect_cause_records:?}"
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&trap_json, "branch_prediction", "flushed"),
        [0; 5]
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &trap_json,
            "redirect_cause",
            "branch_prediction",
            "flushed"
        ),
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
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &trap_json,
            "redirect_cause",
            "branch_prediction",
            "flushed_cycles"
        ),
        [0; 5]
    );
    assert_eq!(
        in_order_redirect_cause_stage_metric_values(&trap_json, "branch_prediction", "records"),
        [0; 5]
    );
    assert_eq!(
        in_order_cause_metric_value(&trap_json, "redirect_cause", "branch_prediction", "records"),
        0
    );
    assert_eq!(
        in_order_artifact_cause_metric_value(
            &trap_json,
            "redirect_cause",
            "branch_prediction",
            "records",
        ),
        0
    );
    assert_in_order_cause_alias(
        &trap_json,
        "redirect_cause",
        "redirectCause",
        "branch_prediction",
        "branchPrediction",
        "records",
        "records",
    );
    assert_eq!(
        in_order_artifact_cause_stage_metric_values(
            &trap_json,
            "redirect_cause",
            "branch_prediction",
            "records"
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
    assert_pipeline_summary_matches_trace(&json);
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
    let pipeline_summary = json
        .pointer("/debug/pipeline_summary")
        .unwrap_or_else(|| panic!("missing pipeline summary JSON: {json}"));
    let data_wait_summary = pipeline_summary
        .pointer("/stall_cause/data_wait")
        .unwrap_or_else(|| panic!("missing data_wait pipeline summary: {pipeline_summary}"));
    assert_eq!(
        json_record_u64(pipeline_summary, "records"),
        trace.len() as u64,
        "pipeline summary should cover the emitted Pipeline debug records"
    );
    assert_eq!(
        json_record_u64(data_wait_summary, "stall_cycles"),
        stall_cycles,
        "pipeline summary should preserve data_wait stall cycles"
    );
    for (stage, cycles) in &stage_blocked_cycles {
        assert_eq!(
            data_wait_summary
                .pointer(&format!("/stage/{stage}/resource_blocked_cycles"))
                .and_then(Value::as_u64),
            Some(*cycles),
            "pipeline summary should preserve data_wait resource-blocked cycles for {stage}: {data_wait_summary}"
        );
    }
    assert_eq!(
        pipeline_summary
            .pointer("/stall_cause/fetch_wait/stage/execute/resource_blocked_cycles")
            .and_then(Value::as_u64),
        Some(0),
        "pipeline summary should include explicit zero lanes for absent cause-stage combinations"
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
    assert_pipeline_summary_matches_trace(&json);
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
    for (stage, cycles) in &stage_flushed_cycles {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.trap_redirect_flushed_cycles"),
            "Cycle",
            *cycles,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.pipeline_trace.redirect_cause.trap_redirect.stage.{stage}.flushed_cycles"
            ),
            "Cycle",
            *cycles,
            "monotonic",
        );
    }
    let pipeline_summary = json
        .pointer("/debug/pipeline_summary")
        .unwrap_or_else(|| panic!("missing pipeline summary JSON: {json}"));
    let trap_redirect_summary = pipeline_summary
        .pointer("/redirect_cause/trap_redirect")
        .unwrap_or_else(|| panic!("missing trap_redirect pipeline summary: {pipeline_summary}"));
    assert_eq!(
        json_record_u64(pipeline_summary, "records"),
        trace.len() as u64,
        "pipeline summary should cover the emitted Pipeline debug records"
    );
    for (stage, flushed) in &stage_flushed {
        assert_eq!(
            trap_redirect_summary
                .pointer(&format!("/stage/{stage}/flushed"))
                .and_then(Value::as_u64),
            Some(*flushed),
            "pipeline summary should preserve trap_redirect flushed count for {stage}: {trap_redirect_summary}"
        );
    }
    for (stage, cycles) in &stage_flushed_cycles {
        assert_eq!(
            trap_redirect_summary
                .pointer(&format!("/stage/{stage}/flushed_cycles"))
                .and_then(Value::as_u64),
            Some(*cycles),
            "pipeline summary should preserve trap_redirect flushed cycles for {stage}: {trap_redirect_summary}"
        );
    }
    assert_eq!(
        pipeline_summary
            .pointer("/redirect_cause/branch_prediction/stage/commit/flushed_cycles")
            .and_then(Value::as_u64),
        Some(0),
        "pipeline summary should include explicit zero lanes for absent redirect cause-stage combinations"
    );
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
    assert_pipeline_summary_matches_trace(&json);
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
    assert_pipeline_summary_matches_trace(&json);
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
    assert_pipeline_summary_matches_trace(&json);
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
    let pipeline_summary = json
        .pointer("/debug/pipeline_summary")
        .unwrap_or_else(|| panic!("missing pipeline summary JSON: {json}"));
    let branch_flush_summary = pipeline_summary
        .pointer("/flush_cause/branch_prediction")
        .unwrap_or_else(|| {
            panic!("missing branch_prediction pipeline summary: {pipeline_summary}")
        });
    assert_eq!(
        json_record_u64(pipeline_summary, "records"),
        trace.len() as u64,
        "pipeline summary should cover the emitted Pipeline debug records"
    );
    for (stage, flushed) in &stage_flushed {
        assert_eq!(
            branch_flush_summary
                .pointer(&format!("/stage/{stage}/flushed"))
                .and_then(Value::as_u64),
            Some(*flushed),
            "pipeline summary should preserve branch_prediction flushed count for {stage}: {branch_flush_summary}"
        );
    }
    for (stage, cycles) in &stage_flushed_cycles {
        assert_eq!(
            branch_flush_summary
                .pointer(&format!("/stage/{stage}/flushed_cycles"))
                .and_then(Value::as_u64),
            Some(*cycles),
            "pipeline summary should preserve branch_prediction flushed cycles for {stage}: {branch_flush_summary}"
        );
    }
    assert_eq!(
        pipeline_summary
            .pointer("/flush_cause/trap_redirect/stage/commit/flushed_cycles")
            .and_then(Value::as_u64),
        Some(0),
        "pipeline summary should include explicit zero lanes for absent flush cause-stage combinations"
    );
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

fn text_stat_value(stdout: &str, path: &str) -> u64 {
    stdout
        .lines()
        .find_map(|line| {
            let mut columns = line.split_whitespace();
            let sample_path = columns.next()?;
            if sample_path == path {
                columns.next()?.parse().ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing text stat {path}: {stdout}"))
}

fn run_execute_wait_ordering_program(path: &std::path::Path, stats_format: &str) -> String {
    run_execute_wait_ordering_program_with_debug(path, stats_format, None)
}

fn run_execute_wait_ordering_program_with_debug(
    path: &std::path::Path,
    stats_format: &str,
    debug_flags: Option<&str>,
) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "240",
        "--stats-format",
        stats_format,
        "--execute",
        "--memory-system",
        "direct",
        "--riscv-in-order-width",
        "4",
    ]);
    if let Some(debug_flags) = debug_flags {
        command.args(["--debug-flags", debug_flags]);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn json_stat<'a>(json: &'a Value, path: &str) -> &'a Value {
    json.pointer("/stats")
        .and_then(Value::as_array)
        .expect("stats array")
        .iter()
        .find(|stat| stat.get("path").and_then(Value::as_str) == Some(path))
        .unwrap_or_else(|| panic!("missing stat path {path}"))
}

fn json_stat_value(json: &Value, path: &str) -> u64 {
    json_stat(json, path)
        .get("value")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric value for stat path {path}"))
}

fn assert_json_stat_ids_are_unique(json: &Value) {
    let mut ids = BTreeSet::new();
    for stat in json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("stats array")
    {
        let id = stat
            .get("id")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing numeric stat id: {stat}"));
        let path = stat
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("<missing path>");
        assert!(ids.insert(id), "duplicate stat id {id} for path {path}");
    }
}

fn assert_in_order_pipeline_aliases(json: &Value) {
    assert_in_order_pipeline_aliases_for_cpu(json, 0, "system.cpu.pipeline.inOrder");
    assert_json_stat_id_after(
        json,
        "system.cpu.pipeline.inOrder.advanced",
        "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.commit.orderingBlockedCycles",
    );
}

fn assert_in_order_pipeline_aliases_for_cpu(json: &Value, cpu: u64, alias_prefix: &str) {
    for (source_name, alias_name) in [
        ("advanced", "advanced"),
        ("flushed", "flushed"),
        ("flush_cycles", "flushCycles"),
        ("resource_blocked", "resourceBlocked"),
        ("ordering_blocked", "orderingBlocked"),
        ("stall_cycles", "stallCycles"),
        ("fetch_wait_cycles", "fetchWaitCycles"),
        ("data_wait_cycles", "dataWaitCycles"),
        ("execute_wait_cycles", "executeWaitCycles"),
        ("branch_prediction_flushes", "branchPredictionFlushes"),
        (
            "branch_prediction_flush_cycles",
            "branchPredictionFlushCycles",
        ),
        ("redirects", "redirects"),
        ("branch_prediction_redirects", "branchPredictionRedirects"),
        ("interrupt_redirects", "interruptRedirects"),
        ("interrupt_redirect_flushes", "interruptRedirectFlushes"),
        (
            "interrupt_redirect_flush_cycles",
            "interruptRedirectFlushCycles",
        ),
        ("trap_redirects", "trapRedirects"),
        ("trap_redirect_flushes", "trapRedirectFlushes"),
        ("trap_redirect_flush_cycles", "trapRedirectFlushCycles"),
        (
            "branch_speculation_predictions",
            "branchSpeculationPredictions",
        ),
        ("branch_speculation_repairs", "branchSpeculationRepairs"),
        (
            "branch_speculation_removed_youngers",
            "branchSpeculationRemovedYoungers",
        ),
        (
            "branch_speculation_max_pending",
            "branchSpeculationMaxPending",
        ),
    ] {
        assert_json_stat_alias(
            json,
            &format!("sim.cpu{cpu}.pipeline.in_order.{source_name}"),
            &format!("{alias_prefix}.{alias_name}"),
        );
    }
}

fn assert_in_order_pipeline_stage_aliases(json: &Value) {
    assert_in_order_pipeline_stage_aliases_for_cpu(json, 0, "system.cpu.pipeline.inOrder");
    assert_json_stat_id_after(
        json,
        "system.cpu.pipeline.inOrder.stage.fetch1.occupiedCycles",
        "system.cpu.pipeline.inOrder.branchSpeculationMaxPending",
    );
}

fn assert_in_order_pipeline_stage_aliases_for_cpu(json: &Value, cpu: u64, alias_prefix: &str) {
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        for (source_name, alias_name) in [
            ("width", "width"),
            ("in_flight", "inFlight"),
            ("max_in_flight", "maxInFlight"),
            ("occupied_cycles", "occupiedCycles"),
            ("advanced", "advanced"),
            ("advanced_cycles", "advancedCycles"),
            ("retired", "retired"),
            ("retired_cycles", "retiredCycles"),
            ("resource_blocked", "resourceBlocked"),
            ("resource_blocked_cycles", "resourceBlockedCycles"),
            ("ordering_blocked", "orderingBlocked"),
            ("ordering_blocked_cycles", "orderingBlockedCycles"),
            ("flushed", "flushed"),
            ("flushed_cycles", "flushedCycles"),
            ("branch_prediction_flushed", "branchPredictionFlushed"),
            (
                "branch_prediction_flushed_cycles",
                "branchPredictionFlushedCycles",
            ),
            ("interrupt_redirect_flushed", "interruptRedirectFlushed"),
            (
                "interrupt_redirect_flushed_cycles",
                "interruptRedirectFlushedCycles",
            ),
            ("trap_redirect_flushed", "trapRedirectFlushed"),
            ("trap_redirect_flushed_cycles", "trapRedirectFlushedCycles"),
        ] {
            assert_json_stat_alias(
                json,
                &format!("sim.cpu{cpu}.pipeline.in_order.stage.{stage}.{source_name}"),
                &format!("{alias_prefix}.stage.{stage}.{alias_name}"),
            );
        }
    }
}

fn assert_json_stat_alias(json: &Value, source_path: &str, alias_path: &str) {
    let alias = json_stat(json, alias_path);
    let source = json_stat(json, source_path);
    assert_eq!(
        alias.get("value").and_then(Value::as_u64),
        source.get("value").and_then(Value::as_u64),
        "value mismatch for {alias_path}"
    );
    assert_eq!(
        alias.get("unit").and_then(Value::as_str),
        source.get("unit").and_then(Value::as_str),
        "unit mismatch for {alias_path}"
    );
    assert_eq!(
        alias.get("reset_policy").and_then(Value::as_str),
        source.get("reset_policy").and_then(Value::as_str),
        "reset policy mismatch for {alias_path}"
    );
}

fn assert_json_stat_id_after(json: &Value, later_path: &str, earlier_path: &str) {
    let later_id = json_stat_id(json, later_path);
    let earlier_id = json_stat_id(json, earlier_path);
    assert!(
        later_id > earlier_id,
        "{later_path} id {later_id} should be appended after {earlier_path} id {earlier_id}"
    );
}

fn json_stat_id(json: &Value, path: &str) -> u64 {
    json_stat(json, path)
        .get("id")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing numeric id for stat path {path}"))
}

fn in_order_stall_cause_stage_metric_values(json: &Value, cause: &str, metric: &str) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        json_stat_value(
            json,
            &format!("sim.cpu0.pipeline.in_order.stall_cause.{cause}.stage.{stage}.{metric}"),
        )
    })
}

fn in_order_stall_cause_metric_value(json: &Value, cause: &str, metric: &str) -> u64 {
    json_stat_value(
        json,
        &format!("sim.cpu0.pipeline.in_order.stall_cause.{cause}.{metric}"),
    )
}

fn in_order_artifact_stall_cause_metric_value(json: &Value, cause: &str, metric: &str) -> u64 {
    let pointer = format!("/cores/0/in_order_pipeline/stall_cause/{cause}/{metric}");
    json.pointer(&pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!(
                "missing artifact in-order stall-cause metric {metric} for {cause} at {pointer}: {json}"
            )
        })
}

fn assert_in_order_stall_cause_alias(
    json: &Value,
    cause: &str,
    alias_cause: &str,
    metric: &str,
    alias_metric: &str,
) {
    let alias_path = format!("system.cpu.pipeline.inOrder.stallCause.{alias_cause}.{alias_metric}");
    let source_path = format!("sim.cpu0.pipeline.in_order.stall_cause.{cause}.{metric}");
    assert_json_stat_alias(json, &source_path, &alias_path);
}

fn assert_in_order_stall_cause_stage_aliases(
    json: &Value,
    cause: &str,
    alias_cause: &str,
    metric: &str,
    alias_metric: &str,
) {
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        let alias_path = format!(
            "system.cpu.pipeline.inOrder.stallCause.{alias_cause}.stage.{stage}.{alias_metric}"
        );
        let source_path =
            format!("sim.cpu0.pipeline.in_order.stall_cause.{cause}.stage.{stage}.{metric}");
        assert_json_stat_alias(json, &source_path, &alias_path);
    }
}

fn assert_in_order_cause_stage_alias_family(json: &Value, family: &str, alias_family: &str) {
    for (cause, alias_cause) in [
        ("branch_prediction", "branchPrediction"),
        ("interrupt_redirect", "interruptRedirect"),
        ("trap_redirect", "trapRedirect"),
    ] {
        for (metric, alias_metric) in [
            ("records", "records"),
            ("flushed", "flushed"),
            ("flushed_cycles", "flushedCycles"),
        ] {
            assert_in_order_cause_stage_aliases(
                json,
                family,
                alias_family,
                cause,
                alias_cause,
                metric,
                alias_metric,
            );
        }
    }
}

fn assert_in_order_cause_stage_aliases(
    json: &Value,
    family: &str,
    alias_family: &str,
    cause: &str,
    alias_cause: &str,
    metric: &str,
    alias_metric: &str,
) {
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        let alias_path = format!(
            "system.cpu.pipeline.inOrder.{alias_family}.{alias_cause}.stage.{stage}.{alias_metric}"
        );
        let source_path =
            format!("sim.cpu0.pipeline.in_order.{family}.{cause}.stage.{stage}.{metric}");
        assert_json_stat_alias(json, &source_path, &alias_path);
    }
}

fn in_order_artifact_stall_cause_stage_metric_values(
    json: &Value,
    cause: &str,
    metric: &str,
) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        let pointer =
            format!("/cores/0/in_order_pipeline/stall_cause/{cause}/stage_{metric}/{stage}");
        json.pointer(&pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!(
                    "missing artifact in-order stall-cause stage metric {metric} for {cause}/{stage} at {pointer}: {json}"
                )
            })
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

fn in_order_cause_metric_value(json: &Value, family: &str, cause: &str, metric: &str) -> u64 {
    json_stat_value(
        json,
        &format!("sim.cpu0.pipeline.in_order.{family}.{cause}.{metric}"),
    )
}

fn in_order_artifact_cause_metric_value(
    json: &Value,
    family: &str,
    cause: &str,
    metric: &str,
) -> u64 {
    let pointer = format!("/cores/0/in_order_pipeline/{family}/{cause}/{metric}");
    json.pointer(&pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!(
                "missing artifact in-order {family} metric {metric} for {cause} at {pointer}: {json}"
            )
        })
}

fn assert_in_order_cause_alias(
    json: &Value,
    family: &str,
    alias_family: &str,
    cause: &str,
    alias_cause: &str,
    metric: &str,
    alias_metric: &str,
) {
    let alias_path =
        format!("system.cpu.pipeline.inOrder.{alias_family}.{alias_cause}.{alias_metric}");
    let source_path = format!("sim.cpu0.pipeline.in_order.{family}.{cause}.{metric}");
    assert_json_stat_alias(json, &source_path, &alias_path);
}

fn in_order_artifact_cause_stage_metric_values(
    json: &Value,
    family: &str,
    cause: &str,
    metric: &str,
) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        let pointer = format!("/cores/0/in_order_pipeline/{family}/{cause}/stage_{metric}/{stage}");
        json.pointer(&pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!(
                    "missing artifact in-order {family} stage metric {metric} for {cause}/{stage} at {pointer}: {json}"
                )
            })
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

fn in_order_artifact_stage_metric_values(json: &Value, metric: &str) -> [u64; 5] {
    ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        let pointer = format!("/cores/0/in_order_pipeline/stage_{metric}/{stage}");
        json.pointer(&pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!(
                    "missing artifact in-order stage metric {metric} for {stage} at {pointer}: {json}"
                )
            })
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
