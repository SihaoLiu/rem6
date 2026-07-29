use super::*;
use crate::pending_address_support::{
    data_endpoint, fetch_endpoint, request, seed_pending_address_core,
    seed_pending_address_core_with_offset, DATA_ROUTE, FETCH_ROUTE, LIVE_TICK,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, TargetOutcome,
    TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn pending_transport() -> MemoryTransport {
    let mut transport = MemoryTransport::new();
    let fetch = transport
        .add_route(
            MemoryRoute::new(
                fetch_endpoint(0),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data = transport
        .add_route(
            MemoryRoute::new(
                data_endpoint(0),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(fetch, FETCH_ROUTE);
    assert_eq!(data, DATA_ROUTE);
    transport
}

#[test]
fn pending_address_source_wake_is_excluded_and_rebound_once() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Parallel,
    );
    let (mut executor, cpus, scheduler_component) = attached_executor(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&checkpoint_record("pending-source"))
            .unwrap(),
    );
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(manifest_chunk(&manifest, &cpus[0], O3LC)).unwrap(),
        seeded.live
    );
    assert_eq!(
        rem6_cpu::O3RuntimeCheckpointPayload::decode(manifest_chunk(&manifest, &cpus[0], O3RT))
            .unwrap(),
        seeded.stable
    );

    let scheduler_payload = manifest_chunk(&manifest, &scheduler_component, "scheduler");
    assert_eq!(
        u64::from_le_bytes(
            scheduler_payload[scheduler_payload.len() - 8..]
                .try_into()
                .unwrap()
        ),
        0
    );
    assert_eq!(
        scheduler.lock().unwrap().snapshot().total_pending_events(),
        1
    );

    let outcome = executor.apply(&restore_record(manifest)).unwrap();
    let SystemActionOutcome::CheckpointRestored {
        rebound_o3_wake_components,
        ..
    } = outcome
    else {
        panic!("unexpected restore outcome: {outcome:?}");
    };
    assert_eq!(
        rebound_o3_wake_components,
        std::collections::BTreeSet::from([cpus[0].clone()])
    );
    let scheduler_snapshot = scheduler.lock().unwrap().snapshot();
    let [rebound] = scheduler_snapshot.partitions()[0].pending_events() else {
        panic!("pending-address restore must leave one rebound wake")
    };
    assert_ne!(rebound.id(), seeded.wake.id());
    assert_eq!(rebound.kind(), ScheduledEventKind::Parallel);
    assert_eq!(
        seeded.core.owned_o3_writeback_wakes(),
        [(scheduler.lock().unwrap().instance_id(), *rebound)]
    );
}

#[test]
fn pending_address_restore_replaces_destination_wake_and_fetch() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let source = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Serial,
    );
    let (mut source_executor, _, _) = attached_executor(&[&source.core], &scheduler);
    let manifest = captured_manifest(
        source_executor
            .apply(&checkpoint_record("pending-replace"))
            .unwrap(),
    );
    drop(source_executor);
    discard(&scheduler, source.wake);

    let destination = seed_pending_address_core_with_offset(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Parallel,
        32,
    );
    assert_ne!(
        destination.core.inner().fetch_events(),
        source.core.inner().fetch_events()
    );
    let destination_wake = destination.wake;
    let (mut destination_executor, _, _) = attached_executor(&[&destination.core], &scheduler);

    destination_executor
        .apply(&restore_record(manifest))
        .unwrap();

    assert_eq!(
        destination.core.inner().fetch_events(),
        source.core.inner().fetch_events()
    );
    let scheduler_snapshot = scheduler.lock().unwrap().snapshot();
    let [rebound] = scheduler_snapshot.partitions()[0].pending_events() else {
        panic!("pending-address restore must replace the destination wake")
    };
    assert_ne!(*rebound, destination_wake);
    assert_eq!(rebound.kind(), ScheduledEventKind::Serial);
    assert_eq!(destination.core.owned_o3_writeback_wakes().len(), 1);
}

#[test]
fn pending_address_restore_requires_full_scheduler_snapshot() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Serial,
    );
    let (mut executor, _, scheduler_component) = attached_executor(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&checkpoint_record("pending-scheduler"))
            .unwrap(),
    );
    let states = manifest
        .states()
        .iter()
        .filter(|state| state.component() != &scheduler_component)
        .cloned()
        .collect();
    let without_scheduler = CheckpointManifest::new(manifest.label(), manifest.tick(), states);
    seeded.core.write_register(reg(7), 0xfeed);
    let scheduler_before = scheduler.lock().unwrap().snapshot();
    let wakes_before = seeded.core.owned_o3_writeback_wakes();
    let fetches_before = seeded.core.inner().fetch_events();

    assert!(executor.apply(&restore_record(without_scheduler)).is_err());

    assert_eq!(seeded.core.read_register(reg(7)), 0xfeed);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    assert_eq!(seeded.core.inner().fetch_events(), fetches_before);
    assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
}

#[test]
fn pending_address_restore_emits_no_request_before_rebound_wake() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(2).unwrap()));
    let seeded = seed_pending_address_core(
        0,
        &mut scheduler.lock().unwrap(),
        ScheduledEventKind::Serial,
    );
    let (mut executor, _, _) = attached_executor(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&checkpoint_record("pending-no-request"))
            .unwrap(),
    );
    executor.apply(&restore_record(manifest)).unwrap();

    let transport = pending_transport();
    let trace = MemoryTrace::new();
    let before_wake = seeded
        .core
        .issue_next_data_access(
            &mut scheduler.lock().unwrap(),
            &transport,
            trace.clone(),
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap();
    assert!(before_wake.is_none());
    assert!(trace.is_empty());

    let rebound_run = scheduler.lock().unwrap().run_until_idle();
    assert_eq!(rebound_run.final_tick(), LIVE_TICK);
    assert!(trace.is_empty());
    let after_wake = seeded
        .core
        .issue_next_data_access(
            &mut scheduler.lock().unwrap(),
            &transport,
            trace.clone(),
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap();
    assert!(
        after_wake.is_some(),
        "rebound wake did not expose one data request: callback={:?}, issue_trace={:?}",
        seeded.core.pending_callback_error(),
        seeded.core.o3_runtime_live_issue_trace_records(),
    );
    scheduler.lock().unwrap().run_until_idle();
    assert_eq!(
        trace.snapshot(),
        [
            MemoryTraceEvent::request(
                LIVE_TICK,
                DATA_ROUTE,
                data_endpoint(0),
                MemoryTraceKind::RequestSent,
                request(0, 3),
            ),
            MemoryTraceEvent::request(
                LIVE_TICK + 2,
                DATA_ROUTE,
                endpoint("l1d0"),
                MemoryTraceKind::RequestArrived,
                request(0, 3),
            ),
        ]
    );
}
