use super::*;
use crate::o3_runtime::o3_runtime_pending_address_set::O3_PENDING_DATA_ADDRESS_CAPACITY;
use rem6_isa_riscv::RiscvHartState;
const RESULT_WRITEBACK_TICK: u64 = 41;
const RESULT_VALUE: u64 = 0xa000;
fn scalar_suffix() -> Vec<(Address, RiscvInstruction)> {
    vec![
        staged_instruction(SCALAR_SUFFIX_PC, addi(8, 7, 1)),
        staged_instruction(EXTRA_SUFFIX_PC, addi(9, 8, 1)),
    ]
}
fn pending_rows(runtime: &O3RuntimeState) -> Vec<O3PendingDataAddress> {
    runtime.pending_data_addresses.iter().cloned().collect()
}
fn pending_sequences(runtime: &O3RuntimeState) -> Vec<u64> {
    pending_rows(runtime)
        .into_iter()
        .map(|row| row.sequence)
        .collect()
}
fn assert_two_pending_allocation(runtime: &O3RuntimeState) {
    let snapshot = runtime.snapshot();
    let sequences = pending_sequences(runtime);
    assert_eq!(snapshot.reorder_buffer().len(), 4);
    assert_eq!(snapshot.load_store_queue().len(), 3);
    assert_eq!(sequences.len(), 2);
    assert!(sequences[0] < sequences[1]);
    let pending_lsq = snapshot
        .load_store_queue()
        .iter()
        .filter(|entry| sequences.contains(&entry.sequence()))
        .collect::<Vec<_>>();
    assert_eq!(pending_lsq.len(), 2);
    assert!(pending_lsq.iter().all(|entry| {
        entry.kind() == O3LoadStoreQueueKind::Load
            && entry.address().is_none()
            && entry.bytes() == 8
    }));
    let head = integer_mapping(runtime, 5).expect("head destination is renamed");
    let first = integer_mapping(runtime, 6).expect("first pending destination is renamed");
    let second = integer_mapping(runtime, 7).expect("second pending destination is renamed");
    assert_ne!(head, first);
    assert_ne!(head, second);
    assert_ne!(first, second);
    assert!(integer_mapping(runtime, 8).is_some());
    assert_eq!(
        pc_rows(runtime),
        [
            HEAD_PC,
            FIRST_PENDING_PC,
            SECOND_PENDING_PC,
            SCALAR_SUFFIX_PC,
        ]
        .map(Address::new)
        .to_vec()
    );
    assert_eq!(runtime.live_data_accesses.len(), 1);
    assert!(runtime.pending_data_accesses.is_empty());
}
fn ready_two_pending_issue(
    issue_width: usize,
    chained: bool,
) -> (
    O3RuntimeState,
    RiscvHartState,
    O3LiveIssueHeadReservation,
    Vec<O3LiveIssueRequest>,
) {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(fixture.runtime.set_issue_width(issue_width));
    let pending = if chained {
        chained_pending_requests()
    } else {
        sibling_pending_requests()
    };
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            pending,
            [staged_instruction(SCALAR_SUFFIX_PC, addi(8, 5, 1))],
        ),
        3
    );
    assert!(fixture.runtime.bind_live_staged_fetch_identity(
        Address::new(SCALAR_SUFFIX_PC),
        decoded(addi(8, 5, 1)).instruction(),
        &[request(13)],
    ));
    let head = fixture
        .runtime
        .live_data_access_head_reservation(fixture.head_fetch)
        .expect("head reservation");
    let mut completed = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(fixture
        .runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            RESULT_WRITEBACK_TICK - 1,
            9,
            Some(&RESULT_VALUE.to_le_bytes()),
        )
        .unwrap());
    assert!(fixture
        .runtime
        .take_ready_live_data_access_event(RESULT_WRITEBACK_TICK)
        .is_some());
    let requests = [
        (FIRST_PENDING_PC, ld(6, 5, 0), 11),
        (SECOND_PENDING_PC, ld(7, if chained { 6 } else { 5 }, 8), 12),
        (SCALAR_SUFFIX_PC, addi(8, 5, 1), 13),
    ]
    .into_iter()
    .map(|(pc, raw, sequence)| {
        O3LiveIssueRequest::new(Address::new(pc), vec![request(sequence)], decoded(raw))
    })
    .collect();
    let mut hart = RiscvHartState::new(HEAD_PC);
    hart.write(reg(5), 0xdead_beef);
    (fixture.runtime, hart, head, requests)
}
fn bind_oldest_pending(runtime: &mut O3RuntimeState) {
    let execution = runtime
        .pending_data_address_execution()
        .cloned()
        .expect("oldest pending execution");
    assert!(runtime
        .bind_pending_data_address_issue(
            &execution,
            request(30),
            Address::new(RESULT_VALUE),
            RESULT_WRITEBACK_TICK,
        )
        .is_some());
}
#[test]
fn two_pending_collection_orders_by_sequence_and_rejects_third() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            sibling_pending_requests(),
            [],
        ),
        2
    );
    let rows = pending_rows(&fixture.runtime);
    assert_eq!(rows.len(), O3_PENDING_DATA_ADDRESS_CAPACITY);
    assert!(rows[0].sequence < rows[1].sequence);
    let mut collection = O3PendingDataAddresses::default();
    assert!(collection.try_push(rows[0].clone()));
    assert!(collection.try_push(rows[1].clone()));
    let mut third = rows[1].clone();
    third.sequence = third.sequence.saturating_add(1);
    third.fetch = fetch_event_with_raw(SCALAR_SUFFIX_PC, 13, ld(8, 5, 16));
    third.consumed_requests = vec![request(13)];
    third.fetch_predecessor_request = request(12);
    assert!(!collection.try_push(third));
    assert_eq!(collection.len(), O3_PENDING_DATA_ADDRESS_CAPACITY);
    let mut out_of_order = O3PendingDataAddresses::default();
    assert!(out_of_order.try_push(rows[1].clone()));
    assert!(!out_of_order.try_push(rows[0].clone()));
    assert_eq!(
        out_of_order
            .iter()
            .map(O3PendingDataAddress::sequence)
            .collect::<Vec<_>>(),
        [rows[1].sequence]
    );
    let mut three = PendingAddressFixture::new(4, 4);
    let before = three.runtime.clone();
    let mut requests = sibling_pending_requests();
    requests.push(pending_request(
        request(12),
        13,
        SCALAR_SUFFIX_PC,
        ld(8, 5, 16),
        reg(5),
    ));
    assert_eq!(
        three
            .runtime
            .stage_pending_data_address_window(three.head_fetch, requests, []),
        0
    );
    assert_eq!(three.runtime, before);
}
#[test]
fn two_pending_sibling_stages_two_addressless_lsq_rows_and_one_suffix() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            sibling_pending_requests(),
            scalar_suffix(),
        ),
        3
    );
    assert_two_pending_allocation(&fixture.runtime);
    let rows = pending_rows(&fixture.runtime);
    assert_eq!(rows[0].producer_register, reg(5));
    assert_eq!(rows[1].producer_register, reg(5));
    assert_eq!(rows[0].producer_sequence, rows[0].root_head.sequence);
    assert_eq!(rows[1].producer_sequence, rows[0].root_head.sequence);
    assert_eq!(rows[0].root_head, rows[1].root_head);
    assert_eq!(rows[0].fetch_predecessor_request, request(10));
    assert_eq!(rows[1].fetch_predecessor_request, request(11));
    assert!(fixture.runtime.pending_data_address_owner_is_consistent());
}
#[test]
fn two_pending_chain_stages_second_with_first_as_immediate_producer() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            chained_pending_requests(),
            scalar_suffix(),
        ),
        3
    );
    assert_two_pending_allocation(&fixture.runtime);
    let rows = pending_rows(&fixture.runtime);
    let expected_range = AddressRange::new(
        Address::new(0x9000),
        AccessSize::new(8).expect("doubleword size"),
    )
    .expect("head range");
    assert_eq!(rows[0].producer_register, reg(5));
    assert_eq!(rows[1].producer_register, reg(6));
    assert_eq!(rows[0].producer_sequence, rows[0].root_head.sequence);
    assert_eq!(rows[1].producer_sequence, rows[0].sequence);
    assert_eq!(rows[0].root_head, rows[1].root_head);
    assert_eq!(rows[0].root_head.fetch_request, fixture.head_fetch);
    assert_eq!(rows[0].root_head.range, expected_range);
    assert!(!rows[0].root_head.atomic_head);
    assert!(fixture.runtime.pending_data_address_owner_is_consistent());
    let first_sequence = rows[0].sequence;
    let root_head = rows[0].root_head;
    let producer_value = 0x9000_u64;
    let mut completed_head = load_event(HEAD_PC, 10, 5, 2, producer_value);
    completed_head.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(fixture
        .runtime
        .complete_live_data_access_response(
            &completed_head,
            request(20),
            40,
            9,
            Some(&producer_value.to_le_bytes()),
        )
        .unwrap());
    assert!(fixture
        .runtime
        .take_ready_live_data_access_event(41)
        .is_some());

    let first_decoded = decoded(ld(6, 5, 0));
    let first_consumed = [request(11)];
    let scheduling = fixture
        .runtime
        .live_issue_scheduling_candidate(
            0,
            Address::new(FIRST_PENDING_PC),
            first_decoded.instruction(),
            &first_consumed,
        )
        .expect("first pending scheduling candidate");
    let candidate = fixture
        .runtime
        .materialize_live_speculative_issue_candidate(&scheduling)
        .expect("first pending materialization candidate");
    let mut hart = RiscvHartState::new(FIRST_PENDING_PC);
    for write in candidate.forwarded_register_writes() {
        hart.write(write.register(), write.value());
    }
    let execution = hart
        .execute_decoded(first_decoded)
        .expect("first pending load materializes");
    assert!(fixture
        .runtime
        .record_pending_data_address_materialization(candidate, &first_consumed, 41, execution)
        .unwrap());
    let materialized = fixture
        .runtime
        .pending_data_address_execution()
        .cloned()
        .expect("first pending execution");
    assert_eq!(
        fixture.runtime.bind_pending_data_address_issue(
            &materialized,
            request(30),
            Address::new(producer_value),
            41,
        ),
        Some(first_consumed.to_vec())
    );

    let remaining = pending_rows(&fixture.runtime);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].producer_sequence, first_sequence);
    assert_eq!(remaining[0].root_head, root_head);
    assert_eq!(
        fixture.runtime.pending_data_address_wakeup_seed(),
        Some((
            request(11),
            [SECOND_PENDING_PC, SCALAR_SUFFIX_PC]
                .map(Address::new)
                .to_vec(),
        ))
    );
    assert!(fixture.runtime.pending_data_address_owner_is_consistent());
}

#[test]
fn two_pending_staging_failure_rolls_back_both_rows_and_rename() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    let before = fixture.runtime.clone();

    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            sibling_pending_requests(),
            [staged_instruction(SECOND_PENDING_PC, addi(8, 7, 1))],
        ),
        0
    );

    assert_eq!(fixture.runtime, before);
    assert_eq!(fixture.runtime.pending_data_address_count(), 0);
    assert_eq!(fixture.runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(fixture.runtime.snapshot().load_store_queue().len(), 1);
    assert_eq!(integer_mapping(&fixture.runtime, 6), None);
    assert_eq!(integer_mapping(&fixture.runtime, 7), None);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            sibling_pending_requests(),
            scalar_suffix(),
        ),
        3
    );
}

#[test]
fn two_pending_discard_from_second_preserves_first_row() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            sibling_pending_requests(),
            scalar_suffix(),
        ),
        3
    );
    let sequences = pending_sequences(&fixture.runtime);

    fixture
        .runtime
        .discard_pending_data_address_from(sequences[1]);

    assert_eq!(fixture.runtime.pending_data_address_count(), 1);
    assert_eq!(pending_sequences(&fixture.runtime), [sequences[0]]);
    assert!(fixture.runtime.pending_data_address_owns_fetch(request(11)));
    assert!(!fixture.runtime.pending_data_address_owns_fetch(request(12)));
    assert_eq!(
        pc_rows(&fixture.runtime),
        [HEAD_PC, FIRST_PENDING_PC].map(Address::new).to_vec()
    );
    assert_eq!(fixture.runtime.snapshot().load_store_queue().len(), 2);
    assert!(fixture
        .runtime
        .snapshot()
        .load_store_queue()
        .iter()
        .any(|entry| entry.sequence() == sequences[0] && entry.address().is_none()));
    assert_eq!(integer_mapping(&fixture.runtime, 7), None);
    assert_eq!(integer_mapping(&fixture.runtime, 8), None);
    assert_eq!(
        fixture.runtime.live_data_access_younger_sequences,
        [sequences[0]].into_iter().collect()
    );
    assert!(fixture.runtime.pending_data_address_owner_is_consistent());
}

#[test]
fn two_pending_retirement_accounting_counts_zero_one_two_rows() {
    assert_eq!(
        O3RuntimeState::default().pending_live_data_access_retirement_count(),
        0
    );

    let mut one = PendingAddressFixture::new(4, 4);
    assert_eq!(one.runtime.pending_live_data_access_retirement_count(), 1);
    assert_eq!(
        one.runtime.stage_pending_data_address_window(
            one.head_fetch,
            vec![pending_request(
                request(10),
                11,
                FIRST_PENDING_PC,
                ld(6, 5, 0),
                reg(5),
            )],
            [],
        ),
        1
    );
    assert_eq!(one.runtime.pending_data_address_count(), 1);
    assert_eq!(one.runtime.pending_live_data_access_retirement_count(), 2);

    let mut two = PendingAddressFixture::new(4, 4);
    assert_eq!(
        two.runtime.stage_pending_data_address_window(
            two.head_fetch,
            sibling_pending_requests(),
            [],
        ),
        2
    );
    assert_eq!(two.runtime.pending_data_address_count(), 2);
    assert_eq!(two.runtime.pending_live_data_access_retirement_count(), 3);
}
#[test]
fn two_pending_staging_removes_both_authorizations_only_after_schedule() {
    let core = core_with_completed_fetches([
        (10, HEAD_PC, ld(5, 2, 0)),
        (11, FIRST_PENDING_PC, ld(6, 5, 0)),
        (12, SECOND_PENDING_PC, ld(7, 5, 8)),
        (13, SCALAR_SUFFIX_PC, addi(8, 5, 1)),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let fetch_events = core.core.state.lock().unwrap().events.clone();
    let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    let mut state = core.state.lock().unwrap();
    let unrelated = request(99);
    {
        let authorizations = &mut state.memory_result_window_authorizations;
        assert!(authorizations.contains_key(&request(11)));
        assert!(authorizations.contains_key(&request(12)));
        let authorization = authorizations[&request(11)];
        authorizations.insert(unrelated, authorization);
    }
    assert!(state.o3_runtime.stage_live_data_access_issue(
        &head,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    stage_o3_data_access_younger_window(&mut state, &head, 31, &fetch_events);
    assert_eq!(state.o3_runtime.pending_data_address_count(), 2);
    let authorizations = &state.memory_result_window_authorizations;
    assert!(!authorizations.contains_key(&request(11)));
    assert!(!authorizations.contains_key(&request(12)));
    assert!(authorizations.contains_key(&unrelated));
}
#[test]
fn two_pending_siblings_width_one_issue_oldest_across_ticks() {
    let (mut runtime, hart, head, requests) = ready_two_pending_issue(1, false);
    runtime
        .schedule_live_speculative_issues(&hart, head, RESULT_WRITEBACK_TICK, &requests)
        .unwrap();
    let rows = pending_rows(&runtime);
    assert_eq!(rows[0].selected_issue_tick, Some(RESULT_WRITEBACK_TICK));
    assert_eq!(rows[1].requested_wake_tick, Some(RESULT_WRITEBACK_TICK + 1));
    runtime
        .schedule_live_speculative_issues(&hart, head, RESULT_WRITEBACK_TICK + 1, &requests)
        .unwrap();
    let rows = pending_rows(&runtime);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].selected_issue_tick, Some(RESULT_WRITEBACK_TICK));
    assert_eq!(rows[0].requested_wake_tick, None);
    assert_eq!(rows[1].selected_issue_tick, Some(RESULT_WRITEBACK_TICK + 1));
    assert!(rows[1].materialized.is_some() && rows[1].requested_wake_tick.is_none());
}
#[test]
fn two_pending_siblings_width_two_keep_one_memory_slot_and_coissue_scalar() {
    let (mut runtime, hart, head, requests) = ready_two_pending_issue(2, false);
    runtime
        .schedule_live_speculative_issues(&hart, head, RESULT_WRITEBACK_TICK, &requests)
        .unwrap();
    let rows = pending_rows(&runtime);
    assert_eq!(
        rows.iter()
            .filter(|row| row.selected_issue_tick == Some(RESULT_WRITEBACK_TICK))
            .count(),
        1
    );
    assert_eq!(rows[1].requested_wake_tick, Some(RESULT_WRITEBACK_TICK + 1));
    assert!(runtime.live_speculative_executions.iter().any(|row| {
        row.execution.pc() == SCALAR_SUFFIX_PC && row.issue_tick == RESULT_WRITEBACK_TICK
    }));
}
#[test]
fn two_pending_chain_initial_schedule_waits_on_first_sequence() {
    let (mut runtime, hart, head, requests) = ready_two_pending_issue(2, true);
    let sequences = pending_sequences(&runtime);
    let second = runtime
        .live_issue_scheduling_candidate(
            1,
            Address::new(SECOND_PENDING_PC),
            decoded(ld(7, 6, 8)).instruction(),
            &[request(12)],
        )
        .expect("second pending candidate");
    assert_eq!(second.data_producers()[0].sequence(), sequences[0]);
    runtime
        .schedule_live_speculative_issues(&hart, head, RESULT_WRITEBACK_TICK, &requests)
        .unwrap();
    let rows = pending_rows(&runtime);
    assert_eq!(rows[0].selected_issue_tick, Some(RESULT_WRITEBACK_TICK));
    assert_eq!(
        (rows[1].selected_issue_tick, rows[1].requested_wake_tick),
        (None, None)
    );
}
#[test]
fn two_pending_typed_wake_seed_separates_second_fetch_predecessor() {
    let (mut runtime, hart, head, requests) = ready_two_pending_issue(2, true);
    runtime
        .schedule_live_speculative_issues(&hart, head, RESULT_WRITEBACK_TICK, &requests)
        .unwrap();
    let first_sequence = pending_sequences(&runtime)[0];
    bind_oldest_pending(&mut runtime);
    let seed = runtime.pending_data_address_wake_seed().unwrap();
    assert_eq!(seed.fetch_predecessor_request(), request(11));
    assert_eq!(
        seed.head_reservation(),
        O3LiveIssueHeadReservation::memory(first_sequence, 0)
    );
    let expected_pcs = [SECOND_PENDING_PC, SCALAR_SUFFIX_PC].map(Address::new);
    assert_eq!(seed.younger_pcs(), expected_pcs);
}
#[test]
fn two_pending_resource_wake_updates_only_blocked_row() {
    let (mut runtime, hart, head, requests) = ready_two_pending_issue(2, false);
    runtime
        .schedule_live_speculative_issues(&hart, head, RESULT_WRITEBACK_TICK, &requests)
        .unwrap();
    let rows = pending_rows(&runtime);
    assert_eq!(rows[0].requested_wake_tick, None);
    assert_eq!(rows[1].requested_wake_tick, Some(RESULT_WRITEBACK_TICK + 1));
    assert_eq!(
        runtime.pending_data_address_wake_tick(),
        Some(RESULT_WRITEBACK_TICK + 1)
    );
}
