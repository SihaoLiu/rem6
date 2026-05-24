use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::{CacheControllerResultKind, MshrQosClass, MshrQueueConfig, MsiCacheBank};
use rem6_coherence::{
    CpuResponseRecord, HarnessError, MsiBankDirectoryHarness, MsiBankDirectoryHarnessSnapshot,
    SubmitKind,
};
use rem6_directory::{DirectoryDataSource, DirectoryLineState};
use rem6_fabric::{QosFixedPriorityPolicy, QosPriority, QosQueueArbiter, QosQueuePolicyKind};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryTrace, MemoryTransport, TargetBatchOutcome, TargetOutcome,
    TransportEndpointId,
};

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn line(address: u64) -> MsiLineId {
    MsiLineId::new(Address::new(address))
}

fn request_id(agent_id: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(agent_id), sequence)
}

fn read(agent_id: u32, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn write(agent_id: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = size(data.len() as u64);
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

fn line_data(byte: u8) -> Vec<u8> {
    vec![byte; layout().bytes() as usize]
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn backing_line_addresses(snapshot: &MsiBankDirectoryHarnessSnapshot) -> Vec<u64> {
    snapshot
        .backing_lines()
        .iter()
        .map(|line| line.line_address().get())
        .collect()
}

fn pending_bank_snapshot(
    cache_agent: AgentId,
    sequence: u64,
    address: u64,
    qos: MshrQosClass,
) -> rem6_cache::MsiCacheBankSnapshot {
    let mut bank = MsiCacheBank::new_with_mshr(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 3, 0).unwrap(),
    );
    bank.accept_cpu_request_with_qos(read(cache_agent.get(), sequence, address), qos)
        .unwrap();
    bank.snapshot()
}

#[test]
fn msi_bank_harness_reports_stable_live_indexes() {
    let mut harness =
        MsiBankDirectoryHarness::new(layout(), [agent(3), agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();

    assert_eq!(harness.cache_count(), 3);
    assert_eq!(harness.cache_agents(), vec![agent(1), agent(2), agent(3)]);
    assert_eq!(
        harness.backing_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
}

#[test]
fn msi_bank_harness_snapshot_exposes_stable_indexes() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(2), agent(1)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 90, 0x1018))
        .unwrap();
    harness
        .submit_cpu_request(agent(1), read(1, 91, 0x1004))
        .unwrap();

    let snapshot = harness.snapshot();
    assert_eq!(snapshot.cache_count(), 2);
    assert_eq!(snapshot.directory_line_count(), 2);
    assert_eq!(snapshot.backing_line_count(), 2);
    assert_eq!(snapshot.cache_agents(), vec![agent(1), agent(2)]);
    assert!(snapshot.cache_snapshot(agent(1)).is_some());
    assert!(snapshot.cache_snapshot(agent(2)).is_some());
    assert!(snapshot.cache_snapshot(agent(3)).is_none());
    assert_eq!(
        snapshot.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let first_line = line_data(0x11);
    let second_line = line_data(0x22);
    assert_eq!(
        snapshot.backing_line(Address::new(0x1004)),
        Some(first_line.as_slice())
    );
    assert_eq!(
        snapshot.backing_line(Address::new(0x1018)),
        Some(second_line.as_slice())
    );
    assert_eq!(snapshot.backing_line(Address::new(0x2000)), None);
}

#[test]
fn msi_bank_harness_snapshot_aggregates_cache_mshr_qos_profiles() {
    let snapshot = MsiBankDirectoryHarnessSnapshot::new(
        layout(),
        BTreeMap::from([
            (
                agent(1),
                pending_bank_snapshot(agent(1), 10, 0x2004, MshrQosClass::new(20, 5)),
            ),
            (
                agent(2),
                pending_bank_snapshot(agent(2), 20, 0x2014, MshrQosClass::new(40, 1)),
            ),
        ]),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let profile = snapshot.mshr_qos_profile();
    assert_eq!(profile.entry_count(), 2);
    assert_eq!(profile.target_count(), 2);
    assert_eq!(profile.qos_target_count(), 2);
    assert_eq!(profile.effective_entry_count(), 2);
    assert_eq!(profile.priority_target_count(1), 1);
    assert_eq!(profile.priority_target_count(5), 1);
    assert_eq!(profile.requestor_target_count(20), 1);
    assert_eq!(profile.requestor_target_count(40), 1);
    assert_eq!(profile.effective_priority_entry_count(1), 1);
    assert_eq!(profile.effective_priority_entry_count(5), 1);
    assert_eq!(profile.effective_requestor_entry_count(20), 1);
    assert_eq!(profile.effective_requestor_entry_count(40), 1);
    assert_eq!(profile.best_effective_priority(), Some(1));
    assert!(profile.has_qos());
    assert_eq!(
        snapshot
            .cache_mshr_qos_profile(agent(2))
            .unwrap()
            .best_effective_priority(),
        Some(1)
    );
    assert!(snapshot.cache_mshr_qos_profile(agent(9)).is_none());

    let mut restored = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [agent(1), agent(2)],
        MshrQueueConfig::new(2, 3, 0).unwrap(),
    )
    .unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.mshr_qos_profile(), profile);
    assert_eq!(
        restored
            .cache_mshr_qos_profile(agent(1))
            .unwrap()
            .unwrap()
            .best_effective_priority(),
        Some(5)
    );
    assert!(restored.cache_mshr_qos_profile(agent(9)).is_err());
}

#[test]
fn msi_bank_harness_keeps_independent_lines_in_one_cache_bank() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    let first = harness
        .submit_cpu_request(agent(1), read(1, 10, 0x1004))
        .unwrap();
    assert_eq!(first.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(first.cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(
        first
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::BackingMemory,
    );

    let second = harness
        .submit_cpu_request(agent(1), read(1, 11, 0x1018))
        .unwrap();
    assert_eq!(second.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(second.cache_result(), CacheControllerResultKind::Miss);

    assert_eq!(
        harness.cache_line_addresses(agent(1)).unwrap(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1010)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000)).with_sharer(agent(1))
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1010)),
        DirectoryLineState::new(line(0x1010)).with_sharer(agent(1))
    );
    assert_eq!(
        harness.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].request(), request_id(1, 10));
    assert_eq!(responses[0].status(), ResponseStatus::Completed);
    assert_eq!(responses[0].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[1].request(), request_id(1, 11));
    assert_eq!(responses[1].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_coalesced_batch_records_all_mshr_targets() {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [agent(1)],
        MshrQueueConfig::new(2, 2, 0).unwrap(),
    )
    .unwrap();
    harness
        .insert_backing_line(Address::new(0x3000), line_data(0x66))
        .unwrap();

    let results = harness
        .submit_coalesced_cpu_requests(7, agent(1), [read(1, 100, 0x3004), read(1, 101, 0x3008)])
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].kind(), SubmitKind::ScheduledMiss);
    assert_eq!(results[0].cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(
        results[0]
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::BackingMemory
    );
    assert_eq!(results[1].kind(), SubmitKind::CoalescedMiss);
    assert_eq!(results[1].cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(harness.directory_decisions().len(), 1);
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x3000)).unwrap(),
        Some(MsiState::Shared)
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].tick(), 7);
    assert_eq!(responses[0].request(), request_id(1, 100));
    assert_eq!(responses[0].data().unwrap(), &[0x66; 8]);
    assert_eq!(responses[1].tick(), 7);
    assert_eq!(responses[1].request(), request_id(1, 101));
    assert_eq!(responses[1].data().unwrap(), &[0x66; 8]);
}

#[test]
fn msi_bank_harness_records_effective_mshr_qos_for_coalesced_targets() {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [agent(1)],
        MshrQueueConfig::new(2, 3, 0).unwrap(),
    )
    .unwrap();
    harness
        .insert_backing_line(Address::new(0x3010), line_data(0x77))
        .unwrap();

    let results = harness
        .submit_coalesced_cpu_requests_with_qos(
            13,
            agent(1),
            [
                (read(1, 110, 0x3014), MshrQosClass::new(20, 5)),
                (read(1, 111, 0x3018), MshrQosClass::new(40, 1)),
            ],
        )
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        results[0].cache_mshr_effective_qos(),
        Some(MshrQosClass::new(20, 5))
    );
    assert_eq!(results[1].kind(), SubmitKind::CoalescedMiss);
    assert_eq!(
        results[1].cache_mshr_effective_qos(),
        Some(MshrQosClass::new(40, 1))
    );
    assert_eq!(
        harness.cache_mshr_effective_qos(agent(1), Address::new(0x3010)),
        Ok(None)
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].tick(), 13);
    assert_eq!(responses[0].request(), request_id(1, 110));
    assert_eq!(responses[0].data().unwrap(), &[0x77; 8]);
    assert_eq!(responses[1].tick(), 13);
    assert_eq!(responses[1].request(), request_id(1, 111));
    assert_eq!(responses[1].data().unwrap(), &[0x77; 8]);
}

#[test]
fn msi_bank_harness_transfers_modified_owner_data_without_touching_other_lines() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    harness
        .submit_cpu_request(agent(1), write(1, 20, 0x1004, vec![0xaa; 8]))
        .unwrap();
    let shared = harness
        .submit_cpu_request(agent(2), read(2, 30, 0x1004))
        .unwrap();

    assert_eq!(shared.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        shared
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::ModifiedOwner(agent(1)),
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000))
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(2), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].request(), request_id(1, 20));
    assert_eq!(responses[0].data(), None);
    assert_eq!(responses[1].request(), request_id(2, 30));
    assert_eq!(responses[1].data().unwrap(), &[0xaa; 8]);
    assert_eq!(
        harness.backing_line(Address::new(0x1010)).unwrap(),
        line_data(0x22).as_slice()
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1010)),
        DirectoryLineState::new(line(0x1010))
    );
}

#[test]
fn msi_bank_harness_parallel_cycle_accepts_independent_lines_in_stable_order() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(2), agent(1)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();

    let run = harness
        .submit_parallel_cycle(
            17,
            [
                (agent(2), read(2, 20, 0x1018)),
                (agent(1), read(1, 10, 0x1004)),
            ],
        )
        .unwrap();

    assert_eq!(run.tick(), 17);
    assert_eq!(run.accepted_count(), 2);
    assert_eq!(
        run.accepted_lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(run.accepted()[0].agent(), agent(1));
    assert_eq!(run.accepted()[0].request(), request_id(1, 10));
    assert_eq!(run.accepted()[0].result().kind(), SubmitKind::ScheduledMiss);
    assert_eq!(run.accepted()[1].agent(), agent(2));
    assert_eq!(run.accepted()[1].request(), request_id(2, 20));

    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1004)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(2), Address::new(0x1018)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].tick(), 17);
    assert_eq!(responses[0].request(), request_id(1, 10));
    assert_eq!(responses[0].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[1].tick(), 17);
    assert_eq!(responses[1].request(), request_id(2, 20));
    assert_eq!(responses[1].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_parallel_cycle_records_mshr_qos_for_scheduled_misses() {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [agent(2), agent(1)],
        MshrQueueConfig::new(4, 2, 0).unwrap(),
    )
    .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x44))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x33))
        .unwrap();

    let run = harness
        .submit_parallel_cycle_with_qos(
            23,
            [
                (agent(2), read(2, 24, 0x1018), MshrQosClass::new(70, 2)),
                (agent(1), read(1, 14, 0x1004), MshrQosClass::new(50, 4)),
            ],
        )
        .unwrap();

    assert_eq!(run.accepted_count(), 2);
    assert_eq!(run.scheduled_miss_count(), 2);
    assert_eq!(run.accepted()[0].agent(), agent(1));
    assert_eq!(run.accepted()[0].request(), request_id(1, 14));
    assert_eq!(
        run.accepted()[0].result().cache_mshr_effective_qos(),
        Some(MshrQosClass::new(50, 4))
    );
    assert_eq!(run.accepted()[1].agent(), agent(2));
    assert_eq!(run.accepted()[1].request(), request_id(2, 24));
    assert_eq!(
        run.accepted()[1].result().cache_mshr_effective_qos(),
        Some(MshrQosClass::new(70, 2))
    );
    assert!(run.has_mshr_qos());
    assert_eq!(run.mshr_qos_accepted_count(), 2);
    assert_eq!(run.accepted_by_effective_mshr_qos_priority(2), 1);
    assert_eq!(run.accepted_by_effective_mshr_qos_priority(4), 1);
    assert_eq!(run.accepted_by_effective_mshr_qos_priority(7), 0);
    assert_eq!(
        run.accepted_by_effective_mshr_qos_priority_counts(),
        BTreeMap::from([(2, 1), (4, 1)])
    );
    assert_eq!(run.accepted_by_effective_mshr_qos_requestor(50), 1);
    assert_eq!(run.accepted_by_effective_mshr_qos_requestor(70), 1);
    assert_eq!(run.accepted_by_effective_mshr_qos_requestor(90), 0);
    assert_eq!(
        run.accepted_by_effective_mshr_qos_requestor_counts(),
        BTreeMap::from([(50, 1), (70, 1)])
    );
    assert_eq!(run.best_mshr_qos_priority(), Some(2));
    assert_eq!(
        harness.cache_mshr_effective_qos(agent(1), Address::new(0x1000)),
        Ok(None)
    );
    assert_eq!(
        harness.cache_mshr_effective_qos(agent(2), Address::new(0x1010)),
        Ok(None)
    );

    let snapshot = harness.snapshot();
    let rebuilt = MsiBankDirectoryHarnessSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(rebuilt, snapshot);
    let rebuilt_run = &rebuilt.parallel_cycle_runs()[0];
    assert_eq!(
        rebuilt_run.accepted()[0]
            .result()
            .cache_mshr_effective_qos(),
        Some(MshrQosClass::new(50, 4))
    );
    assert_eq!(
        rebuilt_run.accepted()[1]
            .result()
            .cache_mshr_effective_qos(),
        Some(MshrQosClass::new(70, 2))
    );

    let history = rebuilt.parallel_cycle_history();
    assert!(history.has_mshr_qos());
    assert_eq!(history.total_mshr_qos_accepted(), 2);
    assert_eq!(history.accepted_by_effective_mshr_qos_priority(2), 1);
    assert_eq!(history.accepted_by_effective_mshr_qos_priority(4), 1);
    assert_eq!(history.accepted_by_effective_mshr_qos_priority(7), 0);
    assert_eq!(history.accepted_by_effective_mshr_qos_requestor(50), 1);
    assert_eq!(history.accepted_by_effective_mshr_qos_requestor(70), 1);
    assert_eq!(history.mshr_qos_parallel_cycle_count(), 1);
    assert_eq!(history.best_mshr_qos_priority(), Some(2));
}

#[test]
fn msi_bank_harness_exports_mshr_qos_to_transport_miss_batch() {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [agent(2), agent(1)],
        MshrQueueConfig::new(4, 2, 0).unwrap(),
    )
    .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x44))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x33))
        .unwrap();
    let plan = harness
        .plan_parallel_cycle_with_qos(
            31,
            [
                (agent(2), read(2, 24, 0x1018), MshrQosClass::new(80, 0)),
                (agent(1), read(1, 14, 0x1004), MshrQosClass::new(90, 5)),
            ],
        )
        .unwrap();

    let transport_run = harness
        .submit_parallel_cycle_plan_for_transport_misses(plan)
        .unwrap();

    assert_eq!(transport_run.cycle_run().scheduled_miss_count(), 2);
    assert_eq!(transport_run.downstream_requests().len(), 2);
    assert_eq!(
        transport_run
            .downstream_requests()
            .iter()
            .map(|request| (
                request.agent(),
                request.line_address(),
                request.transport_qos_class().unwrap().requestor().get(),
                request.transport_qos_class().unwrap().priority().get(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (agent(1), Address::new(0x1000), 90, 5),
            (agent(2), Address::new(0x1010), 80, 0),
        ]
    );

    let delivery_log = Arc::new(Mutex::new(Vec::new()));
    let batch_log = Arc::clone(&delivery_log);
    let mut transport = MemoryTransport::with_qos_policy(
        QosQueueArbiter::new(QosQueuePolicyKind::Fifo),
        QosFixedPriorityPolicy::new(8, QosPriority::new(7)).unwrap(),
    )
    .with_direct_target_batch_responder(move |deliveries, _context| {
        let mut outcomes = Vec::with_capacity(deliveries.len());
        let mut batch = Vec::with_capacity(deliveries.len());
        for delivery in deliveries {
            batch.push((
                delivery.tick(),
                delivery.request().id().agent(),
                delivery.request().line_address(),
            ));
            outcomes.push(TargetBatchOutcome::new(
                delivery.request().id(),
                TargetOutcome::NoResponse,
            ));
        }
        batch_log.lock().unwrap().push(batch);
        Some(outcomes)
    });
    let route_agent_1 = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("cache1"),
                PartitionId::new(0),
                [MemoryRouteHop::new(endpoint("memory"), PartitionId::new(2), 2, 2).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
    let route_agent_2 = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("cache2"),
                PartitionId::new(1),
                [MemoryRouteHop::new(endpoint("memory"), PartitionId::new(2), 2, 2).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let transactions = transport_run
        .into_downstream_requests()
        .into_iter()
        .map(|request| {
            let route = match request.agent() {
                agent_id if agent_id == agent(1) => route_agent_1,
                agent_id if agent_id == agent(2) => route_agent_2,
                _ => unreachable!("test only uses two cache agents"),
            };
            request.into_parallel_transaction(
                route,
                MemoryTrace::new(),
                |_delivery, _context| TargetOutcome::NoResponse,
                |_| panic!("request-only miss transaction must not deliver a response"),
            )
        })
        .collect::<Vec<_>>();

    transport
        .submit_parallel_batch(&mut scheduler, transactions)
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        *delivery_log.lock().unwrap(),
        vec![vec![
            (2, agent(2), Address::new(0x1010)),
            (2, agent(1), Address::new(0x1000)),
        ]]
    );
}

#[test]
fn msi_bank_harness_parallel_cycle_plan_is_stable_and_side_effect_free() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(2), agent(1)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    let before = harness.snapshot();

    let plan = harness
        .plan_parallel_cycle(
            19,
            [
                (agent(2), read(2, 22, 0x1018)),
                (agent(1), read(1, 11, 0x1004)),
            ],
        )
        .unwrap();

    assert_eq!(plan.tick(), 19);
    assert_eq!(plan.entry_count(), 2);
    assert!(plan.has_parallel_work());
    assert_eq!(
        plan.lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(plan.entries()[0].agent(), agent(1));
    assert_eq!(plan.entries()[0].request(), request_id(1, 11));
    assert_eq!(plan.entries()[1].agent(), agent(2));
    assert_eq!(plan.entries()[1].request(), request_id(2, 22));
    assert_eq!(harness.snapshot(), before);

    let run = harness.submit_parallel_cycle_plan(plan).unwrap();

    assert_eq!(run.tick(), 19);
    assert_eq!(run.accepted_count(), 2);
    assert_eq!(run.response_count(), 2);
    assert_eq!(run.scheduled_miss_count(), 2);
    assert_eq!(run.immediate_hit_count(), 0);
    assert_ne!(harness.snapshot(), before);
}

#[test]
fn msi_bank_harness_snapshot_restore_preserves_parallel_cycle_history() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    let first = source
        .submit_parallel_cycle(
            41,
            [
                (agent(2), read(2, 20, 0x1018)),
                (agent(1), read(1, 10, 0x1004)),
            ],
        )
        .unwrap();
    let second = source
        .submit_parallel_cycle(
            47,
            [
                (agent(1), read(1, 11, 0x1004)),
                (agent(2), read(2, 21, 0x1018)),
            ],
        )
        .unwrap();

    assert_eq!(
        source.parallel_cycle_runs(),
        &[first.clone(), second.clone()]
    );
    let snapshot = source.snapshot();
    assert_eq!(snapshot.parallel_cycle_runs(), source.parallel_cycle_runs());

    let rebuilt = MsiBankDirectoryHarnessSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(rebuilt, snapshot);
    assert_eq!(
        rebuilt.parallel_cycle_runs(),
        &[first.clone(), second.clone()]
    );

    let mut restored = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    restored
        .insert_backing_line(Address::new(0x1000), line_data(0xee))
        .unwrap();
    restored
        .submit_parallel_cycle(53, [(agent(1), read(1, 30, 0x1004))])
        .unwrap();
    assert_ne!(restored.parallel_cycle_runs(), source.parallel_cycle_runs());

    restored.restore(&snapshot).unwrap();

    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.parallel_cycle_runs(), &[first, second]);
    assert_eq!(
        restored
            .parallel_cycle_runs()
            .iter()
            .map(|run| (run.tick(), run.accepted_count(), run.response_count()))
            .collect::<Vec<_>>(),
        vec![(41, 2, 2), (47, 2, 2)]
    );
}

#[test]
fn msi_bank_harness_parallel_cycle_history_summarizes_restored_records() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    source
        .submit_parallel_cycle(
            61,
            [
                (agent(2), read(2, 20, 0x1018)),
                (agent(1), read(1, 10, 0x1004)),
            ],
        )
        .unwrap();
    source
        .submit_parallel_cycle(
            67,
            [
                (agent(1), read(1, 11, 0x1004)),
                (agent(2), read(2, 21, 0x1018)),
            ],
        )
        .unwrap();
    let snapshot = source.snapshot();
    let restored = MsiBankDirectoryHarnessSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();

    let history = restored.parallel_cycle_history();

    assert_eq!(history.cycle_count(), 2);
    assert!(!history.is_empty());
    assert!(history.has_parallel_work());
    assert_eq!(history.parallel_cycle_count(), 2);
    assert_eq!(history.single_request_cycle_count(), 0);
    assert_eq!(history.ticks(), vec![61, 67]);
    assert_eq!(history.total_accepted(), 4);
    assert_eq!(history.total_responses(), 4);
    assert_eq!(history.total_immediate_hits(), 2);
    assert_eq!(history.total_scheduled_misses(), 2);
    assert_eq!(history.max_accepted_per_cycle(), 2);
    assert_eq!(
        history.touched_lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(history.accepted_by_tick(61), 2);
    assert_eq!(history.accepted_by_tick(67), 2);
    assert_eq!(history.accepted_by_tick(100), 0);
    assert_eq!(history.accepted_by_line(Address::new(0x1000)), 2);
    assert_eq!(history.accepted_by_line(Address::new(0x1010)), 2);
    assert_eq!(history.accepted_by_line(Address::new(0x2000)), 0);
    assert_eq!(history.accepted_by_agent(agent(1)), 2);
    assert_eq!(history.accepted_by_agent(agent(2)), 2);
    assert_eq!(history.accepted_by_agent(agent(3)), 0);
    assert_eq!(
        history.accepted_by_agent_counts(),
        BTreeMap::from([(agent(1), 2), (agent(2), 2)])
    );
    assert_eq!(
        history.accepted_by_line_counts(),
        BTreeMap::from([(Address::new(0x1000), 2), (Address::new(0x1010), 2)])
    );
    assert_eq!(
        history.accepted_by_tick_counts(),
        BTreeMap::from([(61, 2), (67, 2)])
    );
    assert_eq!(source.parallel_cycle_history(), history);

    let first_run = &restored.parallel_cycle_runs()[0];
    assert_eq!(first_run.agents(), vec![agent(1), agent(2)]);
    assert_eq!(
        first_run.requests(),
        vec![request_id(1, 10), request_id(2, 20)]
    );
    assert_eq!(
        first_run.lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert!(first_run.has_agent(agent(1)));
    assert!(first_run.has_agent(agent(2)));
    assert!(!first_run.has_agent(agent(3)));
    assert!(first_run.has_line(Address::new(0x1000)));
    assert!(first_run.has_line(Address::new(0x1010)));
    assert!(!first_run.has_line(Address::new(0x2000)));
}

#[test]
fn msi_bank_harness_parallel_cycle_reports_hit_and_miss_mix() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .submit_cpu_request(agent(1), read(1, 10, 0x1004))
        .unwrap();

    let run = harness
        .submit_parallel_cycle(
            29,
            [
                (agent(1), read(1, 11, 0x1004)),
                (agent(2), read(2, 20, 0x1018)),
            ],
        )
        .unwrap();

    assert!(run.has_parallel_work());
    assert_eq!(run.accepted_count(), 2);
    assert_eq!(run.immediate_hit_count(), 1);
    assert_eq!(run.scheduled_miss_count(), 1);
    assert_eq!(run.response_count(), 2);
    assert_eq!(run.accepted()[0].result().kind(), SubmitKind::ImmediateHit);
    assert_eq!(run.accepted()[1].result().kind(), SubmitKind::ScheduledMiss);

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1].tick(), 29);
    assert_eq!(responses[1].request(), request_id(1, 11));
    assert_eq!(responses[1].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[2].tick(), 29);
    assert_eq!(responses[2].request(), request_id(2, 20));
    assert_eq!(responses[2].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_parallel_cycle_allows_empty_work_without_mutation() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    let before = harness.snapshot();

    let plan = harness
        .plan_parallel_cycle(31, std::iter::empty::<(AgentId, MemoryRequest)>())
        .unwrap();

    assert_eq!(plan.tick(), 31);
    assert!(plan.is_empty());
    assert!(!plan.has_parallel_work());
    assert_eq!(plan.entry_count(), 0);
    assert!(plan.lines().is_empty());
    assert_eq!(harness.snapshot(), before);

    let run = harness.submit_parallel_cycle_plan(plan).unwrap();

    assert_eq!(run.tick(), 31);
    assert!(run.is_empty());
    assert!(!run.has_parallel_work());
    assert_eq!(run.accepted_count(), 0);
    assert_eq!(run.response_count(), 0);
    assert_eq!(harness.snapshot(), before);
    assert!(harness.cpu_responses().is_empty());
}

#[test]
fn msi_bank_harness_parallel_cycle_rejects_same_line_conflict_without_mutation() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    let before = harness.snapshot();

    assert_eq!(
        harness
            .submit_parallel_cycle(
                23,
                [
                    (agent(2), read(2, 20, 0x1008)),
                    (agent(1), write(1, 10, 0x1004, vec![0xaa; 8])),
                ],
            )
            .unwrap_err(),
        HarnessError::ParallelLineConflict {
            line: Address::new(0x1000),
            first: request_id(1, 10),
            second: request_id(2, 20),
        }
    );
    assert_eq!(harness.snapshot(), before);
    assert!(harness.cpu_responses().is_empty());
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000))
    );
}

#[test]
fn msi_bank_harness_snapshot_restore_reinstates_multi_line_state() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    source
        .submit_cpu_request(agent(1), write(1, 20, 0x1004, vec![0xaa; 8]))
        .unwrap();
    source
        .submit_cpu_request(agent(2), read(2, 30, 0x1004))
        .unwrap();
    source
        .submit_cpu_request(agent(1), read(1, 40, 0x1018))
        .unwrap();

    let snapshot = source.snapshot();
    assert_eq!(snapshot.layout(), layout());
    assert_eq!(snapshot.cache_snapshots().len(), 2);
    assert_eq!(
        snapshot.directory_states(),
        &[
            DirectoryLineState::new(line(0x1000))
                .with_sharer(agent(1))
                .with_sharer(agent(2)),
            DirectoryLineState::new(line(0x1010)).with_sharer(agent(1)),
        ]
    );
    assert_eq!(backing_line_addresses(&snapshot), vec![0x1000, 0x1010]);
    assert_eq!(snapshot.backing_lines()[0].data(), line_data(0x11));
    assert_eq!(snapshot.backing_lines()[1].data(), line_data(0x22));
    assert_eq!(snapshot.cpu_responses().len(), 3);
    assert_eq!(snapshot.directory_decisions().len(), 3);

    let mut restored = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    restored
        .insert_backing_line(Address::new(0x1000), line_data(0xee))
        .unwrap();
    restored
        .submit_cpu_request(agent(2), read(2, 50, 0x1004))
        .unwrap();
    assert_ne!(restored.snapshot(), snapshot);

    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.cache_line_addresses(agent(1)).unwrap(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(
        restored.cache_line_addresses(agent(2)).unwrap(),
        vec![Address::new(0x1000)]
    );
    assert_eq!(
        restored
            .cache_state(agent(1), Address::new(0x1000))
            .unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        restored
            .cache_state(agent(2), Address::new(0x1000))
            .unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        restored.backing_line(Address::new(0x1000)).unwrap(),
        line_data(0x11).as_slice()
    );

    let local_hit = restored
        .submit_cpu_request(agent(2), read(2, 60, 0x1004))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            0,
            CacheControllerResultKind::Hit,
            request_id(2, 60),
            ResponseStatus::Completed,
            Some(vec![0xaa; 8]),
        ))
    );

    let other_line_hit = restored
        .submit_cpu_request(agent(1), read(1, 61, 0x1018))
        .unwrap();
    assert_eq!(other_line_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last().unwrap().data().unwrap(),
        &[0x22; 8]
    );
}

#[test]
fn msi_bank_harness_restore_rejects_snapshot_layout_mismatch() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .submit_cpu_request(agent(1), read(1, 70, 0x1004))
        .unwrap();
    let snapshot = source.snapshot();

    let other_layout = CacheLineLayout::new(32).unwrap();
    let mut restored = MsiBankDirectoryHarness::new(other_layout, [agent(1)]).unwrap();

    assert_eq!(
        restored.restore(&snapshot).unwrap_err(),
        HarnessError::SnapshotResourceMismatch {
            resource: "msi bank directory harness layout",
        }
    );
}

#[test]
fn msi_bank_harness_restore_rejects_cache_set_mismatch() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .submit_cpu_request(agent(1), read(1, 80, 0x1004))
        .unwrap();
    let snapshot = source.snapshot();

    let mut restored = MsiBankDirectoryHarness::new(layout(), [agent(1)]).unwrap();

    assert_eq!(
        restored.restore(&snapshot).unwrap_err(),
        HarnessError::UnknownCache { agent: agent(2) }
    );
}
