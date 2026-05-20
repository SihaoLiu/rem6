use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{
    CpuResponseRecord, DirectoryLineHarness, HarnessError, LineBackingStore, SubmitKind,
};
use rem6_directory::{DirectoryDataSource, DirectoryLineState, DirectorySnoop};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_msi::{MsiEvent, MsiLineId, MsiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MsiLineId {
    MsiLineId::new(Address::new(0x1000))
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

fn harness() -> DirectoryLineHarness {
    DirectoryLineHarness::new(
        layout(),
        Address::new(0x1000),
        LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap(),
        [agent(1), agent(2)],
    )
    .unwrap()
}

#[test]
fn directory_harness_downgrades_modified_owner_and_supplies_peer_data() {
    let mut harness = harness();

    let owner_store = harness
        .submit_cpu_request(agent(1), write(1, 0, 0x1006, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(owner_store.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Modified);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line()).with_owner(agent(1))
    );

    let peer_read = harness
        .submit_cpu_request(agent(2), read(2, 0, 0x1004, 6))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        peer_read.directory_decision().unwrap().snoops(),
        &[DirectorySnoop::new(agent(1), MsiEvent::SnoopRead)]
    );
    assert_eq!(
        peer_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::ModifiedOwner(agent(1))
    );

    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MsiState::Shared);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            0,
            CacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 0xaa, 0xbb, 8, 9]),
        ))
    );
}

#[test]
fn directory_harness_upgrade_invalidates_peer_and_keeps_modified_hit_local() {
    let mut harness = harness();

    harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1000, 8))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 0, 0x1000, 8))
        .unwrap();

    let upgrade = harness
        .submit_cpu_request(agent(2), write(2, 1, 0x1002, vec![0xcc, 0xdd]))
        .unwrap();
    assert_eq!(upgrade.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        upgrade.directory_decision().unwrap().snoops(),
        &[DirectorySnoop::new(agent(1), MsiEvent::SnoopWrite)]
    );
    assert_eq!(
        upgrade
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::NoData
    );

    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Invalid);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MsiState::Modified);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line()).with_owner(agent(2))
    );

    let local_hit = harness
        .submit_cpu_request(agent(2), read(2, 2, 0x1000, 6))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            0,
            CacheControllerResultKind::Hit,
            request_id(2, 2),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xcc, 0xdd, 4, 5]),
        ))
    );
}

#[test]
fn directory_harness_write_miss_steals_modified_line_data() {
    let mut harness = harness();

    harness
        .submit_cpu_request(agent(1), write(1, 0, 0x1000, vec![0x11, 0x22]))
        .unwrap();

    let write_miss = harness
        .submit_cpu_request(agent(2), write(2, 0, 0x1004, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(write_miss.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        write_miss.directory_decision().unwrap().snoops(),
        &[DirectorySnoop::new(agent(1), MsiEvent::SnoopWrite)]
    );
    assert_eq!(
        write_miss
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::ModifiedOwner(agent(1))
    );

    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Invalid);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MsiState::Modified);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line()).with_owner(agent(2))
    );

    let local_hit = harness
        .submit_cpu_request(agent(2), read(2, 1, 0x1000, 8))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            0,
            CacheControllerResultKind::Hit,
            request_id(2, 1),
            ResponseStatus::Completed,
            Some(vec![0x11, 0x22, 2, 3, 0xaa, 0xbb, 6, 7]),
        ))
    );
    assert_eq!(harness.cache_data(agent(1)).unwrap(), None);
}

#[test]
fn directory_harness_records_replayable_directory_decision_order() {
    let mut harness = harness();

    harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1000, 4))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 0, 0x1008, 4))
        .unwrap();
    harness
        .submit_cpu_request(agent(1), write(1, 1, 0x1001, vec![0xee]))
        .unwrap();

    let decisions = harness.directory_decisions();
    assert_eq!(decisions.len(), 3);
    assert_eq!(decisions[0].request(), MemoryRequestId::new(agent(1), 0));
    assert_eq!(
        decisions[0].after(),
        &DirectoryLineState::new(line()).with_sharer(agent(1))
    );
    assert_eq!(decisions[1].request(), MemoryRequestId::new(agent(2), 0));
    assert_eq!(
        decisions[1].after(),
        &DirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(decisions[2].request(), MemoryRequestId::new(agent(1), 1));
    assert_eq!(
        decisions[2].before(),
        &DirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(
        decisions[2].snoops(),
        &[DirectorySnoop::new(agent(2), MsiEvent::SnoopWrite)]
    );
    assert_eq!(
        decisions[2].after(),
        &DirectoryLineState::new(line()).with_owner(agent(1))
    );
}

#[test]
fn directory_harness_rejects_unknown_cache_agent() {
    let mut harness = harness();

    let error = harness
        .submit_cpu_request(agent(7), read(7, 0, 0x1000, 4))
        .unwrap_err();

    assert_eq!(error, HarnessError::UnknownCache { agent: agent(7) });
    assert_eq!(harness.directory_state(), DirectoryLineState::new(line()));
}
