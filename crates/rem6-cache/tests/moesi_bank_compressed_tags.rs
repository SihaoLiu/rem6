use rem6_cache::{
    CacheCompressedTagsConfig, CacheReplacementPolicyKind, MoesiCacheBank, MoesiCacheBankError,
    MoesiCacheControllerResultKind,
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

fn fill_read_line(bank: &mut MoesiCacheBank, cache_agent: AgentId, sequence: u64, address: u64) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        MoesiEvent::DataShared,
    )
    .unwrap();
}

fn fill_read_compressed_line(
    bank: &mut MoesiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let request = read(cache_agent, sequence, address);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(miss.downstream_request().unwrap(), sequence as u8),
        MoesiEvent::DataShared,
        half_line_bits(),
    )
    .unwrap();
}

fn fill_modified_compressed_line(
    bank: &mut MoesiCacheBank,
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
) {
    let request = write(cache_agent, sequence, address, vec![sequence as u8]);
    let miss = bank.accept_cpu_request(request).unwrap();
    bank.accept_fill_with_compressed_size_bits(
        fill(miss.downstream_request().unwrap(), 0x00),
        MoesiEvent::DataModified,
        half_line_bits(),
    )
    .unwrap();
}

#[test]
fn moesi_cache_bank_compressed_tags_keep_same_superblock_lines_resident() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 250, 0x4004);
    fill_read_compressed_line(&mut bank, cache_agent, 251, 0x4014);

    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x4000), Address::new(0x4010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x4000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x4010)), Some((0, 0)));
    assert!(bank.snapshot().compressed_tags().is_some());

    let first_hit = bank
        .accept_cpu_request(read(cache_agent, 252, 0x4004))
        .unwrap();
    assert_eq!(first_hit.kind(), MoesiCacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[250; 8]
    );

    let second_hit = bank
        .accept_cpu_request(read(cache_agent, 253, 0x4014))
        .unwrap();
    assert_eq!(second_hit.kind(), MoesiCacheControllerResultKind::Hit);
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[251; 8]
    );
}

#[test]
fn moesi_cache_bank_compressed_tags_evict_whole_clean_superblock() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 254, 0x5004);
    fill_read_compressed_line(&mut bank, cache_agent, 255, 0x5014);
    fill_read_compressed_line(&mut bank, cache_agent, 256, 0x5024);

    assert_eq!(bank.state(Address::new(0x5000)), None);
    assert_eq!(bank.state(Address::new(0x5010)), None);
    assert_eq!(bank.state(Address::new(0x5020)), Some(MoesiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0x5020)]);
    assert_eq!(bank.replacement_way_for(Address::new(0x5000)), None);
    assert_eq!(bank.replacement_way_for(Address::new(0x5020)), Some((0, 0)));
}

#[test]
fn moesi_cache_bank_compressed_tags_reject_dirty_owner_superblock_eviction_without_write_queue() {
    let cache_agent = agent(30);
    let mut modified_bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_modified_compressed_line(&mut modified_bank, cache_agent, 257, 0x6004);
    fill_read_compressed_line(&mut modified_bank, cache_agent, 258, 0x6014);

    let modified_miss = modified_bank
        .accept_cpu_request(read(cache_agent, 259, 0x6024))
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
        modified_bank.replacement_way_for(Address::new(0x6000)),
        Some((0, 0))
    );
    assert_eq!(
        modified_bank.replacement_way_for(Address::new(0x6020)),
        None
    );

    let mut owned_bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();
    fill_modified_compressed_line(&mut owned_bank, cache_agent, 260, 0x7004);
    owned_bank
        .accept_snoop(Address::new(0x7004), MoesiEvent::SnoopRead)
        .unwrap();
    assert_eq!(
        owned_bank.state(Address::new(0x7000)),
        Some(MoesiState::Owned)
    );
    fill_read_compressed_line(&mut owned_bank, cache_agent, 261, 0x7014);

    let owned_miss = owned_bank
        .accept_cpu_request(read(cache_agent, 262, 0x7024))
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
        owned_bank.replacement_way_for(Address::new(0x7000)),
        Some((0, 0))
    );
    assert_eq!(owned_bank.replacement_way_for(Address::new(0x7020)), None);
}

#[test]
fn moesi_cache_bank_compressed_tags_upgrade_preserves_superblock_siblings() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 263, 0x8004);
    fill_read_compressed_line(&mut bank, cache_agent, 264, 0x8014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 265, 0x8004, vec![0x44]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0x8000)),
        Some(MoesiState::SharedToModified)
    );
    bank.accept_fill_with_compressed_size_bits(
        MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap(),
        MoesiEvent::DataModified,
        half_line_bits(),
    )
    .unwrap();

    assert_eq!(bank.state(Address::new(0x8000)), Some(MoesiState::Modified));
    assert_eq!(bank.state(Address::new(0x8010)), Some(MoesiState::Shared));
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x8000), Address::new(0x8010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0x8000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0x8010)), Some((0, 0)));
}

#[test]
fn moesi_cache_bank_compressed_tags_reject_owned_to_modified_superblock_eviction() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_modified_compressed_line(&mut bank, cache_agent, 266, 0x9004);
    bank.accept_snoop(Address::new(0x9004), MoesiEvent::SnoopRead)
        .unwrap();
    fill_read_compressed_line(&mut bank, cache_agent, 267, 0x9014);

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 268, 0x9004, vec![0x55]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0x9000)),
        Some(MoesiState::OwnedToModified)
    );

    let miss = bank
        .accept_cpu_request(read(cache_agent, 269, 0x9024))
        .unwrap();
    assert_eq!(
        bank.accept_fill_with_compressed_size_bits(
            fill(miss.downstream_request().unwrap(), 0x66),
            MoesiEvent::DataShared,
            half_line_bits(),
        ),
        Err(
            MoesiCacheBankError::TransientReplacementRequiresStableLine {
                line: Address::new(0x9000),
            }
        )
    );
    assert_eq!(
        bank.state(Address::new(0x9000)),
        Some(MoesiState::OwnedToModified)
    );
    assert_eq!(bank.state(Address::new(0x9010)), Some(MoesiState::Shared));
    assert_eq!(
        bank.state(Address::new(0x9020)),
        Some(MoesiState::InvalidToExclusive)
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
fn moesi_cache_bank_compressed_tags_owned_upgrade_preserves_superblock_siblings() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_modified_compressed_line(&mut bank, cache_agent, 270, 0xa004);
    bank.accept_snoop(Address::new(0xa004), MoesiEvent::SnoopRead)
        .unwrap();
    fill_read_compressed_line(&mut bank, cache_agent, 271, 0xa014);
    assert_eq!(bank.state(Address::new(0xa000)), Some(MoesiState::Owned));

    let upgrade = bank
        .accept_cpu_request(write(cache_agent, 272, 0xa004, vec![0x77]))
        .unwrap();
    assert_eq!(
        bank.state(Address::new(0xa000)),
        Some(MoesiState::OwnedToModified)
    );
    bank.accept_fill_with_compressed_size_bits(
        MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap(),
        MoesiEvent::DataModified,
        half_line_bits(),
    )
    .unwrap();

    assert_eq!(bank.state(Address::new(0xa000)), Some(MoesiState::Modified));
    assert_eq!(bank.state(Address::new(0xa010)), Some(MoesiState::Shared));
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0xa000), Address::new(0xa010)]
    );
    assert_eq!(bank.replacement_way_for(Address::new(0xa000)), Some((0, 0)));
    assert_eq!(bank.replacement_way_for(Address::new(0xa010)), Some((0, 0)));
}

#[test]
fn moesi_cache_bank_compressed_tags_snapshot_restore_preserves_superblock_state() {
    let cache_agent = agent(30);
    let config = lru_compressed_tags_config(1, 1, 2);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(cache_agent, layout(), config).unwrap();

    fill_read_compressed_line(&mut bank, cache_agent, 273, 0xb004);
    fill_read_compressed_line(&mut bank, cache_agent, 274, 0xb014);
    let snapshot = bank.snapshot();

    fill_read_compressed_line(&mut bank, cache_agent, 275, 0xb024);
    assert_eq!(bank.line_addresses(), vec![Address::new(0xb020)]);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0xb000), Address::new(0xb010)]
    );
    assert!(bank.snapshot().compressed_tags().is_some());

    let mut incompatible = MoesiCacheBank::new(cache_agent, layout());
    assert_eq!(
        incompatible.restore(&snapshot),
        Err(MoesiCacheBankError::SnapshotCompressedTagsModeMismatch {
            snapshot_has_compressed_tags: true,
            bank_has_compressed_tags: false,
        })
    );
}

#[test]
fn moesi_cache_bank_compressed_tags_default_fill_is_uncompressed() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_compressed_tags(
        cache_agent,
        layout(),
        lru_compressed_tags_config(1, 1, 2),
    )
    .unwrap();

    fill_read_line(&mut bank, cache_agent, 276, 0xc004);
    fill_read_line(&mut bank, cache_agent, 277, 0xc014);

    assert_eq!(bank.state(Address::new(0xc000)), None);
    assert_eq!(bank.state(Address::new(0xc010)), Some(MoesiState::Shared));
    assert_eq!(bank.line_addresses(), vec![Address::new(0xc010)]);
}
