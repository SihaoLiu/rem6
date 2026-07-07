use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

use serde_json::Value;

use crate::support::*;

const M5_EXIT: u32 = 0x21;
const M5_FAIL: u32 = 0x22;
const M5_RESET_STATS: u32 = 0x40;
const M5_DUMP_STATS: u32 = 0x41;
const M5_DUMP_RESET_STATS: u32 = 0x42;
const M5_CHECKPOINT: u32 = 0x43;
const M5_SWITCH_CPU: u32 = 0x52;
const M5_WORK_BEGIN: u32 = 0x5a;
const M5_WORK_END: u32 = 0x5b;
const M5_HYPERCALL: u32 = 0x71;
const SBI_LEGACY_CONSOLE_PUTCHAR: i32 = 1;
const SBI_TIME_EXTENSION: i32 = 0x5449_4d45u32 as i32;
const SBI_TIME_SET_TIMER: i32 = 0;
const SBI_SRST_EXTENSION: i32 = 0x5352_5354;
const SBI_SRST_SYSTEM_RESET: i32 = 0;
const SBI_HSM_EXTENSION: i32 = 0x0048_534d;
const SBI_HSM_HART_GET_STATUS: i32 = 2;
const RISCV_SBI_ENTRY: u64 = 0x8000_0000;

#[test]
fn rem6_run_exec_debug_flag_emits_real_instruction_trace() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-exec", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "60",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Exec",
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
    assert_exec_trace(
        &json,
        &[
            ExpectedExecTraceRecord {
                tick: 2,
                pc: "0x80000000",
                bytes: "93027000",
            },
            ExpectedExecTraceRecord {
                tick: 4,
                pc: "0x80000004",
                bytes: "13831200",
            },
            ExpectedExecTraceRecord {
                tick: 6,
                pc: "0x80000008",
                bytes: "73000000",
            },
        ],
    );
    let trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    let retired_records = trace
        .iter()
        .filter(|record| record.get("retired").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    let exec_bytes = trace
        .iter()
        .map(|record| {
            record
                .get("bytes")
                .and_then(Value::as_str)
                .expect("exec bytes")
                .len() as u64
                / 2
        })
        .sum::<u64>();
    assert_eq!(retired_records, trace.len() as u64);
    assert!(exec_bytes > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.exec_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.exec_trace.retired",
        "Count",
        retired_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.exec_trace.bytes",
        "Byte",
        exec_bytes,
        "monotonic",
    );
    assert_exec_trace_hierarchy_stats(&stdout, trace);
}

#[test]
fn rem6_run_fetch_debug_flag_emits_real_fetch_issue_trace() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fetch", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "60",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Exec,Fetch",
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
        Some(&vec![
            Value::String("Exec".to_string()),
            Value::String("Fetch".to_string())
        ])
    );
    assert_fetch_trace(
        &json,
        &[
            ExpectedFetchTraceRecord {
                tick: 0,
                pc: "0x80000000",
                sequence: 0,
                size: 4,
            },
            ExpectedFetchTraceRecord {
                tick: 2,
                pc: "0x80000004",
                sequence: 1,
                size: 4,
            },
            ExpectedFetchTraceRecord {
                tick: 4,
                pc: "0x80000008",
                sequence: 2,
                size: 4,
            },
        ],
    );
    let fetch_trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    let exec_trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    let fetch_bytes = fetch_trace
        .iter()
        .map(|record| {
            record
                .get("size")
                .and_then(Value::as_u64)
                .expect("fetch size")
        })
        .sum::<u64>();
    let exec_bytes = exec_trace
        .iter()
        .map(|record| {
            record
                .get("bytes")
                .and_then(Value::as_str)
                .expect("exec bytes")
                .len() as u64
                / 2
        })
        .sum::<u64>();
    let trace_records = fetch_trace.len() as u64 + exec_trace.len() as u64;
    let trace_payload_bytes = fetch_bytes + exec_bytes;
    assert!(fetch_bytes > 0, "trace: {fetch_trace:?}");
    assert!(exec_bytes > 0, "trace: {exec_trace:?}");
    assert_eq!(exec_trace.len(), 3);
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.payload_bytes",
        "Byte",
        trace_payload_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fetch_trace.records",
        "Count",
        fetch_trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fetch_trace.bytes",
        "Byte",
        fetch_bytes,
        "monotonic",
    );
    assert_fetch_trace_hierarchy_stats(&stdout, fetch_trace);
}

#[test]
fn rem6_run_branch_debug_flag_emits_real_in_order_branch_trace() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        0x0000_0073,                // target: ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-branch", &elf);

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
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "Branch",
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
        Some(&vec![Value::String("Branch".to_string())])
    );
    let trace = json
        .pointer("/debug/branch_trace")
        .and_then(Value::as_array)
        .expect("debug branch trace array");
    let branch = trace
        .iter()
        .find(|record| record.get("pc").and_then(Value::as_str) == Some("0x80000004"))
        .unwrap_or_else(|| panic!("missing taken branch trace record: {trace:?}"));
    assert_eq!(branch.get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(branch.get("sequence").and_then(Value::as_u64), Some(1));
    assert_eq!(
        branch.get("resolved_stage").and_then(Value::as_str),
        Some("commit")
    );
    assert_eq!(
        branch.get("kind").and_then(Value::as_str),
        Some("conditional")
    );
    assert_eq!(
        branch.get("conditional").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.get("predicted_taken").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(branch.get("predicted_target"), Some(&Value::Null));
    assert_eq!(
        branch.get("resolved_taken").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.get("resolved_target").and_then(Value::as_str),
        Some("0x8000000c")
    );
    assert_eq!(
        branch.get("mispredicted").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.get("repair_target").and_then(Value::as_str),
        Some("0x8000000c")
    );
    assert!(branch.get("cycle").and_then(Value::as_u64).is_some());
    let flushed_sequences = branch
        .get("flushed_sequences")
        .and_then(Value::as_array)
        .expect("flushed sequence array");
    assert_eq!(
        branch.get("flushed_count").and_then(Value::as_u64),
        Some(flushed_sequences.len() as u64)
    );
    assert!(
        !flushed_sequences.is_empty(),
        "taken branch should flush at least one younger fetch: {branch:?}"
    );

    let mut aggregate = BranchTraceStats::default();
    for record in trace {
        aggregate.add_record(record);
    }
    aggregate.assert_stats(&stdout, "sim.debug.branch_trace");
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        1,
        "monotonic",
    );
    assert_branch_trace_hierarchy_stats(&stdout, trace);
}

#[test]
fn rem6_run_pipeline_debug_flag_emits_real_in_order_cycle_trace() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        0x0000_0073,                // target: ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-pipeline", &elf);

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
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert!(!trace.is_empty(), "pipeline trace should not be empty");
    let redirect_cycle = trace
        .iter()
        .find(|record| record.get("redirect_target").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing redirect cycle in pipeline trace: {trace:?}"));
    assert_eq!(redirect_cycle.get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        redirect_cycle.get("state_changed").and_then(Value::as_bool),
        Some(true)
    );
    assert!(json_record_u64(redirect_cycle, "branch_predictions") > 0);
    assert!(json_record_u64(redirect_cycle, "branch_mispredictions") > 0);
    assert!(json_record_u64(redirect_cycle, "branch_prediction_flushed") > 0);
    assert_eq!(
        redirect_cycle.get("flush_cause").and_then(Value::as_str),
        Some("branch_prediction")
    );
    assert_eq!(
        redirect_cycle.get("redirect_cause").and_then(Value::as_str),
        Some("branch_prediction")
    );
    assert!(!redirect_cycle
        .get("advanced")
        .and_then(Value::as_array)
        .expect("advanced pipeline entries")
        .is_empty());
    assert!(!redirect_cycle
        .get("flushed")
        .and_then(Value::as_array)
        .expect("flushed pipeline entries")
        .is_empty());
    assert!(redirect_cycle
        .get("before_in_flight")
        .and_then(Value::as_array)
        .is_some());
    assert!(redirect_cycle
        .get("after_in_flight")
        .and_then(Value::as_array)
        .is_some());
    let branch_prediction_redirects = json_path_u64(
        &json,
        "/cores/0/in_order_pipeline/branch_prediction_redirects",
    );
    let trap_redirects = json_path_u64(&json, "/cores/0/in_order_pipeline/trap_redirects");
    let redirects = json_path_u64(&json, "/cores/0/in_order_pipeline/redirects");
    assert!(branch_prediction_redirects > 0, "{stdout}");
    assert_eq!(branch_prediction_redirects + trap_redirects, redirects);
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.branch_prediction_redirects",
        "Count",
        branch_prediction_redirects,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.trap_redirects",
        "Count",
        trap_redirects,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.redirect_cause.branch_prediction.records",
        "Count",
        branch_prediction_redirects,
        "monotonic",
    );

    let mut aggregate = PipelineTraceStats::default();
    for record in trace {
        aggregate.add_record(record);
    }
    aggregate.assert_stats(&stdout, "sim.debug.pipeline_trace");
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        1,
        "monotonic",
    );
    assert_pipeline_trace_hierarchy_stats(&stdout, trace);
    assert_pipeline_flush_cause(&stdout, trace, "branch_prediction");
    assert_pipeline_trace_stage_flushed(&stdout, trace);
}

#[test]
fn rem6_run_pipeline_debug_flag_classifies_real_wait_stall_causes() {
    let fetch_program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let fetch_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &fetch_program);
    let fetch_path = temp_binary("debug-flags-pipeline-fetch-wait-cause", &fetch_elf);
    let fetch_stdout = run_pipeline_debug_wait_program(
        &fetch_path,
        &["--min-remote-delay", "2", "--memory-route-delay", "5"],
    );
    assert_pipeline_wait_cause(&fetch_stdout, "fetch_wait");

    let mut data_program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),        // sd x6, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    data_program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    data_program.extend_from_slice(&0u64.to_le_bytes());
    data_program.extend_from_slice(&[0; 16]);
    let data_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &data_program);
    let data_path = temp_binary("debug-flags-pipeline-data-wait-cause", &data_elf);
    let data_stdout = run_pipeline_debug_wait_program(&data_path, &[]);
    assert_pipeline_wait_cause(&data_stdout, "data_wait");

    let execute_program = riscv64_program(&[
        0x0060_0093, // addi x1, x0, 6
        0x0070_0113, // addi x2, x0, 7
        0x0220_81b3, // mul x3, x1, x2
        0x0000_0073, // ecall
    ]);
    let execute_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &execute_program);
    let execute_path = temp_binary("debug-flags-pipeline-execute-wait-cause", &execute_elf);
    let execute_stdout =
        run_pipeline_debug_wait_program(&execute_path, &["--memory-system", "direct"]);
    assert_pipeline_wait_cause(&execute_stdout, "execute_wait");
}

#[test]
fn rem6_run_pipeline_debug_flag_attributes_trap_redirect_suppression() {
    let program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        i_type(9, 0, 0x0, 6, 0x13), // wrong-path addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-pipeline-trap-redirect-suppression", &elf);

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
            "--memory-route-delay",
            "5",
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
    let trap_redirect = trace
        .iter()
        .find(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("trap_redirect")
        })
        .unwrap_or_else(|| panic!("missing trap redirect cause in pipeline trace: {trace:?}"));
    let branch_predictions = trace
        .iter()
        .map(|record| json_record_u64(record, "branch_predictions"))
        .sum::<u64>();
    let branch_prediction_flushed = trace
        .iter()
        .map(|record| json_record_u64(record, "branch_prediction_flushed"))
        .sum::<u64>();
    assert_eq!(
        trap_redirect.get("redirect_target").and_then(Value::as_str),
        Some("0x0")
    );
    assert_eq!(trap_redirect.get("flush_cause"), Some(&Value::Null));
    assert!(json_record_u64(trap_redirect, "branch_predictions") == 0);
    assert!(json_record_u64(trap_redirect, "branch_prediction_flushed") == 0);
    assert_eq!(
        branch_predictions, 0,
        "trap-only run should not emit branch predictions: {trace:?}"
    );
    assert_eq!(
        branch_prediction_flushed, 0,
        "trap-only run should not emit branch-prediction flushes: {trace:?}"
    );
    assert!(record_array(trap_redirect, "flushed").is_empty());
    assert!(!stdout.contains("\"x6\":\"0x9\""));
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.branch_predictions",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.branch_prediction_flushed",
        "Count",
        0,
        "monotonic",
    );
    let trap_redirects = json_path_u64(&json, "/cores/0/in_order_pipeline/trap_redirects");
    let branch_prediction_redirects = json_path_u64(
        &json,
        "/cores/0/in_order_pipeline/branch_prediction_redirects",
    );
    let redirects = json_path_u64(&json, "/cores/0/in_order_pipeline/redirects");
    assert_eq!(trap_redirects, 1);
    assert_eq!(branch_prediction_redirects, 0);
    assert_eq!(branch_prediction_redirects + trap_redirects, redirects);
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.trap_redirects",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.branch_prediction_redirects",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        json_path_u64(&json, "/cores/0/in_order_pipeline/trap_redirect_flushes"),
        0
    );
    assert_eq!(
        json_path_u64(
            &json,
            "/cores/0/in_order_pipeline/trap_redirect_flush_cycles"
        ),
        0
    );
    let stage_trap_redirect_flushed = json_stage_summary_from_path(
        &json,
        "/cores/0/in_order_pipeline/stage_trap_redirect_flushed",
    );
    let stage_trap_redirect_flushed_cycles = json_stage_summary_from_path(
        &json,
        "/cores/0/in_order_pipeline/stage_trap_redirect_flushed_cycles",
    );
    assert_eq!(stage_trap_redirect_flushed, [0; 5]);
    assert_eq!(stage_trap_redirect_flushed_cycles, [0; 5]);
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.trap_redirect_flushes",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.trap_redirect_flush_cycles",
        "Cycle",
        0,
        "monotonic",
    );
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.trap_redirect_flushed"),
            "Count",
            0,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.trap_redirect_flushed_cycles"),
            "Cycle",
            0,
            "monotonic",
        );
    }
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.redirect_cause.trap_redirect.records",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.redirect_cause.trap_redirect.branch_prediction_flushed",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.redirect_cause.trap_redirect.flushed",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_pipeline_debug_flag_attributes_widened_trap_redirect_flush_cause() {
    let program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        i_type(9, 0, 0x0, 6, 0x13), // wrong-path addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-pipeline-widened-trap-flush", &elf);

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
    let trap_redirect = trace
        .iter()
        .find(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("trap_redirect")
                && !record_array(record, "flushed").is_empty()
        })
        .unwrap_or_else(|| panic!("missing widened trap redirect flush in trace: {trace:?}"));
    let branch_predictions = trace
        .iter()
        .map(|record| json_record_u64(record, "branch_predictions"))
        .sum::<u64>();
    let branch_prediction_flushed = trace
        .iter()
        .map(|record| json_record_u64(record, "branch_prediction_flushed"))
        .sum::<u64>();
    let flushed = record_array(trap_redirect, "flushed").len() as u64;
    let mut stage_flushed = BTreeMap::<String, u64>::new();
    for flushed in record_array(trap_redirect, "flushed") {
        let stage = flushed
            .get("stage")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
        *stage_flushed.entry(stat_path_segment(stage)).or_default() += 1;
    }

    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("environment_call")
    );
    assert_eq!(
        trap_redirect.get("flush_cause").and_then(Value::as_str),
        Some("trap_redirect"),
        "widened trap redirects that squash younger in-flight instructions need an explicit flush cause: {trap_redirect:?}"
    );
    assert_eq!(
        trap_redirect.get("redirect_target").and_then(Value::as_str),
        Some("0x0")
    );
    assert_eq!(
        branch_predictions, 0,
        "trap-only run should not emit branch predictions: {trace:?}"
    );
    assert_eq!(
        branch_prediction_flushed, 0,
        "trap-only run should not emit branch-prediction flushes: {trace:?}"
    );
    assert!(flushed > 0, "widened trap redirect should flush: {trace:?}");
    assert!(
        !stage_flushed.is_empty(),
        "trap redirect flush cause should preserve flushed stages: {trace:?}"
    );
    assert_eq!(
        json_path_u64(&json, "/cores/0/in_order_pipeline/trap_redirects"),
        1
    );
    assert_eq!(
        json_path_u64(&json, "/cores/0/in_order_pipeline/trap_redirect_flushes"),
        flushed
    );
    assert!(
        json_path_u64(
            &json,
            "/cores/0/in_order_pipeline/trap_redirect_flush_cycles"
        ) > 0,
        "{stdout}"
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.flush_cause.trap_redirect.records",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.flush_cause.trap_redirect.flushed",
        "Count",
        flushed,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.pipeline_trace.flush_cause.trap_redirect.branch_prediction_flushed",
        "Count",
        0,
        "monotonic",
    );
    for (stage, flushed) in stage_flushed {
        assert_stat(
            &stdout,
            &format!("sim.debug.pipeline_trace.flush_cause.trap_redirect.stage.{stage}.flushed"),
            "Count",
            flushed,
            "monotonic",
        );
    }
    assert_pipeline_summary_flush_stage_records(
        &json,
        "flush_cause",
        "trap_redirect",
        &[trap_redirect],
    );
    assert_pipeline_summary_flush_stage_records(
        &json,
        "redirect_cause",
        "trap_redirect",
        &[trap_redirect],
    );
    assert!(!stdout.contains("\"x6\":\"0x9\""));
}

#[test]
fn rem6_run_pipeline_debug_flag_attributes_stage_activity() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13),  // addi x5, x0, 1
        i_type(2, 0, 0x0, 6, 0x13),  // addi x6, x0, 2
        i_type(3, 0, 0x0, 7, 0x13),  // addi x7, x0, 3
        i_type(4, 0, 0x0, 28, 0x13), // addi x28, x0, 4
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-pipeline-resource-blocked", &elf);

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
    assert_pipeline_trace_stage_activity(&stdout);
}

#[test]
fn rem6_run_fetch_debug_flag_keeps_fetches_across_riscv_se_stream_reset() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        0x0070_0293,                   // addi x5, x0, 7
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fetch-riscv-se-reset", &elf);

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
            "--riscv-se",
            "--debug-flags",
            "Fetch",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Fetch".to_string())])
    );
    assert_fetch_pcs(
        &json,
        &[
            "0x80000000",
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
        ],
    );
}

#[test]
fn rem6_run_data_debug_flag_emits_real_data_access_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297,                                   // auipc x5, 0
        0x0402_8293,                                   // addi x5, x5, 64
        0x0052_b023,                                   // sd x5, 0(x5)
        0x0002_b303,                                   // ld x6, 0(x5)
        atomic_type(0x00, false, false, 6, 5, 0x3, 7), // amoadd.d x7, x6, (x5)
        0x0000_0073,                                   // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-data", &elf);

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
            "--debug-flags",
            "Data",
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
        Some(&vec![Value::String("Data".to_string())])
    );
    let trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("debug data trace array");
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[0].get("kind").and_then(Value::as_str), Some("store"));
    assert_eq!(trace[1].get("kind").and_then(Value::as_str), Some("load"));
    assert_eq!(trace[2].get("kind").and_then(Value::as_str), Some("atomic"));
    let load_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("load"))
        .count() as u64;
    let store_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("store"))
        .count() as u64;
    let atomic_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("atomic"))
        .count() as u64;
    let load_bytes = debug_trace_sum(trace, "load", "size");
    let store_bytes = debug_trace_sum(trace, "store", "size");
    let atomic_bytes = debug_trace_sum(trace, "atomic", "size");
    assert!(load_bytes > 0, "trace: {trace:?}");
    assert!(store_bytes > 0, "trace: {trace:?}");
    assert!(atomic_bytes > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.data_trace.loads",
        "Count",
        load_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.stores",
        "Count",
        store_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.atomics",
        "Count",
        atomic_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.load_bytes",
        "Byte",
        load_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.store_bytes",
        "Byte",
        store_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.data_trace.atomic_bytes",
        "Byte",
        atomic_bytes,
        "monotonic",
    );
    assert_data_trace_hierarchy_stats(&stdout, trace);
    for record in trace {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("address").and_then(Value::as_str),
            Some("0x80000040")
        );
        assert_eq!(record.get("size").and_then(Value::as_u64), Some(8));
        assert!(record.get("tick").and_then(Value::as_u64).is_some());
    }
}

#[test]
fn rem6_run_memory_debug_flag_emits_real_transport_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-memory", &elf);

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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Memory".to_string())])
    );
    let trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("debug memory trace array");
    assert!(
        trace.len() >= 6,
        "expected fetch and data transport events, got {trace:?}"
    );
    assert!(trace.iter().any(|record| {
        record.get("channel").and_then(Value::as_str) == Some("fetch")
            && record.get("kind").and_then(Value::as_str) == Some("request_sent")
    }));
    assert!(trace.iter().any(|record| {
        record.get("channel").and_then(Value::as_str) == Some("data")
            && record.get("kind").and_then(Value::as_str) == Some("request_sent")
    }));
    assert!(trace.iter().any(|record| {
        record.get("kind").and_then(Value::as_str) == Some("response_arrived")
            && record.get("response_status").and_then(Value::as_str) == Some("completed")
    }));
    let fetch_records = trace
        .iter()
        .filter(|record| record.get("channel").and_then(Value::as_str) == Some("fetch"))
        .count() as u64;
    let data_records = trace
        .iter()
        .filter(|record| record.get("channel").and_then(Value::as_str) == Some("data"))
        .count() as u64;
    let request_sent_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("request_sent"))
        .count() as u64;
    let request_arrived_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("request_arrived"))
        .count() as u64;
    let response_arrived_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("response_arrived"))
        .count() as u64;
    let completed_responses = trace
        .iter()
        .filter(|record| record.get("response_status").and_then(Value::as_str) == Some("completed"))
        .count() as u64;
    let retry_responses = trace
        .iter()
        .filter(|record| record.get("response_status").and_then(Value::as_str) == Some("retry"))
        .count() as u64;
    let store_conditional_failed_responses = trace
        .iter()
        .filter(|record| {
            record.get("response_status").and_then(Value::as_str)
                == Some("store_conditional_failed")
        })
        .count() as u64;
    let requests = memory_trace_unique_requests(trace, None);
    let fetch_requests = memory_trace_unique_requests(trace, Some("fetch"));
    let data_requests = memory_trace_unique_requests(trace, Some("data"));
    let routes = memory_trace_unique_routes(trace, None);
    let fetch_routes = memory_trace_unique_routes(trace, Some("fetch"));
    let data_routes = memory_trace_unique_routes(trace, Some("data"));
    let request_agents = memory_trace_unique_request_agents(trace);
    let response_latency_ticks = memory_trace_response_latency_sum(trace);
    let max_response_latency_ticks = memory_trace_response_latency_max(trace);
    assert!(request_sent_records > 0, "trace: {trace:?}");
    assert!(request_arrived_records > 0, "trace: {trace:?}");
    assert!(response_arrived_records > 0, "trace: {trace:?}");
    assert!(completed_responses > 0, "trace: {trace:?}");
    assert!(response_latency_ticks > 0, "trace: {trace:?}");
    assert!(max_response_latency_ticks > 0, "trace: {trace:?}");
    assert!(requests > 0, "trace: {trace:?}");
    assert!(fetch_requests > 0, "trace: {trace:?}");
    assert!(data_requests > 0, "trace: {trace:?}");
    assert!(routes > 0, "trace: {trace:?}");
    assert!(fetch_routes > 0, "trace: {trace:?}");
    assert!(data_routes > 0, "trace: {trace:?}");
    assert!(request_agents > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.fetch.records",
        "Count",
        fetch_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.data.records",
        "Count",
        data_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.events.request_sent",
        "Count",
        request_sent_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.events.request_arrived",
        "Count",
        request_arrived_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.events.response_arrived",
        "Count",
        response_arrived_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_status.completed",
        "Count",
        completed_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_status.retry",
        "Count",
        retry_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_status.store_conditional_failed",
        "Count",
        store_conditional_failed_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.requests",
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.fetch.requests",
        "Count",
        fetch_requests,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.data.requests",
        "Count",
        data_requests,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.routes",
        "Count",
        routes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.fetch.routes",
        "Count",
        fetch_routes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.data.routes",
        "Count",
        data_routes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.request_agents",
        "Count",
        request_agents,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.response_latency_ticks",
        "Tick",
        response_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.memory_trace.max_response_latency_ticks",
        "Tick",
        max_response_latency_ticks,
        "monotonic",
    );
    assert_memory_trace_hierarchy_stats(&stdout, trace);
    assert_memory_trace_response_latencies(trace);
    for record in trace {
        assert!(record.get("tick").and_then(Value::as_u64).is_some());
        assert!(record.get("route").and_then(Value::as_u64).is_some());
        assert!(record.get("request").and_then(Value::as_u64).is_some());
        assert!(record.get("endpoint").and_then(Value::as_str).is_some());
    }
}

#[test]
fn rem6_run_cache_debug_flag_emits_real_cache_hierarchy_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03),                 // ld x6, 16(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 48, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 16..DATA_OFFSET + 24]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-cache", &elf);

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
            "--memory-system",
            "cache-fabric-dram",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
            "--debug-flags",
            "Cache",
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
        Some(&vec![Value::String("Cache".to_string())])
    );
    let trace = json
        .pointer("/debug/cache_trace")
        .and_then(Value::as_array)
        .expect("debug cache trace array");
    assert_eq!(trace.len(), 6, "trace should cover I/D L1/L2/L3: {trace:?}");

    assert_cache_trace_record(
        trace,
        "instruction",
        "l1",
        &json,
        "/memory_resources/cache/instruction/l1",
    );
    assert_cache_trace_record(
        trace,
        "instruction",
        "l2",
        &json,
        "/memory_resources/cache/instruction/l2",
    );
    assert_cache_trace_record(
        trace,
        "instruction",
        "l3",
        &json,
        "/memory_resources/cache/instruction/l3",
    );
    assert_cache_trace_record(
        trace,
        "data",
        "l1",
        &json,
        "/memory_resources/cache/data/l1",
    );
    assert_cache_trace_record(
        trace,
        "data",
        "l2",
        &json,
        "/memory_resources/cache/data/l2",
    );
    assert_cache_trace_record(
        trace,
        "data",
        "l3",
        &json,
        "/memory_resources/cache/data/l3",
    );

    let active_scopes = cache_trace_active_count(trace);
    let activity = cache_trace_sum(trace, "activity");
    let cpu_responses = cache_trace_sum(trace, "cpu_responses");
    let directory_decisions = cache_trace_sum(trace, "directory_decisions");
    let dram_accesses = cache_trace_sum(trace, "dram_accesses");
    assert!(active_scopes > 0, "trace: {trace:?}");
    assert!(activity > 0, "trace: {trace:?}");
    assert!(cpu_responses > 0, "trace: {trace:?}");
    assert!(directory_decisions > 0, "trace: {trace:?}");
    assert!(dram_accesses > 0, "trace: {trace:?}");
    assert!(
        json_path_u64(
            &json,
            "/memory_resources/cache/data/l1/prefetch_queue_issued"
        ) > 0,
        "trace: {trace:?}"
    );
    assert!(
        json_path_u64(&json, "/memory_resources/cache/data/l1/prefetch_useful") > 0,
        "trace: {trace:?}"
    );
    assert_eq!(
        active_scopes,
        json_path_u64(&json, "/memory_resources/cache/active")
    );
    assert_eq!(
        activity,
        json_path_u64(&json, "/memory_resources/cache/activity")
    );
    assert_eq!(
        cpu_responses,
        json_path_u64(&json, "/memory_resources/cache/cpu_responses")
    );
    assert_eq!(
        directory_decisions,
        json_path_u64(&json, "/memory_resources/cache/directory_decisions")
    );
    assert_eq!(
        dram_accesses,
        json_path_u64(&json, "/memory_resources/cache/dram_accesses")
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.active_scopes",
        "Count",
        active_scopes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.activity",
        "Count",
        activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.cpu_responses",
        "Count",
        cpu_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.directory_decisions",
        "Count",
        directory_decisions,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.cache_trace.dram_accesses",
        "Count",
        dram_accesses,
        "monotonic",
    );
    for (field, stat_suffix) in CACHE_TRACE_COUNT_FIELDS {
        let value = cache_trace_sum(trace, field);
        assert_eq!(
            value,
            json_path_u64(&json, &format!("/memory_resources/cache/{field}")),
            "cache trace aggregate {field}: {trace:?}"
        );
        assert_stat(
            &stdout,
            &format!("sim.debug.cache_trace.{stat_suffix}"),
            "Count",
            value,
            "monotonic",
        );
    }
    for (field, stat_suffix) in [
        ("prefetch_accuracy_ppm", "prefetch.accuracy_ppm"),
        ("prefetch_coverage_ppm", "prefetch.coverage_ppm"),
    ] {
        let value = json_path_u64(&json, &format!("/memory_resources/cache/{field}"));
        assert_stat(
            &stdout,
            &format!("sim.debug.cache_trace.{stat_suffix}"),
            "Ppm",
            value,
            "monotonic",
        );
    }
    for record in trace {
        assert_cache_trace_hierarchy_stats(&stdout, record);
    }
}

#[test]
fn rem6_run_fabric_debug_flag_emits_real_fabric_activity_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fabric", &elf);

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
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--fabric-link",
            "cpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "8",
            "--fabric-request-virtual-network",
            "3",
            "--fabric-response-virtual-network",
            "4",
            "--fabric-credit-depth",
            "2",
            "--debug-flags",
            "Fabric",
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
        Some(&vec![Value::String("Fabric".to_string())])
    );
    let trace = json
        .pointer("/debug/fabric_trace")
        .and_then(Value::as_array)
        .expect("debug fabric trace array");
    assert!(
        trace.iter().any(|record| {
            record.get("kind").and_then(Value::as_str) == Some("lane")
                && record.get("link").and_then(Value::as_str) == Some("cpu_mem")
                && record.get("virtual_network").and_then(Value::as_u64) == Some(3)
                && record
                    .get("transfer_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|transfers| transfers > 0)
                && record
                    .get("flit_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|flits| flits > 0)
        }),
        "missing request-lane fabric record: {trace:?}"
    );
    assert!(
        trace.iter().any(|record| {
            record.get("kind").and_then(Value::as_str) == Some("hop")
                && record.get("link").and_then(Value::as_str) == Some("cpu_mem")
                && record.get("virtual_network").and_then(Value::as_u64) == Some(4)
                && record
                    .get("arrival_tick")
                    .and_then(Value::as_u64)
                    .zip(record.get("start_tick").and_then(Value::as_u64))
                    .is_some_and(|(arrival, start)| arrival >= start)
        }),
        "missing response-hop fabric record: {trace:?}"
    );
    let lane_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .count() as u64;
    let hop_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("hop"))
        .count() as u64;
    let lane_transfers = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .map(|record| {
            record
                .get("transfer_count")
                .and_then(Value::as_u64)
                .expect("lane transfer_count")
        })
        .sum::<u64>();
    let lane_bytes = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .map(|record| {
            record
                .get("byte_count")
                .and_then(Value::as_u64)
                .expect("lane byte_count")
        })
        .sum::<u64>();
    let lane_flits = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("lane"))
        .map(|record| {
            record
                .get("flit_count")
                .and_then(Value::as_u64)
                .expect("lane flit_count")
        })
        .sum::<u64>();
    let hop_transfers = hop_records;
    let hop_bytes = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("hop"))
        .map(|record| {
            record
                .get("bytes")
                .and_then(Value::as_u64)
                .expect("hop bytes")
        })
        .sum::<u64>();
    let hop_flits = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("hop"))
        .map(|record| {
            record
                .get("flits")
                .and_then(Value::as_u64)
                .expect("hop flits")
        })
        .sum::<u64>();
    let lane_occupied_ticks = fabric_trace_sum(trace, "lane", "occupied_ticks");
    let lane_queue_delay_ticks = fabric_trace_sum(trace, "lane", "queue_delay_ticks");
    let lane_max_queue_delay_ticks = fabric_trace_max(trace, "lane", "max_queue_delay_ticks");
    let lane_credit_delay_ticks = fabric_trace_sum(trace, "lane", "credit_delay_ticks");
    let lane_max_credit_delay_ticks = fabric_trace_max(trace, "lane", "max_credit_delay_ticks");
    let hop_occupied_ticks = fabric_trace_sum(trace, "hop", "occupied_ticks");
    let hop_queue_delay_ticks = fabric_trace_sum(trace, "hop", "queue_delay_ticks");
    let hop_max_queue_delay_ticks = fabric_trace_max(trace, "hop", "queue_delay_ticks");
    let hop_credit_delay_ticks = fabric_trace_sum(trace, "hop", "credit_delay_ticks");
    let hop_max_credit_delay_ticks = fabric_trace_max(trace, "hop", "credit_delay_ticks");
    assert!(lane_records >= 2, "trace: {trace:?}");
    assert!(hop_records >= 2, "trace: {trace:?}");
    assert!(lane_transfers > 0, "trace: {trace:?}");
    assert!(lane_bytes > 0, "trace: {trace:?}");
    assert!(lane_flits > 0, "trace: {trace:?}");
    assert!(hop_bytes > 0, "trace: {trace:?}");
    assert!(hop_flits > 0, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lanes",
        "Count",
        lane_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hops",
        "Count",
        hop_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.payload_bytes",
        "Byte",
        lane_bytes + hop_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.transfers",
        "Count",
        lane_transfers,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.bytes",
        "Byte",
        lane_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.flits",
        "Count",
        lane_flits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.occupied_ticks",
        "Tick",
        lane_occupied_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.queue_delay_ticks",
        "Tick",
        lane_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.max_queue_delay_ticks",
        "Tick",
        lane_max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.credit_delay_ticks",
        "Tick",
        lane_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.lane.max_credit_delay_ticks",
        "Tick",
        lane_max_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.transfers",
        "Count",
        hop_transfers,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.bytes",
        "Byte",
        hop_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.flits",
        "Count",
        hop_flits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.occupied_ticks",
        "Tick",
        hop_occupied_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.queue_delay_ticks",
        "Tick",
        hop_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.max_queue_delay_ticks",
        "Tick",
        hop_max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.credit_delay_ticks",
        "Tick",
        hop_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.fabric_trace.hop.max_credit_delay_ticks",
        "Tick",
        hop_max_credit_delay_ticks,
        "monotonic",
    );
    assert_fabric_trace_hierarchy_stats(&stdout, trace);
}

#[test]
fn rem6_run_dram_debug_flag_emits_real_dram_hierarchy_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-dram", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--debug-flags",
            "Dram",
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
        Some(&vec![Value::String("Dram".to_string())])
    );
    let trace = json
        .pointer("/debug/dram_trace")
        .and_then(Value::as_array)
        .expect("debug DRAM trace array");
    let target_record = trace
        .iter()
        .find(|record| {
            record.get("kind").and_then(Value::as_str) == Some("target")
                && record.get("target").and_then(Value::as_u64) == Some(0)
                && record
                    .get("accesses")
                    .and_then(Value::as_u64)
                    .is_some_and(|accesses| accesses > 0)
                && record
                    .get("reads")
                    .and_then(Value::as_u64)
                    .is_some_and(|reads| reads > 0)
        })
        .unwrap_or_else(|| panic!("missing target DRAM record: {trace:?}"));
    assert!(target_record.get("read_bytes").is_none());
    assert!(target_record.get("write_bytes").is_none());

    let port_record = trace
        .iter()
        .find(|record| {
            record.get("kind").and_then(Value::as_str) == Some("port")
                && record.get("target").and_then(Value::as_u64) == Some(0)
                && record.get("port").and_then(Value::as_u64).is_some()
                && record
                    .get("commands")
                    .and_then(Value::as_u64)
                    .is_some_and(|commands| commands > 0)
        })
        .unwrap_or_else(|| panic!("missing port DRAM record: {trace:?}"));
    assert!(
        port_record
            .get("row_hits")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose row hits: {port_record:?}"
    );
    assert!(
        port_record
            .get("row_misses")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose row misses: {port_record:?}"
    );
    assert!(
        port_record
            .get("refreshes")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose refreshes: {port_record:?}"
    );
    assert!(
        port_record
            .get("refresh_ticks")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose refresh ticks: {port_record:?}"
    );
    assert!(
        port_record
            .get("total_ready_latency_ticks")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose total ready latency: {port_record:?}"
    );
    assert!(
        port_record
            .get("max_ready_latency_ticks")
            .and_then(Value::as_u64)
            .is_some(),
        "port DRAM record should expose max ready latency: {port_record:?}"
    );

    let bank_record = trace
        .iter()
        .find(|record| {
            record.get("kind").and_then(Value::as_str) == Some("bank")
                && record.get("target").and_then(Value::as_u64) == Some(0)
                && record.get("port").and_then(Value::as_u64).is_some()
                && record.get("bank").and_then(Value::as_u64).is_some()
                && record
                    .get("read_bytes")
                    .and_then(Value::as_u64)
                    .is_some_and(|bytes| bytes > 0)
                && record
                    .get("max_ready_latency_ticks")
                    .and_then(Value::as_u64)
                    .is_some()
        })
        .unwrap_or_else(|| panic!("missing bank DRAM record: {trace:?}"));
    assert!(bank_record.get("reads").is_none());
    assert!(bank_record.get("writes").is_none());
    assert!(bank_record.get("turnarounds").is_none());
    let target_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("target"))
        .count() as u64;
    let port_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("port"))
        .count() as u64;
    let bank_records = trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("bank"))
        .count() as u64;
    let target_accesses = debug_trace_sum(trace, "target", "accesses");
    let target_reads = debug_trace_sum(trace, "target", "reads");
    let target_writes = debug_trace_sum(trace, "target", "writes");
    let target_row_hits = debug_trace_sum(trace, "target", "row_hits");
    let target_row_misses = debug_trace_sum(trace, "target", "row_misses");
    let target_refreshes = debug_trace_sum(trace, "target", "refreshes");
    let target_refresh_ticks = debug_trace_sum(trace, "target", "refresh_ticks");
    let target_commands = debug_trace_sum(trace, "target", "commands");
    let target_turnarounds = debug_trace_sum(trace, "target", "turnarounds");
    let target_total_ready_latency_ticks =
        debug_trace_sum(trace, "target", "total_ready_latency_ticks");
    let target_max_ready_latency_ticks =
        debug_trace_max(trace, "target", "max_ready_latency_ticks");
    let port_accesses = debug_trace_sum(trace, "port", "accesses");
    let port_reads = debug_trace_sum(trace, "port", "reads");
    let port_writes = debug_trace_sum(trace, "port", "writes");
    let port_commands = debug_trace_sum(trace, "port", "commands");
    let port_row_hits = debug_trace_sum(trace, "port", "row_hits");
    let port_row_misses = debug_trace_sum(trace, "port", "row_misses");
    let port_refreshes = debug_trace_sum(trace, "port", "refreshes");
    let port_refresh_ticks = debug_trace_sum(trace, "port", "refresh_ticks");
    let port_turnarounds = debug_trace_sum(trace, "port", "turnarounds");
    let port_total_ready_latency_ticks =
        debug_trace_sum(trace, "port", "total_ready_latency_ticks");
    let port_max_ready_latency_ticks = debug_trace_max(trace, "port", "max_ready_latency_ticks");
    let bank_accesses = debug_trace_sum(trace, "bank", "accesses");
    let bank_read_bytes = debug_trace_sum(trace, "bank", "read_bytes");
    let bank_write_bytes = debug_trace_sum(trace, "bank", "write_bytes");
    let bank_row_hits = debug_trace_sum(trace, "bank", "row_hits");
    let bank_row_misses = debug_trace_sum(trace, "bank", "row_misses");
    let bank_refreshes = debug_trace_sum(trace, "bank", "refreshes");
    let bank_refresh_ticks = debug_trace_sum(trace, "bank", "refresh_ticks");
    let bank_commands = debug_trace_sum(trace, "bank", "commands");
    let bank_total_ready_latency_ticks =
        debug_trace_sum(trace, "bank", "total_ready_latency_ticks");
    let bank_max_ready_latency_ticks = debug_trace_max(trace, "bank", "max_ready_latency_ticks");
    assert!(target_records >= 1, "trace: {trace:?}");
    assert!(port_records >= 1, "trace: {trace:?}");
    assert!(bank_records >= 1, "trace: {trace:?}");
    assert!(target_accesses > 0, "trace: {trace:?}");
    assert!(target_reads > 0, "trace: {trace:?}");
    assert!(target_writes > 0, "trace: {trace:?}");
    assert!(port_accesses > 0, "trace: {trace:?}");
    assert!(port_reads > 0, "trace: {trace:?}");
    assert!(port_writes > 0, "trace: {trace:?}");
    assert!(port_commands > 0, "trace: {trace:?}");
    assert!(bank_accesses > 0, "trace: {trace:?}");
    assert!(bank_read_bytes > 0, "trace: {trace:?}");
    assert!(bank_write_bytes > 0, "trace: {trace:?}");
    assert_eq!(port_row_hits, bank_row_hits);
    assert_eq!(port_row_misses, bank_row_misses);
    assert_eq!(port_refreshes, bank_refreshes);
    assert_eq!(port_refresh_ticks, bank_refresh_ticks);
    assert_eq!(
        port_total_ready_latency_ticks,
        bank_total_ready_latency_ticks
    );
    assert_eq!(port_max_ready_latency_ticks, bank_max_ready_latency_ticks);
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.targets",
        "Count",
        target_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.ports",
        "Count",
        port_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.banks",
        "Count",
        bank_records,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.accesses",
        "Count",
        target_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.reads",
        "Count",
        target_reads,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.writes",
        "Count",
        target_writes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.row_hits",
        "Count",
        target_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.row_misses",
        "Count",
        target_row_misses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.refreshes",
        "Count",
        target_refreshes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.refresh_ticks",
        "Tick",
        target_refresh_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.commands",
        "Count",
        target_commands,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.turnarounds",
        "Count",
        target_turnarounds,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.total_ready_latency_ticks",
        "Tick",
        target_total_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.target.max_ready_latency_ticks",
        "Tick",
        target_max_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.accesses",
        "Count",
        port_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.reads",
        "Count",
        port_reads,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.writes",
        "Count",
        port_writes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.commands",
        "Count",
        port_commands,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.row_hits",
        "Count",
        port_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.row_misses",
        "Count",
        port_row_misses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.refreshes",
        "Count",
        port_refreshes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.refresh_ticks",
        "Tick",
        port_refresh_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.turnarounds",
        "Count",
        port_turnarounds,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.total_ready_latency_ticks",
        "Tick",
        port_total_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.port.max_ready_latency_ticks",
        "Tick",
        port_max_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.accesses",
        "Count",
        bank_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.read_bytes",
        "Byte",
        bank_read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.write_bytes",
        "Byte",
        bank_write_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.row_hits",
        "Count",
        bank_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.row_misses",
        "Count",
        bank_row_misses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.refreshes",
        "Count",
        bank_refreshes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.refresh_ticks",
        "Tick",
        bank_refresh_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.commands",
        "Count",
        bank_commands,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.total_ready_latency_ticks",
        "Tick",
        bank_total_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.dram_trace.bank.max_ready_latency_ticks",
        "Tick",
        bank_max_ready_latency_ticks,
        "monotonic",
    );
    for record in trace {
        assert_dram_trace_hierarchy_stats(&stdout, record);
    }
}

#[test]
fn rem6_run_dram_debug_flag_exposes_lpddr_low_power_hierarchy_trace() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(128, 6, 2, 0x3),                      // sd x6, 128(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 192, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-dram-low-power", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "lpddr",
            "--dram-low-power-precharge-powerdown-entry-delay",
            "2",
            "--dram-low-power-self-refresh-entry-delay",
            "5",
            "--dram-low-power-exit-latency",
            "1",
            "--dram-low-power-self-refresh-exit-latency",
            "3",
            "--debug-flags",
            "Dram",
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
        .pointer("/debug/dram_trace")
        .and_then(Value::as_array)
        .expect("debug DRAM trace array");
    let target_self_refresh_entries =
        debug_low_power_trace_sum(trace, "target", "/self_refresh/entries");
    let port_self_refresh_entries =
        debug_low_power_trace_sum(trace, "port", "/self_refresh/entries");
    let bank_self_refresh_entries =
        debug_low_power_trace_sum(trace, "bank", "/self_refresh/entries");
    let bank_self_refresh_ticks = debug_low_power_trace_sum(trace, "bank", "/self_refresh/ticks");
    let bank_exit_latency_ticks = debug_low_power_trace_sum(trace, "bank", "/exit_latency_ticks");

    assert!(target_self_refresh_entries > 0, "trace: {trace:?}");
    assert_eq!(target_self_refresh_entries, port_self_refresh_entries);
    assert_eq!(port_self_refresh_entries, bank_self_refresh_entries);
    assert!(bank_self_refresh_ticks > 0, "trace: {trace:?}");
    assert!(bank_exit_latency_ticks > 0, "trace: {trace:?}");
    for kind in ["target", "port", "bank"] {
        assert_dram_low_power_kind_stats(&stdout, trace, kind);
    }
    for record in trace {
        assert_dram_low_power_trace_stats(&stdout, record);
    }
}

#[test]
fn rem6_run_dram_debug_flag_participates_in_sorted_deduped_flag_lists() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-dram-dedup", &elf);

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
            "--debug-flags",
            "Fetch,Dram,Data,Dram",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![
            Value::String("Data".to_string()),
            Value::String("Dram".to_string()),
            Value::String("Fetch".to_string())
        ])
    );
    assert!(json
        .pointer("/debug/dram_trace")
        .and_then(Value::as_array)
        .is_some_and(|trace| !trace.is_empty()));
    assert!(json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .is_some_and(|trace| !trace.is_empty()));
}

#[test]
fn rem6_run_syscall_debug_flag_emits_real_riscv_se_syscall_trace() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 10, 0x13),   // addi a0, x0, 7
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-syscall", &elf);

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
            "--riscv-se",
            "--debug-flags",
            "Syscall",
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
        Some(&vec![Value::String("Syscall".to_string())])
    );
    let trace = json
        .pointer("/debug/syscall_trace")
        .and_then(Value::as_array)
        .expect("debug syscall trace array");
    assert_eq!(trace.len(), 2);

    assert_eq!(trace[0].get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        trace[0].get("pc").and_then(Value::as_str),
        Some("0x80000008")
    );
    assert_eq!(trace[0].get("number").and_then(Value::as_u64), Some(172));
    assert_eq!(
        trace[0].pointer("/arguments/0").and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        trace[0].pointer("/outcome/kind").and_then(Value::as_str),
        Some("return")
    );
    assert_eq!(
        trace[0].pointer("/outcome/value").and_then(Value::as_u64),
        Some(100)
    );

    assert_eq!(trace[1].get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        trace[1].get("pc").and_then(Value::as_str),
        Some("0x80000014")
    );
    assert_eq!(trace[1].get("number").and_then(Value::as_u64), Some(93));
    assert_eq!(
        trace[1].pointer("/outcome/kind").and_then(Value::as_str),
        Some("exit")
    );
    assert_eq!(
        trace[1].pointer("/outcome/code").and_then(Value::as_i64),
        Some(0)
    );
    let syscall_numbers = syscall_trace_unique_u64(trace, "number");
    let call_sites = syscall_trace_unique_strings(trace, "pc");
    let cpus = syscall_trace_unique_u64(trace, "cpu");
    let argument_words = syscall_trace_argument_words(trace);
    let nonzero_arguments = syscall_trace_nonzero_arguments(trace);
    assert_eq!(syscall_numbers, 2, "trace: {trace:?}");
    assert_eq!(call_sites, 2, "trace: {trace:?}");
    assert_eq!(cpus, 1, "trace: {trace:?}");
    assert_eq!(argument_words, 12, "trace: {trace:?}");
    assert_eq!(nonzero_arguments, 1, "trace: {trace:?}");
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.returns",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.exits",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.blocked",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.syscall_numbers",
        "Count",
        syscall_numbers,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.call_sites",
        "Count",
        call_sites,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.cpus",
        "Count",
        cpus,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.argument_words",
        "Count",
        argument_words,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.syscall_trace.nonzero_arguments",
        "Count",
        nonzero_arguments,
        "monotonic",
    );
    assert_syscall_trace_hierarchy_stats(&stdout, trace);
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_arith_type(funct6: u32, funct3: u32, vs2: u8, vs1_or_rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1_or_rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn fp_r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x53
}

fn fp_r4_type(rs3: u8, funct2: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (u32::from(rs3) << 27)
        | (funct2 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_store_type(vm_unmasked: bool, width: u32, rs1: u8, vs3: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn float_store_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = (imm as u32) & 0xfff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x27
}

fn detailed_o3_runtime_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(7, 0, 0x0, 11, 0x13),
        i_type(0, 5, 0b010, 12, 0x03),
        s_type(4, 12, 5, 0b010),
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_reset_stats_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, 0b010),
        m5op(M5_RESET_STATS),
        i_type(0, 5, 0b010, 12, 0x03),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_memory_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 10, 0x17),
        i_type(60, 10, 0b000, 10, 0x13),
        i_type(8, 10, 0b000, 16, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        vsetvli_type(0xd0, 11, 5),
        vector_unit_stride_load_type(true, 0b110, 10, 1),
        vector_unit_stride_store_type(true, 0b110, 16, 1),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0x1122_3344, 0x5566_7788, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_memory_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0x3, 1, 0x07),
        float_store_type(8, 1, 10, 0x3),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&1.0f64.to_bits().to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_atomic_lsq_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(5, 0, 0x0, 6, 0x13),
        atomic_type(0x00, false, false, 6, 5, 0x3, 7),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([9, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_ordered_atomic_lsq_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 128_i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(0, 5, 0b011, 6, 0x03),
        s_type(8, 6, 5, 0b011),
        atomic_type(0x02, true, false, 0, 5, 0x3, 7),
        i_type(3, 0, 0x0, 8, 0x13),
        atomic_type(0x03, false, true, 8, 5, 0x3, 9),
        i_type(4, 0, 0x0, 10, 0x13),
        atomic_type(0x01, true, true, 10, 5, 0x3, 11),
        s_type(16, 9, 5, 0b011),
        s_type(24, 11, 5, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([9, 0, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_conditional_failure_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(0x2a, 0, 0x0, 6, 0x13),
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),
        s_type(8, 7, 5, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        b_type(8, 5, 5, 0x0),
        i_type(9, 0, 0x0, 6, 0x13),
        i_type(7, 0, 0x0, 7, 0x13),
        s_type(0, 6, 10, 0b011),
        s_type(8, 7, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_not_taken_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(2, 0, 0x0, 6, 0x13),
        b_type(12, 6, 5, 0x0),
        i_type(7, 0, 0x0, 7, 0x13),
        i_type(9, 0, 0x0, 8, 0x13),
        s_type(0, 7, 10, 0b011),
        s_type(8, 8, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_predicted_target_match_debug_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let words = vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(0, 0, 0x0, 9, 0x13),
        b_type(8, 0, 7, 0x1),
        i_type(99, 0, 0x0, 6, 0x13),
        b_type(16, 0, 9, 0x1),
        m5op(M5_SWITCH_CPU),
        i_type(1, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 10, 0x17),
        i_type(data_start - 32, 10, 0x0, 10, 0x13),
        s_type(0, 7, 10, 0b011),
        s_type(8, 9, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let mut words = words;
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_branch_predicted_taken_not_taken_debug_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let words = vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(1, 0, 0x0, 9, 0x13),
        b_type(12, 0, 9, 0x1),
        i_type(11, 0, 0x0, 6, 0x13),
        j_type(16, 0),
        m5op(M5_SWITCH_CPU),
        i_type(0, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 10, 0x17),
        i_type(data_start - 32, 10, 0x0, 10, 0x13),
        s_type(0, 6, 10, 0b011),
        s_type(8, 9, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let mut words = words;
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_direct_jump_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        j_type(8, 0),
        i_type(9, 0, 0x0, 6, 0x13),
        i_type(7, 0, 0x0, 7, 0x13),
        s_type(0, 6, 10, 0b011),
        s_type(8, 7, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_direct_call_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        j_type(8, 1),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_return_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - 4, 10, 0x0, 10, 0x13),
        u_type(0, 1, 0x17),
        i_type(16, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 0, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_indirect_jump_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - 4, 10, 0x0, 10, 0x13),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 0, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_indirect_jump_wrong_target_debug_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let words = vec![
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 0, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
        u_type(0, 10, 0x17),
        i_type(data_start - 36, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let mut words = words;
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_indirect_call_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_start = 64_i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - 4, 10, 0x0, 10, 0x13),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(9, 0, 0x0, 6, 0x13),
        s_type(0, 1, 10, 0b011),
        s_type(8, 6, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_indirect_call_wrong_target_debug_binary(name: &str) -> std::path::PathBuf {
    let data_start = 112_i32;
    let words = vec![
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        i_type(0, 11, 0x0, 1, 0x67),
        i_type(99, 0, 0x0, 6, 0x13),
        m5op(M5_SWITCH_CPU),
        u_type(0, 11, 0x17),
        i_type(16, 11, 0x0, 11, 0x13),
        j_type(-20, 0),
        i_type(77, 0, 0x0, 6, 0x13),
        u_type(0, 10, 0x17),
        i_type(data_start - 36, 10, 0x0, 10, 0x13),
        s_type(0, 11, 10, 0b011),
        s_type(8, 1, 10, 0b011),
        s_type(16, 6, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let mut words = words;
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(42, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        0x0220_81b3,
        0x0220_c1b3,
        m5op(M5_EXIT),
        i_type(77, 0, 0x0, 13, 0x13),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_mvv_type(0b100101, 2, 1, 3),
        vector_mvv_type(0b100000, 2, 1, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_mul_family_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_mvv_type(0b101101, 2, 1, 3),
        vector_mvv_type(0b111000, 2, 1, 8),
        vector_mvv_type(0b111100, 2, 1, 10),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_vector_saturating_mul_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(2, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b100111, 0b000, 2, 1, 3),
        vector_arith_type(0b100111, 0b100, 2, 11, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 8, 1),
        vector_arith_type(0b010111, 0b100, 0, 8, 2),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x08, 2, 1, 0x0, 3),
        fp_r_type(0x0c, 2, 1, 0x0, 4),
        vector_arith_type(0b100100, 0b001, 2, 1, 3),
        vector_arith_type(0b100000, 0b001, 2, 1, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_extended_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        fp_r_type(0x78, 0, 8, 0x0, 3),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 8, 1),
        vector_arith_type(0b010111, 0b100, 0, 8, 2),
        vector_arith_type(0b010111, 0b100, 0, 8, 4),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x00, 2, 1, 0x0, 4),
        fp_r4_type(3, 0x0, 2, 1, 0x0, 5, 0x43),
        fp_r_type(0x2c, 0, 1, 0x0, 6),
        vector_arith_type(0b000000, 0b001, 2, 1, 3),
        vector_arith_type(0b101100, 0b001, 2, 1, 4),
        vector_arith_type(0b010011, 0b001, 1, 0, 5),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_compare_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 8, 1),
        vector_arith_type(0b010111, 0b100, 0, 8, 2),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x50, 2, 1, 0x2, 5),
        vector_arith_type(0b011000, 0b001, 2, 1, 3),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_float_misc_fu_latency_debug_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        u_type(0x3f80_0000, 8, 0x37),
        fp_r_type(0x78, 0, 8, 0x0, 1),
        fp_r_type(0x78, 0, 8, 0x0, 2),
        i_type(3, 0, 0x0, 9, 0x13),
        i_type(2, 0, 0x0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 1, 1),
        vector_arith_type(0b010111, 0b100, 0, 2, 2),
        m5op(M5_SWITCH_CPU),
        fp_r_type(0x68, 0, 9, 0x0, 3),
        fp_r_type(0x10, 2, 1, 0x0, 4),
        vector_arith_type(0b010010, 0b001, 1, 0x02, 3),
        vector_arith_type(0b001000, 0b001, 2, 1, 4),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_forwarding_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, 0b010),
        i_type(0, 5, 0b010, 12, 0x03),
        b_type(8, 11, 12, 0x1),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_forwarding_suppression_debug_binary(
    name: &str,
    load_offset: i32,
    load_funct3: u32,
    expected_register: u8,
) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, 0b010),
        i_type(load_offset, 5, load_funct3, 12, 0x03),
        b_type(8, expected_register, 12, 0x1),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_forwarding_mismatch_debug_binary(name: &str) -> std::path::PathBuf {
    detailed_o3_store_forwarding_suppression_debug_binary(name, 4, 0b010, 0)
}

fn detailed_o3_store_forwarding_byte_mismatch_debug_binary(name: &str) -> std::path::PathBuf {
    detailed_o3_store_forwarding_suppression_debug_binary(name, 0, 0b100, 11)
}

fn detailed_o3_store_forwarding_address_and_byte_mismatch_debug_binary(
    name: &str,
) -> std::path::PathBuf {
    detailed_o3_store_forwarding_suppression_debug_binary(name, 4, 0b100, 0)
}

fn hart1_detailed_o3_debug_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        csr_read(0xf14, 5),
        b_type(8, 0, 5, 0x1),
        b_type(0, 0, 0, 0x0),
        m5op(M5_SWITCH_CPU),
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 6, 0x17),
        i_type(data_start - auipc_pc, 6, 0x0, 6, 0x13),
        i_type(42, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        0x0220_81b3,
        0x0220_c1b3,
        i_type(0, 6, 0b010, 12, 0x03),
        s_type(4, 12, 6, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x1234_5678, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_scheduled_restore_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    for _ in 0..20 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 704_i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(42, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        0x0220_81b3,
        0x0220_c1b3,
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, 0b010),
        i_type(0, 5, 0b010, 12, 0x03),
    ]);
    while words.len() < 170 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn assert_o3_event(
    event: &Value,
    sequence: u64,
    pc: &str,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    system_event: bool,
) {
    assert_eq!(json_record_u64(event, "sequence"), sequence);
    json_record_u64(event, "tick");
    assert_eq!(json_record_str(event, "pc"), pc);
    assert_eq!(json_record_bool(event, "rob_allocated"), true);
    assert_eq!(json_record_bool(event, "rob_committed"), true);
    assert_eq!(json_record_u64(event, "rename_writes"), rename_writes);
    assert_eq!(json_record_u64(event, "lsq_loads"), lsq_loads);
    assert_eq!(json_record_u64(event, "lsq_stores"), lsq_stores);
    assert_o3_event_lsq_address_shape(event, lsq_loads, lsq_stores);
    assert_eq!(json_record_u64(event, "fu_latency_cycles"), 0);
    assert_eq!(event.get("fu_latency_class"), Some(&Value::Null));
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_candidate"),
        false
    );
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_match"),
        false
    );
    assert_eq!(json_record_bool(event, "system_event"), system_event);
}

#[allow(clippy::too_many_arguments)]
fn assert_o3_event_with_fu(
    event: &Value,
    sequence: u64,
    pc: &str,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    fu_latency_cycles: u64,
    fu_latency_class: Option<&str>,
    system_event: bool,
) {
    assert_eq!(json_record_u64(event, "sequence"), sequence);
    json_record_u64(event, "tick");
    assert_eq!(json_record_str(event, "pc"), pc);
    assert_eq!(json_record_bool(event, "rob_allocated"), true);
    assert_eq!(json_record_bool(event, "rob_committed"), true);
    assert_eq!(json_record_u64(event, "rename_writes"), rename_writes);
    assert_eq!(json_record_u64(event, "lsq_loads"), lsq_loads);
    assert_eq!(json_record_u64(event, "lsq_stores"), lsq_stores);
    assert_o3_event_lsq_address_shape(event, lsq_loads, lsq_stores);
    assert_eq!(
        json_record_u64(event, "fu_latency_cycles"),
        fu_latency_cycles
    );
    match fu_latency_class {
        Some(expected) => assert_eq!(json_record_str(event, "fu_latency_class"), expected),
        None => assert_eq!(event.get("fu_latency_class"), Some(&Value::Null)),
    }
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_candidate"),
        false
    );
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_match"),
        false
    );
    assert_eq!(json_record_bool(event, "system_event"), system_event);
}

fn o3_event_fu_latency_class_count(events: &[Value], class: &str) -> u64 {
    events
        .iter()
        .filter(|event| event.get("fu_latency_class").and_then(Value::as_str) == Some(class))
        .count() as u64
}

#[allow(clippy::too_many_arguments)]
fn assert_o3_event_with_store_forwarding(
    event: &Value,
    sequence: u64,
    pc: &str,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    store_load_forwarding_candidate: bool,
    store_load_forwarding_match: bool,
    system_event: bool,
) {
    assert_eq!(json_record_u64(event, "sequence"), sequence);
    json_record_u64(event, "tick");
    assert_eq!(json_record_str(event, "pc"), pc);
    assert_eq!(json_record_bool(event, "rob_allocated"), true);
    assert_eq!(json_record_bool(event, "rob_committed"), true);
    assert_eq!(json_record_u64(event, "rename_writes"), rename_writes);
    assert_eq!(json_record_u64(event, "lsq_loads"), lsq_loads);
    assert_eq!(json_record_u64(event, "lsq_stores"), lsq_stores);
    assert_o3_event_lsq_address_shape(event, lsq_loads, lsq_stores);
    assert_eq!(event.get("fu_latency_class"), Some(&Value::Null));
    assert_eq!(json_record_u64(event, "fu_latency_cycles"), 0);
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_candidate"),
        store_load_forwarding_candidate
    );
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_match"),
        store_load_forwarding_match
    );
    assert_eq!(json_record_bool(event, "system_event"), system_event);
}

fn assert_o3_event_lsq_address_shape(event: &Value, lsq_loads: u64, lsq_stores: u64) {
    if lsq_loads == 0 {
        assert_eq!(event.get("lsq_load_address"), Some(&Value::Null));
    } else {
        json_record_str(event, "lsq_load_address");
    }
    if lsq_stores == 0 {
        assert_eq!(event.get("lsq_store_address"), Some(&Value::Null));
    } else {
        json_record_str(event, "lsq_store_address");
    }
}

fn load_sbi_time_extension(rd: u8) -> [u32; 2] {
    load_sbi_extension(SBI_TIME_EXTENSION, rd)
}

fn load_sbi_srst_extension(rd: u8) -> [u32; 2] {
    load_sbi_extension(SBI_SRST_EXTENSION, rd)
}

fn load_sbi_hsm_extension(rd: u8) -> [u32; 2] {
    load_sbi_extension(SBI_HSM_EXTENSION, rd)
}

fn load_sbi_extension(extension: i32, rd: u8) -> [u32; 2] {
    let upper = (extension + 0x800) & !0xfff;
    let lower = extension - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn host_action_trace_ticks_are_ordered(trace: &[Value]) -> bool {
    trace
        .windows(2)
        .all(|pair| host_action_trace_tick(&pair[0]) <= host_action_trace_tick(&pair[1]))
}

fn host_action_trace_kind_count(trace: &[Value], kind: &str) -> usize {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .count()
}

fn host_action_trace_record<'a>(trace: &'a [Value], kind: &str) -> &'a Value {
    trace
        .iter()
        .find(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .unwrap_or_else(|| panic!("missing host action trace kind {kind}: {trace:?}"))
}

fn sbi_trace_kind_count(trace: &[Value], kind: &str) -> usize {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .count()
}

fn sbi_trace_record<'a>(trace: &'a [Value], kind: &str) -> &'a Value {
    trace
        .iter()
        .find(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .unwrap_or_else(|| panic!("missing SBI trace kind {kind}: {trace:?}"))
}

fn assert_dump_reset_trace_order(trace: &[Value]) {
    let ordered = trace.windows(2).any(|pair| {
        pair[0].get("kind").and_then(Value::as_str) == Some("stats_dump")
            && pair[1].get("kind").and_then(Value::as_str) == Some("stats_reset")
            && pair[0].get("tick").and_then(Value::as_u64)
                == pair[1].get("tick").and_then(Value::as_u64)
            && pair[0].get("epoch").and_then(Value::as_u64)
                == pair[1]
                    .get("epoch")
                    .and_then(Value::as_u64)
                    .and_then(|epoch| epoch.checked_sub(1))
    });
    assert!(
        ordered,
        "missing same-tick dump-before-reset trace: {trace:?}"
    );
}

fn host_action_trace_tick(record: &Value) -> u64 {
    record
        .get("tick")
        .and_then(Value::as_u64)
        .expect("host action trace tick")
}

fn debug_trace_sum(trace: &[Value], kind: &str, field: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{kind} {field}"))
        })
        .sum()
}

fn debug_trace_max(trace: &[Value], kind: &str, field: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{kind} {field}"))
        })
        .max()
        .unwrap_or(0)
}

fn debug_low_power_trace_sum(trace: &[Value], kind: &str, pointer: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| {
            record
                .get("low_power")
                .and_then(|low_power| low_power.pointer(pointer))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("{kind} low_power{pointer}: {record:?}"))
        })
        .sum()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExecTraceStats {
    records: u64,
    retired: u64,
    bytes: u64,
}

impl ExecTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        if record
            .get("retired")
            .and_then(Value::as_bool)
            .expect("exec retired")
        {
            self.retired = self.retired.saturating_add(1);
        }
        self.bytes = self.bytes.saturating_add(
            record
                .get("bytes")
                .and_then(Value::as_str)
                .expect("exec bytes")
                .len() as u64
                / 2,
        );
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("retired", "Count", self.retired),
            ("bytes", "Byte", self.bytes),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_exec_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, ExecTraceStats>::new();
    let mut retirement = BTreeMap::<&'static str, ExecTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let retired = match record
            .get("retired")
            .and_then(Value::as_bool)
            .expect("exec retired")
        {
            true => "retired",
            false => "not_retired",
        };
        cpus.entry(cpu).or_default().add_record(record);
        retirement.entry(retired).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.exec_trace.cpu.cpu{cpu}"));
    }
    for (retirement, stats) in retirement {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.exec_trace.retirement.{retirement}"),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FetchTraceStats {
    records: u64,
    bytes: u64,
}

impl FetchTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(json_record_u64(record, "size"));
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("bytes", "Byte", self.bytes),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_fetch_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, FetchTraceStats>::new();
    let mut endpoints = BTreeMap::<String, FetchTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let endpoint = json_record_str(record, "endpoint").to_string();
        cpus.entry(cpu).or_default().add_record(record);
        endpoints.entry(endpoint).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.fetch_trace.cpu.cpu{cpu}"));
    }
    for (endpoint, stats) in endpoints {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.fetch_trace.endpoint.{}",
                stat_path_segment(&endpoint)
            ),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BranchTraceStats {
    records: u64,
    conditional: u64,
    unconditional: u64,
    predicted_taken: u64,
    resolved_taken: u64,
    mispredictions: u64,
    repairs: u64,
    flushed: u64,
}

impl BranchTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        if json_record_bool(record, "conditional") {
            self.conditional = self.conditional.saturating_add(1);
        } else {
            self.unconditional = self.unconditional.saturating_add(1);
        }
        if json_record_bool(record, "predicted_taken") {
            self.predicted_taken = self.predicted_taken.saturating_add(1);
        }
        if json_record_bool(record, "resolved_taken") {
            self.resolved_taken = self.resolved_taken.saturating_add(1);
        }
        if json_record_bool(record, "mispredicted") {
            self.mispredictions = self.mispredictions.saturating_add(1);
        }
        if record
            .get("repair_target")
            .and_then(Value::as_str)
            .is_some()
        {
            self.repairs = self.repairs.saturating_add(1);
        }
        self.flushed = self
            .flushed
            .saturating_add(json_record_u64(record, "flushed_count"));
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("conditional", self.conditional),
            ("unconditional", self.unconditional),
            ("predicted_taken", self.predicted_taken),
            ("resolved_taken", self.resolved_taken),
            ("mispredictions", self.mispredictions),
            ("repairs", self.repairs),
            ("flushed", self.flushed),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                "Count",
                value,
                "monotonic",
            );
        }
    }
}

fn assert_branch_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, BranchTraceStats>::new();
    let mut kinds = BTreeMap::<String, BranchTraceStats>::new();
    let mut outcomes = BTreeMap::<&'static str, BranchTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let kind = json_record_str(record, "kind").to_string();
        let outcome = match json_record_bool(record, "mispredicted") {
            true => "mispredicted",
            false => "correct",
        };
        cpus.entry(cpu).or_default().add_record(record);
        kinds.entry(kind).or_default().add_record(record);
        outcomes.entry(outcome).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.branch_trace.cpu.cpu{cpu}"));
    }
    for (kind, stats) in kinds {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.branch_trace.kind.{}", stat_path_segment(&kind)),
        );
    }
    for (outcome, stats) in outcomes {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.branch_trace.outcome.{}",
                stat_path_segment(outcome)
            ),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineTraceStats {
    records: u64,
    stall_cycles: u64,
    state_changed: u64,
    advanced: u64,
    retired: u64,
    flushed: u64,
    resource_blocked: u64,
    ordering_blocked: u64,
    branch_predictions: u64,
    branch_mispredictions: u64,
    branch_prediction_flushed: u64,
    redirects: u64,
    before_in_flight: u64,
    after_in_flight: u64,
}

impl PipelineTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        self.stall_cycles = self
            .stall_cycles
            .saturating_add(json_record_u64(record, "stall_cycles"));
        if json_record_bool(record, "state_changed") {
            self.state_changed = self.state_changed.saturating_add(1);
        }
        let advanced = record_array(record, "advanced");
        self.advanced = self.advanced.saturating_add(advanced.len() as u64);
        self.retired = self.retired.saturating_add(
            advanced
                .iter()
                .filter(|entry| json_record_bool(entry, "retires"))
                .count() as u64,
        );
        self.flushed = self
            .flushed
            .saturating_add(record_array(record, "flushed").len() as u64);
        self.resource_blocked = self
            .resource_blocked
            .saturating_add(record_array(record, "resource_blocked").len() as u64);
        self.ordering_blocked = self
            .ordering_blocked
            .saturating_add(record_array(record, "ordering_blocked").len() as u64);
        self.branch_predictions = self
            .branch_predictions
            .saturating_add(json_record_u64(record, "branch_predictions"));
        self.branch_mispredictions = self
            .branch_mispredictions
            .saturating_add(json_record_u64(record, "branch_mispredictions"));
        self.branch_prediction_flushed = self
            .branch_prediction_flushed
            .saturating_add(json_record_u64(record, "branch_prediction_flushed"));
        if record
            .get("redirect_target")
            .and_then(Value::as_str)
            .is_some()
        {
            self.redirects = self.redirects.saturating_add(1);
        }
        self.before_in_flight = self
            .before_in_flight
            .saturating_add(record_array(record, "before_in_flight").len() as u64);
        self.after_in_flight = self
            .after_in_flight
            .saturating_add(record_array(record, "after_in_flight").len() as u64);
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("stall_cycles", self.stall_cycles),
            ("state_changed", self.state_changed),
            ("advanced", self.advanced),
            ("retired", self.retired),
            ("flushed", self.flushed),
            ("resource_blocked", self.resource_blocked),
            ("ordering_blocked", self.ordering_blocked),
            ("branch_predictions", self.branch_predictions),
            ("branch_mispredictions", self.branch_mispredictions),
            ("branch_prediction_flushed", self.branch_prediction_flushed),
            ("redirects", self.redirects),
            ("before_in_flight", self.before_in_flight),
            ("after_in_flight", self.after_in_flight),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                "Count",
                value,
                "monotonic",
            );
        }
    }
}

fn assert_pipeline_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, PipelineTraceStats>::new();
    let mut states = BTreeMap::<&'static str, PipelineTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let state = match json_record_bool(record, "state_changed") {
            true => "changed",
            false => "unchanged",
        };
        cpus.entry(cpu).or_default().add_record(record);
        states.entry(state).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.pipeline_trace.cpu.cpu{cpu}"));
    }
    for (state, stats) in states {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.pipeline_trace.state.{}",
                stat_path_segment(state)
            ),
        );
    }
}

fn assert_pipeline_flush_cause(stdout: &str, trace: &[Value], cause: &str) {
    let json: Value = serde_json::from_str(stdout).unwrap();
    let flush_records = trace
        .iter()
        .filter(|record| record.get("flush_cause").and_then(Value::as_str) == Some(cause))
        .collect::<Vec<_>>();
    assert!(
        !flush_records.is_empty(),
        "missing pipeline flush cause {cause}: {trace:?}"
    );
    let flushed = flush_records
        .iter()
        .map(|record| record_array(record, "flushed").len() as u64)
        .sum::<u64>();
    let branch_prediction_flushed = flush_records
        .iter()
        .map(|record| json_record_u64(record, "branch_prediction_flushed"))
        .sum::<u64>();
    let mut stage_flushed = BTreeMap::<String, u64>::new();
    for record in &flush_records {
        for flushed in record_array(record, "flushed") {
            let stage = flushed
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
            *stage_flushed.entry(stat_path_segment(stage)).or_default() += 1;
        }
    }
    assert!(flushed > 0, "flush cause {cause}: {trace:?}");
    assert!(
        branch_prediction_flushed > 0,
        "flush cause {cause}: {trace:?}"
    );
    assert!(
        !stage_flushed.is_empty(),
        "flush cause {cause} should preserve flushed in-flight instructions: {trace:?}"
    );
    assert_stat(
        stdout,
        &format!("sim.debug.pipeline_trace.flush_cause.{cause}.records"),
        "Count",
        flush_records.len() as u64,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("sim.debug.pipeline_trace.flush_cause.{cause}.flushed"),
        "Count",
        flushed,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("sim.debug.pipeline_trace.flush_cause.{cause}.branch_prediction_flushed"),
        "Count",
        branch_prediction_flushed,
        "monotonic",
    );
    for (stage, flushed) in stage_flushed {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.flush_cause.{cause}.stage.{stage}.flushed"),
            "Count",
            flushed,
            "monotonic",
        );
    }
    assert_pipeline_summary_flush_stage_records(&json, "flush_cause", cause, &flush_records);
}

fn assert_pipeline_summary_flush_stage_records(
    json: &Value,
    category: &str,
    cause: &str,
    records: &[&Value],
) {
    let mut stage_records = BTreeMap::<String, u64>::new();
    for record in records {
        let mut record_stages = BTreeSet::new();
        for flushed in record_array(record, "flushed") {
            let stage = flushed
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
            record_stages.insert(stat_path_segment(stage));
        }
        for stage in record_stages {
            *stage_records.entry(stage).or_default() += 1;
        }
    }
    assert!(
        !stage_records.is_empty(),
        "pipeline summary {category} {cause} should have active flushed stages"
    );
    for (stage, records) in &stage_records {
        assert_eq!(
            json_path_u64(
                json,
                &format!("/debug/pipeline_summary/{category}/{cause}/stage/{stage}/records")
            ),
            *records,
            "pipeline summary {category} {cause} stage {stage} should count active records"
        );
    }
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        if !stage_records.contains_key(stage) {
            assert_eq!(
                json_path_u64(
                    json,
                    &format!("/debug/pipeline_summary/{category}/{cause}/stage/{stage}/records")
                ),
                0,
                "pipeline summary {category} {cause} stage {stage} should expose a zero record lane"
            );
        }
    }
}

fn assert_pipeline_trace_stage_flushed(stdout: &str, trace: &[Value]) {
    let mut stage_flushed = BTreeMap::<String, u64>::new();
    for record in trace {
        for flushed in record_array(record, "flushed") {
            let stage = flushed
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing flushed instruction stage: {flushed}"));
            *stage_flushed.entry(stat_path_segment(stage)).or_default() += 1;
        }
    }
    assert!(
        !stage_flushed.is_empty(),
        "pipeline trace should preserve flushed in-flight instructions: {trace:?}"
    );
    for (stage, flushed) in stage_flushed {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.flushed"),
            "Count",
            flushed,
            "monotonic",
        );
    }
}

fn run_pipeline_debug_wait_program(path: &Path, extra_args: &[&str]) -> String {
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
        "json",
        "--execute",
        "--debug-flags",
        "Pipeline",
    ]);
    command.args(extra_args);
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn assert_pipeline_wait_cause(stdout: &str, cause: &str) {
    let json: Value = serde_json::from_str(stdout).unwrap();
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let wait_records = trace
        .iter()
        .filter(|record| record.get("stall_cause").and_then(Value::as_str) == Some(cause))
        .collect::<Vec<_>>();
    assert!(
        !wait_records.is_empty(),
        "missing pipeline stall cause {cause}: {trace:?}"
    );
    let stall_cycles = wait_records
        .iter()
        .map(|record| json_record_u64(record, "stall_cycles"))
        .sum::<u64>();
    assert!(stall_cycles > 0, "stall cause {cause}: {trace:?}");
    let mut stage_resource_blocked = BTreeMap::<String, u64>::new();
    let mut stage_records = BTreeMap::<String, u64>::new();
    for record in &wait_records {
        let mut record_stages = BTreeSet::new();
        for blocked in record_array(record, "resource_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing blocked instruction stage: {blocked}"));
            let stage = stat_path_segment(stage);
            *stage_resource_blocked.entry(stage.clone()).or_default() += 1;
            record_stages.insert(stage);
        }
        for blocked in record_array(record, "ordering_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing ordering-blocked instruction stage: {blocked}"));
            record_stages.insert(stat_path_segment(stage));
        }
        for stage in record_stages {
            *stage_records.entry(stage).or_default() += 1;
        }
    }
    assert!(
        !stage_resource_blocked.is_empty(),
        "stall cause {cause} should preserve blocked in-flight instructions: {trace:?}"
    );
    assert_stat(
        stdout,
        &format!("sim.debug.pipeline_trace.stall_cause.{cause}.records"),
        "Count",
        wait_records.len() as u64,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("sim.debug.pipeline_trace.stall_cause.{cause}.stall_cycles"),
        "Count",
        stall_cycles,
        "monotonic",
    );
    assert_eq!(
        json_path_u64(
            &json,
            &format!("/debug/pipeline_summary/stall_cause/{cause}/records")
        ),
        wait_records.len() as u64,
        "pipeline summary stall cause {cause} should count records"
    );
    assert_eq!(
        json_path_u64(
            &json,
            &format!("/debug/pipeline_summary/stall_cause/{cause}/stall_cycles")
        ),
        stall_cycles,
        "pipeline summary stall cause {cause} should count stall cycles"
    );
    for (stage, resource_blocked) in stage_resource_blocked {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.stall_cause.{cause}.stage.{stage}.resource_blocked"),
            "Count",
            resource_blocked,
            "monotonic",
        );
    }
    for (stage, records) in &stage_records {
        assert_eq!(
            json_path_u64(
                &json,
                &format!("/debug/pipeline_summary/stall_cause/{cause}/stage/{stage}/records")
            ),
            *records,
            "pipeline summary stall cause {cause} stage {stage} should count active records"
        );
    }
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        if !stage_records.contains_key(stage) {
            assert_eq!(
                json_path_u64(
                    &json,
                    &format!("/debug/pipeline_summary/stall_cause/{cause}/stage/{stage}/records")
                ),
                0,
                "pipeline summary stall cause {cause} stage {stage} should expose a zero record lane"
            );
        }
    }
}

fn assert_pipeline_trace_stage_activity(stdout: &str) {
    let json: Value = serde_json::from_str(stdout).unwrap();
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    let mut stage_advanced = BTreeMap::<String, u64>::new();
    let mut stage_retired = BTreeMap::<String, u64>::new();
    let mut stage_resource_blocked = BTreeMap::<String, u64>::new();
    for record in trace {
        for advanced in record_array(record, "advanced") {
            let stage = advanced
                .get("source_stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing advanced instruction source stage: {advanced}"));
            let stage = stat_path_segment(stage);
            *stage_advanced.entry(stage.clone()).or_default() += 1;
            if json_record_bool(advanced, "retires") {
                *stage_retired.entry(stage).or_default() += 1;
            }
        }
        for blocked in record_array(record, "resource_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing resource-blocked instruction stage: {blocked}"));
            *stage_resource_blocked
                .entry(stat_path_segment(stage))
                .or_default() += 1;
        }
    }
    assert!(
        !stage_advanced.is_empty(),
        "pipeline trace should preserve per-stage advanced instructions: {trace:?}"
    );
    assert!(
        stage_retired.contains_key("commit"),
        "pipeline trace should expose commit-stage retirement: {trace:?}"
    );
    assert!(
        !stage_resource_blocked.is_empty(),
        "pipeline trace should preserve resource-blocked in-flight instructions: {trace:?}"
    );
    assert!(
        stage_resource_blocked.contains_key("commit"),
        "pipeline trace should expose commit-stage resource blocking: {trace:?}"
    );
    for (stage, advanced) in stage_advanced {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.advanced"),
            "Count",
            advanced,
            "monotonic",
        );
    }
    for (stage, retired) in stage_retired {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.retired"),
            "Count",
            retired,
            "monotonic",
        );
    }
    for (stage, resource_blocked) in stage_resource_blocked {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.stage.{stage}.resource_blocked"),
            "Count",
            resource_blocked,
            "monotonic",
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DataTraceStats {
    records: u64,
    loads: u64,
    stores: u64,
    atomics: u64,
    bytes: u64,
    load_bytes: u64,
    store_bytes: u64,
    atomic_bytes: u64,
}

impl DataTraceStats {
    fn add_record(&mut self, record: &Value) {
        let size = json_record_u64(record, "size");
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(size);
        match json_record_str(record, "kind") {
            "load" => {
                self.loads = self.loads.saturating_add(1);
                self.load_bytes = self.load_bytes.saturating_add(size);
            }
            "store" => {
                self.stores = self.stores.saturating_add(1);
                self.store_bytes = self.store_bytes.saturating_add(size);
            }
            "atomic" => {
                self.atomics = self.atomics.saturating_add(1);
                self.atomic_bytes = self.atomic_bytes.saturating_add(size);
            }
            other => panic!("unexpected data trace kind {other}: {record:?}"),
        }
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("loads", "Count", self.loads),
            ("stores", "Count", self.stores),
            ("atomics", "Count", self.atomics),
            ("bytes", "Byte", self.bytes),
            ("load_bytes", "Byte", self.load_bytes),
            ("store_bytes", "Byte", self.store_bytes),
            ("atomic_bytes", "Byte", self.atomic_bytes),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_data_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, DataTraceStats>::new();
    let mut kinds = BTreeMap::<String, DataTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let kind = json_record_str(record, "kind").to_string();
        cpus.entry(cpu).or_default().add_record(record);
        kinds.entry(kind).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.data_trace.cpu.cpu{cpu}"));
    }
    for (kind, stats) in kinds {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.data_trace.kind.{}", stat_path_segment(&kind)),
        );
    }
}

fn assert_dram_trace_hierarchy_stats(stdout: &str, record: &Value) {
    let kind = record
        .get("kind")
        .and_then(Value::as_str)
        .expect("DRAM trace kind");
    let target = record
        .get("target")
        .and_then(Value::as_u64)
        .expect("DRAM trace target");
    match kind {
        "target" => {
            let prefix = format!("sim.debug.dram_trace.target{target}");
            assert_dram_trace_record_stats(
                stdout,
                &prefix,
                record,
                &[
                    ("accesses", "Count"),
                    ("reads", "Count"),
                    ("writes", "Count"),
                    ("row_hits", "Count"),
                    ("row_misses", "Count"),
                    ("refreshes", "Count"),
                    ("refresh_ticks", "Tick"),
                    ("commands", "Count"),
                    ("turnarounds", "Count"),
                    ("total_ready_latency_ticks", "Tick"),
                    ("max_ready_latency_ticks", "Tick"),
                ],
            );
        }
        "port" => {
            let port = record
                .get("port")
                .and_then(Value::as_u64)
                .expect("DRAM trace port");
            let prefix = format!("sim.debug.dram_trace.target{target}.port{port}");
            assert_dram_trace_record_stats(
                stdout,
                &prefix,
                record,
                &[
                    ("accesses", "Count"),
                    ("reads", "Count"),
                    ("writes", "Count"),
                    ("row_hits", "Count"),
                    ("row_misses", "Count"),
                    ("refreshes", "Count"),
                    ("refresh_ticks", "Tick"),
                    ("commands", "Count"),
                    ("turnarounds", "Count"),
                    ("total_ready_latency_ticks", "Tick"),
                    ("max_ready_latency_ticks", "Tick"),
                ],
            );
        }
        "bank" => {
            let port = record
                .get("port")
                .and_then(Value::as_u64)
                .expect("DRAM trace port");
            let bank = record
                .get("bank")
                .and_then(Value::as_u64)
                .expect("DRAM trace bank");
            let prefix = format!("sim.debug.dram_trace.target{target}.port{port}.bank{bank}");
            assert_dram_trace_record_stats(
                stdout,
                &prefix,
                record,
                &[
                    ("accesses", "Count"),
                    ("read_bytes", "Byte"),
                    ("write_bytes", "Byte"),
                    ("row_hits", "Count"),
                    ("row_misses", "Count"),
                    ("refreshes", "Count"),
                    ("refresh_ticks", "Tick"),
                    ("commands", "Count"),
                    ("total_ready_latency_ticks", "Tick"),
                    ("max_ready_latency_ticks", "Tick"),
                ],
            );
        }
        other => panic!("unexpected DRAM trace kind {other}: {record:?}"),
    }
}

fn assert_dram_low_power_trace_stats(stdout: &str, record: &Value) {
    let kind = record
        .get("kind")
        .and_then(Value::as_str)
        .expect("DRAM trace kind");
    let target = record
        .get("target")
        .and_then(Value::as_u64)
        .expect("DRAM trace target");
    let prefix = match kind {
        "target" => format!("sim.debug.dram_trace.target{target}.low_power"),
        "port" => {
            let port = record
                .get("port")
                .and_then(Value::as_u64)
                .expect("DRAM trace port");
            format!("sim.debug.dram_trace.target{target}.port{port}.low_power")
        }
        "bank" => {
            let port = record
                .get("port")
                .and_then(Value::as_u64)
                .expect("DRAM trace port");
            let bank = record
                .get("bank")
                .and_then(Value::as_u64)
                .expect("DRAM trace bank");
            format!("sim.debug.dram_trace.target{target}.port{port}.bank{bank}.low_power")
        }
        other => panic!("unexpected DRAM trace kind {other}: {record:?}"),
    };
    let low_power = record
        .get("low_power")
        .unwrap_or_else(|| panic!("DRAM trace low_power: {record:?}"));
    for (path, pointer, unit) in dram_low_power_stat_fields() {
        assert_stat(
            stdout,
            &format!("{prefix}.{path}"),
            unit,
            low_power
                .pointer(pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("DRAM trace low_power{pointer}: {record:?}")),
            "monotonic",
        );
    }
}

fn assert_dram_low_power_kind_stats(stdout: &str, trace: &[Value], kind: &str) {
    for (path, pointer, unit) in dram_low_power_stat_fields() {
        assert_stat(
            stdout,
            &format!("sim.debug.dram_trace.{kind}.low_power.{path}"),
            unit,
            debug_low_power_trace_sum(trace, kind, pointer),
            "monotonic",
        );
    }
}

fn dram_low_power_stat_fields() -> [(&'static str, &'static str, &'static str); 8] {
    [
        (
            "active_powerdown.entries",
            "/active_powerdown/entries",
            "Count",
        ),
        ("active_powerdown.ticks", "/active_powerdown/ticks", "Tick"),
        (
            "precharge_powerdown.entries",
            "/precharge_powerdown/entries",
            "Count",
        ),
        (
            "precharge_powerdown.ticks",
            "/precharge_powerdown/ticks",
            "Tick",
        ),
        ("self_refresh.entries", "/self_refresh/entries", "Count"),
        ("self_refresh.ticks", "/self_refresh/ticks", "Tick"),
        ("exits", "/exits", "Count"),
        ("exit_latency_ticks", "/exit_latency_ticks", "Tick"),
    ]
}

fn assert_dram_trace_record_stats(
    stdout: &str,
    prefix: &str,
    record: &Value,
    fields: &[(&str, &str)],
) {
    for (field, unit) in fields {
        assert_stat(
            stdout,
            &format!("{prefix}.{field}"),
            unit,
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("DRAM trace {prefix}.{field}: {record:?}")),
            "monotonic",
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FabricHopStats {
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
}

fn assert_fabric_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut hop_stats = BTreeMap::<(String, u64, u64), FabricHopStats>::new();
    for record in trace {
        let kind = record
            .get("kind")
            .and_then(Value::as_str)
            .expect("fabric trace kind");
        match kind {
            "lane" => assert_fabric_lane_trace_stats(stdout, record),
            "hop" => {
                let link = record
                    .get("link")
                    .and_then(Value::as_str)
                    .expect("fabric hop link")
                    .to_string();
                let virtual_network = record
                    .get("virtual_network")
                    .and_then(Value::as_u64)
                    .expect("fabric hop virtual_network");
                let hop_index = record
                    .get("hop_index")
                    .and_then(Value::as_u64)
                    .expect("fabric hop hop_index");
                let summary = hop_stats
                    .entry((link, virtual_network, hop_index))
                    .or_default();
                summary.transfers = summary.transfers.saturating_add(1);
                summary.bytes = summary
                    .bytes
                    .saturating_add(json_record_u64(record, "bytes"));
                summary.flits = summary
                    .flits
                    .saturating_add(json_record_u64(record, "flits"));
                summary.occupied_ticks = summary
                    .occupied_ticks
                    .saturating_add(json_record_u64(record, "occupied_ticks"));
                let queue_delay_ticks = json_record_u64(record, "queue_delay_ticks");
                summary.queue_delay_ticks =
                    summary.queue_delay_ticks.saturating_add(queue_delay_ticks);
                summary.max_queue_delay_ticks =
                    summary.max_queue_delay_ticks.max(queue_delay_ticks);
                summary.credit_delay_ticks = summary
                    .credit_delay_ticks
                    .saturating_add(json_record_u64(record, "credit_delay_ticks"));
                summary.max_credit_delay_ticks = summary
                    .max_credit_delay_ticks
                    .max(json_record_u64(record, "credit_delay_ticks"));
            }
            other => panic!("unexpected fabric trace kind {other}: {record:?}"),
        }
    }
    for ((link, virtual_network, hop_index), summary) in hop_stats {
        let prefix = format!(
            "sim.debug.fabric_trace.hop.link.{}.vn{virtual_network}.hop{hop_index}",
            stat_path_segment(&link)
        );
        for (suffix, unit, value) in [
            ("transfers", "Count", summary.transfers),
            ("bytes", "Byte", summary.bytes),
            ("flits", "Count", summary.flits),
            ("occupied_ticks", "Tick", summary.occupied_ticks),
            ("queue_delay_ticks", "Tick", summary.queue_delay_ticks),
            (
                "max_queue_delay_ticks",
                "Tick",
                summary.max_queue_delay_ticks,
            ),
            ("credit_delay_ticks", "Tick", summary.credit_delay_ticks),
            (
                "max_credit_delay_ticks",
                "Tick",
                summary.max_credit_delay_ticks,
            ),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
}

fn assert_fabric_lane_trace_stats(stdout: &str, record: &Value) {
    let link = record
        .get("link")
        .and_then(Value::as_str)
        .expect("fabric lane link");
    let virtual_network = record
        .get("virtual_network")
        .and_then(Value::as_u64)
        .expect("fabric lane virtual_network");
    let prefix = format!(
        "sim.debug.fabric_trace.lane.link.{}.vn{virtual_network}",
        stat_path_segment(link)
    );
    for (stat_suffix, field, unit) in [
        ("transfers", "transfer_count", "Count"),
        ("bytes", "byte_count", "Byte"),
        ("flits", "flit_count", "Count"),
        ("occupied_ticks", "occupied_ticks", "Tick"),
        ("queue_delay_ticks", "queue_delay_ticks", "Tick"),
        ("max_queue_delay_ticks", "max_queue_delay_ticks", "Tick"),
        ("credit_delay_ticks", "credit_delay_ticks", "Tick"),
        ("max_credit_delay_ticks", "max_credit_delay_ticks", "Tick"),
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{stat_suffix}"),
            unit,
            json_record_u64(record, field),
            "monotonic",
        );
    }
}

fn fabric_trace_sum(trace: &[Value], kind: &str, field: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| json_record_u64(record, field))
        .sum()
}

fn fabric_trace_max(trace: &[Value], kind: &str, field: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some(kind))
        .map(|record| json_record_u64(record, field))
        .max()
        .unwrap_or(0)
}

fn o3_event_u64s(events: &[Value], field: &str) -> Vec<u64> {
    events
        .iter()
        .map(|event| json_record_u64(event, field))
        .collect()
}

#[derive(Debug)]
struct O3LsqDataLatencyTrace {
    stdout: String,
    memory_system: Option<String>,
    load_event_tick: u64,
    load_response_tick: u64,
    load_latency: u64,
    store_event_tick: u64,
    store_response_tick: u64,
    store_latency: u64,
    event_latency_samples: u64,
    event_latency_sum: u64,
    event_latency_min: u64,
    event_latency_max: u64,
}

fn o3_lsq_data_latency_trace(path: &Path, memory_system: Option<&str>) -> O3LsqDataLatencyTrace {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "260",
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3",
    ]);
    if let Some(memory_system) = memory_system {
        command.args(["--memory-system", memory_system]);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let by_pc = |pc: &str| {
        trace
            .iter()
            .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("missing O3 event at {pc}: {trace:?}"))
    };
    let load = by_pc("0x80000010");
    let store = by_pc("0x80000014");
    let load_event_tick = json_record_u64(load, "tick");
    let load_response_tick = json_record_u64(load, "lsq_data_response_tick");
    let load_latency = json_record_u64(load, "lsq_data_latency_ticks");
    let store_event_tick = json_record_u64(store, "tick");
    let store_response_tick = json_record_u64(store, "lsq_data_response_tick");
    let store_latency = json_record_u64(store, "lsq_data_latency_ticks");

    O3LsqDataLatencyTrace {
        stdout,
        memory_system: json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str)
            .map(str::to_owned),
        load_event_tick,
        load_response_tick,
        load_latency,
        store_event_tick,
        store_response_tick,
        store_latency,
        event_latency_samples: 2,
        event_latency_sum: load_latency + store_latency,
        event_latency_min: load_latency.min(store_latency),
        event_latency_max: load_latency.max(store_latency),
    }
}

const fn latency_average_ticks(total: u64, samples: u64) -> u64 {
    if samples == 0 {
        0
    } else {
        total / samples
    }
}

fn json_record_u64(record: &Value, field: &str) -> u64 {
    record
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing JSON u64 field {field}: {record:?}"))
}

fn json_record_bool(record: &Value, field: &str) -> bool {
    record
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("missing JSON bool field {field}: {record:?}"))
}

fn record_array<'a>(record: &'a Value, field: &str) -> &'a [Value] {
    record
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing JSON array field {field}: {record:?}"))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MemoryTraceStats {
    records: u64,
    requests: BTreeSet<(u64, u64)>,
    routes: BTreeSet<u64>,
    request_agents: BTreeSet<u64>,
    events: BTreeMap<String, u64>,
    response_status: BTreeMap<String, u64>,
    response_latency_ticks: u64,
    max_response_latency_ticks: u64,
}

impl MemoryTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        let request_agent = json_record_u64(record, "request_agent");
        let request = json_record_u64(record, "request");
        let route = json_record_u64(record, "route");
        let kind = json_record_str(record, "kind").to_string();
        self.requests.insert((request_agent, request));
        self.routes.insert(route);
        self.request_agents.insert(request_agent);
        self.events
            .entry(kind)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        if let Some(status) = record.get("response_status").and_then(Value::as_str) {
            self.response_status
                .entry(status.to_string())
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
        }
        if let Some(latency_ticks) = record.get("response_latency_ticks").and_then(Value::as_u64) {
            self.response_latency_ticks = self.response_latency_ticks.saturating_add(latency_ticks);
            self.max_response_latency_ticks = self.max_response_latency_ticks.max(latency_ticks);
        }
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("requests", self.requests.len() as u64),
            ("routes", self.routes.len() as u64),
            ("request_agents", self.request_agents.len() as u64),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                "Count",
                value,
                "monotonic",
            );
        }
        for (kind, value) in &self.events {
            assert_stat(
                stdout,
                &format!("{prefix}.events.{kind}"),
                "Count",
                *value,
                "monotonic",
            );
        }
        for (status, value) in &self.response_status {
            assert_stat(
                stdout,
                &format!("{prefix}.response_status.{status}"),
                "Count",
                *value,
                "monotonic",
            );
        }
        assert_stat(
            stdout,
            &format!("{prefix}.response_latency_ticks"),
            "Tick",
            self.response_latency_ticks,
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.max_response_latency_ticks"),
            "Tick",
            self.max_response_latency_ticks,
            "monotonic",
        );
    }
}

fn assert_memory_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut channels = BTreeMap::<String, MemoryTraceStats>::new();
    let mut routes = BTreeMap::<(String, u64, String), MemoryTraceStats>::new();
    let mut request_agents = BTreeMap::<(String, u64), MemoryTraceStats>::new();
    for record in trace {
        let channel = json_record_str(record, "channel").to_string();
        let route = json_record_u64(record, "route");
        let endpoint = json_record_str(record, "endpoint").to_string();
        let request_agent = json_record_u64(record, "request_agent");
        channels
            .entry(channel.clone())
            .or_default()
            .add_record(record);
        routes
            .entry((channel.clone(), route, endpoint))
            .or_default()
            .add_record(record);
        request_agents
            .entry((channel, request_agent))
            .or_default()
            .add_record(record);
    }
    for (channel, stats) in channels {
        let prefix = format!(
            "sim.debug.memory_trace.channel.{}",
            stat_path_segment(&channel)
        );
        stats.assert_stats(stdout, &prefix);
    }
    for ((channel, route, endpoint), stats) in routes {
        let prefix = format!(
            "sim.debug.memory_trace.channel.{}.route{route}.endpoint.{}",
            stat_path_segment(&channel),
            stat_path_segment(&endpoint)
        );
        stats.assert_stats(stdout, &prefix);
    }
    for ((channel, request_agent), stats) in request_agents {
        let prefix = format!(
            "sim.debug.memory_trace.channel.{}.request_agent.agent{request_agent}",
            stat_path_segment(&channel)
        );
        stats.assert_stats(stdout, &prefix);
    }
}

fn assert_memory_trace_response_latencies(trace: &[Value]) {
    let mut request_sent_ticks = BTreeMap::<(String, u64, u64, u64), u64>::new();
    for record in trace {
        let kind = json_record_str(record, "kind");
        let key = (
            json_record_str(record, "channel").to_string(),
            json_record_u64(record, "route"),
            json_record_u64(record, "request_agent"),
            json_record_u64(record, "request"),
        );
        match kind {
            "request_sent" => {
                request_sent_ticks.insert(key, json_record_u64(record, "tick"));
            }
            "response_arrived" => {
                let sent_tick = request_sent_ticks
                    .get(&key)
                    .unwrap_or_else(|| panic!("missing request_sent record: {record:?}"));
                assert_eq!(
                    json_record_u64(record, "response_latency_ticks"),
                    json_record_u64(record, "tick").saturating_sub(*sent_tick),
                    "record: {record:?}"
                );
            }
            _ => {}
        }
    }
}

fn memory_trace_response_latency_sum(trace: &[Value]) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("response_arrived"))
        .map(|record| json_record_u64(record, "response_latency_ticks"))
        .sum()
}

fn memory_trace_response_latency_max(trace: &[Value]) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("kind").and_then(Value::as_str) == Some("response_arrived"))
        .map(|record| json_record_u64(record, "response_latency_ticks"))
        .max()
        .unwrap_or(0)
}

fn json_record_str<'a>(record: &'a Value, field: &str) -> &'a str {
    record
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing JSON string field {field}: {record:?}"))
}

const CACHE_TRACE_COUNT_FIELDS: &[(&str, &str)] = &[
    ("bank_accepted", "bank.accepted"),
    ("bank_immediate_hits", "bank.immediate_hits"),
    ("bank_scheduled_misses", "bank.scheduled_misses"),
    ("bank_coalesced_misses", "bank.coalesced_misses"),
    ("prefetch_identified", "prefetch.identified"),
    ("prefetch_issued", "prefetch.issued"),
    ("prefetch_useful", "prefetch.useful"),
    ("prefetch_useful_but_miss", "prefetch.useful_but_miss"),
    ("prefetch_unused", "prefetch.unused"),
    ("prefetch_demand_mshr_misses", "prefetch.demand_mshr_misses"),
    ("prefetch_hit_in_cache", "prefetch.hit_in_cache"),
    ("prefetch_hit_in_mshr", "prefetch.hit_in_mshr"),
    (
        "prefetch_hit_in_write_buffer",
        "prefetch.hit_in_write_buffer",
    ),
    ("prefetch_late", "prefetch.late"),
    ("prefetch_span_page", "prefetch.span_page"),
    ("prefetch_useful_span_page", "prefetch.useful_span_page"),
    ("prefetch_in_cache", "prefetch.in_cache"),
    ("prefetch_fills", "prefetch.fills"),
    ("prefetch_queue_enqueued", "prefetch.queue.enqueued"),
    ("prefetch_queue_issued", "prefetch.queue.issued"),
    ("prefetch_queue_dropped", "prefetch.queue.dropped"),
    (
        "prefetch_translation_queue_enqueued",
        "prefetch.translation_queue.enqueued",
    ),
    (
        "prefetch_translation_queue_issued",
        "prefetch.translation_queue.issued",
    ),
    (
        "prefetch_translation_queue_translated",
        "prefetch.translation_queue.translated",
    ),
    (
        "prefetch_translation_queue_dropped",
        "prefetch.translation_queue.dropped",
    ),
];

fn assert_cache_trace_record(
    trace: &[Value],
    hierarchy: &str,
    level: &str,
    json: &Value,
    resource_path: &str,
) {
    let record = trace
        .iter()
        .find(|record| {
            record.get("hierarchy").and_then(Value::as_str) == Some(hierarchy)
                && record.get("level").and_then(Value::as_str) == Some(level)
        })
        .unwrap_or_else(|| panic!("missing cache trace record {hierarchy}.{level}: {trace:?}"));
    for field in [
        "activity",
        "active",
        "cpu_responses",
        "directory_decisions",
        "dram_accesses",
    ] {
        assert_eq!(
            record.get(field),
            json.pointer(&format!("{resource_path}/{field}")),
            "cache trace {hierarchy}.{level}.{field}: {record:?}"
        );
    }
    for (field, _) in CACHE_TRACE_COUNT_FIELDS {
        assert_eq!(
            record.get(field),
            json.pointer(&format!("{resource_path}/{field}")),
            "cache trace {hierarchy}.{level}.{field}: {record:?}"
        );
    }
    for field in ["prefetch_accuracy_ppm", "prefetch_coverage_ppm"] {
        assert_eq!(
            record.get(field),
            json.pointer(&format!("{resource_path}/{field}")),
            "cache trace {hierarchy}.{level}.{field}: {record:?}"
        );
    }
}

fn cache_trace_active_count(trace: &[Value]) -> u64 {
    trace
        .iter()
        .filter(|record| {
            record
                .get("active")
                .and_then(Value::as_u64)
                .is_some_and(|active| active > 0)
        })
        .count() as u64
}

fn cache_trace_sum(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("cache trace {field}"))
        })
        .sum()
}

fn assert_cache_trace_hierarchy_stats(stdout: &str, record: &Value) {
    let hierarchy = record
        .get("hierarchy")
        .and_then(Value::as_str)
        .expect("cache trace hierarchy");
    let level = record
        .get("level")
        .and_then(Value::as_str)
        .expect("cache trace level");
    let prefix = format!("sim.debug.cache_trace.hierarchy.{hierarchy}.{level}");
    for field in [
        "activity",
        "active",
        "cpu_responses",
        "directory_decisions",
        "dram_accesses",
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{field}"),
            "Count",
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("cache trace {hierarchy}.{level}.{field}")),
            "monotonic",
        );
    }
    for (field, stat_suffix) in CACHE_TRACE_COUNT_FIELDS {
        assert_stat(
            stdout,
            &format!("{prefix}.{stat_suffix}"),
            "Count",
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("cache trace {hierarchy}.{level}.{field}")),
            "monotonic",
        );
    }
    for (field, stat_suffix) in [
        ("prefetch_accuracy_ppm", "prefetch.accuracy_ppm"),
        ("prefetch_coverage_ppm", "prefetch.coverage_ppm"),
    ] {
        if let Some(value) = record.get(field).and_then(Value::as_u64) {
            assert_stat(
                stdout,
                &format!("{prefix}.{stat_suffix}"),
                "Ppm",
                value,
                "monotonic",
            );
        }
    }
}

fn json_path_u64(json: &Value, path: &str) -> u64 {
    json.pointer(path)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing JSON u64 path {path}"))
}

fn json_stage_summary_from_path(json: &Value, path: &str) -> [u64; 5] {
    let stage = json
        .pointer(path)
        .unwrap_or_else(|| panic!("missing JSON stage summary path {path}"));
    [
        json_record_u64(stage, "fetch1"),
        json_record_u64(stage, "fetch2"),
        json_record_u64(stage, "decode"),
        json_record_u64(stage, "execute"),
        json_record_u64(stage, "commit"),
    ]
}

#[test]
fn rem6_run_power_debug_flag_emits_activity_power_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-power", &elf);

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
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--debug-flags",
            "Power",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Power".to_string())])
    );
    let trace = json
        .pointer("/debug/power_trace")
        .and_then(Value::as_array)
        .expect("debug power trace array");
    let targets = power_trace_unique_strings(trace, "target");
    let states = power_trace_unique_strings(trace, "state");
    let on_records = power_trace_state_count(trace, "on");
    let residency_ticks = power_trace_sum_u64(trace, "residency_ticks");
    let dynamic_microwatts = power_trace_microwatts(trace, "dynamic_watts");
    let static_microwatts = power_trace_microwatts(trace, "static_watts");
    let total_microwatts = power_trace_microwatts(trace, "total_watts");
    let dynamic_microwatt_ticks = power_trace_microwatt_ticks(trace, "dynamic_watts");
    let static_microwatt_ticks = power_trace_microwatt_ticks(trace, "static_watts");
    let total_microwatt_ticks = power_trace_microwatt_ticks(trace, "total_watts");
    let max_temperature_millicelsius = power_trace_max_millicelsius(trace, "temperature_c");
    let json_text = json.to_string();
    assert!(targets > 0, "trace: {trace:?}");
    assert!(states > 0, "trace: {trace:?}");
    assert!(on_records > 0, "trace: {trace:?}");
    assert!(residency_ticks > 0, "trace: {trace:?}");
    assert!(dynamic_microwatts > 0, "trace: {trace:?}");
    assert!(static_microwatts > 0, "trace: {trace:?}");
    assert!(total_microwatts >= dynamic_microwatts, "trace: {trace:?}");
    assert!(dynamic_microwatt_ticks > 0, "trace: {trace:?}");
    assert!(static_microwatt_ticks > 0, "trace: {trace:?}");
    assert!(
        total_microwatt_ticks >= dynamic_microwatt_ticks,
        "trace: {trace:?}"
    );
    assert!(max_temperature_millicelsius > 0, "trace: {trace:?}");
    for target in [
        "cpu0.core",
        "cpu.instruction_cache",
        "cpu.data_cache",
        "memory.transport",
        "memory.dram",
    ] {
        let record = trace
            .iter()
            .find(|record| record.get("target").and_then(Value::as_str) == Some(target))
            .unwrap_or_else(|| panic!("missing power trace target {target}: {trace:?}"));
        assert_eq!(record.get("state").and_then(Value::as_str), Some("on"));
        assert!(
            record
                .get("residency_ticks")
                .and_then(Value::as_u64)
                .is_some_and(|ticks| ticks > 0),
            "missing residency ticks for {target}: {record:?}"
        );
        assert!(
            record
                .get("dynamic_watts")
                .and_then(Value::as_f64)
                .is_some_and(|watts| watts > 0.0),
            "missing dynamic watts for {target}: {record:?}"
        );
        let target_prefix = power_trace_target_stat_prefix(target);
        assert_stat(
            &json_text,
            &format!("{target_prefix}.records"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.states.on"),
            "Count",
            power_trace_record_state_count(record, "on"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.residency_ticks"),
            "Tick",
            power_trace_record_u64(record, "residency_ticks"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.dynamic_microwatts"),
            "MicroWatt",
            power_trace_record_microwatts(record, "dynamic_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.static_microwatts"),
            "MicroWatt",
            power_trace_record_microwatts(record, "static_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.total_microwatts"),
            "MicroWatt",
            power_trace_record_microwatts(record, "total_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.dynamic_microwatt_ticks"),
            "MicroWattTick",
            power_trace_record_microwatt_ticks(record, "dynamic_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.static_microwatt_ticks"),
            "MicroWattTick",
            power_trace_record_microwatt_ticks(record, "static_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.total_microwatt_ticks"),
            "MicroWattTick",
            power_trace_record_microwatt_ticks(record, "total_watts"),
            "monotonic",
        );
        assert_stat(
            &json_text,
            &format!("{target_prefix}.max_temperature_millicelsius"),
            "MilliCelsius",
            power_trace_record_millicelsius(record, "temperature_c"),
            "monotonic",
        );
    }
    assert_stat(
        &json_text,
        "sim.debug.power_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.targets",
        "Count",
        targets,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.states",
        "Count",
        states,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.states.on",
        "Count",
        on_records,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.residency_ticks",
        "Tick",
        residency_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.dynamic_microwatts",
        "MicroWatt",
        dynamic_microwatts,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.static_microwatts",
        "MicroWatt",
        static_microwatts,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.total_microwatts",
        "MicroWatt",
        total_microwatts,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.dynamic_microwatt_ticks",
        "MicroWattTick",
        dynamic_microwatt_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.static_microwatt_ticks",
        "MicroWattTick",
        static_microwatt_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.total_microwatt_ticks",
        "MicroWattTick",
        total_microwatt_ticks,
        "monotonic",
    );
    assert_stat(
        &json_text,
        "sim.debug.power_trace.max_temperature_millicelsius",
        "MilliCelsius",
        max_temperature_millicelsius,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_real_m5_host_action_trace() {
    let program = riscv64_program(&[
        i_type(21, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_BEGIN),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_RESET_STATS),
        m5op(M5_DUMP_STATS),
        m5op(M5_DUMP_RESET_STATS),
        i_type(21, 0, 0x0, 10, 0x13),
        i_type(3, 0, 0x0, 11, 0x13),
        m5op(M5_WORK_END),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-host-action", &elf);

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
            "--memory-system",
            "direct",
            "--debug-flags",
            "HostAction",
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
        Some(&vec![Value::String("HostAction".to_string())])
    );
    let trace = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(host_action_trace_kind_count(trace, "roi_begin"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "roi_end"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "stats_reset"), 3);
    assert_eq!(host_action_trace_kind_count(trace, "stats_dump"), 3);
    assert_eq!(host_action_trace_kind_count(trace, "stop"), 1);
    assert_eq!(trace.len(), 9);
    assert_dump_reset_trace_order(trace);
    let roi_begin = trace
        .iter()
        .find(|record| record.get("kind").and_then(Value::as_str) == Some("roi_begin"))
        .expect("roi begin trace");
    assert_eq!(
        roi_begin.pointer("/work_id").and_then(Value::as_u64),
        Some(21)
    );
    assert_eq!(
        roi_begin.pointer("/thread_id").and_then(Value::as_u64),
        Some(3)
    );
    let stats_dump = trace
        .iter()
        .find(|record| record.get("kind").and_then(Value::as_str) == Some("stats_dump"))
        .expect("stats dump trace");
    assert!(
        stats_dump
            .pointer("/epoch")
            .and_then(Value::as_u64)
            .is_some_and(|epoch| epoch > 0),
        "stats dump trace: {stats_dump:?}"
    );
    assert!(
        stats_dump
            .pointer("/reset_tick")
            .and_then(Value::as_u64)
            .zip(stats_dump.pointer("/tick").and_then(Value::as_u64))
            .is_some_and(|(reset_tick, tick)| reset_tick <= tick),
        "stats dump trace: {stats_dump:?}"
    );
    assert_eq!(
        trace
            .last()
            .and_then(|record| record.pointer("/code"))
            .and_then(Value::as_i64),
        Some(0)
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.roi_begin",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.roi_end",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stats_resets",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stats_dumps",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stops",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_m5_hypercall_checkpoint_and_switch_trace() {
    let program = riscv64_program(&[
        i_type(0x321, 0, 0x0, 10, 0x13),
        i_type(11, 0, 0x0, 11, 0x13),
        i_type(12, 0, 0x0, 12, 0x13),
        i_type(13, 0, 0x0, 13, 0x13),
        i_type(14, 0, 0x0, 14, 0x13),
        i_type(15, 0, 0x0, 15, 0x13),
        m5op(M5_HYPERCALL),
        m5op(M5_CHECKPOINT),
        m5op(M5_SWITCH_CPU),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-host-action-m5-detail", &elf);

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
            "--memory-system",
            "direct",
            "--debug-flags",
            "HostAction",
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
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(host_action_trace_kind_count(trace, "injected_command"), 0);
    assert_eq!(host_action_trace_kind_count(trace, "guest_host_call"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "checkpoint"), 1);
    assert_eq!(
        host_action_trace_kind_count(trace, "execution_mode_switch"),
        1
    );
    assert_eq!(host_action_trace_kind_count(trace, "stop"), 1);
    assert_eq!(trace.len(), 4);

    let call = host_action_trace_record(trace, "guest_host_call");
    assert_eq!(
        call.pointer("/selector").and_then(Value::as_u64),
        Some(0x321)
    );
    assert_eq!(
        call.pointer("/argument_count").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        call.pointer("/response_status").and_then(Value::as_i64),
        Some(-1)
    );
    let checkpoint = host_action_trace_record(trace, "checkpoint");
    assert_eq!(
        checkpoint.pointer("/label").and_then(Value::as_str),
        Some("gem5-m5-checkpoint")
    );
    assert!(
        checkpoint
            .pointer("/component_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| count >= 2),
        "checkpoint trace: {checkpoint:?}"
    );
    assert!(
        checkpoint
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "checkpoint trace: {checkpoint:?}"
    );
    let switch = host_action_trace_record(trace, "execution_mode_switch");
    assert_eq!(
        switch.pointer("/target").and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        switch.pointer("/mode").and_then(Value::as_str),
        Some("detailed")
    );
    assert_eq!(
        switch
            .pointer("/state_transfer_captured")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        switch
            .pointer("/state_transfer_components")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "switch trace: {switch:?}"
    );
    assert_eq!(
        trace
            .last()
            .and_then(|record| record.pointer("/code"))
            .and_then(Value::as_i64),
        Some(0)
    );

    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.injected_commands",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.guest_host_calls",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.checkpoints",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switches",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.stops",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_checker_quiescence_switch_scope() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(5, 5, 0x0, 6, 0x13),
        i_type(1, 6, 0x0, 7, 0x13),
        m5op(M5_SWITCH_CPU),
        i_type(1, 7, 0x0, 8, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-host-action-checker-switch", &elf);

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
            "--memory-system",
            "direct",
            "--debug-flags",
            "HostAction",
            "--checker-cpu",
            "--m5-switch-cpu-mode",
            "timing",
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
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(
        host_action_trace_kind_count(trace, "execution_mode_switch"),
        2
    );
    let switches = trace
        .iter()
        .filter(|record| {
            record.get("kind").and_then(Value::as_str) == Some("execution_mode_switch")
        })
        .collect::<Vec<_>>();
    let host_switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .expect("host execution-mode switch array");
    assert_eq!(host_switches.len(), switches.len());

    let mut previous_checked = 0;
    for (switch, host_switch) in switches.iter().zip(host_switches) {
        assert_eq!(switch.pointer("/target"), host_switch.pointer("/target"));
        assert_eq!(switch.pointer("/mode"), host_switch.pointer("/mode"));
        assert_eq!(
            switch
                .pointer("/state_transfer/captured")
                .and_then(Value::as_bool),
            Some(true),
            "HostAction switch trace should expose nested state transfer: {switch}"
        );
        let transfer = host_switch
            .pointer("/state_transfer")
            .expect("host switch state transfer");
        for field in [
            "manifest_label",
            "manifest_tick",
            "component_count",
            "chunk_count",
            "payload_bytes",
        ] {
            let pointer = format!("/{field}");
            assert_eq!(
                switch.pointer(&format!("/state_transfer{pointer}")),
                transfer.pointer(&pointer),
                "state transfer field {field}: switch trace {switch}; host switch {host_switch}"
            );
        }
        let quiescence = switch
            .pointer("/state_transfer/quiescence_gate")
            .expect("HostAction switch trace should expose nested quiescence gate");
        let host_quiescence = transfer
            .pointer("/quiescence_gate")
            .expect("host switch quiescence gate");
        for field in [
            "validated",
            "target",
            "captured_component_count",
            "captured_chunk_count",
            "captured_payload_bytes",
        ] {
            let pointer = format!("/{field}");
            assert_eq!(
                quiescence.pointer(&pointer),
                host_quiescence.pointer(&pointer),
                "quiescence field {field}: switch trace {switch}; host switch {host_switch}"
            );
        }
        let checker = quiescence
            .pointer("/checker")
            .expect("HostAction switch trace should expose checker quiescence");
        let host_checker = host_quiescence
            .pointer("/checker")
            .expect("host switch checker quiescence");
        assert_eq!(
            checker.pointer("/checked_instructions"),
            host_checker.pointer("/checked_instructions")
        );
        assert_eq!(
            checker.pointer("/mismatches"),
            host_checker.pointer("/mismatches")
        );
        let checked = checker
            .pointer("/checked_instructions")
            .and_then(Value::as_u64)
            .expect("checker checked instructions");
        assert!(
            checked > previous_checked,
            "checker quiescence should advance across switches: {switches:?}"
        );
        previous_checked = checked;
    }

    assert_stat(
        &stdout,
        "sim.host_actions.execution_mode_switch_quiescence.checker.checked_instructions",
        "Count",
        previous_checked,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.host_actions.execution_mode_switch_quiescence.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.execution_mode_switches",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_host_action_debug_flag_emits_scheduled_checkpoint_restore_trace() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-host-action-checkpoint-restore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "HostAction",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
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
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .expect("debug host action trace array");
    assert!(
        host_action_trace_ticks_are_ordered(trace),
        "trace: {trace:?}"
    );
    assert_eq!(host_action_trace_kind_count(trace, "checkpoint"), 1);
    assert_eq!(host_action_trace_kind_count(trace, "checkpoint_restore"), 1);
    assert_eq!(
        host_action_trace_kind_count(trace, "execution_mode_switch"),
        1
    );
    assert_eq!(host_action_trace_kind_count(trace, "stop"), 1);

    let checkpoint = host_action_trace_record(trace, "checkpoint");
    let restore = host_action_trace_record(trace, "checkpoint_restore");
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert!(
        host_action_trace_tick(restore) > host_action_trace_tick(checkpoint),
        "checkpoint trace: {checkpoint:?}; restore trace: {restore:?}"
    );
    assert_eq!(
        restore.pointer("/manifest_tick").and_then(Value::as_u64),
        checkpoint.pointer("/manifest_tick").and_then(Value::as_u64)
    );
    for field in ["component_count", "chunk_count", "payload_bytes"] {
        let field_pointer = format!("/{field}");
        let checkpoint_value = checkpoint
            .pointer(&field_pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("checkpoint {field}: {checkpoint:?}"));
        let restore_value = restore
            .pointer(&field_pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("checkpoint restore {field}: {restore:?}"));
        assert!(restore_value > 0, "checkpoint restore {field}: {restore:?}");
        assert_eq!(
            restore_value, checkpoint_value,
            "restored manifest {field} should match the baseline checkpoint"
        );
    }
    let authority = restore
        .pointer("/execution_mode_authority")
        .unwrap_or_else(|| panic!("checkpoint restore trace should expose authority: {restore}"));
    for (path, expected) in [
        ("/present_manifests", 1),
        ("/cleared_manifests", 0),
        ("/decode_errors", 0),
        ("/targets", 1),
        ("/mode/functional", 0),
        ("/mode/timing", 0),
        ("/mode/detailed", 1),
        ("/target/cpu0/mode/functional", 0),
        ("/target/cpu0/mode/timing", 0),
        ("/target/cpu0/mode/detailed", 1),
    ] {
        assert_eq!(
            authority.pointer(path).and_then(Value::as_u64),
            Some(expected),
            "checkpoint restore authority path {path}: {authority}"
        );
    }

    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.checkpoints",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.host_action_trace.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
    for (path, value) in [
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.manifests",
            1,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.cleared_manifests",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.decode_errors",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.targets",
            1,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.functional",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.timing",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
            1,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.functional",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.timing",
            0,
        ),
        (
            "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
            1,
        ),
    ] {
        assert_stat(&stdout, path, "Count", value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_emits_detailed_runtime_trace() {
    let path = detailed_o3_runtime_debug_binary("debug-flags-o3-detailed-runtime");

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
            "--debug-flags",
            "O3",
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
        Some(&vec![Value::String("O3".to_string())])
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert!(
        record
            .pointer("/checkpoint_restore_labels")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "restore labels: {record}"
    );

    for (field, value) in [
        ("cpu", 0),
        ("stats_epoch", 0),
        ("stats_reset_tick", 0),
        ("checkpoint_restore_count", 0),
        ("instructions", 6),
        ("rob_allocations", 6),
        ("rob_commits", 6),
        ("rename_writes", 4),
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("store_load_forwarding_candidates", 0),
        ("store_load_forwarding_matches", 0),
        ("fu_latency_instructions", 0),
        ("fu_latency_cycles", 0),
        ("max_rob_occupancy", 1),
        ("max_lsq_occupancy", 1),
        ("rename_map_entries", 3),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 trace field {field}"
        );
    }
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 6);
    assert_eq!(
        o3_event_u64s(events, "rob_occupancy"),
        vec![1, 1, 1, 1, 1, 1]
    );
    assert_eq!(
        o3_event_u64s(events, "lsq_occupancy"),
        vec![0, 0, 0, 1, 1, 0]
    );
    assert_eq!(
        o3_event_u64s(events, "rename_map_entries"),
        vec![1, 1, 2, 3, 3, 3]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| json_record_u64(event, "iew_dependency_producers"))
            .collect::<Vec<_>>(),
        vec![0, 1, 0, 1, 1, 0]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| json_record_u64(event, "iew_dependency_consumers"))
            .collect::<Vec<_>>(),
        vec![0, 1, 0, 1, 2, 0]
    );
    assert_o3_event(&events[0], 0, "0x80000004", 1, 0, 0, false);
    assert_o3_event(&events[1], 1, "0x80000008", 1, 0, 0, false);
    assert_o3_event(&events[2], 2, "0x8000000c", 1, 0, 0, false);
    assert_o3_event(&events[3], 3, "0x80000010", 1, 1, 0, false);
    assert_o3_event(&events[4], 4, "0x80000014", 0, 0, 1, false);
    assert_o3_event(&events[5], 5, "0x80000018", 0, 0, 0, true);
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    assert_eq!(
        json_record_u64(event_summary, "records"),
        events.len() as u64
    );
    assert_eq!(
        json_record_u64(event_summary, "first_tick"),
        json_record_u64(&events[0], "tick")
    );
    assert_eq!(
        json_record_u64(event_summary, "last_tick"),
        json_record_u64(&events[events.len() - 1], "tick")
    );
    assert_eq!(
        json_record_u64(event_summary, "span_ticks"),
        json_record_u64(&events[events.len() - 1], "tick") - json_record_u64(&events[0], "tick")
    );
    assert_eq!(
        json_record_u64(event_summary, "max_rob_occupancy"),
        o3_event_u64s(events, "rob_occupancy")
            .into_iter()
            .max()
            .unwrap()
    );
    assert_eq!(
        json_record_u64(event_summary, "max_lsq_occupancy"),
        o3_event_u64s(events, "lsq_occupancy")
            .into_iter()
            .max()
            .unwrap()
    );
    assert_eq!(
        json_record_u64(event_summary, "max_rename_map_entries"),
        o3_event_u64s(events, "rename_map_entries")
            .into_iter()
            .max()
            .unwrap()
    );
    let system_events = events
        .iter()
        .filter(|event| event.get("system_event").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    assert_eq!(
        json_record_u64(event_summary, "system_events"),
        system_events
    );
    let rob_allocations = events
        .iter()
        .filter(|event| event.get("rob_allocated").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    let rob_commits = events
        .iter()
        .filter(|event| event.get("rob_committed").and_then(Value::as_bool) == Some(true))
        .count() as u64;
    let rename_writes = events
        .iter()
        .map(|event| json_record_u64(event, "rename_writes"))
        .sum::<u64>();
    let lsq_loads = events
        .iter()
        .map(|event| json_record_u64(event, "lsq_loads"))
        .sum::<u64>();
    let lsq_stores = events
        .iter()
        .map(|event| json_record_u64(event, "lsq_stores"))
        .sum::<u64>();
    assert_eq!(
        json_record_u64(event_summary, "rob_allocations"),
        rob_allocations
    );
    assert_eq!(json_record_u64(event_summary, "rob_commits"), rob_commits);
    assert_eq!(
        json_record_u64(event_summary, "rename_writes"),
        rename_writes
    );
    let event_summary_rob = event_summary
        .pointer("/rob")
        .expect("O3 event summary should include a nested ROB matrix");
    assert_eq!(
        json_record_u64(event_summary_rob, "allocations"),
        rob_allocations
    );
    assert_eq!(json_record_u64(event_summary_rob, "commits"), rob_commits);
    assert_eq!(
        json_record_u64(event_summary_rob, "max_occupancy"),
        json_record_u64(event_summary, "max_rob_occupancy")
    );
    let event_summary_rename = event_summary
        .pointer("/rename")
        .expect("O3 event summary should include a nested rename matrix");
    assert_eq!(
        json_record_u64(event_summary_rename, "writes"),
        rename_writes
    );
    assert_eq!(
        json_record_u64(event_summary_rename, "map_entries"),
        json_record_u64(event_summary, "max_rename_map_entries")
    );
    assert_eq!(json_record_u64(event_summary, "lsq_loads"), lsq_loads);
    assert_eq!(json_record_u64(event_summary, "lsq_stores"), lsq_stores);
    assert_eq!(
        json_record_u64(event_summary, "lsq_operation_load"),
        events
            .iter()
            .filter(|event| event.get("lsq_operation").and_then(Value::as_str) == Some("load"))
            .count() as u64
    );
    assert_eq!(
        json_record_u64(event_summary, "lsq_operation_store"),
        events
            .iter()
            .filter(|event| event.get("lsq_operation").and_then(Value::as_str) == Some("store"))
            .count() as u64
    );
    let event_summary_iq = event_summary
        .pointer("/iq")
        .expect("O3 event summary should include a nested IQ matrix");
    assert_eq!(
        json_record_u64(event_summary_iq, "insts_issued"),
        events.len() as u64
    );
    assert_eq!(
        json_record_u64(event_summary_iq, "mem_insts_issued"),
        lsq_loads + lsq_stores
    );
    assert_eq!(json_record_u64(event_summary_iq, "branch_insts_issued"), 0);
    let event_summary_issued_inst_type = event_summary_iq
        .pointer("/issued_inst_type")
        .expect("O3 event summary IQ matrix should include issued-inst-type lanes");
    assert_eq!(
        json_record_u64(event_summary_issued_inst_type, "mem_read"),
        lsq_loads
    );
    assert_eq!(
        json_record_u64(event_summary_issued_inst_type, "mem_write"),
        lsq_stores
    );
    assert_eq!(
        json_record_u64(event_summary_issued_inst_type, "int_mul"),
        0
    );
    assert_eq!(
        json_record_u64(event_summary_issued_inst_type, "vector_float_misc"),
        0
    );
    let event_summary_commit = event_summary
        .pointer("/commit")
        .expect("O3 event summary should include a nested commit matrix");
    assert_eq!(
        json_record_u64(event_summary_commit, "branch_mispredicts"),
        0
    );
    let event_summary_committed_inst_type = event_summary_commit
        .pointer("/committed_inst_type")
        .expect("O3 event summary commit matrix should include committed-inst-type lanes");
    assert_eq!(
        json_record_u64(event_summary_committed_inst_type, "mem_read"),
        lsq_loads
    );
    assert_eq!(
        json_record_u64(event_summary_committed_inst_type, "mem_write"),
        lsq_stores
    );
    assert_eq!(
        json_record_u64(event_summary_committed_inst_type, "int_mul"),
        0
    );
    assert_eq!(
        json_record_u64(event_summary_committed_inst_type, "vector_float_misc"),
        0
    );
    let iew_dependency_producers = events
        .iter()
        .map(|event| json_record_u64(event, "iew_dependency_producers"))
        .sum::<u64>();
    let iew_dependency_consumers = events
        .iter()
        .map(|event| json_record_u64(event, "iew_dependency_consumers"))
        .sum::<u64>();
    let event_summary_iew = event_summary
        .pointer("/iew")
        .expect("O3 event summary IEW dependency matrix");
    assert_eq!(
        json_record_u64(event_summary_iew, "producer_inst"),
        iew_dependency_producers
    );
    assert_eq!(
        json_record_u64(event_summary_iew, "consumer_inst"),
        iew_dependency_consumers
    );
    assert_eq!(
        json_record_u64(event_summary_iew, "producer_consumer_fanout_ppm"),
        750_000
    );
    assert_eq!(
        event_summary_iew
            .pointer("/dependency/producer")
            .and_then(Value::as_u64),
        Some(iew_dependency_producers),
        "O3 event summary IEW dependency producer lane: {event_summary_iew}"
    );
    assert_eq!(
        event_summary_iew
            .pointer("/dependency/consumer")
            .and_then(Value::as_u64),
        Some(iew_dependency_consumers),
        "O3 event summary IEW dependency consumer lane: {event_summary_iew}"
    );
    let event_summary_branch_event = event_summary
        .pointer("/branch_event")
        .expect("O3 straight-line event summary should include a branch-event zero object");
    for field in [
        "branches",
        "taken",
        "not_taken",
        "predicted_taken",
        "predicted_not_taken",
        "predicted_targets",
        "predicted_target_matches",
        "predicted_target_mismatches",
        "resolved_targets",
        "mispredictions",
        "squashes",
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_event, field),
            0,
            "O3 straight-line event summary branch-event field {field}"
        );
    }
    for path in [
        "/kind/direct_conditional",
        "/kind/direct_unconditional",
        "/misprediction_kind/direct_conditional",
        "/misprediction_kind/direct_unconditional",
        "/squash_kind/direct_conditional",
        "/squash_kind/direct_unconditional",
    ] {
        assert_eq!(
            event_summary_branch_event
                .pointer(path)
                .and_then(Value::as_u64),
            Some(0),
            "O3 straight-line event summary branch-event path {path}: {event_summary_branch_event}"
        );
    }
    assert_eq!(json_record_str(&events[3], "lsq_operation"), "load");
    assert_eq!(json_record_str(&events[4], "lsq_operation"), "store");

    assert_stat(&stdout, "sim.debug.flags", "Count", 1, "constant");
    for (path, unit, value) in [
        ("sim.debug.trace.records", "Count", 1),
        ("sim.debug.trace.categories", "Count", 1),
        ("sim.debug.trace.active_flags", "Count", 1),
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.stats_epoch", "Count", 0),
        ("sim.debug.o3_trace.stats_reset_tick", "Tick", 0),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 0),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 0),
        ("sim.debug.o3_trace.checkpoint_restore_tick", "Tick", 0),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            0,
        ),
        ("sim.debug.o3_trace.instructions", "Count", 6),
        ("sim.debug.o3_trace.rob_allocations", "Count", 6),
        ("sim.debug.o3_trace.rob_commits", "Count", 6),
        ("sim.debug.o3_trace.rename_writes", "Count", 4),
        ("sim.debug.o3_trace.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.lsq_stores", "Count", 1),
        (
            "sim.debug.o3_trace.store_load_forwarding_candidates",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.store_load_forwarding_matches",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 0),
        ("sim.debug.o3_trace.fu_latency_cycles", "Cycle", 0),
        ("sim.debug.o3_trace.max_rob_occupancy", "Count", 1),
        ("sim.debug.o3_trace.max_lsq_occupancy", "Count", 1),
        ("sim.debug.o3_trace.rename_map_entries", "Count", 3),
        ("sim.debug.o3_trace.event.records", "Count", 6),
        ("sim.debug.o3_trace.event.max_rob_occupancy", "Count", 1),
        ("sim.debug.o3_trace.event.max_lsq_occupancy", "Count", 1),
        (
            "sim.debug.o3_trace.event.max_rename_map_entries",
            "Count",
            3,
        ),
        ("sim.debug.o3_trace.event.rob_allocations", "Count", 6),
        ("sim.debug.o3_trace.event.rob_commits", "Count", 6),
        ("sim.debug.o3_trace.event.rename_writes", "Count", 4),
        (
            "sim.debug.o3_trace.event.iew_dependency_producers",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.iew_dependency_consumers",
            "Count",
            4,
        ),
        ("sim.debug.o3_trace.event.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_stores", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_operation.load", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 1),
        ("sim.debug.o3_trace.event.fu_latency_cycles", "Cycle", 0),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_marks_lsq_data_response_latency() {
    let path = detailed_o3_runtime_debug_binary("debug-flags-o3-lsq-data-latency");
    let direct = o3_lsq_data_latency_trace(&path, Some("direct"));
    let cache = o3_lsq_data_latency_trace(&path, None);

    assert_eq!(direct.memory_system.as_deref(), Some("direct"));
    assert_eq!(cache.memory_system.as_deref(), Some("cache-fabric-dram"));
    for trace in [&direct, &cache] {
        assert!(trace.load_latency > 0, "{trace:?}");
        assert!(trace.store_latency > 0, "{trace:?}");
        assert!(
            trace.load_response_tick > trace.load_event_tick,
            "{trace:?}"
        );
        assert!(
            trace.store_response_tick > trace.store_event_tick,
            "{trace:?}"
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_data_latency_ticks",
            "Tick",
            trace.event_latency_sum,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_data_latency_samples",
            "Count",
            trace.event_latency_samples,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_data_latency_max_ticks",
            "Tick",
            trace.event_latency_max,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_data_latency_min_ticks",
            "Tick",
            trace.event_latency_min,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_data_latency_avg_ticks",
            "Tick",
            latency_average_ticks(trace.event_latency_sum, trace.event_latency_samples),
            "monotonic",
        );
        let json: Value = serde_json::from_str(&trace.stdout).unwrap();
        let event_summary = json
            .pointer("/debug/o3_trace/0/event_summary")
            .expect("O3 trace event summary should be embedded with the trace record");
        let summary_latency = event_summary
            .pointer("/lsq_data_latency")
            .expect("O3 event summary LSQ data latency aggregate");
        assert_eq!(
            json_record_u64(summary_latency, "samples"),
            trace.event_latency_samples
        );
        assert_eq!(
            json_record_u64(summary_latency, "ticks"),
            trace.event_latency_sum
        );
        assert_eq!(
            json_record_u64(summary_latency, "max_ticks"),
            trace.event_latency_max
        );
        assert_eq!(
            json_record_u64(summary_latency, "min_ticks"),
            trace.event_latency_min
        );
        assert_eq!(
            json_record_u64(summary_latency, "avg_ticks"),
            latency_average_ticks(trace.event_latency_sum, trace.event_latency_samples)
        );
        for (operation, latency) in [("load", trace.load_latency), ("store", trace.store_latency)] {
            let operation_summary = event_summary
                .pointer(&format!("/lsq_operation/{operation}"))
                .unwrap_or_else(|| {
                    panic!("missing event summary LSQ operation {operation}: {event_summary:?}")
                });
            assert_eq!(
                json_record_u64(operation_summary, "count"),
                1,
                "event summary LSQ operation {operation} count"
            );
            let operation_latency = operation_summary
                .pointer("/latency")
                .unwrap_or_else(|| panic!("missing {operation} latency: {operation_summary:?}"));
            assert_eq!(
                json_record_u64(operation_latency, "samples"),
                1,
                "event summary LSQ operation {operation} latency samples"
            );
            assert_eq!(
                json_record_u64(operation_latency, "ticks"),
                latency,
                "event summary LSQ operation {operation} latency ticks"
            );
            assert_eq!(
                json_record_u64(operation_latency, "max_ticks"),
                latency,
                "event summary LSQ operation {operation} max latency"
            );
            assert_eq!(
                json_record_u64(operation_latency, "min_ticks"),
                latency,
                "event summary LSQ operation {operation} min latency"
            );
            assert_eq!(
                json_record_u64(operation_latency, "avg_ticks"),
                latency,
                "event summary LSQ operation {operation} avg latency"
            );
        }
        let absent_operation = event_summary
            .pointer("/lsq_operation/load_reserved")
            .expect("event summary LSQ matrix should include load_reserved zero lane");
        assert_eq!(json_record_u64(absent_operation, "count"), 0);
        let absent_latency = absent_operation
            .pointer("/latency")
            .expect("event summary load_reserved latency lane");
        assert_eq!(json_record_u64(absent_latency, "samples"), 0);
        assert_eq!(json_record_u64(absent_latency, "ticks"), 0);
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.load_latency_ticks",
            "Tick",
            trace.load_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.load_latency_max_ticks",
            "Tick",
            trace.load_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.load_latency_min_ticks",
            "Tick",
            trace.load_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.load_latency_avg_ticks",
            "Tick",
            trace.load_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.store_latency_ticks",
            "Tick",
            trace.store_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.store_latency_max_ticks",
            "Tick",
            trace.store_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.store_latency_min_ticks",
            "Tick",
            trace.store_latency,
            "monotonic",
        );
        assert_stat(
            &trace.stdout,
            "sim.debug.o3_trace.event.lsq_operation.store_latency_avg_ticks",
            "Tick",
            trace.store_latency,
            "monotonic",
        );
    }
    assert!(
        cache.event_latency_sum >= direct.event_latency_sum,
        "cache-backed O3 LSQ latency should include at least the direct-path latency: direct={direct:?}, cache={cache:?}"
    );
}

#[test]
fn rem6_run_o3_debug_flag_marks_m5_reset_stats_scope() {
    let path = detailed_o3_reset_stats_debug_binary("debug-flags-o3-reset-stats-scope");

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
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:4",
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
        Some("5a000000")
    );
    let reset = json
        .pointer("/host_actions/stats_resets/0")
        .unwrap_or_else(|| panic!("missing host stats reset: {json}"));
    let reset_epoch = json_record_u64(reset, "epoch");
    let reset_tick = json_record_u64(reset, "tick");
    assert_eq!(reset_epoch, 1);

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "stats_epoch"), reset_epoch);
    assert_eq!(json_record_u64(record, "stats_reset_tick"), reset_tick);
    assert_eq!(json_record_u64(record, "instructions"), 2);
    assert_eq!(json_record_u64(record, "lsq_loads"), 1);
    assert_eq!(json_record_u64(record, "lsq_stores"), 0);

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 2);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(event_pcs, ["0x80000018", "0x8000001c"]);
    assert!(!event_pcs.contains(&"0x80000010"));
    assert_eq!(json_record_str(&events[0], "lsq_operation"), "load");

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.stats_epoch", "Count", reset_epoch),
        ("sim.debug.o3_trace.stats_reset_tick", "Tick", reset_tick),
        ("sim.debug.o3_trace.instructions", "Count", 2),
        ("sim.debug.o3_trace.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.lsq_stores", "Count", 0),
        ("sim.debug.o3_trace.event.records", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_stores", "Count", 0),
        ("sim.debug.o3_trace.event.lsq_operation.load", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 0),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_marks_checkpoint_restore_replay_scope() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-checkpoint-restore-scope");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--dump-memory",
            "0x800002c0:8",
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
        Some(&vec![Value::String("O3".to_string())])
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000000000000")
    );
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .unwrap_or_else(|| panic!("missing host checkpoint: {json}"));
    let checkpoint_payload_bytes = json_record_u64(checkpoint, "payload_bytes");
    assert_eq!(
        checkpoint.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert!(
        checkpoint_payload_bytes > 0,
        "checkpoint payload: {checkpoint}"
    );
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing host checkpoint restore: {json}"));
    let restore_tick = json_record_u64(restore, "tick");
    let restored_manifest_tick = json_record_u64(restore, "manifest_tick");
    let restored_payload_bytes = json_record_u64(restore, "payload_bytes");
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    let restore_execution_modes = restore
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .expect("restored checkpoint execution-mode authority");
    assert_eq!(
        restore_execution_modes.len(),
        1,
        "restored checkpoint should decode one execution-mode authority: {restore_execution_modes:?}"
    );
    assert_eq!(
        restore_execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        restore_execution_modes[0]
            .pointer("/mode")
            .and_then(Value::as_str),
        Some("detailed")
    );
    assert!(restored_payload_bytes > 0, "restore payload: {restore}");
    assert_eq!(restored_payload_bytes, checkpoint_payload_bytes);

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 1);
    let labels = record
        .pointer("/checkpoint_restore_labels")
        .and_then(Value::as_array)
        .expect("O3 restore label array");
    assert_eq!(
        labels
            .iter()
            .map(|label| label.as_str().expect("restore label string"))
            .collect::<Vec<_>>(),
        ["debug-baseline"]
    );
    assert_eq!(
        record
            .get("checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_tick"),
        restore_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_manifest_tick"),
        restored_manifest_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_payload_bytes"),
        restored_payload_bytes
    );
    let restore_scope = record
        .pointer("/checkpoint_restore")
        .unwrap_or_else(|| panic!("O3 trace should expose structured restore scope: {record}"));
    assert_eq!(
        restore_scope.pointer("/count").and_then(Value::as_u64),
        Some(1)
    );
    let scope_labels = restore_scope
        .pointer("/labels")
        .and_then(Value::as_array)
        .expect("structured O3 restore label array")
        .iter()
        .map(|label| label.as_str().expect("restore label string"))
        .collect::<Vec<_>>();
    assert_eq!(scope_labels, ["debug-baseline"]);
    assert_eq!(
        restore_scope
            .pointer("/latest_label")
            .and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert_eq!(
        restore_scope
            .pointer("/latest_tick")
            .and_then(Value::as_u64),
        Some(restore_tick)
    );
    assert_eq!(
        restore_scope
            .pointer("/latest_manifest_tick")
            .and_then(Value::as_u64),
        Some(restored_manifest_tick)
    );
    assert_eq!(
        restore_scope
            .pointer("/latest_payload_bytes")
            .and_then(Value::as_u64),
        Some(restored_payload_bytes)
    );
    let authority = restore_scope
        .pointer("/execution_mode_authority")
        .unwrap_or_else(|| {
            panic!("O3 restore scope should expose execution-mode authority: {restore_scope}")
        });
    for (path, expected) in [
        ("/present_manifests", 1),
        ("/cleared_manifests", 0),
        ("/decode_errors", 0),
        ("/targets", 1),
        ("/mode/functional", 0),
        ("/mode/timing", 0),
        ("/mode/detailed", 1),
        ("/target/cpu0/mode/functional", 0),
        ("/target/cpu0/mode/timing", 0),
        ("/target/cpu0/mode/detailed", 1),
    ] {
        assert_eq!(
            authority.pointer(path).and_then(Value::as_u64),
            Some(expected),
            "authority path {path}: {authority}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(!events.is_empty(), "restored O3 replay events: {record}");
    assert!(
        events
            .iter()
            .all(|event| json_record_u64(event, "tick") > restore_tick),
        "O3 trace should only include post-restore replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000070"
                && json_record_str(event, "lsq_operation") == "store"
                && json_record_str(event, "lsq_store_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000074"
                && json_record_str(event, "lsq_operation") == "load"
                && json_record_str(event, "lsq_load_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 1),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            restored_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.manifests",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.cleared_manifests",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.decode_errors",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
            "Count",
            1,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_counts_multiple_checkpoint_restore_scopes() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-multi-checkpoint-restore-scope");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--host-restore-checkpoint",
            "190:debug-baseline",
            "--dump-memory",
            "0x800002c0:8",
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
        Some("5a00000000000000")
    );
    assert_eq!(
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let restores = json
        .pointer("/host_actions/checkpoint_restores")
        .and_then(Value::as_array)
        .expect("host checkpoint restore array");
    assert_eq!(restores.len(), 2);
    let latest_restore = &restores[1];
    let latest_restore_tick = json_record_u64(latest_restore, "tick");
    let latest_manifest_tick = json_record_u64(latest_restore, "manifest_tick");
    let latest_payload_bytes = json_record_u64(latest_restore, "payload_bytes");
    assert_eq!(
        latest_restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert!(
        latest_payload_bytes > 0,
        "restore payload: {latest_restore}"
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 2);
    assert_eq!(
        record
            .get("checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("debug-baseline")
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_tick"),
        latest_restore_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_manifest_tick"),
        latest_manifest_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_payload_bytes"),
        latest_payload_bytes
    );

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(!events.is_empty(), "restored O3 replay events: {record}");
    assert!(
        events
            .iter()
            .all(|event| json_record_u64(event, "tick") > latest_restore_tick),
        "O3 trace should only include events replayed after the latest restore: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000070"
                && json_record_str(event, "lsq_operation") == "store"
                && json_record_str(event, "lsq_store_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000074"
                && json_record_str(event, "lsq_operation") == "load"
                && json_record_str(event, "lsq_load_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 2),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 1),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            latest_restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            latest_payload_bytes,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_tracks_distinct_checkpoint_restore_labels() {
    let path = detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-distinct-restore-labels");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--host-checkpoint",
            "100:debug-replayed",
            "--host-restore-checkpoint",
            "190:debug-replayed",
            "--dump-memory",
            "0x800002c0:8",
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
        Some("5a00000000000000")
    );

    let checkpoints = json
        .pointer("/host_actions/checkpoints")
        .and_then(Value::as_array)
        .expect("host checkpoint array");
    assert_eq!(checkpoints.len(), 2);
    let checkpoint_labels = checkpoints
        .iter()
        .map(|checkpoint| {
            checkpoint
                .pointer("/label")
                .and_then(Value::as_str)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(checkpoint_labels, ["debug-baseline", "debug-replayed"]);

    let restores = json
        .pointer("/host_actions/checkpoint_restores")
        .and_then(Value::as_array)
        .expect("host checkpoint restore array");
    assert_eq!(restores.len(), 2);
    let restore_labels = restores
        .iter()
        .map(|restore| restore.pointer("/label").and_then(Value::as_str).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(restore_labels, ["debug-baseline", "debug-replayed"]);
    let latest_restore = &restores[1];
    let latest_restore_tick = json_record_u64(latest_restore, "tick");
    let latest_manifest_tick = json_record_u64(latest_restore, "manifest_tick");
    let latest_payload_bytes = json_record_u64(latest_restore, "payload_bytes");
    assert!(
        latest_payload_bytes > 0,
        "restore payload: {latest_restore}"
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 2);
    let trace_labels = record
        .pointer("/checkpoint_restore_labels")
        .and_then(Value::as_array)
        .expect("O3 restore label array")
        .iter()
        .map(|label| label.as_str().expect("restore label string"))
        .collect::<Vec<_>>();
    assert_eq!(trace_labels, ["debug-baseline", "debug-replayed"]);
    assert_eq!(
        record
            .get("checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("debug-replayed")
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_tick"),
        latest_restore_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_manifest_tick"),
        latest_manifest_tick
    );
    assert_eq!(
        json_record_u64(record, "checkpoint_restore_payload_bytes"),
        latest_payload_bytes
    );

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(!events.is_empty(), "restored O3 replay events: {record}");
    assert!(
        events
            .iter()
            .all(|event| json_record_u64(event, "tick") > latest_restore_tick),
        "O3 trace should only include events replayed after debug-replayed restore: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000070"
                && json_record_str(event, "lsq_operation") == "store"
                && json_record_str(event, "lsq_store_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );
    assert!(
        events.iter().any(|event| {
            json_record_str(event, "pc") == "0x80000074"
                && json_record_str(event, "lsq_operation") == "load"
                && json_record_str(event, "lsq_load_address") == "0x800002c0"
        }),
        "restored O3 replay events: {events:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 2),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 1),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            latest_restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            latest_payload_bytes,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_scopes_multicore_checkpoint_restore_traces() {
    let path =
        detailed_o3_scheduled_restore_debug_binary("debug-flags-o3-multicore-checkpoint-restore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "8:debug-baseline",
            "--host-restore-checkpoint",
            "70:debug-baseline",
            "--dump-memory",
            "0x800002c0:8",
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
        Some("5a00000000000000")
    );
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("host checkpoint restore");
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("debug-baseline")
    );
    let restore_tick = json_record_u64(restore, "tick");
    let restored_manifest_tick = json_record_u64(restore, "manifest_tick");
    let restored_payload_bytes = json_record_u64(restore, "payload_bytes");
    assert!(restored_payload_bytes > 0, "restore payload: {restore}");
    let restore_execution_modes = restore
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .expect("host restore execution-mode authority");
    let mut restore_mode_counts = BTreeMap::<&str, u64>::new();
    let mut restore_target_mode_counts = BTreeMap::<(&str, &str), u64>::new();
    for execution_mode in restore_execution_modes {
        let target = execution_mode
            .pointer("/target")
            .and_then(Value::as_str)
            .expect("restore authority target");
        let mode = execution_mode
            .pointer("/mode")
            .and_then(Value::as_str)
            .expect("restore authority mode");
        *restore_mode_counts.entry(mode).or_default() += 1;
        *restore_target_mode_counts
            .entry((target, mode))
            .or_default() += 1;
    }

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 2, "multicore O3 restore trace: {trace:?}");
    for (record, cpu) in trace.iter().zip([0, 1]) {
        assert_eq!(json_record_u64(record, "cpu"), cpu);
        assert_eq!(json_record_u64(record, "checkpoint_restore_count"), 1);
        let labels = record
            .pointer("/checkpoint_restore_labels")
            .and_then(Value::as_array)
            .expect("O3 restore label array")
            .iter()
            .map(|label| label.as_str().expect("restore label string"))
            .collect::<Vec<_>>();
        assert_eq!(labels, ["debug-baseline"]);
        assert_eq!(
            record
                .get("checkpoint_restore_label")
                .and_then(Value::as_str),
            Some("debug-baseline")
        );
        assert_eq!(
            json_record_u64(record, "checkpoint_restore_tick"),
            restore_tick
        );
        assert_eq!(
            json_record_u64(record, "checkpoint_restore_manifest_tick"),
            restored_manifest_tick
        );
        assert_eq!(
            json_record_u64(record, "checkpoint_restore_payload_bytes"),
            restored_payload_bytes
        );

        let events = record
            .pointer("/events")
            .and_then(Value::as_array)
            .expect("O3 trace events array");
        assert!(
            events
                .iter()
                .all(|event| json_record_u64(event, "tick") > restore_tick),
            "cpu{cpu} O3 trace should only include events replayed after restore: {events:?}"
        );
        assert!(
            events.iter().any(|event| {
                json_record_str(event, "pc") == "0x80000070"
                    && json_record_str(event, "lsq_operation") == "store"
                    && json_record_str(event, "lsq_store_address") == "0x800002c0"
            }),
            "cpu{cpu} restored O3 replay events: {events:?}"
        );
        assert!(
            events.iter().any(|event| {
                json_record_str(event, "pc") == "0x80000074"
                    && json_record_str(event, "lsq_operation") == "load"
                    && json_record_str(event, "lsq_load_address") == "0x800002c0"
            }),
            "cpu{cpu} restored O3 replay events: {events:?}"
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 2),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 1),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 2),
        (
            "sim.debug.o3_trace.checkpoint_restore_tick",
            "Tick",
            restore_tick,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            restored_payload_bytes,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.manifests",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.cleared_manifests",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.decode_errors",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.targets",
            "Count",
            restore_execution_modes.len() as u64,
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.functional",
            "Count",
            restore_mode_counts.get("functional").copied().unwrap_or(0),
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.timing",
            "Count",
            restore_mode_counts.get("timing").copied().unwrap_or(0),
        ),
        (
            "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
            "Count",
            restore_mode_counts.get("detailed").copied().unwrap_or(0),
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
    for (target, mode) in [
        ("cpu0", "functional"),
        ("cpu0", "timing"),
        ("cpu0", "detailed"),
        ("cpu1", "functional"),
        ("cpu1", "timing"),
        ("cpu1", "detailed"),
    ] {
        assert_stat(
            &stdout,
            &format!(
                "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
            ),
            "Count",
            restore_target_mode_counts
                .get(&(target, mode))
                .copied()
                .unwrap_or(0),
            "monotonic",
        );
    }
    for cpu in ["cpu0", "cpu1"] {
        for (path, value) in [
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.manifests"
                ),
                1,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.cleared_manifests"
                ),
                0,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.decode_errors"
                ),
                0,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.targets"
                ),
                restore_execution_modes.len() as u64,
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.mode.functional"
                ),
                restore_mode_counts.get("functional").copied().unwrap_or(0),
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.mode.timing"
                ),
                restore_mode_counts.get("timing").copied().unwrap_or(0),
            ),
            (
                format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.mode.detailed"
                ),
                restore_mode_counts.get("detailed").copied().unwrap_or(0),
            ),
        ] {
            assert_stat(&stdout, &path, "Count", value, "monotonic");
        }
        for (target, mode) in [
            ("cpu0", "functional"),
            ("cpu0", "timing"),
            ("cpu0", "detailed"),
            ("cpu1", "functional"),
            ("cpu1", "timing"),
            ("cpu1", "detailed"),
        ] {
            assert_stat(
                &stdout,
                &format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
                ),
                "Count",
                restore_target_mode_counts
                    .get(&(target, mode))
                    .copied()
                    .unwrap_or(0),
                "monotonic",
            );
        }
    }
}

#[test]
fn rem6_run_o3_debug_flag_emits_vector_lsq_byte_events() {
    let path = detailed_o3_vector_memory_debug_binary("debug-flags-o3-vector-lsq-bytes");

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
            "--debug-flags",
            "O3",
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
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("lsq_load_bytes", 8),
        ("lsq_store_bytes", 8),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 vector trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let vector_load = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000018"))
        .unwrap_or_else(|| panic!("missing vector load O3 event: {events:?}"));
    assert_eq!(json_record_u64(vector_load, "rename_writes"), 1);
    assert_eq!(json_record_u64(vector_load, "lsq_loads"), 1);
    assert_eq!(json_record_str(vector_load, "lsq_operation"), "vector_load");
    assert_eq!(json_record_u64(vector_load, "lsq_load_bytes"), 8);
    assert_eq!(json_record_u64(vector_load, "lsq_stores"), 0);
    assert_eq!(json_record_u64(vector_load, "lsq_store_bytes"), 0);
    let vector_load_latency = json_record_u64(vector_load, "lsq_data_latency_ticks");
    assert!(vector_load_latency > 0, "{vector_load:?}");

    let vector_store = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000001c"))
        .unwrap_or_else(|| panic!("missing vector store O3 event: {events:?}"));
    assert_eq!(json_record_u64(vector_store, "rename_writes"), 0);
    assert_eq!(json_record_u64(vector_store, "lsq_loads"), 0);
    assert_eq!(json_record_u64(vector_store, "lsq_load_bytes"), 0);
    assert_eq!(json_record_u64(vector_store, "lsq_stores"), 1);
    assert_eq!(
        json_record_str(vector_store, "lsq_operation"),
        "vector_store"
    );
    assert_eq!(json_record_u64(vector_store, "lsq_store_bytes"), 8);
    let vector_store_latency = json_record_u64(vector_store, "lsq_data_latency_ticks");
    assert!(vector_store_latency > 0, "{vector_store:?}");

    for (path, unit, value) in [
        ("sim.cpu0.o3.lsq_load_bytes", "Byte", 8),
        ("sim.cpu0.o3.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 8),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_avg_ticks",
            "Tick",
            vector_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_min_ticks",
            "Tick",
            vector_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_avg_ticks",
            "Tick",
            vector_store_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_min_ticks",
            "Tick",
            vector_store_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_lsq_operation_shape() {
    let path = detailed_o3_float_memory_debug_binary("debug-flags-o3-float-lsq-operation");

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
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("000000000000f03f000000000000f03f")
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("lsq_load_bytes", 8),
        ("lsq_store_bytes", 8),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 float trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let float_load = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing float load O3 event: {events:?}"));
    assert_eq!(json_record_u64(float_load, "rename_writes"), 1);
    assert_eq!(json_record_u64(float_load, "lsq_loads"), 1);
    assert_eq!(json_record_str(float_load, "lsq_operation"), "float_load");
    assert_eq!(
        json_record_str(float_load, "lsq_load_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(float_load, "lsq_load_bytes"), 8);
    assert_eq!(json_record_u64(float_load, "lsq_stores"), 0);
    assert_eq!(json_record_u64(float_load, "lsq_store_bytes"), 0);
    let float_load_latency = json_record_u64(float_load, "lsq_data_latency_ticks");
    assert!(float_load_latency > 0, "{float_load:?}");

    let float_store = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing float store O3 event: {events:?}"));
    assert_eq!(json_record_u64(float_store, "rename_writes"), 0);
    assert_eq!(json_record_u64(float_store, "lsq_loads"), 0);
    assert_eq!(json_record_u64(float_store, "lsq_load_bytes"), 0);
    assert_eq!(json_record_u64(float_store, "lsq_stores"), 1);
    assert_eq!(json_record_str(float_store, "lsq_operation"), "float_store");
    assert_eq!(
        json_record_str(float_store, "lsq_store_address"),
        "0x80000048"
    );
    assert_eq!(json_record_u64(float_store, "lsq_store_bytes"), 8);
    let float_store_latency = json_record_u64(float_store, "lsq_data_latency_ticks");
    assert!(float_store_latency > 0, "{float_store:?}");

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.float_loads", "Count", 1),
        ("sim.debug.o3_trace.float_stores", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 8),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_avg_ticks",
            "Tick",
            float_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_min_ticks",
            "Tick",
            float_load_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_avg_ticks",
            "Tick",
            float_store_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_min_ticks",
            "Tick",
            float_store_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_atomic_lsq_operation_shape() {
    let path = detailed_o3_atomic_lsq_debug_binary("debug-flags-o3-atomic-lsq-operation");

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
            "--debug-flags",
            "O3",
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
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 5),
        ("rename_writes", 4),
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("lsq_load_bytes", 8),
        ("lsq_store_bytes", 8),
        ("store_load_forwarding_candidates", 0),
        ("store_load_forwarding_matches", 0),
        ("max_lsq_occupancy", 2),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 atomic trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 5);
    let atomic = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing atomic O3 event: {events:?}"));
    assert_eq!(json_record_str(atomic, "lsq_operation"), "atomic");
    assert_eq!(json_record_u64(atomic, "rename_writes"), 1);
    assert_eq!(json_record_u64(atomic, "lsq_loads"), 1);
    assert_eq!(json_record_u64(atomic, "lsq_stores"), 1);
    assert_eq!(json_record_u64(atomic, "lsq_occupancy"), 2);
    assert_eq!(json_record_u64(atomic, "lsq_load_bytes"), 8);
    assert_eq!(json_record_u64(atomic, "lsq_store_bytes"), 8);
    assert_eq!(json_record_str(atomic, "lsq_load_address"), "0x80000040");
    assert_eq!(json_record_str(atomic, "lsq_store_address"), "0x80000040");
    let atomic_latency = json_record_u64(atomic, "lsq_data_latency_ticks");
    assert!(atomic_latency > 0, "{atomic:?}");
    assert_eq!(
        json_record_bool(atomic, "store_load_forwarding_candidate"),
        false
    );
    assert_eq!(
        json_record_bool(atomic, "store_load_forwarding_match"),
        false
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 8),
        ("sim.debug.o3_trace.event.lsq_operation.atomic", "Count", 1),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_avg_ticks",
            "Tick",
            atomic_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_min_ticks",
            "Tick",
            atomic_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_lsq_memory_ordering() {
    let path = detailed_o3_ordered_atomic_lsq_debug_binary("debug-flags-o3-lsq-ordering");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
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
        Some("04000000000000000900000000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000300000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 13),
        ("rename_writes", 8),
        ("lsq_loads", 3),
        ("lsq_stores", 5),
        ("lsq_load_bytes", 24),
        ("lsq_store_bytes", 40),
        ("max_lsq_occupancy", 2),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 ordered LSQ trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 13);
    let by_pc = |pc: &str| {
        events
            .iter()
            .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("missing O3 event at {pc}: {events:?}"))
    };
    for (pc, operation, ordering, acquire, release) in [
        ("0x8000000c", "load", "none", false, false),
        ("0x80000010", "store", "none", false, false),
        ("0x80000014", "load_reserved", "acquire", true, false),
        ("0x8000001c", "store_conditional", "release", false, true),
        ("0x80000024", "atomic", "acquire_release", true, true),
        ("0x80000028", "store", "none", false, false),
        ("0x8000002c", "store", "none", false, false),
    ] {
        let event = by_pc(pc);
        assert_eq!(json_record_str(event, "lsq_operation"), operation);
        assert_eq!(json_record_str(event, "lsq_ordering"), ordering);
        assert_eq!(json_record_bool(event, "lsq_acquire"), acquire);
        assert_eq!(json_record_bool(event, "lsq_release"), release);
    }
    let load_reserved_latency = json_record_u64(by_pc("0x80000014"), "lsq_data_latency_ticks");
    let store_conditional_latency = json_record_u64(by_pc("0x8000001c"), "lsq_data_latency_ticks");
    let atomic_latency = json_record_u64(by_pc("0x80000024"), "lsq_data_latency_ticks");
    assert!(load_reserved_latency > 0, "{events:?}");
    assert!(store_conditional_latency > 0, "{events:?}");
    assert!(atomic_latency > 0, "{events:?}");
    let lsq_latency_values = events
        .iter()
        .map(|event| json_record_u64(event, "lsq_data_latency_ticks"))
        .filter(|latency| *latency > 0)
        .collect::<Vec<_>>();
    let lsq_latency_ticks = lsq_latency_values.iter().sum::<u64>();
    let lsq_latency_samples = lsq_latency_values.len() as u64;
    let lsq_latency_min = lsq_latency_values.iter().copied().min().unwrap_or(0);
    let lsq_latency_max = lsq_latency_values.iter().copied().max().unwrap_or(0);
    let lsq_latency_avg = if lsq_latency_samples == 0 {
        0
    } else {
        lsq_latency_ticks / lsq_latency_samples
    };
    let operation_latency = |operation: &str| {
        let values = events
            .iter()
            .filter(|event| json_record_str(event, "lsq_operation") == operation)
            .map(|event| json_record_u64(event, "lsq_data_latency_ticks"))
            .filter(|latency| *latency > 0)
            .collect::<Vec<_>>();
        let samples = values.len() as u64;
        let ticks = values.iter().sum::<u64>();
        let min_ticks = values.iter().copied().min().unwrap_or(0);
        let max_ticks = values.iter().copied().max().unwrap_or(0);
        let avg_ticks = if samples == 0 { 0 } else { ticks / samples };
        (samples, ticks, max_ticks, min_ticks, avg_ticks)
    };
    let lsq = record
        .pointer("/lsq")
        .unwrap_or_else(|| panic!("missing O3 trace LSQ summary: {record}"));
    for (pointer, value) in [
        ("/loads", 3),
        ("/stores", 5),
        ("/load_bytes", 24),
        ("/store_bytes", 40),
        ("/store_conditional_failures", 0),
        ("/max_occupancy", 2),
        ("/operation/load/count", 1),
        ("/operation/store/count", 3),
        ("/operation/load_reserved/count", 1),
        ("/operation/store_conditional/count", 1),
        ("/operation/atomic/count", 1),
        ("/operation/float_load/count", 0),
        ("/operation/vector_load/count", 0),
        ("/ordering/acquire", 1),
        ("/ordering/release", 1),
        ("/ordering/acquire_release", 1),
        ("/data_latency/samples", lsq_latency_samples),
        ("/data_latency/ticks", lsq_latency_ticks),
        ("/data_latency/max_ticks", lsq_latency_max),
        ("/data_latency/min_ticks", lsq_latency_min),
        ("/data_latency/avg_ticks", lsq_latency_avg),
    ] {
        assert_eq!(
            lsq.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "O3 ordered LSQ summary path {pointer}: {lsq}"
        );
    }
    for operation in ["load_reserved", "store_conditional", "atomic"] {
        let (samples, ticks, max_ticks, min_ticks, avg_ticks) = operation_latency(operation);
        for (metric, value) in [
            ("samples", samples),
            ("ticks", ticks),
            ("max_ticks", max_ticks),
            ("min_ticks", min_ticks),
            ("avg_ticks", avg_ticks),
        ] {
            let pointer = format!("/operation/{operation}/latency/{metric}");
            assert_eq!(
                lsq.pointer(&pointer).and_then(Value::as_u64),
                Some(value),
                "O3 ordered LSQ operation-latency summary path {pointer}: {lsq}"
            );
        }
    }
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    let event_summary_ordering = event_summary
        .pointer("/lsq_ordering")
        .expect("O3 event summary LSQ ordering matrix");
    for (ordering, value) in [("acquire", 1), ("release", 1), ("acquire_release", 1)] {
        assert_eq!(
            event_summary_ordering
                .pointer(&format!("/{ordering}"))
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary LSQ ordering {ordering}: {event_summary_ordering}"
        );
    }
    assert!(
        event_summary_ordering.pointer("/none").is_none(),
        "O3 event summary LSQ ordering matrix should track ordered lanes only: {event_summary_ordering}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 24),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 40),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 24),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 40),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 3),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.atomic", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_ordering.acquire", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_ordering.release", "Count", 1),
        (
            "sim.debug.o3_trace.event.lsq_ordering.acquire_release",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_avg_ticks",
            "Tick",
            load_reserved_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_min_ticks",
            "Tick",
            load_reserved_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_avg_ticks",
            "Tick",
            store_conditional_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_min_ticks",
            "Tick",
            store_conditional_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_avg_ticks",
            "Tick",
            atomic_latency,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_min_ticks",
            "Tick",
            atomic_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_marks_store_conditional_failures() {
    let path = detailed_o3_store_conditional_failure_debug_binary(
        "debug-flags-o3-store-conditional-failure",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("88776655443322110100000000000000")
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_load_bytes", 0),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 failed SC trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let store_conditional = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing store-conditional O3 event: {events:?}"));
    assert_eq!(
        json_record_str(store_conditional, "lsq_operation"),
        "store_conditional"
    );
    assert_eq!(json_record_u64(store_conditional, "rename_writes"), 1);
    assert_eq!(json_record_u64(store_conditional, "lsq_loads"), 0);
    assert_eq!(json_record_u64(store_conditional, "lsq_stores"), 1);
    assert_eq!(
        json_record_str(store_conditional, "lsq_store_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(store_conditional, "lsq_store_bytes"), 8);
    assert_eq!(
        json_record_bool(store_conditional, "lsq_store_conditional_failed"),
        true
    );

    let result_store = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .unwrap_or_else(|| panic!("missing SC result store O3 event: {events:?}"));
    assert_eq!(json_record_str(result_store, "lsq_operation"), "store");
    assert_eq!(
        json_record_bool(result_store, "lsq_store_conditional_failed"),
        false
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 16),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.lsq_store_conditional_failures",
            "Count",
            1,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_indirect_call_branch_links() {
    let path = detailed_o3_indirect_call_debug_binary("debug-flags-o3-indirect-call-events");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("18000080000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 9),
        ("rename_writes", 5),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 indirect call trace field {field}"
        );
    }
    let branch_event = record
        .pointer("/branch_event")
        .unwrap_or_else(|| panic!("missing O3 trace branch-event summary: {record}"));
    for (field, value) in [
        ("branches", 1),
        ("taken", 1),
        ("not_taken", 0),
        ("predicted_taken", 0),
        ("predicted_not_taken", 1),
        ("predicted_targets", 0),
        ("predicted_target_matches", 0),
        ("predicted_target_mismatches", 0),
        ("resolved_targets", 1),
        ("link_writes", 1),
        ("without_link_writes", 0),
        ("squashes", 1),
        ("squashed_targets", 1),
        ("squashed_targets_with_link_writes", 1),
        ("squashed_targets_without_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(branch_event, field),
            value,
            "O3 indirect call branch-event summary field {field}"
        );
    }
    for path in [
        "/kind/call_indirect",
        "/taken_kind/call_indirect",
        "/predicted_not_taken_kind/call_indirect",
        "/resolved_target_kind/call_indirect",
        "/link_write_kind/call_indirect",
        "/squash_kind/call_indirect",
        "/squashed_target_link_write_kind/call_indirect",
    ] {
        assert_eq!(
            branch_event.pointer(path).and_then(Value::as_u64),
            Some(1),
            "O3 indirect call branch-event summary path {path}"
        );
    }
    assert_eq!(
        branch_event
            .pointer("/squashed_target_without_link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0)
    );
    let branch_direction_mismatch = record
        .pointer("/branch_direction_mismatch")
        .unwrap_or_else(|| panic!("missing O3 trace branch-direction-mismatch summary: {record}"));
    for (field, value) in [
        ("mismatches", 1),
        ("without_link_writes", 0),
        ("squashed_targets", 1),
        ("squashed_target_without_link_writes", 0),
        ("squashed_target_link_writes", 1),
    ] {
        assert_eq!(
            json_record_u64(branch_direction_mismatch, field),
            value,
            "O3 indirect call branch-direction-mismatch summary field {field}"
        );
    }
    for path in [
        "/kind/call_indirect",
        "/link_write_kind/call_indirect",
        "/squashed_target_kind/call_indirect",
        "/squashed_target_link_write_kind/call_indirect",
    ] {
        assert_eq!(
            branch_direction_mismatch
                .pointer(path)
                .and_then(Value::as_u64),
            Some(1),
            "O3 indirect call branch-direction-mismatch summary path {path}"
        );
    }
    for path in [
        "/kind/direct_conditional",
        "/without_link_write_kind/call_indirect",
        "/squashed_target_without_link_write_kind/call_indirect",
    ] {
        assert_eq!(
            branch_direction_mismatch
                .pointer(path)
                .and_then(Value::as_u64),
            Some(0),
            "O3 indirect call branch-direction-mismatch summary zero path {path}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 9);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
            "0x8000001c",
            "0x80000020",
            "0x80000024",
            "0x80000028",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000018"));
    let first = &events[0];
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(json_record_bool(first, "branch_mispredicted"), false);
    assert_eq!(json_record_bool(first, "branch_link_register_write"), false);
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_resolved_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[8], "system_event"), true);
    let system_events = events
        .iter()
        .filter(|event| json_record_bool(event, "system_event"))
        .count() as u64;
    assert!(
        system_events > 0,
        "O3 events should include system work: {events:?}"
    );
    let branch_predicted_not_taken = events
        .iter()
        .filter(|event| {
            json_record_bool(event, "branch_event")
                && !json_record_bool(event, "branch_predicted_taken")
        })
        .count() as u64;
    assert_eq!(branch_predicted_not_taken, 1);
    let branch_squashed_targets = events
        .iter()
        .filter(|event| {
            json_record_bool(event, "branch_event")
                && event
                    .get("branch_squashed_target")
                    .is_some_and(|target| !target.is_null())
        })
        .count() as u64;
    assert_eq!(branch_squashed_targets, 1);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .unwrap_or_else(|| panic!("missing O3 indirect call branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "call_indirect");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(json_record_bool(branch, "branch_link_register_write"), true);
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x8000001c"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000018"
    );

    for (path, unit, value) in [
        (
            "sim.debug.o3_trace.event.system_events",
            "Count",
            system_events,
        ),
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_predicted_not_taken",
            "Count",
            branch_predicted_not_taken,
        ),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 1),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_squashed_targets",
            "Count",
            branch_squashed_targets,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_link_writes",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_without_link_writes",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.event.branch_link_writes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_taken_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_not_taken_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_resolved_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_link_write_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_link_write_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_without_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_kind.call_indirect",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_json_stats_exposes_o3_call_indirect_wrong_target_runtime_matrix() {
    let path = detailed_o3_indirect_call_wrong_target_debug_binary(
        "stats-o3-indirect-call-wrong-target-runtime-matrix",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "280",
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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .expect("structured O3 runtime JSON");
    let branch_repair = o3_runtime
        .get("branch_repair")
        .expect("structured O3 branch-repair runtime matrix");
    let branch_event = o3_runtime
        .get("branch_event")
        .expect("structured O3 branch-event runtime matrix");

    assert_eq!(json_record_u64(branch_event, "branches"), 2);
    assert_eq!(json_record_u64(branch_event, "taken"), 2);
    assert_eq!(json_record_u64(branch_event, "not_taken"), 0);
    assert_eq!(json_record_u64(branch_event, "resolved_targets"), 2);
    assert_eq!(json_record_u64(branch_event, "mispredictions"), 2);
    assert_eq!(json_record_u64(branch_event, "link_writes"), 1);
    assert_eq!(json_record_u64(branch_event, "without_link_writes"), 1);
    assert_eq!(
        branch_event
            .pointer("/without_link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0),
        "call-indirect runtime matrix should subtract link-write branches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-unconditional runtime matrix should expose branches without link writes: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/misprediction_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "call-indirect runtime matrix should expose mispredicted branches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/misprediction_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-unconditional runtime matrix should expose mispredicted branches: {branch_event}"
    );
    assert_eq!(
        json_record_u64(branch_event, "squashed_targets_with_link_writes"),
        1
    );
    assert_eq!(
        branch_event
            .pointer("/squashed_target_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "call-indirect runtime matrix should expose squashed targets by branch kind: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/squashed_target_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-unconditional runtime matrix should expose squashed targets by branch kind: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/squashed_target_link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "call-indirect runtime matrix should expose squashed target link writes: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/squashed_target_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(0),
        "direct-unconditional runtime matrix should expose zero link-write squashes: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/squashed_target_without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-unconditional runtime matrix should expose no-link squashes: {branch_event}"
    );
    assert_eq!(json_record_u64(branch_repair, "targetless_mismatches"), 0);
    assert_eq!(json_record_u64(branch_repair, "wrong_targets"), 1);
    assert_eq!(
        json_record_u64(branch_repair, "direction_only_mismatches"),
        1
    );
    assert_eq!(
        branch_repair
            .pointer("/wrong_target_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "call-indirect wrong-target runtime matrix should be structured: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/direction_only_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-unconditional direction-only runtime matrix should be structured: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/targetless_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(0),
        "targetless runtime matrix should include zero-valued branch kinds: {branch_repair}"
    );
    assert_eq!(
        json_record_u64(o3_runtime, "iew_predicted_taken_incorrect"),
        1
    );
    assert_eq!(
        json_record_u64(o3_runtime, "iew_predicted_not_taken_incorrect"),
        1
    );

    for (path, unit, value) in [
        ("sim.cpu0.o3.branch_event.branches", "Count", 2),
        ("sim.cpu0.o3.branch_event.taken", "Count", 2),
        ("sim.cpu0.o3.branch_event.not_taken", "Count", 0),
        ("sim.cpu0.o3.branch_event.resolved_targets", "Count", 2),
        ("sim.cpu0.o3.branch_event.mispredictions", "Count", 2),
        ("sim.cpu0.o3.branch_event.link_writes", "Count", 1),
        ("sim.cpu0.o3.branch_event.without_link_writes", "Count", 1),
        (
            "sim.cpu0.o3.branch_event.without_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.misprediction_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.misprediction_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_targets_with_link_writes",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_target_link_write_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_target_link_write_kind.direct_unconditional",
            "Count",
            0,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_repair_targetless_mismatches",
            "Count",
            0,
        ),
        ("sim.cpu0.o3.branch_repair_wrong_targets", "Count", 1),
        (
            "sim.cpu0.o3.branch_repair_direction_only_mismatches",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_repair_wrong_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_repair_direction_only_kind.direct_unconditional",
            "Count",
            1,
        ),
        ("sim.cpu0.o3.iew.predicted_taken_incorrect", "Count", 1),
        ("sim.cpu0.o3.iew.predicted_not_taken_incorrect", "Count", 1),
        ("system.cpu.iew.predictedTakenIncorrect", "Count", 1),
        ("system.cpu.iew.predictedNotTakenIncorrect", "Count", 1),
        ("system.cpu.ftq.squashedTargets_0::CallIndirect", "Count", 1),
        ("system.cpu.ftq.squashedTargets_0::DirectUncond", "Count", 1),
        ("system.cpu.ftq.squashedTargets_0::total", "Count", 2),
        (
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
            "Count",
            1,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::DirectUncond",
            "Count",
            0,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::total",
            "Count",
            1,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::CallIndirect",
            "Count",
            0,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
            "Count",
            1,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::total",
            "Count",
            1,
        ),
        ("system.cpu.iew.branchMispredicts", "Count", 2),
        ("system.cpu.commit.branchMispredicts", "Count", 2),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_indirect_call_branch_wrong_targets() {
    let path = detailed_o3_indirect_call_wrong_target_debug_binary(
        "debug-flags-o3-indirect-call-wrong-target",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "280",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000070:16",
            "--dump-memory",
            "0x80000080:16",
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
        Some("24000080000000000c00008000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    let branch_repair = record
        .pointer("/branch_repair")
        .unwrap_or_else(|| panic!("missing O3 trace branch-repair summary: {record}"));
    assert_eq!(json_record_u64(branch_repair, "targetless_mismatches"), 0);
    assert_eq!(json_record_u64(branch_repair, "wrong_targets"), 1);
    assert_eq!(
        json_record_u64(branch_repair, "direction_only_mismatches"),
        1
    );
    assert_eq!(
        branch_repair
            .pointer("/wrong_target_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-repair summary should expose call-indirect wrong targets: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/direction_only_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-repair summary should expose direct-unconditional direction-only repairs: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/targetless_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-repair summary should include zero-valued branch kinds: {branch_repair}"
    );
    let branch_event = record
        .pointer("/branch_event")
        .unwrap_or_else(|| panic!("missing O3 trace branch-event summary: {record}"));
    assert_eq!(
        json_record_u64(branch_event, "mispredictions"),
        2,
        "O3 trace branch-event summary should expose aggregate mispredictions: {branch_event}"
    );
    for (path, value) in [
        ("/misprediction_kind/call_indirect", 1),
        ("/misprediction_kind/direct_unconditional", 1),
        ("/misprediction_kind/direct_conditional", 0),
    ] {
        assert_eq!(
            branch_event.pointer(path).and_then(Value::as_u64),
            Some(value),
            "O3 trace branch-event misprediction-kind path {path}: {branch_event}"
        );
    }
    for (path, value) in [
        ("/squashed_target_kind/call_indirect", 1),
        ("/squashed_target_kind/direct_unconditional", 1),
        ("/squashed_target_kind/direct_conditional", 0),
    ] {
        assert_eq!(
            branch_event.pointer(path).and_then(Value::as_u64),
            Some(value),
            "O3 trace branch-event squashed-target-kind path {path}: {branch_event}"
        );
    }
    for branch_kind in [
        "call_indirect",
        "direct_unconditional",
        "direct_conditional",
    ] {
        assert_eq!(
            branch_event
                .pointer(&format!("/squashed_target_kind/{branch_kind}"))
                .and_then(Value::as_u64),
            Some(
                branch_event
                    .pointer(&format!("/squashed_target_link_write_kind/{branch_kind}"))
                    .and_then(Value::as_u64)
                    .unwrap()
                    + branch_event
                        .pointer(&format!(
                            "/squashed_target_without_link_write_kind/{branch_kind}"
                        ))
                        .and_then(Value::as_u64)
                        .unwrap()
            ),
            "O3 trace branch-event squashed-target conservation for {branch_kind}: {branch_event}"
        );
    }
    let event_summary_branch_repair = record
        .pointer("/event_summary/branch_repair")
        .expect("O3 event summary branch-repair matrix");
    for (field, value) in [
        ("targetless_mismatches", 0),
        ("wrong_targets", 1),
        ("direction_only_mismatches", 1),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_repair, field),
            value,
            "O3 event summary wrong-target branch-repair field {field}"
        );
    }
    for (path, value) in [
        ("/wrong_target_kind/call_indirect", 1),
        ("/direction_only_kind/direct_unconditional", 1),
        ("/targetless_mismatch_kind/direct_conditional", 0),
    ] {
        assert_eq!(
            event_summary_branch_repair
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary wrong-target branch-repair path {path}: {event_summary_branch_repair}"
        );
    }
    let branch_target_mismatch = record
        .pointer("/branch_target_mismatch")
        .unwrap_or_else(|| panic!("missing O3 trace branch-target mismatch summary: {record}"));
    for (field, value) in [
        ("targetless_mismatches", 0),
        ("targetless_mismatch_without_link_writes", 0),
        ("targetless_mismatch_squashed_targets", 0),
        ("targetless_mismatch_squashed_target_without_link_writes", 0),
        ("wrong_targets", 1),
        ("wrong_target_squashed_targets", 1),
        ("wrong_target_squashed_target_without_link_writes", 0),
        ("wrong_target_squashed_target_link_writes", 1),
        ("wrong_target_link_writes", 1),
        ("wrong_target_without_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(branch_target_mismatch, field),
            value,
            "O3 call-indirect branch-target mismatch summary field {field}"
        );
    }
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-target mismatch summary should expose call-indirect wrong targets: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_squashed_target_link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-target mismatch summary should expose linked call-indirect wrong-target squashes: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_without_link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-target mismatch summary should include zero-valued no-link wrong-target lanes: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/targetless_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-target mismatch summary should include zero-valued targetless lanes: {branch_target_mismatch}"
    );
    let event_summary_branch_target_mismatch = record
        .pointer("/event_summary/branch_target_mismatch")
        .expect("O3 event summary branch-target mismatch matrix");
    for (field, value) in [
        ("targetless_mismatches", 0),
        ("targetless_mismatch_without_link_writes", 0),
        ("targetless_mismatch_squashed_targets", 0),
        ("targetless_mismatch_squashed_target_without_link_writes", 0),
        ("wrong_targets", 1),
        ("wrong_target_squashed_targets", 1),
        ("wrong_target_squashed_target_without_link_writes", 0),
        ("wrong_target_squashed_target_link_writes", 1),
        ("wrong_target_link_writes", 1),
        ("wrong_target_without_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_target_mismatch, field),
            value,
            "O3 event summary wrong-target branch-target mismatch field {field}"
        );
    }
    for (path, value) in [
        ("/wrong_target_kind/call_indirect", 1),
        (
            "/wrong_target_squashed_target_link_write_kind/call_indirect",
            1,
        ),
        ("/wrong_target_without_link_write_kind/call_indirect", 0),
        ("/targetless_mismatch_kind/direct_conditional", 0),
    ] {
        assert_eq!(
            event_summary_branch_target_mismatch
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary wrong-target branch-target mismatch path {path}: {event_summary_branch_target_mismatch}"
        );
    }
    let event_summary_branch_direction_mismatch = record
        .pointer("/event_summary/branch_direction_mismatch")
        .expect("O3 event summary branch-direction mismatch matrix");
    for (field, value) in [
        ("mismatches", 1),
        ("without_link_writes", 1),
        ("squashed_targets", 1),
        ("squashed_target_without_link_writes", 1),
        ("squashed_target_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_direction_mismatch, field),
            value,
            "O3 event summary wrong-target branch-direction mismatch field {field}"
        );
    }
    for (path, value) in [
        ("/kind/direct_unconditional", 1),
        ("/kind/call_indirect", 0),
        ("/without_link_write_kind/direct_unconditional", 1),
        (
            "/squashed_target_without_link_write_kind/direct_unconditional",
            1,
        ),
        ("/link_write_kind/call_indirect", 0),
    ] {
        assert_eq!(
            event_summary_branch_direction_mismatch
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary wrong-target branch-direction mismatch path {path}: {event_summary_branch_direction_mismatch}"
        );
    }
    let event_summary_branch_event = record
        .pointer("/event_summary/branch_event")
        .expect("O3 event summary branch-event matrix");
    for (field, value) in [
        ("branches", 2),
        ("taken", 2),
        ("not_taken", 0),
        ("predicted_taken", 1),
        ("predicted_not_taken", 1),
        ("predicted_targets", 1),
        ("predicted_target_matches", 0),
        ("predicted_target_mismatches", 1),
        ("resolved_targets", 2),
        ("mispredictions", 2),
        ("squashes", 2),
        ("link_writes", 1),
        ("without_link_writes", 1),
        ("squashed_targets", 2),
        ("squashed_targets_with_link_writes", 1),
        ("squashed_targets_without_link_writes", 1),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_event, field),
            value,
            "O3 event summary wrong-target branch-event field {field}"
        );
    }
    for (path, value) in [
        ("/kind/call_indirect", 1),
        ("/kind/direct_unconditional", 1),
        ("/taken_kind/call_indirect", 1),
        ("/taken_kind/direct_unconditional", 1),
        ("/predicted_taken_kind/call_indirect", 1),
        ("/predicted_not_taken_kind/direct_unconditional", 1),
        ("/predicted_target_kind/call_indirect", 1),
        ("/predicted_target_match_kind/call_indirect", 0),
        ("/predicted_target_mismatch_kind/call_indirect", 1),
        ("/resolved_target_kind/call_indirect", 1),
        ("/resolved_target_kind/direct_unconditional", 1),
        ("/misprediction_kind/call_indirect", 1),
        ("/misprediction_kind/direct_unconditional", 1),
        ("/squash_kind/call_indirect", 1),
        ("/squash_kind/direct_unconditional", 1),
        ("/link_write_kind/call_indirect", 1),
        ("/link_write_kind/direct_unconditional", 0),
        ("/squashed_target_link_write_kind/call_indirect", 1),
        ("/squashed_target_link_write_kind/direct_unconditional", 0),
        ("/squashed_target_without_link_write_kind/call_indirect", 0),
        (
            "/squashed_target_without_link_write_kind/direct_unconditional",
            1,
        ),
        ("/squashed_target_kind/call_indirect", 1),
        ("/squashed_target_kind/direct_unconditional", 1),
        ("/squashed_target_kind/direct_conditional", 0),
    ] {
        assert_eq!(
            event_summary_branch_event
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary wrong-target branch-event path {path}: {event_summary_branch_event}"
        );
    }
    for branch_kind in [
        "call_indirect",
        "direct_unconditional",
        "direct_conditional",
    ] {
        assert_eq!(
            event_summary_branch_event
                .pointer(&format!("/squashed_target_kind/{branch_kind}"))
                .and_then(Value::as_u64),
            Some(
                event_summary_branch_event
                    .pointer(&format!(
                        "/squashed_target_link_write_kind/{branch_kind}"
                    ))
                    .and_then(Value::as_u64)
                    .unwrap()
                    + event_summary_branch_event
                        .pointer(&format!(
                            "/squashed_target_without_link_write_kind/{branch_kind}"
                        ))
                        .and_then(Value::as_u64)
                        .unwrap()
            ),
            "O3 event summary branch-event squashed-target conservation for {branch_kind}: {event_summary_branch_event}"
        );
    }
    let event_summary_iew = record
        .pointer("/event_summary/iew")
        .expect("O3 event summary IEW matrix");
    for (field, value) in [
        ("predicted_taken_incorrect", 1),
        ("predicted_not_taken_incorrect", 1),
        ("branch_mispredicts", 2),
    ] {
        assert_eq!(
            json_record_u64(event_summary_iew, field),
            value,
            "O3 event summary wrong-target IEW field {field}"
        );
    }
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 11);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000014",
            "0x80000018",
            "0x8000001c",
            "0x80000008",
            "0x80000024",
            "0x80000028",
            "0x8000002c",
            "0x80000030",
            "0x80000034",
            "0x80000038",
            "0x8000003c",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000010"));
    assert!(!event_pcs.contains(&"0x80000020"));
    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000008"))
        .unwrap_or_else(|| panic!("missing O3 indirect call branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "call_indirect");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(json_record_bool(branch, "branch_wrong_target"), true);
    assert_eq!(json_record_str(branch, "branch_repair"), "wrong_target");
    assert_eq!(json_record_bool(branch, "branch_link_register_write"), true);
    assert_eq!(
        json_record_str(branch, "branch_predicted_target"),
        "0x80000010"
    );
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x80000024"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000010"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 2),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 2),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_matches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_mismatches",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatches",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.event.branch_wrong_targets", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_link_writes",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_link_writes",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_targetless_mismatches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_mismatches",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.iew_predicted_taken_incorrect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.iew_predicted_not_taken_incorrect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.iew_branch_mispredicts",
            "Count",
            2,
        ),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 2),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 2),
        ("sim.debug.o3_trace.event.branch_link_writes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_mismatch_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_link_write_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_link_write_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_resolved_target_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_link_write_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.call_indirect",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_unconditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 3),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 24),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_indirect_unconditional_branch_targets() {
    let path = detailed_o3_indirect_jump_debug_binary("debug-flags-o3-indirect-jump-events");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("1c000080000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 9),
        ("rename_writes", 4),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 indirect jump trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 9);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
            "0x8000001c",
            "0x80000020",
            "0x80000024",
            "0x80000028",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000018"));
    let first = &events[0];
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(json_record_bool(first, "branch_mispredicted"), false);
    assert_eq!(json_record_bool(first, "branch_link_register_write"), false);
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_resolved_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[8], "system_event"), true);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .unwrap_or_else(|| panic!("missing O3 indirect jump branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(
        json_record_str(branch, "branch_kind"),
        "indirect_unconditional"
    );
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(
        json_record_bool(branch, "branch_link_register_write"),
        false
    );
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x8000001c"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000018"
    );
    let branch_resolved_targets = events
        .iter()
        .filter(|event| {
            json_record_bool(event, "branch_event")
                && event
                    .get("branch_resolved_target")
                    .is_some_and(|target| !target.is_null())
        })
        .count() as u64;
    assert_eq!(branch_resolved_targets, 1);

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 1),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_resolved_targets",
            "Count",
            branch_resolved_targets,
        ),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_without_link_writes",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.branch_link_writes", "Count", 0),
        (
            "sim.debug.o3_trace.event.branch_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_taken_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_resolved_target_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_link_write_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_link_write_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_without_link_write_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.indirect_unconditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_indirect_unconditional_branch_wrong_targets() {
    let path =
        detailed_o3_indirect_jump_wrong_target_debug_binary("debug-flags-o3-indirect-wrong-target");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000060:16",
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
        Some("24000080000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    let branch_repair = record
        .pointer("/branch_repair")
        .unwrap_or_else(|| panic!("missing O3 trace branch-repair summary: {record}"));
    assert_eq!(json_record_u64(branch_repair, "targetless_mismatches"), 0);
    assert_eq!(json_record_u64(branch_repair, "wrong_targets"), 1);
    assert_eq!(
        json_record_u64(branch_repair, "direction_only_mismatches"),
        1
    );
    assert_eq!(
        branch_repair
            .pointer("/wrong_target_kind/indirect_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-repair summary should expose indirect-unconditional wrong targets: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/direction_only_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-repair summary should expose direct-unconditional direction-only repairs: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/targetless_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-repair summary should include zero-valued targetless branch kinds: {branch_repair}"
    );
    let branch_target_mismatch = record
        .pointer("/branch_target_mismatch")
        .unwrap_or_else(|| panic!("missing O3 trace branch-target mismatch summary: {record}"));
    for (field, value) in [
        ("targetless_mismatches", 0),
        ("targetless_mismatch_without_link_writes", 0),
        ("targetless_mismatch_squashed_targets", 0),
        ("targetless_mismatch_squashed_target_without_link_writes", 0),
        ("wrong_targets", 1),
        ("wrong_target_squashed_targets", 1),
        ("wrong_target_squashed_target_without_link_writes", 1),
        ("wrong_target_squashed_target_link_writes", 0),
        ("wrong_target_link_writes", 0),
        ("wrong_target_without_link_writes", 1),
    ] {
        assert_eq!(
            json_record_u64(branch_target_mismatch, field),
            value,
            "O3 indirect-unconditional branch-target mismatch summary field {field}"
        );
    }
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_kind/indirect_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-target mismatch summary should expose indirect-unconditional wrong targets: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_squashed_target_without_link_write_kind/indirect_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-target mismatch summary should expose no-link indirect-unconditional wrong-target squashes: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_link_write_kind/indirect_unconditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-target mismatch summary should include zero-valued link-write wrong-target lanes: {branch_target_mismatch}"
    );
    let branch_direction_mismatch = record
        .pointer("/branch_direction_mismatch")
        .unwrap_or_else(|| panic!("missing O3 trace branch-direction-mismatch summary: {record}"));
    for (field, value) in [
        ("mismatches", 1),
        ("without_link_writes", 1),
        ("squashed_targets", 1),
        ("squashed_target_without_link_writes", 1),
        ("squashed_target_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(branch_direction_mismatch, field),
            value,
            "O3 indirect-unconditional branch-direction-mismatch summary field {field}"
        );
    }
    assert_eq!(
        branch_direction_mismatch
            .pointer("/kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-direction-mismatch summary should expose direct-unconditional mismatches: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/kind/indirect_unconditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-direction-mismatch summary should include zero-valued indirect-unconditional lanes: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-direction-mismatch summary should expose no-link direct-unconditional lanes: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/squashed_target_without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-direction-mismatch summary should expose squashed no-link direct-unconditional lanes: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-direction-mismatch summary should include zero-valued link-write branch kinds: {branch_direction_mismatch}"
    );
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 10);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000014",
            "0x80000018",
            "0x8000001c",
            "0x80000008",
            "0x80000024",
            "0x80000028",
            "0x8000002c",
            "0x80000030",
            "0x80000034",
            "0x80000038",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000010"));
    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000008"))
        .unwrap_or_else(|| panic!("missing O3 indirect jump branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(
        json_record_str(branch, "branch_kind"),
        "indirect_unconditional"
    );
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(json_record_str(branch, "branch_repair"), "wrong_target");
    assert_eq!(
        json_record_bool(branch, "branch_link_register_write"),
        false
    );
    assert_eq!(
        json_record_str(branch, "branch_predicted_target"),
        "0x80000010"
    );
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x80000024"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000010"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 2),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 2),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_matches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_mismatches",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.branch_wrong_targets", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_without_link_writes",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_without_link_writes",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_targetless_mismatches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_mismatches",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 2),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 2),
        (
            "sim.debug.o3_trace.event.branch_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_mismatch_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_without_link_write_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_without_link_write_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_target_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_resolved_target_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.indirect_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_unconditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_return_branch_taken_targets() {
    let path = detailed_o3_return_debug_binary("debug-flags-o3-return-events");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("1c000080000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 9),
        ("rename_writes", 4),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 return trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 9);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
            "0x8000001c",
            "0x80000020",
            "0x80000024",
            "0x80000028",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000018"));
    let first = &events[0];
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(json_record_bool(first, "branch_resolved_taken"), false);
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[8], "system_event"), true);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .unwrap_or_else(|| panic!("missing O3 return branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "return");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x8000001c"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000018"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 1),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 1),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 1),
        ("sim.debug.o3_trace.event.branch_kind.return", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_taken_kind.return",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.return",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.return",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_direct_call_branch_mispredictions() {
    let path = detailed_o3_direct_call_debug_binary("debug-flags-o3-direct-call-events");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("10000080000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 7),
        ("rename_writes", 3),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 direct call trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 7);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000014",
            "0x80000018",
            "0x8000001c",
            "0x80000020",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000010"));
    let first = &events[0];
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(json_record_bool(first, "branch_mispredicted"), false);
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[6], "system_event"), true);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing O3 direct call branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "call_direct");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x80000014"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000010"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 1),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 1),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_kind.call_direct",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.call_direct",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.call_direct",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_direct_unconditional_branch_squashes() {
    let path = detailed_o3_direct_jump_debug_binary("debug-flags-o3-direct-jump-events");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("00000000000000000700000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 8),
        ("rename_writes", 3),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 direct jump trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 8);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000014",
            "0x80000018",
            "0x8000001c",
            "0x80000020",
            "0x80000024",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000010"));
    let first = &events[0];
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[7], "system_event"), true);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing O3 direct jump branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(
        json_record_str(branch, "branch_kind"),
        "direct_unconditional"
    );
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x80000014"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000010"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 1),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 1),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_unconditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_branch_events() {
    let path = detailed_o3_branch_debug_binary("debug-flags-o3-branch-events");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("00000000000000000700000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 9),
        ("rename_writes", 4),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 branch trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 9);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    // The trailing m5_fail sentinel is visible; the branch fall-through is not.
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000018",
            "0x8000001c",
            "0x80000020",
            "0x80000024",
            "0x80000028",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000014"));
    let first = &events[0];
    assert_eq!(json_record_bool(first, "system_event"), false);
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(first.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_resolved_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[8], "system_event"), true);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing O3 branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "direct_conditional");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x80000018"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000014"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 1),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 1),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_conditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_direct_conditional_branch_not_taken() {
    let path = detailed_o3_branch_not_taken_debug_binary("debug-flags-o3-branch-not-taken");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000040:16",
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
        Some("07000000000000000900000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    for (field, value) in [
        ("instructions", 11),
        ("rename_writes", 6),
        ("lsq_loads", 0),
        ("lsq_stores", 2),
        ("lsq_store_bytes", 16),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 branch-not-taken trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 11);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
            "0x80000018",
            "0x8000001c",
            "0x80000020",
            "0x80000024",
            "0x80000028",
            "0x8000002c",
        ]
    );
    let first = &events[0];
    assert_eq!(json_record_bool(first, "branch_event"), false);
    assert_eq!(json_record_str(first, "branch_kind"), "no_branch");
    assert_eq!(json_record_bool(first, "branch_mispredicted"), false);
    assert_eq!(json_record_bool(first, "branch_link_register_write"), false);
    assert_eq!(json_record_bool(first, "branch_squash"), false);
    assert_eq!(first.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_resolved_target"), Some(&Value::Null));
    assert_eq!(first.get("branch_squashed_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(&events[10], "system_event"), true);

    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000014"))
        .unwrap_or_else(|| panic!("missing O3 not-taken branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "direct_conditional");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), false);
    assert_eq!(
        json_record_bool(branch, "branch_link_register_write"),
        false
    );
    assert_eq!(branch.get("branch_predicted_target"), Some(&Value::Null));
    assert_eq!(branch.get("branch_resolved_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(branch, "branch_squash"), false);
    assert_eq!(branch.get("branch_squashed_target"), Some(&Value::Null));

    let branch_event = json
        .pointer("/cores/0/o3_runtime/branch_event")
        .expect("structured O3 branch-event runtime matrix");
    assert_eq!(json_record_u64(branch_event, "branches"), 1);
    assert_eq!(json_record_u64(branch_event, "taken"), 0);
    assert_eq!(json_record_u64(branch_event, "not_taken"), 1);
    assert_eq!(json_record_u64(branch_event, "resolved_targets"), 0);
    let debug_branch_event = json
        .pointer("/debug/o3_trace/0/branch_event")
        .expect("debug O3 branch-event matrix");
    assert_eq!(
        debug_branch_event
            .pointer("/not_taken_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "debug O3 branch-event matrix should expose direct-conditional not-taken outcomes: {debug_branch_event}"
    );
    assert_eq!(
        debug_branch_event
            .pointer("/not_taken_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0),
        "debug O3 branch-event matrix should expose zero call-indirect not-taken outcomes: {debug_branch_event}"
    );
    assert_eq!(
        debug_branch_event
            .pointer("/not_taken_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(0),
        "debug O3 branch-event matrix should expose zero direct-unconditional not-taken outcomes: {debug_branch_event}"
    );
    assert_eq!(
        debug_branch_event
            .pointer("/without_link_write_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "debug O3 branch-event matrix should expose direct-conditional no-link outcomes: {debug_branch_event}"
    );
    assert_eq!(
        debug_branch_event
            .pointer("/without_link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0),
        "debug O3 branch-event matrix should expose zero call-indirect no-link outcomes: {debug_branch_event}"
    );
    assert_eq!(
        debug_branch_event
            .pointer("/without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(0),
        "debug O3 branch-event matrix should expose zero direct-unconditional no-link outcomes: {debug_branch_event}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 1),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 0),
        ("sim.debug.o3_trace.event.branch_not_taken", "Count", 1),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 0),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 0),
        ("sim.debug.o3_trace.event.branch_link_writes", "Count", 0),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_taken_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_not_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_resolved_target_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_link_write_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_conditional",
            "Count",
            0,
        ),
        ("sim.cpu0.o3.branch_event.branches", "Count", 1),
        ("sim.cpu0.o3.branch_event.taken", "Count", 0),
        ("sim.cpu0.o3.branch_event.not_taken", "Count", 1),
        ("sim.cpu0.o3.branch_event.resolved_targets", "Count", 0),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_direct_conditional_branch_predicted_target_matches() {
    let path = detailed_o3_branch_predicted_target_match_debug_binary(
        "debug-flags-o3-branch-predicted-target-match",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000060:16",
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
        Some("01000000000000000100000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let events = trace[0]
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 10);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000018",
            "0x8000001c",
            "0x80000008",
            "0x80000010",
            "0x80000020",
            "0x80000024",
            "0x80000028",
            "0x8000002c",
            "0x80000030",
            "0x80000034",
        ]
    );
    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000008"))
        .unwrap_or_else(|| panic!("missing warmed O3 branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "direct_conditional");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), false);
    assert_eq!(
        json_record_str(branch, "branch_predicted_target"),
        "0x80000010"
    );
    assert_eq!(
        json_record_str(branch, "branch_resolved_target"),
        "0x80000010"
    );
    assert_eq!(json_record_bool(branch, "branch_squash"), false);
    assert_eq!(branch.get("branch_squashed_target"), Some(&Value::Null));
    let branch_predicted_targets = events
        .iter()
        .filter(|event| {
            json_record_bool(event, "branch_event")
                && event
                    .get("branch_predicted_target")
                    .is_some_and(|target| !target.is_null())
        })
        .count() as u64;
    assert_eq!(branch_predicted_targets, 1);

    let branch_event = json
        .pointer("/cores/0/o3_runtime/branch_event")
        .expect("structured O3 branch-event runtime matrix");
    assert_eq!(json_record_u64(branch_event, "branches"), 3);
    assert_eq!(json_record_u64(branch_event, "predicted_taken"), 1);
    assert_eq!(json_record_u64(branch_event, "predicted_not_taken"), 2);
    assert_eq!(json_record_u64(branch_event, "predicted_targets"), 1);
    assert_eq!(json_record_u64(branch_event, "predicted_target_matches"), 1);
    assert_eq!(
        json_record_u64(branch_event, "predicted_target_mismatches"),
        0
    );
    assert_eq!(
        branch_event
            .pointer("/predicted_taken_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose predicted-taken branches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/predicted_not_taken_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose predicted-not-taken branches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/predicted_target_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose predicted targets: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/predicted_target_match_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose predicted target matches: {branch_event}"
    );
    let event_summary = trace[0]
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    let event_summary_branch_event = event_summary
        .pointer("/branch_event")
        .expect("O3 event summary branch-event matrix");
    for (field, value) in [
        ("branches", 3),
        ("taken", 3),
        ("not_taken", 0),
        ("predicted_taken", 1),
        ("predicted_not_taken", 2),
        ("predicted_targets", 1),
        ("predicted_target_matches", 1),
        ("predicted_target_mismatches", 0),
        ("resolved_targets", 3),
        ("mispredictions", 2),
        ("squashes", 2),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_event, field),
            value,
            "O3 event summary branch-event field {field}"
        );
    }
    for (path, value) in [
        ("/kind/direct_conditional", 2),
        ("/kind/direct_unconditional", 1),
        ("/taken_kind/direct_conditional", 2),
        ("/taken_kind/direct_unconditional", 1),
        ("/not_taken_kind/direct_conditional", 0),
        ("/predicted_taken_kind/direct_conditional", 1),
        ("/predicted_not_taken_kind/direct_conditional", 1),
        ("/predicted_target_kind/direct_conditional", 1),
        ("/predicted_target_match_kind/direct_conditional", 1),
        ("/predicted_target_mismatch_kind/direct_conditional", 0),
        ("/resolved_target_kind/direct_conditional", 2),
        ("/resolved_target_kind/direct_unconditional", 1),
        ("/misprediction_kind/direct_conditional", 1),
        ("/misprediction_kind/direct_unconditional", 1),
        ("/squash_kind/direct_conditional", 1),
        ("/squash_kind/direct_unconditional", 1),
    ] {
        assert_eq!(
            event_summary_branch_event
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary branch-event path {path}: {event_summary_branch_event}"
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 3),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 3),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_targets",
            "Count",
            branch_predicted_targets,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_kind.direct_conditional",
            "Count",
            branch_predicted_targets,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_matches",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 2),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 2),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_conditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_unconditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_match_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_unconditional",
            "Count",
            1,
        ),
        ("sim.cpu0.o3.branch_event.predicted_taken", "Count", 1),
        ("sim.cpu0.o3.branch_event.predicted_not_taken", "Count", 2),
        ("sim.cpu0.o3.branch_event.predicted_targets", "Count", 1),
        (
            "sim.cpu0.o3.branch_event.predicted_target_matches",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_target_mismatches",
            "Count",
            0,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_not_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_target_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_target_match_kind.direct_conditional",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_direct_conditional_branch_predicted_taken_not_taken_mismatch()
{
    let path = detailed_o3_branch_predicted_taken_not_taken_debug_binary(
        "debug-flags-o3-branch-predicted-taken-not-taken",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--debug-flags",
            "O3",
            "--dump-memory",
            "0x80000060:16",
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
        Some("0b000000000000000000000000000000")
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    let branch_repair = record
        .pointer("/branch_repair")
        .unwrap_or_else(|| panic!("missing O3 trace branch-repair summary: {record}"));
    assert_eq!(json_record_u64(branch_repair, "targetless_mismatches"), 1);
    assert_eq!(json_record_u64(branch_repair, "wrong_targets"), 0);
    assert_eq!(
        json_record_u64(branch_repair, "direction_only_mismatches"),
        2
    );
    assert_eq!(
        branch_repair
            .pointer("/targetless_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-repair summary should expose direct-conditional targetless repairs: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/direction_only_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(2),
        "O3 trace branch-repair summary should expose direct-unconditional direction-only repairs: {branch_repair}"
    );
    assert_eq!(
        branch_repair
            .pointer("/wrong_target_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-repair summary should include zero-valued wrong-target branch kinds: {branch_repair}"
    );
    let event_summary_branch_repair = record
        .pointer("/event_summary/branch_repair")
        .expect("O3 event summary branch-repair matrix");
    for (field, value) in [
        ("targetless_mismatches", 1),
        ("wrong_targets", 0),
        ("direction_only_mismatches", 2),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_repair, field),
            value,
            "O3 event summary targetless branch-repair field {field}"
        );
    }
    for (path, value) in [
        ("/targetless_mismatch_kind/direct_conditional", 1),
        ("/direction_only_kind/direct_unconditional", 2),
        ("/wrong_target_kind/direct_conditional", 0),
    ] {
        assert_eq!(
            event_summary_branch_repair
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary targetless branch-repair path {path}: {event_summary_branch_repair}"
        );
    }
    let branch_target_mismatch = record
        .pointer("/branch_target_mismatch")
        .unwrap_or_else(|| panic!("missing O3 trace branch-target mismatch summary: {record}"));
    for (field, value) in [
        ("targetless_mismatches", 1),
        ("targetless_mismatch_without_link_writes", 1),
        ("targetless_mismatch_squashed_targets", 1),
        ("targetless_mismatch_squashed_target_without_link_writes", 1),
        ("wrong_targets", 0),
        ("wrong_target_squashed_targets", 0),
        ("wrong_target_squashed_target_without_link_writes", 0),
        ("wrong_target_squashed_target_link_writes", 0),
        ("wrong_target_link_writes", 0),
        ("wrong_target_without_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(branch_target_mismatch, field),
            value,
            "O3 direct-conditional branch-target mismatch summary field {field}"
        );
    }
    assert_eq!(
        branch_target_mismatch
            .pointer("/targetless_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-target mismatch summary should expose direct-conditional targetless mismatches: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/targetless_mismatch_squashed_target_without_link_write_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-target mismatch summary should expose no-link direct-conditional targetless squashes: {branch_target_mismatch}"
    );
    assert_eq!(
        branch_target_mismatch
            .pointer("/wrong_target_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-target mismatch summary should include zero-valued wrong-target branch kinds: {branch_target_mismatch}"
    );
    let branch_direction_mismatch = record
        .pointer("/branch_direction_mismatch")
        .unwrap_or_else(|| panic!("missing O3 trace branch-direction-mismatch summary: {record}"));
    for (field, value) in [
        ("mismatches", 3),
        ("without_link_writes", 3),
        ("squashed_targets", 3),
        ("squashed_target_without_link_writes", 3),
        ("squashed_target_link_writes", 0),
    ] {
        assert_eq!(
            json_record_u64(branch_direction_mismatch, field),
            value,
            "O3 direct branch-direction-mismatch summary field {field}"
        );
    }
    assert_eq!(
        branch_direction_mismatch
            .pointer("/kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-direction-mismatch summary should expose direct-conditional mismatches: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(2),
        "O3 trace branch-direction-mismatch summary should expose direct-unconditional mismatches: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/without_link_write_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "O3 trace branch-direction-mismatch summary should expose no-link direct-conditional lanes: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/squashed_target_without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(2),
        "O3 trace branch-direction-mismatch summary should expose squashed no-link direct-unconditional lanes: {branch_direction_mismatch}"
    );
    assert_eq!(
        branch_direction_mismatch
            .pointer("/link_write_kind/call_indirect")
            .and_then(Value::as_u64),
        Some(0),
        "O3 trace branch-direction-mismatch summary should include zero-valued link-write branch kinds: {branch_direction_mismatch}"
    );
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 11);
    let event_pcs = events
        .iter()
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(
        event_pcs,
        [
            "0x80000018",
            "0x8000001c",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000020",
            "0x80000024",
            "0x80000028",
            "0x8000002c",
            "0x80000030",
            "0x80000034",
        ]
    );
    assert!(!event_pcs.contains(&"0x80000014"));
    let branch = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000008"))
        .unwrap_or_else(|| panic!("missing warmed O3 branch event: {events:?}"));
    assert_eq!(json_record_bool(branch, "branch_event"), true);
    assert_eq!(json_record_str(branch, "branch_kind"), "direct_conditional");
    assert_eq!(json_record_bool(branch, "branch_predicted_taken"), true);
    assert_eq!(json_record_bool(branch, "branch_resolved_taken"), false);
    assert_eq!(json_record_bool(branch, "branch_mispredicted"), true);
    assert_eq!(json_record_bool(branch, "branch_wrong_target"), false);
    assert_eq!(json_record_bool(branch, "branch_targetless_mismatch"), true);
    assert_eq!(
        json_record_str(branch, "branch_repair"),
        "targetless_mismatch"
    );
    assert_eq!(
        json_record_bool(branch, "branch_link_register_write"),
        false
    );
    assert_eq!(
        json_record_str(branch, "branch_predicted_target"),
        "0x80000014"
    );
    assert_eq!(branch.get("branch_resolved_target"), Some(&Value::Null));
    assert_eq!(json_record_bool(branch, "branch_squash"), true);
    assert_eq!(
        json_record_str(branch, "branch_squashed_target"),
        "0x80000014"
    );
    let mut direction_only_branch_pcs = events
        .iter()
        .filter(|event| {
            json_record_str(event, "branch_kind") == "direct_unconditional"
                && json_record_str(event, "branch_repair") == "direction_only"
        })
        .map(|event| json_record_str(event, "pc"))
        .collect::<Vec<_>>();
    direction_only_branch_pcs.sort_unstable();
    assert_eq!(direction_only_branch_pcs, ["0x80000010", "0x8000001c"]);

    let branch_event = json
        .pointer("/cores/0/o3_runtime/branch_event")
        .expect("structured O3 branch-event runtime matrix");
    assert_eq!(json_record_u64(branch_event, "branches"), 3);
    assert_eq!(json_record_u64(branch_event, "predicted_taken"), 1);
    assert_eq!(json_record_u64(branch_event, "predicted_not_taken"), 2);
    assert_eq!(json_record_u64(branch_event, "predicted_targets"), 1);
    assert_eq!(json_record_u64(branch_event, "predicted_target_matches"), 0);
    assert_eq!(
        json_record_u64(branch_event, "predicted_target_mismatches"),
        1
    );
    assert_eq!(
        branch_event
            .pointer("/not_taken_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose not-taken branches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/without_link_write_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose branches without link writes: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/without_link_write_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(2),
        "direct-unconditional runtime matrix should expose branches without link writes: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/predicted_target_mismatch_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose predicted target mismatches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/misprediction_kind/direct_conditional")
            .and_then(Value::as_u64),
        Some(1),
        "direct-conditional runtime matrix should expose mispredicted branches: {branch_event}"
    );
    assert_eq!(
        branch_event
            .pointer("/misprediction_kind/direct_unconditional")
            .and_then(Value::as_u64),
        Some(2),
        "direct-unconditional runtime matrix should expose mispredicted branches: {branch_event}"
    );
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    let event_summary_branch_event = event_summary
        .pointer("/branch_event")
        .expect("O3 event summary branch-event matrix");
    let event_summary_commit = event_summary
        .pointer("/commit")
        .expect("O3 event summary mismatch commit matrix");
    let event_branch_mispredictions = events
        .iter()
        .filter(|event| json_record_bool(event, "branch_mispredicted"))
        .count() as u64;
    assert!(
        event_branch_mispredictions > 0,
        "expected positive branch-mispredict events: {events:?}"
    );
    assert_eq!(
        json_record_u64(event_summary_commit, "branch_mispredicts"),
        event_branch_mispredictions,
        "O3 event summary commit branch mispredicts should derive from emitted branch events"
    );
    for (field, value) in [
        ("branches", 3),
        ("taken", 2),
        ("not_taken", 1),
        ("predicted_taken", 1),
        ("predicted_not_taken", 2),
        ("predicted_targets", 1),
        ("predicted_target_matches", 0),
        ("predicted_target_mismatches", 1),
        ("resolved_targets", 2),
        ("mispredictions", 3),
        ("squashes", 3),
    ] {
        assert_eq!(
            json_record_u64(event_summary_branch_event, field),
            value,
            "O3 event summary mismatch branch-event field {field}"
        );
    }
    for (path, value) in [
        ("/kind/direct_conditional", 1),
        ("/kind/direct_unconditional", 2),
        ("/taken_kind/direct_unconditional", 2),
        ("/not_taken_kind/direct_conditional", 1),
        ("/predicted_taken_kind/direct_conditional", 1),
        ("/predicted_not_taken_kind/direct_unconditional", 2),
        ("/predicted_target_kind/direct_conditional", 1),
        ("/predicted_target_match_kind/direct_conditional", 0),
        ("/predicted_target_mismatch_kind/direct_conditional", 1),
        ("/resolved_target_kind/direct_unconditional", 2),
        ("/misprediction_kind/direct_conditional", 1),
        ("/misprediction_kind/direct_unconditional", 2),
        ("/without_link_write_kind/direct_conditional", 1),
        ("/without_link_write_kind/direct_unconditional", 2),
        ("/squash_kind/direct_conditional", 1),
        ("/squash_kind/direct_unconditional", 2),
    ] {
        assert_eq!(
            event_summary_branch_event
                .pointer(path)
                .and_then(Value::as_u64),
            Some(value),
            "O3 event summary mismatch branch-event path {path}: {event_summary_branch_event}"
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.event.branches", "Count", 3),
        ("sim.debug.o3_trace.event.branch_taken", "Count", 2),
        ("sim.debug.o3_trace.event.branch_not_taken", "Count", 1),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_matches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_mismatches",
            "Count",
            1,
        ),
        ("sim.cpu0.o3.branch_event.predicted_taken", "Count", 1),
        (
            "sim.cpu0.o3.branch_event.predicted_not_taken",
            "Count",
            2,
        ),
        ("sim.cpu0.o3.branch_event.predicted_targets", "Count", 1),
        (
            "sim.cpu0.o3.branch_event.predicted_target_matches",
            "Count",
            0,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_target_mismatches",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.not_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.without_link_write_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.cpu0.o3.branch_event.predicted_target_mismatch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.misprediction_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.cpu0.o3.branch_event.misprediction_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatches",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_targets",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_writes",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_writes",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatches",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_without_link_writes",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_without_link_writes",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.branch_wrong_targets", "Count", 0),
        (
            "sim.debug.o3_trace.event.branch_repair_targetless_mismatches",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_mismatches",
            "Count",
            2,
        ),
        ("sim.debug.o3_trace.event.branch_mispredictions", "Count", 3),
        ("sim.debug.o3_trace.event.branch_squashes", "Count", 3),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_not_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_taken_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_match_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_mismatch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_write_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_write_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_write_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_write_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_without_link_write_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_targetless_mismatch_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_kind.direct_unconditional",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_misprediction_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_conditional",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.branch_squash_kind.direct_unconditional",
            "Count",
            2,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 16),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_emits_fu_latency_event_classes() {
    let path = detailed_o3_fu_latency_debug_binary("debug-flags-o3-fu-latency-runtime");

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
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];

    for (field, value) in [
        ("instructions", 5),
        ("rob_allocations", 5),
        ("rob_commits", 5),
        ("rename_writes", 4),
        ("lsq_loads", 0),
        ("lsq_stores", 0),
        ("fu_latency_instructions", 2),
        ("fu_latency_cycles", 21),
        ("fu_integer_mul_instructions", 1),
        ("fu_integer_mul_latency_cycles", 2),
        ("fu_integer_div_instructions", 1),
        ("fu_integer_div_latency_cycles", 19),
        ("max_rob_occupancy", 1),
        ("max_lsq_occupancy", 0),
        ("rename_map_entries", 3),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 trace field {field}"
        );
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 5);
    let event_ticks = events
        .iter()
        .map(|event| json_record_u64(event, "tick"))
        .collect::<Vec<_>>();
    assert!(
        event_ticks.windows(2).all(|window| window[0] < window[1]),
        "O3 event ticks should be strictly increasing: {event_ticks:?}"
    );
    assert_o3_event_with_fu(&events[0], 0, "0x80000004", 1, 0, 0, 0, None, false);
    assert_o3_event_with_fu(&events[1], 1, "0x80000008", 1, 0, 0, 0, None, false);
    assert_o3_event_with_fu(
        &events[2],
        2,
        "0x8000000c",
        1,
        0,
        0,
        2,
        Some("scalar_integer_mul"),
        false,
    );
    assert_o3_event_with_fu(
        &events[3],
        3,
        "0x80000010",
        1,
        0,
        0,
        19,
        Some("scalar_integer_div"),
        false,
    );
    assert_o3_event_with_fu(&events[4], 4, "0x80000014", 0, 0, 0, 0, None, true);

    let first_tick = event_ticks[0];
    let last_tick = *event_ticks.last().expect("O3 event tick");
    let tick_span = last_tick.saturating_sub(first_tick);
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    let event_fu_latency_instructions = events
        .iter()
        .filter(|event| json_record_u64(event, "fu_latency_cycles") > 0)
        .count() as u64;
    let event_fu_latency_cycles = events
        .iter()
        .map(|event| json_record_u64(event, "fu_latency_cycles"))
        .sum::<u64>();
    assert_eq!(
        json_record_u64(event_summary, "fu_latency_instructions"),
        event_fu_latency_instructions
    );
    assert_eq!(
        json_record_u64(event_summary, "fu_latency_cycles"),
        event_fu_latency_cycles
    );
    assert_eq!(json_record_u64(event_summary, "fu_latency_max_cycles"), 19);
    assert_eq!(json_record_u64(event_summary, "fu_latency_min_cycles"), 2);
    assert_eq!(json_record_u64(event_summary, "fu_latency_avg_cycles"), 10);
    let event_summary_fu_latency_class = event_summary
        .pointer("/fu_latency_class")
        .expect("O3 trace event summary FU latency class matrix");
    let event_summary_iq = event_summary
        .pointer("/iq")
        .expect("O3 trace event summary IQ matrix");
    let event_summary_issued_inst_type = event_summary_iq
        .pointer("/issued_inst_type")
        .expect("O3 trace event summary IQ issued-inst-type matrix");
    let event_summary_commit = event_summary
        .pointer("/commit")
        .expect("O3 trace event summary commit matrix");
    let event_summary_committed_inst_type = event_summary_commit
        .pointer("/committed_inst_type")
        .expect("O3 trace event summary commit committed-inst-type matrix");
    assert_eq!(
        json_record_u64(event_summary_iq, "insts_issued"),
        events.len() as u64
    );
    assert_eq!(json_record_u64(event_summary_iq, "mem_insts_issued"), 0);
    assert_eq!(json_record_u64(event_summary_iq, "branch_insts_issued"), 0);
    for (class, instructions, cycles) in [("integer_mul", 1, 2), ("integer_div", 1, 19)] {
        let class_summary = event_summary_fu_latency_class
            .pointer(&format!("/{class}"))
            .unwrap_or_else(|| {
                panic!(
                    "missing event summary FU latency class {class}: {event_summary_fu_latency_class:?}"
                )
            });
        assert_eq!(
            json_record_u64(class_summary, "instructions"),
            instructions,
            "event summary FU latency class {class} instructions"
        );
        assert_eq!(
            json_record_u64(class_summary, "cycles"),
            cycles,
            "event summary FU latency class {class} cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "max_cycles"),
            cycles,
            "event summary FU latency class {class} max cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "min_cycles"),
            cycles,
            "event summary FU latency class {class} min cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "avg_cycles"),
            cycles,
            "event summary FU latency class {class} avg cycles"
        );
    }
    for (trace_class, matrix_class) in [
        ("scalar_integer_mul", "int_mul"),
        ("scalar_integer_div", "int_div"),
    ] {
        let class_events = o3_event_fu_latency_class_count(events, trace_class);
        assert!(
            class_events > 0,
            "expected positive O3 event count for {trace_class}: {events:?}"
        );
        assert_eq!(
            event_summary_issued_inst_type
                .pointer(&format!("/{matrix_class}"))
                .and_then(Value::as_u64),
            Some(class_events),
            "event summary IQ issued-inst-type {matrix_class} should derive from emitted {trace_class} events: {event_summary_issued_inst_type}"
        );
        assert_eq!(
            event_summary_committed_inst_type
                .pointer(&format!("/{matrix_class}"))
                .and_then(Value::as_u64),
            Some(class_events),
            "event summary commit committed-inst-type {matrix_class} should derive from emitted {trace_class} events: {event_summary_committed_inst_type}"
        );
    }
    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.instructions", "Count", 5),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 2),
        ("sim.debug.o3_trace.fu_latency_cycles", "Cycle", 21),
        ("sim.debug.o3_trace.fu_integer_mul_instructions", "Count", 1),
        (
            "sim.debug.o3_trace.fu_integer_mul_latency_cycles",
            "Cycle",
            2,
        ),
        ("sim.debug.o3_trace.fu_integer_div_instructions", "Count", 1),
        (
            "sim.debug.o3_trace.fu_integer_div_latency_cycles",
            "Cycle",
            19,
        ),
        ("sim.debug.o3_trace.event.records", "Count", 5),
        ("sim.debug.o3_trace.event.first_tick", "Tick", first_tick),
        ("sim.debug.o3_trace.event.last_tick", "Tick", last_tick),
        ("sim.debug.o3_trace.event.tick_span", "Tick", tick_span),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            2,
        ),
        ("sim.debug.o3_trace.event.fu_latency_cycles", "Cycle", 21),
        (
            "sim.debug.o3_trace.event.fu_latency_max_cycles",
            "Cycle",
            19,
        ),
        ("sim.debug.o3_trace.event.fu_latency_min_cycles", "Cycle", 2),
        (
            "sim.debug.o3_trace.event.fu_latency_avg_cycles",
            "Cycle",
            10,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_max_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_min_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_avg_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_max_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_min_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_avg_cycles",
            "Cycle",
            19,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_vector_integer_fu_latency_events() {
    let path =
        detailed_o3_vector_fu_latency_debug_binary("debug-flags-o3-vector-fu-latency-runtime");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert!(
        events.len() >= 5,
        "expected vector FU latency events: {events:?}"
    );

    let vector_mul = events
        .iter()
        .find(|event| json_record_str(event, "pc") == "0x8000000c")
        .unwrap_or_else(|| panic!("missing vector mul O3 event: {events:?}"));
    let vector_div = events
        .iter()
        .find(|event| json_record_str(event, "pc") == "0x80000010")
        .unwrap_or_else(|| panic!("missing vector div O3 event: {events:?}"));
    let vector_mul_latency = json_record_u64(vector_mul, "fu_latency_cycles");
    let vector_div_latency = json_record_u64(vector_div, "fu_latency_cycles");
    assert_o3_event_with_fu(
        vector_mul,
        2,
        "0x8000000c",
        0,
        0,
        0,
        vector_mul_latency,
        Some("vector_integer_mul"),
        false,
    );
    assert_o3_event_with_fu(
        vector_div,
        3,
        "0x80000010",
        0,
        0,
        0,
        vector_div_latency,
        Some("vector_integer_div"),
        false,
    );
    assert!(vector_mul_latency > 0, "{vector_mul:?}");
    assert!(
        vector_div_latency > vector_mul_latency,
        "vector div should model a longer FU latency than vector mul: mul={vector_mul_latency}, div={vector_div_latency}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 2),
        (
            "sim.debug.o3_trace.fu_latency_cycles",
            "Cycle",
            vector_mul_latency + vector_div_latency,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            vector_mul_latency + vector_div_latency,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            vector_mul_latency,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            vector_div_latency,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_vector_mul_family_fu_latency_events() {
    let path = detailed_o3_vector_mul_family_fu_latency_debug_binary(
        "debug-flags-o3-vector-mul-family-fu-latency-runtime",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let expected_events = [("0x8000000c", 2), ("0x80000010", 3), ("0x80000014", 4)];
    let mut vector_mul_cycles = 0;
    for (pc, sequence) in expected_events {
        let event = events
            .iter()
            .find(|event| json_record_str(event, "pc") == pc)
            .unwrap_or_else(|| {
                panic!("missing vector multiply-family O3 event at {pc}: {events:?}")
            });
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        vector_mul_cycles += latency;
        assert_o3_event_with_fu(
            event,
            sequence,
            pc,
            0,
            0,
            0,
            latency,
            Some("vector_integer_mul"),
            false,
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 3),
        (
            "sim.debug.o3_trace.fu_latency_cycles",
            "Cycle",
            vector_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            vector_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            3,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            vector_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_vector_saturating_mul_fu_latency_events() {
    let path = detailed_o3_vector_saturating_mul_fu_latency_debug_binary(
        "debug-flags-o3-vector-saturating-mul-fu-latency-runtime",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    let expected_events = [("0x80000010", 3), ("0x80000014", 4)];
    let mut vector_mul_cycles = 0;
    for (pc, sequence) in expected_events {
        let event = events
            .iter()
            .find(|event| json_record_str(event, "pc") == pc)
            .unwrap_or_else(|| {
                panic!("missing vector saturating multiply O3 event at {pc}: {events:?}")
            });
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        vector_mul_cycles += latency;
        assert_o3_event_with_fu(
            event,
            sequence,
            pc,
            0,
            0,
            0,
            latency,
            Some("vector_integer_mul"),
            false,
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 2),
        (
            "sim.debug.o3_trace.fu_latency_cycles",
            "Cycle",
            vector_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            vector_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            vector_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_fu_latency_events() {
    let path = detailed_o3_float_fu_latency_debug_binary("debug-flags-o3-float-fu-latency-runtime");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
    let record = json
        .pointer("/debug/o3_trace/0")
        .expect("first O3 trace record");
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 4);
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");

    let expected_events = [
        ("0x80000020", 0, "scalar_float_mul", 1),
        ("0x80000024", 1, "scalar_float_div", 1),
        ("0x80000028", 2, "vector_float_mul", 0),
        ("0x8000002c", 3, "vector_float_div", 0),
    ];
    let mut scalar_float_mul_cycles = 0;
    let mut scalar_float_div_cycles = 0;
    let mut vector_float_mul_cycles = 0;
    let mut vector_float_div_cycles = 0;
    for (pc, sequence, class, rename_writes) in expected_events {
        let event = events
            .iter()
            .find(|event| json_record_str(event, "pc") == pc)
            .unwrap_or_else(|| panic!("missing float FU latency O3 event at {pc}: {events:?}"));
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        match class {
            "scalar_float_mul" => scalar_float_mul_cycles += latency,
            "scalar_float_div" => scalar_float_div_cycles += latency,
            "vector_float_mul" => vector_float_mul_cycles += latency,
            "vector_float_div" => vector_float_div_cycles += latency,
            _ => unreachable!("covered class literal"),
        }
        assert_o3_event_with_fu(
            event,
            sequence,
            pc,
            rename_writes,
            0,
            0,
            latency,
            Some(class),
            false,
        );
    }
    let float_cycles = scalar_float_mul_cycles
        + scalar_float_div_cycles
        + vector_float_mul_cycles
        + vector_float_div_cycles;
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        float_cycles,
        "{record:?}"
    );
    let runtime = json
        .pointer("/cores/0/o3_runtime")
        .expect("runtime O3 summary");
    let fu_latency_class = record
        .pointer("/fu_latency_class")
        .expect("debug O3 trace FU latency class matrix");
    let expected_fu_classes = [
        "integer_mul",
        "integer_div",
        "float_add",
        "float_compare",
        "float_misc",
        "float_mul",
        "float_fma",
        "float_div",
        "float_sqrt",
        "vector_integer_mul",
        "vector_integer_div",
        "vector_float_add",
        "vector_float_compare",
        "vector_float_misc",
        "vector_float_mul",
        "vector_float_fma",
        "vector_float_div",
        "vector_float_sqrt",
    ];
    let fu_latency_class_object = fu_latency_class
        .as_object()
        .expect("debug O3 trace FU latency class object");
    assert_eq!(
        fu_latency_class_object.len(),
        expected_fu_classes.len(),
        "debug O3 trace FU latency class fixed axis: {fu_latency_class:?}"
    );
    for class in expected_fu_classes {
        assert!(
            fu_latency_class_object.contains_key(class),
            "debug O3 trace FU latency class should include {class}: {fu_latency_class:?}"
        );
    }
    for (class, instructions, cycles) in [
        ("float_mul", 1, scalar_float_mul_cycles),
        ("float_div", 1, scalar_float_div_cycles),
        ("vector_float_mul", 1, vector_float_mul_cycles),
        ("vector_float_div", 1, vector_float_div_cycles),
        ("float_add", 0, 0),
        ("integer_mul", 0, 0),
        ("integer_div", 0, 0),
        ("vector_integer_mul", 0, 0),
    ] {
        let class_summary = fu_latency_class
            .pointer(&format!("/{class}"))
            .unwrap_or_else(|| panic!("missing FU latency class {class}: {fu_latency_class:?}"));
        assert_eq!(
            json_record_u64(class_summary, "instructions"),
            instructions,
            "O3 trace FU latency class {class} instructions"
        );
        assert_eq!(
            json_record_u64(class_summary, "cycles"),
            cycles,
            "O3 trace FU latency class {class} cycles"
        );
        assert_eq!(
            json_record_u64(class_summary, "instructions"),
            json_record_u64(runtime, &format!("fu_{class}_instructions")),
            "O3 trace should mirror runtime FU class {class} instructions"
        );
        assert_eq!(
            json_record_u64(class_summary, "cycles"),
            json_record_u64(runtime, &format!("fu_{class}_latency_cycles")),
            "O3 trace should mirror runtime FU class {class} cycles"
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 4),
        (
            "sim.debug.o3_trace.fu_latency_cycles",
            "Cycle",
            float_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            4,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            float_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_latency_cycles",
            "Cycle",
            scalar_float_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_latency_cycles",
            "Cycle",
            scalar_float_div_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_latency_cycles",
            "Cycle",
            vector_float_mul_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_latency_cycles",
            "Cycle",
            vector_float_div_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_extended_float_fu_latency_events() {
    let path = detailed_o3_float_extended_fu_latency_debug_binary(
        "debug-flags-o3-float-extended-fu-latency-runtime",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
    let record = json
        .pointer("/debug/o3_trace/0")
        .expect("first O3 trace record");
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 6);
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");

    let expected_events = [
        ("0x80000028", 0, "scalar_float_add", 1),
        ("0x8000002c", 1, "scalar_float_fma", 1),
        ("0x80000030", 2, "scalar_float_sqrt", 1),
        ("0x80000034", 3, "vector_float_add", 0),
        ("0x80000038", 4, "vector_float_fma", 0),
        ("0x8000003c", 5, "vector_float_sqrt", 0),
    ];
    let mut scalar_float_add_cycles = 0;
    let mut scalar_float_fma_cycles = 0;
    let mut scalar_float_sqrt_cycles = 0;
    let mut vector_float_add_cycles = 0;
    let mut vector_float_fma_cycles = 0;
    let mut vector_float_sqrt_cycles = 0;
    for (pc, sequence, class, rename_writes) in expected_events {
        let event = events
            .iter()
            .find(|event| json_record_str(event, "pc") == pc)
            .unwrap_or_else(|| {
                panic!("missing extended float FU latency O3 event at {pc}: {events:?}")
            });
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        match class {
            "scalar_float_add" => scalar_float_add_cycles += latency,
            "scalar_float_fma" => scalar_float_fma_cycles += latency,
            "scalar_float_sqrt" => scalar_float_sqrt_cycles += latency,
            "vector_float_add" => vector_float_add_cycles += latency,
            "vector_float_fma" => vector_float_fma_cycles += latency,
            "vector_float_sqrt" => vector_float_sqrt_cycles += latency,
            _ => unreachable!("covered class literal"),
        }
        assert_o3_event_with_fu(
            event,
            sequence,
            pc,
            rename_writes,
            0,
            0,
            latency,
            Some(class),
            false,
        );
    }
    let float_cycles = scalar_float_add_cycles
        + scalar_float_fma_cycles
        + scalar_float_sqrt_cycles
        + vector_float_add_cycles
        + vector_float_fma_cycles
        + vector_float_sqrt_cycles;
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        float_cycles,
        "{record:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 6),
        (
            "sim.debug.o3_trace.fu_latency_cycles",
            "Cycle",
            float_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            6,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            float_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_add_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_add_latency_cycles",
            "Cycle",
            scalar_float_add_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_fma_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_fma_latency_cycles",
            "Cycle",
            scalar_float_fma_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_sqrt_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_sqrt_latency_cycles",
            "Cycle",
            scalar_float_sqrt_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_add_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_add_latency_cycles",
            "Cycle",
            vector_float_add_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_fma_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_fma_latency_cycles",
            "Cycle",
            vector_float_fma_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_sqrt_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_sqrt_latency_cycles",
            "Cycle",
            vector_float_sqrt_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_compare_fu_latency_events() {
    let path =
        detailed_o3_float_compare_fu_latency_debug_binary("debug-flags-o3-float-compare-runtime");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
    let record = json
        .pointer("/debug/o3_trace/0")
        .expect("first O3 trace record");
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 2);
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");

    let expected_events = [
        ("0x80000020", 0, "scalar_float_compare", 1),
        ("0x80000024", 1, "vector_float_compare", 0),
    ];
    let mut scalar_float_compare_cycles = 0;
    let mut vector_float_compare_cycles = 0;
    for (pc, sequence, class, rename_writes) in expected_events {
        let event = events
            .iter()
            .find(|event| json_record_str(event, "pc") == pc)
            .unwrap_or_else(|| panic!("missing float compare O3 event at {pc}: {events:?}"));
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        match class {
            "scalar_float_compare" => scalar_float_compare_cycles += latency,
            "vector_float_compare" => vector_float_compare_cycles += latency,
            _ => unreachable!("covered class literal"),
        }
        assert_o3_event_with_fu(
            event,
            sequence,
            pc,
            rename_writes,
            0,
            0,
            latency,
            Some(class),
            false,
        );
    }
    let compare_cycles = scalar_float_compare_cycles + vector_float_compare_cycles;
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        compare_cycles,
        "{record:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 2),
        (
            "sim.debug.o3_trace.fu_latency_cycles",
            "Cycle",
            compare_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            compare_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_compare_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_compare_latency_cycles",
            "Cycle",
            scalar_float_compare_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_compare_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_compare_latency_cycles",
            "Cycle",
            vector_float_compare_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_add_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_add_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_fma_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_fma_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_sqrt_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_sqrt_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_add_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_add_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_fma_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_fma_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_sqrt_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_sqrt_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_float_misc_fu_latency_events() {
    let path = detailed_o3_float_misc_fu_latency_debug_binary("debug-flags-o3-float-misc-runtime");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
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
    let record = json
        .pointer("/debug/o3_trace/0")
        .expect("first O3 trace record");
    assert_eq!(json_record_u64(record, "fu_latency_instructions"), 4);
    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");

    let expected_events = [
        ("0x80000024", 0, "scalar_float_misc", 1),
        ("0x80000028", 1, "scalar_float_misc", 1),
        ("0x8000002c", 2, "vector_float_misc", 0),
        ("0x80000030", 3, "vector_float_misc", 0),
    ];
    let mut scalar_float_misc_cycles = 0;
    let mut vector_float_misc_cycles = 0;
    let mut scalar_float_misc_latencies = Vec::new();
    let mut vector_float_misc_latencies = Vec::new();
    for (pc, sequence, class, rename_writes) in expected_events {
        let event = events
            .iter()
            .find(|event| json_record_str(event, "pc") == pc)
            .unwrap_or_else(|| panic!("missing float misc O3 event at {pc}: {events:?}"));
        let latency = json_record_u64(event, "fu_latency_cycles");
        assert!(latency > 0, "{event:?}");
        match class {
            "scalar_float_misc" => {
                scalar_float_misc_cycles += latency;
                scalar_float_misc_latencies.push(latency);
            }
            "vector_float_misc" => {
                vector_float_misc_cycles += latency;
                vector_float_misc_latencies.push(latency);
            }
            _ => unreachable!("covered class literal"),
        }
        assert_o3_event_with_fu(
            event,
            sequence,
            pc,
            rename_writes,
            0,
            0,
            latency,
            Some(class),
            false,
        );
    }
    let scalar_float_misc_count = scalar_float_misc_latencies.len() as u64;
    let vector_float_misc_count = vector_float_misc_latencies.len() as u64;
    let scalar_float_misc_max = scalar_float_misc_latencies.iter().copied().max().unwrap();
    let scalar_float_misc_min = scalar_float_misc_latencies.iter().copied().min().unwrap();
    let vector_float_misc_max = vector_float_misc_latencies.iter().copied().max().unwrap();
    let vector_float_misc_min = vector_float_misc_latencies.iter().copied().min().unwrap();
    let misc_cycles = scalar_float_misc_cycles + vector_float_misc_cycles;
    assert_eq!(
        json_record_u64(record, "fu_latency_cycles"),
        misc_cycles,
        "{record:?}"
    );

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 4),
        ("sim.debug.o3_trace.fu_latency_cycles", "Cycle", misc_cycles),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            4,
        ),
        (
            "sim.debug.o3_trace.event.fu_latency_cycles",
            "Cycle",
            misc_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_misc_instructions",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_misc_latency_cycles",
            "Cycle",
            scalar_float_misc_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_misc_latency_max_cycles",
            "Cycle",
            scalar_float_misc_max,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_misc_latency_min_cycles",
            "Cycle",
            scalar_float_misc_min,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_misc_latency_avg_cycles",
            "Cycle",
            scalar_float_misc_cycles / scalar_float_misc_count,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_misc_instructions",
            "Count",
            2,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_misc_latency_cycles",
            "Cycle",
            vector_float_misc_cycles,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_misc_latency_max_cycles",
            "Cycle",
            vector_float_misc_max,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_misc_latency_min_cycles",
            "Cycle",
            vector_float_misc_min,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_misc_latency_avg_cycles",
            "Cycle",
            vector_float_misc_cycles / vector_float_misc_count,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_add_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_add_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_compare_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_compare_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_fma_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_fma_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_sqrt_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_float_sqrt_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_add_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_add_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_compare_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_compare_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_fma_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_fma_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_sqrt_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_float_sqrt_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_emits_store_forwarding_events() {
    let path = detailed_o3_store_forwarding_debug_binary("debug-flags-o3-store-forwarding-runtime");

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
            "--debug-flags",
            "O3",
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
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];

    for (field, value) in [
        ("instructions", 7),
        ("rob_allocations", 7),
        ("rob_commits", 7),
        ("rename_writes", 4),
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("store_load_forwarding_candidates", 1),
        ("store_load_forwarding_matches", 1),
        ("fu_latency_instructions", 0),
        ("fu_latency_cycles", 0),
        ("max_rob_occupancy", 1),
        ("max_lsq_occupancy", 1),
        ("rename_map_entries", 3),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 trace field {field}"
        );
    }
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    for (field, value) in [
        ("store_load_forwarding_candidates", 1),
        ("store_load_forwarding_matches", 1),
        ("store_load_forwarding_suppressed", 0),
        ("store_load_forwarding_address_mismatches", 0),
        ("store_load_forwarding_byte_mismatches", 0),
    ] {
        assert_eq!(
            json_record_u64(event_summary, field),
            value,
            "O3 event summary forwarding field {field}"
        );
    }
    for (operation, expected) in [("load", 1), ("store", 0)] {
        let operation_summary = event_summary
            .pointer(&format!("/lsq_operation/{operation}"))
            .unwrap_or_else(|| {
                panic!("missing event summary LSQ operation {operation}: {event_summary:?}")
            });
        for (field, value) in [
            ("forwarding_candidates", expected),
            ("forwarding_matches", expected),
            ("forwarding_suppressed", 0),
            ("forwarding_address_mismatches", 0),
            ("forwarding_byte_mismatches", 0),
        ] {
            assert_eq!(
                json_record_u64(operation_summary, field),
                value,
                "O3 event summary {operation} forwarding field {field}"
            );
        }
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 7);
    assert_o3_event_with_store_forwarding(
        &events[0],
        0,
        "0x80000004",
        1,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(
        &events[1],
        1,
        "0x80000008",
        1,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(
        &events[2],
        2,
        "0x8000000c",
        1,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(
        &events[3],
        3,
        "0x80000010",
        0,
        0,
        1,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(&events[4], 4, "0x80000014", 1, 1, 0, true, true, false);
    assert_eq!(
        json_record_str(&events[3], "lsq_store_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(&events[3], "lsq_store_bytes"), 4);
    assert_eq!(
        json_record_str(&events[4], "lsq_load_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(&events[4], "lsq_load_bytes"), 4);
    assert_o3_event_with_store_forwarding(
        &events[5],
        5,
        "0x80000018",
        0,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(&events[6], 6, "0x8000001c", 0, 0, 0, false, false, true);

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.instructions", "Count", 7),
        ("sim.debug.o3_trace.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.lsq_stores", "Count", 1),
        (
            "sim.debug.o3_trace.store_load_forwarding_candidates",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.store_load_forwarding_matches",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.records", "Count", 7),
        ("sim.debug.o3_trace.event.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_stores", "Count", 1),
        (
            "sim.debug.o3_trace.event.store_load_forwarding_candidates",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.store_load_forwarding_matches",
            "Count",
            1,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_marks_store_forwarding_suppression_reasons() {
    for (path, load_address, load_bytes, address_mismatches, byte_mismatches) in [
        (
            detailed_o3_store_forwarding_mismatch_debug_binary(
                "debug-flags-o3-store-forwarding-address-mismatch",
            ),
            "0x80000044",
            4,
            1,
            0,
        ),
        (
            detailed_o3_store_forwarding_byte_mismatch_debug_binary(
                "debug-flags-o3-store-forwarding-byte-mismatch",
            ),
            "0x80000040",
            1,
            0,
            1,
        ),
        (
            detailed_o3_store_forwarding_address_and_byte_mismatch_debug_binary(
                "debug-flags-o3-store-forwarding-address-byte-mismatch",
            ),
            "0x80000044",
            1,
            1,
            0,
        ),
    ] {
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
                "--debug-flags",
                "O3",
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
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .expect("debug O3 trace array");
        assert_eq!(trace.len(), 1);
        let record = &trace[0];
        for (field, value) in [
            ("store_load_forwarding_candidates", 0),
            ("store_load_forwarding_matches", 0),
            ("store_load_forwarding_suppressed", 1),
            (
                "store_load_forwarding_address_mismatches",
                address_mismatches,
            ),
            ("store_load_forwarding_byte_mismatches", byte_mismatches),
        ] {
            assert_eq!(
                json_record_u64(record, field),
                value,
                "O3 trace field {field}"
            );
        }
        let event_summary = record
            .pointer("/event_summary")
            .expect("O3 trace event summary should be embedded with the trace record");
        for (field, value) in [
            ("store_load_forwarding_candidates", 0),
            ("store_load_forwarding_matches", 0),
            ("store_load_forwarding_suppressed", 1),
            (
                "store_load_forwarding_address_mismatches",
                address_mismatches,
            ),
            ("store_load_forwarding_byte_mismatches", byte_mismatches),
        ] {
            assert_eq!(
                json_record_u64(event_summary, field),
                value,
                "O3 event summary forwarding field {field}"
            );
        }
        let load_summary = event_summary
            .pointer("/lsq_operation/load")
            .expect("event summary LSQ load lane");
        for (field, value) in [
            ("forwarding_candidates", 0),
            ("forwarding_matches", 0),
            ("forwarding_suppressed", 1),
            ("forwarding_address_mismatches", address_mismatches),
            ("forwarding_byte_mismatches", byte_mismatches),
        ] {
            assert_eq!(
                json_record_u64(load_summary, field),
                value,
                "O3 event summary load forwarding field {field}"
            );
        }
        let store_summary = event_summary
            .pointer("/lsq_operation/store")
            .expect("event summary LSQ store lane");
        for field in [
            "forwarding_candidates",
            "forwarding_matches",
            "forwarding_suppressed",
            "forwarding_address_mismatches",
            "forwarding_byte_mismatches",
        ] {
            assert_eq!(
                json_record_u64(store_summary, field),
                0,
                "O3 event summary store forwarding field {field}"
            );
        }

        let events = record
            .pointer("/events")
            .and_then(Value::as_array)
            .expect("O3 trace events array");
        assert_eq!(events.len(), 7);
        assert_o3_event_with_store_forwarding(
            &events[3],
            3,
            "0x80000010",
            0,
            0,
            1,
            false,
            false,
            false,
        );
        assert_o3_event_with_store_forwarding(
            &events[4],
            4,
            "0x80000014",
            1,
            1,
            0,
            false,
            false,
            false,
        );
        assert_eq!(
            json_record_str(&events[3], "lsq_store_address"),
            "0x80000040"
        );
        assert_eq!(json_record_u64(&events[3], "lsq_store_bytes"), 4);
        assert_eq!(
            json_record_str(&events[4], "lsq_load_address"),
            load_address
        );
        assert_eq!(json_record_u64(&events[4], "lsq_load_bytes"), load_bytes);
        for field in [
            "store_load_forwarding_suppressed",
            "store_load_forwarding_address_mismatch",
            "store_load_forwarding_byte_mismatch",
        ] {
            assert_eq!(json_record_bool(&events[3], field), false);
        }
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_suppressed"),
            true
        );
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_address_mismatch"),
            address_mismatches == 1
        );
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_byte_mismatch"),
            byte_mismatches == 1
        );

        for (path, unit, value) in [
            ("sim.debug.o3_trace.records", "Count", 1),
            ("sim.debug.o3_trace.instructions", "Count", 7),
            ("sim.debug.o3_trace.lsq_loads", "Count", 1),
            ("sim.debug.o3_trace.lsq_stores", "Count", 1),
            (
                "sim.debug.o3_trace.store_load_forwarding_candidates",
                "Count",
                0,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_matches",
                "Count",
                0,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_suppressed",
                "Count",
                1,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_address_mismatches",
                "Count",
                address_mismatches,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_byte_mismatches",
                "Count",
                byte_mismatches,
            ),
            ("sim.debug.o3_trace.event.records", "Count", 7),
            ("sim.debug.o3_trace.event.lsq_loads", "Count", 1),
            ("sim.debug.o3_trace.event.lsq_stores", "Count", 1),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_candidates",
                "Count",
                0,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_matches",
                "Count",
                0,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_suppressed",
                "Count",
                1,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_address_mismatches",
                "Count",
                address_mismatches,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_byte_mismatches",
                "Count",
                byte_mismatches,
            ),
        ] {
            assert_stat(&stdout, path, unit, value, "monotonic");
        }
    }
}

#[test]
fn rem6_run_o3_debug_flag_sums_multicore_runtime_trace_stats() {
    let path = detailed_o3_runtime_debug_binary("debug-flags-o3-multicore-runtime");

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
            "--debug-flags",
            "O3",
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
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 2);
    let mut all_event_ticks = Vec::new();
    for (record, cpu) in trace.iter().zip([0, 1]) {
        assert_eq!(json_record_u64(record, "cpu"), cpu);
        assert_eq!(json_record_u64(record, "instructions"), 6);
        assert_eq!(json_record_u64(record, "rob_allocations"), 6);
        assert_eq!(json_record_u64(record, "rob_commits"), 6);
        assert_eq!(json_record_u64(record, "rename_writes"), 4);
        assert_eq!(json_record_u64(record, "lsq_loads"), 1);
        assert_eq!(json_record_u64(record, "lsq_stores"), 1);
        assert_eq!(json_record_u64(record, "max_rob_occupancy"), 1);
        assert_eq!(json_record_u64(record, "max_lsq_occupancy"), 1);
        assert_eq!(json_record_u64(record, "rename_map_entries"), 3);
        let events = record
            .pointer("/events")
            .and_then(Value::as_array)
            .expect("O3 trace events array");
        let event_ticks = events
            .iter()
            .map(|event| json_record_u64(event, "tick"))
            .collect::<Vec<_>>();
        assert!(
            event_ticks.windows(2).all(|window| window[0] < window[1]),
            "O3 event ticks should be strictly increasing per CPU: {event_ticks:?}"
        );
        let first_event_tick = *event_ticks.first().expect("per-CPU O3 event tick");
        let last_event_tick = *event_ticks.last().expect("per-CPU O3 event tick");
        let event_tick_span = last_event_tick.saturating_sub(first_event_tick);
        for (suffix, unit, value) in [
            ("records", "Count", 1),
            ("instructions", "Count", 6),
            ("rob_allocations", "Count", 6),
            ("rob_commits", "Count", 6),
            ("rename_writes", "Count", 4),
            ("lsq_loads", "Count", 1),
            ("lsq_stores", "Count", 1),
            ("event.records", "Count", events.len() as u64),
            ("event.first_tick", "Tick", first_event_tick),
            ("event.last_tick", "Tick", last_event_tick),
            ("event.tick_span", "Tick", event_tick_span),
        ] {
            assert_stat(
                &stdout,
                &format!("sim.debug.o3_trace.cpu.cpu{cpu}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
        all_event_ticks.extend(event_ticks);
    }
    let first_tick = *all_event_ticks.iter().min().expect("O3 event tick");
    let last_tick = *all_event_ticks.iter().max().expect("O3 event tick");
    let tick_span = last_tick.saturating_sub(first_tick);

    for (path, unit, value) in [
        ("sim.debug.trace.records", "Count", 2),
        ("sim.debug.trace.categories", "Count", 1),
        ("sim.debug.trace.active_flags", "Count", 1),
        ("sim.debug.o3_trace.records", "Count", 2),
        ("sim.debug.o3_trace.instructions", "Count", 12),
        ("sim.debug.o3_trace.rob_allocations", "Count", 12),
        ("sim.debug.o3_trace.rob_commits", "Count", 12),
        ("sim.debug.o3_trace.rename_writes", "Count", 8),
        ("sim.debug.o3_trace.lsq_loads", "Count", 2),
        ("sim.debug.o3_trace.lsq_stores", "Count", 2),
        ("sim.debug.o3_trace.max_rob_occupancy", "Count", 1),
        ("sim.debug.o3_trace.max_lsq_occupancy", "Count", 1),
        ("sim.debug.o3_trace.rename_map_entries", "Count", 6),
        ("sim.debug.o3_trace.event.records", "Count", 12),
        ("sim.debug.o3_trace.event.first_tick", "Tick", first_tick),
        ("sim.debug.o3_trace.event.last_tick", "Tick", last_tick),
        ("sim.debug.o3_trace.event.tick_span", "Tick", tick_span),
        ("sim.debug.o3_trace.event.rob_allocations", "Count", 12),
        ("sim.debug.o3_trace.event.rob_commits", "Count", 12),
        ("sim.debug.o3_trace.event.rename_writes", "Count", 8),
        ("sim.debug.o3_trace.event.lsq_loads", "Count", 2),
        ("sim.debug.o3_trace.event.lsq_stores", "Count", 2),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_labels_hart_targeted_detailed_mode_trace() {
    let path = hart1_detailed_o3_debug_binary("debug-flags-o3-hart1-detailed-mode-authority");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--debug-flags",
            "O3",
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
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(
        trace.len(),
        1,
        "only hart 1 should emit O3 trace: {trace:?}"
    );
    let record = &trace[0];
    assert_eq!(json_record_u64(record, "cpu"), 1);
    assert_eq!(
        record.get("target").and_then(Value::as_str),
        Some("cpu1"),
        "O3 trace should expose the execution-mode target: {record}"
    );
    assert_eq!(
        record.get("execution_mode").and_then(Value::as_str),
        Some("detailed"),
        "O3 trace should expose the detailed-mode authority: {record}"
    );
    assert_eq!(json_record_u64(record, "fu_integer_mul_instructions"), 1);
    assert_eq!(json_record_u64(record, "fu_integer_div_instructions"), 1);
    assert_eq!(json_record_u64(record, "lsq_load_bytes"), 4);
    assert_eq!(json_record_u64(record, "lsq_store_bytes"), 4);

    let modes = json
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .expect("final execution-mode authority array");
    assert_eq!(modes.len(), 1, "final execution-mode authority: {modes:?}");
    assert_eq!(
        modes[0].pointer("/target").and_then(Value::as_str),
        Some("cpu1")
    );
    assert_eq!(
        modes[0].pointer("/mode").and_then(Value::as_str),
        Some("detailed")
    );
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json.pointer("/cores/1/o3_runtime").is_some());
    for path in ["iq", "iew", "rob", "rename"] {
        assert_eq!(
            record.pointer(&format!("/{path}")),
            json.pointer(&format!("/cores/1/o3_runtime/{path}")),
            "O3 trace should mirror the runtime {path} summary matrix"
        );
    }
    assert_eq!(
        json_record_u64(record.pointer("/iq").unwrap(), "insts_issued"),
        json_record_u64(record, "instructions")
    );
    assert_eq!(
        json_record_u64(record.pointer("/iew").unwrap(), "writeback_count"),
        json_record_u64(record, "instructions")
    );
    assert_eq!(
        json_record_u64(record.pointer("/rob").unwrap(), "commits"),
        json_record_u64(record, "rob_commits")
    );
    assert_eq!(
        json_record_u64(record.pointer("/rename").unwrap(), "writes"),
        json_record_u64(record, "rename_writes")
    );
    assert_eq!(
        record.pointer("/commit"),
        json.pointer("/cores/1/o3_runtime/commit"),
        "O3 trace should mirror the runtime commit summary matrix"
    );
    let committed_inst_type = record
        .pointer("/commit/committed_inst_type")
        .expect("debug O3 trace commit instruction-type matrix");
    for (field, value) in [
        ("mem_read", 1),
        ("mem_write", 1),
        ("int_mul", 1),
        ("int_div", 1),
        ("float_misc", 0),
        ("vector_integer_mul", 0),
        ("vector_float_misc", 0),
    ] {
        assert_eq!(
            json_record_u64(committed_inst_type, field),
            value,
            "O3 trace commit committed_inst_type.{field}"
        );
    }

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.instructions", "Count", 9),
        ("sim.debug.o3_trace.fu_integer_mul_instructions", "Count", 1),
        ("sim.debug.o3_trace.fu_integer_div_instructions", "Count", 1),
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 4),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 4),
        (
            "sim.debug.o3_trace.execution_mode_authority.targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.execution_mode_authority.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.execution_mode_authority.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.execution_mode_authority.mode.detailed",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.execution_mode_authority.target.cpu1.mode.detailed",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.execution_mode_authority.target.cpu1.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.execution_mode_authority.target.cpu1.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.targets",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.mode.detailed",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.target.cpu1.mode.functional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.target.cpu1.mode.timing",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.target.cpu1.mode.detailed",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.records", "Count", 9),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_omits_timing_mode_runtime_trace() {
    let path = detailed_o3_runtime_debug_binary("debug-flags-o3-timing-runtime");

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
            "--m5-switch-cpu-mode",
            "timing",
            "--debug-flags",
            "O3",
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
        Some(&vec![Value::String("O3".to_string())])
    );
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert!(trace.is_empty(), "timing-mode O3 trace: {trace:?}");

    assert_stat(&stdout, "sim.debug.flags", "Count", 1, "constant");
    for (path, unit, value) in [
        ("sim.debug.trace.records", "Count", 0),
        ("sim.debug.trace.categories", "Count", 0),
        ("sim.debug.trace.active_flags", "Count", 0),
        ("sim.debug.o3_trace.records", "Count", 0),
        ("sim.debug.o3_trace.stats_epoch", "Count", 0),
        ("sim.debug.o3_trace.stats_reset_tick", "Tick", 0),
        ("sim.debug.o3_trace.checkpoint_restores", "Count", 0),
        ("sim.debug.o3_trace.checkpoint_restore_records", "Count", 0),
        ("sim.debug.o3_trace.checkpoint_restore_tick", "Tick", 0),
        (
            "sim.debug.o3_trace.checkpoint_restore_payload_bytes",
            "Byte",
            0,
        ),
        ("sim.debug.o3_trace.instructions", "Count", 0),
        ("sim.debug.o3_trace.rob_allocations", "Count", 0),
        ("sim.debug.o3_trace.rob_commits", "Count", 0),
        ("sim.debug.o3_trace.rename_writes", "Count", 0),
        ("sim.debug.o3_trace.lsq_loads", "Count", 0),
        ("sim.debug.o3_trace.lsq_stores", "Count", 0),
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 0),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 0),
        ("sim.debug.o3_trace.float_loads", "Count", 0),
        ("sim.debug.o3_trace.float_stores", "Count", 0),
        (
            "sim.debug.o3_trace.store_load_forwarding_candidates",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.store_load_forwarding_matches",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.fu_latency_instructions", "Count", 0),
        ("sim.debug.o3_trace.fu_latency_cycles", "Cycle", 0),
        ("sim.debug.o3_trace.fu_integer_mul_instructions", "Count", 0),
        (
            "sim.debug.o3_trace.fu_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        ("sim.debug.o3_trace.fu_integer_div_instructions", "Count", 0),
        (
            "sim.debug.o3_trace.fu_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
        ("sim.debug.o3_trace.max_rob_occupancy", "Count", 0),
        ("sim.debug.o3_trace.max_lsq_occupancy", "Count", 0),
        ("sim.debug.o3_trace.rename_map_entries", "Count", 0),
        ("sim.debug.o3_trace.event.records", "Count", 0),
        ("sim.debug.o3_trace.event.system_events", "Count", 0),
        ("sim.debug.o3_trace.event.first_tick", "Tick", 0),
        ("sim.debug.o3_trace.event.last_tick", "Tick", 0),
        ("sim.debug.o3_trace.event.tick_span", "Tick", 0),
        ("sim.debug.o3_trace.event.lsq_data_latency_ticks", "Tick", 0),
        (
            "sim.debug.o3_trace.event.lsq_data_latency_samples",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_data_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_data_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_data_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_max_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.atomic_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load_latency_avg_ticks",
            "Tick",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store_latency_avg_ticks",
            "Tick",
            0,
        ),
        ("sim.debug.o3_trace.event.max_rob_occupancy", "Count", 0),
        ("sim.debug.o3_trace.event.max_lsq_occupancy", "Count", 0),
        (
            "sim.debug.o3_trace.event.max_rename_map_entries",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.event.rob_allocations", "Count", 0),
        ("sim.debug.o3_trace.event.rob_commits", "Count", 0),
        ("sim.debug.o3_trace.event.rename_writes", "Count", 0),
        ("sim.debug.o3_trace.event.lsq_loads", "Count", 0),
        ("sim.debug.o3_trace.event.lsq_stores", "Count", 0),
        ("sim.debug.o3_trace.event.lsq_operation.load", "Count", 0),
        ("sim.debug.o3_trace.event.lsq_operation.store", "Count", 0),
        (
            "sim.debug.o3_trace.event.lsq_operation.load_reserved",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.store_conditional",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.event.lsq_operation.atomic", "Count", 0),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_load",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.float_store",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_load",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_operation.vector_store",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.lsq_store_conditional_failures",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_not_taken",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_not_taken_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_write_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_write_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_predicted_target_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_without_link_write_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_without_link_write_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_wrong_target_without_link_write_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_targetless_mismatches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_mismatches",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_targetless_mismatch_kind.direct_conditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_target_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_wrong_target_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_repair_direction_only_kind.direct_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_targets",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_link_write_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_without_link_writes",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_without_link_write_kind.indirect_unconditional",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_squashed_target_kind.call_indirect",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.branch_resolved_targets",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 0),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 0),
        (
            "sim.debug.o3_trace.event.fu_latency_instructions",
            "Count",
            0,
        ),
        ("sim.debug.o3_trace.event.fu_latency_cycles", "Cycle", 0),
        ("sim.debug.o3_trace.event.fu_latency_max_cycles", "Cycle", 0),
        ("sim.debug.o3_trace.event.fu_latency_min_cycles", "Cycle", 0),
        ("sim.debug.o3_trace.event.fu_latency_avg_cycles", "Cycle", 0),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_max_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_min_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_mul_latency_avg_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_max_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_min_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_avg_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_mul_latency_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_instructions",
            "Count",
            0,
        ),
        (
            "sim.debug.o3_trace.event.fu_vector_integer_div_latency_cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_sbi_debug_flag_emits_real_riscv_sbi_trace() {
    let mut words = Vec::new();
    words.extend([
        i_type(b'S' as i32, 0, 0x0, 10, 0x13),
        i_type(SBI_LEGACY_CONSOLE_PUTCHAR, 0, 0x0, 17, 0x13),
        0x0000_0073,
        load_sbi_time_extension(17)[0],
        load_sbi_time_extension(17)[1],
        i_type(96, 0, 0x0, 10, 0x13),
        i_type(SBI_TIME_SET_TIMER, 0, 0x0, 16, 0x13),
        0x0000_0073,
        load_sbi_hsm_extension(17)[0],
        load_sbi_hsm_extension(17)[1],
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(SBI_HSM_HART_GET_STATUS, 0, 0x0, 16, 0x13),
        0x0000_0073,
        load_sbi_srst_extension(17)[0],
        load_sbi_srst_extension(17)[1],
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        i_type(SBI_SRST_SYSTEM_RESET, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("debug-flags-sbi", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-sbi",
            "--debug-flags",
            "Sbi",
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
        Some(&vec![Value::String("Sbi".to_string())])
    );
    let trace = json
        .pointer("/debug/sbi_trace")
        .and_then(Value::as_array)
        .expect("debug SBI trace array");
    assert_eq!(sbi_trace_kind_count(trace, "console"), 1);
    assert_eq!(sbi_trace_kind_count(trace, "timer"), 1);
    assert_eq!(sbi_trace_kind_count(trace, "hsm_status"), 1);
    assert_eq!(sbi_trace_kind_count(trace, "reset"), 1);
    assert_eq!(trace.len(), 4);

    let console = sbi_trace_record(trace, "console");
    assert_eq!(console.pointer("/bytes").and_then(Value::as_u64), Some(1));
    assert_eq!(console.pointer("/text").and_then(Value::as_str), Some("S"));
    assert_eq!(console.pointer("/hex").and_then(Value::as_str), Some("53"));

    let timer = sbi_trace_record(trace, "timer");
    assert_eq!(timer.pointer("/cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(timer.pointer("/deadline").and_then(Value::as_u64), Some(96));

    let hsm_status = sbi_trace_record(trace, "hsm_status");
    assert_eq!(
        hsm_status.pointer("/source_cpu").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        hsm_status.pointer("/target_hart").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        hsm_status.pointer("/status_name").and_then(Value::as_str),
        Some("started")
    );

    let reset = sbi_trace_record(trace, "reset");
    assert_eq!(reset.pointer("/cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        reset.pointer("/reset_type").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        reset.pointer("/reset_reason").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(reset.pointer("/code").and_then(Value::as_i64), Some(0));

    assert_stat(
        &stdout,
        "sim.debug.sbi_trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.sbi_trace.console",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.sbi_trace.timers",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.sbi_trace.hsm_statuses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.sbi_trace.resets",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.sbi_trace.console.bytes",
        "Byte",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.records",
        "Count",
        trace.len() as u64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.categories",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.debug.trace.active_flags",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_loads_debug_flags_from_toml_config() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("debug-flags-config");
    let binary = workspace.join("kernel.elf");
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"kernel.elf\"\nmax_tick = 60\nexecute = true\nmemory_system = \"direct\"\nstats_format = \"json\"\ndebug_flags = [\"Exec\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_exec_trace(
        &json,
        &[
            ExpectedExecTraceRecord {
                tick: 2,
                pc: "0x80000000",
                bytes: "93027000",
            },
            ExpectedExecTraceRecord {
                tick: 4,
                pc: "0x80000004",
                bytes: "73000000",
            },
        ],
    );
}

#[test]
fn rem6_run_rejects_unknown_debug_flag() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-unknown", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--execute",
            "--debug-flags",
            "Exec,NoSuchFlag",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported debug flag NoSuchFlag"));
}

#[test]
fn rem6_run_rejects_empty_debug_flag_entries() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-empty", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--execute",
            "--debug-flags",
            "Exec,",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("empty debug flag entry"));
}

#[test]
fn rem6_run_rejects_debug_flags_without_execution() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-no-execute", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "json",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--debug-flags requires --execute"));
}

#[test]
fn rem6_run_rejects_exec_debug_flags_with_text_stats() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-text-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "text",
            "--execute",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--debug-flags requires --stats-format json"));
}

struct ExpectedExecTraceRecord {
    tick: u64,
    pc: &'static str,
    bytes: &'static str,
}

struct ExpectedFetchTraceRecord {
    tick: u64,
    pc: &'static str,
    sequence: u64,
    size: u64,
}

fn stdout_json(stdout: Vec<u8>) -> Value {
    serde_json::from_slice(&stdout)
        .unwrap_or_else(|error| panic!("invalid JSON stdout: {error}; stdout={:?}", stdout))
}

fn assert_exec_trace(json: &Value, expected: &[ExpectedExecTraceRecord]) {
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Exec".to_string())])
    );
    let trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    assert_eq!(trace.len(), expected.len());
    for (record, expected) in trace.iter().zip(expected) {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("tick").and_then(Value::as_u64),
            Some(expected.tick)
        );
        assert_eq!(record.get("pc").and_then(Value::as_str), Some(expected.pc));
        assert_eq!(
            record.get("bytes").and_then(Value::as_str),
            Some(expected.bytes)
        );
        assert_eq!(record.get("retired").and_then(Value::as_bool), Some(true));
    }
}

fn assert_fetch_trace(json: &Value, expected: &[ExpectedFetchTraceRecord]) {
    let trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    assert_eq!(trace.len(), expected.len());
    for (record, expected) in trace.iter().zip(expected) {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("tick").and_then(Value::as_u64),
            Some(expected.tick)
        );
        assert_eq!(record.get("pc").and_then(Value::as_str), Some(expected.pc));
        assert_eq!(
            record.get("sequence").and_then(Value::as_u64),
            Some(expected.sequence)
        );
        assert_eq!(
            record.get("size").and_then(Value::as_u64),
            Some(expected.size)
        );
    }
}

fn assert_fetch_pcs(json: &Value, expected: &[&str]) {
    let trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    let pcs = trace
        .iter()
        .map(|record| record.get("pc").and_then(Value::as_str).unwrap_or(""))
        .collect::<Vec<_>>();
    assert_eq!(pcs, expected);
}

fn memory_trace_unique_requests(trace: &[Value], channel: Option<&str>) -> u64 {
    let mut requests = BTreeSet::new();
    for record in trace {
        if !memory_trace_channel_matches(record, channel) {
            continue;
        }
        let channel = record
            .get("channel")
            .and_then(Value::as_str)
            .expect("memory trace channel");
        let request_agent = record
            .get("request_agent")
            .and_then(Value::as_u64)
            .expect("memory trace request agent");
        let request = record
            .get("request")
            .and_then(Value::as_u64)
            .expect("memory trace request");
        requests.insert((channel, request_agent, request));
    }
    requests.len() as u64
}

fn memory_trace_unique_routes(trace: &[Value], channel: Option<&str>) -> u64 {
    let mut routes = BTreeSet::new();
    for record in trace {
        if !memory_trace_channel_matches(record, channel) {
            continue;
        }
        let channel = record
            .get("channel")
            .and_then(Value::as_str)
            .expect("memory trace channel");
        let route = record
            .get("route")
            .and_then(Value::as_u64)
            .expect("memory trace route");
        routes.insert((channel, route));
    }
    routes.len() as u64
}

fn memory_trace_unique_request_agents(trace: &[Value]) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get("request_agent")
                .and_then(Value::as_u64)
                .expect("memory trace request agent")
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn memory_trace_channel_matches(record: &Value, channel: Option<&str>) -> bool {
    channel.map_or(true, |expected| {
        record.get("channel").and_then(Value::as_str) == Some(expected)
    })
}

fn syscall_trace_unique_u64(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("syscall trace {field}"))
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn syscall_trace_unique_strings(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("syscall trace {field}"))
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn syscall_trace_argument_words(trace: &[Value]) -> u64 {
    trace
        .iter()
        .map(|record| syscall_trace_arguments(record).len() as u64)
        .sum()
}

fn syscall_trace_nonzero_arguments(trace: &[Value]) -> u64 {
    trace
        .iter()
        .map(syscall_trace_record_nonzero_arguments)
        .sum()
}

fn syscall_trace_record_nonzero_arguments(record: &Value) -> u64 {
    syscall_trace_arguments(record)
        .iter()
        .filter(|argument| argument.as_u64().is_some_and(|value| value != 0))
        .count() as u64
}

fn syscall_trace_arguments(record: &Value) -> &[Value] {
    record
        .get("arguments")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .expect("syscall trace arguments")
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SyscallTraceStats {
    records: u64,
    syscall_numbers: BTreeSet<u64>,
    call_sites: BTreeSet<String>,
    cpus: BTreeSet<u64>,
    returns: u64,
    exits: u64,
    blocked: u64,
    argument_words: u64,
    nonzero_arguments: u64,
}

impl SyscallTraceStats {
    fn add_record(&mut self, record: &Value) {
        self.records = self.records.saturating_add(1);
        self.syscall_numbers
            .insert(json_record_u64(record, "number"));
        self.call_sites
            .insert(json_record_str(record, "pc").to_string());
        self.cpus.insert(json_record_u64(record, "cpu"));
        self.argument_words = self
            .argument_words
            .saturating_add(syscall_trace_arguments(record).len() as u64);
        self.nonzero_arguments = self
            .nonzero_arguments
            .saturating_add(syscall_trace_record_nonzero_arguments(record));
        match syscall_trace_outcome_kind(record) {
            "return" => self.returns = self.returns.saturating_add(1),
            "exit" => self.exits = self.exits.saturating_add(1),
            "blocked" => self.blocked = self.blocked.saturating_add(1),
            other => panic!("unexpected syscall outcome {other}: {record:?}"),
        }
    }

    fn assert_stats(&self, stdout: &str, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("returns", self.returns),
            ("exits", self.exits),
            ("blocked", self.blocked),
            ("syscall_numbers", self.syscall_numbers.len() as u64),
            ("call_sites", self.call_sites.len() as u64),
            ("cpus", self.cpus.len() as u64),
            ("argument_words", self.argument_words),
            ("nonzero_arguments", self.nonzero_arguments),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}.{suffix}"),
                "Count",
                value,
                "monotonic",
            );
        }
    }
}

fn assert_syscall_trace_hierarchy_stats(stdout: &str, trace: &[Value]) {
    let mut cpus = BTreeMap::<u64, SyscallTraceStats>::new();
    let mut numbers = BTreeMap::<u64, SyscallTraceStats>::new();
    let mut call_sites = BTreeMap::<String, SyscallTraceStats>::new();
    let mut outcomes = BTreeMap::<String, SyscallTraceStats>::new();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let number = json_record_u64(record, "number");
        let call_site = json_record_str(record, "pc").to_string();
        let outcome = syscall_trace_outcome_kind(record).to_string();
        cpus.entry(cpu).or_default().add_record(record);
        numbers.entry(number).or_default().add_record(record);
        call_sites.entry(call_site).or_default().add_record(record);
        outcomes.entry(outcome).or_default().add_record(record);
    }
    for (cpu, stats) in cpus {
        stats.assert_stats(stdout, &format!("sim.debug.syscall_trace.cpu.cpu{cpu}"));
    }
    for (number, stats) in numbers {
        stats.assert_stats(
            stdout,
            &format!("sim.debug.syscall_trace.number.syscall{number}"),
        );
    }
    for (call_site, stats) in call_sites {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.syscall_trace.call_site.{}",
                stat_path_segment(&call_site)
            ),
        );
    }
    for (outcome, stats) in outcomes {
        stats.assert_stats(
            stdout,
            &format!(
                "sim.debug.syscall_trace.outcome.{}",
                stat_path_segment(&outcome)
            ),
        );
    }
}

fn syscall_trace_outcome_kind(record: &Value) -> &str {
    record
        .pointer("/outcome/kind")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("syscall trace outcome kind: {record:?}"))
}

fn power_trace_unique_strings(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| {
            record
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("power trace {field}"))
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn power_trace_state_count(trace: &[Value], state: &str) -> u64 {
    trace
        .iter()
        .filter(|record| record.get("state").and_then(Value::as_str) == Some(state))
        .count() as u64
}

fn power_trace_sum_u64(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| power_trace_record_u64(record, field))
        .sum()
}

fn power_trace_microwatts(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| power_trace_record_microwatts(record, field))
        .sum()
}

fn power_trace_microwatt_ticks(trace: &[Value], field: &str) -> u64 {
    trace.iter().fold(0u64, |acc, record| {
        acc.saturating_add(power_trace_record_microwatt_ticks(record, field))
    })
}

fn power_trace_record_u64(record: &Value, field: &str) -> u64 {
    record
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("power trace {field}"))
}

fn power_trace_record_state_count(record: &Value, state: &str) -> u64 {
    u64::from(record.get("state").and_then(Value::as_str) == Some(state))
}

fn power_trace_record_microwatts(record: &Value, field: &str) -> u64 {
    let watts = record
        .get(field)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("power trace {field}"));
    watts_to_microwatts(watts)
}

fn power_trace_record_microwatt_ticks(record: &Value, field: &str) -> u64 {
    let residency_ticks = power_trace_record_u64(record, "residency_ticks");
    power_trace_record_microwatts(record, field).saturating_mul(residency_ticks)
}

fn power_trace_target_stat_prefix(target: &str) -> String {
    let target_path = target
        .split('.')
        .map(stat_path_segment)
        .collect::<Vec<_>>()
        .join(".");
    format!("sim.debug.power_trace.target.{target_path}")
}

fn power_trace_record_millicelsius(record: &Value, field: &str) -> u64 {
    let celsius = record
        .get(field)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("power trace {field}"));
    celsius_to_millicelsius(celsius)
}

fn power_trace_max_millicelsius(trace: &[Value], field: &str) -> u64 {
    trace
        .iter()
        .map(|record| power_trace_record_millicelsius(record, field))
        .max()
        .unwrap_or(0)
}

fn watts_to_microwatts(watts: f64) -> u64 {
    if !watts.is_finite() || watts <= 0.0 {
        0
    } else {
        (watts * 1_000_000.0).round() as u64
    }
}

fn celsius_to_millicelsius(celsius: f64) -> u64 {
    if !celsius.is_finite() || celsius <= 0.0 {
        0
    } else {
        (celsius * 1_000.0).round() as u64
    }
}
