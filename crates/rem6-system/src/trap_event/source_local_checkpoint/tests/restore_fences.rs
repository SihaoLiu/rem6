use super::*;

#[path = "restore_fences/cleanup_ownership.rs"]
mod cleanup_ownership;

#[test]
fn scheduled_restore_preflight_blocks_new_fetches_until_delivery() {
    let fixture = fixture(3, 2);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    fixture
        .trap
        .schedule_host_checkpoint_restore_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            2,
            "missing".to_string(),
        )
        .unwrap();

    assert_eq!(scheduler.snapshot().total_pending_events(), 3);
    run_until_tick(&mut scheduler, 1);
    assert!(drive_fetch(&fixture, &mut scheduler).is_none());
    run_until_tick(&mut scheduler, 2);
    assert!(drive_fetch(&fixture, &mut scheduler).is_none());

    run_until_next_host_result(&fixture, &mut scheduler);

    assert!(matches!(
        fixture.controller.lock().unwrap().action_errors(),
        [SystemError::MissingCheckpointManifest { label }] if label == "missing"
    ));
}

#[test]
fn scheduled_restore_source_emit_does_not_duplicate_preflight_fences() {
    let fixture = fixture(3, 2);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    fixture
        .trap
        .schedule_host_checkpoint_restore_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            2,
            "missing".to_string(),
        )
        .unwrap();

    run_until_tick(&mut scheduler, 3);
    fixture.core.release_source_local_checkpoint_capture(5);
    fixture
        .core
        .release_source_local_checkpoint_restore_after(2, 5);
    assert!(!fixture
        .core
        .source_local_checkpoint_restore_blocks_new_work(scheduler.now()));
    assert!(matches!(
        drive_fetch(&fixture, &mut scheduler),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn scheduled_restore_preflight_drains_completed_pipeline_through_delivery() {
    let fixture = fixture(3, 2);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    fixture
        .core
        .issue_next_fetch(
            &mut scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(0x0000_0013_u32.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
    scheduler.run_until_idle_conservative();
    assert_eq!(
        fixture
            .core
            .inner()
            .fetch_events()
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        [CpuFetchEventKind::Issued, CpuFetchEventKind::Completed]
    );
    let restore_tick = scheduler.now() + 2;
    fixture
        .trap
        .schedule_host_checkpoint_restore_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            restore_tick,
            "missing".to_string(),
        )
        .unwrap();

    run_until_tick(&mut scheduler, restore_tick - 1);
    let action = drive_fetch(&fixture, &mut scheduler).expect("preflight drain action");
    assert!(!matches!(action, RiscvCoreDriveAction::FetchIssued { .. }));

    run_until_tick(&mut scheduler, restore_tick);
    let action = drive_fetch(&fixture, &mut scheduler).expect("source-tick drain action");
    assert!(!matches!(action, RiscvCoreDriveAction::FetchIssued { .. }));

    run_until_tick(&mut scheduler, restore_tick + 1);
    let action = drive_fetch(&fixture, &mut scheduler).expect("post-source drain action");
    assert!(!matches!(action, RiscvCoreDriveAction::FetchIssued { .. }));

    run_until_next_host_result(&fixture, &mut scheduler);
    assert!(drive_fetch(&fixture, &mut scheduler).is_some());
}

#[test]
fn canceled_scheduled_restore_releases_preflight_fences_at_deadline() {
    let fixture = fixture(3, 2);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    let restore = fixture
        .trap
        .schedule_host_checkpoint_restore_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            2,
            "missing".to_string(),
        )
        .unwrap();

    scheduler.cancel_event(restore).unwrap();
    run_until_tick(&mut scheduler, 4);
    assert!(drive_fetch(&fixture, &mut scheduler).is_none());

    run_until_tick(&mut scheduler, 5);
    assert!(matches!(
        drive_fetch(&fixture, &mut scheduler),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn scheduled_restore_preflight_does_not_block_an_earlier_checkpoint_delivery() {
    let fixture = fixture(3, 2);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    fixture
        .trap
        .schedule_host_checkpoint_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            0,
            "overlap".to_string(),
        )
        .unwrap();
    fixture
        .trap
        .schedule_host_checkpoint_restore_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(2),
            PartitionId::new(0),
            2,
            "overlap".to_string(),
        )
        .unwrap();

    run_until_next_host_result(&fixture, &mut scheduler);
    assert_eq!(scheduler.now(), 3);
    run_until_next_host_result(&fixture, &mut scheduler);

    assert_eq!(scheduler.now(), 3);
    let controller = fixture.controller.lock().unwrap();
    assert!(controller.action_errors().is_empty());
    assert_eq!(controller.run().action_outcomes().len(), 2);
}

fn run_until_tick(scheduler: &mut PartitionedScheduler, target: Tick) {
    for _ in 0..16 {
        if scheduler.now() == target {
            return;
        }
        assert!(scheduler.now() < target);
        scheduler.run_next_epoch();
    }
    panic!("scheduler did not reach tick {target}");
}

#[test]
fn generic_restore_delivery_does_not_release_source_local_restore_fence() {
    let fixture = fixture(3, 2);
    fixture.core.prepare_source_local_checkpoint_restore(3);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    fixture
        .trap
        .schedule_host_checkpoint_restore_event(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            0,
            "missing".to_string(),
        )
        .unwrap();

    run_until_next_host_result(&fixture, &mut scheduler);

    assert_eq!(scheduler.now(), 3);
    assert!(matches!(
        fixture.controller.lock().unwrap().action_errors(),
        [SystemError::MissingCheckpointManifest { label }] if label == "missing"
    ));
    assert!(drive_fetch(&fixture, &mut scheduler).is_none());
    fixture.core.release_source_local_checkpoint_restore(3);
    assert!(matches!(
        drive_fetch(&fixture, &mut scheduler),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn scheduled_restore_delivery_releases_only_its_preflight_reference() {
    let fixture = fixture(3, 2);
    fixture.core.prepare_source_local_checkpoint_capture(5);
    fixture.core.prepare_source_local_checkpoint_restore(5);
    let mut scheduler = fixture.scheduler.lock().unwrap();
    fixture
        .trap
        .schedule_host_checkpoint_restore_event_on_source_parallel(
            &mut scheduler,
            GuestEventId::new(1),
            PartitionId::new(0),
            2,
            "missing".to_string(),
        )
        .unwrap();

    run_until_next_host_result(&fixture, &mut scheduler);

    assert!(drive_fetch(&fixture, &mut scheduler).is_none());
    fixture.core.release_source_local_checkpoint_capture(5);
    fixture.core.release_source_local_checkpoint_restore(5);
    assert!(matches!(
        drive_fetch(&fixture, &mut scheduler),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}
