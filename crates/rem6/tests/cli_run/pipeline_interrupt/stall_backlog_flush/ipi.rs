use super::*;

const SBI_IPI_EXTENSION: i32 = 0x0073_5049;
const SBI_IPI_SEND_IPI: i32 = 0;
const RISCV_SUPERVISOR_SOFTWARE_INTERRUPT: u64 = 1;

const CPU1_FETCH_WAIT_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(1, "fetch_wait", "ordering_blocked", "fetch2", "commit");
const CPU1_FETCH_WAIT_COMMIT_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(1, "fetch_wait", "ordering_blocked", "fetch1", "commit");
const CPU1_FETCH_WAIT_RESOURCE_INTERRUPT: InterruptBacklogPair =
    InterruptBacklogPair::new(1, "fetch_wait", "resource_blocked", "fetch2", "commit");

fn secondary_interrupt_ipi_fetch_wait_program_path(name: &str) -> InterruptProgram {
    let mut words = vec![i_type(1, 0, 0x0, 10, 0x13)];
    let secondary_auipc_index = words.len();
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0, 11, 0x0, 11, 0x13),
        i_type(0x68, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    for _ in 0..12 {
        words.push(i_type(1, 14, 0x0, 14, 0x13));
    }
    words.extend([
        i_type(1 << 1, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        load_ipi_extension(17)[0],
        load_ipi_extension(17)[1],
        i_type(SBI_IPI_SEND_IPI, 0, 0x0, 16, 0x13),
        0x0000_0073,
        b_type(0, 0, 0, 0x0),
    ]);

    let secondary_index = words.len();
    let stvec_auipc_index = words.len();
    words.extend([
        u_type(0, 5, 0x17),
        i_type(0, 5, 0x0, 5, 0x13),
        csr_write(0x105, 5),
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x104, 5),
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x100, 5),
    ]);
    let loop_index = words.len();
    words.push(b_type(0, 0, 0, 0x0));
    let handler_index = words.len();
    words.extend([
        csr_read(0x142, 5),
        csr_read(0x141, 6),
        i_type(0x4d, 0, 0x0, 7, 0x13),
        0x0010_0073,
    ]);

    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    words[stvec_auipc_index + 1] = i_type(
        ((handler_index - stvec_auipc_index) * 4) as i32,
        5,
        0x0,
        5,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    InterruptProgram {
        path: temp_binary(name, &elf),
        handler_pc: RISCV_SBI_ENTRY + handler_index as u64 * 4,
        loop_pc: RISCV_SBI_ENTRY + loop_index as u64 * 4,
    }
}

fn load_ipi_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_IPI_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_IPI_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

#[test]
fn rem6_run_pipeline_debug_correlates_cpu1_ipi_fetch_wait_backlog_with_interrupt_flush() {
    let program = secondary_interrupt_ipi_fetch_wait_program_path(
        "pipeline-cpu1-ipi-fetch-wait-backlog-flush",
    );
    let stdout = run_interrupt_program_with_args(
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
        ],
    );
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert_pipeline_summary_matches_trace(&json);
    assert_software_interrupt_handler_completed_for_cpu(&json, 1);
    assert_ipi_fixture_evidence(&json);
    assert_eq!(
        json_path_u64(&json, "/cores/0/in_order_pipeline/interrupt_redirects"),
        0
    );
    assert_cpu_interrupt_backlog_empty(&json, &stdout, CPU0_FETCH_WAIT_INTERRUPT);

    let interrupts = trace
        .iter()
        .filter(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
        })
        .collect::<Vec<_>>();
    assert_eq!(interrupts.len(), 1, "{interrupts:?}");
    let interrupt = interrupts[0];
    assert_eq!(interrupt.get("cpu").and_then(Value::as_u64), Some(1));
    assert_eq!(
        interrupt.get("flush_cause").and_then(Value::as_str),
        Some("interrupt_redirect")
    );
    assert_eq!(
        interrupt.get("redirect_target").and_then(Value::as_str),
        Some(format!("0x{:x}", program.handler_pc).as_str())
    );
    let terminal_sequence = assert_interrupt_terminal_advance(&json, interrupt, 1, program.loop_pc);
    let flushed = record_array(interrupt, "flushed");
    assert_eq!(flushed.len(), 1, "{interrupt:?}");
    assert_eq!(
        flushed[0].get("stage").and_then(Value::as_str),
        Some("commit")
    );
    let flushed_sequence = json_record_u64(&flushed[0], "sequence");
    assert_ne!(terminal_sequence, flushed_sequence);
    let fetch_pcs = fetch_trace_pcs_by_sequence(&json, 1);
    assert_eq!(
        fetch_pcs.get(&flushed_sequence).map(String::as_str),
        Some(format!("0x{:x}", program.loop_pc).as_str())
    );
    let waits = trace
        .iter()
        .filter(|record| {
            record.get("cpu").and_then(Value::as_u64) == Some(1)
                && record.get("stall_cause").and_then(Value::as_str) == Some("fetch_wait")
                && record_array(record, "ordering_blocked")
                    .iter()
                    .any(|blocked| {
                        blocked.get("stage").and_then(Value::as_str) == Some("fetch2")
                            && json_record_u64(blocked, "sequence") == flushed_sequence
                    })
        })
        .collect::<Vec<_>>();
    assert_eq!(waits.len(), 12, "{waits:?}");
    assert!(waits
        .iter()
        .all(|record| json_record_u64(record, "stall_cycles") == 1));
    let ordering_expected = BacklogFlushTotals {
        sequences: 1,
        stall_records: 12,
        stall_cycles: 12,
    };
    let commit_ordering_expected = BacklogFlushTotals {
        sequences: 1,
        stall_records: 8,
        stall_cycles: 8,
    };
    let resource_expected = ordering_expected;
    let aggregate_expected = BacklogFlushTotals {
        sequences: 2,
        stall_records: 32,
        stall_cycles: 32,
    };
    assert_eq!(
        raw_interrupt_backlog_flush_totals(trace, CPU1_FETCH_WAIT_INTERRUPT),
        ordering_expected,
        "{trace:?}"
    );
    assert_eq!(
        raw_interrupt_backlog_flush_totals(trace, CPU1_FETCH_WAIT_COMMIT_INTERRUPT),
        commit_ordering_expected,
        "{trace:?}"
    );
    assert_eq!(
        raw_interrupt_backlog_flush_totals(trace, CPU1_FETCH_WAIT_RESOURCE_INTERRUPT),
        resource_expected,
        "{trace:?}"
    );
    assert_interrupt_backlog_flush_outputs(
        &json,
        &stdout,
        CPU1_FETCH_WAIT_INTERRUPT,
        aggregate_expected,
        &[
            (CPU1_FETCH_WAIT_RESOURCE_INTERRUPT, resource_expected),
            (CPU1_FETCH_WAIT_COMMIT_INTERRUPT, commit_ordering_expected),
            (CPU1_FETCH_WAIT_INTERRUPT, ordering_expected),
        ],
    );

    let suppressed_stdout = run_interrupt_program_with_args(
        &program.path,
        "json",
        Some("Pipeline,Fetch"),
        &[
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--riscv-branch-lookahead",
            "1",
        ],
    );
    let suppressed_json: Value = serde_json::from_str(&suppressed_stdout).unwrap();
    let suppressed_trace = suppressed_json
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("debug pipeline trace array");
    assert_pipeline_summary_matches_trace(&suppressed_json);
    assert_software_interrupt_handler_completed_for_cpu(&suppressed_json, 1);
    assert_ipi_fixture_evidence(&suppressed_json);
    assert_eq!(
        json_path_u64(
            &suppressed_json,
            "/cores/0/in_order_pipeline/interrupt_redirects"
        ),
        0
    );
    assert_cpu_interrupt_backlog_empty(
        &suppressed_json,
        &suppressed_stdout,
        CPU0_FETCH_WAIT_INTERRUPT,
    );
    let suppressed_interrupts = suppressed_trace
        .iter()
        .filter(|record| {
            record.get("redirect_cause").and_then(Value::as_str) == Some("interrupt_redirect")
        })
        .collect::<Vec<_>>();
    assert_eq!(suppressed_interrupts.len(), 1, "{suppressed_interrupts:?}");
    let suppressed_interrupt = suppressed_interrupts[0];
    assert_eq!(
        suppressed_interrupt.get("cpu").and_then(Value::as_u64),
        Some(1)
    );
    assert!(record_array(suppressed_interrupt, "flushed").is_empty());
    assert!(suppressed_interrupt
        .get("flush_cause")
        .is_none_or(Value::is_null));
    assert_eq!(
        suppressed_interrupt
            .get("redirect_target")
            .and_then(Value::as_str),
        Some(format!("0x{:x}", program.handler_pc).as_str())
    );
    assert_interrupt_terminal_advance(&suppressed_json, suppressed_interrupt, 1, program.loop_pc);
    let ordering_expected = BacklogFlushTotals::default();
    let commit_ordering_expected = BacklogFlushTotals::default();
    let resource_expected = BacklogFlushTotals {
        sequences: 1,
        stall_records: 12,
        stall_cycles: 12,
    };
    assert_eq!(
        raw_interrupt_backlog_flush_totals(suppressed_trace, CPU1_FETCH_WAIT_INTERRUPT),
        ordering_expected,
        "{suppressed_trace:?}"
    );
    assert_eq!(
        raw_interrupt_backlog_flush_totals(suppressed_trace, CPU1_FETCH_WAIT_COMMIT_INTERRUPT),
        commit_ordering_expected,
        "{suppressed_trace:?}"
    );
    assert_eq!(
        raw_interrupt_backlog_flush_totals(suppressed_trace, CPU1_FETCH_WAIT_RESOURCE_INTERRUPT),
        resource_expected,
        "{suppressed_trace:?}"
    );
    assert_interrupt_backlog_flush_outputs(
        &suppressed_json,
        &suppressed_stdout,
        CPU1_FETCH_WAIT_INTERRUPT,
        resource_expected,
        &[
            (CPU1_FETCH_WAIT_RESOURCE_INTERRUPT, resource_expected),
            (CPU1_FETCH_WAIT_COMMIT_INTERRUPT, commit_ordering_expected),
            (CPU1_FETCH_WAIT_INTERRUPT, ordering_expected),
        ],
    );
}

fn assert_software_interrupt_handler_completed_for_cpu(json: &Value, cpu: u64) {
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
                RISCV_INTERRUPT_BIT | RISCV_SUPERVISOR_SOFTWARE_INTERRUPT
            )
            .as_str()
        )
    );
    assert_eq!(
        json.pointer(&format!("/cores/{cpu}/registers/x7"))
            .and_then(Value::as_str),
        Some("0x4d")
    );
}

fn assert_ipi_fixture_evidence(json: &Value) {
    let hsm_events = json
        .pointer("/riscv_sbi_hsm_events")
        .and_then(Value::as_array)
        .expect("HSM event array");
    assert_eq!(hsm_events.len(), 1, "{hsm_events:?}");
    let hsm = &hsm_events[0];
    assert_eq!(hsm.get("source_cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(hsm.get("function").and_then(Value::as_u64), Some(0));
    assert_eq!(hsm.get("target_hart").and_then(Value::as_u64), Some(1));
    assert_eq!(hsm.get("opaque").and_then(Value::as_str), Some("0x68"));

    let ipis = json
        .pointer("/riscv_sbi_ipis")
        .and_then(Value::as_array)
        .expect("IPI event array");
    assert_eq!(ipis.len(), 1, "{ipis:?}");
    let ipi = &ipis[0];
    assert_eq!(ipi.get("source_cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(ipi.get("hart_mask").and_then(Value::as_str), Some("0x2"));
    assert_eq!(
        ipi.get("hart_mask_base").and_then(Value::as_str),
        Some("0x0")
    );
    let targets = ipi
        .get("targets")
        .and_then(Value::as_array)
        .expect("IPI target list");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].as_u64(), Some(1));
    assert!(json
        .pointer("/riscv_sbi_timers")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
}
