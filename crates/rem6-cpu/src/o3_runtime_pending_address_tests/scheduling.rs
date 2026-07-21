use super::*;

use rem6_isa_riscv::RiscvHartState;

const HEAD_RESPONSE_TICK: u64 = 40;
const HEAD_WRITEBACK_TICK: u64 = 41;
const PRODUCER_VALUE: u64 = 0xa000;

struct PendingAddressSchedulingFixture {
    runtime: O3RuntimeState,
    hart: RiscvHartState,
    head: O3LiveIssueHeadReservation,
    head_execution: RiscvCpuExecutionEvent,
    requests: Vec<O3LiveIssueRequest>,
}

impl PendingAddressSchedulingFixture {
    fn new(issue_width: usize) -> Self {
        let mut staging = PendingAddressFixture::new(4, 4);
        assert!(staging.runtime.set_issue_width(issue_width));
        assert_eq!(staging.stage_default(), 3);
        for (pc, raw, sequence) in [
            (FIRST_SUFFIX_PC, addi(7, 5, 8), 12),
            (SECOND_SUFFIX_PC, add(8, 6, 7), 13),
        ] {
            assert!(staging.runtime.bind_live_staged_fetch_identity(
                Address::new(pc),
                decoded(raw).instruction(),
                &[request(sequence)],
            ));
        }
        let head_execution = load_event(HEAD_PC, 10, 5, 2, 0x9000);
        let head = staging
            .runtime
            .live_data_access_head_reservation(head_execution.fetch().request_id())
            .expect("pending-address head reservation");
        let requests = [
            (PENDING_PC, ld(6, 5, 0), 11),
            (FIRST_SUFFIX_PC, addi(7, 5, 8), 12),
            (SECOND_SUFFIX_PC, add(8, 6, 7), 13),
        ]
        .into_iter()
        .map(|(pc, raw, sequence)| {
            O3LiveIssueRequest::new(Address::new(pc), vec![request(sequence)], decoded(raw))
        })
        .collect();
        let mut hart = RiscvHartState::new(HEAD_PC);
        hart.write(reg(5), 0xdead_beef);
        Self {
            runtime: staging.runtime,
            hart,
            head,
            head_execution,
            requests,
        }
    }

    fn complete_head(&mut self, value: u64) {
        let mut completed = self.head_execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(self
            .runtime
            .complete_live_data_access_response(
                &completed,
                request(20),
                HEAD_RESPONSE_TICK,
                9,
                Some(&value.to_le_bytes()),
            )
            .unwrap());
        assert_eq!(
            self.runtime
                .earliest_unpublished_memory_result_writeback_tick(),
            Some(HEAD_WRITEBACK_TICK)
        );
    }

    fn publish_head(&mut self) {
        assert!(self
            .runtime
            .take_ready_live_data_access_event(HEAD_WRITEBACK_TICK)
            .is_some());
    }

    fn schedule(&mut self, tick: u64) -> Result<(), O3RuntimeError> {
        self.runtime
            .schedule_live_speculative_issues(&self.hart, self.head, tick, &self.requests)
    }

    fn scalar_issue_tick(&self) -> Option<u64> {
        self.runtime
            .live_speculative_executions
            .iter()
            .find(|execution| execution.execution.pc() == FIRST_SUFFIX_PC)
            .map(|execution| execution.issue_tick)
    }

    fn fan_in_is_recorded(&self) -> bool {
        self.runtime
            .live_speculative_executions
            .iter()
            .any(|execution| execution.execution.pc() == SECOND_SUFFIX_PC)
    }

    fn assert_replayed_to_head(&self) {
        assert!(!self.runtime.has_pending_data_address());
        assert_eq!(self.runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(self.runtime.snapshot().load_store_queue().len(), 1);
        assert!(self.runtime.live_data_access_younger_sequences.is_empty());
        assert!(self.runtime.live_speculative_executions.is_empty());
    }
}

#[test]
fn pending_address_scheduler_waits_for_head_writeback() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);

    fixture.schedule(HEAD_RESPONSE_TICK).unwrap();

    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        None
    );
    assert_eq!(fixture.scalar_issue_tick(), None);
    assert!(!fixture.fan_in_is_recorded());

    fixture.publish_head();
    fixture.schedule(HEAD_WRITEBACK_TICK).unwrap();
    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK)
    );
}

#[test]
fn pending_address_scheduler_width_one_orders_memory_before_scalar() {
    let mut fixture = PendingAddressSchedulingFixture::new(1);
    fixture.complete_head(PRODUCER_VALUE);
    fixture.publish_head();

    fixture.schedule(HEAD_WRITEBACK_TICK).unwrap();

    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK)
    );
    assert_eq!(fixture.scalar_issue_tick(), Some(HEAD_WRITEBACK_TICK + 1));
    assert!(!fixture.fan_in_is_recorded());

    let mut blocked = PendingAddressSchedulingFixture::new(1);
    blocked.complete_head(PRODUCER_VALUE);
    blocked.publish_head();
    let head_sequence = blocked.runtime.snapshot().reorder_buffer()[0].sequence();
    blocked.head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        HEAD_WRITEBACK_TICK,
        blocked.head_execution.instruction(),
    );

    blocked.schedule(HEAD_WRITEBACK_TICK).unwrap();

    assert_eq!(
        blocked
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        None
    );
    assert_eq!(
        blocked.runtime.pending_data_address_wake_tick(),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    blocked.hart.write(reg(5), PRODUCER_VALUE);
    blocked
        .runtime
        .remove_live_data_access_rows(head_sequence, 1);
    assert_eq!(
        blocked.runtime.pending_data_address_wakeup_seed(),
        Some((
            request(10),
            [PENDING_PC, FIRST_SUFFIX_PC, SECOND_SUFFIX_PC]
                .map(Address::new)
                .to_vec(),
        ))
    );
    blocked.head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        HEAD_WRITEBACK_TICK + 1,
        blocked.head_execution.instruction(),
    );

    blocked.schedule(HEAD_WRITEBACK_TICK + 1).unwrap();

    assert_eq!(blocked.scalar_issue_tick(), None);
    assert_eq!(
        blocked.runtime.pending_data_address_wake_tick(),
        Some(HEAD_WRITEBACK_TICK + 2)
    );
    blocked.head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        0,
        blocked.head_execution.instruction(),
    );
    blocked.schedule(HEAD_WRITEBACK_TICK + 2).unwrap();
    assert_eq!(
        blocked
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK + 2)
    );
    assert_eq!(blocked.runtime.pending_data_address_wake_tick(), None);
    assert!(matches!(
        blocked
            .runtime
            .pending_data_address_materialized_execution_for_test()
            .and_then(|event| event.execution().memory_access()),
        Some(MemoryAccessKind::Load {
            address: PRODUCER_VALUE,
            ..
        })
    ));

    let mut fired = PendingAddressSchedulingFixture::new(1);
    fired.complete_head(PRODUCER_VALUE);
    fired.publish_head();
    let head_sequence = fired.runtime.snapshot().reorder_buffer()[0].sequence();
    fired.head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        HEAD_WRITEBACK_TICK,
        fired.head_execution.instruction(),
    );
    fired.schedule(HEAD_WRITEBACK_TICK).unwrap();
    fired.hart.write(reg(5), PRODUCER_VALUE);
    fired.runtime.remove_live_data_access_rows(head_sequence, 1);
    let core = core_with_completed_fetches([
        (10, HEAD_PC, ld(5, 2, 0)),
        (11, PENDING_PC, ld(6, 5, 0)),
        (12, FIRST_SUFFIX_PC, addi(7, 5, 8)),
        (13, SECOND_SUFFIX_PC, add(8, 6, 7)),
    ]);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart = fired.hart;
        state.o3_runtime = fired.runtime;
    }
    assert_eq!(
        core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    register_o3_wake(&core, HEAD_WRITEBACK_TICK + 1);
    assert_eq!(
        core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK),
        None
    );

    core.mark_o3_writeback_wake_fired(HEAD_WRITEBACK_TICK + 1);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .o3_runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    assert_eq!(state.o3_runtime.pending_data_address_wake_tick(), None);
    drop(state);

    let mut stale = PendingAddressSchedulingFixture::new(1);
    stale.complete_head(PRODUCER_VALUE);
    stale.publish_head();
    let head_sequence = stale.runtime.snapshot().reorder_buffer()[0].sequence();
    stale.head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        HEAD_WRITEBACK_TICK,
        stale.head_execution.instruction(),
    );
    stale.schedule(HEAD_WRITEBACK_TICK).unwrap();
    stale.hart.write(reg(5), PRODUCER_VALUE);
    stale.runtime.remove_live_data_access_rows(head_sequence, 1);
    let stale_core = core_with_completed_fetches([(10, HEAD_PC, ld(5, 2, 0))]);
    {
        let mut state = stale_core.state.lock().expect("riscv core lock");
        state.hart = stale.hart;
        state.o3_runtime = stale.runtime;
    }
    assert_eq!(
        stale_core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    register_o3_wake(&stale_core, HEAD_WRITEBACK_TICK + 1);

    stale_core.mark_o3_writeback_wake_fired(HEAD_WRITEBACK_TICK + 1);

    let state = stale_core.state.lock().expect("riscv core lock");
    assert!(!state.o3_runtime.has_pending_data_address());
    drop(state);
    assert_eq!(
        stale_core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK + 1),
        None
    );
}

#[test]
fn pending_address_scheduler_width_two_coissues_memory_and_scalar() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    fixture.publish_head();

    fixture.schedule(HEAD_WRITEBACK_TICK).unwrap();

    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK)
    );
    assert_eq!(fixture.scalar_issue_tick(), Some(HEAD_WRITEBACK_TICK));
    assert!(!fixture.fan_in_is_recorded());
}

#[test]
fn pending_address_materialization_uses_admitted_producer_value() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    fixture.publish_head();

    fixture.schedule(HEAD_WRITEBACK_TICK).unwrap();

    let materialized = fixture
        .runtime
        .pending_data_address_materialized_execution_for_test()
        .expect("pending load materializes");
    assert_eq!(materialized.fetch().request_id(), request(11));
    assert!(matches!(
        materialized.execution().memory_access(),
        Some(MemoryAccessKind::Load {
            rd,
            address: PRODUCER_VALUE,
            width: MemoryWidth::Doubleword,
            ..
        }) if *rd == reg(6)
    ));
}

#[test]
fn pending_address_materialization_stale_identity_replays_and_discards_suffix() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    fixture.publish_head();
    let pending_sequence = fixture
        .runtime
        .pending_data_address_sequence_for_test()
        .expect("pending sequence");
    fixture
        .runtime
        .live_staged_fetch_identities
        .remove(&pending_sequence);

    fixture.schedule(HEAD_WRITEBACK_TICK).unwrap();

    fixture.assert_replayed_to_head();

    let (core, head, fetch_events) = pending_address_core_fixture(ld(6, 5, 0), ld(6, 5, 0));
    let mut state = core.state.lock().expect("riscv core lock");
    stage_o3_data_access_younger_window(&mut state, &head, 10, &fetch_events);
    let pending_sequence = state
        .o3_runtime
        .pending_data_address_sequence_for_test()
        .expect("pending sequence");
    state
        .o3_runtime
        .live_staged_fetch_identities
        .remove(&pending_sequence);

    wake_o3_data_access_younger_window(&mut state, HEAD_WRITEBACK_TICK, &fetch_events);

    assert!(!state.o3_runtime.has_pending_data_address());
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
    assert!(state
        .o3_runtime
        .live_data_access_younger_sequences
        .is_empty());

    let mut lineage = PendingAddressSchedulingFixture::new(2);
    lineage.complete_head(PRODUCER_VALUE);
    lineage.publish_head();
    lineage
        .runtime
        .corrupt_pending_data_address_producer_sequence_for_test(u64::MAX);

    lineage.schedule(HEAD_WRITEBACK_TICK).unwrap();

    lineage.assert_replayed_to_head();
}

#[test]
fn pending_address_materialization_failure_replays_without_callback_error() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    fixture.publish_head();
    fixture
        .runtime
        .corrupt_pending_data_address_lsq_bytes_for_test(4);

    fixture
        .schedule(HEAD_WRITEBACK_TICK)
        .expect("pending materialization failure is replay, not callback error");

    fixture.assert_replayed_to_head();

    let mut short = PendingAddressSchedulingFixture::new(2);
    short.complete_head(PRODUCER_VALUE);
    short.publish_head();
    let request = &short.requests[0];
    let scheduling = short
        .runtime
        .live_issue_scheduling_candidate(
            0,
            request.pc(),
            request.instruction(),
            request.consumed_requests(),
        )
        .expect("pending scheduling candidate");
    let candidate = short
        .runtime
        .materialize_live_speculative_issue_candidate(&scheduling)
        .expect("pending materialization candidate");
    let execution = RiscvExecutionRecord::new_with_instruction_bytes(
        request.instruction(),
        2,
        request.pc().get(),
        request.pc().get() + 2,
        Vec::new(),
        Some(MemoryAccessKind::Load {
            rd: reg(6),
            address: PRODUCER_VALUE,
            width: MemoryWidth::Doubleword,
            signed: true,
        }),
    );
    assert!(!short
        .runtime
        .record_pending_data_address_materialization(
            candidate,
            request.consumed_requests(),
            HEAD_WRITEBACK_TICK,
            execution,
        )
        .unwrap());
}

#[test]
fn pending_address_materialization_does_not_allocate_a_request() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    fixture.publish_head();
    let pending_sequence = fixture
        .runtime
        .pending_data_address_sequence_for_test()
        .expect("pending sequence");
    let before_snapshot = fixture.runtime.snapshot();
    let before_next_sequence = fixture.runtime.next_sequence;
    let before_next_physical = fixture.runtime.next_physical_register;

    fixture.schedule(HEAD_WRITEBACK_TICK).unwrap();

    assert_eq!(fixture.runtime.snapshot(), before_snapshot);
    assert_eq!(fixture.runtime.next_sequence, before_next_sequence);
    assert_eq!(fixture.runtime.next_physical_register, before_next_physical);
    assert_eq!(fixture.runtime.live_data_accesses.len(), 1);
    assert!(fixture.runtime.pending_data_accesses.is_empty());
    assert!(fixture
        .runtime
        .writeback_reservation(pending_sequence)
        .is_none());
    assert!(fixture.runtime.has_pending_data_address());
}

fn register_o3_wake(core: &RiscvCore, tick: u64) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let event = scheduler
        .schedule_at(PartitionId::new(0), tick, |_| {})
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );
}
