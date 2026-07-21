use super::*;

#[test]
fn pending_address_stages_addressless_lsq_and_live_rename_once() {
    let mut fixture = PendingAddressFixture::new(4, 4);

    assert_eq!(fixture.stage_default(), 3);

    let snapshot = fixture.runtime.snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 4);
    assert_eq!(snapshot.load_store_queue().len(), 2);
    let head_sequence = snapshot.reorder_buffer()[0].sequence();
    let pending_sequence = fixture
        .runtime
        .pending_data_address_sequence_for_test()
        .expect("pending owner is recorded");
    let pending_lsq = snapshot
        .load_store_queue()
        .iter()
        .find(|entry| entry.sequence() == pending_sequence)
        .copied()
        .expect("pending address LSQ row exists");
    assert_eq!(pending_lsq.address(), None);
    assert_eq!(pending_lsq.bytes(), 8);
    assert_eq!(pending_lsq.kind(), O3LoadStoreQueueKind::Load);
    let mut resolved_pending_lsq = pending_lsq;
    assert!(resolved_pending_lsq.resolve_address(Address::new(0x9000)));
    assert!(resolved_pending_lsq.resolve_address(Address::new(0x9000)));
    assert!(!resolved_pending_lsq.resolve_address(Address::new(0x9008)));
    assert_eq!(resolved_pending_lsq.address(), Some(Address::new(0x9000)));
    assert_eq!(
        integer_mapping(&fixture.runtime, 5),
        snapshot.reorder_buffer()[0].destination()
    );
    assert_eq!(
        integer_mapping(&fixture.runtime, 6),
        snapshot.reorder_buffer()[1].destination()
    );
    assert_ne!(
        integer_mapping(&fixture.runtime, 5),
        integer_mapping(&fixture.runtime, 6)
    );
    assert_eq!(
        fixture.runtime.pending_data_address_owner_count_for_test(),
        1
    );
    assert_eq!(fixture.runtime.live_data_accesses.len(), 1);

    fixture
        .runtime
        .remove_live_data_access_rows(head_sequence, 1);
    assert!(fixture.runtime.has_pending_data_address());
    assert_eq!(
        pc_rows(&fixture.runtime),
        [PENDING_PC, FIRST_SUFFIX_PC, SECOND_SUFFIX_PC]
            .map(Address::new)
            .to_vec()
    );
    assert_eq!(fixture.runtime.snapshot().load_store_queue().len(), 1);
    assert!(fixture
        .runtime
        .snapshot()
        .load_store_queue()
        .iter()
        .any(|entry| entry.sequence() == pending_sequence && entry.address().is_none()));
}

#[test]
fn pending_address_window_stages_two_scalar_suffix_rows() {
    let (core, head, fetch_events) = pending_address_core_fixture(ld(6, 5, 0), ld(6, 5, 0));
    let mut state = core.state.lock().expect("riscv core lock");

    stage_o3_data_access_younger_window(&mut state, &head, 10, &fetch_events);

    assert_eq!(
        pc_rows(&state.o3_runtime),
        [HEAD_PC, PENDING_PC, FIRST_SUFFIX_PC, SECOND_SUFFIX_PC]
            .map(Address::new)
            .to_vec()
    );
    assert!(state.o3_runtime.has_pending_data_address());
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 2);
    assert_eq!(state.o3_runtime.live_data_access_younger_sequences.len(), 3);
    assert!(integer_mapping(&state.o3_runtime, 7).is_some());
    assert!(integer_mapping(&state.o3_runtime, 8).is_some());
    assert!(state.o3_runtime.stats().dependency_blocked_row_cycles() > 0);
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(11)));
}

#[test]
fn pending_address_rejects_a_second_owner() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert_eq!(fixture.stage_default(), 3);
    let before = fixture.runtime.snapshot();
    let previous_max_physical = [6, 7, 8]
        .into_iter()
        .filter_map(|register| integer_mapping(&fixture.runtime, register))
        .map(O3PhysicalRegisterId::get)
        .max()
        .expect("pending window allocated integer destinations");

    assert_eq!(fixture.stage_default(), 0);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            vec![pending_request(
                request(13),
                14,
                EXTRA_SUFFIX_PC,
                ld(9, 5, 0),
                reg(5),
            )],
            [],
        ),
        0
    );

    assert_eq!(fixture.runtime.snapshot(), before);
    assert_eq!(
        fixture.runtime.pending_data_address_owner_count_for_test(),
        1
    );
    assert!(fixture
        .runtime
        .pending_data_address_owns_fetch(fixture.pending.fetch().request_id()));
    assert_eq!(fixture.runtime.live_data_accesses.len(), 1);

    fixture.runtime.discard_pending_data_address();
    assert_eq!(fixture.stage_default(), 3);
    assert_eq!(
        integer_mapping(&fixture.runtime, 6).map(O3PhysicalRegisterId::get),
        Some(previous_max_physical + 1),
        "rejected duplicate staging must not advance the physical allocator"
    );

    let (core, head, fetch_events) = pending_address_core_fixture(ld(9, 5, 0), ld(6, 5, 0));
    let mut state = core.state.lock().expect("riscv core lock");
    stage_o3_data_access_younger_window(&mut state, &head, 10, &fetch_events);
    assert!(!state.o3_runtime.has_pending_data_address());
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(11)));

    drop(state);
    let (core, head, fetch_events) =
        pending_address_core_fixture_with_live_depth(ld(6, 5, 0), ld(6, 5, 0), 8);
    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .stage_live_instruction(
            Address::new(FIRST_SUFFIX_PC),
            decoded(addi(9, 5, 1)).instruction(),
            0,
        )
        .is_some());

    stage_o3_data_access_younger_window(&mut state, &head, 10, &fetch_events);

    assert!(!state.o3_runtime.has_pending_data_address());
    assert_eq!(
        pc_rows(&state.o3_runtime),
        [HEAD_PC, FIRST_SUFFIX_PC].map(Address::new).to_vec()
    );
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(11)));
}

#[test]
fn pending_address_window_stays_four_rows_at_scalar_live_depth_eight() {
    let mut fixture = PendingAddressFixture::new(4, 8);
    fixture
        .suffix
        .push(staged_instruction(EXTRA_SUFFIX_PC, addi(9, 8, 1)));

    assert_eq!(fixture.stage_default(), 3);

    assert_eq!(fixture.runtime.scalar_live_window_limit(), 8);
    assert_eq!(fixture.runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(fixture.runtime.snapshot().load_store_queue().len(), 2);
    assert_eq!(
        pc_rows(&fixture.runtime),
        [HEAD_PC, PENDING_PC, FIRST_SUFFIX_PC, SECOND_SUFFIX_PC]
            .map(Address::new)
            .to_vec()
    );
}

#[test]
fn pending_address_discard_restores_prior_rename_and_removes_lsq() {
    let prior_x6 =
        O3RenameMapEntry::new(O3RegisterClass::Integer, 6, O3PhysicalRegisterId::new(42));
    let mut fixture = PendingAddressFixture::new(4, 4);
    let seeded = O3RuntimeSnapshot::new(
        [],
        [],
        [prior_x6],
        default_o3_runtime_snapshot().pending_state().clone(),
    )
    .unwrap();
    fixture.runtime.restore(seeded).unwrap();
    let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    assert!(fixture.runtime.stage_live_data_access_issue(
        &head,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    assert_eq!(
        integer_mapping(&fixture.runtime, 6),
        Some(prior_x6.physical())
    );
    assert_eq!(fixture.stage_default(), 3);
    assert_ne!(
        integer_mapping(&fixture.runtime, 6),
        Some(prior_x6.physical())
    );
    let pending_sequence = fixture
        .runtime
        .pending_data_address_sequence_for_test()
        .expect("pending owner is recorded");

    fixture
        .runtime
        .discard_pending_data_address_from(pending_sequence);

    assert!(!fixture.runtime.has_pending_data_address());
    assert_eq!(fixture.runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(fixture.runtime.snapshot().load_store_queue().len(), 1);
    assert_eq!(
        integer_mapping(&fixture.runtime, 5),
        fixture.runtime.snapshot().reorder_buffer()[0].destination()
    );
    assert_eq!(
        integer_mapping(&fixture.runtime, 6),
        Some(prior_x6.physical())
    );
    assert_eq!(integer_mapping(&fixture.runtime, 7), None);
    assert_eq!(integer_mapping(&fixture.runtime, 8), None);
    assert!(fixture
        .runtime
        .live_data_access_younger_sequences
        .is_empty());
}
