use super::*;

const FETCH_WAIT_PAIR: &str =
    "/debug/pipeline_summary/stall_backlog_flush/stall_cause/fetch_wait/flush_cause/branch_prediction";
const FETCH_WAIT_STAT_PAIR: &str =
    "sim.debug.pipeline_trace.stall_backlog_flush.stall_cause.fetch_wait.flush_cause.branch_prediction";
const CPU0_FETCH_WAIT_STAT_PAIR: &str =
    "sim.debug.pipeline_trace.cpu.cpu0.stall_backlog_flush.stall_cause.fetch_wait.flush_cause.branch_prediction";

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
        assert_backlog_flush_summary(&json, FETCH_WAIT_PAIR, expected);
        let cpu_pair_path =
            FETCH_WAIT_PAIR.replace("/stall_backlog_flush/", "/stall_backlog_flush/cpu/cpu0/");
        assert_backlog_flush_summary(&json, &cpu_pair_path, expected);
        assert_backlog_flush_stats(&stdout, FETCH_WAIT_STAT_PAIR, expected);
        assert_backlog_flush_stats(&stdout, CPU0_FETCH_WAIT_STAT_PAIR, expected);

        if expected.sequences > 0 {
            let stage_json_suffix =
                "/block_kind/ordering_blocked/blocked_stage/fetch1/flushed_stage/fetch1";
            let stage_stat_suffix =
                ".block_kind.ordering_blocked.blocked_stage.fetch1.flushed_stage.fetch1";
            let stage_path = format!("{FETCH_WAIT_PAIR}{stage_json_suffix}");
            assert_backlog_flush_summary(&json, &stage_path, expected);
            assert_backlog_flush_summary(
                &json,
                &format!("{cpu_pair_path}{stage_json_suffix}"),
                expected,
            );
            assert_backlog_flush_stats(
                &stdout,
                &format!("{FETCH_WAIT_STAT_PAIR}{stage_stat_suffix}"),
                expected,
            );
            assert_backlog_flush_stats(
                &stdout,
                &format!("{CPU0_FETCH_WAIT_STAT_PAIR}{stage_stat_suffix}"),
                expected,
            );
        } else {
            assert_backlog_flush_stage_cells_absent(
                &json,
                FETCH_WAIT_PAIR,
                &stdout,
                FETCH_WAIT_STAT_PAIR,
            );
            assert_backlog_flush_stage_cells_absent(
                &json,
                &cpu_pair_path,
                &stdout,
                CPU0_FETCH_WAIT_STAT_PAIR,
            );
        }
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
            lookahead,
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
    let json = serde_json::from_str(&stdout).unwrap();
    (stdout, json)
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
