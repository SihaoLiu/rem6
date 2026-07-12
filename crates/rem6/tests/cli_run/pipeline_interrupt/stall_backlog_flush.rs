use std::collections::BTreeSet;

use super::*;

const INTERRUPT_BACKLOG_SUMMARY: &str =
    "/debug/pipeline_summary/stall_backlog_flush/stall_cause/fetch_wait/flush_cause/interrupt_redirect";
const CPU0_INTERRUPT_BACKLOG_SUMMARY: &str =
    "/debug/pipeline_summary/stall_backlog_flush/cpu/cpu0/stall_cause/fetch_wait/flush_cause/interrupt_redirect";
const INTERRUPT_BACKLOG_STAT: &str =
    "sim.debug.pipeline_trace.stall_backlog_flush.stall_cause.fetch_wait.flush_cause.interrupt_redirect";
const CPU0_INTERRUPT_BACKLOG_STAT: &str =
    "sim.debug.pipeline_trace.cpu.cpu0.stall_backlog_flush.stall_cause.fetch_wait.flush_cause.interrupt_redirect";
const INTERRUPT_BACKLOG_JSON_STAGE: &str =
    "/block_kind/ordering_blocked/blocked_stage/execute/flushed_stage/execute";
const INTERRUPT_BACKLOG_STAT_STAGE: &str =
    ".block_kind.ordering_blocked.blocked_stage.execute.flushed_stage.execute";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BacklogFlushTotals {
    sequences: u64,
    stall_records: u64,
    stall_cycles: u64,
}

#[test]
fn rem6_run_pipeline_debug_correlates_fetch_wait_backlog_with_interrupt_flush() {
    let program = interrupt_timer_flush_program_path(
        "pipeline-interrupt-stall-backlog-flush",
        INTERRUPT_FLUSH_WITH_YOUNGERS_DEADLINE,
    );
    let stdout = run_interrupt_timer_program(&program.path, "json", Some("Pipeline,Fetch"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert_pipeline_summary_matches_trace(&json);
    assert_timer_handler_completed(&json);

    let interrupt_redirects = trace
        .iter()
        .filter(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
        })
        .collect::<Vec<_>>();
    assert_eq!(interrupt_redirects.len(), 1, "{interrupt_redirects:?}");
    let interrupt_redirect = interrupt_redirects[0];
    assert_eq!(
        interrupt_redirect
            .get("flush_cause")
            .and_then(Value::as_str),
        Some("interrupt_redirect")
    );
    assert_eq!(
        interrupt_redirect
            .get("redirect_target")
            .and_then(Value::as_str),
        Some(format!("0x{:x}", program.handler_pc).as_str())
    );

    let flushed = record_array(interrupt_redirect, "flushed");
    assert_eq!(
        flushed.len(),
        1,
        "the calibrated timer row must flush exactly one younger instruction: {interrupt_redirect:?}"
    );
    assert_eq!(
        flushed[0].get("stage").and_then(Value::as_str),
        Some("execute")
    );
    let flushed_sequence = json_record_u64(&flushed[0], "sequence");
    let fetch_pcs = fetch_trace_pcs_by_sequence(&json);
    assert_eq!(
        fetch_pcs.get(&flushed_sequence).map(String::as_str),
        Some(format!("0x{:x}", program.loop_pc).as_str())
    );

    let redirect_cycle = json_record_u64(interrupt_redirect, "cycle");
    let raw_backlog = trace
        .iter()
        .filter(|record| {
            json_record_u64(record, "cycle") < redirect_cycle
                && record.get("stall_cause").and_then(Value::as_str) == Some("fetch_wait")
                && record_array(record, "ordering_blocked")
                    .iter()
                    .any(|blocked| {
                        json_record_u64(blocked, "sequence") == flushed_sequence
                            && blocked.get("stage").and_then(Value::as_str) == Some("execute")
                    })
        })
        .collect::<Vec<_>>();
    assert_eq!(raw_backlog.len(), 6, "{raw_backlog:?}");
    assert!(
        raw_backlog
            .iter()
            .all(|record| json_record_u64(record, "stall_cycles") == 1),
        "each calibrated interrupt backlog row must represent one cycle: {raw_backlog:?}"
    );
    assert_eq!(
        raw_backlog
            .iter()
            .map(|record| json_record_u64(record, "stall_cycles"))
            .sum::<u64>(),
        6,
        "{raw_backlog:?}"
    );
    assert!(
        raw_backlog.iter().all(|record| {
            record_array(record, "after_in_flight")
                .iter()
                .any(|instruction| json_record_u64(instruction, "sequence") == flushed_sequence)
        }),
        "the correlated sequence must remain live through every prior wait: {raw_backlog:?}"
    );
    assert_interrupt_backlog_flush_outputs(
        &json,
        &stdout,
        BacklogFlushTotals {
            sequences: 1,
            stall_records: 6,
            stall_cycles: 6,
        },
    );

    let suppressed_stdout =
        run_interrupt_timer_program_with_lookahead(&program.path, "json", Some("Pipeline"), "1");
    let suppressed_json: Value = serde_json::from_str(&suppressed_stdout).unwrap();
    let suppressed_trace = suppressed_json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert_pipeline_summary_matches_trace(&suppressed_json);
    assert_timer_handler_completed(&suppressed_json);
    let suppressed_interrupts = suppressed_trace
        .iter()
        .filter(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
        })
        .collect::<Vec<_>>();
    assert_eq!(suppressed_interrupts.len(), 1, "{suppressed_interrupts:?}");
    let suppressed_interrupt = suppressed_interrupts[0];
    assert!(
        record_array(suppressed_interrupt, "flushed").is_empty(),
        "lookahead one must leave no younger interrupt-flushed row: {suppressed_interrupt:?}"
    );
    assert!(
        suppressed_interrupt
            .get("flush_cause")
            .is_none_or(Value::is_null),
        "redirect-only interrupt rows must not claim a flush cause: {suppressed_interrupt:?}"
    );
    assert_eq!(
        suppressed_interrupt
            .get("redirect_target")
            .and_then(Value::as_str),
        Some(format!("0x{:x}", program.handler_pc).as_str())
    );
    assert_eq!(
        json_path_u64(
            &suppressed_json,
            "/cores/0/in_order_pipeline/interrupt_redirects"
        ),
        1
    );
    assert_eq!(
        json_path_u64(
            &suppressed_json,
            "/cores/0/in_order_pipeline/interrupt_redirect_flushes"
        ),
        0
    );
    assert_interrupt_backlog_flush_outputs(
        &suppressed_json,
        &suppressed_stdout,
        BacklogFlushTotals::default(),
    );
}

fn assert_timer_handler_completed(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("breakpoint")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some(
            format!(
                "0x{:x}",
                RISCV_INTERRUPT_BIT | RISCV_SUPERVISOR_TIMER_INTERRUPT
            )
            .as_str()
        )
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x7")
            .and_then(Value::as_str),
        Some("0x5a")
    );
}

fn assert_interrupt_backlog_flush_outputs(
    json: &Value,
    stdout: &str,
    expected: BacklogFlushTotals,
) {
    for (metric, unit, expected) in [
        ("sequences", "Count", expected.sequences),
        ("stall_records", "Count", expected.stall_records),
        ("stall_cycles", "Cycle", expected.stall_cycles),
    ] {
        for prefix in [INTERRUPT_BACKLOG_SUMMARY, CPU0_INTERRUPT_BACKLOG_SUMMARY] {
            assert_eq!(
                json.pointer(&format!("{prefix}/{metric}"))
                    .and_then(Value::as_u64),
                Some(expected),
                "missing interrupt backlog summary {prefix}/{metric}: {json}"
            );
        }
        for prefix in [INTERRUPT_BACKLOG_STAT, CPU0_INTERRUPT_BACKLOG_STAT] {
            assert_stat(
                stdout,
                &format!("{prefix}.{metric}"),
                unit,
                expected,
                "monotonic",
            );
        }
    }

    if expected.sequences == 0 {
        for prefix in [INTERRUPT_BACKLOG_SUMMARY, CPU0_INTERRUPT_BACKLOG_SUMMARY] {
            let block_kind = json
                .pointer(&format!("{prefix}/block_kind"))
                .and_then(Value::as_object)
                .expect("interrupt backlog block-kind object");
            assert!(block_kind.is_empty(), "{block_kind:?}");
        }
        for prefix in [INTERRUPT_BACKLOG_STAT, CPU0_INTERRUPT_BACKLOG_STAT] {
            assert!(!stdout.contains(&format!("{prefix}.block_kind.")));
        }
        return;
    }

    for prefix in [INTERRUPT_BACKLOG_SUMMARY, CPU0_INTERRUPT_BACKLOG_SUMMARY] {
        let block_kinds = json
            .pointer(&format!("{prefix}/block_kind"))
            .and_then(Value::as_object)
            .expect("interrupt backlog block-kind object");
        assert_eq!(block_kinds.len(), 1, "{block_kinds:?}");
        let blocked_stages = json
            .pointer(&format!(
                "{prefix}/block_kind/ordering_blocked/blocked_stage"
            ))
            .and_then(Value::as_object)
            .expect("interrupt backlog blocked-stage object");
        assert_eq!(blocked_stages.len(), 1, "{blocked_stages:?}");
        let flushed_stages = json
            .pointer(&format!(
                "{prefix}/block_kind/ordering_blocked/blocked_stage/execute/flushed_stage"
            ))
            .and_then(Value::as_object)
            .expect("interrupt backlog flushed-stage object");
        assert_eq!(flushed_stages.len(), 1, "{flushed_stages:?}");
        for (metric, expected) in [
            ("sequences", expected.sequences),
            ("stall_records", expected.stall_records),
            ("stall_cycles", expected.stall_cycles),
        ] {
            assert_eq!(
                json.pointer(&format!("{prefix}{INTERRUPT_BACKLOG_JSON_STAGE}/{metric}"))
                    .and_then(Value::as_u64),
                Some(expected)
            );
        }
    }
    for prefix in [INTERRUPT_BACKLOG_STAT, CPU0_INTERRUPT_BACKLOG_STAT] {
        for (metric, unit, expected) in [
            ("sequences", "Count", expected.sequences),
            ("stall_records", "Count", expected.stall_records),
            ("stall_cycles", "Cycle", expected.stall_cycles),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}{INTERRUPT_BACKLOG_STAT_STAGE}.{metric}"),
                unit,
                expected,
                "monotonic",
            );
        }

        let stage_prefix = format!("{prefix}.block_kind.");
        let expected_prefix = format!("{prefix}{INTERRUPT_BACKLOG_STAT_STAGE}");
        let observed = json
            .pointer("/stats")
            .and_then(Value::as_array)
            .expect("JSON stats array")
            .iter()
            .filter_map(|stat| stat.get("path").and_then(Value::as_str))
            .filter(|path| path.starts_with(&stage_prefix))
            .collect::<BTreeSet<_>>();
        let expected = ["sequences", "stall_records", "stall_cycles"]
            .into_iter()
            .map(|metric| format!("{expected_prefix}.{metric}"))
            .collect::<BTreeSet<_>>();
        assert_eq!(observed, expected.iter().map(String::as_str).collect());
    }
}
