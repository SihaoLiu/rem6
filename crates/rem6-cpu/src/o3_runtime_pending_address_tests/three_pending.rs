use super::*;

use rem6_isa_riscv::RiscvHartState;

const HEAD_RESPONSE_TICK: u64 = 40;
const HEAD_WRITEBACK_TICK: u64 = 41;
const RESULT_VALUE: u64 = 0xa000;

fn pending_three(sources: [u8; 3]) -> Vec<O3PendingDataAddressRequest> {
    [11, 12, 13]
        .into_iter()
        .zip([FIRST_PENDING_PC, SECOND_PENDING_PC, THIRD_PENDING_PC])
        .zip([6, 7, 8])
        .zip(sources)
        .map(|(((sequence, pc), rd), source)| {
            pending_request(
                request(sequence - 1),
                sequence,
                pc,
                ld(rd, source, 0),
                reg(source),
            )
        })
        .collect()
}

fn stage_three(fixture: &mut PendingAddressFixture, sources: [u8; 3]) -> usize {
    fixture.runtime.stage_pending_data_address_window(
        fixture.head_fetch,
        pending_three(sources),
        [],
    )
}

fn staged_three_pending(sources: [u8; 3], issue_width: usize) -> PendingAddressFixture {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert!(fixture.runtime.set_issue_width(issue_width));
    assert!(fixture.runtime.set_memory_issue_width(issue_width));
    assert_eq!(stage_three(&mut fixture, sources), 3);
    assert_three_addressless_rows(&fixture.runtime);
    fixture
}

fn assert_three_addressless_rows(runtime: &O3RuntimeState) {
    let snapshot = runtime.snapshot();
    let sequences = runtime.pending_data_address_sequences_for_test();
    assert_eq!(
        (
            snapshot.reorder_buffer().len(),
            snapshot.load_store_queue().len(),
            sequences.len(),
        ),
        (4, 4, 3)
    );
    assert!(sequences.windows(2).all(|rows| rows[0] < rows[1]));
    let pending_lsq = snapshot
        .load_store_queue()
        .iter()
        .filter(|entry| sequences.contains(&entry.sequence()))
        .collect::<Vec<_>>();
    assert_eq!(pending_lsq.len(), 3);
    assert!(pending_lsq
        .iter()
        .all(|entry| { entry.kind() == O3LoadStoreQueueKind::Load && entry.address().is_none() }));
}

fn complete_head(
    fixture: &mut PendingAddressFixture,
) -> (RiscvHartState, O3LiveIssueHeadReservation) {
    let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    let reservation = fixture
        .runtime
        .live_data_access_head_reservation(fixture.head_fetch)
        .expect("head reservation");
    let mut completed = head.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(fixture
        .runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            HEAD_RESPONSE_TICK,
            9,
            Some(&RESULT_VALUE.to_le_bytes()),
        )
        .unwrap());
    assert!(fixture
        .runtime
        .take_ready_live_data_access_event(HEAD_WRITEBACK_TICK)
        .is_some());
    let mut hart = RiscvHartState::new(HEAD_PC);
    hart.write(reg(5), RESULT_VALUE);
    (hart, reservation)
}

fn complete_oldest_pending(
    fixture: &mut PendingAddressFixture,
    hart: &mut RiscvHartState,
    data_request: MemoryRequestId,
    issue_tick: u64,
    response_tick: u64,
    destination: u8,
) -> (RiscvCpuExecutionEvent, u64, O3LiveIssueHeadReservation) {
    let row = fixture.runtime.pending_data_address_rows_for_test()[0].clone();
    fixture.runtime.bind_oldest_pending_data_address_for_test(
        data_request,
        Address::new(RESULT_VALUE),
        issue_tick,
    );
    let (completed, admitted) = fixture.runtime.complete_pending_data_address_for_test(
        row.fetch.request_id(),
        data_request,
        response_tick,
        &RESULT_VALUE.to_le_bytes(),
    );
    assert!(fixture
        .runtime
        .take_ready_live_data_access_event(admitted)
        .is_some());
    hart.write(reg(destination), RESULT_VALUE);
    (
        completed.clone(),
        admitted,
        O3LiveIssueHeadReservation::for_instruction(
            row.sequence,
            admitted,
            completed.instruction(),
        ),
    )
}

fn schedule(
    fixture: &mut PendingAddressFixture,
    hart: &RiscvHartState,
    head: O3LiveIssueHeadReservation,
    tick: u64,
) {
    fixture
        .runtime
        .schedule_live_speculative_issues(hart, head, tick)
        .unwrap();
}

fn pending_ticks(runtime: &O3RuntimeState) -> Vec<Option<u64>> {
    runtime
        .pending_data_address_rows_for_test()
        .iter()
        .map(|row| row.selected_issue_tick)
        .collect()
}

fn assert_blocked_suffix_unselected(runtime: &O3RuntimeState, selected_prefix: usize) {
    assert!(pending_ticks(runtime)
        .into_iter()
        .skip(selected_prefix)
        .all(|tick| tick.is_none()));
}

fn assert_oldest_pending_selected_at_or_after(runtime: &O3RuntimeState, tick: u64) {
    let selected = runtime.pending_data_address_rows_for_test()[0]
        .selected_issue_tick
        .expect("oldest pending row selected");
    assert!(selected >= tick, "{selected} < {tick}");
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

#[test]
fn three_pending_staging_allocates_three_addressless_lsq_rows() {
    let mut fixture = PendingAddressFixture::new(4, 4);

    assert_eq!(stage_three(&mut fixture, [5, 5, 5]), 3);

    assert_three_addressless_rows(&fixture.runtime);
}

#[test]
fn three_pending_sibling_width_one_issues_in_sequence() {
    let mut fixture = staged_three_pending([5, 5, 5], 1);
    let (hart, head) = complete_head(&mut fixture);

    for tick in [
        HEAD_WRITEBACK_TICK,
        HEAD_WRITEBACK_TICK + 1,
        HEAD_WRITEBACK_TICK + 2,
    ] {
        schedule(&mut fixture, &hart, head, tick);
    }

    assert_eq!(
        pending_ticks(&fixture.runtime),
        [
            Some(HEAD_WRITEBACK_TICK),
            Some(HEAD_WRITEBACK_TICK + 1),
            Some(HEAD_WRITEBACK_TICK + 2),
        ]
    );
}

#[test]
fn three_pending_sibling_width_two_issues_two_then_one() {
    let mut fixture = staged_three_pending([5, 5, 5], 2);
    let (hart, head) = complete_head(&mut fixture);

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK);
    assert_eq!(
        pending_ticks(&fixture.runtime),
        [Some(HEAD_WRITEBACK_TICK), Some(HEAD_WRITEBACK_TICK), None,]
    );

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK + 1);
    assert_eq!(
        pending_ticks(&fixture.runtime),
        [
            Some(HEAD_WRITEBACK_TICK),
            Some(HEAD_WRITEBACK_TICK),
            Some(HEAD_WRITEBACK_TICK + 1),
        ]
    );
}

#[test]
fn three_pending_sibling_width_four_issues_all_three_together() {
    let mut fixture = staged_three_pending([5, 5, 5], 4);
    let (hart, head) = complete_head(&mut fixture);

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK);

    assert_eq!(
        pending_ticks(&fixture.runtime),
        [Some(HEAD_WRITEBACK_TICK); 3]
    );
}

#[test]
fn three_pending_chain_waits_for_each_admitted_writeback() {
    let mut fixture = staged_three_pending([5, 6, 7], 4);
    let (mut hart, head) = complete_head(&mut fixture);

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK);
    assert_eq!(
        pending_ticks(&fixture.runtime),
        [Some(HEAD_WRITEBACK_TICK), None, None]
    );
    assert_blocked_suffix_unselected(&fixture.runtime, 1);

    let root_ready = fixture.runtime.live_data_accesses[0].execution.clone();
    fixture
        .runtime
        .record_retired_instruction_with_trace(&root_ready, true);
    let (first_completed, first_admitted, first) = complete_oldest_pending(
        &mut fixture,
        &mut hart,
        request(30),
        HEAD_WRITEBACK_TICK,
        HEAD_WRITEBACK_TICK + 1,
        6,
    );

    schedule(&mut fixture, &hart, first, first_admitted);
    assert_oldest_pending_selected_at_or_after(&fixture.runtime, first_admitted);
    assert_eq!(pending_ticks(&fixture.runtime).len(), 2);
    assert_blocked_suffix_unselected(&fixture.runtime, 1);

    fixture
        .runtime
        .record_retired_instruction_with_trace(&first_completed, true);
    let (_second_completed, second_admitted, second) = complete_oldest_pending(
        &mut fixture,
        &mut hart,
        request(31),
        first_admitted,
        first_admitted + 1,
        7,
    );

    schedule(&mut fixture, &hart, second, second_admitted);
    assert_oldest_pending_selected_at_or_after(&fixture.runtime, second_admitted);
    assert_eq!(pending_ticks(&fixture.runtime).len(), 1);
}

#[test]
fn three_pending_mixed_fanout_coissues_two_and_blocks_third() {
    let mut fixture = staged_three_pending([5, 5, 7], 4);
    let (hart, head) = complete_head(&mut fixture);

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK);

    assert_eq!(
        pending_ticks(&fixture.runtime),
        [Some(HEAD_WRITEBACK_TICK), Some(HEAD_WRITEBACK_TICK), None]
    );
}

#[test]
fn three_pending_resource_wake_updates_only_the_blocked_suffix() {
    let mut fixture = staged_three_pending([5, 5, 5], 2);
    let (hart, head) = complete_head(&mut fixture);

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK);

    let rows = fixture.runtime.pending_data_address_rows_for_test();
    assert_eq!(rows[0].requested_wake_tick, None);
    assert_eq!(rows[1].requested_wake_tick, None);
    assert_eq!(rows[2].requested_wake_tick, Some(HEAD_WRITEBACK_TICK + 1));
}

#[test]
fn three_pending_replay_from_middle_preserves_older_and_discards_younger() {
    let mut fixture = staged_three_pending([5, 5, 5], 1);
    let (hart, head) = complete_head(&mut fixture);
    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK);
    let sequences = fixture.runtime.pending_data_address_sequences_for_test();
    fixture
        .runtime
        .corrupt_pending_data_address_lsq_bytes_for_fetch_for_test(request(12), 4);

    schedule(&mut fixture, &hart, head, HEAD_WRITEBACK_TICK + 1);

    assert_eq!(
        fixture.runtime.pending_data_address_sequences_for_test(),
        [sequences[0]]
    );
    assert_eq!(
        pc_rows(&fixture.runtime),
        [HEAD_PC, FIRST_PENDING_PC].map(Address::new).to_vec()
    );
}

#[test]
fn three_pending_interrupt_reset_htm_and_mode_cleanup_remove_all_rows() {
    let staged = || staged_three_pending([5, 5, 5], 2).runtime;
    let interrupt = core_with_completed_fetches([(30, HEAD_PC, addi(9, 0, 1))]);
    interrupt.state.lock().unwrap().o3_runtime = staged();
    interrupt.set_detailed_live_retire_gate_enabled(true);
    let interrupt_bit = 1_u64 << 1;
    interrupt.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    interrupt.set_machine_interrupt_enable(interrupt_bit);
    interrupt.set_machine_interrupt_pending(interrupt_bit);
    interrupt
        .execute_next_completed_fetch_serial(&mut PartitionedScheduler::new(1).unwrap())
        .unwrap()
        .unwrap();
    assert_no_pending_rows(&interrupt.state.lock().unwrap().o3_runtime);

    let reset = core_with_completed_fetches(std::iter::empty());
    reset.state.lock().unwrap().o3_runtime = staged();
    reset.reset_instruction_fetch_stream(37);
    assert_no_pending_rows(&reset.state.lock().unwrap().o3_runtime);

    let htm = core_with_completed_fetches(std::iter::empty());
    htm.state.lock().unwrap().o3_runtime = staged();
    let begin = htm.begin_htm_transaction().unwrap();
    htm.abort_htm_transaction(begin.uid(), crate::HtmFailureCause::Explicit)
        .unwrap();
    assert_no_pending_rows(&htm.state.lock().unwrap().o3_runtime);

    let mode = core_with_completed_fetches(std::iter::empty());
    mode.state.lock().unwrap().o3_runtime = staged();
    mode.set_detailed_live_retire_gate_enabled(true);
    mode.set_detailed_live_retire_gate_enabled(false);
    assert_resident_head_only(&mode.state.lock().unwrap().o3_runtime);
}

#[test]
fn three_pending_live_checkpoint_and_addressless_handoff_reject() {
    let runtime = staged_three_pending([5, 5, 5], 2).runtime;
    assert_eq!(runtime.pending_data_address_count(), 3);
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

    let state = core.state.lock().unwrap();
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 4);
    assert_eq!(state.o3_runtime.pending_data_address_count(), 3);
}
