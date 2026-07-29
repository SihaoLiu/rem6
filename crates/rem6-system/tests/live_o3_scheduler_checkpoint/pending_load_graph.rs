use super::*;
use crate::pending_load_graph_support::{
    assert_pending_load_graph_shape, corrupt_second_row_destination, graph_fetches,
    seed_pending_load_graph_core, seed_pending_load_graph_core_on_scheduler,
    PendingLoadGraphProgram,
};

#[path = "pending_load_graph/atomicity.rs"]
mod atomicity;

#[test]
fn pending_load_graph_source_wake_is_excluded_and_rebound_once() {
    let source = seed_pending_load_graph_core();
    let (mut executor, cpus, scheduler_component) =
        attached_executor(&[&source.core], &source.scheduler);
    let manifest = captured_manifest(
        executor
            .apply(&graph_checkpoint_record(
                "pending-load-graph-source",
                source.capture_tick,
            ))
            .unwrap(),
    );

    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(manifest_chunk(&manifest, &cpus[0], O3LC)).unwrap(),
        source.live
    );
    assert_eq!(
        rem6_cpu::O3RuntimeCheckpointPayload::decode(manifest_chunk(&manifest, &cpus[0], O3RT))
            .unwrap(),
        source.stable
    );
    let scheduler_state = manifest
        .states()
        .iter()
        .find(|state| state.component() == &scheduler_component)
        .cloned()
        .expect("captured scheduler state");
    let scheduler_manifest =
        CheckpointManifest::new(manifest.label(), manifest.tick(), vec![scheduler_state]);
    let (partition_count, min_remote_delay, max_parallel_workers) = {
        let scheduler = source.scheduler.lock().unwrap();
        (
            scheduler.partition_count(),
            scheduler.min_remote_delay(),
            scheduler.max_parallel_workers(),
        )
    };
    let restored_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(
            partition_count,
            min_remote_delay,
            max_parallel_workers,
        )
        .unwrap(),
    ));
    let scheduler_port =
        SchedulerCheckpointPort::new(scheduler_component.clone(), Arc::clone(&restored_scheduler));
    let mut scheduler_registry = CheckpointRegistry::new();
    scheduler_port.register(&mut scheduler_registry).unwrap();
    scheduler_registry.restore(&scheduler_manifest).unwrap();
    scheduler_port.restore_from(&scheduler_registry).unwrap();
    assert_eq!(
        restored_scheduler
            .lock()
            .unwrap()
            .snapshot()
            .total_pending_events(),
        0
    );
    assert_eq!(
        source
            .scheduler
            .lock()
            .unwrap()
            .snapshot()
            .total_pending_events(),
        1
    );
    assert_eq!(source.live.wake.scheduler_order, source.wake.order());
    assert_eq!(source.live.wake.tick, source.capture_tick);
    assert_eq!(source.live.wake.kind, source.wake_kind);
    assert_eq!(source.live.service.requested_tick, source.capture_tick);

    let outcome = executor.apply(&graph_restore_record(manifest)).unwrap();
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
    let scheduler_snapshot = source.scheduler.lock().unwrap().snapshot();
    let [rebound] = scheduler_snapshot.partitions()[0].pending_events() else {
        panic!("pending-load graph restore must leave one rebound wake")
    };
    assert_ne!(rebound.id(), source.wake.id());
    assert_ne!(rebound.order(), source.wake.order());
    assert_eq!(rebound.tick(), source.capture_tick);
    assert_eq!(rebound.kind(), source.wake_kind);
    assert_eq!(
        source.core.owned_o3_writeback_wakes(),
        [(source.scheduler.lock().unwrap().instance_id(), *rebound)]
    );
}

#[test]
fn pending_load_graph_restore_replaces_destination_fetches_and_rename() {
    let source = seed_pending_load_graph_core();
    let (mut source_executor, _, _) = attached_executor(&[&source.core], &source.scheduler);
    let manifest = captured_manifest(
        source_executor
            .apply(&graph_checkpoint_record(
                "pending-load-graph-replace",
                source.capture_tick,
            ))
            .unwrap(),
    );

    discard(&source.scheduler, source.wake);
    let destination = seed_pending_load_graph_core_on_scheduler(
        PendingLoadGraphProgram::DivergentRegisters,
        Arc::clone(&source.scheduler),
    );
    assert_ne!(
        destination.core.inner().fetch_events(),
        graph_fetches(&source.live)
    );
    assert_ne!(
        destination.core.o3_runtime_snapshot(),
        source.core.o3_runtime_snapshot()
    );
    assert_ne!(
        destination.core.o3_runtime_snapshot().rename_map(),
        source.stable.snapshot().rename_map()
    );
    let (mut destination_executor, _, _) =
        attached_executor(&[&destination.core], &destination.scheduler);

    destination_executor
        .apply(&graph_restore_record(manifest))
        .unwrap();

    assert_eq!(
        destination.core.inner().fetch_events(),
        graph_fetches(&source.live)
    );
    let restored = destination.core.o3_runtime_snapshot();
    assert_eq!(
        restored.reorder_buffer(),
        source.stable.snapshot().reorder_buffer()
    );
    assert_eq!(
        restored.load_store_queue(),
        source.stable.snapshot().load_store_queue()
    );
    assert_eq!(
        restored.rename_map(),
        [
            source.stable.snapshot().rename_map()[0],
            source.live.pending_addresses[0]
                .destination
                .expect("first pending load destination"),
            source.live.pending_addresses[1]
                .destination
                .expect("second pending load destination"),
            source.live.pending_addresses[2]
                .destination
                .expect("third pending load destination"),
        ]
    );
    let destination_scheduler_id = destination.scheduler.lock().unwrap().instance_id();
    let destination_wakes = destination.core.owned_o3_writeback_wakes();
    let [(_, rebound)] = destination_wakes.as_slice() else {
        panic!("pending-load graph restore must own one rebound wake")
    };
    assert_pending_load_graph_shape(
        &destination.core,
        &source.stable,
        &source.live,
        source.scheduler.lock().unwrap().instance_id(),
        source.wake,
        destination_scheduler_id,
        *rebound,
        PendingLoadGraphProgram::MixedFanout,
        source.capture_tick,
    );
}

fn graph_checkpoint_record(label: &str, tick: u64) -> HostActionRecord {
    record(
        tick,
        1,
        HostAction::Checkpoint {
            label: label.into(),
        },
    )
}

fn graph_restore_record(manifest: CheckpointManifest) -> HostActionRecord {
    record(
        manifest.tick() + 1,
        2,
        HostAction::RestoreCheckpoint { manifest },
    )
}
