use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    RandomTrafficGenerator, TrafficGeneratorError, TrafficRandomConfig, TrafficRandomSnapshot,
    TrafficRequestKind,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn random_config() -> TrafficRandomConfig {
    TrafficRandomConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x1040),
        AccessSize::new(16).unwrap(),
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap()
}

#[test]
fn random_traffic_generator_aligns_addresses_and_builds_memory_requests() {
    let mut generator = RandomTrafficGenerator::new(random_config());

    let first = generator.next_request(10, 0).unwrap().unwrap();
    let second = generator.next_request(14, 0).unwrap().unwrap();
    let third = generator.next_request(18, 0).unwrap().unwrap();
    let fourth = generator.next_request(22, 0).unwrap().unwrap();

    assert_eq!(first.tick(), 14);
    assert_eq!(first.sequence(), 0);
    assert_eq!(first.kind(), TrafficRequestKind::Read);
    assert_eq!(first.address(), Address::new(0x1000));
    assert_eq!(
        first.request().id(),
        MemoryRequestId::new(AgentId::new(7), 0)
    );
    assert_eq!(first.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(first.request().range().start(), Address::new(0x1000));
    assert_eq!(first.request().size(), AccessSize::new(16).unwrap());

    assert_eq!(second.address(), Address::new(0x1030));
    assert_eq!(third.address(), Address::new(0x1030));
    assert_eq!(fourth.address(), Address::new(0x1000));
    assert_eq!(fourth.sequence(), 3);
    assert_eq!(generator.summary().packet_count(), 4);
    assert_eq!(generator.summary().read_count(), 4);
    assert_eq!(generator.summary().bytes_read(), 64);
    assert_eq!(generator.summary().first_tick(), Some(14));
    assert_eq!(generator.summary().last_tick(), Some(26));
}

#[test]
fn random_traffic_generator_emits_writes_with_payload_and_byte_mask() {
    let config = random_config().with_read_percent(0).unwrap();
    let mut generator = RandomTrafficGenerator::new(config);

    let event = generator.next_request(1, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 5);
    assert_eq!(event.kind(), TrafficRequestKind::Write);
    assert_eq!(event.address(), Address::new(0x1000));
    assert_eq!(event.request().operation(), MemoryOperation::Write);
    assert_eq!(event.request().data(), Some(&vec![7; 16][..]));
    assert_eq!(event.request().byte_mask().unwrap().len(), 16);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_written(), 16);
}

#[test]
fn random_traffic_generator_separates_period_kind_and_address_samples() {
    let config = random_config()
        .with_period(2, 6)
        .unwrap()
        .with_read_percent(10)
        .unwrap();
    let mut generator = RandomTrafficGenerator::new(config);

    let event = generator.next_request(10, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 12);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
    assert_eq!(event.address(), Address::new(0x1000));
}

#[test]
fn random_traffic_generator_fixed_period_consumes_rng_for_address_order() {
    let mut generator = RandomTrafficGenerator::new(random_config());

    assert_eq!(generator.schedule_tick(0, 0).unwrap(), 4);
    let event = generator.next_request(10, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 14);
    assert_eq!(event.address(), Address::new(0x1000));
}

#[test]
fn random_traffic_generator_stops_at_data_limit() {
    let config = random_config().with_data_limit(32).unwrap();
    let mut generator = RandomTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());
    assert_eq!(generator.next_request(8, 0).unwrap(), None);

    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 32);
}

#[test]
fn random_traffic_generator_treats_zero_data_limit_as_unlimited() {
    let config = random_config().with_data_limit(0).unwrap();
    let mut generator = RandomTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());

    assert_eq!(generator.summary().packet_count(), 2);
}

#[test]
fn random_traffic_generator_applies_non_elastic_backpressure_to_next_tick() {
    let mut elastic = RandomTrafficGenerator::new(random_config());
    let mut non_elastic = RandomTrafficGenerator::new(random_config().with_elastic_requests(false));

    assert_eq!(elastic.schedule_tick(100, 3).unwrap(), 104);
    assert_eq!(non_elastic.schedule_tick(100, 3).unwrap(), 101);
    assert_eq!(non_elastic.schedule_tick(100, 9).unwrap(), 100);
}

#[test]
fn random_traffic_generator_snapshot_restores_summary_and_rng_state() {
    let mut generator = RandomTrafficGenerator::new(random_config().with_data_limit(64).unwrap());
    let first = generator.next_request(10, 0).unwrap().unwrap();
    assert_eq!(first.address(), Address::new(0x1000));

    let snapshot = generator.snapshot();
    let mut restored = RandomTrafficGenerator::restore(snapshot).unwrap();
    let next = restored.next_request(14, 0).unwrap().unwrap();

    assert_eq!(restored.summary().packet_count(), 2);
    assert_eq!(next.sequence(), 1);
    assert_eq!(next.address(), Address::new(0x1030));
    assert_eq!(
        next.request().id(),
        MemoryRequestId::new(AgentId::new(7), 1)
    );
}

#[test]
fn random_traffic_generator_rejects_invalid_config_and_snapshots() {
    assert_eq!(
        TrafficRandomConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x2000),
            Address::new(0x2000),
            AccessSize::new(16).unwrap(),
        )
        .unwrap_err(),
        TrafficGeneratorError::EmptyAddressRange {
            start: Address::new(0x2000),
            end: Address::new(0x2000),
        }
    );

    assert_eq!(
        random_config().with_read_percent(101).unwrap_err(),
        TrafficGeneratorError::InvalidReadPercent { read_percent: 101 }
    );

    assert_eq!(
        random_config().with_period(9, 3).unwrap_err(),
        TrafficGeneratorError::InvertedPeriod {
            min_period: 9,
            max_period: 3,
        }
    );

    let snapshot = TrafficRandomSnapshot::new(
        random_config(),
        u64::MAX,
        0,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    )
    .unwrap();
    let mut restored = RandomTrafficGenerator::restore(snapshot).unwrap();
    assert_eq!(
        restored.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::CounterOverflow {
            counter: "next_sequence",
            value: u64::MAX,
            increment: 1,
        }
    );
}

#[test]
fn random_traffic_generator_allows_gem5_partial_and_unaligned_ranges() {
    let partial = TrafficRandomConfig::new(
        AgentId::new(1),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x1030),
        AccessSize::new(32).unwrap(),
    )
    .unwrap();
    assert_eq!(partial.start(), Address::new(0x1000));
    assert_eq!(partial.end(), Address::new(0x1030));

    let unaligned = TrafficRandomConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1008),
        Address::new(0x1048),
        AccessSize::new(16).unwrap(),
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap();
    let mut generator = RandomTrafficGenerator::new(unaligned);

    let event = generator.next_request(0, 0).unwrap().unwrap();

    assert_eq!(event.address(), Address::new(0x1000));
}
