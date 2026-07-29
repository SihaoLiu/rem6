use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointManifest, CheckpointRegistry};
use rem6_cpu::{RiscvCore, RiscvO3LiveCheckpointPayload};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, PendingEventSnapshot, ScheduledEventKind};
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore};
use rem6_stats::StatsRegistry;
use rem6_system::{
    ExecutionMode, ExecutionModeTarget, GuestEventId, GuestSourceId, HostAction, HostActionRecord,
    MemoryStoreCheckpointBank, MemoryStoreCheckpointPort, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, SchedulerCheckpointBank, SchedulerCheckpointPort,
    SystemActionExecutor, SystemActionOutcome,
};

#[path = "support/live_o3_pending_address.rs"]
mod pending_address_support;
#[path = "support/live_o3_pending_load_graph.rs"]
mod pending_load_graph_support;
#[path = "support/live_o3.rs"]
mod support;
use support::{
    core, seed_live_core, seed_live_core_at, SeededLiveCore, LIVE_TICK, O3LC, O3LH, O3RT,
};

#[path = "live_o3_scheduler_checkpoint/pending_address.rs"]
mod pending_address;
#[path = "live_o3_scheduler_checkpoint/pending_load_graph.rs"]
mod pending_load_graph;

const FIRST_PARTITION_FRONTIERS: [usize; 2] = [5 * 8 + 4 + 8, 5 * 8 + 4 + 16];

#[rustfmt::skip]
fn attached_executor(cores: &[&RiscvCore], scheduler: &Arc<Mutex<PartitionedScheduler>>) -> (SystemActionExecutor, Vec<CheckpointComponentId>, CheckpointComponentId) {
    let cpus = (0..cores.len()).map(|cpu| CheckpointComponentId::new(format!("cpu{cpu}")).unwrap()).collect::<Vec<_>>();
    let sched = CheckpointComponentId::new("scheduler0").unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    let ports = cpus.iter().zip(cores).map(|(cpu, core)| RiscvCoreCheckpointPort::new(cpu.clone(), RiscvCore::clone(core)));
    executor.attach_riscv_checkpoint_bank(RiscvCoreCheckpointBank::new(ports).unwrap()).unwrap();
    executor.attach_scheduler_checkpoint_bank(SchedulerCheckpointBank::new([
        SchedulerCheckpointPort::new(sched.clone(), Arc::clone(scheduler)),
    ]).unwrap()).unwrap();
    (executor, cpus, sched)
}

fn attached_executor_with_memory(
    cores: &[&RiscvCore],
    scheduler: &Arc<Mutex<PartitionedScheduler>>,
) -> (
    SystemActionExecutor,
    Vec<CheckpointComponentId>,
    CheckpointComponentId,
    Arc<Mutex<PartitionedMemoryStore>>,
    MemoryTargetId,
) {
    let (mut executor, cpus, scheduler_component) = attached_executor(cores, scheduler);
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    let layout = CacheLineLayout::new(16).unwrap();
    store.add_partition(target, layout).unwrap();
    store
        .map_region(
            target,
            Address::new(0x9000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    store
        .insert_line(target, Address::new(0x9000), vec![0x11; 16])
        .unwrap();
    let store = Arc::new(Mutex::new(store));
    executor
        .attach_memory_checkpoint_bank(
            MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                CheckpointComponentId::new("memory0").unwrap(),
                Arc::clone(&store),
            )])
            .unwrap(),
        )
        .unwrap();
    (executor, cpus, scheduler_component, store, target)
}

#[rustfmt::skip]
fn seed(scheduler: &Arc<Mutex<PartitionedScheduler>>, cpu: u32, kind: ScheduledEventKind) -> SeededLiveCore {
    seed_live_core(cpu, &mut scheduler.lock().unwrap(), kind)
}

#[rustfmt::skip]
fn live_fixture(kind: ScheduledEventKind, label: &str) -> (Arc<Mutex<PartitionedScheduler>>, SeededLiveCore, SystemActionExecutor, CheckpointComponentId, CheckpointComponentId, CheckpointManifest) {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed(&scheduler, 0, kind);
    let (mut executor, cpus, scheduler_component) = attached_executor(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(executor.apply(&checkpoint_record(label)).unwrap());
    (scheduler, seeded, executor, cpus[0].clone(), scheduler_component, manifest)
}

#[rustfmt::skip]
fn record(tick: u64, event: u64, action: HostAction) -> HostActionRecord {
    HostActionRecord::new(tick, PartitionId::new(0), PartitionId::new(0), GuestEventId::new(event), GuestSourceId::new(1), action)
}

fn checkpoint_record(label: &str) -> HostActionRecord {
    let action = HostAction::Checkpoint {
        label: label.into(),
    };
    record(LIVE_TICK, 1, action)
}

fn restore_record(manifest: CheckpointManifest) -> HostActionRecord {
    record(LIVE_TICK + 1, 2, HostAction::RestoreCheckpoint { manifest })
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn captured_manifest(outcome: SystemActionOutcome) -> CheckpointManifest {
    match outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[rustfmt::skip]
fn discard(scheduler: &Arc<Mutex<PartitionedScheduler>>, event: PendingEventSnapshot) {
    scheduler.lock().unwrap().checkpoint_access().discard_exact_events(&[event]).unwrap();
}

#[rustfmt::skip]
fn assert_capture_rejected(executor: &mut SystemActionExecutor, scheduler: &Arc<Mutex<PartitionedScheduler>>, label: &str) {
    let checkpoints = executor.checkpoints().clone();
    let snapshot = scheduler.lock().unwrap().snapshot();
    assert!(executor.apply(&checkpoint_record(label)).is_err());
    assert_eq!(executor.checkpoints(), &checkpoints);
    assert_eq!(scheduler.lock().unwrap().snapshot(), snapshot);
}

#[rustfmt::skip]
fn manifest_chunk<'a>(manifest: &'a CheckpointManifest, component: &CheckpointComponentId, name: &str) -> &'a [u8] {
    manifest.states().iter().find(|state| state.component() == component).unwrap().chunks().iter().find(|chunk| chunk.name() == name).unwrap().payload()
}

#[test]
#[rustfmt::skip]
fn live_o3_wake_is_excluded_from_source_scheduler_snapshot() {
    let (scheduler, seeded, _, cpu, scheduler_component, manifest) = live_fixture(ScheduledEventKind::Serial, "live-o3");
    assert_eq!(seeded.live.wake.kind, ScheduledEventKind::Serial);
    for name in [O3LC, O3LH, O3RT] { assert!(!manifest_chunk(&manifest, &cpu, name).is_empty()); }
    let payload = manifest_chunk(&manifest, &scheduler_component, "scheduler");
    assert_eq!(u64::from_le_bytes(payload[payload.len() - 8..].try_into().unwrap()), 0);
    assert_eq!(scheduler.lock().unwrap().snapshot().total_pending_events(), 1);
}

#[test]
#[rustfmt::skip]
fn live_o3_capture_rejects_same_partition_tick_sources_before_publication() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let first = seed(&scheduler, 0, ScheduledEventKind::Serial);
    let second = seed(&scheduler, 1, ScheduledEventKind::Parallel);
    assert_ne!(first.wake.order(), second.wake.order());
    let (mut executor, _, _) = attached_executor(&[&first.core, &second.core], &scheduler);
    first.core.write_register(reg(7), 0xcafe); second.core.write_register(reg(8), 0xbeef);
    assert_capture_rejected(&mut executor, &scheduler, "same-tick-source");
    assert_eq!(first.core.read_register(reg(7)), 0xcafe); assert_eq!(second.core.read_register(reg(8)), 0xbeef);
}

#[test]
#[rustfmt::skip]
fn live_o3_capture_rejects_missing_tracked_source_wake_before_publication() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let seeded = seed(&scheduler, 0, ScheduledEventKind::Serial);
    let (mut executor, _, _) = attached_executor(&[&seeded.core], &scheduler);
    discard(&scheduler, seeded.wake);
    seeded.core.write_register(reg(7), 0xcafe);
    let wakes = seeded.core.owned_o3_writeback_wakes();
    assert_capture_rejected(&mut executor, &scheduler, "missing-source-wake");
    assert_eq!(seeded.core.read_register(reg(7)), 0xcafe); assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes);
}

#[test]
#[rustfmt::skip]
fn live_o3_restore_discards_destination_wake_and_rebinds_once() {
    let (scheduler, seeded, mut executor, cpu, scheduler_component, manifest) = live_fixture(ScheduledEventKind::Parallel, "rebind");
    for offset in FIRST_PARTITION_FRONTIERS {
        let exhausted = rewrite_chunk(&manifest, &scheduler_component, "scheduler", |payload| payload[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes()));
        assert!(executor.apply(&restore_record(exhausted)).is_err());
    }
    let outcome = executor.apply(&restore_record(manifest)).unwrap();
    let SystemActionOutcome::CheckpointRestored { rebound_o3_wake_components, .. } = outcome else { panic!("unexpected restore outcome: {outcome:?}"); };
    assert_eq!(rebound_o3_wake_components, std::collections::BTreeSet::from([cpu]));
    let snapshot = scheduler.lock().unwrap().snapshot(); let pending = snapshot.partitions()[0].pending_events();
    assert_eq!(pending.len(), 1); assert_ne!(pending[0].id(), seeded.wake.id());
    assert_eq!(pending[0].kind(), ScheduledEventKind::Parallel); assert_eq!(seeded.core.owned_o3_writeback_wakes().len(), 1);
}

#[test]
fn live_o3_due_now_parallel_rebind_holds_restored_scheduler_frontier() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    scheduler
        .lock()
        .unwrap()
        .schedule_at(PartitionId::new(0), LIVE_TICK, |_| {})
        .unwrap();
    scheduler.lock().unwrap().run_until_idle();
    let seeded = seed(&scheduler, 0, ScheduledEventKind::Parallel);
    let (mut executor, cpus, _) = attached_executor(&[&seeded.core], &scheduler);
    let manifest = captured_manifest(executor.apply(&checkpoint_record("due-now")).unwrap());

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
    let mut scheduler = scheduler.lock().unwrap();
    let pending = scheduler.snapshot().partitions()[0].pending_events()[0];
    assert_eq!(pending.tick(), LIVE_TICK);
    assert_eq!(pending.kind(), ScheduledEventKind::Parallel);
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    assert_eq!(plan.horizon(), LIVE_TICK);
    let run = scheduler.run_next_epoch_parallel_recorded().unwrap();
    assert_eq!(run.summary().final_tick(), LIVE_TICK);
}

#[test]
#[rustfmt::skip]
fn live_o3_restore_validates_saved_scheduler_order_and_preserves_kind() {
    let (scheduler, seeded, mut executor, cpu, _, manifest) = live_fixture(ScheduledEventKind::Parallel, "order");
    let invalid = rewrite_o3lc(&manifest, &cpu, |live| live.wake.scheduler_order = u64::MAX);
    seeded.core.write_register(reg(7), 0xfeed);
    assert!(executor.apply(&restore_record(invalid)).is_err()); assert_eq!(seeded.core.read_register(reg(7)), 0xfeed);
    executor.apply(&restore_record(manifest)).unwrap();
    assert_eq!(scheduler.lock().unwrap().snapshot().partitions()[0].pending_events()[0].kind(), ScheduledEventKind::Parallel);
}

#[test]
fn live_o3_restore_rejects_saved_wake_kind_that_disagrees_with_source_authority() {
    let (scheduler, seeded, mut executor, cpu, _, manifest) =
        live_fixture(ScheduledEventKind::Serial, "kind-authority");
    let corrupt = rewrite_o3lc(&manifest, &cpu, |live| {
        live.wake.kind = ScheduledEventKind::Parallel;
    });
    seeded.core.write_register(reg(7), 0xfeed);
    let core_before = seeded.core.checkpoint_hart_state();
    let wakes_before = seeded.core.owned_o3_writeback_wakes();
    let scheduler_before = scheduler.lock().unwrap().snapshot();

    assert!(executor.apply(&restore_record(corrupt)).is_err());

    assert_eq!(seeded.core.checkpoint_hart_state(), core_before);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
}

#[test]
#[rustfmt::skip]
fn live_o3_restore_requires_scheduler_snapshot_not_discard_only() {
    let (scheduler, seeded, mut executor, _, scheduler_component, manifest) = live_fixture(ScheduledEventKind::Serial, "snapshot");
    let states = manifest.states().iter().filter(|state| state.component() != &scheduler_component).cloned().collect();
    let without_scheduler = CheckpointManifest::new(manifest.label(), manifest.tick(), states);
    seeded.core.write_register(reg(7), 0xfeed);
    let scheduler_before = scheduler.lock().unwrap().snapshot(); let wakes_before = seeded.core.owned_o3_writeback_wakes();
    assert!(executor.apply(&restore_record(without_scheduler)).is_err());
    assert_eq!(seeded.core.read_register(reg(7)), 0xfeed); assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
}

#[test]
#[rustfmt::skip]
fn live_o3_restore_rejects_same_partition_tick_competitor_preflight() {
    let (scheduler, seeded, mut executor, _, _, manifest) = live_fixture(ScheduledEventKind::Serial, "competitor");
    scheduler.lock().unwrap().schedule_at(PartitionId::new(0), LIVE_TICK, |_| {}).unwrap();
    seeded.core.write_register(reg(7), 0xbeef);
    let scheduler_before = scheduler.lock().unwrap().snapshot(); let wakes_before = seeded.core.owned_o3_writeback_wakes();
    assert!(executor.apply(&restore_record(manifest)).is_err()); assert_eq!(seeded.core.read_register(reg(7)), 0xbeef);
    assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before); assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
}

#[test]
#[rustfmt::skip]
fn live_o3_restore_excludes_complete_destination_discard_set_from_competitors() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let source_live = seed(&scheduler, 0, ScheduledEventKind::Serial); let source_drained = core(1);
    let (mut source, _, _) = attached_executor(&[&source_live.core, &source_drained], &scheduler);
    let manifest = captured_manifest(source.apply(&checkpoint_record("mixed")).unwrap()); drop(source); discard(&scheduler, source_live.wake);
    let destination_live = seed(&scheduler, 1, ScheduledEventKind::Parallel); let destination_drained = core(0);
    let (mut destination, _, _) = attached_executor(&[&destination_drained, &destination_live.core], &scheduler);
    destination.apply(&restore_record(manifest)).unwrap();
    let snapshot = scheduler.lock().unwrap().snapshot(); let pending = snapshot.partitions()[0].pending_events();
    assert_eq!(pending.len(), 1); assert_eq!(pending[0].kind(), ScheduledEventKind::Serial);
    assert_eq!(destination_drained.owned_o3_writeback_wakes().len(), 1); assert!(destination_live.core.owned_o3_writeback_wakes().is_empty());
    assert!(destination_live.core.requested_o3_writeback_wake_tick(snapshot.now()).is_none());
}

#[test]
#[rustfmt::skip]
fn live_o3_restore_rejects_exhausted_scheduler_frontiers_preflight() {
    let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
    let first = seed(&scheduler, 0, ScheduledEventKind::Serial);
    let second = seed_live_core_at(1, &mut scheduler.lock().unwrap(), ScheduledEventKind::Parallel, LIVE_TICK + 1, LIVE_TICK);
    let (mut executor, _, scheduler_component) = attached_executor(&[&first.core, &second.core], &scheduler);
    let manifest = captured_manifest(executor.apply(&checkpoint_record("frontier")).unwrap());
    first.core.write_register(reg(7), 0xcafe); second.core.write_register(reg(8), 0xbeef);
    let scheduler_before = scheduler.lock().unwrap().snapshot();
    for offset in FIRST_PARTITION_FRONTIERS {
        let corrupted = rewrite_chunk(&manifest, &scheduler_component, "scheduler", |payload| payload[offset..offset + 8].copy_from_slice(&(u64::MAX - 1).to_le_bytes()));
        let error = executor.apply(&restore_record(corrupted)).unwrap_err();
        assert!(format!("{error:?}").contains("restored scheduler event frontier is exhausted"), "unexpected restore error: {error:?}");
        assert_eq!(first.core.read_register(reg(7)), 0xcafe); assert_eq!(second.core.read_register(reg(8)), 0xbeef);
        assert_eq!(scheduler.lock().unwrap().snapshot(), scheduler_before);
    }
}

#[rustfmt::skip]
fn rewrite_o3lc(manifest: &CheckpointManifest, component: &CheckpointComponentId, edit: impl FnOnce(&mut RiscvO3LiveCheckpointPayload)) -> CheckpointManifest {
    rewrite_chunk(manifest, component, O3LC, |payload| { let mut live = RiscvO3LiveCheckpointPayload::decode(payload).unwrap(); edit(&mut live); *payload = live.encode().unwrap(); })
}

#[rustfmt::skip]
fn rewrite_chunk(manifest: &CheckpointManifest, component: &CheckpointComponentId, name: &str, edit: impl FnOnce(&mut Vec<u8>)) -> CheckpointManifest {
    let mut registry = CheckpointRegistry::new();
    for state in manifest.states() { registry.register(state.component().clone()).unwrap(); }
    registry.restore(manifest).unwrap();
    let mut payload = registry.chunk(component, name).unwrap().to_vec(); edit(&mut payload); registry.write_chunk(component, name, payload).unwrap();
    registry.capture(manifest.label(), manifest.tick()).unwrap()
}
