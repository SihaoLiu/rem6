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

fn detailed_o3_store_forwarding_mismatch_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, 0b010),
        i_type(4, 5, 0b010, 12, 0x03),
        b_type(8, 0, 12, 0x1),
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
    for record in &wait_records {
        for blocked in record_array(record, "resource_blocked") {
            let stage = blocked
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("missing blocked instruction stage: {blocked}"));
            *stage_resource_blocked
                .entry(stat_path_segment(stage))
                .or_default() += 1;
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
    for (stage, resource_blocked) in stage_resource_blocked {
        assert_stat(
            stdout,
            &format!("sim.debug.pipeline_trace.stall_cause.{cause}.stage.{stage}.resource_blocked"),
            "Count",
            resource_blocked,
            "monotonic",
        );
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

    for (field, value) in [
        ("cpu", 0),
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
    assert_o3_event(&events[0], 0, "0x80000004", 1, 0, 0, false);
    assert_o3_event(&events[1], 1, "0x80000008", 1, 0, 0, false);
    assert_o3_event(&events[2], 2, "0x8000000c", 1, 0, 0, false);
    assert_o3_event(&events[3], 3, "0x80000010", 1, 1, 0, false);
    assert_o3_event(&events[4], 4, "0x80000014", 0, 0, 1, false);
    assert_o3_event(&events[5], 5, "0x80000018", 0, 0, 0, true);
    assert_eq!(json_record_str(&events[3], "lsq_operation"), "load");
    assert_eq!(json_record_str(&events[4], "lsq_operation"), "store");

    assert_stat(&stdout, "sim.debug.flags", "Count", 1, "constant");
    for (path, unit, value) in [
        ("sim.debug.trace.records", "Count", 1),
        ("sim.debug.trace.categories", "Count", 1),
        ("sim.debug.trace.active_flags", "Count", 1),
        ("sim.debug.o3_trace.records", "Count", 1),
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
        ("sim.debug.o3_trace.event.fu_latency_cycles", "Cycle", 21),
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
            "sim.debug.o3_trace.event.fu_integer_div_instructions",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.fu_integer_div_latency_cycles",
            "Cycle",
            19,
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
fn rem6_run_o3_debug_flag_suppresses_store_forwarding_on_address_mismatch() {
    let path = detailed_o3_store_forwarding_mismatch_debug_binary(
        "debug-flags-o3-store-forwarding-address-mismatch",
    );

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
    assert_eq!(
        json_record_u64(record, "store_load_forwarding_candidates"),
        0
    );
    assert_eq!(json_record_u64(record, "store_load_forwarding_matches"), 0);

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
        "0x80000044"
    );
    assert_eq!(json_record_u64(&events[4], "lsq_load_bytes"), 4);

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
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
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

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.instructions", "Count", 9),
        ("sim.debug.o3_trace.fu_integer_mul_instructions", "Count", 1),
        ("sim.debug.o3_trace.fu_integer_div_instructions", "Count", 1),
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 4),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 4),
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
        ("sim.debug.o3_trace.instructions", "Count", 0),
        ("sim.debug.o3_trace.rob_allocations", "Count", 0),
        ("sim.debug.o3_trace.rob_commits", "Count", 0),
        ("sim.debug.o3_trace.rename_writes", "Count", 0),
        ("sim.debug.o3_trace.lsq_loads", "Count", 0),
        ("sim.debug.o3_trace.lsq_stores", "Count", 0),
        ("sim.debug.o3_trace.lsq_load_bytes", "Byte", 0),
        ("sim.debug.o3_trace.lsq_store_bytes", "Byte", 0),
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
        ("sim.debug.o3_trace.event.first_tick", "Tick", 0),
        ("sim.debug.o3_trace.event.last_tick", "Tick", 0),
        ("sim.debug.o3_trace.event.tick_span", "Tick", 0),
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
        ("sim.debug.o3_trace.event.lsq_load_bytes", "Byte", 0),
        ("sim.debug.o3_trace.event.lsq_store_bytes", "Byte", 0),
        ("sim.debug.o3_trace.event.fu_latency_cycles", "Cycle", 0),
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
