use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    DramTrafficGenerator, TrafficDramAddressMapping, TrafficDramConfig, TrafficDramMode,
    TrafficDramSnapshot, TrafficGeneratorError, TrafficGeneratorSummary, TrafficRequestKind,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn dram_config(mode: TrafficDramMode) -> TrafficDramConfig {
    TrafficDramConfig::new(
        AgentId::new(7),
        line_layout(),
        mode,
        Address::new(0),
        Address::new(0x4000),
        AccessSize::new(16).unwrap(),
        64,
        2,
        2,
        TrafficDramAddressMapping::RoRaBaCoCh,
        2,
        2,
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap()
}

fn bank_rank_row_col(address: Address) -> (u64, u64, u64, u64) {
    let block_bits = 4;
    let page_bits = 2;
    let bank_bits = 1;
    let rank_bits = 1;
    let raw = address.get();
    let col = (raw >> block_bits) & ((1 << page_bits) - 1);
    let bank = (raw >> (block_bits + page_bits)) & ((1 << bank_bits) - 1);
    let rank = (raw >> (block_bits + page_bits + bank_bits)) & ((1 << rank_bits) - 1);
    let row = raw >> (block_bits + page_bits + bank_bits + rank_bits);
    (bank, rank, row, col)
}

#[test]
fn dram_rotate_generator_rotates_banks_types_and_ranks() {
    let config = dram_config(TrafficDramMode::DramRotate)
        .with_read_percent(50)
        .unwrap();
    let mut generator = DramTrafficGenerator::new(config);

    let events = (0..9)
        .map(|index| generator.next_request(index * 4, 0).unwrap().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(events[0].tick(), 4);
    assert_eq!(events[0].sequence(), 0);
    assert_eq!(events[0].kind(), TrafficRequestKind::Write);
    assert_eq!(
        events[0].request().id(),
        MemoryRequestId::new(AgentId::new(7), 0)
    );
    assert_eq!(events[0].request().operation(), MemoryOperation::Write);
    assert_eq!(events[0].request().data(), Some(&vec![7; 16][..]));
    assert_eq!(events[0].request().byte_mask().unwrap().len(), 16);

    assert_eq!(events[1].kind(), TrafficRequestKind::Write);
    assert_eq!(bank_rank_row_col(events[0].address()).0, 0);
    assert_eq!(bank_rank_row_col(events[1].address()).0, 0);
    assert_eq!(bank_rank_row_col(events[0].address()).1, 0);
    assert_eq!(bank_rank_row_col(events[1].address()).1, 0);
    assert_eq!(
        bank_rank_row_col(events[1].address()).3,
        bank_rank_row_col(events[0].address()).3 + 1
    );

    assert_eq!(events[2].kind(), TrafficRequestKind::Write);
    assert_eq!(bank_rank_row_col(events[2].address()).0, 1);
    assert_eq!(bank_rank_row_col(events[2].address()).1, 0);

    assert_eq!(events[4].kind(), TrafficRequestKind::Read);
    assert_eq!(bank_rank_row_col(events[4].address()).0, 0);
    assert_eq!(bank_rank_row_col(events[4].address()).1, 0);

    assert_eq!(events[8].kind(), TrafficRequestKind::Write);
    assert_eq!(bank_rank_row_col(events[8].address()).0, 0);
    assert_eq!(bank_rank_row_col(events[8].address()).1, 1);

    assert_eq!(generator.summary().packet_count(), 9);
    assert_eq!(generator.summary().read_count(), 4);
    assert_eq!(generator.summary().write_count(), 5);
}

#[test]
fn dram_generator_keeps_sequential_packets_inside_page_for_ro_co_mapping() {
    let config = TrafficDramConfig::new(
        AgentId::new(7),
        line_layout(),
        TrafficDramMode::Dram,
        Address::new(0),
        Address::new(0x4000),
        AccessSize::new(16).unwrap(),
        64,
        2,
        2,
        TrafficDramAddressMapping::RoCoRaBaCh,
        2,
        3,
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap();
    let mut generator = DramTrafficGenerator::new(config);

    let first = generator.next_request(0, 0).unwrap().unwrap();
    let second = generator.next_request(4, 0).unwrap().unwrap();
    let third = generator.next_request(8, 0).unwrap().unwrap();

    assert_eq!(first.kind(), TrafficRequestKind::Read);
    assert_eq!(second.kind(), TrafficRequestKind::Read);
    assert_eq!(third.kind(), TrafficRequestKind::Read);
    assert_eq!(first.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(first.request().range().start(), first.address());
    assert_eq!(first.request().size(), AccessSize::new(16).unwrap());

    assert_eq!(
        ro_co_bank_rank_col(first.address()).0,
        ro_co_bank_rank_col(second.address()).0
    );
    assert_eq!(
        ro_co_bank_rank_col(first.address()).1,
        ro_co_bank_rank_col(second.address()).1
    );
    assert_eq!(
        ro_co_bank_rank_col(second.address()).2,
        ro_co_bank_rank_col(first.address()).2 + 1
    );
    assert_eq!(
        ro_co_bank_rank_col(third.address()).2,
        ro_co_bank_rank_col(second.address()).2 + 1
    );
}

fn ro_co_bank_rank_col(address: Address) -> (u64, u64, u64) {
    let block_bits = 4;
    let bank_bits = 1;
    let rank_bits = 1;
    let page_bits = 2;
    let raw = address.get();
    let bank = (raw >> block_bits) & ((1 << bank_bits) - 1);
    let rank = (raw >> (block_bits + bank_bits)) & ((1 << rank_bits) - 1);
    let col = (raw >> (block_bits + bank_bits + rank_bits)) & ((1 << page_bits) - 1);
    (bank, rank, col)
}

#[test]
fn nvm_generator_uses_buffer_geometry_and_write_payloads() {
    let config = dram_config(TrafficDramMode::Nvm)
        .with_read_percent(0)
        .unwrap();
    let mut generator = DramTrafficGenerator::new(config);

    let first = generator.next_request(0, 0).unwrap().unwrap();
    let second = generator.next_request(4, 0).unwrap().unwrap();

    assert_eq!(first.kind(), TrafficRequestKind::Write);
    assert_eq!(second.kind(), TrafficRequestKind::Write);
    assert_eq!(first.request().operation(), MemoryOperation::Write);
    assert_eq!(first.request().data(), Some(&vec![7; 16][..]));
    assert_eq!(
        bank_rank_row_col(second.address()).3,
        bank_rank_row_col(first.address()).3 + 1
    );
    assert_eq!(generator.summary().bytes_written(), 32);
}

#[test]
fn dram_generator_data_limit_and_non_elastic_timing_match_random_family() {
    let config = dram_config(TrafficDramMode::Dram)
        .with_data_limit(17)
        .unwrap()
        .with_elastic_requests(false);
    let mut generator = DramTrafficGenerator::new(config);

    assert_eq!(generator.schedule_tick(100, 3).unwrap(), 101);
    assert_eq!(generator.schedule_tick(100, 9).unwrap(), 100);
    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());
    assert_eq!(generator.next_request(8, 0).unwrap(), None);
    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 32);
}

#[test]
fn dram_generator_snapshot_restores_series_cursor_and_rng_state() {
    let mut generator = DramTrafficGenerator::new(dram_config(TrafficDramMode::Dram));
    let first = generator.next_request(0, 0).unwrap().unwrap();
    let snapshot = generator.snapshot();
    let mut restored = DramTrafficGenerator::restore(snapshot).unwrap();
    let next = restored.next_request(4, 0).unwrap().unwrap();

    assert_eq!(next.sequence(), 1);
    assert_eq!(
        bank_rank_row_col(next.address()).0,
        bank_rank_row_col(first.address()).0
    );
    assert_eq!(
        bank_rank_row_col(next.address()).1,
        bank_rank_row_col(first.address()).1
    );
    assert_eq!(
        bank_rank_row_col(next.address()).3,
        bank_rank_row_col(first.address()).3 + 1
    );
    assert_eq!(restored.summary().packet_count(), 2);
}

#[test]
fn dram_generator_rejects_invalid_config_and_snapshots() {
    assert_eq!(
        TrafficDramConfig::new(
            AgentId::new(7),
            line_layout(),
            TrafficDramMode::Dram,
            Address::new(0),
            Address::new(0x4000),
            AccessSize::new(16).unwrap(),
            64,
            2,
            3,
            TrafficDramAddressMapping::RoRaBaCoCh,
            1,
            1,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramBanksUtilExceedsAvailable {
            banks_util: 3,
            banks: 2,
        }
    );

    assert_eq!(
        TrafficDramConfig::new(
            AgentId::new(7),
            line_layout(),
            TrafficDramMode::Dram,
            Address::new(0),
            Address::new(0x4000),
            AccessSize::new(64).unwrap(),
            96,
            2,
            2,
            TrafficDramAddressMapping::RoRaBaCoCh,
            1,
            1,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramGeometryNotMultiple {
            field: "page_or_buffer_size",
            value: 96,
            factor: 64,
        }
    );

    assert_eq!(
        TrafficDramConfig::new(
            AgentId::new(7),
            CacheLineLayout::new(1 << 30).unwrap(),
            TrafficDramMode::Dram,
            Address::new(0),
            Address::new(1 << 30),
            AccessSize::new(1 << 30).unwrap(),
            1 << 30,
            1 << 31,
            1,
            TrafficDramAddressMapping::RoRaBaCoCh,
            1 << 31,
            1,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramAddressBitWidthTooLarge {
            block_bits: 30,
            page_bits: 0,
            bank_bits: 31,
            rank_bits: 31,
        }
    );

    assert_eq!(
        TrafficDramConfig::new(
            AgentId::new(7),
            CacheLineLayout::new(1).unwrap(),
            TrafficDramMode::DramRotate,
            Address::new(0),
            Address::new(1024),
            AccessSize::new(1).unwrap(),
            1,
            1 << 31,
            1 << 31,
            TrafficDramAddressMapping::RoRaBaCoCh,
            2,
            1,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramRotationCycleTooLarge {
            ranks: 2,
            max_seq_count_per_rank: 1 << 31,
        }
    );

    let wide_rotate = TrafficDramConfig::new(
        AgentId::new(7),
        CacheLineLayout::new(1).unwrap(),
        TrafficDramMode::DramRotate,
        Address::new(0),
        Address::new(1024),
        AccessSize::new(1).unwrap(),
        1,
        1 << 31,
        1 << 31,
        TrafficDramAddressMapping::RoRaBaCoCh,
        1,
        1,
    )
    .unwrap();
    assert_eq!(
        wide_rotate.with_read_percent(50).unwrap_err(),
        TrafficGeneratorError::TrafficDramRotationCycleTooLarge {
            ranks: 1,
            max_seq_count_per_rank: 1 << 32,
        }
    );

    assert_eq!(
        dram_config(TrafficDramMode::DramRotate)
            .with_read_percent(25)
            .unwrap_err(),
        TrafficGeneratorError::TrafficDramRotateUnsupportedReadPercent { read_percent: 25 }
    );

    let snapshot = TrafficDramSnapshot::new(
        dram_config(TrafficDramMode::Dram),
        u64::MAX,
        0,
        TrafficGeneratorSummary::default(),
        1,
        0,
        Address::new(0),
        TrafficRequestKind::Read,
        0,
    )
    .unwrap();
    let mut restored = DramTrafficGenerator::restore(snapshot).unwrap();
    assert_eq!(
        restored.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::CounterOverflow {
            counter: "next_sequence",
            value: u64::MAX,
            increment: 1,
        }
    );

    assert_eq!(
        TrafficDramSnapshot::new(
            dram_config(TrafficDramMode::Dram),
            0,
            0,
            TrafficGeneratorSummary::default(),
            1,
            3,
            Address::new(0),
            TrafficRequestKind::Read,
            0,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramSnapshotSeriesOutsideRange {
            series_remaining: 3,
            num_seq_packets: 2,
        }
    );

    assert_eq!(
        TrafficDramSnapshot::new(
            dram_config(TrafficDramMode::Dram),
            0,
            0,
            TrafficGeneratorSummary::default(),
            1,
            1,
            Address::new(0),
            TrafficRequestKind::Atomic,
            0,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramSnapshotUnsupportedKind {
            current_kind: TrafficRequestKind::Atomic,
        }
    );

    let rotating_config = dram_config(TrafficDramMode::DramRotate)
        .with_read_percent(50)
        .unwrap();
    assert_eq!(
        TrafficDramSnapshot::new(
            rotating_config,
            0,
            0,
            TrafficGeneratorSummary::default(),
            1,
            0,
            Address::new(0),
            TrafficRequestKind::Read,
            8,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficDramSnapshotRotateSequenceOutsideCycle {
            rotate_sequence_count: 8,
            cycle_size: 8,
        }
    );
}
