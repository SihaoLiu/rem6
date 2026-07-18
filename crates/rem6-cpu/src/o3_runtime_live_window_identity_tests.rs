use rem6_isa_riscv::{Register, RegisterWrite, RiscvExecutionRecord};
use rem6_memory::{Address, AgentId, MemoryRequestId};

use crate::RiscvCpuExecutionEvent;

use super::super::super::{O3RuntimeState, O3WritebackReservation};
use super::{addi, div_x3, execution_event, fetch_event, request, retire_live};

#[test]
fn mismatched_live_speculative_record_does_not_claim_early_issue() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();

    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    retire_live(&mut runtime, &divide, 29);
    runtime.record_retired_instruction_with_trace(&divide, true);
    let younger = RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 2),
        younger_instruction,
        RiscvExecutionRecord::new(
            younger_instruction,
            0x8004,
            0x8008,
            vec![RegisterWrite::new(Register::new(4).unwrap(), 2)],
            None,
        ),
    );
    retire_live(&mut runtime, &younger, 30);
    runtime.record_retired_instruction_with_trace(&younger, true);

    let trace = runtime.trace_records().last().copied().unwrap();
    assert_eq!(trace.issue_tick(), 30);
    assert_eq!(trace.commit_tick(), 30);
}

#[test]
fn mismatched_split_fetch_suffix_does_not_claim_early_issue() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(2), request(3)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    let speculative_sequence = runtime.live_speculative_executions[0].sequence;

    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    retire_live(&mut runtime, &divide, 29);
    runtime.record_retired_instruction_with_trace(&divide, true);
    let younger = execution_event(younger_instruction, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&younger, &[request(2), request(4)], 30);
    assert_eq!(
        runtime
            .writeback_reservation(speculative_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(10),
        "rebinding after the admitted tick must preserve historical occupancy"
    );
    runtime.record_retired_instruction_with_trace(&younger, true);

    let trace = runtime.trace_records().last().copied().unwrap();
    assert_eq!(trace.issue_tick(), 30);
    assert_eq!(trace.commit_tick(), 30);
}

#[test]
fn malformed_live_speculative_fetch_identity_does_not_occupy_candidate() {
    let younger_instruction = addi(4, 0);
    let malformed_identities = [
        Vec::new(),
        vec![request(2), request(2)],
        vec![request(3), request(2)],
        vec![request(2), MemoryRequestId::new(AgentId::new(8), 3)],
        vec![request(2), request(3), request(4)],
    ];

    for consumed_requests in malformed_identities {
        let mut runtime = O3RuntimeState::default();
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &consumed_requests,
                10,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();

        assert!(runtime.live_speculative_executions.is_empty());
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .is_some());
    }
}

#[test]
fn live_speculative_execution_and_fetch_identity_are_transient_across_restore() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    assert_eq!(runtime.live_speculative_executions.len(), 1);

    let checkpoint = runtime.checkpoint_payload();
    runtime.restore_checkpoint_payload(checkpoint).unwrap();
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .is_none());

    runtime.discard_live_staged_instructions();
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();

    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(3)],
            20,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    runtime.discard_live_speculative_executions();
    assert!(runtime.live_speculative_executions.is_empty());
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            candidate,
            &[request(4)],
            21,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        )
        .unwrap();
    runtime.discard_live_staged_instructions();
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
}

#[test]
fn wrong_same_pc_retirement_does_not_claim_bound_live_staged_row() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    let younger_sequence = runtime
        .snapshot
        .reorder_buffer
        .iter()
        .find(|entry| entry.pc() == Address::new(0x8004))
        .expect("younger live row")
        .sequence();
    assert_eq!(
        runtime.live_staged_sequence_for_fetch_identity(
            Address::new(0x8004),
            younger_instruction,
            &[request(2)],
        ),
        None,
        "unbound live rows must not claim selected prediction authority"
    );
    assert!(runtime.bind_live_staged_fetch_identity(Address::new(0x8000), div_x3(), &[request(1)],));
    assert!(runtime.bind_live_staged_fetch_identity(
        Address::new(0x8004),
        younger_instruction,
        &[request(2)],
    ));
    assert_eq!(
        runtime.live_staged_sequence_for_fetch_identity(
            Address::new(0x8004),
            younger_instruction,
            &[request(2)],
        ),
        Some(younger_sequence)
    );
    assert_eq!(
        runtime.live_staged_sequence_for_fetch_identity(
            Address::new(0x8004),
            addi(4, 1),
            &[request(2)],
        ),
        None
    );
    assert_eq!(
        runtime.live_staged_sequence_for_fetch_identity(
            Address::new(0x8004),
            younger_instruction,
            &[request(3)],
        ),
        None
    );

    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    runtime.retire_live_staged_instruction(&divide, &[request(1)], 29);
    let staged_row = runtime.snapshot.reorder_buffer[0];
    let stale_destination = staged_row.destination().unwrap();
    let retired_before = runtime.live_retired_instructions.clone();
    let committed_rename_before = runtime.snapshot.rename_map.clone();

    let wrong_instruction = addi(4, 1);
    let wrong = execution_event(wrong_instruction, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&wrong, &[request(2)], 30);
    assert!(runtime.snapshot.reorder_buffer.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime
        .live_retired_instructions
        .iter()
        .any(|instruction| instruction.request == request(2)));
    assert_eq!(runtime.snapshot.rename_map, committed_rename_before);

    runtime.record_retired_instruction_with_trace(&wrong, true);

    assert!(runtime.snapshot.reorder_buffer.is_empty());
    assert_eq!(runtime.live_retired_instructions, retired_before);
    let committed_x4 = runtime
        .snapshot
        .rename_map
        .iter()
        .find(|entry| entry.architectural() == 4)
        .copied()
        .expect("wrong same-PC instruction should retire through a fresh rename");
    assert_ne!(committed_x4.physical(), stale_destination);
    assert_eq!(
        runtime
            .trace_records()
            .last()
            .unwrap()
            .rob_commits_at_tick(),
        1
    );
}

#[test]
fn restored_live_staged_row_without_transient_identity_fails_closed() {
    let mut runtime = O3RuntimeState::default();
    let younger_instruction = addi(4, 0);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        Some((Address::new(0x8004), younger_instruction)),
    );
    assert!(runtime.bind_live_staged_fetch_identity(Address::new(0x8000), div_x3(), &[request(1)],));
    assert!(runtime.bind_live_staged_fetch_identity(
        Address::new(0x8004),
        younger_instruction,
        &[request(2)],
    ));

    let checkpoint = runtime.checkpoint_payload();
    runtime.restore_checkpoint_payload(checkpoint).unwrap();
    assert!(runtime.live_staged_fetch_identities.is_empty());
    let stale_destinations = runtime
        .snapshot
        .reorder_buffer
        .iter()
        .filter_map(|entry| entry.destination())
        .collect::<Vec<_>>();
    let retired_before = runtime.live_retired_instructions.clone();
    let committed_rename_before = runtime.snapshot.rename_map.clone();

    let younger = execution_event(younger_instruction, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&younger, &[request(2)], 30);
    assert_eq!(runtime.snapshot.reorder_buffer.len(), 1);
    assert_eq!(
        runtime.snapshot.reorder_buffer[0].pc(),
        Address::new(0x8000)
    );
    assert!(runtime.snapshot.reorder_buffer[0].is_live_staged());
    assert!(!runtime.snapshot.reorder_buffer[0].is_ready());
    assert!(runtime
        .live_retired_instructions
        .iter()
        .any(|instruction| instruction.request == request(2)));
    assert_eq!(runtime.snapshot.rename_map, committed_rename_before);

    runtime.record_retired_instruction_with_trace(&younger, true);

    assert_eq!(runtime.snapshot.reorder_buffer.len(), 1);
    assert_eq!(
        runtime.snapshot.reorder_buffer[0].pc(),
        Address::new(0x8000)
    );
    assert!(runtime.snapshot.reorder_buffer[0].is_live_staged());
    assert!(!runtime.snapshot.reorder_buffer[0].is_ready());
    assert_eq!(runtime.live_retired_instructions, retired_before);
    let committed_x4 = runtime
        .snapshot
        .rename_map
        .iter()
        .find(|entry| entry.architectural() == 4)
        .copied()
        .expect("post-restore instruction should retire through a fresh rename");
    assert!(!stale_destinations.contains(&committed_x4.physical()));
    assert_eq!(
        runtime
            .trace_records()
            .last()
            .unwrap()
            .rob_commits_at_tick(),
        1
    );
}

#[test]
fn invalidated_descendant_fetch_identity_keeps_retirement_authority() {
    let mut runtime = O3RuntimeState::default();
    let producer = addi(4, 0);
    let consumer = addi(5, 4);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        div_x3(),
        29,
        [
            (Address::new(0x8004), producer),
            (Address::new(0x8008), consumer),
        ],
    );
    for (pc, instruction, request_id, destination) in
        [(0x8004, producer, 2, 4), (0x8008, consumer, 3, 5)]
    {
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(request_id)],
                10,
                RiscvExecutionRecord::new(
                    instruction,
                    pc,
                    pc + 4,
                    vec![RegisterWrite::new(Register::new(destination).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();
    }

    let divide = execution_event(div_x3(), 0x8000, 1, 3);
    retire_live(&mut runtime, &divide, 29);
    runtime.record_retired_instruction_with_trace(&divide, true);
    let producer = execution_event(producer, 0x8004, 2, 4);
    runtime.retire_live_staged_instruction(&producer, &[request(4)], 30);
    runtime.record_retired_instruction_with_trace(&producer, true);

    assert!(runtime.snapshot.reorder_buffer.is_empty());
    assert!(runtime.live_retired_instructions.is_empty());
    assert!(runtime.live_speculative_executions.is_empty());
    assert_eq!(runtime.invalidated_live_staged_fetch_identities.len(), 1);
    assert!(runtime.has_live_retirement_authority());
    assert!(runtime.owns_pending_retirement_authority(request(3)));
}
