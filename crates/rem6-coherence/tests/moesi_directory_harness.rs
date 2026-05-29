use rem6_cache::MoesiCacheControllerResultKind;
use rem6_coherence::{
    HarnessError, LineBackingStore, MoesiCpuResponseRecord, MoesiDirectoryLineHarness,
    MoesiDirectoryLineHarnessSnapshot, MoesiHarnessError, PartitionedCacheAgentConfig,
    PartitionedDramMemoryConfig, PartitionedMoesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarnessSnapshot, PartitionedRouteHopConfig, SubmitKind,
};
use rem6_directory::{MoesiDirectoryDataSource, MoesiDirectoryLineState, MoesiDirectorySnoop};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_fabric::{FabricLinkId, FabricPath, FabricPathHop};
use rem6_kernel::{ParallelRunProfile, PartitionId, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, ResponseStatus,
};
use rem6_protocol_moesi::{MoesiEvent, MoesiLineId, MoesiState};
use rem6_transport::TransportEndpointId;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MoesiLineId {
    MoesiLineId::new(Address::new(0x5000))
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

fn harness() -> MoesiDirectoryLineHarness {
    MoesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x5000),
        LineBackingStore::new(layout(), Address::new(0x5000), line_data()).unwrap(),
        [agent(1), agent(2), agent(3)],
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

fn partitioned_harness() -> PartitionedMoesiDirectoryLineHarness {
    PartitionedMoesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x5000),
        LineBackingStore::new(layout(), Address::new(0x5000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [
            cache_config(1, 0, "l1d0", 3, 9),
            cache_config(2, 1, "l1d1", 5, 7),
            cache_config(3, 3, "l1d2", 2, 4).with_route_hops([
                PartitionedRouteHopConfig::new(PartitionId::new(4), endpoint("mesh0"), 2, 3),
                PartitionedRouteHopConfig::new(PartitionId::new(2), endpoint("dir0"), 4, 5),
            ]),
        ],
    )
    .unwrap()
}

fn partitioned_harness_with_two_caches() -> PartitionedMoesiDirectoryLineHarness {
    PartitionedMoesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x5000),
        LineBackingStore::new(layout(), Address::new(0x5000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [
            cache_config(1, 0, "l1d0", 3, 9),
            cache_config(2, 1, "l1d1", 5, 7),
        ],
    )
    .unwrap()
}

fn partitioned_harness_with_dram_memory() -> PartitionedMoesiDirectoryLineHarness {
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
            AccessSize::new(0x8000).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(target, Address::new(0x5000), line_data())
        .unwrap();

    PartitionedMoesiDirectoryLineHarness::new_with_dram_memory(
        layout(),
        Address::new(0x5000),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedDramMemoryConfig::new(PartitionId::new(4), endpoint("mem0"), 7, 11, memory),
        [
            cache_config(1, 0, "l1d0", 3, 9),
            cache_config(2, 1, "l1d1", 5, 7),
            cache_config(3, 3, "l1d2", 2, 4),
        ],
    )
    .unwrap()
}

fn partitioned_harness_with_fabric_routes() -> PartitionedMoesiDirectoryLineHarness {
    PartitionedMoesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x5000),
        LineBackingStore::new(layout(), Address::new(0x5000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [
            cache_config(1, 0, "l1d0", 3, 9).with_route_hops([PartitionedRouteHopConfig::new(
                PartitionId::new(2),
                endpoint("dir0"),
                3,
                9,
            )
            .with_request_fabric_path(fabric_path("l1d0_dir_req", 2, 16))
            .with_response_fabric_path(fabric_path("dir_l1d0_rsp", 2, 16))]),
            cache_config(2, 1, "l1d1", 5, 7).with_route_hops([PartitionedRouteHopConfig::new(
                PartitionId::new(2),
                endpoint("dir0"),
                5,
                7,
            )
            .with_request_fabric_path(fabric_path("l1d1_dir_req", 2, 16))
            .with_response_fabric_path(fabric_path("dir_l1d1_rsp", 2, 16))]),
            cache_config(3, 3, "l1d2", 2, 4),
        ],
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

#[test]
fn moesi_harness_snapshot_restore_reinstates_serial_state() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    source
        .submit_cpu_request(agent(2), read(2, 0, 0x5000, 4))
        .unwrap();
    source
        .submit_cpu_request(agent(3), read(3, 0, 0x5000, 4))
        .unwrap();
    let snapshot = source.snapshot();

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(3), write(3, 4, 0x5000, vec![0x99]))
        .unwrap();
    assert_ne!(restored.snapshot(), snapshot);

    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.directory_state(),
        MoesiDirectoryLineState::new(line())
            .with_owner(agent(1), MoesiState::Owned)
            .with_sharer(agent(2))
            .with_sharer(agent(3))
    );

    let store = restored
        .submit_cpu_request(agent(2), write(2, 9, 0x5001, vec![0xcc]))
        .unwrap();
    assert_eq!(store.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        store.directory_decision().unwrap().snoops(),
        &[
            MoesiDirectorySnoop::new(agent(1), MoesiEvent::SnoopWrite),
            MoesiDirectorySnoop::new(agent(3), MoesiEvent::SnoopWrite),
        ]
    );

    let local_hit = restored
        .submit_cpu_request(agent(2), read(2, 10, 0x5000, 4))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            0,
            MoesiCacheControllerResultKind::Hit,
            request_id(2, 10),
            ResponseStatus::Completed,
            Some(vec![0, 0xcc, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn moesi_harness_restore_rejects_backing_line_mismatch_without_mutation() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), write(1, 0, 0x5002, vec![0xaa]))
        .unwrap();
    let snapshot = source.snapshot();
    let bad_snapshot = MoesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.directory().clone(),
        snapshot.caches().clone(),
        LineBackingStore::new(layout(), Address::new(0x6000), line_data()).unwrap(),
        snapshot.cpu_responses().to_vec(),
        snapshot.directory_decisions().to_vec(),
    );

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(2), write(2, 9, 0x5004, vec![0xdd]))
        .unwrap();
    let before = restored.snapshot();

    assert_eq!(
        restored.restore(&bad_snapshot).unwrap_err(),
        MoesiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x5000),
            actual: Address::new(0x6000),
        })
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_restores_owned_state() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x5000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    restored
        .submit_cpu_request_parallel(agent(3), write(3, 7, 0x5001, vec![0xee]))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(
        restored.directory_state(),
        MoesiDirectoryLineState::new(line())
            .with_owner(agent(1), MoesiState::Owned)
            .with_sharer(agent(2))
    );
    assert_eq!(restored.trace(), snapshot.trace());
    assert_eq!(restored.cpu_responses(), snapshot.cpu_responses());
    assert_eq!(
        restored.directory_decisions(),
        snapshot.directory_decisions()
    );

    let hit = restored
        .submit_cpu_request_parallel(agent(2), read(2, 9, 0x5000, 4))
        .unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            snapshot.scheduler().now(),
            MoesiCacheControllerResultKind::Hit,
            request_id(2, 9),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_restores_dirty_owner_transfer() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x5000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    restored.restore_quiescent(&snapshot).unwrap();
    restored
        .submit_cpu_request_parallel(agent(2), write(2, 1, 0x5001, vec![0xcc]))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();

    assert_eq!(restored.cache_state(agent(1)).unwrap(), MoesiState::Invalid);
    assert_eq!(
        restored.cache_state(agent(2)).unwrap(),
        MoesiState::Modified
    );
    assert_eq!(
        restored.directory_state(),
        MoesiDirectoryLineState::new(line()).with_owner(agent(2), MoesiState::Modified)
    );
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            40,
            MoesiCacheControllerResultKind::Fill,
            request_id(2, 1),
            ResponseStatus::Completed,
            None,
        ))
    );
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_restores_dram_memory_state() {
    let mut source = partitioned_harness_with_dram_memory();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    assert!(snapshot.dram_memory().is_some());
    assert_eq!(snapshot.dram_accesses(), source.dram_memory_accesses());

    let mut expected = source;
    expected
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x5000, 4))
        .unwrap();
    expected.run_until_idle_parallel().unwrap();

    let mut restored = partitioned_harness_with_dram_memory();
    restored
        .submit_cpu_request_parallel(agent(3), read(3, 7, 0x5004, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(restored.dram_memory_accesses(), snapshot.dram_accesses());

    restored
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x5000, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_eq!(
        restored.quiescent_snapshot().unwrap(),
        expected.quiescent_snapshot().unwrap()
    );
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_restores_fabric_lane_state() {
    let mut source = partitioned_harness_with_fabric_routes();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    let lanes = snapshot.fabric_lanes().unwrap();
    assert!(!lanes.is_empty());

    let mut expected = source;
    expected
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x5000, 4))
        .unwrap();
    expected.run_until_idle_parallel().unwrap();

    let mut restored = partitioned_harness_with_fabric_routes();
    restored
        .submit_cpu_request_parallel(agent(2), read(2, 9, 0x5004, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    restored
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x5000, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_eq!(restored.trace(), expected.trace());
    assert_eq!(restored.cpu_responses(), expected.cpu_responses());
    assert_eq!(
        restored.quiescent_snapshot().unwrap(),
        expected.quiescent_snapshot().unwrap()
    );
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_restores_parallel_run_history() {
    let mut source = partitioned_harness_with_fabric_routes();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    let first = source.run_until_idle_parallel_recorded().unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x5000, 4))
        .unwrap();
    let second = source.run_until_idle_parallel_recorded().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    assert_eq!(source.parallel_runs(), &[first.clone(), second.clone()]);
    assert_eq!(snapshot.parallel_runs(), source.parallel_runs());
    assert_eq!(
        source.parallel_run_history(),
        snapshot.parallel_run_history()
    );

    let history = snapshot.parallel_run_history();
    assert_eq!(history.run_count(), 2);
    assert_eq!(history.profile(), first.profile().merge(second.profile()));
    assert_eq!(
        history.total_cpu_responses(),
        first.cpu_response_count() + second.cpu_response_count()
    );
    assert_eq!(
        history.total_directory_decisions(),
        first.directory_decision_count() + second.directory_decision_count()
    );
    assert_eq!(
        history.total_fabric_transfers(),
        first.fabric_transfer_count() + second.fabric_transfer_count()
    );
    assert!(history.has_parallel_work());
    assert!(history.has_directory_activity());
    assert!(history.has_resource_activity());

    let mut restored = partitioned_harness_with_fabric_routes();
    restored
        .submit_cpu_request_parallel(agent(3), read(3, 9, 0x5008, 4))
        .unwrap();
    restored.run_until_idle_parallel_recorded().unwrap();
    assert_ne!(restored.parallel_runs(), snapshot.parallel_runs());

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.parallel_runs(), &[first, second]);
    assert_eq!(restored.parallel_run_history(), history);
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_rejects_pending_events() {
    let mut harness = partitioned_harness();
    harness
        .submit_cpu_request_parallel(agent(1), read(1, 3, 0x5000, 4))
        .unwrap();

    assert_eq!(
        harness.quiescent_snapshot().unwrap_err(),
        MoesiHarnessError::Scheduler(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: 1
        })
    );
}

#[test]
fn partitioned_moesi_harness_restore_quiescent_rejects_current_pending_events() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x5000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    restored
        .submit_cpu_request_parallel(agent(2), read(2, 7, 0x5000, 4))
        .unwrap();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        MoesiHarnessError::Scheduler(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: 1
        })
    );
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_rejects_resource_mismatch() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness_with_dram_memory();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        MoesiHarnessError::SnapshotResourceMismatch { resource: "dram" }
    );
}

#[test]
fn partitioned_moesi_harness_quiescent_restore_rejects_backing_line_mismatch_without_mutation() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    let bad_snapshot = PartitionedMoesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        snapshot.directory().clone(),
        snapshot.caches().clone(),
        LineBackingStore::new(layout(), Address::new(0x6000), line_data()).unwrap(),
        snapshot.dram_memory().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );

    let mut restored = partitioned_harness();
    let before = restored.quiescent_snapshot().unwrap();

    assert_eq!(
        restored.restore_quiescent(&bad_snapshot).unwrap_err(),
        MoesiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x5000),
            actual: Address::new(0x6000),
        })
    );
    assert_eq!(restored.quiescent_snapshot().unwrap(), before);
}

#[test]
fn partitioned_moesi_harness_quiescent_snapshot_rejects_cache_shape_mismatch() {
    let mut source = partitioned_harness_with_two_caches();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x5000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        MoesiHarnessError::UnknownCache { agent: agent(3) }
    );
}

#[test]
fn partitioned_moesi_harness_waits_for_owner_snoop_before_peer_fill() {
    let mut harness = partitioned_harness();

    let first_write = harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(first_write.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        MoesiState::InvalidToModified
    );
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 12);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Modified);
    assert_eq!(
        harness.directory_state(),
        MoesiDirectoryLineState::new(line()).with_owner(agent(1), MoesiState::Modified)
    );

    let peer_read = harness
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x5000, 4))
        .unwrap();
    assert_eq!(peer_read.kind(), SubmitKind::ScheduledMiss);
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 26);
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
            26,
            MoesiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let shared_store = harness
        .submit_cpu_request_parallel(agent(2), write(2, 1, 0x5001, vec![0xcc]))
        .unwrap();
    assert_eq!(shared_store.kind(), SubmitKind::ScheduledMiss);
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 40);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Invalid);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MoesiState::Modified);
    assert_eq!(
        harness.directory_state(),
        MoesiDirectoryLineState::new(line()).with_owner(agent(2), MoesiState::Modified)
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MoesiCpuResponseRecord::new(
            40,
            MoesiCacheControllerResultKind::Fill,
            request_id(2, 1),
            ResponseStatus::Completed,
            None,
        ))
    );
}

#[test]
fn partitioned_moesi_harness_recorded_parallel_run_reports_protocol_activity() {
    let mut harness = partitioned_harness();

    let first_write = harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(first_write.kind(), SubmitKind::ScheduledMiss);
    let run = harness.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.summary().final_tick(), 12);
    assert_eq!(run.final_tick(), 12);
    assert_eq!(run.cpu_response_count(), 1);
    assert_eq!(run.directory_decision_count(), 1);
    assert_eq!(run.dram_access_count(), 0);
    assert_eq!(run.summary().executed_events(), run.executed_events());
    assert_eq!(run.executed_events(), run.dispatch_count());
    assert_eq!(run.profile().epoch_count(), run.epoch_count());
    assert_eq!(run.profile().dispatch_count(), run.dispatch_count());
    assert_eq!(run.profile().batch_count(), run.batch_count());
    assert_eq!(run.scheduler_epochs().len(), run.epoch_count());
    assert_eq!(run.dispatches().len(), run.dispatch_count());
    assert_eq!(run.batches().len(), run.batch_count());
    assert_eq!(run.scheduler_run().dispatch_count(), run.dispatch_count());
    assert_eq!(run.scheduler_run().batch_count(), run.batch_count());
    assert_eq!(
        run.parallel_worker_partitions().len(),
        run.total_parallel_workers()
    );
    assert_eq!(run.protocol_activity_count(), 2);
    assert_eq!(
        run.profile(),
        ParallelRunProfile::new(
            run.epoch_count(),
            run.empty_epoch_count(),
            run.batch_count(),
            run.dispatch_count(),
            run.total_parallel_workers(),
            run.max_parallel_workers(),
        )
    );
    assert!(run.has_parallel_work());
    assert!(run.has_directory_activity());
    assert!(!run.has_dram_activity());
    assert_eq!(
        run.scheduler_run().profile().dispatch_count(),
        run.dispatch_count()
    );
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Modified);
}

#[test]
fn partitioned_moesi_harness_recorded_parallel_run_counts_only_scheduled_window_activity() {
    let mut harness = partitioned_harness();

    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    let first_run = harness.run_until_idle_parallel_recorded().unwrap();
    assert_eq!(first_run.cpu_response_count(), 1);
    assert_eq!(first_run.directory_decision_count(), 1);

    let hit = harness
        .submit_cpu_request_parallel(agent(1), read(1, 1, 0x5002, 2))
        .unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    let idle_run = harness.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(idle_run.summary().epochs(), 0);
    assert_eq!(idle_run.summary().executed_events(), 0);
    assert_eq!(idle_run.final_tick(), 12);
    assert_eq!(idle_run.cpu_response_count(), 0);
    assert_eq!(idle_run.directory_decision_count(), 0);
    assert_eq!(idle_run.dram_access_count(), 0);
    assert_eq!(idle_run.protocol_activity_count(), 0);
    assert_eq!(idle_run.profile(), ParallelRunProfile::default());
    assert!(!idle_run.has_parallel_work());
    assert!(!idle_run.has_directory_activity());
    assert!(!idle_run.has_dram_activity());
    assert_eq!(harness.cpu_responses().len(), 2);
}

#[test]
fn partitioned_moesi_harness_recorded_parallel_run_reports_dram_activity() {
    let mut harness = partitioned_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x5002, vec![0xaa, 0xbb]))
        .unwrap();
    let run = harness.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.cpu_response_count(), 1);
    assert_eq!(run.directory_decision_count(), 1);
    assert_eq!(run.dram_access_count(), 1);
    assert_eq!(run.summary().executed_events(), run.executed_events());
    assert_eq!(run.executed_events(), run.dispatch_count());
    assert_eq!(run.protocol_activity_count(), 3);
    assert_eq!(run.profile().dispatch_count(), run.dispatch_count());
    assert_eq!(run.profile().batch_count(), run.batch_count());
    assert!(run.final_tick() > 0);
    assert!(run.has_parallel_work());
    assert!(run.has_directory_activity());
    assert!(run.has_dram_activity());
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MoesiState::Modified);
    assert_eq!(harness.dram_memory_accesses().len(), 1);
}
