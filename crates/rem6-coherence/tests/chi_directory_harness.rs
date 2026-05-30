use rem6_cache::ChiCacheControllerResultKind;
use rem6_coherence::{
    ChiCpuResponseRecord, ChiDirectoryLineHarness, ChiDirectoryLineHarnessSnapshot,
    ChiHarnessError, HarnessError, LineBackingStore, PartitionedCacheAgentConfig,
    PartitionedChiDirectoryLineHarness, PartitionedChiDirectoryLineHarnessSnapshot,
    PartitionedRouteHopConfig, SubmitKind,
};
use rem6_directory::{ChiDirectoryDataSource, ChiDirectoryLineState, ChiDirectorySnoop};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_chi::{ChiEvent, ChiLineId, ChiState};
use rem6_transport::{MemoryTraceKind, TransportEndpointId};

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

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn cache_config(
    agent_id: u32,
    partition: u32,
    endpoint_name: &str,
    request_latency: u64,
    response_latency: u64,
) -> PartitionedCacheAgentConfig {
    PartitionedCacheAgentConfig::new(
        agent(agent_id),
        PartitionId::new(partition),
        endpoint(endpoint_name),
        request_latency,
        response_latency,
    )
}

fn route_hop(
    partition: u32,
    endpoint_name: &str,
    request_latency: u64,
    response_latency: u64,
) -> PartitionedRouteHopConfig {
    PartitionedRouteHopConfig::new(
        PartitionId::new(partition),
        endpoint(endpoint_name),
        request_latency,
        response_latency,
    )
}

fn partitioned_harness() -> PartitionedChiDirectoryLineHarness {
    PartitionedChiDirectoryLineHarness::new(
        layout(),
        Address::new(0x6000),
        LineBackingStore::new(layout(), Address::new(0x6000), line_data()).unwrap(),
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

fn partitioned_harness_with_hop() -> PartitionedChiDirectoryLineHarness {
    PartitionedChiDirectoryLineHarness::new(
        layout(),
        Address::new(0x6000),
        LineBackingStore::new(layout(), Address::new(0x6000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [cache_config(1, 0, "l1d0", 0, 0)
            .with_route_hops([route_hop(1, "noc0", 2, 6), route_hop(2, "dir0", 3, 8)])],
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

#[test]
fn chi_harness_restore_rejects_backing_line_mismatch_without_mutation() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), write(1, 0, 0x6002, vec![0xaa]))
        .unwrap();
    let snapshot = source.snapshot();
    let bad_snapshot = ChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.directory().clone(),
        snapshot.caches().clone(),
        LineBackingStore::new(layout(), Address::new(0x7000), line_data()).unwrap(),
        snapshot.cpu_responses().to_vec(),
        snapshot.directory_decisions().to_vec(),
    );

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(2), write(2, 9, 0x6004, vec![0xdd]))
        .unwrap();
    let before = restored.snapshot();

    assert_eq!(
        restored.restore(&bad_snapshot).unwrap_err(),
        ChiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x6000),
            actual: Address::new(0x7000),
        })
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn chi_harness_restore_rejects_directory_line_mismatch_without_mutation() {
    let mut source = harness();
    source
        .submit_cpu_request(agent(1), write(1, 0, 0x6002, vec![0xaa]))
        .unwrap();
    let snapshot = source.snapshot();
    let bad_snapshot = ChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        ChiDirectoryLineState::new(ChiLineId::new(Address::new(0x7000))),
        snapshot.caches().clone(),
        snapshot.backing().clone(),
        snapshot.cpu_responses().to_vec(),
        snapshot.directory_decisions().to_vec(),
    );

    let mut restored = harness();
    restored
        .submit_cpu_request(agent(2), write(2, 9, 0x6004, vec![0xdd]))
        .unwrap();
    let before = restored.snapshot();

    assert_eq!(
        restored.restore(&bad_snapshot).unwrap_err(),
        ChiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x6000),
            actual: Address::new(0x7000),
        })
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn partitioned_chi_harness_waits_for_owner_snoop_before_peer_fill() {
    let mut harness = partitioned_harness();

    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x6002, vec![0xaa, 0xbb]))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 12);
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        ChiState::UniqueDirty
    );

    harness
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x6000, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 26);
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        ChiState::SharedClean
    );
    assert_eq!(
        harness.cache_state(agent(2)).unwrap(),
        ChiState::SharedClean
    );
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            26,
            ChiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
    let decisions = harness.directory_decisions();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[1].tick(), 17);
    assert_eq!(decisions[1].requester(), agent(2));
    assert_eq!(
        decisions[1].decision().snoops(),
        &[ChiDirectorySnoop::new(agent(1), ChiEvent::SnoopShared)]
    );
}

#[test]
fn partitioned_chi_harness_quiescent_snapshot_restores_state() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x6002, vec![0xaa, 0xbb]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    source
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x6000, 4))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();

    let mut restored = partitioned_harness();
    restored
        .submit_cpu_request_parallel(agent(3), write(3, 7, 0x6001, vec![0xee]))
        .unwrap();
    restored.run_until_idle_parallel().unwrap();
    assert_ne!(restored.quiescent_snapshot().unwrap(), snapshot);

    restored.restore_quiescent(&snapshot).unwrap();
    assert_eq!(restored.quiescent_snapshot().unwrap(), snapshot);
    assert_eq!(
        restored.directory_state(),
        ChiDirectoryLineState::new(line())
            .with_sharer(agent(1), ChiState::SharedClean)
            .with_sharer(agent(2), ChiState::SharedClean)
    );

    let hit = restored
        .submit_cpu_request_parallel(agent(2), read(2, 9, 0x6000, 4))
        .unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            snapshot.scheduler().now(),
            ChiCacheControllerResultKind::Hit,
            request_id(2, 9),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
}

#[test]
fn partitioned_chi_harness_quiescent_restore_rejects_backing_line_mismatch_without_mutation() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x6002, vec![0xaa]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    let bad_snapshot = PartitionedChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        snapshot.directory().clone(),
        snapshot.caches().clone(),
        LineBackingStore::new(layout(), Address::new(0x7000), line_data()).unwrap(),
        snapshot.dram_memory().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
    );

    let mut restored = partitioned_harness();
    let before = restored.quiescent_snapshot().unwrap();

    assert_eq!(
        restored.restore_quiescent(&bad_snapshot).unwrap_err(),
        ChiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x6000),
            actual: Address::new(0x7000),
        })
    );
    assert_eq!(restored.quiescent_snapshot().unwrap(), before);
}

#[test]
fn partitioned_chi_harness_quiescent_restore_rejects_directory_line_mismatch_without_mutation() {
    let mut source = partitioned_harness();
    source
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x6002, vec![0xaa]))
        .unwrap();
    source.run_until_idle_parallel().unwrap();
    let snapshot = source.quiescent_snapshot().unwrap();
    let bad_snapshot = PartitionedChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        ChiDirectoryLineState::new(ChiLineId::new(Address::new(0x7000))),
        snapshot.caches().clone(),
        snapshot.backing().clone(),
        snapshot.dram_memory().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
    );

    let mut restored = partitioned_harness();
    let before = restored.quiescent_snapshot().unwrap();

    assert_eq!(
        restored.restore_quiescent(&bad_snapshot).unwrap_err(),
        ChiHarnessError::Backing(HarnessError::WrongLine {
            expected: Address::new(0x6000),
            actual: Address::new(0x7000),
        })
    );
    assert_eq!(restored.quiescent_snapshot().unwrap(), before);
}

#[test]
fn partitioned_chi_harness_records_multihop_response_trace() {
    let mut harness = partitioned_harness_with_hop();

    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x6002, vec![0xaa, 0xbb]))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 19);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            19,
            ChiCacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            None,
        ))
    );

    let route = harness.route(agent(1)).unwrap();
    let trace = harness.trace();
    assert!(trace.iter().all(|event| event.route() == route));
    assert_eq!(
        trace
            .iter()
            .map(|event| (
                event.tick(),
                event.endpoint().as_str().to_owned(),
                event.kind()
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "l1d0".to_owned(), MemoryTraceKind::RequestSent),
            (2, "noc0".to_owned(), MemoryTraceKind::RequestArrived),
            (5, "dir0".to_owned(), MemoryTraceKind::RequestArrived),
            (13, "noc0".to_owned(), MemoryTraceKind::ResponseArrived),
            (19, "l1d0".to_owned(), MemoryTraceKind::ResponseArrived),
        ]
    );
    assert_eq!(
        trace
            .iter()
            .filter(|event| event.kind() == MemoryTraceKind::ResponseArrived)
            .map(|event| event.response_status())
            .collect::<Vec<_>>(),
        vec![
            Some(ResponseStatus::Completed),
            Some(ResponseStatus::Completed)
        ]
    );
}
