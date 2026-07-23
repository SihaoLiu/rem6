use super::*;

#[test]
fn translated_result_pair_rejects_aliased_physical_ranges() {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);
    install_ready_younger_translation(&core, false);
    mutate_sole_ready_translation(&core, |translated| {
        translated.physical_address = Address::new(0x9000);
    });

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );
}

#[test]
fn translated_alias_waits_for_older_o3_retirement_after_response() {
    let core = translated_result_pair_with_outstanding_head(2);
    install_ready_younger_translation(&core, false);
    mutate_sole_ready_translation(&core, |translated| {
        translated.physical_address = Address::new(0x9000);
    });
    core.state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .clear();

    assert_eq!(
        core.translated_result_pair_progress(0),
        O3ResultPairProgress::Blocked
    );
}

#[test]
fn serial_driver_rechecks_physical_alias_after_translation_completion() {
    let mut fixture = translated_result_pair_ready_to_issue(2);
    fixture
        .core
        .issue_next_translated_data_access(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("translated head request issues");
    assert!(matches!(
        drive_serial_once(&mut fixture),
        Some(RiscvCoreDriveAction::InstructionExecuted(event))
            if event.fetch_pc() == Address::new(0x8004)
    ));
    fixture.page_map = aliased_page_map();

    assert!(drive_serial_once(&mut fixture).is_none());
    let state = fixture.core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(state.ready_translated_data.len(), 1);
}

#[test]
fn serial_translated_result_pair_wait_requests_selected_issue_tick() {
    let mut fixture = translated_result_pair_ready_to_issue(1);
    fixture
        .core
        .issue_next_translated_data_access(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("translated head request issues");
    let issue_tick = outstanding_issue_tick(&fixture.core);

    assert!(drive_serial_once(&mut fixture).is_none());
    assert_eq!(
        fixture
            .core
            .requested_o3_writeback_wake_tick(fixture.scheduler.now()),
        Some(issue_tick + 1)
    );
}

#[test]
fn parallel_translated_result_pair_wait_requests_selected_issue_tick() {
    let core = translated_result_pair_with_outstanding_head(1);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        crate::riscv_cluster_translation::translated_result_pair_drive_ready(&core, issue_tick),
        None
    );
    assert_eq!(
        core.requested_o3_writeback_wake_tick(issue_tick),
        Some(issue_tick + 1)
    );
}

fn aliased_page_map() -> TranslationPageMap {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    for virtual_address in [HEAD_VIRTUAL_ADDRESS, 0x5000] {
        page_map
            .map(
                Address::new(virtual_address),
                Address::new(0x9000),
                1,
                TranslationPagePermissions::read_write_execute(),
            )
            .unwrap();
    }
    page_map
}
