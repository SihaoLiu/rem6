use rem6_cache::MesiCacheControllerResultKind;
use rem6_coherence::{
    LineBackingStore, MesiCpuResponseRecord, MesiDirectoryLineHarness, PartitionedCacheAgentConfig,
    PartitionedMesiDirectoryLineHarness, SubmitKind,
};
use rem6_directory::{MesiDirectoryDataSource, MesiDirectoryLineState, MesiDirectorySnoop};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::{MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MesiLineId {
    MesiLineId::new(Address::new(0x3000))
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
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

fn cache_config(
    agent: u32,
    partition: u32,
    endpoint_name: &str,
    request_latency: u64,
    response_latency: u64,
) -> PartitionedCacheAgentConfig {
    PartitionedCacheAgentConfig::new(
        AgentId::new(agent),
        PartitionId::new(partition),
        endpoint(endpoint_name),
        request_latency,
        response_latency,
    )
}

fn harness() -> MesiDirectoryLineHarness {
    MesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x3000),
        LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap(),
        [agent(1), agent(2), agent(3)],
    )
    .unwrap()
}

fn partitioned_harness() -> PartitionedMesiDirectoryLineHarness {
    PartitionedMesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x3000),
        LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [
            cache_config(1, 0, "l1d0", 3, 9),
            cache_config(2, 1, "l1d1", 5, 7),
            cache_config(3, 3, "l1d2", 2, 4),
        ],
    )
    .unwrap()
}

#[test]
fn mesi_harness_tracks_exclusive_owner_downgrade_and_shared_upgrade() {
    let mut harness = harness();

    let first_read = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    assert_eq!(first_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        first_read.cache_result(),
        MesiCacheControllerResultKind::Miss
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );
    assert_eq!(
        first_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MesiDirectoryDataSource::BackingMemory
    );

    let store_hit = harness
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(store_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(store_hit.directory_decision(), None);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Modified);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );

    let peer_read = harness
        .submit_cpu_request(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        peer_read.directory_decision().unwrap().snoops(),
        &[MesiDirectorySnoop::new(agent(1), MesiEvent::SnoopRead)]
    );
    assert_eq!(
        peer_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MesiDirectoryDataSource::OwnedCache(agent(1))
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MesiState::Shared);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            0,
            MesiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let third_read = harness
        .submit_cpu_request(agent(3), read(3, 0, 0x3000, 4))
        .unwrap();
    assert_eq!(third_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        third_read
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MesiDirectoryDataSource::BackingMemory
    );
    assert_eq!(harness.cache_state(agent(3)).unwrap(), MesiState::Shared);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            0,
            MesiCacheControllerResultKind::Fill,
            request_id(3, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let shared_store = harness
        .submit_cpu_request(agent(2), write(2, 1, 0x3001, vec![0xcc]))
        .unwrap();
    assert_eq!(shared_store.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        shared_store.directory_decision().unwrap().snoops(),
        &[
            MesiDirectorySnoop::new(agent(1), MesiEvent::SnoopWrite),
            MesiDirectorySnoop::new(agent(3), MesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        shared_store
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        MesiDirectoryDataSource::NoData
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Invalid);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MesiState::Modified);
    assert_eq!(harness.cache_state(agent(3)).unwrap(), MesiState::Invalid);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(2), MesiState::Modified)
    );

    let local_hit = harness
        .submit_cpu_request(agent(2), read(2, 2, 0x3000, 4))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            0,
            MesiCacheControllerResultKind::Hit,
            request_id(2, 2),
            ResponseStatus::Completed,
            Some(vec![0, 0xcc, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn partitioned_mesi_harness_waits_for_owner_snoop_before_peer_fill() {
    let mut harness = partitioned_harness();

    let first_read = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    assert_eq!(first_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        MesiState::InvalidToExclusive
    );

    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 12);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );
    assert_eq!(
        harness.cpu_responses(),
        vec![MesiCpuResponseRecord::new(
            12,
            MesiCacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 2, 3]),
        )]
    );
    let decisions = harness.directory_decisions();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].tick(), 3);
    assert_eq!(decisions[0].requester(), agent(1));
    assert_eq!(
        decisions[0].decision().after(),
        &MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );

    let store_hit = harness
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(store_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Modified);

    let peer_read = harness
        .submit_cpu_request(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 26);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MesiState::Shared);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            26,
            MesiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let third_read = harness
        .submit_cpu_request(agent(3), read(3, 0, 0x3000, 4))
        .unwrap();
    assert_eq!(third_read.kind(), SubmitKind::ScheduledMiss);
    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 32);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            32,
            MesiCacheControllerResultKind::Fill,
            request_id(3, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let route = harness.route(agent(1)).unwrap();
    assert_eq!(
        &harness.trace()[..3],
        &[
            MemoryTraceEvent::request(
                0,
                route,
                endpoint("l1d0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("dir0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::response(
                12,
                route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}
