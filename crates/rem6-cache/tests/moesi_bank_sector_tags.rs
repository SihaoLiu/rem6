use rem6_cache::{
    CacheReplacementDirectoryConfig, CacheReplacementPolicyKind, CacheSectorTagsConfig,
    MoesiCacheBank, MoesiCacheBankError,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_moesi::{MoesiEvent, MoesiState};
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

fn fill_read_line(bank: &mut MoesiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        MoesiEvent::DataShared,
    )
    .unwrap();
}

fn fill_modified_line(
    bank: &mut MoesiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let store = write(cache_agent, sequence, address, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        MoesiEvent::DataModified,
    )
    .unwrap();
}

#[test]
fn moesi_cache_bank_sector_tags_keep_same_sector_subblocks_resident() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_sector_tags(
        cache_agent,
        layout(),
        lru_sector_tags_config(1, 1, 4),
    )
    .unwrap();

    fill_read_line(&mut bank, cache_agent, 220, 0x4004);
    fill_read_line(&mut bank, cache_agent, 221, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 222, 0x4004))
        .unwrap();
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[220; 8]
    );
    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 223, 0x4014))
        .unwrap();
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[221; 8]
    );
}

#[test]
fn moesi_cache_bank_sector_tags_evict_whole_clean_sector() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_sector_tags(
        cache_agent,
        layout(),
        lru_sector_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_line(&mut bank, cache_agent, 224, 0x5004);
    fill_read_line(&mut bank, cache_agent, 225, 0x5014);
    fill_read_line(&mut bank, cache_agent, 226, 0x5024);

    assert_eq!(bank.state(Address::new(0x5000)), None);
    assert_eq!(bank.state(Address::new(0x5010)), None);
    assert_eq!(bank.state(Address::new(0x5020)), Some(MoesiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0x5020)]);
    assert_eq!(bank.replacement_way_for(Address::new(0x5000)), None);
    assert_eq!(bank.replacement_way_for(Address::new(0x5020)), Some((0, 0)));
}

#[test]
fn moesi_cache_bank_sector_tags_reject_dirty_owner_sector_eviction_without_write_queue() {
    let cache_agent = agent(30);
    let mut modified_bank = MoesiCacheBank::new_with_sector_tags(
        cache_agent,
        layout(),
        lru_sector_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_modified_line(&mut modified_bank, cache_agent, 227, 0x6004);
    fill_read_line(&mut modified_bank, cache_agent, 228, 0x6014);

    let modified_miss = modified_bank
        .accept_cpu_request(read(cache_agent, 229, 0x6024))
        .unwrap();
    assert_eq!(
        modified_bank.accept_fill(
            fill(modified_miss.downstream_request().unwrap(), 0x33),
            MoesiEvent::DataShared,
        ),
        Err(MoesiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x6000),
        })
    );
    assert_eq!(
        modified_bank.state(Address::new(0x6000)),
        Some(MoesiState::Modified)
    );
    assert_eq!(
        modified_bank.state(Address::new(0x6010)),
        Some(MoesiState::Shared)
    );
    assert_eq!(
        modified_bank.state(Address::new(0x6020)),
        Some(MoesiState::InvalidToExclusive)
    );
    assert_eq!(
        modified_bank.line_addresses(),
        vec![
            Address::new(0x6000),
            Address::new(0x6010),
            Address::new(0x6020)
        ]
    );
    assert_eq!(
        modified_bank.replacement_way_for(Address::new(0x6000)),
        Some((0, 0))
    );
    assert_eq!(
        modified_bank.replacement_way_for(Address::new(0x6020)),
        None
    );

    let mut owned_bank = MoesiCacheBank::new_with_sector_tags(
        cache_agent,
        layout(),
        lru_sector_tags_config(1, 1, 2),
    )
    .unwrap();
    fill_modified_line(&mut owned_bank, cache_agent, 230, 0x7004);
    owned_bank
        .accept_snoop(Address::new(0x7004), MoesiEvent::SnoopRead)
        .unwrap();
    assert_eq!(
        owned_bank.state(Address::new(0x7000)),
        Some(MoesiState::Owned)
    );
    fill_read_line(&mut owned_bank, cache_agent, 231, 0x7014);

    let owned_miss = owned_bank
        .accept_cpu_request(read(cache_agent, 232, 0x7024))
        .unwrap();
    assert_eq!(
        owned_bank.accept_fill(
            fill(owned_miss.downstream_request().unwrap(), 0x44),
            MoesiEvent::DataShared,
        ),
        Err(MoesiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x7000),
        })
    );
    assert_eq!(
        owned_bank.state(Address::new(0x7000)),
        Some(MoesiState::Owned)
    );
    assert_eq!(
        owned_bank.state(Address::new(0x7010)),
        Some(MoesiState::Shared)
    );
    assert_eq!(
        owned_bank.state(Address::new(0x7020)),
        Some(MoesiState::InvalidToExclusive)
    );
    assert_eq!(
        owned_bank.line_addresses(),
        vec![
            Address::new(0x7000),
            Address::new(0x7010),
            Address::new(0x7020)
        ]
    );
    assert_eq!(
        owned_bank.replacement_way_for(Address::new(0x7000)),
        Some((0, 0))
    );
    assert_eq!(owned_bank.replacement_way_for(Address::new(0x7020)), None);
}

#[test]
fn moesi_cache_bank_sector_tags_reject_transient_sector_eviction() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_sector_tags(
        cache_agent,
        layout(),
        lru_sector_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_line(&mut bank, cache_agent, 233, 0x8004);
    fill_read_line(&mut bank, cache_agent, 234, 0x8014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 235, 0x8004, vec![0x44]))
        .unwrap();
    let upgrade_downstream = upgrade.downstream_request().unwrap().clone();
    assert_eq!(
        bank.state(Address::new(0x8000)),
        Some(MoesiState::SharedToModified)
    );

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 236, 0x8024))
        .unwrap();
    assert_eq!(
        bank.accept_fill(
            fill(read_miss.downstream_request().unwrap(), 0x55),
            MoesiEvent::DataShared,
        ),
        Err(
            MoesiCacheBankError::TransientReplacementRequiresStableLine {
                line: Address::new(0x8000),
            }
        )
    );
    assert_eq!(
        bank.state(Address::new(0x8000)),
        Some(MoesiState::SharedToModified)
    );
    assert_eq!(bank.state(Address::new(0x8010)), Some(MoesiState::Shared));
    assert_eq!(
        bank.state(Address::new(0x8020)),
        Some(MoesiState::InvalidToExclusive)
    );
    assert_eq!(
        bank.pending_fill_line(upgrade_downstream.id()),
        Some(Address::new(0x8000))
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x8000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x8020)), None);
}

#[test]
fn moesi_cache_bank_sector_tags_snapshot_restore_preserves_sector_state() {
    let cache_agent = agent(30);
    let config = lru_sector_tags_config(1, 1, 2);
    let mut bank = MoesiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 237, 0x9004);
    fill_read_line(&mut bank, cache_agent, 238, 0x9014);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 239, 0x9024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0x9020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x9000), Address::new(0x9010)]
    );
    assert!(bank.snapshot().sector_tags().is_some());

    let mut plain_bank = MoesiCacheBank::new(cache_agent, layout());
    assert_eq!(
        plain_bank.restore(&snapshot),
        Err(MoesiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: true,
            bank_has_sector_tags: false,
        })
    );
}

#[test]
fn moesi_cache_bank_sector_tags_snapshot_restore_preserves_lru_state() {
    let cache_agent = agent(30);
    let config = lru_sector_tags_config(1, 2, 2);
    let mut bank = MoesiCacheBank::new_with_sector_tags(cache_agent, layout(), config).unwrap();

    fill_read_line(&mut bank, cache_agent, 240, 0xa004);
    fill_read_line(&mut bank, cache_agent, 241, 0xa024);
    let hit = bank
        .accept_cpu_request(read(cache_agent, 242, 0xa004))
        .unwrap();
    assert_eq!(response_data(hit.target_outcome().unwrap()), &[240; 8]);
    let snapshot = bank.snapshot();

    fill_read_line(&mut bank, cache_agent, 243, 0xa044);
    assert_eq!(bank.state(Address::new(0xa000)), Some(MoesiState::Shared));
    assert_eq!(bank.state(Address::new(0xa020)), None);
    assert_eq!(bank.state(Address::new(0xa040)), Some(MoesiState::Shared));

    bank.restore(&snapshot).unwrap();
    fill_read_line(&mut bank, cache_agent, 244, 0xa044);
    assert_eq!(bank.state(Address::new(0xa000)), Some(MoesiState::Shared));
    assert_eq!(bank.state(Address::new(0xa020)), None);
    assert_eq!(bank.state(Address::new(0xa040)), Some(MoesiState::Shared));
}

#[test]
fn moesi_cache_bank_sector_tags_restore_rejects_tag_backend_mismatches() {
    let cache_agent = agent(30);
    let sector_config = lru_sector_tags_config(1, 1, 2);
    let mut sector_bank =
        MoesiCacheBank::new_with_sector_tags(cache_agent, layout(), sector_config).unwrap();

    let plain_snapshot = MoesiCacheBank::new(cache_agent, layout()).snapshot();
    assert_eq!(
        sector_bank.restore(&plain_snapshot),
        Err(MoesiCacheBankError::SnapshotSectorTagsModeMismatch {
            snapshot_has_sector_tags: false,
            bank_has_sector_tags: true,
        })
    );

    let directory_snapshot = MoesiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap()
    .snapshot();
    assert_eq!(
        sector_bank.restore(&directory_snapshot),
        Err(
            MoesiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: true,
                bank_has_replacement_directory: false,
            }
        )
    );

    fill_read_line(&mut sector_bank, cache_agent, 245, 0xb004);
    let sector_snapshot = sector_bank.snapshot();
    let mut directory_bank = MoesiCacheBank::new_with_replacement_directory(
        cache_agent,
        layout(),
        lru_replacement_config(1, 1),
    )
    .unwrap();
    assert_eq!(
        directory_bank.restore(&sector_snapshot),
        Err(
            MoesiCacheBankError::SnapshotReplacementDirectoryModeMismatch {
                snapshot_has_replacement_directory: false,
                bank_has_replacement_directory: true,
            }
        )
    );
}
