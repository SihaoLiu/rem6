use rem6_cpu::{HtmFailureCause, HtmTransactionUid};
use rem6_kernel::PartitionId;
use rem6_memory::{Address, CacheLineLayout};
use rem6_traffic::{
    TrafficTrace, TrafficTraceConfig, TrafficTraceEvent, TrafficTraceGenerator,
    TrafficTraceHtmEvent, TrafficTraceResponseEvent,
};
use rem6_transport::{MemoryRouteId, TransportEndpointId};
use rem6_workload::{WorkloadDataCacheProtocol, WorkloadRiscvDataCache, WorkloadRouteId};

use super::{
    PartitionedCacheAgentConfig, WorkloadDataCacheBackend, WorkloadDataCacheLineBackend,
    WorkloadDataCacheLineMemory,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn cache_config(agent: u32) -> PartitionedCacheAgentConfig {
    PartitionedCacheAgentConfig::new(
        rem6_memory::AgentId::new(agent),
        PartitionId::new(agent),
        endpoint(&format!("l1d{agent}")),
        2,
        3,
    )
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

fn data_cache_backend() -> WorkloadDataCacheBackend {
    WorkloadDataCacheBackend::new([WorkloadDataCacheLineBackend::new(
        &data_cache_config(WorkloadDataCacheProtocol::Msi),
        layout(),
        Address::new(0x3000),
        WorkloadDataCacheLineMemory::Line(line_data()),
        vec![cache_config(1), cache_config(2)],
    )
    .unwrap()])
}

fn route(value: u64) -> MemoryRouteId {
    MemoryRouteId::new(value)
}

fn capture_trace_htm_rollback(
    backend: &mut WorkloadDataCacheBackend,
    route: MemoryRouteId,
    transaction_uid: HtmTransactionUid,
) -> bool {
    backend.capture_trace_htm_rollback_from_event(route, transaction_uid, 3, htm_request_event())
}

fn htm_request_event() -> TrafficTraceHtmEvent {
    let trace =
        TrafficTrace::from_gem5_packet_trace(&gem5_packet_trace_htm_request(), 1_000).unwrap();
    let config =
        TrafficTraceConfig::new(rem6_memory::AgentId::new(7), layout(), 99, trace).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(0);
    match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Htm(event) => event,
        event => panic!("unexpected trace event: {event:?}"),
    }
}

fn response_event(command: u32, address: u64, size: u32) -> TrafficTraceResponseEvent {
    let trace =
        TrafficTrace::from_gem5_packet_trace(&gem5_packet_trace(command, address, size), 1_000)
            .unwrap();
    let config =
        TrafficTraceConfig::new(rem6_memory::AgentId::new(7), layout(), 99, trace).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(0);
    match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Response(event) => event,
        event => panic!("unexpected trace event: {event:?}"),
    }
}

fn read_response_event() -> TrafficTraceResponseEvent {
    response_event(2, 0x3008, 8)
}

fn write_response_event() -> TrafficTraceResponseEvent {
    response_event(5, 0x3008, 8)
}

fn store_conditional_response_event() -> TrafficTraceResponseEvent {
    response_event(29, 0x3008, 8)
}

fn upgrade_response_event() -> TrafficTraceResponseEvent {
    response_event(19, 0x3000, 64)
}

fn read_exclusive_response_event() -> TrafficTraceResponseEvent {
    response_event(23, 0x3000, 64)
}

fn locked_rmw_read_response_event() -> TrafficTraceResponseEvent {
    response_event(31, 0x3008, 8)
}

fn gem5_packet_trace_htm_request() -> Vec<u8> {
    let mut bytes = vec![0x67, 0x65, 0x6d, 0x35];
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, 1_000);
    append_record(&mut bytes, &header);

    let mut packet = Vec::new();
    append_key(&mut packet, 1, 0);
    append_varint(&mut packet, 4);
    append_key(&mut packet, 2, 0);
    append_varint(&mut packet, 56);
    append_record(&mut bytes, &packet);

    bytes
}

fn gem5_packet_trace(command: u32, address: u64, size: u32) -> Vec<u8> {
    let mut bytes = vec![0x67, 0x65, 0x6d, 0x35];
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, 1_000);
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
fn external_cache_write_marks_reader_memory_conflict() {
    let mut backend = data_cache_backend();
    let reader_route = route(11);
    let writer_route = route(12);
    let reader_uid = HtmTransactionUid::new(1);

    assert!(capture_trace_htm_rollback(
        &mut backend,
        reader_route,
        reader_uid
    ));
    assert_eq!(
        backend
            .record_trace_htm_access_event(5, reader_route, reader_uid, read_response_event(), true)
            .len(),
        1
    );
    assert_eq!(
        backend.trace_htm_abort_cause(reader_route, reader_uid),
        HtmFailureCause::Other
    );

    assert!(backend.record_trace_htm_write_conflict_event(
        writer_route,
        None,
        write_response_event(),
        true
    ));

    assert_eq!(
        backend.trace_htm_abort_cause(reader_route, reader_uid),
        HtmFailureCause::Memory
    );
}

#[test]
fn failed_external_store_conditional_does_not_mark_memory_conflict() {
    let mut backend = data_cache_backend();
    let reader_route = route(11);
    let writer_route = route(12);
    let reader_uid = HtmTransactionUid::new(1);

    assert!(capture_trace_htm_rollback(
        &mut backend,
        reader_route,
        reader_uid
    ));
    assert_eq!(
        backend
            .record_trace_htm_access_event(5, reader_route, reader_uid, read_response_event(), true)
            .len(),
        1
    );

    assert!(!backend.record_trace_htm_write_conflict_event(
        writer_route,
        None,
        store_conditional_response_event(),
        false
    ));

    assert_eq!(
        backend.trace_htm_abort_cause(reader_route, reader_uid),
        HtmFailureCause::Other
    );
}

#[test]
fn active_writable_intent_responses_mark_reader_memory_conflict() {
    for (event, expected_records) in [
        (upgrade_response_event(), 1),
        (read_exclusive_response_event(), 2),
        (locked_rmw_read_response_event(), 2),
    ] {
        let mut backend = data_cache_backend();
        let reader_route = route(11);
        let writer_route = route(12);
        let reader_uid = HtmTransactionUid::new(1);
        let writer_uid = HtmTransactionUid::new(2);

        assert!(capture_trace_htm_rollback(
            &mut backend,
            reader_route,
            reader_uid
        ));
        assert_eq!(
            backend
                .record_trace_htm_access_event(
                    5,
                    reader_route,
                    reader_uid,
                    read_response_event(),
                    true
                )
                .len(),
            1
        );
        assert!(capture_trace_htm_rollback(
            &mut backend,
            writer_route,
            writer_uid
        ));
        assert_eq!(
            backend
                .record_trace_htm_access_event(7, writer_route, writer_uid, event, true)
                .len(),
            expected_records
        );

        assert_eq!(
            backend.trace_htm_abort_cause(reader_route, reader_uid),
            HtmFailureCause::Memory
        );
        assert_eq!(
            backend.trace_htm_abort_cause(writer_route, writer_uid),
            HtmFailureCause::Other
        );
    }
}

#[test]
fn external_writable_intent_responses_mark_reader_memory_conflict() {
    for event in [
        upgrade_response_event(),
        read_exclusive_response_event(),
        locked_rmw_read_response_event(),
    ] {
        let mut backend = data_cache_backend();
        let reader_route = route(11);
        let writer_route = route(12);
        let reader_uid = HtmTransactionUid::new(1);

        assert!(capture_trace_htm_rollback(
            &mut backend,
            reader_route,
            reader_uid
        ));
        assert_eq!(
            backend
                .record_trace_htm_access_event(
                    5,
                    reader_route,
                    reader_uid,
                    read_response_event(),
                    true
                )
                .len(),
            1
        );

        assert!(backend.record_trace_htm_write_conflict_event(writer_route, None, event, true));

        assert_eq!(
            backend.trace_htm_abort_cause(reader_route, reader_uid),
            HtmFailureCause::Memory
        );
    }
}

#[test]
fn dropped_trace_transaction_no_longer_contributes_conflicts() {
    let mut backend = data_cache_backend();
    let stale_route = route(11);
    let writer_route = route(12);
    let stale_uid = HtmTransactionUid::new(1);
    let writer_uid = HtmTransactionUid::new(2);

    assert!(capture_trace_htm_rollback(
        &mut backend,
        stale_route,
        stale_uid
    ));
    assert_eq!(
        backend
            .record_trace_htm_access_event(5, stale_route, stale_uid, read_response_event(), true)
            .len(),
        1
    );
    assert!(backend.discard_trace_htm_transaction(stale_route, stale_uid));

    assert!(capture_trace_htm_rollback(
        &mut backend,
        writer_route,
        writer_uid
    ));
    assert_eq!(
        backend
            .record_trace_htm_access_event(
                7,
                writer_route,
                writer_uid,
                write_response_event(),
                true
            )
            .len(),
        1
    );

    assert_eq!(
        backend.trace_htm_abort_cause(writer_route, writer_uid),
        HtmFailureCause::Other
    );
}

#[test]
fn active_writer_does_not_mark_itself_when_other_transaction_conflicts() {
    let mut backend = data_cache_backend();
    let reader_route = route(11);
    let writer_route = route(12);
    let reader_uid = HtmTransactionUid::new(1);
    let writer_uid = HtmTransactionUid::new(2);

    assert!(capture_trace_htm_rollback(
        &mut backend,
        reader_route,
        reader_uid
    ));
    assert_eq!(
        backend
            .record_trace_htm_access_event(5, reader_route, reader_uid, read_response_event(), true)
            .len(),
        1
    );
    assert!(capture_trace_htm_rollback(
        &mut backend,
        writer_route,
        writer_uid
    ));
    assert_eq!(
        backend
            .record_trace_htm_access_event(
                7,
                writer_route,
                writer_uid,
                write_response_event(),
                true
            )
            .len(),
        1
    );

    assert_eq!(
        backend.trace_htm_abort_cause(reader_route, reader_uid),
        HtmFailureCause::Memory
    );
    assert_eq!(
        backend.trace_htm_abort_cause(writer_route, writer_uid),
        HtmFailureCause::Other
    );
}
