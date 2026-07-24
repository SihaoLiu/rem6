use super::*;

#[test]
fn live_issue_queue_materialization_is_sequence_ordered_and_requires_bound_packets() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    bind_queue_row(&mut runtime, THIRD_PC, instructions[2], 13);

    let first = materialized_queue(&runtime);
    assert_eq!(
        first.sequences().collect::<Vec<_>>(),
        vec![sequences[0], sequences[2]]
    );

    bind_queue_row(&mut runtime, SECOND_PC, instructions[1], 12);
    let second = materialized_queue(&runtime);
    assert_eq!(second.sequences().collect::<Vec<_>>(), sequences);
}

#[test]
fn live_issue_queue_materializes_resident_sequences_without_rob_inventory_scan() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_rows(&mut runtime, instructions);
    assert!(runtime.live_issue.remove_exact_at(
        sequences[1],
        O3LiveIssueTraceAction::Retired,
        Address::new(SECOND_PC),
        O3LiveIssueTraceClass::IntegerMulDiv,
        20,
    ));

    let queue = materialized_queue(&runtime);
    assert_eq!(
        queue.sequences().collect::<Vec<_>>(),
        vec![sequences[0], sequences[2]]
    );
    assert!(runtime.live_staged_issue_packet(sequences[1]).is_some());
}

#[test]
fn live_issue_queue_rejects_stale_ordinary_resident_sequence() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_row_at(&mut runtime, BRANCH_PC, instructions[0], 11, 20);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequences[0]));

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence })
            if sequence == sequences[0]
    ));
}

#[test]
fn live_issue_queue_returns_exact_pending_replay_boundary() {
    let mut runtime = O3RuntimeState::default();
    let sequence = stage_queue_pending_row(&mut runtime);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()).unwrap(),
        O3LiveIssueQueueCapture::ReplayPending(replay) if replay == sequence
    ));
}

#[test]
fn live_issue_queue_preserves_architectural_sequence_order() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_row_at(&mut runtime, THIRD_PC, instructions[2], 13, 20);
    bind_queue_row_at(&mut runtime, BRANCH_PC, instructions[0], 11, 20);
    bind_queue_row_at(&mut runtime, SECOND_PC, instructions[1], 12, 20);

    let queue = materialized_queue(&runtime);
    assert_eq!(queue.sequences().collect::<Vec<_>>(), sequences);
}

#[test]
fn live_issue_queue_lookup_is_sequence_owned() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_rows(&mut runtime, instructions);
    let queue = materialized_queue(&runtime);
    let middle = queue.entry(sequences[1]).expect("middle queue entry");
    assert_eq!(middle.scheduling().pc(), Address::new(SECOND_PC));
    assert_eq!(middle.packet().instruction(), instructions[1]);
    assert_eq!(middle.packet().consumed_requests(), [request(12)]);
    assert!(queue.entry(99).is_none());
}

#[test]
fn live_issue_queue_does_not_enqueue_unsupported_bound_packets() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    for (pc, raw, request_sequence) in [
        (BRANCH_PC, 0x0020_81d3, 11),
        (SECOND_PC, 0x0220_81d7, 12),
        (THIRD_PC, 0x0000_0073, 13),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        assert!(runtime
            .stage_live_instruction(Address::new(pc), decoded.instruction(), 20)
            .is_some());
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }

    assert!(runtime.live_issue.resident_sequences().is_empty());
    let queue = materialized_queue(&runtime);
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_rejects_duplicate_sequence_inventory() {
    let (mut runtime, instructions, _) = queue_rows();
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    let queue = materialized_queue(&runtime);
    let duplicate = queue.entries()[0].clone();
    let duplicate_sequence = duplicate.sequence();

    assert!(matches!(
        O3LiveIssueQueue::from_entries_for_test(vec![duplicate.clone(), duplicate]),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence }) if sequence == duplicate_sequence
    ));
}

#[test]
fn live_issue_queue_excludes_invalidated_descendant_identities() {
    let (mut runtime, instructions, _) = queue_rows();
    bind_queue_rows(&mut runtime, instructions);
    let mut hart = RiscvHartState::new(BRANCH_PC);
    let execution = hart.execute_decoded(decoded(instructions[0])).unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(BRANCH_PC, 99), instructions[0], execution),
        &[request(99)],
        30,
    );

    let queue = materialized_queue(&runtime);
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_rejects_materialized_pending_resident_sequence() {
    let mut runtime = O3RuntimeState::default();
    let sequence = stage_queue_pending_row(&mut runtime);
    runtime.set_pending_data_address_materialized_for_test(
        40,
        queue_load_event(BRANCH_PC, 11, 13, 12, 0x9100),
    );

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence: stale })
            if stale == sequence
    ));
}
