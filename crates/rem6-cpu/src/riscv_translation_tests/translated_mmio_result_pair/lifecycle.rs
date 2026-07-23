use super::*;

#[test]
fn younger_translation_fault_preserves_older_request_and_allocates_no_younger_request() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.authorize_and_execute_head();
    let older = issue_memory_head_no_response(&mut fixture);
    execute_younger(&mut fixture);
    fixture.page_map = page_map_without_younger();

    fixture
        .core
        .advance_next_data_translation(fixture.scheduler.now(), &fixture.page_map)
        .unwrap();

    let state = fixture.core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.outstanding_data.contains_key(&older));
    assert!(state.pending_trap.is_some());
    assert!(state.pending_data_translations.is_empty());
    assert!(state.ready_translated_data.is_empty());
    assert!(state.memory_result_window_authorizations.is_empty());
}

#[test]
fn younger_target_mismatch_preserves_older_request_and_allocates_no_younger_request() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.authorize_and_execute_head();
    let older = issue_memory_head_no_response(&mut fixture);
    execute_younger(&mut fixture);
    fixture
        .core
        .advance_next_data_translation(fixture.scheduler.now(), &fixture.page_map)
        .unwrap();
    let younger_fetch = {
        let mut state = fixture.core.state.lock().expect("riscv core lock");
        let younger_fetch = *state.ready_translated_data.keys().next().unwrap();
        assert!(
            state.bind_translated_result_target(younger_fetch, O3MemoryResultWindowRoute::Memory,)
        );
        younger_fetch
    };

    let error = fixture
        .core
        .issue_next_translated_mmio_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.bus,
            &fixture.page_map,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::TranslatedResultAuthorizationMismatch { fetch }
            if fetch == younger_fetch
    ));
    let state = fixture.core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.outstanding_data.contains_key(&older));
    assert!(state.pending_data_translations.is_empty());
    assert!(state.ready_translated_data.is_empty());
    assert!(state.memory_result_window_authorizations.is_empty());
}

#[test]
fn translated_suffix_cleanup_preserves_other_agent_authorization() {
    let core = translated_pair_core();
    assert!(core
        .next_cached_translated_memory_fetch_ahead_before_retire()
        .is_some());
    let unrelated_request = MemoryRequestId::new(AgentId::new(8), 0);
    let mut state = core.state.lock().expect("riscv core lock");
    let unrelated_authorization = *state
        .memory_result_window_authorizations
        .get(&request(1))
        .unwrap();
    state
        .memory_result_window_authorizations
        .insert(unrelated_request, unrelated_authorization);

    state.discard_translated_result_pair_from(request(0));

    assert_eq!(
        state
            .memory_result_window_authorizations
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![unrelated_request]
    );
}

#[test]
fn older_retry_discards_younger_translation_and_ignores_stale_completion() {
    let mut fixture = TranslatedMemoryMmioPairFixture::with_translation_latency(20);
    fixture.authorize_and_execute_head();
    issue_memory_head_retry(&mut fixture);
    execute_younger(&mut fixture);
    fixture
        .core
        .advance_next_data_translation(fixture.scheduler.now(), &fixture.page_map)
        .unwrap();
    let stale_tick = fixture.core.next_data_translation_ready_tick().unwrap();
    assert_state_counts(&fixture.core, 1, 1, 0, 1, 0);

    fixture.scheduler.run_until_idle_parallel().unwrap();
    assert_state_counts(&fixture.core, 0, 0, 0, 0, 0);

    fixture
        .core
        .advance_next_data_translation(stale_tick, &fixture.page_map)
        .unwrap();
    assert_state_counts(&fixture.core, 0, 0, 0, 0, 0);
    assert!(fixture
        .core
        .issue_next_translated_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_none());
}

#[test]
fn younger_retry_preserves_completed_older_result() {
    let mut fixture = TranslatedMemoryMmioPairFixture::with_mmio_delay(20);
    fixture
        .core
        .write_register(reg(3), YOUNGER_VIRTUAL_ADDRESS + 8);
    fixture.authorize_and_execute_head();
    let older = issue_memory_head_completed(&mut fixture);
    execute_younger(&mut fixture);
    let younger = issue_younger_mmio(&mut fixture);
    assert_ne!(older, younger);
    assert_state_counts(&fixture.core, 2, 0, 0, 0, 0);

    while fixture.core.owns_outstanding_data_request(older) {
        fixture.scheduler.run_next_epoch_parallel().unwrap();
    }
    {
        let state = fixture.core.state.lock().expect("riscv core lock");
        assert_eq!(state.outstanding_data.len(), 1);
        assert!(state.outstanding_data.contains_key(&younger));
        assert!(state.o3_runtime.snapshot().load_store_queue()[0].is_completed());
    }

    fixture.scheduler.run_until_idle_parallel().unwrap();
    assert_state_counts(&fixture.core, 0, 0, 0, 0, 0);
    let older_event = fixture
        .core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("completed older result remains publishable");
    assert_eq!(older_event.fetch_pc(), Address::new(HEAD_PC));
    assert_eq!(fixture.core.read_register(reg(11)), HEAD_VALUE);
}

#[test]
fn redirect_clears_translated_pair_authorization_binding_and_ready_state() {
    let fixture = fixture_with_ready_younger();
    prime_ready_younger_lookahead(&fixture);
    assert_state_counts(&fixture.core, 1, 0, 1, 1, 1);

    fixture.core.redirect_pc(Address::new(0x9000));

    assert_state_counts(&fixture.core, 0, 0, 0, 0, 0);
    let snapshot = fixture.core.o3_runtime_snapshot();
    assert!(snapshot.reorder_buffer().is_empty());
    assert!(snapshot.load_store_queue().is_empty());

    let fixture = fixture_with_ready_younger();
    prime_ready_younger_lookahead(&fixture);
    fixture
        .core
        .reset_instruction_fetch_stream(fixture.scheduler.now());
    assert_state_counts(&fixture.core, 1, 0, 0, 0, 0);

    let fixture = fixture_with_ready_younger();
    prime_ready_younger_lookahead(&fixture);
    fixture.core.set_detailed_live_retire_gate_enabled(false);
    assert_state_counts(&fixture.core, 1, 0, 0, 0, 0);
}

#[test]
fn translated_result_pair_live_checkpoint_rejects_and_drained_capture_succeeds() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.issue_pair(true);
    assert!(!fixture.core.data_access_lifecycle_is_quiescent());
    fixture
        .core
        .finalize_quiescent_o3_writeback_for_checkpoint();
    assert!(!fixture.core.data_access_lifecycle_is_quiescent());
    assert_eq!(
        fixture.core.capture_o3_live_data_handoff_status(),
        RiscvO3LiveDataHandoffCapture::Captured(
            fixture.core.capture_o3_live_data_handoff().unwrap()
        )
    );

    fixture.scheduler.run_until_idle_parallel().unwrap();
    while fixture
        .core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_some()
    {}
    assert!(fixture.core.data_access_lifecycle_is_quiescent());
    let checkpoint = fixture.core.o3_runtime_checkpoint_payload();
    fixture
        .core
        .restore_o3_runtime_checkpoint_payload(checkpoint)
        .unwrap();
    let snapshot = fixture.core.o3_runtime_snapshot();
    assert!(snapshot.reorder_buffer().is_empty());
    assert!(snapshot.load_store_queue().is_empty());
}

fn issue_memory_head_no_response(fixture: &mut TranslatedMemoryMmioPairFixture) -> MemoryRequestId {
    fixture
        .core
        .issue_next_translated_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("translated memory head issues");
    *fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .keys()
        .next()
        .unwrap()
}

fn fixture_with_ready_younger() -> TranslatedMemoryMmioPairFixture {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.authorize_and_execute_head();
    issue_memory_head_no_response(&mut fixture);
    execute_younger(&mut fixture);
    fixture
        .core
        .advance_next_data_translation(fixture.scheduler.now(), &fixture.page_map)
        .unwrap();
    fixture
}

fn prime_ready_younger_lookahead(fixture: &TranslatedMemoryMmioPairFixture) {
    let fetch_request = *fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .ready_translated_data
        .keys()
        .next()
        .unwrap();
    let _ = fixture
        .core
        .next_ready_translated_memory_fetch_ahead_before_issue(fetch_request);
    assert!(fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .translated_scalar_load_window_fetches
        .contains(&fetch_request));
}

fn issue_memory_head_retry(fixture: &mut TranslatedMemoryMmioPairFixture) -> MemoryRequestId {
    fixture
        .core
        .issue_next_translated_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |delivery, _| TargetOutcome::RespondAfter {
                delay: 2,
                response: MemoryResponse::retry(delivery.request()),
            },
        )
        .unwrap()
        .expect("translated memory head issues");
    *fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .keys()
        .next()
        .unwrap()
}

fn issue_memory_head_completed(fixture: &mut TranslatedMemoryMmioPairFixture) -> MemoryRequestId {
    fixture
        .core
        .issue_next_translated_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |delivery, _| TargetOutcome::RespondAfter {
                delay: 1,
                response: MemoryResponse::completed(
                    delivery.request(),
                    Some(HEAD_VALUE.to_le_bytes().to_vec()),
                )
                .unwrap(),
            },
        )
        .unwrap()
        .expect("translated memory head issues");
    *fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .keys()
        .next()
        .unwrap()
}

fn execute_younger(fixture: &mut TranslatedMemoryMmioPairFixture) {
    let action = fixture
        .core
        .drive_next_action_with_data_translation(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &fixture.page_map,
            |_, _| TargetOutcome::NoResponse,
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap();
    assert!(matches!(
        action,
        Some(RiscvCoreDriveAction::InstructionExecuted(event))
            if event.fetch_pc() == Address::new(YOUNGER_PC)
    ));
}

fn issue_younger_mmio(fixture: &mut TranslatedMemoryMmioPairFixture) -> MemoryRequestId {
    fixture
        .core
        .issue_next_translated_mmio_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.bus,
            &fixture.page_map,
        )
        .unwrap()
        .expect("translated MMIO younger issues");
    let state = fixture.core.state.lock().expect("riscv core lock");
    *state.outstanding_data.keys().next_back().unwrap()
}

fn assert_state_counts(
    core: &RiscvCore,
    outstanding: usize,
    pending: usize,
    ready: usize,
    authorizations: usize,
    lookahead: usize,
) {
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), outstanding);
    assert_eq!(state.pending_data_translations.len(), pending);
    assert_eq!(
        state
            .data_translation
            .as_ref()
            .map(CpuTranslationFrontend::pending_count)
            .unwrap_or_default(),
        pending
    );
    assert_eq!(state.ready_translated_data.len(), ready);
    assert_eq!(
        state.memory_result_window_authorizations.len(),
        authorizations
    );
    assert_eq!(state.translated_scalar_load_window_fetches.len(), lookahead);
}

fn page_map_without_younger() -> TranslationPageMap {
    page_map_with_younger_mapping(None)
}

fn page_map_with_younger_mapping(physical_address: Option<u64>) -> TranslationPageMap {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(HEAD_VIRTUAL_ADDRESS),
            Address::new(HEAD_PHYSICAL_ADDRESS),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    if let Some(physical_address) = physical_address {
        page_map
            .map(
                Address::new(YOUNGER_VIRTUAL_ADDRESS),
                Address::new(physical_address),
                1,
                TranslationPagePermissions::read_write_execute(),
            )
            .unwrap();
    }
    page_map
}
