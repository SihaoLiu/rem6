use super::*;

#[test]
fn canceled_scheduled_restore_preserves_same_tuple_restore_reference() {
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
    fixture
        .core
        .prepare_source_local_checkpoint_restore_after(2, 5);

    scheduler.cancel_event(restore).unwrap();
    run_until_tick(&mut scheduler, 5);

    assert!(fixture
        .core
        .source_local_checkpoint_restore_blocks_new_work(5));
    fixture
        .core
        .release_source_local_checkpoint_restore_after(2, 5);
    assert!(!fixture
        .core
        .source_local_checkpoint_restore_blocks_new_work(5));
}

#[test]
fn canceled_restore_delivery_releases_only_its_fence_references() {
    let fixture = fixture(3, 2);
    fixture.core.prepare_source_local_checkpoint_capture(5);
    fixture
        .core
        .prepare_source_local_checkpoint_restore_after(2, 5);
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

    run_until_tick(&mut scheduler, 2);
    let mut deadline_events = scheduler.snapshot().partitions()[0]
        .pending_events()
        .iter()
        .copied()
        .filter(|event| event.tick() == 5)
        .collect::<Vec<_>>();
    deadline_events.sort_by_key(|event| event.order());
    assert_eq!(deadline_events.len(), 3);
    scheduler.cancel_event(deadline_events[1].id()).unwrap();
    run_until_tick(&mut scheduler, 5);

    assert!(fixture
        .core
        .source_local_checkpoint_restore_blocks_new_work(5));
    assert!(drive_fetch(&fixture, &mut scheduler).is_none());
    fixture.core.release_source_local_checkpoint_capture(5);
    fixture
        .core
        .release_source_local_checkpoint_restore_after(2, 5);
    assert!(!fixture
        .core
        .source_local_checkpoint_restore_blocks_new_work(5));
    assert!(matches!(
        drive_fetch(&fixture, &mut scheduler),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}
