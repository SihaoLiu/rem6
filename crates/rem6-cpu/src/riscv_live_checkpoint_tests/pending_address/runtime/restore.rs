use super::*;

#[test]
fn pending_store_capture_uses_pending_profile_after_producer_commit() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let live = captured_pending(&projection);
    let pending = live.pending_address.as_ref().expect("pending store row");

    assert_eq!(
        live.profile,
        RiscvO3LiveCheckpointProfile::PendingDataAddress
    );
    assert_eq!(live.captured_tick, CAPTURED_TICK);
    assert_eq!(live.resident_sequences, [LIVE_STORE_SEQUENCE]);
    assert_eq!(
        live.issue_rows,
        [RiscvO3LiveCheckpointIssueRow {
            sequence: LIVE_STORE_SEQUENCE,
            fetch_request: request(STORE_SEQUENCE),
        }]
    );
    assert_eq!(live.service.requested_tick, CAPTURED_TICK);
    assert_eq!(live.wake.tick, CAPTURED_TICK);
    assert_eq!(
        live.wake.scheduler_instance_raw,
        fixture.scheduler.checkpoint_raw()
    );
    assert_eq!(pending.sequence, LIVE_STORE_SEQUENCE);
    assert_eq!(pending.producer_sequence, LIVE_PRODUCER_SEQUENCE);
    assert_eq!(pending.root_sequence, LIVE_PRODUCER_SEQUENCE);
    assert_eq!(pending.root_range.start(), Address::new(ROOT_ADDRESS));
    assert_eq!(pending.root_range.size(), AccessSize::new(8).unwrap());
    assert_eq!(pending.published_producer_ready_tick, CAPTURED_TICK);
    assert_eq!(pending.requested_wake_tick, CAPTURED_TICK);
    assert_eq!(fixture.core.read_register(reg(5)), STORE_ADDRESS);
}

#[test]
fn pending_store_capture_keeps_generic_execution_events_empty() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let live = captured_pending(&projection);

    assert!(live.events.is_empty());
    assert!(live.executed_fetch_requests.is_empty());
    assert!(live.issued_fetch_requests.is_empty());
    assert!(live.rename_rows.is_empty());
    assert!(live.completed_result.is_none());
    assert!(live.reservation.is_none());
    assert!(live.writeback_counted_sequences.is_empty());
    assert!(live.writeback_published_sequences.is_empty());
    assert_eq!(fixture.core.execution_events().len(), 1);
}

#[test]
fn pending_store_capture_canonicalizes_advanced_hart_for_restore() {
    let fixture = PendingStoreCheckpointFixture::new();
    fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .hart
        .set_pc(STORE_PC + 4);

    let projection = fixture.capture();

    assert_eq!(
        fixture
            .core
            .state
            .lock()
            .expect("riscv core lock")
            .hart
            .pc(),
        STORE_PC + 4
    );
    assert_eq!(projection.replay().hart.pc(), STORE_PC);
    let destination = pending_store_core();
    let prepared = destination
        .prepare_checkpoint_restore(projection.replay().clone())
        .unwrap();
    destination.install_prepared_checkpoint_restore(prepared);
    assert_eq!(
        destination.state.lock().expect("riscv core lock").hart.pc(),
        STORE_PC
    );
}

#[test]
fn pending_store_prepare_rebuilds_exact_destinationless_owner_set() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let live = captured_pending(&projection).clone();
    let destination = pending_store_core();

    install_pending_projection(&destination, &fixture.core, &projection);

    let snapshot = destination.o3_runtime_snapshot();
    let [rob] = snapshot.reorder_buffer() else {
        panic!("expected one restored ROB owner: {snapshot:?}");
    };
    let [lsq] = snapshot.load_store_queue() else {
        panic!("expected one restored LSQ owner: {snapshot:?}");
    };
    assert_eq!(rob.sequence(), LIVE_STORE_SEQUENCE);
    assert_eq!(rob.pc(), Address::new(STORE_PC));
    assert!(rob.destination().is_none());
    assert!(rob.is_live_staged());
    assert!(!rob.is_ready());
    assert_eq!(lsq.sequence(), LIVE_STORE_SEQUENCE);
    assert_eq!(lsq.kind(), O3LoadStoreQueueKind::Store);
    assert_eq!(lsq.address(), None);
    assert_eq!(lsq.bytes(), 8);
    assert!(!lsq.is_completed());

    let restored_telemetry = destination.o3_runtime_live_issue_telemetry();
    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.pending_data_address_count(), 1);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        [LIVE_STORE_SEQUENCE]
    );
    assert_eq!(
        state.o3_runtime.live_issue_service_tick(),
        Some(CAPTURED_TICK)
    );
    assert_eq!(
        restored_telemetry,
        O3LiveIssueTelemetry::from_checkpoint_for_test([
            live.service.telemetry.enqueued_rows,
            live.service.telemetry.service_turns,
            live.service.telemetry.wake_requests,
            live.service.telemetry.current_occupancy,
            live.service.telemetry.peak_occupancy,
            live.service.telemetry.scalar_integer_issued_rows,
            live.service.telemetry.integer_mul_div_issued_rows,
            live.service.telemetry.memory_agu_issued_rows,
            live.service.telemetry.control_issued_rows,
            live.service.telemetry.scalar_float_issued_rows,
            live.service.telemetry.vector_to_scalar_issued_rows,
        ])
    );
    assert!(state
        .o3_runtime
        .live_issue_queue_materializes_for_checkpoint());
    assert_eq!(
        state.o3_runtime.pending_data_address_wake_tick(),
        Some(CAPTURED_TICK)
    );
    drop(state);
    assert_eq!(
        destination.pending_o3_live_data_access_retirement_count(),
        1
    );
}

#[test]
fn pending_store_prepare_restores_fetch_without_execution_or_data_issue() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let pending = captured_pending(&projection)
        .pending_address
        .as_ref()
        .expect("pending store row")
        .clone();
    let destination = pending_store_core();

    install_pending_projection(&destination, &fixture.core, &projection);

    assert_eq!(destination.inner().fetch_events(), [pending.fetch]);
    assert!(destination.execution_events().is_empty());
    let state = destination.state.lock().expect("riscv core lock");
    assert!(!state.executed_fetches.contains(&request(STORE_SEQUENCE)));
    assert!(!state
        .issued_data_for_fetches
        .contains(&request(STORE_SEQUENCE)));
    assert_eq!(state.hart.pc(), STORE_PC);
}

#[test]
fn pending_store_recapture_before_wake_is_identical() {
    let fixture = PendingStoreCheckpointFixture::new();
    let first = fixture.capture();
    let destination = pending_store_core();
    install_pending_projection(&destination, &fixture.core, &first);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    let second = destination.capture_checkpoint_projection(CAPTURED_TICK);

    assert_eq!(captured_pending(&second), captured_pending(&first));
    assert_eq!(second.stable(), first.stable());
}

#[test]
fn pending_store_rebound_wake_materializes_restored_request() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let destination = pending_store_core();
    install_pending_projection(&destination, &fixture.core, &projection);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    destination.mark_o3_writeback_wake_fired(CAPTURED_TICK);

    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.pending_data_address_count(), 1);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
    assert_eq!(
        state
            .o3_runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(CAPTURED_TICK)
    );
    assert!(state
        .o3_runtime
        .pending_data_address_materialized_execution_for_test()
        .is_some());
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        []
    );
}

#[test]
fn pending_store_prepare_rejects_stable_cross_reference_without_mutation() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let stable = projection.stable().clone();
    let live = captured_pending(&projection).clone();
    let invalid_stable = super::compute::rebuilt_stable(
        &stable,
        None,
        Some(vec![O3LoadStoreQueueEntry::store(
            LIVE_STORE_SEQUENCE + 1,
            None,
            8,
        )]),
        None,
    )
    .unwrap();
    let destination = pending_store_core();
    destination.write_register(reg(9), 0xfeed_face);
    destination.inner().set_pc(Address::new(0xb000));
    let destination_cpu = destination.core.checkpoint_state();
    let destination_riscv = destination.state.lock().expect("riscv core lock").clone();

    assert!(destination
        .prepare_checkpoint_restore(checkpoint_input_with_live(
            &fixture.core,
            invalid_stable,
            live.clone(),
        ))
        .is_err());

    assert_eq!(projection.stable(), &stable);
    assert_eq!(captured_pending(&projection), &live);
    assert_eq!(destination.core.checkpoint_state(), destination_cpu);
    assert_eq!(
        *destination.state.lock().expect("riscv core lock"),
        destination_riscv
    );
}
