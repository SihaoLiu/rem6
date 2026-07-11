use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, SchedulerCheckpointBank, SchedulerCheckpointPort,
    SystemActionExecutor, SystemActionOutcome,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn core_and_transport() -> (RiscvCore, MemoryTransport) {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    (core, transport)
}

fn loaded_div_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let div: u32 = (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), div.to_le_bytes().to_vec())
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(rem6_transport::RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome
       + Send
       + 'static {
    move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    }
}

fn drive_to_live_gate(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
) {
    assert!(matches!(
        core.drive_next_action(
            scheduler,
            transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            responder(Arc::clone(store)),
            responder(Arc::clone(store)),
        )
        .unwrap(),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        core.drive_next_action(
            scheduler,
            transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            responder(Arc::clone(store)),
            responder(Arc::clone(store)),
        )
        .unwrap(),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(
        core.drive_next_action(
            scheduler,
            transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            responder(Arc::clone(store)),
            responder(Arc::clone(store)),
        )
        .unwrap(),
        None
    );
}

fn attached_checkpoint_executor(
    core: &RiscvCore,
    scheduler: &Arc<Mutex<PartitionedScheduler>>,
) -> (
    SystemActionExecutor,
    CheckpointComponentId,
    CheckpointComponentId,
) {
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let scheduler_component = CheckpointComponentId::new("scheduler0").unwrap();
    let riscv_bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        cpu_component.clone(),
        core.clone(),
    )])
    .unwrap();
    let scheduler_bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
        scheduler_component.clone(),
        Arc::clone(scheduler),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_checkpoint_bank(riscv_bank).unwrap();
    executor
        .attach_scheduler_checkpoint_bank(scheduler_bank)
        .unwrap();
    (executor, cpu_component, scheduler_component)
}

#[test]
fn attached_scheduler_checkpoint_captures_restores_and_rearms_live_div_gate_once() {
    let (core, transport) = core_and_transport();
    core.write_register(Register::new(1).unwrap(), 84);
    core.write_register(Register::new(2).unwrap(), 7);
    core.set_detailed_live_retire_gate_enabled(true);
    let store = loaded_div_store();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_min_remote_delay(2, 1).unwrap(),
    ));
    drive_to_live_gate(&core, &mut scheduler.lock().unwrap(), &transport, &store);
    let ready_tick = core
        .checkpoint_owned_live_retire_gate_wakes()
        .first()
        .copied()
        .expect("scheduled detailed DIV gate")
        .1
        .tick();
    assert!(matches!(
        scheduler.lock().unwrap().quiescent_snapshot(),
        Err(SchedulerError::SnapshotContainsPendingEvents { pending_events: 1 })
    ));

    let (mut executor, cpu_component, scheduler_component) =
        attached_checkpoint_executor(&core, &scheduler);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(31);
    let checkpoint = HostActionRecord::new(
        scheduler.lock().unwrap().now(),
        host,
        host,
        GuestEventId::new(51),
        source,
        HostAction::Checkpoint {
            label: "live-div-gate".to_string(),
        },
    );

    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &cpu_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "o3-runtime-state")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &scheduler_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "scheduler")
    }));
    assert_eq!(
        scheduler.lock().unwrap().snapshot().total_pending_events(),
        1
    );

    let restore = HostActionRecord::new(
        checkpoint.tick().saturating_add(1),
        host,
        host,
        GuestEventId::new(52),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: restore.tick(),
            event: restore.event(),
            source,
            manifest,
        }
    );
    assert!(scheduler.lock().unwrap().is_idle());
    assert!(core.checkpoint_owned_live_retire_gate_wakes().is_empty());

    let mut scheduler = scheduler.lock().unwrap();
    drive_to_live_gate(&core, &mut scheduler, &transport, &store);
    assert_eq!(scheduler.snapshot().total_pending_events(), 1);
    assert_eq!(
        core.checkpoint_owned_live_retire_gate_wakes()
            .first()
            .copied()
            .expect("restored gate should rearm once")
            .1
            .tick(),
        ready_tick
    );
    assert_eq!(
        core.drive_next_action(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            responder(Arc::clone(&store)),
            responder(Arc::clone(&store)),
        )
        .unwrap(),
        None
    );
    assert_eq!(scheduler.snapshot().total_pending_events(), 1);

    scheduler.run_until_idle_conservative();
    assert!(matches!(
        core.drive_next_action(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            responder(Arc::clone(&store)),
            responder(Arc::clone(&store)),
        )
        .unwrap(),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(core.read_register(Register::new(3).unwrap()), 12);
    assert!(scheduler.is_idle());
}

#[test]
fn redirected_live_gate_wake_remains_owned_until_scheduler_restore_discards_it() {
    let (core, transport) = core_and_transport();
    core.write_register(Register::new(1).unwrap(), 84);
    core.write_register(Register::new(2).unwrap(), 7);
    core.set_detailed_live_retire_gate_enabled(true);
    let store = loaded_div_store();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_min_remote_delay(2, 1).unwrap(),
    ));
    drive_to_live_gate(&core, &mut scheduler.lock().unwrap(), &transport, &store);
    let wake = core
        .checkpoint_owned_live_retire_gate_wakes()
        .first()
        .copied()
        .expect("scheduled detailed DIV gate");

    core.redirect_pc(Address::new(0x9000));

    assert_eq!(core.checkpoint_owned_live_retire_gate_wakes(), vec![wake]);
    assert_eq!(
        scheduler.lock().unwrap().snapshot().total_pending_events(),
        1
    );
    let (mut executor, _, _) = attached_checkpoint_executor(&core, &scheduler);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(41);
    let checkpoint = HostActionRecord::new(
        scheduler.lock().unwrap().now(),
        host,
        host,
        GuestEventId::new(61),
        source,
        HostAction::Checkpoint {
            label: "redirected-live-div-gate".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert_eq!(
        scheduler.lock().unwrap().snapshot().total_pending_events(),
        1
    );

    let restore = HostActionRecord::new(
        checkpoint.tick().saturating_add(1),
        host,
        host,
        GuestEventId::new(62),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    assert!(matches!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored { .. }
    ));
    assert!(scheduler.lock().unwrap().is_idle());
    assert!(core.checkpoint_owned_live_retire_gate_wakes().is_empty());
}
