use rem6_cache::{
    CacheCompressedTagsConfig, CacheControllerResultKind, CacheReplacementPolicyKind, MsiCacheBank,
    MsiCacheBankError,
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

fn fill_read_line(bank: &mut MsiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill(fill(miss.downstream_request().unwrap(), sequence as u8))
        .unwrap();
}

fn fill_read_compressed_line(
    bank: &mut MsiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        half_line_bits(),
    )
    .unwrap();
}

#[test]
fn msi_cache_bank_compressed_tags_keep_same_superblock_lines_resident() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 210, 0x4004);
    fill_read_compressed_line(&mut bank, cache_agent, 211, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));

    let snapshot = bank.snapshot();
    let compressed_tags = snapshot.compressed_tags().unwrap();
    assert_eq!(compressed_tags.config().max_compression_ratio(), 2);
    assert_eq!(
        compressed_tags
            .config()
            .superblock_base(Address::new(0x4010)),
        Address::new(0x4000)
    );

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 212, 0x4004))
        .unwrap();
    assert_eq!(first_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[210; 8]
    );

    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 213, 0x4014))
        .unwrap();
    assert_eq!(second_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[211; 8]
    );
}

#[test]
fn msi_cache_bank_compressed_tags_evict_whole_clean_superblock() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 214, 0x5004);
    fill_read_compressed_line(&mut bank, cache_agent, 215, 0x5014);
    fill_read_compressed_line(&mut bank, cache_agent, 216, 0x5024);

    assert_eq!(bank.state(Address::new(0x5000)), None);
    assert_eq!(bank.state(Address::new(0x5010)), None);
    assert_eq!(bank.state(Address::new(0x5020)), Some(MsiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0x5020)]);
    assert_eq!(bank.replacement_way_for(Address::new(0x5000)), None);
    assert_eq!(bank.replacement_way_for(Address::new(0x5020)), Some((0, 0)));
}

#[test]
fn msi_cache_bank_compressed_tags_reject_modified_superblock_eviction_without_write_queue() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    let store = write(cache_agent, 217, 0x6004, vec![0xde, 0xad]);
    let store_miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(store_miss.downstream_request().unwrap(), 0x00),
        half_line_bits(),
    )
    .unwrap();
    fill_read_compressed_line(&mut bank, cache_agent, 218, 0x6014);

    let read_miss = bank
        .accept_cpu_request(read(cache_agent, 219, 0x6024))
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
    assert_eq!(bank.replacement_way_for(Address::new(0x6000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x6020)), None);
}

#[test]
fn msi_cache_bank_compressed_tags_upgrade_preserves_superblock_siblings() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 220, 0x7004);
    fill_read_compressed_line(&mut bank, cache_agent, 221, 0x7014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 222, 0x7004, vec![0x44]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0x7000)),
        Some(MsiState::SharedToModified)
    );
    bank.accept_fill_with_compressed_size_bits(
        MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap(),
        half_line_bits(),
    )
    .unwrap();

    assert_eq!(bank.state(Address::new(0x7000)), Some(MsiState::Modified));
    assert_eq!(bank.state(Address::new(0x7010)), Some(MsiState::Shared));
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x7000), Address::new(0x7010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x7000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x7010)), Some((0, 0)));
}

#[test]
fn msi_cache_bank_compressed_tags_snapshot_restore_preserves_superblock_state() {
    let cache_agent = agent(7);
    let config = lru_compressed_tags_config(1, 1, 2);
    let mut bank = MsiCacheBank::new_with_compressed_tags(cache_agent, layout(), config).unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 223, 0x8004);
    fill_read_compressed_line(&mut bank, cache_agent, 224, 0x8014);
    let snapshot = bank.snapshot();

    fill_read_compressed_line(&mut bank, cache_agent, 225, 0x8024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0x8020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x8000), Address::new(0x8010)]
    );
    assert!(bank.snapshot().compressed_tags().is_some());

    let mut incompatible = MsiCacheBank::new(cache_agent, layout());
    assert_eq!(
        incompatible.restore(&snapshot),
        Err(MsiCacheBankError::SnapshotCompressedTagsModeMismatch {
            snapshot_has_compressed_tags: true,
            bank_has_compressed_tags: false,
        })
    );
}

#[test]
fn msi_cache_bank_compressed_tags_default_fill_is_uncompressed() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_line(&mut bank, cache_agent, 226, 0x9004);
    fill_read_line(&mut bank, cache_agent, 227, 0x9014);

    assert_eq!(bank.state(Address::new(0x9000)), None);
    assert_eq!(bank.state(Address::new(0x9010)), Some(MsiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0x9010)]);
}
