use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    StridedTrafficGenerator, TrafficGeneratorError, TrafficRequestKind, TrafficStridedConfig,
    TrafficStridedSnapshot,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn strided_config() -> TrafficStridedConfig {
    TrafficStridedConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x1000),
        Address::new(0x10a0),
        0,
        AccessSize::new(16).unwrap(),
        32,
        64,
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap()
}

#[test]
fn strided_traffic_generator_walks_superblocks_strides_and_wraps() {
    let mut generator = StridedTrafficGenerator::new(strided_config());

    let first = generator.next_request(10, 0).unwrap().unwrap();
    let second = generator.next_request(14, 0).unwrap().unwrap();
    let third = generator.next_request(18, 0).unwrap().unwrap();
    let fourth = generator.next_request(22, 0).unwrap().unwrap();
    let fifth = generator.next_request(26, 0).unwrap().unwrap();
    let sixth = generator.next_request(30, 0).unwrap().unwrap();
    let wrapped = generator.next_request(34, 0).unwrap().unwrap();

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
    assert_eq!(third.address(), Address::new(0x1040));
    assert_eq!(fourth.address(), Address::new(0x1050));
    assert_eq!(fifth.address(), Address::new(0x1080));
    assert_eq!(sixth.address(), Address::new(0x1090));
    assert_eq!(wrapped.address(), Address::new(0x1000));
    assert_eq!(wrapped.sequence(), 6);
    assert_eq!(generator.summary().packet_count(), 7);
    assert_eq!(generator.summary().read_count(), 7);
    assert_eq!(generator.summary().bytes_read(), 112);
    assert_eq!(generator.summary().first_tick(), Some(14));
    assert_eq!(generator.summary().last_tick(), Some(38));
}

#[test]
fn strided_traffic_generator_uses_offset_and_mixed_read_rng_after_wait() {
    let config = TrafficStridedConfig::new(
        AgentId::new(7),
        line_layout(),
        Address::new(0x2000),
        Address::new(0x2100),
        0x40,
        AccessSize::new(16).unwrap(),
        32,
        64,
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(10)
    .unwrap();
    let mut generator = StridedTrafficGenerator::new(config);

    let event = generator.next_request(10, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 14);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
    assert_eq!(event.address(), Address::new(0x2040));
}

#[test]
fn strided_traffic_generator_emits_writes_with_payload_and_byte_mask() {
    let config = strided_config().with_read_percent(0).unwrap();
    let mut generator = StridedTrafficGenerator::new(config);

    let event = generator.next_request(1, 0).unwrap().unwrap();

    assert_eq!(event.kind(), TrafficRequestKind::Write);
    assert_eq!(event.request().operation(), MemoryOperation::Write);
    assert_eq!(event.request().data(), Some(&vec![7; 16][..]));
    assert_eq!(event.request().byte_mask().unwrap().len(), 16);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_written(), 16);
}

#[test]
fn strided_traffic_generator_stops_after_partial_data_limit_overshoots_one_block() {
    let config = strided_config().with_data_limit(17).unwrap();
    let mut generator = StridedTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());
    assert_eq!(generator.next_request(8, 0).unwrap(), None);

    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 32);
}

#[test]
fn strided_traffic_generator_treats_zero_data_limit_as_unlimited() {
    let config = strided_config().with_data_limit(0).unwrap();
    let mut generator = StridedTrafficGenerator::new(config);

    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());

    assert_eq!(generator.summary().packet_count(), 2);
}

#[test]
fn strided_traffic_generator_applies_non_elastic_backpressure_to_next_tick() {
    let mut elastic = StridedTrafficGenerator::new(strided_config());
    let mut non_elastic =
        StridedTrafficGenerator::new(strided_config().with_elastic_requests(false));

    assert_eq!(elastic.schedule_tick(100, 3).unwrap(), 104);
    assert_eq!(non_elastic.schedule_tick(100, 3).unwrap(), 101);
    assert_eq!(non_elastic.schedule_tick(100, 9).unwrap(), 100);
}

#[test]
fn strided_traffic_generator_snapshot_restores_cursor_summary_and_rng_state() {
    let mut generator = StridedTrafficGenerator::new(strided_config().with_data_limit(64).unwrap());
    let first = generator.next_request(10, 0).unwrap().unwrap();
    assert_eq!(first.address(), Address::new(0x1000));

    let snapshot = generator.snapshot();
    let mut restored = StridedTrafficGenerator::restore(snapshot).unwrap();
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
fn strided_traffic_generator_rejects_overflowing_restored_counters() {
    let snapshot = TrafficStridedSnapshot::new(
        strided_config(),
        Address::new(0x1000),
        u64::MAX,
        0,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    )
    .unwrap();
    let mut restored = StridedTrafficGenerator::restore(snapshot).unwrap();
    assert_eq!(
        restored.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::CounterOverflow {
            counter: "next_sequence",
            value: u64::MAX,
            increment: 1,
        }
    );

    let snapshot = TrafficStridedSnapshot::new(
        strided_config(),
        Address::new(0x1000),
        0,
        u64::MAX - 7,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    )
    .unwrap();
    let mut restored = StridedTrafficGenerator::restore(snapshot).unwrap();
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
fn strided_traffic_generator_rejects_invalid_config_and_snapshots() {
    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x2000),
            Address::new(0x2000),
            0,
            AccessSize::new(16).unwrap(),
            32,
            64,
        )
        .unwrap_err(),
        TrafficGeneratorError::EmptyAddressRange {
            start: Address::new(0x2000),
            end: Address::new(0x2000),
        }
    );

    assert_eq!(
        strided_config().with_read_percent(101).unwrap_err(),
        TrafficGeneratorError::InvalidReadPercent { read_percent: 101 }
    );

    assert_eq!(
        strided_config().with_period(9, 3).unwrap_err(),
        TrafficGeneratorError::InvertedPeriod {
            min_period: 9,
            max_period: 3,
        }
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            0,
            AccessSize::new(128).unwrap(),
            128,
            256,
        )
        .unwrap_err(),
        TrafficGeneratorError::BlockSizeExceedsCacheLine {
            block_size: 128,
            cache_line_size: 64,
        }
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            0,
            AccessSize::new(16).unwrap(),
            0,
            64,
        )
        .unwrap_err(),
        TrafficGeneratorError::ZeroSuperblockSize
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            0,
            AccessSize::new(16).unwrap(),
            32,
            0,
        )
        .unwrap_err(),
        TrafficGeneratorError::ZeroStrideSize
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            0,
            AccessSize::new(16).unwrap(),
            40,
            80,
        )
        .unwrap_err(),
        TrafficGeneratorError::SuperblockSizeNotMultipleOfBlockSize {
            superblock_size: 40,
            block_size: 16,
        }
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            16,
            AccessSize::new(16).unwrap(),
            32,
            64,
        )
        .unwrap_err(),
        TrafficGeneratorError::OffsetNotMultipleOfSuperblock {
            offset: 16,
            superblock_size: 32,
        }
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            0,
            AccessSize::new(16).unwrap(),
            32,
            48,
        )
        .unwrap_err(),
        TrafficGeneratorError::StrideSizeNotMultipleOfSuperblock {
            stride_size: 48,
            superblock_size: 32,
        }
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(0x1000),
            Address::new(0x1100),
            0x100,
            AccessSize::new(16).unwrap(),
            32,
            64,
        )
        .unwrap_err(),
        TrafficGeneratorError::StridedOffsetOutsideRange {
            next_address: Address::new(0x1100),
            start: Address::new(0x1000),
            end: Address::new(0x1100),
        }
    );

    assert_eq!(
        TrafficStridedConfig::new(
            AgentId::new(1),
            line_layout(),
            Address::new(u64::MAX - 8),
            Address::new(u64::MAX),
            32,
            AccessSize::new(16).unwrap(),
            16,
            16,
        )
        .unwrap_err(),
        TrafficGeneratorError::AddressOverflow {
            label: "strided_start",
            value: u64::MAX - 8,
            increment: 32,
        }
    );

    let snapshot = TrafficStridedSnapshot::new(
        strided_config(),
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
            end: Address::new(0x10a0),
        }
    );

    let snapshot = TrafficStridedSnapshot::new(
        strided_config(),
        Address::new(0x1020),
        0,
        0,
        rem6_traffic::TrafficGeneratorSummary::default(),
        1,
    );
    assert_eq!(
        snapshot.unwrap_err(),
        TrafficGeneratorError::SnapshotCursorOutsideBlockGrid {
            next_address: Address::new(0x1020),
            start: Address::new(0x1000),
            block_size: 16,
        }
    );
}
