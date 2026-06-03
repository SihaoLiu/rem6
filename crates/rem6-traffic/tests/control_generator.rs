use rem6_traffic::{
    TrafficExitConfig, TrafficExitGenerator, TrafficExitSnapshot, TrafficGeneratorError,
    TrafficIdleConfig, TrafficIdleGenerator, TrafficIdleSnapshot, TrafficRequestEvent,
};

fn expect_no_request(
    result: Result<Option<TrafficRequestEvent>, TrafficGeneratorError>,
) -> Result<(), TrafficGeneratorError> {
    assert_eq!(result?, None);
    Ok(())
}

#[test]
fn idle_traffic_generator_never_schedules_or_emits_packets() {
    let config = TrafficIdleConfig::new(37);
    let mut generator = TrafficIdleGenerator::new(config);

    generator.enter();

    assert_eq!(generator.config().duration(), 37);
    assert!(generator.entered());
    assert_eq!(generator.schedule_tick(10, 7).unwrap(), u64::MAX);
    expect_no_request(generator.next_request(10, 7)).unwrap();
    assert_eq!(generator.summary().packet_count(), 0);
}

#[test]
fn idle_traffic_generator_snapshot_restores_entry_state() {
    let mut generator = TrafficIdleGenerator::new(TrafficIdleConfig::new(11));
    generator.enter();

    let snapshot = generator.snapshot();
    let restored = TrafficIdleGenerator::restore(snapshot);

    assert_eq!(restored.config().duration(), 11);
    assert!(restored.entered());

    let explicit = TrafficIdleSnapshot::new(TrafficIdleConfig::new(9), false);
    let restored = TrafficIdleGenerator::restore(explicit);

    assert_eq!(restored.config().duration(), 9);
    assert!(!restored.entered());
}

#[test]
fn exit_traffic_generator_records_typed_exit_event_on_entry() {
    let config = TrafficExitConfig::new(23);
    let mut generator = TrafficExitGenerator::new(config);

    let event = generator.enter(123);

    assert_eq!(event.tick(), 123);
    assert_eq!(event.duration(), 23);
    assert_eq!(event.reason(), "traffic generator exit state entered");
    assert!(generator.exited());
    assert_eq!(generator.exit_tick(), Some(123));
    assert_eq!(generator.schedule_tick(123, 3).unwrap(), u64::MAX);
    expect_no_request(generator.next_request(123, 3)).unwrap();
}

#[test]
fn exit_traffic_generator_preserves_first_exit_tick_on_repeated_entry() {
    let mut generator = TrafficExitGenerator::new(TrafficExitConfig::new(23));

    let first = generator.enter(123);
    let repeated = generator.enter(456);

    assert_eq!(first.tick(), 123);
    assert_eq!(repeated.tick(), 123);
    assert_eq!(generator.exit_tick(), Some(123));
}

#[test]
fn exit_traffic_generator_snapshot_restores_exit_tick() {
    let mut generator = TrafficExitGenerator::new(TrafficExitConfig::new(17));
    generator.enter(99);

    let snapshot = generator.snapshot();
    let restored = TrafficExitGenerator::restore(snapshot);

    assert_eq!(restored.config().duration(), 17);
    assert!(restored.exited());
    assert_eq!(restored.exit_tick(), Some(99));

    let explicit = TrafficExitSnapshot::new(TrafficExitConfig::new(5), None);
    let restored = TrafficExitGenerator::restore(explicit);

    assert_eq!(restored.config().duration(), 5);
    assert!(!restored.exited());
    assert_eq!(restored.exit_tick(), None);
}
