use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{
    CpuResponseRecord, DirectoryDecisionRecord, HarnessError, LineBackingStore,
    PartitionedCacheAgentConfig, PartitionedDirectoryLineHarness, PartitionedMemoryConfig,
    SubmitKind,
};
use rem6_directory::{DirectoryDataSource, DirectoryLineState, DirectorySnoop};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_msi::{MsiEvent, MsiLineId, MsiState};
use rem6_transport::{MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

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

fn harness() -> PartitionedDirectoryLineHarness {
    PartitionedDirectoryLineHarness::new(
        layout(),
        Address::new(0x1000),
        LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [
            cache_config(1, 0, "l1d0", 3, 5),
            cache_config(2, 1, "l1d1", 3, 5),
        ],
    )
    .unwrap()
}

fn harness_with_memory() -> PartitionedDirectoryLineHarness {
    PartitionedDirectoryLineHarness::new_with_memory(
        layout(),
        Address::new(0x1000),
        LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedMemoryConfig::new(PartitionId::new(3), endpoint("mem0"), 7, 11),
        [
            cache_config(1, 0, "l1d0", 3, 5),
            cache_config(2, 1, "l1d1", 3, 5),
        ],
    )
    .unwrap()
}

#[test]
fn partitioned_directory_harness_routes_write_miss_through_directory_partition() {
    let mut harness = harness();

    let submit = harness
        .submit_cpu_request(agent(1), write(1, 0, 0x1006, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(submit.cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        MsiState::InvalidToModified
    );
    assert_eq!(harness.directory_state(), DirectoryLineState::new(line()));

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 3);
    assert_eq!(run.final_tick(), 8);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Modified);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line()).with_owner(agent(1))
    );
    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            8,
            CacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            None,
        )]
    );

    let route = harness.route(agent(1)).unwrap();
    assert_eq!(
        harness.trace(),
        vec![
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
                8,
                route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn partitioned_directory_harness_routes_backing_read_through_memory_partition() {
    let mut harness = harness_with_memory();

    let submit = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(submit.directory_decision(), None);
    assert_eq!(harness.directory_state(), DirectoryLineState::new(line()));

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 5);
    assert_eq!(run.final_tick(), 26);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Shared);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line()).with_sharer(agent(1))
    );
    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            26,
            CacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );

    let cache_route = harness.route(agent(1)).unwrap();
    let memory_route = harness.memory_route().unwrap();
    assert_eq!(
        harness.trace(),
        vec![
            MemoryTraceEvent::request(
                0,
                cache_route,
                endpoint("l1d0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                3,
                cache_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                3,
                memory_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                10,
                memory_route,
                endpoint("mem0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::response(
                21,
                memory_route,
                endpoint("dir0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                26,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
    assert_eq!(harness.directory_decisions()[0].tick(), 3);
}

#[test]
fn partitioned_directory_harness_downgrades_owner_on_peer_read() {
    let mut harness = harness();
    harness
        .submit_cpu_request(agent(1), write(1, 0, 0x1006, vec![0xaa, 0xbb]))
        .unwrap();
    harness.run_until_idle();

    let peer_read = harness
        .submit_cpu_request(agent(2), read(2, 0, 0x1004, 6))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(peer_read.directory_decision(), None);
    assert_eq!(
        harness.cache_state(agent(2)).unwrap(),
        MsiState::InvalidToShared
    );

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 3);
    assert_eq!(run.final_tick(), 16);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MsiState::Shared);
    assert_eq!(
        harness.directory_state(),
        DirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );

    let decisions = harness.directory_decisions();
    let decision = decisions.last().unwrap();
    assert_eq!(decision.tick(), 11);
    assert_eq!(decision.requester(), agent(2));
    assert_eq!(
        decision,
        &DirectoryDecisionRecord::new(11, agent(2), decision.decision().clone())
    );
    assert_eq!(
        decision.decision().snoops(),
        &[DirectorySnoop::new(agent(1), MsiEvent::SnoopRead)]
    );
    assert_eq!(
        decision.decision().grant().unwrap().data_source(),
        DirectoryDataSource::ModifiedOwner(agent(1))
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            16,
            CacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 0xaa, 0xbb, 8, 9]),
        ))
    );
}

#[test]
fn partitioned_directory_harness_rejects_unknown_agent_without_events() {
    let mut harness = harness();

    let error = harness
        .submit_cpu_request(agent(7), read(7, 0, 0x1000, 4))
        .unwrap_err();

    assert_eq!(error, HarnessError::UnknownCache { agent: agent(7) });
    assert_eq!(harness.run_until_idle().executed_events(), 0);
}
