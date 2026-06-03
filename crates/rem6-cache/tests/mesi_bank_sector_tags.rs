use rem6_cache::{
    CacheReplacementDirectoryConfig, CacheReplacementPolicyKind, CacheSectorTagsConfig,
    MesiCacheBank, MesiCacheBankError, MesiCacheControllerResultKind,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_mesi::{MesiEvent, MesiState};
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

fn fill_read_line_with_event(
    bank: &mut MesiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
    event: MesiEvent,
) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        event,
    )
    .unwrap();
}

fn fill_read_line(bank: &mut MesiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    fill_read_line_with_event(bank, cache_agent, sequence, address, MesiEvent::DataShared);
}

#[test]
fn mesi_cache_bank_sector_tags_keep_same_sector_subblocks_resident() {
    let cache_agent = agent(20);
    let mut bank =
        MesiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 4))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 200, 0x4004);
    fill_read_line(&mut bank, cache_agent, 201, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 202, 0x4004))
        .unwrap();
    assert_eq!(first_hit.kind(), MesiCacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[200; 8]
    );
    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 203, 0x4014))
        .unwrap();
    assert_eq!(second_hit.kind(), MesiCacheControllerResultKind::Hit);
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[201; 8]
    );
}

#[test]
fn mesi_cache_bank_sector_tags_evict_whole_clean_sector() {
    let cache_agent = agent(20);
    let mut bank =
        MesiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_read_line_with_event(
        &mut bank,
        cache_agent,
        204,
        0x5004,
        MesiEvent::DataExclusive,
    );
    fill_read_line(&mut bank, cache_agent, 205, 0x5014);
    fill_read_line(&mut bank, cache_agent, 206, 0x5024);

    assert_eq!(bank.state(Address::new(0x5000)), None);
    assert_eq!(bank.state(Address::new(0x5010)), None);
    assert_eq!(bank.state(Address::new(0x5020)), Some(MesiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0x5020)]);
    assert_eq!(bank.replacement_way_for(Address::new(0x5000)), None);
    assert_eq!(bank.replacement_way_for(Address::new(0x5020)), Some((0, 0)));
}

#[test]
fn mesi_cache_bank_sector_tags_reject_modified_sector_eviction_without_write_queue() {
    let cache_agent = agent(20);
    let mut bank =
        MesiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    let store = write(cache_agent, 207, 0x6004, vec![0xde, 0xad]);
    let store_miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(store_miss.downstream_request().unwrap(), 0x00),
        MesiEvent::DataModified,
    )
    .unwrap();
    fill_read_line(&mut bank, cache_agent, 208, 0x6014);

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 209, 0x6024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(
            fill(read_miss.downstream_request().unwrap(), 0x33),
            MesiEvent::DataShared,
        ),
        Err(MesiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x6000),
        })
    );
    assert_eq!(bank.state(Address::new(0x6000)), Some(MesiState::Modified));
    assert_eq!(bank.state(Address::new(0x6010)), Some(MesiState::Shared));
    assert_eq!(
        bank.state(Address::new(0x6020)),
        Some(MesiState::InvalidToExclusive)
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
fn mesi_cache_bank_sector_tags_reject_transient_sector_eviction() {
    let cache_agent = agent(20);
    let mut bank =
        MesiCacheBank::new_with_sector_tags(cache_agent, layout(), lru_sector_tags_config(1, 1, 2))
            .unwrap();

    fill_read_line(&mut bank, cache_agent, 210, 0x7004);
    fill_read_line(&mut bank, cache_agent, 211, 0x7014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 212, 0x7004, vec![0x44]))
        .unwrap();
    let upgrade_downstream = upgrade.downstream_request().unwrap().clone();
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(MesiState::SharedToModified)
    );

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 213, 0x7024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(
            fill(read_miss.downstream_request().unwrap(), 0x55),
            MesiEvent::DataShared,
        ),
        Err(MesiCacheBankError::TransientReplacementRequiresStableLine {
            line: Address::new(0x7000),
        })
    );
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(MesiState::SharedToModified)
    );
    assert_eq!(bank.state(Address::new(0x7010)), Some(MesiState::Shared));
    assert_eq!(
        bank.state(Address::new(0x7020)),
        Some(MesiState::InvalidToExclusive)
    );
    assert_eq!(
        bank.pending_fill_line(upgrade_downstream.id()),
        Some(Address::new(0x7000))
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x7000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x7020)), None);
}

#[test]
fn mesi_cache_bank_sector_tags_snapshot_restore_preserves_sector_state() {
    let cache_agent = agent(20);
    let config = lru_sector_tags_config(1, 1, 2);
    let mut bank = MesiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 214, 0x8004);
    fill_read_line(&mut bank, cache_agent, 215, 0x8014);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 216, 0x8024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0x8020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x8000), Address::new(0x8010)]
    );
    assert!(bank.snapshot().sector_tags().is_some());

    let mut plain_bank = MesiCacheBank::new(cache_agent, layout());
    assert_eq!(
        plain_bank.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: true,
            bank_has_sector_tags: false,
        })
    );
}

#[test]
fn mesi_cache_bank_sector_tags_snapshot_restore_preserves_lru_state() {
    let cache_agent = agent(20);
    let config = lru_sector_tags_config(1, 2, 2);
    let mut bank = MesiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 217, 0x9004);
    fill_read_line(&mut bank, cache_agent, 218, 0x9024);
    let hit = bank
        .accept_cpu_request(read(cache_agent, 219, 0x9004))
        .unwrap();
    assert_eq!(hit.kind(), MesiCacheControllerResultKind::Hit);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 220, 0x9044);
    assert_eq!(bank.state(Address::new(0x9000)), Some(MesiState::Shared));
    assert_eq!(bank.state(Address::new(0x9020)), None);
    assert_eq!(bank.state(Address::new(0x9040)), Some(MesiState::Shared));

    bank.restore(&snapshot).unwrap();
    fill_read_line(&mut bank, cache_agent, 221, 0x9044);
    assert_eq!(bank.state(Address::new(0x9000)), Some(MesiState::Shared));
    assert_eq!(bank.state(Address::new(0x9020)), None);
    assert_eq!(bank.state(Address::new(0x9040)), Some(MesiState::Shared));
}

#[test]
fn mesi_cache_bank_sector_tags_restore_rejects_tag_backend_mismatches() {
    let cache_agent = agent(20);
    let sector_config = lru_sector_tags_config(1, 1, 2);
    let mut sector_bank =
        MesiCacheBank::new_with_sector_tags(cache_agent, layout(), sector_config).unwrap();

    let plain_snapshot = MesiCacheBank::new(cache_agent, layout()).snapshot();
    assert_eq!(
        sector_bank.restore(&plain_snapshot),
        Err(MesiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: false,
            bank_has_sector_tags: true,
        })
    );

    let directory_snapshot = MesiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap()
    .snapshot();
    assert_eq!(
        sector_bank.restore(&directory_snapshot),
        Err(
            MesiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: true,
                bank_has_replacement_directory: false,
            }
        )
    );

    fill_read_line(&mut sector_bank, cache_agent, 222, 0xa004);
    let sector_snapshot = sector_bank.snapshot();
    let mut directory_bank = MesiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap();
    assert_eq!(
        directory_bank.restore(&sector_snapshot),
        Err(
            MesiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: false,
                bank_has_replacement_directory: true,
            }
        )
    );
}
