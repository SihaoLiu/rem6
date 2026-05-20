use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{
    CoherentLineHarness, CpuResponseRecord, HarnessError, LineBackingStore, SubmitKind,
};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_msi::MsiState;
use rem6_transport::{MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(1), sequence)
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn read(sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn harness() -> CoherentLineHarness {
    CoherentLineHarness::new(
        AgentId::new(10),
        layout(),
        Address::new(0x1000),
        PartitionId::new(0),
        PartitionId::new(1),
        endpoint("l1d0"),
        endpoint("memory0"),
        3,
        5,
        LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap(),
    )
    .unwrap()
}

#[test]
fn harness_runs_read_miss_through_transport_and_fill() {
    let mut harness = harness();
    let request = read(1, 0x1004, 4);

    let submit = harness.submit_cpu_request(request.clone()).unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(submit.cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(harness.cache_state(), MsiState::InvalidToShared);

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 3);
    assert_eq!(run.final_tick(), 8);
    assert_eq!(harness.cache_state(), MsiState::Shared);

    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            8,
            CacheControllerResultKind::Fill,
            request.id(),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );

    let route = harness.route();
    assert_eq!(
        harness.trace(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                endpoint("l1d0"),
                MemoryTraceKind::RequestSent,
                MemoryRequestId::new(AgentId::new(10), 0),
            ),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                MemoryRequestId::new(AgentId::new(10), 0),
            ),
            MemoryTraceEvent::response(
                8,
                route,
                endpoint("l1d0"),
                MemoryRequestId::new(AgentId::new(10), 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn harness_upgrades_shared_line_for_store_then_serves_modified_hit() {
    let mut harness = harness();

    harness.submit_cpu_request(read(1, 0x1000, 8)).unwrap();
    harness.run_until_idle();
    assert_eq!(harness.cache_state(), MsiState::Shared);

    let write = write(2, 0x1006, vec![0xaa, 0xbb]);
    let upgrade = harness.submit_cpu_request(write.clone()).unwrap();
    assert_eq!(upgrade.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(harness.cache_state(), MsiState::SharedToModified);
    harness.run_until_idle();
    assert_eq!(harness.cache_state(), MsiState::Modified);

    let read_back = read(3, 0x1004, 6);
    let hit = harness.submit_cpu_request(read_back.clone()).unwrap();
    assert_eq!(hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(hit.cache_result(), CacheControllerResultKind::Hit);

    assert_eq!(
        harness.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            16,
            CacheControllerResultKind::Hit,
            read_back.id(),
            ResponseStatus::Completed,
            Some(vec![4, 5, 0xaa, 0xbb, 8, 9]),
        ))
    );
}

#[test]
fn harness_rejects_new_cpu_request_while_miss_is_pending() {
    let mut harness = harness();

    harness.submit_cpu_request(read(1, 0x1000, 4)).unwrap();
    let error = harness.submit_cpu_request(read(2, 0x1008, 4)).unwrap_err();

    assert_eq!(
        error,
        HarnessError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );
}

#[test]
fn backing_store_rejects_wrong_line_and_bad_line_data() {
    assert_eq!(
        LineBackingStore::new(layout(), Address::new(0x1000), vec![0; 32]).unwrap_err(),
        HarnessError::LineDataSizeMismatch {
            expected: 64,
            actual: 32,
        }
    );

    let mut store = LineBackingStore::new(layout(), Address::new(0x1000), line_data()).unwrap();
    let wrong = read(3, 0x2000, 4);

    assert_eq!(
        store.respond(&wrong).unwrap_err(),
        HarnessError::WrongLine {
            expected: Address::new(0x1000),
            actual: Address::new(0x2000),
        }
    );
}
