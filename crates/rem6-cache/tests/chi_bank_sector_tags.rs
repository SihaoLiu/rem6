use rem6_cache::{
    CacheReplacementDirectoryConfig, CacheReplacementPolicyKind, CacheSectorTagsConfig,
    ChiCacheBank, ChiCacheBankError,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_chi::{ChiEvent, ChiState};
use rem6_transport::TargetOutcome;

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn read(agent_id: AgentId, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn write(agent_id: AgentId, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let access_size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        access_size,
        data,
        ByteMask::full(access_size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn fill(request: &MemoryRequest, byte: u8) -> MemoryResponse {
    MemoryResponse::completed(request, Some(vec![byte; layout().bytes() as usize])).unwrap()
}

fn response_data(outcome: &TargetOutcome) -> &[u8] {
    match outcome {
        TargetOutcome::Respond(response) => response.data().unwrap(),
        other => panic!("expected immediate response, got {other:?}"),
    }
}

fn lru_sector_tags_config(
    sets: usize,
    ways: usize,
    blocks_per_sector: usize,
) -> CacheSectorTagsConfig {
    CacheSectorTagsConfig::new(
        CacheReplacementPolicyKind::Lru,
        layout(),
        sets,
        ways,
        blocks_per_sector,
    )
    .unwrap()
}

fn lru_replacement_config(sets: usize, ways: usize) -> CacheReplacementDirectoryConfig {
    CacheReplacementDirectoryConfig::new(CacheReplacementPolicyKind::Lru, layout(), sets, ways)
        .unwrap()
}

fn fill_read_line(bank: &mut ChiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        ChiEvent::CompDataSharedClean,
    )
    .unwrap();
}

fn fill_dirty_line(bank: &mut ChiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    let store = write(cache_agent, sequence, address, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        ChiEvent::CompDataUniqueDirty,
    )
    .unwrap();
}

#[test]
fn chi_cache_bank_sector_tags_keep_same_sector_subblocks_resident() {
    let cache_agent = agent(40);
    let mut bank =
        ChiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 4))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 240, 0x4004);
    fill_read_line(&mut bank, cache_agent, 241, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 242, 0x4004))
        .unwrap();
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[240; 8]
    );
    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 243, 0x4014))
        .unwrap();
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[241; 8]
    );
}

#[test]
fn chi_cache_bank_sector_tags_evict_whole_clean_sector() {
    let cache_agent = agent(40);
    let mut bank =
        ChiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 244, 0x5004);
    fill_read_line(&mut bank, cache_agent, 245, 0x5014);
    fill_read_line(&mut bank, cache_agent, 246, 0x5024);

    assert_eq!(bank.state(Address::new(0x5000)), None);
    assert_eq!(bank.state(Address::new(0x5010)), None);
    assert_eq!(
        bank.state(Address::new(0x5020)),
        Some(ChiState::SharedClean)
    );
    assert_eq!(bank.line_addresses(), vec![Address::new(0x5020)]);
    assert_eq!(bank.replacement_way_for(Address::new(0x5000)), None);
    assert_eq!(bank.replacement_way_for(Address::new(0x5020)), Some((0, 0)));
}

#[test]
fn chi_cache_bank_sector_tags_reject_dirty_sector_eviction_without_write_queue() {
    let cache_agent = agent(40);
    let mut bank =
        ChiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_dirty_line(&mut bank, cache_agent, 247, 0x6004);
    fill_read_line(&mut bank, cache_agent, 248, 0x6014);

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 249, 0x6024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(
            fill(read_miss.downstream_request().unwrap(), 0x33),
            ChiEvent::CompDataSharedClean,
        ),
        Err(ChiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x6000),
        })
    );
    assert_eq!(
        bank.state(Address::new(0x6000)),
        Some(ChiState::UniqueDirty)
    );
    assert_eq!(
        bank.state(Address::new(0x6020)),
        Some(ChiState::InvalidToSharedClean)
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x6000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x6020)), None);
}

#[test]
fn chi_cache_bank_sector_tags_reject_transient_sector_eviction() {
    let cache_agent = agent(40);
    let mut bank =
        ChiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 250, 0x7004);
    fill_read_line(&mut bank, cache_agent, 251, 0x7014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 252, 0x7004, vec![0x44]))
        .unwrap();
    let upgrade_downstream = upgrade.downstream_request().unwrap().clone();
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(ChiState::SharedCleanToUniqueClean)
    );

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 253, 0x7024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(
            fill(read_miss.downstream_request().unwrap(), 0x55),
            ChiEvent::CompDataSharedClean,
        ),
        Err(ChiCacheBankError::TransientReplacementRequiresStableLine {
            line: Address::new(0x7000),
        })
    );
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(ChiState::SharedCleanToUniqueClean)
    );
    assert_eq!(
        bank.pending_fill_line(upgrade_downstream.id()),
        Some(Address::new(0x7000))
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x7000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x7020)), None);
}

#[test]
fn chi_cache_bank_sector_tags_snapshot_restore_preserves_sector_state() {
    let cache_agent = agent(40);
    let config = lru_sector_tags_config(1, 1, 2);
    let mut bank = ChiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 254, 0x8004);
    fill_read_line(&mut bank, cache_agent, 255, 0x8014);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 256, 0x8024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0x8020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x8000), Address::new(0x8010)]
    );
    assert!(bank.snapshot().sector_tags().is_some());

    let mut plain_bank = ChiCacheBank::new(cache_agent, layout());
    assert_eq!(
        plain_bank.restore(&snapshot),
        Err(ChiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: true,
            bank_has_sector_tags: false,
        })
    );
}

#[test]
fn chi_cache_bank_sector_tags_snapshot_restore_preserves_lru_state() {
    let cache_agent = agent(40);
    let config = lru_sector_tags_config(1, 2, 2);
    let mut bank = ChiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 257, 0x9004);
    fill_read_line(&mut bank, cache_agent, 258, 0x9024);
    bank.accept_cpu_request(read(cache_agent, 259, 0x9004))
        .unwrap();
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 260, 0x9044);
    assert_eq!(
        bank.state(Address::new(0x9000)),
        Some(ChiState::SharedClean)
    );
    assert_eq!(bank.state(Address::new(0x9020)), None);
    assert_eq!(
        bank.state(Address::new(0x9040)),
        Some(ChiState::SharedClean)
    );

    bank.restore(&snapshot).unwrap();
    fill_read_line(&mut bank, cache_agent, 261, 0x9044);
    assert_eq!(
        bank.state(Address::new(0x9000)),
        Some(ChiState::SharedClean)
    );
    assert_eq!(bank.state(Address::new(0x9020)), None);
    assert_eq!(
        bank.state(Address::new(0x9040)),
        Some(ChiState::SharedClean)
    );
}

#[test]
fn chi_cache_bank_sector_tags_restore_rejects_tag_backend_mismatches() {
    let cache_agent = agent(40);
    let sector_config = lru_sector_tags_config(1, 1, 2);
    let mut sector_bank =
        ChiCacheBank::new_with_sector_tags(cache_agent, layout(), sector_config).unwrap();

    let plain_snapshot = ChiCacheBank::new(cache_agent, layout()).snapshot();
    assert_eq!(
        sector_bank.restore(&plain_snapshot),
        Err(ChiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: false,
            bank_has_sector_tags: true,
        })
    );

    let directory_snapshot = ChiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap()
    .snapshot();
    assert_eq!(
        sector_bank.restore(&directory_snapshot),
        Err(
            ChiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: true,
                bank_has_replacement_directory: false,
            }
        )
    );

    fill_read_line(&mut sector_bank, cache_agent, 262, 0xa004);
    let sector_snapshot = sector_bank.snapshot();
    let mut directory_bank = ChiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap();
    assert_eq!(
        directory_bank.restore(&sector_snapshot),
        Err(
            ChiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: false,
                bank_has_replacement_directory: true,
            }
        )
    );
}
