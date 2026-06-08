use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AccessSize, AgentId, ByteMask, MemoryRequest, MemoryRequestId};
use rem6_protocol_mesi::MesiState;
use rem6_traffic::{
    TrafficTrace, TrafficTraceConfig, TrafficTraceEvent, TrafficTraceGenerator,
    TrafficTraceResponseKind,
};
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport, TransportEndpointId};
use rem6_workload::WorkloadRouteId;

use super::*;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
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

fn cache_config(agent: u32) -> PartitionedCacheAgentConfig {
    PartitionedCacheAgentConfig::new(
        AgentId::new(agent),
        PartitionId::new(agent),
        endpoint(&format!("l1d{agent}")),
        2,
        3,
    )
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

fn data_cache_config(protocol: WorkloadDataCacheProtocol) -> WorkloadRiscvDataCache {
    WorkloadRiscvDataCache::new(
        protocol,
        0,
        Address::new(0x3000),
        2,
        "dir0",
        WorkloadRouteId::new("memory").unwrap(),
    )
    .unwrap()
}

fn response_event(command: u32, address: u64, size: u32) -> TrafficTraceResponseEvent {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace_with_shape(1_000, command, address, size),
        1_000,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), layout(), 99, trace).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(0);
    match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Response(event) => event,
        event => panic!("unexpected trace event: {event:?}"),
    }
}

fn clean_shared_response_event() -> TrafficTraceResponseEvent {
    response_event(43, 0x3000, 64)
}

fn gem5_packet_trace_with_shape(
    tick_frequency: u64,
    command: u32,
    address: u64,
    size: u32,
) -> Vec<u8> {
    let mut bytes = vec![0x67, 0x65, 0x6d, 0x35];
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    let mut packet = Vec::new();
    append_key(&mut packet, 1, 0);
    append_varint(&mut packet, 4);
    append_key(&mut packet, 2, 0);
    append_varint(&mut packet, u64::from(command));
    append_key(&mut packet, 3, 0);
    append_varint(&mut packet, address);
    append_key(&mut packet, 4, 0);
    append_varint(&mut packet, u64::from(size));
    append_record(&mut bytes, &packet);

    bytes
}

fn append_record(bytes: &mut Vec<u8>, message: &[u8]) {
    append_varint(bytes, message.len() as u64);
    bytes.extend_from_slice(message);
}

fn append_key(bytes: &mut Vec<u8>, field: u32, wire_type: u8) {
    append_varint(bytes, (u64::from(field) << 3) | u64::from(wire_type));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

#[test]
fn trace_flush_mesi_harness_writes_dirty_owner_to_backing() {
    let mut harness = PartitionedMesiDirectoryLineHarness::new(
        layout(),
        Address::new(0x3000),
        LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap(),
        PartitionId::new(2),
        endpoint("dir0"),
        [cache_config(1)],
    )
    .unwrap();

    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3008, vec![0xaa, 0xbb]))
        .unwrap();
    harness.run_until_idle_parallel_recorded().unwrap();
    let before = harness.quiescent_snapshot().unwrap();
    assert_eq!(
        &before
            .caches()
            .get(&agent(1))
            .unwrap()
            .cached_data()
            .unwrap()[8..10],
        &[0xaa, 0xbb]
    );

    flush_mesi_harness(&mut harness, MemoryTargetId::new(0), Address::new(0x3000)).unwrap();

    let snapshot = harness.quiescent_snapshot().unwrap();
    assert_eq!(&snapshot.backing().data()[8..10], &[0xaa, 0xbb]);
    let cache = snapshot.caches().get(&agent(1)).unwrap();
    assert_eq!(cache.state(), MesiState::Invalid);
    assert!(cache.cached_data().is_none());
}

#[test]
fn trace_clean_shared_response_cleans_dirty_mesi_line_without_invalidating() {
    let mut backend = WorkloadDataCacheLineBackend::new(
        &data_cache_config(WorkloadDataCacheProtocol::Mesi),
        layout(),
        Address::new(0x3000),
        WorkloadDataCacheLineMemory::Line(line_data()),
        vec![cache_config(1)],
    )
    .unwrap();

    let WorkloadDataCacheHarness::Mesi(harness) = &mut backend.harness else {
        panic!("expected MESI backend harness");
    };
    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3008, vec![0xaa, 0xbb]))
        .unwrap();
    harness.run_until_idle_parallel_recorded().unwrap();
    let before = harness.quiescent_snapshot().unwrap();
    let before_cache = before.caches().get(&agent(1)).unwrap();
    assert_eq!(before_cache.state(), MesiState::Modified);
    assert_eq!(&before_cache.cached_data().unwrap()[8..10], &[0xaa, 0xbb]);

    let event = clean_shared_response_event();
    assert_eq!(event.kind(), TrafficTraceResponseKind::CleanShared);
    assert!(event.cleans_line());
    assert!(!event.invalidates_line());

    assert!(backend.apply_trace_response_event(event));

    let WorkloadDataCacheHarness::Mesi(harness) = &backend.harness else {
        panic!("expected MESI backend harness");
    };
    let snapshot = harness.quiescent_snapshot().unwrap();
    assert_eq!(&snapshot.backing().data()[8..10], &[0xaa, 0xbb]);
    let cache = snapshot.caches().get(&agent(1)).unwrap();
    assert_eq!(cache.state(), MesiState::Exclusive);
    assert_eq!(&cache.cached_data().unwrap()[8..10], &[0xaa, 0xbb]);
}

#[test]
fn data_cache_controller_error_keeps_delivery_context() {
    let backend = std::sync::Arc::new(std::sync::Mutex::new(WorkloadDataCacheBackend::new([
        WorkloadDataCacheLineBackend::new(
            &data_cache_config(WorkloadDataCacheProtocol::Msi),
            layout(),
            Address::new(0x3000),
            WorkloadDataCacheLineMemory::Line(line_data()),
            vec![cache_config(1)],
        )
        .unwrap(),
    ])));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu2"),
                PartitionId::new(0),
                endpoint("l1d2"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let request = write(2, 4, 0x3008, vec![0xcc]);

    let target_backend = std::sync::Arc::clone(&backend);
    transport
        .submit_parallel_at(
            &mut scheduler,
            11,
            route,
            request.clone(),
            MemoryTrace::new(),
            move |delivery, _context| {
                target_backend
                    .lock()
                    .unwrap()
                    .respond(&delivery)
                    .expect("delivery reaches configured data-cache line")
            },
            |_delivery| {},
        )
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    let error = backend.lock().unwrap().take_error().unwrap();
    let RiscvWorkloadReplayError::DataCacheController { record } = error else {
        panic!("expected contextual data-cache controller error");
    };
    assert_eq!(record.tick(), 14);
    assert_eq!(record.request_id(), request.id());
    assert_eq!(record.protocol(), RiscvDataCacheProtocol::Msi);
    assert_eq!(record.target(), MemoryTargetId::new(0));
    assert_eq!(record.address(), Address::new(0x3008));
    assert_eq!(record.line(), Address::new(0x3000));
    assert_eq!(record.operation(), request.operation());
    assert!(matches!(
        record.error(),
        RiscvDataCacheControllerError::Msi(rem6_coherence::HarnessError::UnknownCache { agent })
            if *agent == request.id().agent()
    ));
}
