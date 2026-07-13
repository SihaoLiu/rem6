use super::*;

#[derive(Clone, Copy)]
struct BacklogFlushPair {
    summary_path: &'static str,
    cpu_summary_path: &'static str,
    stat_path: &'static str,
    cpu_stat_path: &'static str,
    stage_cell: (&'static str, &'static str, &'static str),
}

const FETCH_WAIT_BRANCH: BacklogFlushPair = BacklogFlushPair {
    summary_path:
        "/debug/pipeline_summary/stall_backlog_flush/stall_cause/fetch_wait/flush_cause/branch_prediction",
    cpu_summary_path:
        "/debug/pipeline_summary/stall_backlog_flush/cpu/cpu0/stall_cause/fetch_wait/flush_cause/branch_prediction",
    stat_path:
        "sim.debug.pipeline_trace.stall_backlog_flush.stall_cause.fetch_wait.flush_cause.branch_prediction",
    cpu_stat_path:
        "sim.debug.pipeline_trace.cpu.cpu0.stall_backlog_flush.stall_cause.fetch_wait.flush_cause.branch_prediction",
    stage_cell: ("ordering_blocked", "fetch1", "fetch1"),
};
const DATA_WAIT_BRANCH: BacklogFlushPair = BacklogFlushPair {
    summary_path:
        "/debug/pipeline_summary/stall_backlog_flush/stall_cause/data_wait/flush_cause/branch_prediction",
    cpu_summary_path:
        "/debug/pipeline_summary/stall_backlog_flush/cpu/cpu0/stall_cause/data_wait/flush_cause/branch_prediction",
    stat_path:
        "sim.debug.pipeline_trace.stall_backlog_flush.stall_cause.data_wait.flush_cause.branch_prediction",
    cpu_stat_path:
        "sim.debug.pipeline_trace.cpu.cpu0.stall_backlog_flush.stall_cause.data_wait.flush_cause.branch_prediction",
    stage_cell: ("ordering_blocked", "fetch1", "execute"),
};
const EXECUTE_WAIT_TRAP: BacklogFlushPair = BacklogFlushPair {
    summary_path:
        "/debug/pipeline_summary/stall_backlog_flush/stall_cause/execute_wait/flush_cause/trap_redirect",
    cpu_summary_path:
        "/debug/pipeline_summary/stall_backlog_flush/cpu/cpu0/stall_cause/execute_wait/flush_cause/trap_redirect",
    stat_path:
        "sim.debug.pipeline_trace.stall_backlog_flush.stall_cause.execute_wait.flush_cause.trap_redirect",
    cpu_stat_path:
        "sim.debug.pipeline_trace.cpu.cpu0.stall_backlog_flush.stall_cause.execute_wait.flush_cause.trap_redirect",
    stage_cell: ("ordering_blocked", "fetch1", "fetch2"),
};

#[test]
fn rem6_run_pipeline_debug_correlates_fetch_wait_backlog_with_branch_flush() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // wrong path: addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // target: addi x7, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-stall-backlog-fetch-wait-flush", &elf);

    for (lookahead, expected) in [
        (
            "2",
            RawBacklogFlushTotals {
                sequences: 2,
                stall_records: 6,
                stall_cycles: 6,
            },
        ),
        ("1", RawBacklogFlushTotals::default()),
    ] {
        let (stdout, json) = pipeline_branch_json(&path, lookahead);
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("executed_until_trap")
        );
        assert_eq!(
            json.pointer("/cores/0/registers/x5")
                .and_then(Value::as_str),
            Some("0x1")
        );
        assert_eq!(
            json.pointer("/cores/0/registers/x7")
                .and_then(Value::as_str),
            Some("0x7")
        );
        assert!(
            json.pointer("/cores/0/registers/x6").is_none(),
            "wrong-path register must remain absent: {json}"
        );

        let trace = json
            .pointer("/debug/pipeline_trace")
            .and_then(Value::as_array)
            .expect("Pipeline debug trace");
        assert_pipeline_trace_redirect_flush_conservation(trace);
        assert!(
            trace.iter().any(|record| {
                record.get("flush_cause").and_then(Value::as_str) == Some("branch_prediction")
                    && !record_array(record, "flushed").is_empty()
            }),
            "both rows must execute a real branch flush: {trace:?}"
        );
        let branch_flush_records = trace
            .iter()
            .filter(|record| {
                record.get("flush_cause").and_then(Value::as_str) == Some("branch_prediction")
            })
            .count() as u64;
        assert_eq!(
            json_stat_value(
                &json,
                "sim.debug.pipeline_trace.flush_cause.branch_prediction.records"
            ),
            branch_flush_records,
            "branch flush-cause stats must equal raw Pipeline trace rows: {trace:?}"
        );
        assert_eq!(
            json_stat_value(
                &json,
                "sim.cpu0.pipeline.in_order.flush_cause.branch_prediction.records"
            ),
            branch_flush_records,
            "core branch flush-cause stats must equal raw Pipeline trace rows: {trace:?}"
        );
        let observed =
            raw_backlog_flush_totals(trace, "fetch_wait", "ordering_blocked", "branch_prediction");
        assert_eq!(observed, expected, "lookahead {lookahead}: {trace:?}");
        assert_backlog_flush_pair(&json, &stdout, FETCH_WAIT_BRANCH, expected);
    }
}

#[test]
fn rem6_run_pipeline_debug_correlates_younger_data_wait_backlog_with_branch_flush() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(20, 2, 0x2, 5, 0x03), // lw x5, 20(x2)
        b_type(8, 0, 0, 0x0),        // beq x0, x0, target
        i_type(9, 0, 0x0, 6, 0x13),  // wrong path: addi x6, x0, 9
        0x0000_0073,                 // target: ecall
    ]);
    program.extend_from_slice(&0x1122_3344u32.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-stall-backlog-data-wait-flush", &elf);

    for (mode, expected) in [
        (
            "detailed",
            RawBacklogFlushTotals {
                sequences: 1,
                stall_records: 2,
                stall_cycles: 2,
            },
        ),
        ("timing", RawBacklogFlushTotals::default()),
    ] {
        let (stdout, json) = pipeline_data_wait_branch_json(&path, mode);
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("executed_until_trap")
        );
        assert_eq!(
            json.pointer("/cores/0/registers/x5")
                .and_then(Value::as_str),
            Some("0x11223344")
        );
        assert!(
            json.pointer("/cores/0/registers/x6").is_none(),
            "wrong-path register must remain absent: {json}"
        );

        let trace = json
            .pointer("/debug/pipeline_trace")
            .and_then(Value::as_array)
            .expect("Pipeline debug trace");
        assert_pipeline_trace_redirect_flush_conservation(trace);
        assert!(
            trace.iter().any(|record| {
                record.get("stall_cause").and_then(Value::as_str) == Some("data_wait")
            }),
            "both rows must preserve a real data wait: {trace:?}"
        );
        assert!(
            trace.iter().any(|record| {
                record.get("flush_cause").and_then(Value::as_str) == Some("branch_prediction")
                    && !record_array(record, "flushed").is_empty()
            }),
            "both rows must preserve a real branch flush: {trace:?}"
        );

        let observed =
            raw_backlog_flush_totals(trace, "data_wait", "ordering_blocked", "branch_prediction");
        assert_eq!(observed, expected, "mode {mode}: {trace:?}");
        assert_backlog_flush_pair(&json, &stdout, DATA_WAIT_BRANCH, expected);
    }
}

#[test]
fn rem6_run_pipeline_debug_does_not_correlate_completed_execute_wait_with_later_trap_flush() {
    let program = riscv64_program(&[
        b_type(8, 0, 0, 0x1),             // bne x0, x0, +8
        r_type(0x01, 0, 0, 0x4, 3, 0x33), // div x3, x0, x0
        0x0000_0073,                      // ecall
        i_type(9, 0, 0x0, 6, 0x13),       // younger: addi x6, x0, 9
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-stall-backlog-execute-wait-flush", &elf);

    for (lookahead, expected_trap_flush_records) in [("2", Some(1)), ("1", None)] {
        let expected = RawBacklogFlushTotals::default();
        let (stdout, json) = pipeline_execute_wait_trap_json(&path, lookahead);
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("executed_until_trap")
        );
        assert_eq!(
            json.pointer("/cores/0/registers/x3")
                .and_then(Value::as_str),
            Some("0xffffffffffffffff")
        );
        assert!(
            json.pointer("/cores/0/registers/x6").is_none(),
            "post-trap register must remain absent: {json}"
        );
        assert_eq!(
            json_stat_value(
                &json,
                "sim.debug.pipeline_trace.stall_cause.execute_wait.records"
            ),
            19,
            "both rows must preserve DIV execute wait: {json}"
        );
        assert_eq!(
            json_stat_value(
                &json,
                "sim.debug.pipeline_trace.redirect_cause.trap_redirect.records"
            ),
            1,
            "both rows must preserve the trap redirect: {json}"
        );

        let trace = json
            .pointer("/debug/pipeline_trace")
            .and_then(Value::as_array)
            .expect("Pipeline debug trace");
        assert_pipeline_trace_redirect_flush_conservation(trace);
        let observed =
            raw_backlog_flush_totals(trace, "execute_wait", "ordering_blocked", "trap_redirect");
        assert_eq!(observed, expected, "lookahead {lookahead}: {trace:?}");
        let trap_flush_records = optional_json_stat_value(
            &json,
            "sim.debug.pipeline_trace.flush_cause.trap_redirect.records",
        );
        assert_eq!(
            trap_flush_records, expected_trap_flush_records,
            "lookahead two must retain the later younger trap flush without attributing the completed DIV wait to it: {json}"
        );
        assert_backlog_flush_pair(&json, &stdout, EXECUTE_WAIT_TRAP, expected);
    }
}

fn assert_pipeline_trace_redirect_flush_conservation(trace: &[Value]) {
    for record in trace {
        let flushed = record_array(record, "flushed");
        let flush_cause = record.get("flush_cause").unwrap_or(&Value::Null);
        let redirect_cause = record.get("redirect_cause").unwrap_or(&Value::Null);
        if flushed.is_empty() {
            assert!(
                flush_cause.is_null(),
                "a cycle without flushed rows must not expose a flush cause: {record}"
            );
        } else {
            assert!(
                !flush_cause.is_null(),
                "a cycle with flushed rows must expose a typed flush cause: {record}"
            );
            assert_eq!(
                flush_cause, redirect_cause,
                "flushed rows must use the plan-owned redirect cause: {record}"
            );
        }
    }
}

fn pipeline_branch_json(path: &std::path::Path, lookahead: &str) -> (String, Value) {
    pipeline_correlation_json(path, "160", &["--riscv-branch-lookahead", lookahead])
}

fn pipeline_data_wait_branch_json(path: &std::path::Path, mode: &str) -> (String, Value) {
    pipeline_correlation_json(
        path,
        "160",
        &[
            "--riscv-branch-lookahead",
            "2",
            "--riscv-execution-mode",
            mode,
            "--riscv-o3-scalar-memory-depth",
            "4",
        ],
    )
}

fn pipeline_execute_wait_trap_json(path: &std::path::Path, lookahead: &str) -> (String, Value) {
    pipeline_correlation_json(
        path,
        "260",
        &[
            "--riscv-in-order-width",
            "1",
            "--riscv-branch-lookahead",
            lookahead,
        ],
    )
}

fn pipeline_correlation_json(
    path: &std::path::Path,
    max_tick: &str,
    extra_args: &[&str],
) -> (String, Value) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            max_tick,
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Pipeline",
        ])
        .args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json = serde_json::from_str(&stdout).unwrap();
    (stdout, json)
}

fn assert_backlog_flush_pair(
    json: &Value,
    stdout: &str,
    pair: BacklogFlushPair,
    expected: RawBacklogFlushTotals,
) {
    assert_backlog_flush_summary(json, pair.summary_path, expected);
    assert_backlog_flush_summary(json, pair.cpu_summary_path, expected);
    assert_backlog_flush_stats(stdout, pair.stat_path, expected);
    assert_backlog_flush_stats(stdout, pair.cpu_stat_path, expected);

    let (block_kind, blocked_stage, flushed_stage) = pair.stage_cell;
    if expected.sequences == 0 {
        assert_backlog_flush_stage_cells_absent(json, pair.summary_path, stdout, pair.stat_path);
        assert_backlog_flush_stage_cells_absent(
            json,
            pair.cpu_summary_path,
            stdout,
            pair.cpu_stat_path,
        );
        return;
    }

    let json_suffix = format!(
        "/block_kind/{block_kind}/blocked_stage/{blocked_stage}/flushed_stage/{flushed_stage}"
    );
    let stat_suffix = format!(
        ".block_kind.{block_kind}.blocked_stage.{blocked_stage}.flushed_stage.{flushed_stage}"
    );
    assert_backlog_flush_summary(
        json,
        &format!("{}{json_suffix}", pair.summary_path),
        expected,
    );
    assert_backlog_flush_summary(
        json,
        &format!("{}{json_suffix}", pair.cpu_summary_path),
        expected,
    );
    assert_backlog_flush_stats(
        stdout,
        &format!("{}{stat_suffix}", pair.stat_path),
        expected,
    );
    assert_backlog_flush_stats(
        stdout,
        &format!("{}{stat_suffix}", pair.cpu_stat_path),
        expected,
    );
    assert_only_backlog_flush_stage_cell(
        json,
        pair.summary_path,
        pair.stat_path,
        block_kind,
        blocked_stage,
        flushed_stage,
    );
    assert_only_backlog_flush_stage_cell(
        json,
        pair.cpu_summary_path,
        pair.cpu_stat_path,
        block_kind,
        blocked_stage,
        flushed_stage,
    );
}

fn assert_only_backlog_flush_stage_cell(
    json: &Value,
    json_prefix: &str,
    stat_prefix: &str,
    block_kind: &str,
    blocked_stage: &str,
    flushed_stage: &str,
) {
    let block_kinds = json
        .pointer(&format!("{json_prefix}/block_kind"))
        .and_then(Value::as_object)
        .expect("pipeline correlation block-kind object");
    assert_eq!(
        block_kinds.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![block_kind],
        "positive correlation pair must expose exactly one block kind: {block_kinds:?}"
    );
    let blocked_stages = json
        .pointer(&format!(
            "{json_prefix}/block_kind/{block_kind}/blocked_stage"
        ))
        .and_then(Value::as_object)
        .expect("pipeline correlation blocked-stage object");
    assert_eq!(
        blocked_stages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![blocked_stage],
        "positive correlation pair must expose exactly one blocked stage: {blocked_stages:?}"
    );
    let flushed_stages = json
        .pointer(&format!(
            "{json_prefix}/block_kind/{block_kind}/blocked_stage/{blocked_stage}/flushed_stage"
        ))
        .and_then(Value::as_object)
        .expect("pipeline correlation flushed-stage object");
    assert_eq!(
        flushed_stages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![flushed_stage],
        "positive correlation pair must expose exactly one flushed stage: {flushed_stages:?}"
    );

    let stage_prefix = format!("{stat_prefix}.block_kind.");
    let expected_prefix = format!(
        "{stage_prefix}{block_kind}.blocked_stage.{blocked_stage}.flushed_stage.{flushed_stage}"
    );
    let stage_stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("JSON stats array")
        .iter()
        .filter_map(|stat| stat.get("path").and_then(Value::as_str))
        .filter(|path| path.starts_with(&stage_prefix))
        .collect::<BTreeSet<_>>();
    let expected_stats = ["sequences", "stall_records", "stall_cycles"]
        .into_iter()
        .map(|metric| format!("{expected_prefix}.{metric}"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        stage_stats,
        expected_stats.iter().map(String::as_str).collect(),
        "positive correlation pair must expose exactly one stage-cell metric family"
    );
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RawBacklogFlushTotals {
    sequences: u64,
    stall_records: u64,
    stall_cycles: u64,
}

fn raw_backlog_flush_totals(
    trace: &[Value],
    stall_cause: &str,
    blocked_field: &str,
    flush_cause: &str,
) -> RawBacklogFlushTotals {
    let mut active = BTreeMap::<(u64, u64), (u64, u64)>::new();
    let mut totals = RawBacklogFlushTotals::default();
    for record in trace {
        let cpu = json_record_u64(record, "cpu");
        let matches_flush_cause =
            record.get("flush_cause").and_then(Value::as_str) == Some(flush_cause);
        for flushed in record_array(record, "flushed") {
            let sequence = json_record_u64(flushed, "sequence");
            if let Some((stall_records, stall_cycles)) = active.remove(&(cpu, sequence)) {
                if matches_flush_cause {
                    totals.sequences += 1;
                    totals.stall_records += stall_records;
                    totals.stall_cycles += stall_cycles;
                }
            }
        }

        if record.get("stall_cause").and_then(Value::as_str) == Some(stall_cause) {
            let stall_cycles = json_record_u64(record, "stall_cycles");
            let blocked_sequences = record_array(record, blocked_field)
                .iter()
                .map(|blocked| json_record_u64(blocked, "sequence"))
                .collect::<BTreeSet<_>>();
            for sequence in blocked_sequences {
                let backlog = active.entry((cpu, sequence)).or_default();
                backlog.0 += 1;
                backlog.1 += stall_cycles;
            }
        }

        let live = record_array(record, "after_in_flight")
            .iter()
            .map(|instruction| json_record_u64(instruction, "sequence"))
            .collect::<BTreeSet<_>>();
        active.retain(|(candidate_cpu, sequence), _| {
            *candidate_cpu != cpu || live.contains(sequence)
        });
    }
    totals
}

fn assert_backlog_flush_stage_cells_absent(
    json: &Value,
    json_prefix: &str,
    stdout: &str,
    stat_prefix: &str,
) {
    let block_kind = json
        .pointer(&format!("{json_prefix}/block_kind"))
        .and_then(Value::as_object)
        .expect("pipeline correlation block-kind object");
    assert!(
        block_kind.is_empty(),
        "zero correlation pair must not emit stage cells: {block_kind:?}"
    );
    assert!(
        !stdout.contains(&format!("{stat_prefix}.block_kind.")),
        "zero correlation pair must not emit stage-cell stats: {stdout}"
    );
}

fn assert_backlog_flush_summary(json: &Value, prefix: &str, expected: RawBacklogFlushTotals) {
    for (metric, expected) in [
        ("sequences", expected.sequences),
        ("stall_records", expected.stall_records),
        ("stall_cycles", expected.stall_cycles),
    ] {
        assert_eq!(
            json.pointer(&format!("{prefix}/{metric}"))
                .and_then(Value::as_u64),
            Some(expected),
            "missing pipeline correlation summary {prefix}/{metric}: {json}"
        );
    }
}

fn assert_backlog_flush_stats(stdout: &str, prefix: &str, expected: RawBacklogFlushTotals) {
    for (metric, unit, expected) in [
        ("sequences", "Count", expected.sequences),
        ("stall_records", "Count", expected.stall_records),
        ("stall_cycles", "Cycle", expected.stall_cycles),
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{metric}"),
            unit,
            expected,
            "monotonic",
        );
    }
}

fn optional_json_stat_value(json: &Value, path: &str) -> Option<u64> {
    json.pointer("/stats")
        .and_then(Value::as_array)
        .and_then(|stats| {
            stats
                .iter()
                .find(|stat| stat.get("path").and_then(Value::as_str) == Some(path))
        })
        .and_then(|stat| stat.get("value"))
        .and_then(Value::as_u64)
}
