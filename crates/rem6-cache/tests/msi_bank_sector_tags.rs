use rem6_cache::{
    CacheControllerResultKind, CacheReplacementDirectoryConfig, CacheReplacementPolicyKind,
    CacheSectorTagsConfig, MsiCacheBank, MsiCacheBankError,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_msi::MsiState;
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

fn lru_replacement_config(sets: usize, ways: usize) -> CacheReplacementDirectoryConfig {
    CacheReplacementDirectoryConfig::new(CacheReplacementPolicyKind::Lru, layout(), sets, ways)
        .unwrap()
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

fn fill_read_line(bank: &mut MsiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill(fill(miss.downstream_request().unwrap(), sequence as u8))
        .unwrap();
}

#[test]
fn msi_cache_bank_sector_tags_keep_same_sector_subblocks_resident() {
    let cache_agent = agent(7);
    let mut bank =
        MsiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 4))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 170, 0x4004);
    fill_read_line(&mut bank, cache_agent, 171, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 172, 0x4004))
        .unwrap();
    assert_eq!(first_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[170; 8]
    );

    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 173, 0x4014))
        .unwrap();
    assert_eq!(second_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[171; 8]
    );
}

#[test]
fn msi_cache_bank_sector_tags_evict_whole_clean_sector() {
    let cache_agent = agent(7);
    let mut bank =
        MsiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 174, 0x5004);
    fill_read_line(&mut bank, cache_agent, 175, 0x5014);
    fill_read_line(&mut bank, cache_agent, 176, 0x5024);

    assert_eq!(bank.state(Address::new(0x5000)), None);
    assert_eq!(bank.state(Address::new(0x5010)), None);
    assert_eq!(bank.state(Address::new(0x5020)), Some(MsiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0x5020)]);
    assert_eq!(bank.replacement_way_for(Address::new(0x5000)), None);
    assert_eq!(bank.replacement_way_for(Address::new(0x5020)), Some((0, 0)));
}

#[test]
fn msi_cache_bank_sector_tags_reject_modified_sector_eviction_without_write_queue() {
    let cache_agent = agent(7);
    let mut bank =
        MsiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    let store = write(cache_agent, 177, 0x6004, vec![0xde, 0xad]);
    let store_miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(fill(store_miss.downstream_request().unwrap(), 0x00))
        .unwrap();
    fill_read_line(&mut bank, cache_agent, 178, 0x6014);

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 179, 0x6024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(fill(read_miss.downstream_request().unwrap(), 0x33)),
        Err(MsiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x6000),
        })
    );
    assert_eq!(bank.state(Address::new(0x6000)), Some(MsiState::Modified));
    assert_eq!(bank.state(Address::new(0x6010)), Some(MsiState::Shared));
    assert_eq!(
        bank.state(Address::new(0x6020)),
        Some(MsiState::InvalidToShared)
    );
    assert_eq!(
        bank.line_addresses(),
        vec![
            Address::new(0x6000),
            Address::new(0x6010),
            Address::new(0x6020)
        ]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x6000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x6020)), None);
}

#[test]
fn msi_cache_bank_sector_tags_reject_transient_sector_eviction() {
    let cache_agent = agent(7);
    let mut bank =
        MsiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 180, 0x7004);
    fill_read_line(&mut bank, cache_agent, 181, 0x7014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 182, 0x7004, vec![0x44]))
        .unwrap();
    let upgrade_downstream = upgrade.downstream_request().unwrap().clone();
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(MsiState::SharedToModified)
    );

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 183, 0x7024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(fill(read_miss.downstream_request().unwrap(), 0x55)),
        Err(MsiCacheBankError::TransientReplacementRequiresStableLine {
            line: Address::new(0x7000),
        })
    );
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(MsiState::SharedToModified)
    );
    assert_eq!(bank.state(Address::new(0x7010)), Some(MsiState::Shared));
    assert_eq!(
        bank.state(Address::new(0x7020)),
        Some(MsiState::InvalidToShared)
    );
    assert_eq!(
        bank.pending_fill_line(upgrade_downstream.id()),
        Some(Address::new(0x7000))
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x7000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x7020)), None);
}

#[test]
fn msi_cache_bank_sector_tags_snapshot_restore_preserves_sector_state() {
    let cache_agent = agent(7);
    let config = lru_sector_tags_config(1, 1, 2);
    let mut bank = MsiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 184, 0x8004);
    fill_read_line(&mut bank, cache_agent, 185, 0x8014);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 186, 0x8024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0x8020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x8000), Address::new(0x8010)]
    );
    assert!(bank.snapshot().sector_tags().is_some());

    let mut incompatible = MsiCacheBank::new(cache_agent, layout());
    assert_eq!(
        incompatible.restore(&snapshot),
        Err(MsiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: true,
            bank_has_sector_tags: false,
        })
    );
}

#[test]
fn msi_cache_bank_sector_tags_snapshot_restore_preserves_lru_state() {
    let cache_agent = agent(7);
    let config = lru_sector_tags_config(1, 2, 2);
    let mut bank = MsiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 187, 0x9004);
    fill_read_line(&mut bank, cache_agent, 188, 0x9024);
    let hit = bank
        .accept_cpu_request(read(cache_agent, 189, 0x9004))
        .unwrap();
    assert_eq!(hit.kind(), CacheControllerResultKind::Hit);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 190, 0x9044);
    assert_eq!(bank.state(Address::new(0x9000)), Some(MsiState::Shared));
    assert_eq!(bank.state(Address::new(0x9020)), None);
    assert_eq!(bank.state(Address::new(0x9040)), Some(MsiState::Shared));

    bank.restore(&snapshot).unwrap();
    fill_read_line(&mut bank, cache_agent, 191, 0x9044);
    assert_eq!(bank.state(Address::new(0x9000)), Some(MsiState::Shared));
    assert_eq!(bank.state(Address::new(0x9020)), None);
    assert_eq!(bank.state(Address::new(0x9040)), Some(MsiState::Shared));
}

#[test]
fn msi_cache_bank_sector_tags_restore_rejects_tag_backend_mismatches() {
    let cache_agent = agent(7);
    let sector_config = lru_sector_tags_config(1, 1, 2);
    let mut sector_bank =
        MsiCacheBank::new_with_sector_tags(cache_agent, layout(), sector_config).unwrap();

    let plain_snapshot = MsiCacheBank::new(cache_agent, layout()).snapshot();
    assert_eq!(
        sector_bank.restore(&plain_snapshot),
        Err(MsiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: false,
            bank_has_sector_tags: true,
        })
    );

    let directory_snapshot = MsiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap()
    .snapshot();
    assert_eq!(
        sector_bank.restore(&directory_snapshot),
        Err(
            MsiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: true,
                bank_has_replacement_directory: false,
            }
        )
    );

    fill_read_line(&mut sector_bank, cache_agent, 192, 0xa004);
    let sector_snapshot = sector_bank.snapshot();
    let mut directory_bank = MsiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap();
    assert_eq!(
        directory_bank.restore(&sector_snapshot),
        Err(
            MsiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: false,
                bank_has_replacement_directory: true,
            }
        )
    );
}
