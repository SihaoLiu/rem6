use rem6_cache::MoesiCacheControllerResultKind;
use rem6_coherence::{
    LineBackingStore, MoesiCpuResponseRecord, MoesiDirectoryLineHarness, SubmitKind,
};
use rem6_directory::{MoesiDirectoryDataSource, MoesiDirectoryLineState, MoesiDirectorySnoop};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_moesi::{MoesiEvent, MoesiLineId, MoesiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MoesiLineId {
    MoesiLineId::new(Address::new(0x5000))
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn read(agent: u32, sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent, sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(agent: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        request_id(agent, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn harness() -> MoesiDirectoryLineHarness {
    MoesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x5000),
        LineBackingStore::new(layout(), Address::new(0x5000), line_data()).unwrap(),
        [agent(1), agent(2), agent(3)],
    )
    .unwrap()
}

#[test]
fn moesi_harness_keeps_dirty_owner_for_peer_reads() {
    let mut harness = harness();

    let first_write = harness
        .submit_cpu_request(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(first_write.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        first_write.cache_result(),
        MoesiCacheControllerResultKind::Miss
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Modified);
    assert_eq!(
        harness.directory_state(),
        MoesiDirectoryLineState::new(line()).with_owner(agent(1), MoesiState::Modified)
    );
    assert_eq!(
        first_write
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MoesiDirectoryDataSource::BackingMemory
    );

    let peer_read = harness
        .submit_cpu_request(agent(2), read(2, 0, 0x5000, 4))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        peer_read.directory_decision().unwrap().snoops(),
        &[MoesiDirectorySnoop::new(agent(1), MoesiEvent::SnoopRead)]
    );
    assert_eq!(
        peer_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MoesiDirectoryDataSource::OwnerCache(agent(1))
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Owned);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MoesiState::Shared);
    assert_eq!(
        harness.directory_state(),
        MoesiDirectoryLineState::new(line())
            .with_owner(agent(1), MoesiState::Owned)
            .with_sharer(agent(2))
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            0,
            MoesiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let third_read = harness
        .submit_cpu_request(agent(3), read(3, 0, 0x5000, 4))
        .unwrap();
    assert_eq!(third_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        third_read.directory_decision().unwrap().snoops(),
        &[MoesiDirectorySnoop::new(agent(1), MoesiEvent::SnoopRead)]
    );
    assert_eq!(
        third_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MoesiDirectoryDataSource::OwnerCache(agent(1))
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Owned);
    assert_eq!(harness.cache_state(agent(3)).unwrap(), MoesiState::Shared);
    assert_eq!(
        harness.directory_state(),
        MoesiDirectoryLineState::new(line())
            .with_owner(agent(1), MoesiState::Owned)
            .with_sharer(agent(2))
            .with_sharer(agent(3))
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            0,
            MoesiCacheControllerResultKind::Fill,
            request_id(3, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn moesi_harness_shared_store_steals_owned_line_data() {
    let mut harness = harness();

    harness
        .submit_cpu_request(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 0, 0x5000, 4))
        .unwrap();
    harness
        .submit_cpu_request(agent(3), read(3, 0, 0x5000, 4))
        .unwrap();

    let shared_store = harness
        .submit_cpu_request(agent(2), write(2, 1, 0x5001, vec![0xcc]))
        .unwrap();

    assert_eq!(shared_store.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        shared_store.directory_decision().unwrap().snoops(),
        &[
            MoesiDirectorySnoop::new(agent(1), MoesiEvent::SnoopWrite),
            MoesiDirectorySnoop::new(agent(3), MoesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        shared_store
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MoesiDirectoryDataSource::OwnerCache(agent(1))
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Invalid);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MoesiState::Modified);
    assert_eq!(harness.cache_state(agent(3)).unwrap(), MoesiState::Invalid);
    assert_eq!(
        harness.directory_state(),
        MoesiDirectoryLineState::new(line()).with_owner(agent(2), MoesiState::Modified)
    );

    let local_hit = harness
        .submit_cpu_request(agent(2), read(2, 2, 0x5000, 4))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            0,
            MoesiCacheControllerResultKind::Hit,
            request_id(2, 2),
            ResponseStatus::Completed,
            Some(vec![0, 0xcc, 0xaa, 0xbb]),
        ))
    );
}
