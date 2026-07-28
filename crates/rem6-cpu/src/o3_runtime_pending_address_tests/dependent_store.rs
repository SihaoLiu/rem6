use super::*;

fn dependent_store(sequence: u64, predecessor: u64) -> O3PendingDataAddressRequest {
    pending_request(
        request(predecessor),
        sequence,
        FIRST_PENDING_PC + 4 * (sequence - 11),
        0x0062_b023,
        reg(5),
    )
}

#[test]
fn pending_terminal_store_rejects_a_younger_pending_row_atomically() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    let before = fixture.runtime.clone();
    let younger_load = pending_request(request(11), 12, SECOND_PENDING_PC, ld(7, 5, 8), reg(5));
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            [dependent_store(11, 10), younger_load],
            [],
            0,
        ),
        0
    );
    assert_eq!(fixture.runtime, before);
}

#[test]
fn pending_store_alone_keeps_checkpoint_and_handoff_nonquiescent() {
    let mut fixture = PendingAddressFixture::new(4, 4);
    assert_eq!(
        fixture.runtime.stage_pending_data_address_window(
            fixture.head_fetch,
            [dependent_store(11, 10)],
            [],
            0,
        ),
        1
    );
    let head_sequence = fixture.runtime.snapshot().reorder_buffer()[0].sequence();
    fixture
        .runtime
        .remove_live_data_access_rows(head_sequence, 1);
    assert!(fixture.runtime.has_pending_data_address());
    assert!(!fixture.runtime.live_data_access_lifecycle_is_quiescent());
    assert!(fixture.runtime.live_scalar_memory_handoff().is_none());
}

#[test]
fn pending_store_rejects_an_unresolved_value_source_atomically() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(4, 4));
    assert!(runtime
        .stage_live_instruction(
            Address::new(HEAD_PC - 4),
            decoded(addi(6, 0, 9)).instruction(),
            0,
        )
        .is_some());
    let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &head,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    let before = runtime.clone();

    assert_eq!(
        runtime.stage_pending_data_address_window(
            head.fetch().request_id(),
            [dependent_store(11, 10)],
            [],
            0,
        ),
        0
    );
    assert_eq!(runtime, before);
}
