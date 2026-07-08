use super::*;

#[test]
fn rem6_run_o3_runtime_json_exposes_trace_event_summary() {
    let path =
        detailed_o3_iq_iew_commit_matrix_binary("m5-switch-cpu-detailed-o3-runtime-event-summary");

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
            "--debug-flags",
            "O3",
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

    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    let runtime_summary = o3_runtime
        .pointer("/event_summary")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event summary: {o3_runtime}"));
    let debug_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {json}"));

    assert_eq!(
        runtime_summary.pointer("/records"),
        o3_runtime.pointer("/instructions"),
        "runtime event summary should count the same real O3 events as runtime instructions: {runtime_summary}"
    );
    for pointer in [
        "/records",
        "/span_ticks",
        "/max_rob_occupancy",
        "/max_lsq_occupancy",
        "/max_rename_map_entries",
        "/event_window/max_rob_occupancy/tick",
        "/event_window/max_lsq_occupancy/sequence",
        "/rob/allocations",
        "/rob/commits",
        "/rename/writes",
        "/lsq_operation/load/count",
        "/lsq_operation/store/count",
        "/lsq_data_latency/samples",
        "/lsq_data_latency/ticks",
        "/iq/issued_inst_type/int_mul",
        "/iq/issued_inst_type/int_div",
        "/iew/dispatched_insts",
        "/iew/writeback_count",
        "/commit/committed_inst_type/int_mul",
        "/commit/committed_inst_type/int_div",
        "/fu_latency_class/integer_mul/instructions",
        "/fu_latency_class/integer_div/cycles",
    ] {
        assert_eq!(
            runtime_summary.pointer(pointer),
            debug_summary.pointer(pointer),
            "runtime event-summary lane {pointer} should mirror debug trace event summary"
        );
        assert!(
            runtime_summary
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "representative runtime event-summary lane {pointer} should be positive: {runtime_summary}"
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_keeps_trace_event_summary_null_without_debug_trace() {
    let path = detailed_o3_float_extended_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-runtime-event-summary-suppressed",
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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    assert!(
        o3_runtime
            .pointer("/event_summary")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should expose an explicit null event summary: {o3_runtime}"
    );
}
