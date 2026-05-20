use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCluster, CpuClusterError, CpuCore, CpuFetchConfig, CpuId, CpuResetState};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, TargetOutcome,
    TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn core(
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: u64,
    endpoint_name: &str,
    route: rem6_transport::MemoryRouteId,
) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint(endpoint_name),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

#[test]
fn cluster_issues_parallel_fetches_from_distinct_partitions() {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(0x8004))
        .add_segment(Address::new(0x8004), vec![0x11, 0x12, 0x13, 0x14])
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    BootImage::new(Address::new(0x9004))
        .add_segment(Address::new(0x9004), vec![0x21, 0x22, 0x23, 0x24])
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();

    let store = Arc::new(Mutex::new(store));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let memory = endpoint("memory0");
    let route0 = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.icache"),
                PartitionId::new(0),
                memory.clone(),
                PartitionId::new(2),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let route1 = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.icache"),
                PartitionId::new(1),
                memory.clone(),
                PartitionId::new(2),
                5,
                7,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu0 = core(0, 0, 10, 0x8004, "cpu0.icache", route0);
    let cpu1 = core(1, 1, 11, 0x9004, "cpu1.icache", route1);
    let cluster = CpuCluster::new([cpu0.clone(), cpu1.clone()]).unwrap();
    let trace = MemoryTrace::new();

    let memory_store = Arc::clone(&store);
    cluster
        .issue_next_fetch(
            CpuId::new(0),
            &mut scheduler,
            &transport,
            trace.clone(),
            move |delivery, _context| {
                let response = memory_store
                    .lock()
                    .unwrap()
                    .respond(delivery.request())
                    .unwrap()
                    .response()
                    .cloned()
                    .unwrap();
                TargetOutcome::Respond(response)
            },
        )
        .unwrap();
    let memory_store = Arc::clone(&store);
    cluster
        .issue_next_fetch(
            CpuId::new(1),
            &mut scheduler,
            &transport,
            trace.clone(),
            move |delivery, _context| {
                let response = memory_store
                    .lock()
                    .unwrap()
                    .respond(delivery.request())
                    .unwrap()
                    .response()
                    .cloned()
                    .unwrap();
                TargetOutcome::Respond(response)
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 6);
    assert_eq!(summary.final_tick(), 12);
    assert_eq!(cpu0.pc(), Address::new(0x8008));
    assert_eq!(cpu1.pc(), Address::new(0x9008));
    assert_eq!(cluster.core_ids(), vec![CpuId::new(0), CpuId::new(1)]);
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route0,
                endpoint("cpu0.icache"),
                MemoryTraceKind::RequestSent,
                cpu0.fetch_events()[0].request_id(),
            ),
            MemoryTraceEvent::request(
                0,
                route1,
                endpoint("cpu1.icache"),
                MemoryTraceKind::RequestSent,
                cpu1.fetch_events()[0].request_id(),
            ),
            MemoryTraceEvent::request(
                3,
                route0,
                memory.clone(),
                MemoryTraceKind::RequestArrived,
                cpu0.fetch_events()[0].request_id(),
            ),
            MemoryTraceEvent::request(
                5,
                route1,
                memory.clone(),
                MemoryTraceKind::RequestArrived,
                cpu1.fetch_events()[0].request_id(),
            ),
            MemoryTraceEvent::response(
                8,
                route0,
                endpoint("cpu0.icache"),
                cpu0.fetch_events()[0].request_id(),
                rem6_memory::ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                12,
                route1,
                endpoint("cpu1.icache"),
                cpu1.fetch_events()[0].request_id(),
                rem6_memory::ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn cluster_rejects_duplicate_cpu_agents_and_fetch_endpoints() {
    let mut transport = MemoryTransport::new();
    let route0 = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.icache"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(2),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let route1 = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.icache"),
                PartitionId::new(1),
                endpoint("memory0"),
                PartitionId::new(2),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        CpuCluster::new([
            core(0, 0, 10, 0x8000, "cpu0.icache", route0),
            core(0, 1, 11, 0x9000, "cpu1.icache", route1),
        ])
        .unwrap_err(),
        CpuClusterError::DuplicateCpu { cpu: CpuId::new(0) }
    );
    assert_eq!(
        CpuCluster::new([
            core(0, 0, 10, 0x8000, "cpu0.icache", route0),
            core(1, 1, 10, 0x9000, "cpu1.icache", route1),
        ])
        .unwrap_err(),
        CpuClusterError::DuplicateAgent {
            agent: AgentId::new(10),
            existing: CpuId::new(0),
            duplicate: CpuId::new(1),
        }
    );
    assert_eq!(
        CpuCluster::new([
            core(0, 0, 10, 0x8000, "cpu0.icache", route0),
            core(1, 1, 11, 0x9000, "cpu0.icache", route1),
        ])
        .unwrap_err(),
        CpuClusterError::DuplicateFetchEndpoint {
            endpoint: endpoint("cpu0.icache"),
            existing: CpuId::new(0),
            duplicate: CpuId::new(1),
        }
    );
}

#[test]
fn cluster_reports_unknown_cpu_without_scheduling() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let transport = MemoryTransport::new();
    let cluster = CpuCluster::new([core(
        0,
        0,
        10,
        0x8000,
        "cpu0.icache",
        rem6_transport::MemoryRouteId::new(7),
    )])
    .unwrap();

    assert_eq!(
        cluster
            .issue_next_fetch(
                CpuId::new(9),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |_, _| panic!("unknown CPU must not issue a transport request"),
            )
            .unwrap_err(),
        CpuClusterError::UnknownCpu { cpu: CpuId::new(9) }
    );
    assert!(scheduler.is_idle());
}
