use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuError, CpuFetchConfig, CpuFetchEvent, CpuFetchEventKind, CpuFetchRecord, CpuId,
    CpuResetState,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
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

#[test]
fn cpu_fetches_boot_entry_through_partitioned_transport() {
    let memory_target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(memory_target, layout()).unwrap();
    store
        .map_region(
            memory_target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    let image = BootImage::new(Address::new(0x8004))
        .add_segment(Address::new(0x8004), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap();
    image
        .load_into_partitioned_store(&mut store, memory_target)
        .unwrap();

    let store = Arc::new(Mutex::new(store));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let core_endpoint = endpoint("cpu0.icache");
    let memory_endpoint = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core_endpoint.clone(),
                PartitionId::new(0),
                memory_endpoint.clone(),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let trace = MemoryTrace::new();
    let core = CpuCore::new(
        CpuResetState::from_boot_image(CpuId::new(0), PartitionId::new(0), AgentId::new(7), &image),
        CpuFetchConfig::new(
            core_endpoint.clone(),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();

    let memory_store = Arc::clone(&store);
    let responder_endpoint = memory_endpoint.clone();
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        trace.clone(),
        move |delivery, _context| {
            assert_eq!(delivery.endpoint(), &responder_endpoint);
            assert_eq!(
                delivery.request().operation(),
                MemoryOperation::InstructionFetch
            );
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

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 8);
    assert_eq!(core.pc(), Address::new(0x8008));
    assert_eq!(core.next_sequence(), 1);
    assert_eq!(
        core.fetch_events(),
        vec![
            CpuFetchEvent::issued(CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                route,
                core_endpoint.clone(),
                MemoryRequestId::new(AgentId::new(7), 0),
                Address::new(0x8004),
                AccessSize::new(4).unwrap(),
            )),
            CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    8,
                    PartitionId::new(0),
                    route,
                    core_endpoint.clone(),
                    MemoryRequestId::new(AgentId::new(7), 0),
                    Address::new(0x8004),
                    AccessSize::new(4).unwrap(),
                ),
                vec![0x13, 0x05, 0x00, 0x00],
            ),
        ]
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                core_endpoint.clone(),
                MemoryTraceKind::RequestSent,
                core.fetch_events()[0].request_id(),
            ),
            MemoryTraceEvent::request(
                3,
                route,
                memory_endpoint,
                MemoryTraceKind::RequestArrived,
                core.fetch_events()[0].request_id(),
            ),
            MemoryTraceEvent::response(
                8,
                route,
                core_endpoint,
                core.fetch_events()[0].request_id(),
                rem6_memory::ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn cpu_rejects_fetch_that_crosses_cache_line() {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.icache"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(0x800e),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.icache"),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    assert_eq!(
        core.issue_next_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_, _| {
            panic!("cross-line fetch must not reach transport")
        })
        .unwrap_err(),
        CpuError::FetchCrossesLine {
            pc: Address::new(0x800e),
            size: AccessSize::new(4).unwrap(),
            line_size: 16,
        }
    );
    assert!(scheduler.is_idle());
    assert_eq!(core.pc(), Address::new(0x800e));
    assert_eq!(core.next_sequence(), 0);
    assert!(core.fetch_events().is_empty());
}

#[test]
fn cpu_rejects_route_that_does_not_start_at_cpu_partition() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(0x8000),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.icache"),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();

    assert_eq!(
        core.issue_next_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_, _| {
            panic!("mismatched route must not reach memory")
        })
        .unwrap_err(),
        CpuError::RoutePartitionMismatch {
            route,
            expected: PartitionId::new(0),
            actual: PartitionId::new(1),
        }
    );
    assert!(scheduler.is_idle());
    assert_eq!(
        core.fetch_events()
            .iter()
            .map(CpuFetchEvent::kind)
            .collect::<Vec<_>>(),
        Vec::<CpuFetchEventKind>::new()
    );
}
