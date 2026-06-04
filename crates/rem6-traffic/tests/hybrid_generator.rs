use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    HybridTrafficGenerator, TrafficDramAddressMapping, TrafficGeneratorError,
    TrafficGeneratorSummary, TrafficHybridConfig, TrafficHybridSide, TrafficHybridSideConfig,
    TrafficHybridSnapshot, TrafficRequestKind,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn dram_side() -> TrafficHybridSideConfig {
    TrafficHybridSideConfig::new(
        Address::new(0),
        Address::new(0x4000),
        AccessSize::new(16).unwrap(),
        64,
        2,
        2,
        2,
        2,
    )
    .unwrap()
}

fn nvm_side() -> TrafficHybridSideConfig {
    TrafficHybridSideConfig::new(
        Address::new(0x10000),
        Address::new(0x14000),
        AccessSize::new(32).unwrap(),
        128,
        4,
        4,
        1,
        3,
    )
    .unwrap()
}

fn hybrid_config(mapping: TrafficDramAddressMapping) -> TrafficHybridConfig {
    TrafficHybridConfig::new(
        AgentId::new(7),
        line_layout(),
        dram_side(),
        nvm_side(),
        mapping,
    )
    .unwrap()
    .with_period(4, 4)
    .unwrap()
    .with_read_percent(100)
    .unwrap()
}

fn open_page_bank_rank_col(
    address: Address,
    block_bits: u32,
    page_bits: u32,
    bank_bits: u32,
    rank_bits: u32,
) -> (u64, u64, u64) {
    let raw = address.get();
    let col = (raw >> block_bits) & ((1 << page_bits) - 1);
    let bank = (raw >> (block_bits + page_bits)) & ((1 << bank_bits) - 1);
    let rank = if rank_bits == 0 {
        0
    } else {
        (raw >> (block_bits + page_bits + bank_bits)) & ((1 << rank_bits) - 1)
    };
    (bank, rank, col)
}

fn ro_co_bank_rank_col(
    address: Address,
    block_bits: u32,
    bank_bits: u32,
    rank_bits: u32,
    page_bits: u32,
) -> (u64, u64, u64) {
    let raw = address.get();
    let bank = (raw >> block_bits) & ((1 << bank_bits) - 1);
    let rank = if rank_bits == 0 {
        0
    } else {
        (raw >> (block_bits + bank_bits)) & ((1 << rank_bits) - 1)
    };
    let col = (raw >> (block_bits + bank_bits + rank_bits)) & ((1 << page_bits) - 1);
    (bank, rank, col)
}

#[test]
fn hybrid_generator_routes_forced_dram_and_nvm_series_with_distinct_geometry() {
    let mut dram_generator =
        HybridTrafficGenerator::new(hybrid_config(TrafficDramAddressMapping::RoRaBaCoCh));

    let first = dram_generator.next_request(0, 0).unwrap().unwrap();
    let second = dram_generator.next_request(4, 0).unwrap().unwrap();

    assert_eq!(first.tick(), 4);
    assert_eq!(first.sequence(), 0);
    assert_eq!(first.kind(), TrafficRequestKind::Read);
    assert_eq!(
        first.request().id(),
        MemoryRequestId::new(AgentId::new(7), 0)
    );
    assert_eq!(first.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(first.request().size(), AccessSize::new(16).unwrap());
    assert!(first.address().get() < 0x4000);
    assert_eq!(
        open_page_bank_rank_col(second.address(), 4, 2, 1, 1).2,
        open_page_bank_rank_col(first.address(), 4, 2, 1, 1).2 + 1
    );
    assert_eq!(dram_generator.summary().bytes_read(), 32);

    let nvm_config = hybrid_config(TrafficDramAddressMapping::RoRaBaCoCh)
        .with_nvm_percent(100)
        .unwrap()
        .with_read_percent(0)
        .unwrap();
    let mut nvm_generator = HybridTrafficGenerator::new(nvm_config);

    let first = nvm_generator.next_request(0, 0).unwrap().unwrap();
    let second = nvm_generator.next_request(4, 0).unwrap().unwrap();
    let third = nvm_generator.next_request(8, 0).unwrap().unwrap();

    assert_eq!(first.kind(), TrafficRequestKind::Write);
    assert_eq!(first.request().operation(), MemoryOperation::Write);
    assert_eq!(first.request().size(), AccessSize::new(32).unwrap());
    assert_eq!(first.request().data(), Some(&vec![7; 32][..]));
    assert!(first.address().get() >= 0x10000);
    assert!(second.address().get() >= 0x10000);
    assert!(third.address().get() >= 0x10000);
    assert_eq!(
        open_page_bank_rank_col(second.address(), 5, 2, 2, 0).2,
        open_page_bank_rank_col(first.address(), 5, 2, 2, 0).2 + 1
    );
    assert_eq!(
        open_page_bank_rank_col(third.address(), 5, 2, 2, 0).2,
        open_page_bank_rank_col(second.address(), 5, 2, 2, 0).2 + 1
    );
    assert_eq!(nvm_generator.summary().bytes_written(), 96);
}

#[test]
fn hybrid_generator_keeps_ro_co_series_on_selected_side_and_column() {
    let config = hybrid_config(TrafficDramAddressMapping::RoCoRaBaCh)
        .with_nvm_percent(100)
        .unwrap();
    let mut generator = HybridTrafficGenerator::new(config);

    let first = generator.next_request(0, 0).unwrap().unwrap();
    let second = generator.next_request(4, 0).unwrap().unwrap();
    let third = generator.next_request(8, 0).unwrap().unwrap();

    assert_eq!(
        ro_co_bank_rank_col(second.address(), 5, 2, 0, 2).0,
        ro_co_bank_rank_col(first.address(), 5, 2, 0, 2).0
    );
    assert_eq!(
        ro_co_bank_rank_col(third.address(), 5, 2, 0, 2).0,
        ro_co_bank_rank_col(first.address(), 5, 2, 0, 2).0
    );
    assert_eq!(
        ro_co_bank_rank_col(second.address(), 5, 2, 0, 2).2,
        ro_co_bank_rank_col(first.address(), 5, 2, 0, 2).2 + 1
    );
    assert_eq!(
        ro_co_bank_rank_col(third.address(), 5, 2, 0, 2).2,
        ro_co_bank_rank_col(second.address(), 5, 2, 0, 2).2 + 1
    );
}

#[test]
fn hybrid_generator_data_limit_and_non_elastic_timing_are_global() {
    let config = hybrid_config(TrafficDramAddressMapping::RoRaBaCoCh)
        .with_nvm_percent(100)
        .unwrap()
        .with_data_limit(33)
        .unwrap()
        .with_elastic_requests(false);
    let mut generator = HybridTrafficGenerator::new(config);

    assert_eq!(generator.schedule_tick(100, 3).unwrap(), 101);
    assert_eq!(generator.schedule_tick(100, 9).unwrap(), 100);
    assert!(generator.next_request(0, 0).unwrap().is_some());
    assert!(generator.next_request(4, 0).unwrap().is_some());
    assert_eq!(generator.next_request(8, 0).unwrap(), None);
    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 64);
}

#[test]
fn hybrid_generator_snapshot_restores_active_side_series_and_rng_state() {
    let config = hybrid_config(TrafficDramAddressMapping::RoRaBaCoCh)
        .with_nvm_percent(100)
        .unwrap();
    let mut generator = HybridTrafficGenerator::new(config);
    let first = generator.next_request(0, 0).unwrap().unwrap();
    let snapshot = generator.snapshot();

    assert_eq!(snapshot.current_side(), TrafficHybridSide::Nvm);
    assert_eq!(snapshot.series_remaining(), 2);

    let mut restored = HybridTrafficGenerator::restore(snapshot).unwrap();
    let next = restored.next_request(4, 0).unwrap().unwrap();

    assert_eq!(next.sequence(), 1);
    assert_eq!(
        open_page_bank_rank_col(next.address(), 5, 2, 2, 0).0,
        open_page_bank_rank_col(first.address(), 5, 2, 2, 0).0
    );
    assert_eq!(
        open_page_bank_rank_col(next.address(), 5, 2, 2, 0).2,
        open_page_bank_rank_col(first.address(), 5, 2, 2, 0).2 + 1
    );
    assert_eq!(restored.summary().packet_count(), 2);
}

#[test]
fn hybrid_generator_rejects_invalid_config_and_snapshots() {
    assert_eq!(
        hybrid_config(TrafficDramAddressMapping::RoRaBaCoCh)
            .with_nvm_percent(101)
            .unwrap_err(),
        TrafficGeneratorError::TrafficHybridInvalidNvmPercent { nvm_percent: 101 }
    );

    assert_eq!(
        TrafficHybridSideConfig::new(
            Address::new(0),
            Address::new(0x4000),
            AccessSize::new(16).unwrap(),
            64,
            2,
            3,
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
        TrafficHybridSnapshot::new(
            hybrid_config(TrafficDramAddressMapping::RoRaBaCoCh),
            0,
            0,
            TrafficGeneratorSummary::default(),
            1,
            3,
            Address::new(0),
            TrafficRequestKind::Read,
            TrafficHybridSide::Dram,
        )
        .unwrap_err(),
        TrafficGeneratorError::TrafficHybridSnapshotSeriesOutsideRange {
            side: TrafficHybridSide::Dram,
            series_remaining: 3,
            num_seq_packets: 2,
        }
    );
}
