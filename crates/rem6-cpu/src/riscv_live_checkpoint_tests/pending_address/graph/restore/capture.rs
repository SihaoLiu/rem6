use super::*;

#[test]
fn pending_load_graph_capture_projects_all_addressless_owners() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let projection = fixture.capture();
    let live = captured_graph(&projection);
    let rows = live.pending_addresses.as_slice();

    assert_eq!(
        live.profile,
        RiscvO3LiveCheckpointProfile::PendingDataAddress
    );
    assert_eq!(live.captured_tick, CAPTURED_TICK);
    assert_eq!(live.events, []);
    assert_eq!(live.resident_sequences, [1, 2, 3]);
    assert_eq!(live.issue_rows, expected_issue_rows());
    assert_eq!(live.rename_rows, []);
    assert_eq!(live.executed_fetch_requests, []);
    assert_eq!(live.issued_fetch_requests, []);
    assert!(live.completed_result.is_none());
    assert!(live.reservation.is_none());
    assert_eq!(live.service.requested_tick, CAPTURED_TICK);
    assert_eq!(live.wake.tick, CAPTURED_TICK);
    assert_eq!(
        live.wake.scheduler_instance_raw,
        fixture.scheduler.checkpoint_raw()
    );
    assert_payload_rows(rows, [5, 5, 7], [0, 0, 2], [true, true, false]);
    assert_stable_graph_owner_set(projection.stable(), rows);
    assert_eq!(projection.replay().hart.pc(), FIRST_LOAD_PC);

    let extra_fetch = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    extra_fetch
        .core
        .core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(completed_fetch(
            next_fetch_pc(),
            last_load_sequence() + 1,
            ld(9, ROOT_REGISTER),
        ));
    extra_fetch
        .core
        .inner()
        .advance_sequence_past(request(last_load_sequence() + 1));
    assert!(matches!(
        extra_fetch.capture().live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));

    let atomic_root = PendingLoadGraphCheckpointFixture::new_with_atomic_root([5, 5, 7], 2);
    assert!(matches!(
        atomic_root.capture().live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[test]
fn pending_load_graph_capture_restore_accepts_independent_request_sequence_gaps() {
    let fixture = PendingLoadGraphCheckpointFixture::new_with_request_gaps([5, 5, 7], 2);
    let projection = fixture.capture();
    let live = captured_graph(&projection);
    assert_eq!(live.resident_sequences, [1, 2, 3]);
    assert_eq!(
        live.pending_addresses
            .iter()
            .map(|row| row.fetch.request_id().sequence())
            .collect::<Vec<_>>(),
        [1, 3, 4]
    );
    let encoded = live.encode().unwrap();
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Ok(live.clone())
    );

    let destination = graph_core(fixture.issue_width);
    install_graph_projection(&destination, &fixture.core, &projection);
    assert_restored_snapshot_owns_graph(
        &destination.o3_runtime_snapshot(),
        live.pending_addresses.as_slice(),
    );
}

#[test]
fn pending_load_graph_capture_rejects_fetch_cursor_after_graph_rows() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    fixture
        .core
        .inner()
        .set_pc(Address::new(next_fetch_pc() + 4));

    assert!(matches!(
        fixture.capture().live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[test]
fn pending_load_graph_capture_rejects_residual_fetch_authorizations() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 6, 7], 4);
    install_graph_authorizations(&fixture.core, [5, 6, 7]);

    assert!(matches!(
        fixture.capture().live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[test]
fn pending_load_graph_v3_accepts_independent_root_and_row_request_gaps() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let projection = fixture.capture();
    let mut live = captured_graph(&projection).clone();
    introduce_independent_request_gaps(&mut live);

    let encoded = live.encode().unwrap();
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(live));
}

#[test]
fn pending_load_graph_restore_accepts_independent_root_and_row_request_gaps() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let projection = fixture.capture();
    let mut live = captured_graph(&projection).clone();
    introduce_independent_request_gaps(&mut live);
    let destination = graph_core(fixture.issue_width);

    let prepared = destination
        .prepare_checkpoint_restore(checkpoint_input_with_replay_hart(
            &projection,
            &fixture.core,
            projection.stable().clone(),
            live.clone(),
        ))
        .unwrap();
    destination.install_prepared_checkpoint_restore(prepared);
    assert_restored_snapshot_owns_graph(
        &destination.o3_runtime_snapshot(),
        live.pending_addresses.as_slice(),
    );
}

fn introduce_independent_request_gaps(live: &mut RiscvO3LiveCheckpointPayload) {
    let root_fetch_request = request(ROOT_FETCH_SEQUENCE + 1);
    let mut predecessor = root_fetch_request;
    for (index, row) in live.pending_addresses.iter_mut().enumerate() {
        let fetch_sequence = FIRST_LOAD_SEQUENCE + index as u64 + 2;
        let raw = u32::from_le_bytes(row.fetch.data().unwrap().try_into().unwrap());
        row.fetch = completed_fetch(row.fetch.pc().get(), fetch_sequence, raw);
        row.consumed_requests = vec![request(fetch_sequence)];
        row.fetch_predecessor_request = predecessor;
        row.root_fetch_request = root_fetch_request;
        live.issue_rows[index].fetch_request = request(fetch_sequence);
        predecessor = request(fetch_sequence);
    }
    live.next_fetch_request_sequence = last_load_sequence() + 3;
}
