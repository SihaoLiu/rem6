use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{
    CpuResponseRecord, DirectoryDecisionRecord, DramMemoryAccessRecord, HarnessError,
    LineBackingStore, PartitionedCacheAgentConfig, PartitionedDirectoryLineHarness,
    PartitionedDramMemoryConfig, PartitionedMemoryConfig, PartitionedRouteHopConfig, SubmitKind,
};
use rem6_directory::{DirectoryDataSource, DirectoryLineState, DirectorySnoop};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_fabric::{FabricLinkId, FabricPath, FabricPathHop};
use rem6_kernel::{PartitionId, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, ResponseStatus,
};
use rem6_protocol_msi::{MsiEvent, MsiLineId, MsiState};
use rem6_transport::{MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MsiLineId {
    MsiLineId::new(Address::new(0x1000))
}

fn dram_target() -> MemoryTargetId {
    MemoryTargetId::new(0)
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

fn fabric_link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn fabric_path(name: &str, latency: u64, bandwidth_bytes_per_tick: u64) -> FabricPath {
    FabricPath::new([
        FabricPathHop::new(fabric_link(name), latency, bandwidth_bytes_per_tick).unwrap(),
    ])
    .unwrap()
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

fn harness_with_slow_snoop_memory() -> PartitionedDirectoryLineHarness {
    PartitionedDirectoryLineHarness::new_with_memory(
        layout(),
        Address::new(0x1000),
        LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedMemoryConfig::new(PartitionId::new(3), endpoint("mem0"), 7, 11),
        [
            cache_config(1, 0, "l1d0", 3, 30),
            cache_config(2, 1, "l1d1", 3, 5),
        ],
    )
    .unwrap()
}

fn harness_with_fabric_memory() -> PartitionedDirectoryLineHarness {
    PartitionedDirectoryLineHarness::new_with_memory(
        layout(),
        Address::new(0x1000),
        LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedMemoryConfig::new(PartitionId::new(4), endpoint("mem0"), 7, 11).with_route_hops(
            [
                PartitionedRouteHopConfig::new(PartitionId::new(4), endpoint("mem0"), 7, 11)
                    .with_request_fabric_path(fabric_path("dir_mem_req", 2, 16))
                    .with_response_fabric_path(fabric_path("mem_dir_rsp", 2, 16)),
            ],
        ),
        [
            cache_config(1, 0, "l1d0", 3, 5).with_route_hops([PartitionedRouteHopConfig::new(
                PartitionId::new(2),
                endpoint("dir0"),
                3,
                5,
            )
            .with_request_fabric_path(fabric_path("l1d0_dir_req", 2, 16))
            .with_response_fabric_path(fabric_path("dir_l1d0_rsp", 2, 16))]),
            cache_config(2, 1, "l1d1", 3, 5).with_route_hops([PartitionedRouteHopConfig::new(
                PartitionId::new(2),
                endpoint("dir0"),
                3,
                5,
            )
            .with_request_fabric_path(fabric_path("l1d1_dir_req", 2, 16))
            .with_response_fabric_path(fabric_path("dir_l1d1_rsp", 2, 16))]),
        ],
    )
    .unwrap()
}

fn harness_with_dram_memory() -> PartitionedDirectoryLineHarness {
    let target = dram_target();
    let mut memory = DramMemoryController::new();
    memory
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            DramGeometry::new(4, 256, 64).unwrap(),
            DramTiming::new(3, 5, 7, 2, 4).unwrap(),
        ))
        .unwrap();
    memory
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(target, Address::new(0x1000), line_data())
        .unwrap();

    PartitionedDirectoryLineHarness::new_with_dram_memory(
        layout(),
        Address::new(0x1000),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedDramMemoryConfig::new(PartitionId::new(3), endpoint("mem0"), 7, 11, memory),
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
fn partitioned_directory_harness_waits_for_backing_snoop_before_write_fill() {
    let mut harness = harness_with_slow_snoop_memory();
    harness
        .submit_cpu_request(agent(1), read(1, 40, 0x1004, 4))
        .unwrap();
    harness.run_until_idle();

    harness
        .submit_cpu_request(agent(2), write(2, 41, 0x1006, vec![0xaa, 0xbb]))
        .unwrap();

    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 89);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Invalid);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MsiState::Modified);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            89,
            CacheControllerResultKind::Fill,
            request_id(2, 41),
            ResponseStatus::Completed,
            None,
        ))
    );
}

#[test]
fn partitioned_directory_harness_routes_backing_read_with_dram_ready_cycle() {
    let mut harness = harness_with_dram_memory();

    let submit = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(harness.directory_state(), DirectoryLineState::new(line()));

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 6);
    assert_eq!(run.final_tick(), 34);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Shared);
    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            34,
            CacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );
    assert_eq!(
        harness.dram_memory_accesses(),
        vec![DramMemoryAccessRecord::new(
            10,
            dram_target(),
            request_id(1, 0),
            0,
            4,
            false,
            18,
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
                29,
                memory_route,
                endpoint("dir0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                34,
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
fn partitioned_directory_harness_quiescent_snapshot_restores_state() {
    let mut source = harness_with_memory();
    source
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    source.run_until_idle();
    source
        .submit_cpu_request(agent(2), read(2, 0, 0x1008, 4))
        .unwrap();
    source.run_until_idle();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = harness_with_memory();
    restored
        .submit_cpu_request(agent(1), write(1, 7, 0x1001, vec![0xee]))
        .unwrap();
    restored.run_until_idle();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(
        restored.directory_state(),
        DirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(restored.trace(), snapshot.trace());
    assert_eq!(restored.cpu_responses(), snapshot.cpu_responses());
    assert_eq!(
        restored.directory_decisions(),
        snapshot.directory_decisions()
    );

    let hit = restored
        .submit_cpu_request(agent(2), read(2, 9, 0x1008, 4))
        .unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            snapshot.scheduler().now(),
            CacheControllerResultKind::Hit,
            request_id(2, 9),
            ResponseStatus::Completed,
            Some(vec![8, 9, 10, 11]),
        ))
    );
}

#[test]
fn partitioned_directory_harness_quiescent_snapshot_restores_dram_memory_state() {
    let mut source = harness_with_dram_memory();
    source
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    source.run_until_idle();
    let snapshot = source.quiescent_snapshot().unwrap();
    assert!(snapshot.backing().is_none());
    assert!(snapshot.dram_memory().is_some());
    assert_eq!(snapshot.dram_accesses(), source.dram_memory_accesses());

    let mut expected = source;
    expected
        .submit_cpu_request(agent(2), read(2, 1, 0x1008, 4))
        .unwrap();
    expected.run_until_idle();

    let mut restored = harness_with_dram_memory();
    restored
        .submit_cpu_request(agent(2), write(2, 7, 0x1001, vec![0xee]))
        .unwrap();
    restored.run_until_idle();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(restored.dram_memory_accesses(), snapshot.dram_accesses());

    restored
        .submit_cpu_request(agent(2), read(2, 1, 0x1008, 4))
        .unwrap();
    restored.run_until_idle();
    assert_eq!(
        restored.quiescent_snapshot().unwrap(),
        expected.quiescent_snapshot().unwrap()
    );
}

#[test]
fn partitioned_directory_harness_quiescent_snapshot_restores_fabric_lane_state() {
    let mut source = harness_with_fabric_memory();
    source
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    source.run_until_idle();
    let snapshot = source.quiescent_snapshot().unwrap();
    let lanes = snapshot.fabric_lanes().unwrap();
    assert!(!lanes.is_empty());

    let mut expected = source;
    expected
        .submit_cpu_request(agent(2), read(2, 1, 0x1008, 4))
        .unwrap();
    expected.run_until_idle();

    let mut restored = harness_with_fabric_memory();
    restored
        .submit_cpu_request(agent(2), read(2, 9, 0x100c, 4))
        .unwrap();
    restored.run_until_idle();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    restored
        .submit_cpu_request(agent(2), read(2, 1, 0x1008, 4))
        .unwrap();
    restored.run_until_idle();
    assert_eq!(restored.trace(), expected.trace());
    assert_eq!(restored.cpu_responses(), expected.cpu_responses());
    assert_eq!(
        restored.quiescent_snapshot().unwrap(),
        expected.quiescent_snapshot().unwrap()
    );
}

#[test]
fn partitioned_directory_harness_quiescent_snapshot_rejects_resource_mismatch() {
    let mut source = harness_with_dram_memory();
    source
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    source.run_until_idle();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = harness_with_memory();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        HarnessError::SnapshotResourceMismatch {
            resource: "backing"
        }
    );
}

#[test]
fn partitioned_directory_harness_quiescent_snapshot_rejects_pending_events() {
    let mut harness = harness();
    harness
        .submit_cpu_request(agent(1), read(1, 3, 0x1000, 4))
        .unwrap();

    assert_eq!(
        harness.quiescent_snapshot().unwrap_err(),
        HarnessError::Scheduler(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: 1
        })
    );
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
    assert_eq!(run.executed_events(), 5);
    assert_eq!(run.final_tick(), 21);
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
            21,
            CacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 0xaa, 0xbb, 8, 9]),
        ))
    );
}

#[test]
fn partitioned_directory_harness_waits_for_owner_snoop_before_peer_fill() {
    let mut harness = harness();
    harness
        .submit_cpu_request(agent(1), write(1, 7, 0x1006, vec![0xaa, 0xbb]))
        .unwrap();
    harness.run_until_idle();

    harness
        .submit_cpu_request(agent(2), read(2, 8, 0x1004, 6))
        .unwrap();

    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 21);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MsiState::Shared);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            21,
            CacheControllerResultKind::Fill,
            request_id(2, 8),
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
