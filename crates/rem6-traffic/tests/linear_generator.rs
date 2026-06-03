use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    LinearTrafficGenerator, TrafficGeneratorError, TrafficLinearConfig, TrafficLinearSnapshot,
    TrafficRequestKind,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn linear_config() -> TrafficLinearConfig {
    TrafficLinearConfig::new(
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
fn linear_traffic_generator_wraps_addresses_and_builds_memory_requests() {
    let mut generator = LinearTrafficGenerator::new(linear_config());

    let first = generator.next_request(10, 0).unwrap().unwrap();
    let second = generator.next_request(14, 0).unwrap().unwrap();
    let third = generator.next_request(18, 0).unwrap().unwrap();
    let fourth = generator.next_request(22, 0).unwrap().unwrap();
    let wrapped = generator.next_request(26, 0).unwrap().unwrap();

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

    assert_eq!(second.address(), Address::new(0x1010));
    assert_eq!(third.address(), Address::new(0x1020));
    assert_eq!(fourth.address(), Address::new(0x1030));
    assert_eq!(wrapped.address(), Address::new(0x1000));
    assert_eq!(wrapped.sequence(), 4);
    assert_eq!(generator.summary().packet_count(), 5);
    assert_eq!(generator.summary().read_count(), 5);
    assert_eq!(generator.summary().bytes_read(), 80);
    assert_eq!(generator.summary().first_tick(), Some(14));
    assert_eq!(generator.summary().last_tick(), Some(30));
}

#[test]
fn linear_traffic_generator_emits_writes_with_payload_and_byte_mask() {
    let config = linear_config().with_read_percent(0).unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    let event = generator.next_request(1, 0).unwrap().unwrap();

    assert_eq!(event.kind(), TrafficRequestKind::Write);
    assert_eq!(event.request().operation(), MemoryOperation::Write);
    assert_eq!(event.request().data(), Some(&vec![7; 16][..]));
    assert_eq!(event.request().byte_mask().unwrap().len(), 16);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_written(), 16);
}

#[test]
fn linear_traffic_generator_consumes_independent_period_samples() {
    let config = linear_config()
        .with_period(2, 6)
        .unwrap()
        .with_read_percent(100)
        .unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    let first = generator.next_request(10, 0).unwrap().unwrap();
    let second = generator.next_request(20, 0).unwrap().unwrap();

    assert_eq!(first.tick(), 12);
    assert_eq!(second.tick(), 25);
}

#[test]
fn linear_traffic_generator_separates_period_and_kind_samples() {
    let config = linear_config()
        .with_period(2, 6)
        .unwrap()
        .with_read_percent(10)
        .unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    let event = generator.next_request(10, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 12);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
}

#[test]
fn linear_traffic_generator_fixed_period_consumes_rng_for_kind_order() {
    let config = linear_config()
        .with_period(4, 4)
        .unwrap()
        .with_read_percent(10)
        .unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    let event = generator.next_request(10, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 14);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
}

#[test]
fn linear_traffic_generator_accepts_full_u64_period_range() {
    let config = linear_config()
        .with_period(0, u64::MAX)
        .unwrap()
        .with_rng_state(11);
    let mut generator = LinearTrafficGenerator::new(config);

    assert_eq!(generator.schedule_tick(0, 0).unwrap(), 11);
}

#[test]
fn linear_traffic_generator_stops_at_data_limit() {
    let config = linear_config().with_data_limit(32).unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());
    assert_eq!(generator.next_request(8, 0).unwrap(), None);

    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 32);
}

#[test]
fn linear_traffic_generator_treats_zero_data_limit_as_unlimited() {
    let config = linear_config().with_data_limit(0).unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());

    assert_eq!(generator.summary().packet_count(), 2);
}

#[test]
fn linear_traffic_generator_snapshots_after_partial_block_limit() {
    let config = linear_config().with_data_limit(17).unwrap();
    let mut generator = LinearTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());
    assert_eq!(generator.next_request(8, 0).unwrap(), None);

    let snapshot = generator.snapshot();
    let mut restored = LinearTrafficGenerator::restore(snapshot).unwrap();

    assert_eq!(restored.data_manipulated(), 32);
    assert_eq!(restored.next_request(12, 0).unwrap(), None);
}

#[test]
fn linear_traffic_generator_applies_non_elastic_backpressure_to_next_tick() {
    let mut elastic = LinearTrafficGenerator::new(linear_config());
    let mut non_elastic = LinearTrafficGenerator::new(linear_config().with_elastic_requests(false));

    assert_eq!(elastic.schedule_tick(100, 3).unwrap(), 104);
    assert_eq!(non_elastic.schedule_tick(100, 3).unwrap(), 101);
    assert_eq!(non_elastic.schedule_tick(100, 9).unwrap(), 100);
}

#[test]
fn linear_traffic_generator_snapshot_restores_cursor_summary_and_rng_state() {
    let mut generator = LinearTrafficGenerator::new(linear_config().with_data_limit(64).unwrap());
    let first = generator.next_request(10, 0).unwrap().unwrap();
    assert_eq!(first.address(), Address::new(0x1000));

    let snapshot = generator.snapshot();
    let mut restored = LinearTrafficGenerator::restore(snapshot).unwrap();
    let next = restored.next_request(14, 0).unwrap().unwrap();

    assert_eq!(restored.summary().packet_count(), 2);
    assert_eq!(next.sequence(), 1);
    assert_eq!(next.address(), Address::new(0x1010));
    assert_eq!(
        next.request().id(),
        MemoryRequestId::new(AgentId::new(7), 1)
    );
}

#[test]
fn linear_traffic_generator_rejects_overflowing_restored_counters() {
    let snapshot = TrafficLinearSnapshot::new(
        linear_config(),
        Address::new(0x1000),
        u64::MAX,
        0,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    )
    .unwrap();
    let mut restored = LinearTrafficGenerator::restore(snapshot).unwrap();
    assert_eq!(
        restored.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::CounterOverflow {
            counter: "next_sequence",
            value: u64::MAX,
            increment: 1,
        }
    );

    let snapshot = TrafficLinearSnapshot::new(
        linear_config(),
        Address::new(0x1000),
        0,
        u64::MAX - 7,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    )
    .unwrap();
    let mut restored = LinearTrafficGenerator::restore(snapshot).unwrap();
    assert_eq!(
        restored.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::CounterOverflow {
            counter: "data_manipulated",
            value: u64::MAX - 7,
            increment: 16,
        }
    );
}

#[test]
fn linear_traffic_generator_rejects_invalid_config_and_snapshots() {
    assert_eq!(
        TrafficLinearConfig::new(
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
        linear_config().with_read_percent(101).unwrap_err(),
        TrafficGeneratorError::InvalidReadPercent { read_percent: 101 }
    );

    assert_eq!(
        linear_config().with_period(9, 3).unwrap_err(),
        TrafficGeneratorError::InvertedPeriod {
            min_period: 9,
            max_period: 3,
        }
    );

    assert_eq!(
        TrafficLinearConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            AccessSize::new(128).unwrap(),
        )
        .unwrap_err(),
        TrafficGeneratorError::BlockSizeExceedsCacheLine {
            block_size: 128,
            cache_line_size: 64,
        }
    );

    assert_eq!(
        TrafficLinearConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1030),
            AccessSize::new(32).unwrap(),
        )
        .unwrap_err(),
        TrafficGeneratorError::BlockSizeDoesNotDivideRange {
            block_size: 32,
            range_size: 48,
        }
    );

    let snapshot = TrafficLinearSnapshot::new(
        linear_config(),
        Address::new(0x2000),
        0,
        0,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    );
    assert_eq!(
        snapshot.unwrap_err(),
        TrafficGeneratorError::SnapshotCursorOutsideRange {
            next_address: Address::new(0x2000),
            start: Address::new(0x1000),
            end: Address::new(0x1040),
        }
    );

    let snapshot = TrafficLinearSnapshot::new(
        linear_config(),
        Address::new(0x1001),
        0,
        0,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    );
    assert_eq!(
        snapshot.unwrap_err(),
        TrafficGeneratorError::SnapshotCursorOutsideBlockGrid {
            next_address: Address::new(0x1001),
            start: Address::new(0x1000),
            block_size: 16,
        }
    );
}
