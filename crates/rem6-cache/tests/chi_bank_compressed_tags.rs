use rem6_cache::{
    CacheCompressedTagsConfig, CacheReplacementPolicyKind, ChiCacheBank, ChiCacheBankError,
    ChiCacheControllerResultKind,
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

fn half_line_bits() -> usize {
    (layout().bytes() as usize * 8) / 2
}

fn response_data(outcome: &TargetOutcome) -> &[u8] {
    match outcome {
        TargetOutcome::Respond(response) => response.data().unwrap(),
        other => panic!("expected immediate response, got {other:?}"),
    }
}

fn lru_compressed_tags_config(
    sets: usize,
    ways: usize,
    max_compression_ratio: usize,
) -> CacheCompressedTagsConfig {
    CacheCompressedTagsConfig::new(
        CacheReplacementPolicyKind::Lru,
        layout(),
        sets,
        ways,
        max_compression_ratio,
    )
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

fn fill_read_compressed_line(
    bank: &mut ChiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        ChiEvent::CompDataSharedClean,
        half_line_bits(),
    )
    .unwrap();
}

fn fill_unique_dirty_compressed_line(
    bank: &mut ChiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let request = write(cache_agent, sequence, address, vec![sequence as u8]);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(miss.downstream_request().unwrap(), 0x00),
        ChiEvent::CompDataUniqueDirty,
        half_line_bits(),
    )
    .unwrap();
}

fn fill_shared_dirty_compressed_line(
    bank: &mut ChiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        ChiEvent::CompDataSharedDirty,
        half_line_bits(),
    )
    .unwrap();
}

#[test]
fn chi_cache_bank_compressed_tags_keep_same_superblock_lines_resident() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 280, 0x4004);
    fill_read_compressed_line(&mut bank, cache_agent, 281, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));
    assert!(bank.snapshot().compressed_tags().is_some());

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 282, 0x4004))
        .unwrap();
    assert_eq!(first_hit.kind(), ChiCacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[280_u16 as u8; 8]
    );

    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 283, 0x4014))
        .unwrap();
    assert_eq!(second_hit.kind(), ChiCacheControllerResultKind::Hit);
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[281_u16 as u8; 8]
    );
}

#[test]
fn chi_cache_bank_compressed_tags_evict_whole_clean_superblock() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 284, 0x5004);
    fill_read_compressed_line(&mut bank, cache_agent, 285, 0x5014);
    fill_read_compressed_line(&mut bank, cache_agent, 286, 0x5024);

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
fn chi_cache_bank_compressed_tags_reject_dirty_superblock_eviction_without_write_queue() {
    let cache_agent = agent(40);
    let mut unique_dirty_bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_unique_dirty_compressed_line(&mut unique_dirty_bank, cache_agent, 287, 0x6004);
    fill_read_compressed_line(&mut unique_dirty_bank, cache_agent, 288, 0x6014);

    let unique_dirty_miss = unique_dirty_bank
        .accept_cpu_request(read(cache_agent, 289, 0x6024))
        .unwrap();
    assert_eq!(
        unique_dirty_bank.accept_fill(
            fill(unique_dirty_miss.downstream_request().unwrap(), 0x33),
            ChiEvent::CompDataSharedClean,
        ),
        Err(ChiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x6000),
        })
    );
    assert_eq!(
        unique_dirty_bank.state(Address::new(0x6000)),
        Some(ChiState::UniqueDirty)
    );
    assert_eq!(
        unique_dirty_bank.state(Address::new(0x6020)),
        Some(ChiState::InvalidToSharedClean)
    );
    assert_eq!(
        unique_dirty_bank.replacement_way_for(Address::new(0x6000)),
        Some((0, 0))
    );
    assert_eq!(
        unique_dirty_bank.replacement_way_for(Address::new(0x6020)),
        None
    );

    let mut shared_dirty_bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();
    fill_shared_dirty_compressed_line(&mut shared_dirty_bank, cache_agent, 290, 0x7004);
    fill_read_compressed_line(&mut shared_dirty_bank, cache_agent, 291, 0x7014);

    let shared_dirty_miss = shared_dirty_bank
        .accept_cpu_request(read(cache_agent, 292, 0x7024))
        .unwrap();
    assert_eq!(
        shared_dirty_bank.accept_fill(
            fill(shared_dirty_miss.downstream_request().unwrap(), 0x44),
            ChiEvent::CompDataSharedClean,
        ),
        Err(ChiCacheBankError::DirtyReplacementRequiresWriteQueue {
            line: Address::new(0x7000),
        })
    );
    assert_eq!(
        shared_dirty_bank.state(Address::new(0x7000)),
        Some(ChiState::SharedDirty)
    );
    assert_eq!(
        shared_dirty_bank.state(Address::new(0x7020)),
        Some(ChiState::InvalidToSharedClean)
    );
    assert_eq!(
        shared_dirty_bank.replacement_way_for(Address::new(0x7000)),
        Some((0, 0))
    );
    assert_eq!(
        shared_dirty_bank.replacement_way_for(Address::new(0x7020)),
        None
    );
}

#[test]
fn chi_cache_bank_compressed_tags_upgrade_preserves_superblock_siblings() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 293, 0x8004);
    fill_read_compressed_line(&mut bank, cache_agent, 294, 0x8014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 295, 0x8004, vec![0x55]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0x8000)),
        Some(ChiState::SharedCleanToUniqueClean)
    );
    bank.accept_fill_with_compressed_size_bits(
        MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap(),
        ChiEvent::CompDataUniqueDirty,
        half_line_bits(),
    )
    .unwrap();

    assert_eq!(
        bank.state(Address::new(0x8000)),
        Some(ChiState::UniqueDirty)
    );
    assert_eq!(
        bank.state(Address::new(0x8010)),
        Some(ChiState::SharedClean)
    );
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x8000), Address::new(0x8010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x8000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x8010)), Some((0, 0)));
}

#[test]
fn chi_cache_bank_compressed_tags_reject_shared_dirty_to_unique_dirty_superblock_eviction() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_shared_dirty_compressed_line(&mut bank, cache_agent, 296, 0x9004);
    fill_read_compressed_line(&mut bank, cache_agent, 297, 0x9014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 298, 0x9004, vec![0x66]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0x9000)),
        Some(ChiState::SharedDirtyToUniqueDirty)
    );

    let miss = bank
        .accept_cpu_request(read(cache_agent, 299, 0x9024))
        .unwrap();
    assert_eq!(
        bank.accept_fill_with_compressed_size_bits(
            fill(miss.downstream_request().unwrap(), 0x77),
            ChiEvent::CompDataSharedClean,
            half_line_bits(),
        ),
        Err(ChiCacheBankError::TransientReplacementRequiresStableLine {
            line: Address::new(0x9000),
        })
    );
    assert_eq!(
        bank.state(Address::new(0x9000)),
        Some(ChiState::SharedDirtyToUniqueDirty)
    );
    assert_eq!(
        bank.pending_fill_line(upgrade.downstream_request().unwrap().id()),
        Some(Address::new(0x9000))
    );
    assert_eq!(
        bank.pending_fill_line(miss.downstream_request().unwrap().id()),
        Some(Address::new(0x9020))
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x9000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x9020)), None);
}

#[test]
fn chi_cache_bank_compressed_tags_shared_dirty_upgrade_preserves_superblock_siblings() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_shared_dirty_compressed_line(&mut bank, cache_agent, 300, 0xa004);
    fill_read_compressed_line(&mut bank, cache_agent, 301, 0xa014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 302, 0xa004, vec![0x88]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0xa000)),
        Some(ChiState::SharedDirtyToUniqueDirty)
    );
    bank.accept_fill_with_compressed_size_bits(
        MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap(),
        ChiEvent::CompDataUniqueDirty,
        half_line_bits(),
    )
    .unwrap();

    assert_eq!(
        bank.state(Address::new(0xa000)),
        Some(ChiState::UniqueDirty)
    );
    assert_eq!(
        bank.state(Address::new(0xa010)),
        Some(ChiState::SharedClean)
    );
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0xa000), Address::new(0xa010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0xa000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0xa010)), Some((0, 0)));
}

#[test]
fn chi_cache_bank_compressed_tags_snapshot_restore_preserves_superblock_state() {
    let cache_agent = agent(40);
    let config = lru_compressed_tags_config(1, 1, 2);
    let mut bank = ChiCacheBank::new_with_compressed_tags(cache_agent, layout(), config).unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 303, 0xb004);
    fill_read_compressed_line(&mut bank, cache_agent, 304, 0xb014);
    let snapshot = bank.snapshot();

    fill_read_compressed_line(&mut bank, cache_agent, 305, 0xb024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0xb020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0xb000), Address::new(0xb010)]
    );
    assert!(bank.snapshot().compressed_tags().is_some());

    let mut incompatible = ChiCacheBank::new(cache_agent, layout());
    assert_eq!(
        incompatible.restore(&snapshot),
        Err(ChiCacheBankError::SnapshotCompressedTagsModeMismatch {
            snapshot_has_compressed_tags: true,
            bank_has_compressed_tags: false,
        })
    );
}

#[test]
fn chi_cache_bank_compressed_tags_default_fill_is_uncompressed() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_line(&mut bank, cache_agent, 306, 0xc004);
    fill_read_line(&mut bank, cache_agent, 307, 0xc014);

    assert_eq!(bank.state(Address::new(0xc000)), None);
    assert_eq!(
        bank.state(Address::new(0xc010)),
        Some(ChiState::SharedClean)
    );
    assert_eq!(bank.line_addresses(), vec![Address::new(0xc010)]);
}
