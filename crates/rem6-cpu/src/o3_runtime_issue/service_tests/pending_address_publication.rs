use super::*;
use crate::o3_runtime::o3_runtime_pending_address_tests::scheduling::{
    PendingAddressSchedulingFixture, HEAD_WRITEBACK_TICK, PRODUCER_VALUE,
};

#[test]
fn pending_address_publication_rearms_same_tick_without_replay() {
    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    fixture
        .runtime
        .live_issue
        .request_service_at(HEAD_WRITEBACK_TICK);

    let retained = fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, HEAD_WRITEBACK_TICK)
        .unwrap();
    assert_eq!(retained.replay_boundary(), None);
    assert_eq!(retained.next_service_tick(), Some(HEAD_WRITEBACK_TICK + 1));
    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK)
    );

    let published = fixture
        .runtime
        .take_ready_live_data_access_event(HEAD_WRITEBACK_TICK)
        .expect("head publication");
    fixture.hart.write(reg(5), PRODUCER_VALUE);
    fixture
        .runtime
        .record_pending_data_address_producer_publication(
            &published,
            HEAD_WRITEBACK_TICK,
            HEAD_WRITEBACK_TICK,
        );
    fixture
        .runtime
        .live_issue
        .request_live_issue_after_writeback_change(HEAD_WRITEBACK_TICK);
    assert_eq!(
        fixture.runtime.live_issue_service_tick(),
        Some(HEAD_WRITEBACK_TICK)
    );
    fixture
        .runtime
        .record_retired_instruction_with_trace(&published, true);

    let issued = fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, HEAD_WRITEBACK_TICK)
        .unwrap();
    assert_eq!(issued.replay_boundary(), None);
    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(HEAD_WRITEBACK_TICK)
    );
}

#[test]
fn pending_address_late_publication_keeps_ready_tick_and_clamps_wake_to_now() {
    const LATE_TICK: u64 = HEAD_WRITEBACK_TICK + 9;

    let mut fixture = PendingAddressSchedulingFixture::new(2);
    fixture.complete_head(PRODUCER_VALUE);
    let published = fixture
        .runtime
        .take_ready_live_data_access_event(LATE_TICK)
        .expect("late head publication");
    fixture.hart.write(reg(5), PRODUCER_VALUE);
    fixture
        .runtime
        .record_pending_data_address_producer_publication(
            &published,
            HEAD_WRITEBACK_TICK,
            LATE_TICK,
        );
    fixture
        .runtime
        .live_issue
        .request_live_issue_after_writeback_change(LATE_TICK);
    fixture
        .runtime
        .record_retired_instruction_with_trace(&published, true);

    assert_eq!(
        fixture
            .runtime
            .pending_data_address_committed_producer_ready_tick(fixture.head.sequence(), reg(5),),
        Some(HEAD_WRITEBACK_TICK),
    );
    assert_eq!(
        fixture.runtime.pending_data_address_wake_tick(),
        Some(LATE_TICK)
    );
    assert!(fixture.runtime.pending_data_address_owner_is_consistent());

    fixture.schedule(LATE_TICK).unwrap();
    assert_eq!(
        fixture
            .runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(LATE_TICK),
    );
}
