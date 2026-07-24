use super::*;

fn prepared_scalar_row(
    runtime: &O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
    issue_tick: u64,
    destination: Register,
    value: u64,
) -> O3PreparedLiveIssue {
    O3PreparedLiveIssue {
        candidate: runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .expect("scalar candidate is available"),
        consumed_requests: vec![request(request_sequence)],
        issue_tick,
        execution: rem6_isa_riscv::RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            vec![RegisterWrite::new(destination, value)],
            None,
        ),
    }
}

fn bind_scalar_row(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
) {
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
        0,
    ));
}

struct WritebackReplanRollbackFixture {
    runtime: O3RuntimeState,
    prepared: Vec<O3PreparedLiveIssue>,
    child_sequence: u64,
    rejected_sequence: u64,
}

fn writeback_replan_rollback_fixture() -> WritebackReplanRollbackFixture {
    const PRODUCER_PC: u64 = 0x9000;
    const CHILD_PC: u64 = 0x9004;
    const FIRST_PC: u64 = 0x9008;
    const SECOND_PC: u64 = 0x900c;
    const REJECTED_PC: u64 = 0x9010;

    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime.set_scalar_memory_window_limit(4);
    let producer = addi(6, 0, 1);
    let child = addi(7, 6, 1);
    let rows = [addi(20, 0, 1), addi(21, 0, 2), addi(22, 0, 3)];
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(PRODUCER_PC), producer),
                (Address::new(CHILD_PC), child),
            ],
        ),
        2
    );
    runtime
        .stage_live_retire_window(
            Address::new(FIRST_PC),
            rows[0],
            0,
            [
                (Address::new(SECOND_PC), rows[1]),
                (Address::new(REJECTED_PC), rows[2]),
            ],
        )
        .unwrap();
    let producer_sequence = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(PRODUCER_PC))
        .unwrap()
        .sequence();
    for (pc, instruction, request_sequence) in [
        (PRODUCER_PC, producer, 50),
        (CHILD_PC, child, 51),
        (FIRST_PC, rows[0], 60),
        (SECOND_PC, rows[1], 61),
        (REJECTED_PC, rows[2], 62),
    ] {
        bind_scalar_row(&mut runtime, pc, instruction, request_sequence);
    }
    let producer_row = prepared_scalar_row(&runtime, PRODUCER_PC, producer, 50, 50, reg(6), 1);
    assert!(runtime
        .record_live_speculative_execution(
            producer_row.candidate,
            &producer_row.consumed_requests,
            producer_row.issue_tick,
            producer_row.execution,
        )
        .unwrap());
    let child_row = prepared_scalar_row(&runtime, CHILD_PC, child, 51, 50, reg(7), 2);
    let child_sequence = child_row.candidate.sequence();
    assert_eq!(
        child_row.candidate.producer_sequences(),
        [producer_sequence]
    );
    assert!(runtime
        .record_live_speculative_execution(
            child_row.candidate,
            &child_row.consumed_requests,
            child_row.issue_tick,
            child_row.execution,
        )
        .unwrap());

    let mut prepared = vec![
        prepared_scalar_row(&runtime, FIRST_PC, rows[0], 60, 49, reg(20), 1),
        prepared_scalar_row(&runtime, SECOND_PC, rows[1], 61, 49, reg(21), 2),
        prepared_scalar_row(&runtime, REJECTED_PC, rows[2], 62, 49, reg(22), 3),
    ];
    let rejected_sequence = prepared[2].candidate.sequence();
    prepared[2].execution = rem6_isa_riscv::RiscvExecutionRecord::new(
        rows[2],
        REJECTED_PC,
        REJECTED_PC + 4,
        Vec::new(),
        None,
    );
    WritebackReplanRollbackFixture {
        runtime,
        prepared,
        child_sequence,
        rejected_sequence,
    }
}

#[test]
fn live_issue_transaction_writeback_replan_rollback_restores_ports_and_descendants() {
    let probe = writeback_replan_rollback_fixture();
    let ready = probe
        .prepared
        .iter()
        .map(|row| O3LiveWritebackReady::fixed_fu(row.candidate.sequence(), raw_ready_tick(row)))
        .collect::<Vec<_>>();
    let pre_replan_trace_records = probe.runtime.live_issue.trace_records().len();
    let mut probe_runtime = probe.runtime;
    probe_runtime.reserve_writeback_completions(ready).unwrap();
    assert!(probe_runtime
        .live_speculative_executions
        .iter()
        .all(|execution| execution.sequence != probe.child_sequence));
    assert!(probe_runtime
        .live_issue
        .resident_sequences()
        .contains(&probe.child_sequence));
    assert!(probe_runtime.live_issue.trace_records().len() > pre_replan_trace_records);

    let fixture = writeback_replan_rollback_fixture();
    let mut runtime = fixture.runtime;
    let before = touched(&runtime);
    assert!(matches!(
        runtime.record_live_issue_batch(fixture.prepared),
        Err(O3LiveIssueTransactionError::Runtime(
            O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence }
        ))
            if sequence == fixture.rejected_sequence
    ));
    assert_eq!(touched(&runtime), before);
    assert!(runtime
        .live_speculative_executions
        .iter()
        .any(|execution| execution.sequence == fixture.child_sequence));
}
