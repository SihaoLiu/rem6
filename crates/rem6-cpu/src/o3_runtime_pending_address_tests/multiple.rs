use super::*;
use crate::o3_runtime::o3_runtime_issue::queue::{O3LiveIssueQueue, O3LiveIssueQueueCapture};
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
fn stage_pending(
    fixture: &mut PendingAddressFixture,
    pending: Vec<O3PendingDataAddressRequest>,
    suffix: impl IntoIterator<Item = (Address, RiscvInstruction)>,
) -> usize {
    fixture
        .runtime
        .stage_pending_data_address_window(fixture.head_fetch, pending, suffix)
}
fn schedule(
    runtime: &mut O3RuntimeState,
    hart: &RiscvHartState,
    head: O3LiveIssueHeadReservation,
    tick: u64,
) {
    runtime
        .schedule_live_speculative_issues(hart, head, tick)
        .unwrap();
}
fn assert_two_pending_allocation(runtime: &O3RuntimeState) {
    let snapshot = runtime.snapshot();
    let sequences = runtime.pending_data_address_sequences_for_test();
    assert_eq!(
        (
            snapshot.reorder_buffer().len(),
            snapshot.load_store_queue().len(),
            sequences.len(),
        ),
        (4, 3, 2)
    );
    assert!(sequences[0] < sequences[1]);
    let pending_lsq = snapshot
        .load_store_queue()
        .iter()
        .filter(|entry| sequences.contains(&entry.sequence()))
        .collect::<Vec<_>>();
    assert!(
        pending_lsq.len() == 2
            && pending_lsq.iter().all(|entry| {
                entry.kind() == O3LoadStoreQueueKind::Load
                    && entry.address().is_none()
                    && entry.bytes() == 8
            })
    );
    let [head, first, second] =
        [5, 6, 7].map(|register| integer_mapping(runtime, register).expect("integer destination"));
    assert!(head != first && head != second && first != second);
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
    assert!(runtime.live_data_accesses.len() == 1 && runtime.pending_data_accesses.is_empty());
}
fn ready_two_pending_issue(
    issue_width: usize,
    chained: bool,
) -> (O3RuntimeState, RiscvHartState, O3LiveIssueHeadReservation) {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(fixture.runtime.set_issue_width(issue_width));
    let pending = if chained {
        chained_pending_requests()
    } else {
        sibling_pending_requests()
    };
    assert_eq!(
        stage_pending(
            &mut fixture,
            pending,
            [staged_instruction(SCALAR_SUFFIX_PC, addi(8, 5, 1))],
        ),
        3
    );
    assert!(fixture.runtime.bind_live_staged_issue_packet(
        Address::new(SCALAR_SUFFIX_PC),
        decoded(addi(8, 5, 1)),
        &[request(13)],
    ));
    let head = fixture
        .runtime
        .live_data_access_head_reservation(fixture.head_fetch)
        .expect("head reservation");
    let (_, admitted) = fixture.runtime.complete_pending_data_address_for_test(
        fixture.head_fetch,
        request(20),
        RESULT_WRITEBACK_TICK - 1,
        &RESULT_VALUE.to_le_bytes(),
    );
    assert_eq!(admitted, RESULT_WRITEBACK_TICK);
    assert!(fixture
        .runtime
        .take_ready_live_data_access_event(RESULT_WRITEBACK_TICK)
        .is_some());
    let mut hart = RiscvHartState::new(HEAD_PC);
    hart.write(reg(5), 0xdead_beef);
    (fixture.runtime, hart, head)
}
#[test]
fn pending_address_collection_orders_by_sequence_and_rejects_fourth() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(stage_pending(&mut fixture, sibling_pending_requests(), []) == 2);
    let rows = fixture.runtime.pending_data_address_rows_for_test();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].sequence < rows[1].sequence);
    let mut collection = O3PendingDataAddresses::default();
    assert!(collection.try_push(rows[0].clone()));
    assert!(collection.try_push(rows[1].clone()));
    for sequence in [13, 14] {
        let mut row = rows[1].clone();
        row.sequence = sequence;
        row.fetch = fetch_event_with_raw(
            SCALAR_SUFFIX_PC + 4 * (sequence - 13),
            sequence,
            ld(8, 5, 16),
        );
        row.consumed_requests = vec![request(sequence)];
        row.fetch_predecessor_request = request(sequence - 1);
        assert_eq!(
            collection.try_push(row),
            sequence == 13,
            "sequence {sequence}"
        );
    }
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
}
#[test]
fn two_pending_sibling_stages_two_addressless_lsq_rows_and_one_suffix() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(stage_pending(&mut fixture, sibling_pending_requests(), scalar_suffix()) == 3);
    assert_two_pending_allocation(&fixture.runtime);
    let rows = fixture.runtime.pending_data_address_rows_for_test();
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
    let (runtime, _, _) = ready_two_pending_issue(2, true);
    assert_two_pending_allocation(&runtime);
    let rows = runtime.pending_data_address_rows_for_test();
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
    assert_eq!(rows[0].root_head.fetch_request, request(10));
    assert_eq!(rows[0].root_head.range, expected_range);
    assert!(!rows[0].root_head.atomic_head);
    assert!(runtime.pending_data_address_owner_is_consistent());
}
#[test]
fn two_pending_staging_failure_rolls_back_both_rows_and_rename() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    let before = fixture.runtime.clone();
    assert_eq!(
        stage_pending(
            &mut fixture,
            sibling_pending_requests(),
            [staged_instruction(SECOND_PENDING_PC, addi(8, 7, 1))],
        ),
        0
    );
    assert_eq!(fixture.runtime, before);
    assert!(stage_pending(&mut fixture, sibling_pending_requests(), scalar_suffix()) == 3);
}
#[test]
fn two_pending_discard_from_second_preserves_first_row() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(stage_pending(&mut fixture, sibling_pending_requests(), scalar_suffix()) == 3);
    let sequences = fixture.runtime.pending_data_address_sequences_for_test();
    fixture
        .runtime
        .discard_pending_data_address_from(sequences[1]);
    assert_eq!(
        fixture.runtime.pending_data_address_sequences_for_test(),
        [sequences[0]]
    );
    assert!(
        fixture.runtime.pending_data_address_owns_fetch(request(11))
            && !fixture.runtime.pending_data_address_owns_fetch(request(12))
    );
    assert_eq!(
        pc_rows(&fixture.runtime),
        [HEAD_PC, FIRST_PENDING_PC].map(Address::new).to_vec()
    );
    let snapshot = fixture.runtime.snapshot();
    assert!(
        snapshot.load_store_queue().len() == 2
            && snapshot
                .load_store_queue()
                .iter()
                .any(|entry| entry.sequence() == sequences[0] && entry.address().is_none())
    );
    assert_eq!(
        [
            integer_mapping(&fixture.runtime, 7),
            integer_mapping(&fixture.runtime, 8)
        ],
        [None, None]
    );
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
    let pending = vec![one.pending.clone()];
    assert_eq!(stage_pending(&mut one, pending, []), 1);
    assert_eq!(
        (
            one.runtime.pending_data_address_count(),
            one.runtime.pending_live_data_access_retirement_count(),
        ),
        (1, 2)
    );
    let mut two = PendingAddressFixture::new(4, 4);
    assert_eq!(stage_pending(&mut two, sibling_pending_requests(), []), 2);
    assert_eq!(
        (
            two.runtime.pending_data_address_count(),
            two.runtime.pending_live_data_access_retirement_count(),
        ),
        (2, 3)
    );
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
    let (mut runtime, hart, head) = ready_two_pending_issue(1, false);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let rows = runtime.pending_data_address_rows_for_test();
    assert_eq!(rows[0].selected_issue_tick, Some(RESULT_WRITEBACK_TICK));
    assert_eq!(rows[1].requested_wake_tick, Some(RESULT_WRITEBACK_TICK + 1));
    let next_tick = RESULT_WRITEBACK_TICK + 1;
    schedule(&mut runtime, &hart, head, next_tick);
    let rows = runtime.pending_data_address_rows_for_test();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].selected_issue_tick, Some(RESULT_WRITEBACK_TICK));
    assert_eq!(rows[0].requested_wake_tick, None);
    assert_eq!(rows[1].selected_issue_tick, Some(RESULT_WRITEBACK_TICK + 1));
    assert!(rows[1].materialized.is_some() && rows[1].requested_wake_tick.is_none());
}
#[test]
fn two_pending_siblings_width_two_keep_one_memory_slot_and_coissue_scalar() {
    let (mut runtime, hart, head) = ready_two_pending_issue(2, false);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let rows = runtime.pending_data_address_rows_for_test();
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
    let (mut runtime, hart, head) = ready_two_pending_issue(2, true);
    let sequences = runtime.pending_data_address_sequences_for_test();
    let queue = match O3LiveIssueQueue::capture(&runtime, head).unwrap() {
        O3LiveIssueQueueCapture::Ready(queue) => queue,
        O3LiveIssueQueueCapture::ReplayPending(sequence) => {
            panic!("unexpected pending replay at {sequence}")
        }
    };
    let second = queue
        .entry(sequences[1])
        .expect("second pending queue entry");
    assert_eq!(second.packet().consumed_requests(), [request(12)]);
    let second = second.scheduling();
    assert_eq!(second.data_producers()[0].sequence(), sequences[0]);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let rows = runtime.pending_data_address_rows_for_test();
    assert_eq!(rows[0].selected_issue_tick, Some(RESULT_WRITEBACK_TICK));
    assert_eq!(
        (rows[1].selected_issue_tick, rows[1].requested_wake_tick),
        (None, None)
    );
}
#[test]
fn two_pending_typed_wake_seed_separates_second_fetch_predecessor() {
    let (mut runtime, hart, head) = ready_two_pending_issue(2, true);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let first_sequence = runtime.pending_data_address_sequences_for_test()[0];
    runtime.bind_oldest_pending_data_address_for_test(
        request(30),
        Address::new(RESULT_VALUE),
        RESULT_WRITEBACK_TICK,
    );
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
    let (mut runtime, hart, head) = ready_two_pending_issue(2, false);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let rows = runtime.pending_data_address_rows_for_test();
    assert_eq!(rows[0].requested_wake_tick, None);
    assert_eq!(rows[1].requested_wake_tick, Some(RESULT_WRITEBACK_TICK + 1));
    assert_eq!(
        runtime.pending_data_address_wake_tick(),
        Some(RESULT_WRITEBACK_TICK + 1)
    );
}
#[test]
fn two_pending_first_materialization_replay_discards_complete_chain() {
    let (mut runtime, hart, head) = ready_two_pending_issue(1, false);
    runtime.corrupt_pending_data_address_lsq_bytes_for_test(4);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    assert_eq!(runtime.pending_data_address_count(), 0);
    assert_eq!(pc_rows(&runtime), [HEAD_PC].map(Address::new));
    assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    assert!(runtime.pending_data_address_wake_tick().is_none());
}
#[test]
fn two_pending_second_materialization_replay_preserves_older_row() {
    let (mut runtime, hart, head) = ready_two_pending_issue(1, false);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let sequences = runtime.pending_data_address_sequences_for_test();
    runtime.corrupt_pending_data_address_lsq_bytes_for_fetch_for_test(request(12), 4);
    let next_tick = RESULT_WRITEBACK_TICK + 1;
    schedule(&mut runtime, &hart, head, next_tick);
    assert_eq!(
        runtime.pending_data_address_sequences_for_test(),
        [sequences[0]]
    );
    assert_eq!(
        pc_rows(&runtime),
        [HEAD_PC, FIRST_PENDING_PC].map(Address::new)
    );
    assert!(runtime.pending_data_address_owner_is_consistent());
    assert!(runtime.pending_data_address_wake_tick().is_none());
}
#[test]
fn two_pending_chain_wakes_second_after_first_admitted_writeback() {
    let (mut runtime, hart, head) = ready_two_pending_issue(2, true);
    schedule(&mut runtime, &hart, head, RESULT_WRITEBACK_TICK);
    let first_sequence = runtime.pending_data_address_sequences_for_test()[0];
    runtime.bind_oldest_pending_data_address_for_test(
        request(30),
        Address::new(RESULT_VALUE),
        RESULT_WRITEBACK_TICK,
    );
    let root_ready = runtime.live_data_accesses[0].execution.clone();
    runtime.record_retired_instruction_with_trace(&root_ready, true);
    let (completed, admitted) = runtime.complete_pending_data_address_for_test(
        request(11),
        request(30),
        RESULT_WRITEBACK_TICK + 1,
        &RESULT_VALUE.to_le_bytes(),
    );
    assert!(runtime
        .take_ready_live_data_access_event(admitted)
        .is_some());
    let first = O3LiveIssueHeadReservation::for_instruction(
        first_sequence,
        admitted,
        completed.instruction(),
    );
    schedule(&mut runtime, &hart, first, admitted);
    let row = runtime.pending_data_address_rows_for_test().pop().unwrap();
    assert_eq!(row.fetch.request_id(), request(12));
    assert_eq!(row.selected_issue_tick, Some(admitted));
    assert!(row.materialized.is_some());
}
#[test]
fn two_pending_interrupt_reset_restore_and_mode_cleanup_remove_all_rows() {
    let interrupt = core_with_completed_fetches([(30, HEAD_PC, addi(9, 0, 1))]);
    interrupt.state.lock().unwrap().o3_runtime = staged_two_pending_runtime();
    interrupt.set_detailed_live_retire_gate_enabled(true);
    let interrupt_bit = 1_u64 << 1;
    interrupt.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    interrupt.set_machine_interrupt_enable(interrupt_bit);
    interrupt.set_machine_interrupt_pending(interrupt_bit);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    interrupt
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .unwrap();
    assert_no_pending_rows(&interrupt.state.lock().unwrap().o3_runtime);
    let reset = core_with_completed_fetches(std::iter::empty());
    reset.state.lock().unwrap().o3_runtime = staged_two_pending_runtime();
    reset.reset_instruction_fetch_stream(37);
    assert_no_pending_rows(&reset.state.lock().unwrap().o3_runtime);
    let mut restored = staged_two_pending_runtime();
    restored
        .restore(O3RuntimeState::default().snapshot().clone())
        .unwrap();
    assert_no_pending_rows(&restored);
    let mode = core_with_completed_fetches(std::iter::empty());
    mode.state.lock().unwrap().o3_runtime = staged_two_pending_runtime();
    mode.set_detailed_live_retire_gate_enabled(true);
    mode.set_detailed_live_retire_gate_enabled(false);
    assert_resident_head_only(&mode.state.lock().unwrap().o3_runtime);
}
#[test]
fn two_pending_live_checkpoint_and_handoff_reject_two_rows() {
    let runtime = staged_two_pending_runtime();
    assert_eq!(runtime.pending_data_address_count(), 2);
    assert!(runtime.live_scalar_memory_handoff().is_none());
    let core = core_with_completed_fetches(std::iter::empty());
    core.state.lock().unwrap().o3_runtime = runtime;
    assert!(!core.data_access_lifecycle_is_quiescent());
    assert_eq!(
        core.capture_o3_live_data_handoff_status(),
        crate::RiscvO3LiveDataHandoffCapture::Rejected
    );
    core.finalize_quiescent_o3_writeback_for_checkpoint();
    assert!(core.has_pending_o3_runtime_retirement());
}
fn staged_two_pending_runtime() -> O3RuntimeState {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(stage_pending(&mut fixture, sibling_pending_requests(), scalar_suffix()) == 3);
    fixture.runtime
}
fn assert_no_pending_rows(runtime: &O3RuntimeState) {
    let snapshot = runtime.snapshot();
    assert!(
        snapshot.reorder_buffer().is_empty()
            && snapshot.load_store_queue().is_empty()
            && snapshot.rename_map().is_empty()
            && runtime.live_data_accesses.is_empty()
            && runtime.live_data_access_younger_sequences.is_empty()
            && runtime.pending_data_address_count() == 0
            && runtime.pending_data_address_wake_tick().is_none()
    );
}
fn assert_resident_head_only(runtime: &O3RuntimeState) {
    let snapshot = runtime.snapshot();
    assert!(
        snapshot.reorder_buffer().len() == 1
            && snapshot.load_store_queue().len() == 1
            && snapshot.rename_map().len() == 1
            && runtime.live_data_accesses.len() == 1
            && runtime.live_data_accesses[0].outcome == O3LiveDataAccessOutcome::Resident
            && runtime.live_data_access_younger_sequences.is_empty()
            && runtime.pending_data_address_count() == 0
            && runtime.pending_data_address_wake_tick().is_none()
            && !runtime.live_data_access_lifecycle_is_quiescent()
            && runtime.pending_live_data_access_retirement_count() == 1
    );
}
