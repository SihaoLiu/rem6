use std::collections::{BTreeMap, BTreeSet};

use super::*;

const SBI_HSM_EXTENSION: i32 = 0x0048_534d;
const SBI_HSM_HART_START: i32 = 0;

const CPU0_FETCH_WAIT_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(0, "fetch_wait", "ordering_blocked", "execute", "execute");
const CPU0_EXECUTE_WAIT_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(0, "execute_wait", "ordering_blocked", "decode", "execute");
const CPU0_DATA_WAIT_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(0, "data_wait", "ordering_blocked", "decode", "commit");
const CPU1_EXECUTE_WAIT_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(1, "execute_wait", "ordering_blocked", "decode", "execute");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InterruptBacklogPair {
    cpu: u64,
    stall_cause: &'static str,
    block_kind: &'static str,
    blocked_stage: &'static str,
    flushed_stage: &'static str,
}

impl InterruptBacklogPair {
    const fn new(
        cpu: u64,
        stall_cause: &'static str,
        block_kind: &'static str,
        blocked_stage: &'static str,
        flushed_stage: &'static str,
    ) -> Self {
        Self {
            cpu,
            stall_cause,
            block_kind,
            blocked_stage,
            flushed_stage,
        }
    }

    fn aggregate_summary_prefix(self) -> String {
        let suffix = format!(
            "stall_cause/{}/flush_cause/interrupt_redirect",
            self.stall_cause
        );
        format!("/debug/pipeline_summary/stall_backlog_flush/{suffix}")
    }

    fn cpu_summary_prefix(self) -> String {
        let suffix = format!(
            "stall_cause/{}/flush_cause/interrupt_redirect",
            self.stall_cause
        );
        format!(
            "/debug/pipeline_summary/stall_backlog_flush/cpu/cpu{}/{suffix}",
            self.cpu
        )
    }

    fn summary_prefixes(self) -> [String; 2] {
        [self.aggregate_summary_prefix(), self.cpu_summary_prefix()]
    }

    fn aggregate_stat_prefix(self) -> String {
        let suffix = format!(
            "stall_cause.{}.flush_cause.interrupt_redirect",
            self.stall_cause
        );
        format!("sim.debug.pipeline_trace.stall_backlog_flush.{suffix}")
    }

    fn cpu_stat_prefix(self) -> String {
        let suffix = format!(
            "stall_cause.{}.flush_cause.interrupt_redirect",
            self.stall_cause
        );
        format!(
            "sim.debug.pipeline_trace.cpu.cpu{}.stall_backlog_flush.{suffix}",
            self.cpu
        )
    }

    fn stat_prefixes(self) -> [String; 2] {
        [self.aggregate_stat_prefix(), self.cpu_stat_prefix()]
    }

    fn json_stage_suffix(self) -> String {
        format!(
            "/block_kind/{}/blocked_stage/{}/flushed_stage/{}",
            self.block_kind, self.blocked_stage, self.flushed_stage
        )
    }

    fn stat_stage_suffix(self) -> String {
        format!(
            ".block_kind.{}.blocked_stage.{}.flushed_stage.{}",
            self.block_kind, self.blocked_stage, self.flushed_stage
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BacklogFlushTotals {
    sequences: u64,
    stall_records: u64,
    stall_cycles: u64,
}

fn raw_interrupt_backlog_flush_totals(
    trace: &[Value],
    pair: InterruptBacklogPair,
) -> BacklogFlushTotals {
    let mut active = BTreeMap::<u64, (u64, u64)>::new();
    let mut totals = BacklogFlushTotals::default();
    for record in trace {
        if json_record_u64(record, "cpu") != pair.cpu {
            continue;
        }

        let matches_interrupt =
            record.get("flush_cause").and_then(Value::as_str) == Some("interrupt_redirect");
        for flushed in record_array(record, "flushed") {
            let sequence = json_record_u64(flushed, "sequence");
            if let Some((stall_records, stall_cycles)) = active.remove(&sequence) {
                if matches_interrupt {
                    totals.sequences += 1;
                    totals.stall_records += stall_records;
                    totals.stall_cycles += stall_cycles;
                }
            }
        }

        if record.get("stall_cause").and_then(Value::as_str) == Some(pair.stall_cause) {
            let stall_cycles = json_record_u64(record, "stall_cycles");
            let blocked_sequences = record_array(record, pair.block_kind)
                .iter()
                .map(|blocked| json_record_u64(blocked, "sequence"))
                .collect::<BTreeSet<_>>();
            for sequence in blocked_sequences {
                let backlog = active.entry(sequence).or_default();
                backlog.0 += 1;
                backlog.1 += stall_cycles;
            }
        }

        let live = record_array(record, "after_in_flight")
            .iter()
            .map(|instruction| json_record_u64(instruction, "sequence"))
            .collect::<BTreeSet<_>>();
        active.retain(|sequence, _| live.contains(sequence));
    }
    totals
}

fn interrupt_timer_data_wait_program_path(name: &str, timer_deadline: i32) -> InterruptProgram {
    const DATA_OFFSET: usize = 256;

    let mut words = Vec::new();
    let stvec_auipc_index = append_interrupt_timer_setup(&mut words, timer_deadline);
    let data_auipc_index = words.len();
    words.extend([
        u_type(0, 2, 0x17),
        i_type(
            DATA_OFFSET as i32 - (data_auipc_index * 4) as i32,
            2,
            0x0,
            2,
            0x13,
        ),
        i_type(0, 2, 0x3, 10, 0x03),  // ld x10, 0(x2)
        i_type(1, 8, 0x0, 8, 0x13),   // addi x8, x8, 1
        i_type(1, 9, 0x0, 9, 0x13),   // addi x9, x9, 1
        i_type(1, 13, 0x0, 13, 0x13), // addi x13, x13, 1
    ]);
    let loop_index = words.len();
    words.push(b_type(0, 0, 0, 0x0));
    let handler_index = append_interrupt_breakpoint_handler(&mut words);
    patch_interrupt_handler_pc(&mut words, stvec_auipc_index, handler_index);
    while words.len() * 4 < DATA_OFFSET {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344]);

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    InterruptProgram {
        path: temp_binary(name, &elf),
        handler_pc: RISCV_SBI_ENTRY + handler_index as u64 * 4,
        loop_pc: RISCV_SBI_ENTRY + loop_index as u64 * 4,
    }
}

fn secondary_interrupt_timer_flush_program_path(
    name: &str,
    timer_deadline: i32,
) -> InterruptProgram {
    let mut words = vec![i_type(1, 0, 0x0, 10, 0x13)];
    let secondary_auipc_index = words.len();
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0, 11, 0x0, 11, 0x13),
        i_type(0x66, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
        b_type(0, 0, 0, 0x0),
    ]);

    let secondary_index = words.len();
    let stvec_auipc_index = append_interrupt_timer_setup(&mut words, timer_deadline);
    words.extend([
        i_type(97, 0, 0x0, 11, 0x13),
        i_type(3, 0, 0x0, 12, 0x13),
        r_type(0x01, 12, 11, 0x4, 10, 0x33),
        i_type(1, 8, 0x0, 8, 0x13),
        i_type(1, 9, 0x0, 9, 0x13),
    ]);
    let loop_index = words.len();
    words.push(b_type(0, 0, 0, 0x0));
    let handler_index = append_interrupt_breakpoint_handler(&mut words);
    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    patch_interrupt_handler_pc(&mut words, stvec_auipc_index, handler_index);

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    InterruptProgram {
        path: temp_binary(name, &elf),
        handler_pc: RISCV_SBI_ENTRY + handler_index as u64 * 4,
        loop_pc: RISCV_SBI_ENTRY + loop_index as u64 * 4,
    }
}

fn load_hsm_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_HSM_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_HSM_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
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
    let expected = BacklogFlushTotals {
        sequences: 1,
        stall_records: 6,
        stall_cycles: 6,
    };
    assert_eq!(
        raw_interrupt_backlog_flush_totals(trace, CPU0_FETCH_WAIT_INTERRUPT),
        expected,
        "{trace:?}"
    );
    assert_interrupt_backlog_flush_outputs(&json, &stdout, CPU0_FETCH_WAIT_INTERRUPT, expected);

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
    let expected = BacklogFlushTotals::default();
    assert_eq!(
        raw_interrupt_backlog_flush_totals(suppressed_trace, CPU0_FETCH_WAIT_INTERRUPT),
        expected,
        "{suppressed_trace:?}"
    );
    assert_interrupt_backlog_flush_outputs(
        &suppressed_json,
        &suppressed_stdout,
        CPU0_FETCH_WAIT_INTERRUPT,
        expected,
    );
}

#[test]
fn rem6_run_pipeline_debug_correlates_execute_wait_backlog_with_interrupt_flush() {
    let program =
        interrupt_timer_flush_program_path("pipeline-interrupt-execute-wait-backlog-flush", 103);
    for (mode, expected) in [
        (
            "detailed",
            BacklogFlushTotals {
                sequences: 3,
                stall_records: 57,
                stall_cycles: 57,
            },
        ),
        ("timing", BacklogFlushTotals::default()),
    ] {
        let stdout = run_interrupt_timer_program_in_mode(
            &program.path,
            "json",
            Some("Pipeline,O3,Fetch"),
            mode,
        );
        let json: Value = serde_json::from_str(&stdout).unwrap();
        let trace = json
            .pointer("/debug/pipeline_trace")
            .and_then(Value::as_array)
            .expect("debug pipeline trace array");
        assert_pipeline_summary_matches_trace(&json);
        assert_timer_handler_completed(&json);

        let interrupts = trace
            .iter()
            .filter(|record| {
                record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
            })
            .collect::<Vec<_>>();
        assert_eq!(interrupts.len(), 1, "mode {mode}: {interrupts:?}");
        let interrupt = interrupts[0];
        assert_eq!(
            interrupt.get("redirect_target").and_then(Value::as_str),
            Some(format!("0x{:x}", program.handler_pc).as_str())
        );

        let flushed = record_array(interrupt, "flushed");
        if mode == "detailed" {
            assert_eq!(
                json.pointer("/cores/0/registers/x6")
                    .and_then(Value::as_str),
                Some("0x80000038")
            );
            assert!(
                json.pointer("/cores/0/registers/x10").is_none(),
                "the interrupting DIV must not write its architectural destination: {json}"
            );
            assert_eq!(
                interrupt.get("flush_cause").and_then(Value::as_str),
                Some("interrupt_redirect")
            );
            assert_eq!(flushed.len(), 3, "{interrupt:?}");
            assert!(flushed.iter().all(|instruction| {
                instruction.get("stage").and_then(Value::as_str) == Some("execute")
            }));
            let flushed_sequences = flushed
                .iter()
                .map(|instruction| json_record_u64(instruction, "sequence"))
                .collect::<BTreeSet<_>>();
            let raw = trace
                .iter()
                .filter(|record| {
                    record.get("stall_cause").and_then(Value::as_str) == Some("execute_wait")
                        && record_array(record, "ordering_blocked")
                            .iter()
                            .any(|blocked| {
                                blocked.get("stage").and_then(Value::as_str) == Some("decode")
                                    && flushed_sequences
                                        .contains(&json_record_u64(blocked, "sequence"))
                            })
                })
                .collect::<Vec<_>>();
            assert_eq!(raw.len(), 19, "{raw:?}");
            assert!(raw
                .iter()
                .all(|record| json_record_u64(record, "stall_cycles") == 1));
            assert_eq!(
                raw.iter()
                    .map(|record| {
                        record_array(record, "ordering_blocked")
                            .iter()
                            .filter(|blocked| {
                                flushed_sequences.contains(&json_record_u64(blocked, "sequence"))
                            })
                            .count()
                    })
                    .sum::<usize>(),
                57
            );
            for sequence in &flushed_sequences {
                assert_eq!(
                    raw.iter()
                        .filter(|record| record_array(record, "ordering_blocked")
                            .iter()
                            .any(|blocked| json_record_u64(blocked, "sequence") == *sequence))
                        .count(),
                    19,
                    "sequence {sequence} must remain blocked for the full DIV latency"
                );
            }
            assert_detailed_interrupt_discards_younger_fu_window(&json);
        } else {
            assert!(flushed.is_empty(), "mode {mode}: {interrupt:?}");
            assert!(interrupt.get("flush_cause").is_none_or(Value::is_null));
            assert!(json
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty));
        }
        assert_eq!(
            raw_interrupt_backlog_flush_totals(trace, CPU0_EXECUTE_WAIT_INTERRUPT),
            expected,
            "mode {mode}: {trace:?}"
        );
        assert_interrupt_backlog_flush_outputs(
            &json,
            &stdout,
            CPU0_EXECUTE_WAIT_INTERRUPT,
            expected,
        );
    }
}

#[test]
fn rem6_run_pipeline_debug_correlates_data_wait_backlog_with_interrupt_flush() {
    let program =
        interrupt_timer_data_wait_program_path("pipeline-interrupt-data-wait-backlog-flush", 110);
    for (mode, expected) in [
        (
            "detailed",
            BacklogFlushTotals {
                sequences: 2,
                stall_records: 12,
                stall_cycles: 12,
            },
        ),
        ("timing", BacklogFlushTotals::default()),
    ] {
        let stdout = run_interrupt_timer_program_with_args(
            &program.path,
            "json",
            Some("Pipeline,O3,Fetch,Memory,Data"),
            &[
                "--riscv-branch-lookahead",
                "2",
                "--riscv-execution-mode",
                mode,
                "--riscv-o3-scalar-memory-depth",
                "4",
            ],
        );
        let json: Value = serde_json::from_str(&stdout).unwrap();
        let trace = json
            .pointer("/debug/pipeline_trace")
            .and_then(Value::as_array)
            .expect("debug pipeline trace array");
        assert_pipeline_summary_matches_trace(&json);
        assert_timer_handler_completed(&json);
        assert_eq!(
            json.pointer("/cores/0/registers/x10")
                .and_then(Value::as_str),
            Some("0x1122334455667788")
        );
        assert_one_completed_data_load(&json);

        let interrupts = trace
            .iter()
            .filter(|record| {
                record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
            })
            .collect::<Vec<_>>();
        assert_eq!(interrupts.len(), 1, "mode {mode}: {interrupts:?}");
        let interrupt = interrupts[0];
        assert_eq!(
            interrupt.get("redirect_target").and_then(Value::as_str),
            Some(format!("0x{:x}", program.handler_pc).as_str())
        );
        let flushed = record_array(interrupt, "flushed");
        if mode == "detailed" {
            assert_eq!(
                json.pointer("/cores/0/registers/x6")
                    .and_then(Value::as_str),
                Some("0x8000003c")
            );
            assert_eq!(
                interrupt.get("flush_cause").and_then(Value::as_str),
                Some("interrupt_redirect")
            );
            assert_eq!(flushed.len(), 2, "{interrupt:?}");
            assert!(flushed.iter().all(|instruction| {
                instruction.get("stage").and_then(Value::as_str) == Some("commit")
            }));
            let flushed_sequences = flushed
                .iter()
                .map(|instruction| json_record_u64(instruction, "sequence"))
                .collect::<BTreeSet<_>>();
            let waits = trace
                .iter()
                .filter(|record| {
                    record.get("stall_cause").and_then(Value::as_str) == Some("data_wait")
                })
                .collect::<Vec<_>>();
            assert_eq!(waits.len(), 6, "{waits:?}");
            assert!(waits.iter().all(|record| {
                json_record_u64(record, "stall_cycles") == 1
                    && flushed_sequences.iter().all(|sequence| {
                        record_array(record, "ordering_blocked")
                            .iter()
                            .any(|blocked| {
                                blocked.get("stage").and_then(Value::as_str) == Some("decode")
                                    && json_record_u64(blocked, "sequence") == *sequence
                            })
                    })
            }));
            assert_eq!(
                waits
                    .iter()
                    .map(|record| {
                        record_array(record, "ordering_blocked")
                            .iter()
                            .filter(|blocked| {
                                flushed_sequences.contains(&json_record_u64(blocked, "sequence"))
                            })
                            .count()
                    })
                    .sum::<usize>(),
                12
            );
            assert_detailed_interrupt_discards_younger_load_window(&json);
        } else {
            assert!(flushed.is_empty(), "mode {mode}: {interrupt:?}");
            assert!(interrupt.get("flush_cause").is_none_or(Value::is_null));
            assert_eq!(
                json.pointer("/cores/0/registers/x8")
                    .and_then(Value::as_str),
                Some("0x1")
            );
            assert_eq!(
                json.pointer("/cores/0/registers/x9")
                    .and_then(Value::as_str),
                Some("0x1")
            );
            assert!(json.pointer("/cores/0/registers/x13").is_none());
            assert!(json
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty));
        }
        assert_eq!(
            raw_interrupt_backlog_flush_totals(trace, CPU0_DATA_WAIT_INTERRUPT),
            expected,
            "mode {mode}: {trace:?}"
        );
        assert_interrupt_backlog_flush_outputs(&json, &stdout, CPU0_DATA_WAIT_INTERRUPT, expected);
    }
}

#[test]
fn rem6_run_pipeline_debug_correlates_cpu1_execute_wait_backlog_with_interrupt_flush() {
    let program = secondary_interrupt_timer_flush_program_path(
        "pipeline-cpu1-interrupt-execute-wait-backlog-flush",
        145,
    );
    for (mode, expected) in [
        (
            "detailed",
            BacklogFlushTotals {
                sequences: 1,
                stall_records: 19,
                stall_cycles: 19,
            },
        ),
        ("timing", BacklogFlushTotals::default()),
    ] {
        let stdout = run_interrupt_timer_program_with_args(
            &program.path,
            "json",
            Some("Pipeline,Fetch"),
            &[
                "--cores",
                "2",
                "--parallel-workers",
                "2",
                "--riscv-branch-lookahead",
                "2",
                "--riscv-execution-mode",
                mode,
            ],
        );
        let json: Value = serde_json::from_str(&stdout).unwrap();
        let trace = json
            .pointer("/debug/pipeline_trace")
            .and_then(Value::as_array)
            .expect("debug pipeline trace array");
        assert_pipeline_summary_matches_trace(&json);
        assert_timer_handler_completed_for_cpu(&json, 1);
        let hsm = json
            .pointer("/riscv_sbi_hsm_events/0")
            .expect("CPU0 HSM start event");
        assert_eq!(hsm.get("source_cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(hsm.get("function").and_then(Value::as_u64), Some(0));
        assert_eq!(hsm.get("target_hart").and_then(Value::as_u64), Some(1));
        assert_eq!(hsm.get("opaque").and_then(Value::as_str), Some("0x66"));
        let timer = json
            .pointer("/riscv_sbi_timers/0")
            .expect("CPU1 timer event");
        assert_eq!(timer.get("cpu").and_then(Value::as_u64), Some(1));
        assert_eq!(timer.get("deadline").and_then(Value::as_u64), Some(145));
        assert_eq!(
            json_path_u64(&json, "/cores/0/in_order_pipeline/interrupt_redirects"),
            0
        );
        assert_cpu_interrupt_backlog_empty(&json, &stdout, CPU0_EXECUTE_WAIT_INTERRUPT);

        let interrupts = trace
            .iter()
            .filter(|record| {
                record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
            })
            .collect::<Vec<_>>();
        assert_eq!(interrupts.len(), 1, "mode {mode}: {interrupts:?}");
        let interrupt = interrupts[0];
        assert_eq!(interrupt.get("cpu").and_then(Value::as_u64), Some(1));
        assert_eq!(
            interrupt.get("redirect_target").and_then(Value::as_str),
            Some(format!("0x{:x}", program.handler_pc).as_str())
        );
        let flushed = record_array(interrupt, "flushed");
        if mode == "detailed" {
            assert_eq!(
                json.pointer("/cores/1/registers/x6")
                    .and_then(Value::as_str),
                Some("0x8000005c")
            );
            assert!(
                json.pointer("/cores/1/registers/x10").is_none(),
                "the interrupting CPU1 DIV must not write its architectural destination: {json}"
            );
            assert_eq!(
                interrupt.get("flush_cause").and_then(Value::as_str),
                Some("interrupt_redirect")
            );
            assert_eq!(flushed.len(), 1, "{interrupt:?}");
            assert_eq!(
                flushed[0].get("stage").and_then(Value::as_str),
                Some("execute")
            );
            let sequence = json_record_u64(&flushed[0], "sequence");
            let waits = trace
                .iter()
                .filter(|record| {
                    record.get("cpu").and_then(Value::as_u64) == Some(1)
                        && record.get("stall_cause").and_then(Value::as_str) == Some("execute_wait")
                        && record_array(record, "ordering_blocked")
                            .iter()
                            .any(|blocked| {
                                blocked.get("stage").and_then(Value::as_str) == Some("decode")
                                    && json_record_u64(blocked, "sequence") == sequence
                            })
                })
                .collect::<Vec<_>>();
            assert_eq!(waits.len(), 19, "{waits:?}");
            assert!(waits
                .iter()
                .all(|record| json_record_u64(record, "stall_cycles") == 1));
        } else {
            assert!(flushed.is_empty(), "{interrupt:?}");
            assert!(interrupt.get("flush_cause").is_none_or(Value::is_null));
        }
        for register in ["x8", "x9"] {
            assert!(json
                .pointer(&format!("/cores/1/registers/{register}"))
                .is_none());
        }
        assert_eq!(
            raw_interrupt_backlog_flush_totals(trace, CPU1_EXECUTE_WAIT_INTERRUPT),
            expected,
            "mode {mode}: {trace:?}"
        );
        assert_interrupt_backlog_flush_outputs(
            &json,
            &stdout,
            CPU1_EXECUTE_WAIT_INTERRUPT,
            expected,
        );
    }
}

fn assert_one_completed_data_load(json: &Value) {
    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("Data debug trace");
    assert_eq!(data.len(), 1, "{data:?}");
    assert_eq!(data[0].get("kind").and_then(Value::as_str), Some("load"));
    assert_eq!(
        data[0].get("address").and_then(Value::as_str),
        Some("0x80000100")
    );
    assert_eq!(data[0].get("size").and_then(Value::as_u64), Some(8));

    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("Memory debug trace");
    let data_records = memory
        .iter()
        .filter(|record| record.get("channel").and_then(Value::as_str) == Some("data"))
        .collect::<Vec<_>>();
    assert_eq!(data_records.len(), 3, "{data_records:?}");
    assert_eq!(
        data_records
            .iter()
            .map(|record| record.get("kind").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        ["request_sent", "request_arrived", "response_arrived"]
    );
    assert_eq!(
        data_records[2]
            .get("response_latency_ticks")
            .and_then(Value::as_u64),
        Some(6)
    );
}

fn assert_detailed_interrupt_discards_younger_load_window(json: &Value) {
    for register in ["x8", "x9", "x13"] {
        assert!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .is_none(),
            "interrupt-flushed {register} must remain architecturally absent: {json}"
        );
    }
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    let o3 = json
        .pointer("/debug/o3_trace/0")
        .expect("detailed O3 trace summary");
    assert_eq!(o3.get("max_rob_occupancy").and_then(Value::as_u64), Some(4));
    assert_eq!(o3.get("max_lsq_occupancy").and_then(Value::as_u64), Some(1));
    let events = o3
        .get("events")
        .and_then(Value::as_array)
        .expect("detailed O3 events");
    let load = events
        .iter()
        .find(|event| event.get("pc").and_then(Value::as_str) == Some("0x80000038"))
        .expect("completed detailed load O3 event");
    assert_eq!(load.get("issue_tick").and_then(Value::as_u64), Some(108));
    assert_eq!(
        load.get("lsq_data_response_tick").and_then(Value::as_u64),
        Some(114)
    );
    for pc in ["0x8000003c", "0x80000040", "0x80000044"] {
        assert!(
            events
                .iter()
                .all(|event| event.get("pc").and_then(Value::as_str) != Some(pc)),
            "interrupt-flushed instruction {pc} must not retire through O3: {events:?}"
        );
    }
}

fn assert_detailed_interrupt_discards_younger_fu_window(json: &Value) {
    assert!(json.pointer("/cores/0/registers/x8").is_none());
    assert!(json.pointer("/cores/0/registers/x9").is_none());
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    let rename = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .expect("detailed O3 rename map");
    assert!(rename.iter().all(|entry| {
        !matches!(
            entry.get("architectural").and_then(Value::as_u64),
            Some(8 | 9)
        )
    }));
    let o3 = json
        .pointer("/debug/o3_trace/0")
        .expect("detailed O3 trace summary");
    assert_eq!(o3.get("max_rob_occupancy").and_then(Value::as_u64), Some(3));
    let events = o3
        .get("events")
        .and_then(Value::as_array)
        .expect("detailed O3 events");
    for pc in ["0x80000038", "0x8000003c", "0x80000040", "0x80000044"] {
        assert!(
            events
                .iter()
                .all(|event| event.get("pc").and_then(Value::as_str) != Some(pc)),
            "interrupt-flushed instruction {pc} must not retire through O3: {events:?}"
        );
    }
}

fn assert_timer_handler_completed(json: &Value) {
    assert_timer_handler_completed_for_cpu(json, 0);
}

fn assert_timer_handler_completed_for_cpu(json: &Value, cpu: u64) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("breakpoint")
    );
    assert_eq!(
        json.pointer(&format!("/cores/{cpu}/registers/x5"))
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
        json.pointer(&format!("/cores/{cpu}/registers/x7"))
            .and_then(Value::as_str),
        Some("0x5a")
    );
}

fn assert_cpu_interrupt_backlog_empty(json: &Value, stdout: &str, pair: InterruptBacklogPair) {
    let summary_prefix = pair.cpu_summary_prefix();
    let stat_prefix = pair.cpu_stat_prefix();
    for (metric, unit) in [
        ("sequences", "Count"),
        ("stall_records", "Count"),
        ("stall_cycles", "Cycle"),
    ] {
        assert_eq!(
            json.pointer(&format!("{summary_prefix}/{metric}"))
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_stat(
            stdout,
            &format!("{stat_prefix}.{metric}"),
            unit,
            0,
            "monotonic",
        );
    }
    assert!(json
        .pointer(&format!("{summary_prefix}/block_kind"))
        .and_then(Value::as_object)
        .is_some_and(serde_json::Map::is_empty));
    assert!(!stdout.contains(&format!("{stat_prefix}.block_kind.")));
}

fn assert_interrupt_backlog_flush_outputs(
    json: &Value,
    stdout: &str,
    pair: InterruptBacklogPair,
    expected: BacklogFlushTotals,
) {
    let summary_prefixes = pair.summary_prefixes();
    let stat_prefixes = pair.stat_prefixes();
    let json_stage_suffix = pair.json_stage_suffix();
    let stat_stage_suffix = pair.stat_stage_suffix();
    for (metric, unit, expected) in [
        ("sequences", "Count", expected.sequences),
        ("stall_records", "Count", expected.stall_records),
        ("stall_cycles", "Cycle", expected.stall_cycles),
    ] {
        for prefix in &summary_prefixes {
            assert_eq!(
                json.pointer(&format!("{prefix}/{metric}"))
                    .and_then(Value::as_u64),
                Some(expected),
                "missing interrupt backlog summary {prefix}/{metric}: {json}"
            );
        }
        for prefix in &stat_prefixes {
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
        for prefix in &summary_prefixes {
            let block_kind = json
                .pointer(&format!("{prefix}/block_kind"))
                .and_then(Value::as_object)
                .expect("interrupt backlog block-kind object");
            assert!(block_kind.is_empty(), "{block_kind:?}");
        }
        for prefix in &stat_prefixes {
            assert!(!stdout.contains(&format!("{prefix}.block_kind.")));
        }
        return;
    }

    for prefix in &summary_prefixes {
        let block_kinds = json
            .pointer(&format!("{prefix}/block_kind"))
            .and_then(Value::as_object)
            .expect("interrupt backlog block-kind object");
        assert_eq!(block_kinds.len(), 1, "{block_kinds:?}");
        let blocked_stages = json
            .pointer(&format!(
                "{prefix}/block_kind/{}/blocked_stage",
                pair.block_kind
            ))
            .and_then(Value::as_object)
            .expect("interrupt backlog blocked-stage object");
        assert_eq!(blocked_stages.len(), 1, "{blocked_stages:?}");
        let flushed_stages = json
            .pointer(&format!(
                "{prefix}/block_kind/{}/blocked_stage/{}/flushed_stage",
                pair.block_kind, pair.blocked_stage
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
                json.pointer(&format!("{prefix}{json_stage_suffix}/{metric}"))
                    .and_then(Value::as_u64),
                Some(expected)
            );
        }
    }
    for prefix in &stat_prefixes {
        for (metric, unit, expected) in [
            ("sequences", "Count", expected.sequences),
            ("stall_records", "Count", expected.stall_records),
            ("stall_cycles", "Cycle", expected.stall_cycles),
        ] {
            assert_stat(
                stdout,
                &format!("{prefix}{stat_stage_suffix}.{metric}"),
                unit,
                expected,
                "monotonic",
            );
        }

        let stage_prefix = format!("{prefix}.block_kind.");
        let expected_prefix = format!("{prefix}{stat_stage_suffix}");
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
