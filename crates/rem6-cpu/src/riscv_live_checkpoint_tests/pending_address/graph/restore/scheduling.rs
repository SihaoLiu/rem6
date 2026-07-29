use super::*;

#[test]
fn pending_load_graph_restore_reenters_publication_tick_after_partial_issue() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 5], 2);
    let projection = fixture.capture();
    let live = captured_graph(&projection).clone();
    let destination = graph_core(fixture.issue_width);
    install_graph_projection(&destination, &fixture.core, &projection);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    destination.mark_o3_writeback_wake_fired(CAPTURED_TICK);

    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .o3_runtime
            .pending_data_address_selected_issue_ticks_for_test(),
        [Some(CAPTURED_TICK), Some(CAPTURED_TICK), None]
    );
    assert_eq!(
        state.o3_runtime.live_issue_service_tick(),
        Some(CAPTURED_TICK)
    );
    drop(state);

    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);
    destination.mark_o3_writeback_wake_fired(CAPTURED_TICK);

    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(
        state.o3_runtime.live_issue_service_tick(),
        Some(CAPTURED_TICK + 1)
    );
    assert_eq!(
        state.o3_runtime.live_issue_telemetry().wake_requests(),
        live.service.telemetry.wake_requests + 3
    );
    assert_eq!(
        state
            .o3_runtime
            .live_issue_trace_records()
            .iter()
            .filter(|record| {
                record.action() == O3LiveIssueTraceAction::RetainedResource
                    && record.service_tick() == CAPTURED_TICK
            })
            .count(),
        2
    );
}

#[test]
fn pending_load_graph_restore_reenters_publication_tick_for_chain_dependency() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 6, 7], 4);
    let projection = fixture.capture();
    let destination = graph_core(fixture.issue_width);
    install_graph_projection(&destination, &fixture.core, &projection);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    destination.mark_o3_writeback_wake_fired(CAPTURED_TICK);

    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .o3_runtime
            .pending_data_address_selected_issue_ticks_for_test(),
        [Some(CAPTURED_TICK), None, None]
    );
    assert_eq!(
        state.o3_runtime.live_issue_service_tick(),
        Some(CAPTURED_TICK)
    );
}
