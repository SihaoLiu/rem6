use rem6_cache::MesiCacheControllerResultKind;
use rem6_coherence::{
    DramMemoryAccessRecord, HarnessError, LineBackingStore, MesiCpuResponseRecord,
    MesiDirectoryLineHarness, MesiDirectoryLineHarnessSnapshot, MesiHarnessError,
    PartitionedCacheAgentConfig, PartitionedDramMemoryConfig, PartitionedMemoryConfig,
    PartitionedMesiDirectoryLineHarness, PartitionedMesiDirectoryLineHarnessSnapshot,
    PartitionedRouteHopConfig, SubmitKind,
};
use rem6_directory::{MesiDirectoryDataSource, MesiDirectoryLineState, MesiDirectorySnoop};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_kernel::{ParallelRunProfile, PartitionId, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, ResponseStatus,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::{MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MesiLineId {
    MesiLineId::new(Address::new(0x3000))
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

fn partitioned_harness_with_two_caches() -> PartitionedMesiDirectoryLineHarness {
    PartitionedMesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x3000),
        LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [
            cache_config(1, 0, "l1d0", 3, 9),
            cache_config(2, 1, "l1d1", 5, 7),
        ],
    )
    .unwrap()
}

fn partitioned_harness_with_memory() -> PartitionedMesiDirectoryLineHarness {
    PartitionedMesiDirectoryLineHarness::new_with_memory(
        layout(),
        Address::new(0x3000),
        LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedMemoryConfig::new(PartitionId::new(4), endpoint("mem0"), 7, 11),
        [
            cache_config(1, 0, "l1d0", 3, 9),
            cache_config(2, 1, "l1d1", 5, 7),
            cache_config(3, 3, "l1d2", 2, 4),
        ],
    )
    .unwrap()
}

fn partitioned_harness_with_dram_memory() -> PartitionedMesiDirectoryLineHarness {
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
        .insert_line(target, Address::new(0x3000), line_data())
        .unwrap();

    PartitionedMesiDirectoryLineHarness::new_with_dram_memory(
        layout(),
        Address::new(0x3000),
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

fn partitioned_harness_with_dram_memory_hops() -> PartitionedMesiDirectoryLineHarness {
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
        .insert_line(target, Address::new(0x3000), line_data())
        .unwrap();

    PartitionedMesiDirectoryLineHarness::new_with_dram_memory(
        layout(),
        Address::new(0x3000),
        PartitionId::new(2),
        endpoint("dir0"),
        PartitionedDramMemoryConfig::new(PartitionId::new(4), endpoint("mem0"), 12, 15, memory)
            .with_route_hops([
                PartitionedRouteHopConfig::new(PartitionId::new(6), endpoint("mesh_r1"), 4, 6),
                PartitionedRouteHopConfig::new(PartitionId::new(4), endpoint("mem0"), 8, 9),
            ]),
        [
            cache_config(1, 0, "l1d0", 7, 10).with_route_hops([
                PartitionedRouteHopConfig::new(PartitionId::new(5), endpoint("mesh_r0"), 2, 3),
                PartitionedRouteHopConfig::new(PartitionId::new(2), endpoint("dir0"), 5, 7),
            ]),
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

#[test]
fn partitioned_mesi_harness_runs_parallel_epochs_for_peer_downgrade() {
    let mut harness = partitioned_harness();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 12);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);

    harness
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Modified);

    harness
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();

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
}

#[test]
fn partitioned_mesi_harness_recorded_parallel_run_reports_protocol_activity() {
    let mut harness = partitioned_harness();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
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
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
}

#[test]
fn partitioned_mesi_harness_recorded_parallel_run_counts_only_scheduled_window_activity() {
    let mut harness = partitioned_harness();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    let first_run = harness.run_until_idle_parallel_recorded().unwrap();
    assert_eq!(first_run.cpu_response_count(), 1);
    assert_eq!(first_run.directory_decision_count(), 1);

    let hit = harness
        .submit_cpu_request_parallel(agent(1), read(1, 1, 0x3004, 4))
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
fn partitioned_mesi_harness_recorded_parallel_run_reports_peer_snoop_activity() {
    let mut harness = partitioned_harness();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    harness.run_until_idle_parallel_recorded().unwrap();
    harness
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();

    harness
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.final_tick(), 26);
    assert_eq!(run.cpu_response_count(), 1);
    assert_eq!(run.directory_decision_count(), 1);
    assert_eq!(run.dram_access_count(), 0);
    assert_eq!(run.summary().executed_events(), run.executed_events());
    assert_eq!(run.executed_events(), run.dispatch_count());
    assert_eq!(run.protocol_activity_count(), 2);
    assert_eq!(run.profile().dispatch_count(), run.dispatch_count());
    assert!(run.has_parallel_work());
    assert!(run.has_directory_activity());
    assert!(!run.has_dram_activity());
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MesiState::Shared);
}

#[test]
fn mesi_harness_snapshot_restore_reinstates_serial_state() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    source
        .submit_cpu_request(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    let snapshot = source.snapshot();

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(3), write(3, 4, 0x3000, vec![0x99]))
        .unwrap();
    assert_ne!(restored.snapshot(), snapshot);

    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.directory_state(),
        MesiDirectoryLineState::new(line())
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );

    let store = restored
        .submit_cpu_request(agent(2), write(2, 9, 0x3001, vec![0xcc]))
        .unwrap();
    assert_eq!(store.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        store.directory_decision().unwrap().snoops(),
        &[MesiDirectorySnoop::new(agent(1), MesiEvent::SnoopWrite)]
    );

    let local_hit = restored
        .submit_cpu_request(agent(2), read(2, 10, 0x3000, 4))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            0,
            MesiCacheControllerResultKind::Hit,
            request_id(2, 10),
            ResponseStatus::Completed,
            Some(vec![0, 0xcc, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn mesi_harness_restore_rejects_backing_line_mismatch_without_mutation() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), write(1, 0, 0x3002, vec![0xaa]))
        .unwrap();
    let snapshot = source.snapshot();
    let bad_snapshot = MesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.directory().clone(),
        snapshot.caches().clone(),
        LineBackingStore::new(layout(), Address::new(0x4000), line_data()).unwrap(),
        snapshot.cpu_responses().to_vec(),
        snapshot.directory_decisions().to_vec(),
    );

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(2), write(2, 9, 0x3004, vec![0xdd]))
        .unwrap();
    let before = restored.snapshot();

    assert_eq!(
        restored.restore(&bad_snapshot).unwrap_err(),
        MesiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x3000),
            actual: Address::new(0x4000),
        })
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn mesi_harness_restore_rejects_directory_line_mismatch_without_mutation() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), write(1, 0, 0x3002, vec![0xaa]))
        .unwrap();
    let snapshot = source.snapshot();
    let bad_snapshot = MesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        MesiDirectoryLineState::new(MesiLineId::new(Address::new(0x4000))),
        snapshot.caches().clone(),
        snapshot.backing().clone(),
        snapshot.cpu_responses().to_vec(),
        snapshot.directory_decisions().to_vec(),
    );

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(2), write(2, 9, 0x3004, vec![0xdd]))
        .unwrap();
    let before = restored.snapshot();

    assert_eq!(
        restored.restore(&bad_snapshot).unwrap_err(),
        MesiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x3000),
            actual: Address::new(0x4000),
        })
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_restores_state() {
    let mut source = partitioned_harness_with_memory();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    source
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness_with_memory();
    restored
        .submit_cpu_request(agent(3), write(3, 7, 0x3001, vec![0xee]))
        .unwrap();
    restored.run_until_idle();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(
        restored.directory_state(),
        MesiDirectoryLineState::new(line())
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
        .submit_cpu_request(agent(2), read(2, 9, 0x3000, 4))
        .unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            snapshot.scheduler().now(),
            MesiCacheControllerResultKind::Hit,
            request_id(2, 9),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_restores_backing_after_downgrade() {
    let mut source = partitioned_harness_with_memory();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    source
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness_with_memory();
    restored.restore_quiescent(&snapshot).unwrap();
    restored
        .submit_cpu_request_parallel(agent(3), read(3, 1, 0x3000, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();

    assert_eq!(
        restored.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            68,
            MesiCacheControllerResultKind::Fill,
            request_id(3, 1),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_restores_exclusive_owner_state() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    assert_eq!(
        snapshot.directory(),
        &MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );

    let mut restored = partitioned_harness();
    restored
        .submit_cpu_request_parallel(agent(2), read(2, 7, 0x3000, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    let store = restored
        .submit_cpu_request(agent(1), write(1, 8, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    assert_eq!(store.kind(), SubmitKind::ImmediateHit);
    assert_eq!(restored.cache_state(agent(1)).unwrap(), MesiState::Modified);
    assert_eq!(
        restored.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_restores_dram_memory_state() {
    let mut source = partitioned_harness_with_dram_memory();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3004, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    assert!(snapshot.dram_memory().is_some());
    assert_eq!(snapshot.dram_accesses(), source.dram_memory_accesses());

    let mut expected = source;
    expected
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x3008, 4))
        .unwrap();
    expected.run_until_idle_parallel().unwrap();

    let mut restored = partitioned_harness_with_dram_memory();
    restored
        .submit_cpu_request(agent(2), write(2, 7, 0x3001, vec![0xee]))
        .unwrap();
    restored.run_until_idle();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(restored.dram_memory_accesses(), snapshot.dram_accesses());

    restored
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x3008, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_eq!(
        restored.quiescent_snapshot().unwrap(),
        expected.quiescent_snapshot().unwrap()
    );
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_restores_intermediate_hop_state() {
    let mut source = partitioned_harness_with_dram_memory_hops();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3004, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    assert_eq!(snapshot.trace(), source.trace());

    let mut expected = source;
    expected
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x3008, 4))
        .unwrap();
    expected.run_until_idle_parallel().unwrap();

    let mut restored = partitioned_harness_with_dram_memory_hops();
    restored
        .submit_cpu_request_parallel(agent(3), read(3, 9, 0x300c, 4))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    restored
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x3008, 4))
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
fn partitioned_mesi_harness_quiescent_snapshot_restores_parallel_run_history() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    let first = source.run_until_idle_parallel_recorded().unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 1, 0x3004, 4))
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
        history.total_protocol_activity(),
        first.protocol_activity_count() + second.protocol_activity_count()
    );
    assert!(history.has_parallel_work());
    assert!(history.has_directory_activity());
    assert!(!history.has_dram_activity());

    let mut restored = partitioned_harness();
    restored
        .submit_cpu_request_parallel(agent(3), write(3, 9, 0x300c, vec![0xdd]))
        .unwrap();
    restored.run_until_idle_parallel_recorded().unwrap();
    assert_ne!(restored.parallel_runs(), snapshot.parallel_runs());

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.parallel_runs(), &[first, second]);
    assert_eq!(restored.parallel_run_history(), history);
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_rejects_pending_events() {
    let mut harness = partitioned_harness();
    harness
        .submit_cpu_request(agent(1), read(1, 3, 0x3000, 4))
        .unwrap();

    assert_eq!(
        harness.quiescent_snapshot().unwrap_err(),
        MesiHarnessError::Scheduler(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: 1
        })
    );
}

#[test]
fn partitioned_mesi_harness_restore_quiescent_rejects_current_pending_events() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    restored
        .submit_cpu_request(agent(2), read(2, 7, 0x3000, 4))
        .unwrap();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        MesiHarnessError::Scheduler(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: 1
        })
    );
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_rejects_resource_mismatch() {
    let mut source = partitioned_harness_with_memory();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness_with_dram_memory();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        MesiHarnessError::SnapshotResourceMismatch { resource: "dram" }
    );
}

#[test]
fn partitioned_mesi_harness_quiescent_restore_rejects_backing_line_mismatch_without_mutation() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3002, vec![0xaa]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    let bad_snapshot = PartitionedMesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        snapshot.directory().clone(),
        snapshot.caches().clone(),
        LineBackingStore::new(layout(), Address::new(0x4000), line_data()).unwrap(),
        snapshot.dram_memory().cloned(),
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
        MesiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x3000),
            actual: Address::new(0x4000),
        })
    );
    assert_eq!(restored.quiescent_snapshot().unwrap(), before);
}

#[test]
fn partitioned_mesi_harness_quiescent_restore_rejects_directory_line_mismatch_without_mutation() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3002, vec![0xaa]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    let bad_snapshot = PartitionedMesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        MesiDirectoryLineState::new(MesiLineId::new(Address::new(0x4000))),
        snapshot.caches().clone(),
        snapshot.backing().clone(),
        snapshot.dram_memory().cloned(),
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
        MesiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x3000),
            actual: Address::new(0x4000),
        })
    );
    assert_eq!(restored.quiescent_snapshot().unwrap(), before);
}

#[test]
fn partitioned_mesi_harness_quiescent_snapshot_rejects_cache_shape_mismatch() {
    let mut source = partitioned_harness_with_two_caches();
    source
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    assert_eq!(
        restored.restore_quiescent(&snapshot).unwrap_err(),
        MesiHarnessError::UnknownCache { agent: agent(3) }
    );
}

#[test]
fn partitioned_mesi_harness_routes_backing_reads_through_memory_partition() {
    let mut harness = partitioned_harness_with_memory();

    harness
        .submit_cpu_request(agent(1), read(1, 0, 0x3000, 4))
        .unwrap();
    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 30);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );
    assert_eq!(
        harness.cpu_responses(),
        vec![MesiCpuResponseRecord::new(
            30,
            MesiCacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 2, 3]),
        )]
    );

    harness
        .submit_cpu_request(agent(1), write(1, 1, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 44);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Shared);
    assert_eq!(harness.cache_state(agent(2)).unwrap(), MesiState::Shared);

    harness
        .submit_cpu_request(agent(3), read(3, 0, 0x3000, 4))
        .unwrap();
    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 68);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&MesiCpuResponseRecord::new(
            68,
            MesiCacheControllerResultKind::Fill,
            request_id(3, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );

    let cache_route = harness.route(agent(1)).unwrap();
    let memory_route = harness.memory_route().unwrap();
    assert_eq!(
        &harness.trace()[..6],
        &[
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
                30,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn partitioned_mesi_harness_routes_dram_backing_reads_in_parallel_epochs() {
    let mut harness = partitioned_harness_with_dram_memory();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3004, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel_recorded().unwrap();
    assert_eq!(run.final_tick(), 38);
    assert_eq!(run.cpu_response_count(), 1);
    assert_eq!(run.directory_decision_count(), 1);
    assert_eq!(run.dram_access_count(), 1);
    assert_eq!(run.summary().executed_events(), run.executed_events());
    assert_eq!(run.executed_events(), run.dispatch_count());
    assert!(run.has_parallel_work());
    assert!(run.has_directory_activity());
    assert!(run.has_dram_activity());
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
    assert_eq!(
        harness.directory_state(),
        MesiDirectoryLineState::new(line()).with_owner(agent(1), MesiState::Exclusive)
    );
    assert_eq!(
        harness.cpu_responses(),
        vec![MesiCpuResponseRecord::new(
            38,
            MesiCacheControllerResultKind::Fill,
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
            12,
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
                38,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn partitioned_mesi_harness_routes_dram_backing_reads_through_intermediate_hops() {
    let mut harness = partitioned_harness_with_dram_memory_hops();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3004, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 52);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
    assert_eq!(
        harness.cpu_responses(),
        vec![MesiCpuResponseRecord::new(
            52,
            MesiCacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );
    assert_eq!(
        harness.dram_memory_accesses(),
        vec![DramMemoryAccessRecord::new(
            19,
            dram_target(),
            request_id(1, 0),
            0,
            12,
            false,
            27,
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
                2,
                cache_route,
                endpoint("mesh_r0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                7,
                cache_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                7,
                memory_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                11,
                memory_route,
                endpoint("mesh_r1"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                19,
                memory_route,
                endpoint("mem0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::response(
                36,
                memory_route,
                endpoint("mesh_r1"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                42,
                memory_route,
                endpoint("dir0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                49,
                cache_route,
                endpoint("mesh_r0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                52,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}
