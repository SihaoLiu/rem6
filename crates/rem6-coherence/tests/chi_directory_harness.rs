use rem6_cache::ChiCacheControllerResultKind;
use rem6_coherence::{ChiCpuResponseRecord, ChiDirectoryLineHarness, LineBackingStore, SubmitKind};
use rem6_directory::{ChiDirectoryDataSource, ChiDirectoryLineState, ChiDirectorySnoop};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_chi::{ChiEvent, ChiLineId, ChiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> ChiLineId {
    ChiLineId::new(Address::new(0x6000))
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn request_id(agent_id: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(agent_id), sequence)
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn read(agent_id: u32, sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent_id, sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(agent_id: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        request_id(agent_id, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn harness() -> ChiDirectoryLineHarness {
    ChiDirectoryLineHarness::new(
        layout(),
        Address::new(0x6000),
        LineBackingStore::new(layout(), Address::new(0x6000), line_data()).unwrap(),
        [agent(1), agent(2), agent(3)],
    )
    .unwrap()
}

#[test]
fn chi_harness_unique_dirty_peer_read_updates_backing_for_later_reader() {
    let mut harness = harness();

    let first_write = harness
        .submit_cpu_request(agent(1), write(1, 0, 0x6002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(first_write.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        first_write.cache_result(),
        ChiCacheControllerResultKind::Miss
    );
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        ChiState::UniqueDirty
    );
    assert_eq!(
        harness.directory_state(),
        ChiDirectoryLineState::new(line()).with_unique_owner(agent(1), ChiState::UniqueDirty)
    );
    assert_eq!(
        first_write
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        ChiDirectoryDataSource::BackingMemory
    );

    let peer_read = harness
        .submit_cpu_request(agent(2), read(2, 0, 0x6000, 4))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        peer_read.directory_decision().unwrap().snoops(),
        &[ChiDirectorySnoop::new(agent(1), ChiEvent::SnoopShared)]
    );
    assert_eq!(
        peer_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        ChiDirectoryDataSource::OwnerCache(agent(1))
    );
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        ChiState::SharedClean
    );
    assert_eq!(
        harness.cache_state(agent(2)).unwrap(),
        ChiState::SharedClean
    );
    assert_eq!(
        harness.directory_state(),
        ChiDirectoryLineState::new(line())
            .with_sharer(agent(1), ChiState::SharedClean)
            .with_sharer(agent(2), ChiState::SharedClean)
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            0,
            ChiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let later_read = harness
        .submit_cpu_request(agent(3), read(3, 0, 0x6000, 4))
        .unwrap();
    assert_eq!(later_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(later_read.directory_decision().unwrap().snoops(), &[]);
    assert_eq!(
        later_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        ChiDirectoryDataSource::BackingMemory
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            0,
            ChiCacheControllerResultKind::Fill,
            request_id(3, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn chi_harness_shared_store_uses_make_read_unique_and_invalidates_peer() {
    let mut harness = harness();

    harness
        .submit_cpu_request(agent(1), read(1, 0, 0x6000, 8))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 0, 0x6000, 8))
        .unwrap();

    let shared_store = harness
        .submit_cpu_request(agent(2), write(2, 1, 0x6001, vec![0xcc]))
        .unwrap();

    assert_eq!(shared_store.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        shared_store.directory_decision().unwrap().snoops(),
        &[ChiDirectorySnoop::new(agent(1), ChiEvent::SnoopUnique)]
    );
    assert_eq!(
        shared_store
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        ChiDirectoryDataSource::NoData
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), ChiState::Invalid);
    assert_eq!(
        harness.cache_state(agent(2)).unwrap(),
        ChiState::UniqueDirty
    );
    assert_eq!(
        harness.directory_state(),
        ChiDirectoryLineState::new(line()).with_unique_owner(agent(2), ChiState::UniqueDirty)
    );

    let local_hit = harness
        .submit_cpu_request(agent(2), read(2, 2, 0x6000, 4))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            0,
            ChiCacheControllerResultKind::Hit,
            request_id(2, 2),
            ResponseStatus::Completed,
            Some(vec![0, 0xcc, 2, 3]),
        ))
    );
}

#[test]
fn chi_harness_snapshot_restore_reinstates_serial_state() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), read(1, 0, 0x6000, 8))
        .unwrap();
    source
        .submit_cpu_request(agent(2), read(2, 0, 0x6000, 8))
        .unwrap();
    source
        .submit_cpu_request(agent(2), write(2, 1, 0x6002, vec![0xdd]))
        .unwrap();
    let snapshot = source.snapshot();

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(1), write(1, 4, 0x6000, vec![0x99]))
        .unwrap();
    assert_ne!(restored.snapshot(), snapshot);

    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.directory_state(),
        ChiDirectoryLineState::new(line()).with_unique_owner(agent(2), ChiState::UniqueDirty)
    );

    let local_hit = restored
        .submit_cpu_request(agent(2), read(2, 9, 0x6000, 4))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            0,
            ChiCacheControllerResultKind::Hit,
            request_id(2, 9),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xdd, 3]),
        ))
    );
}
