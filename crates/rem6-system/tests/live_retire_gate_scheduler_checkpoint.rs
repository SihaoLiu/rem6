use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_cpu::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, InOrderPipelineStallCause, RiscvCore,
    RiscvCoreDriveAction,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, SchedulerCheckpointBank, SchedulerCheckpointPort,
    SystemActionExecutor, SystemActionOutcome, SystemError,
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

fn loaded_nop_store() -> Arc<Mutex<PartitionedMemoryStore>> {
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
        .add_segment(Address::new(0x8000), 0x0000_0013u32.to_le_bytes().to_vec())
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

fn drive_non_pipeline_action(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
) -> Option<RiscvCoreDriveAction> {
    for _ in 0..8 {
        let action = core
            .drive_next_action(
                scheduler,
                transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(Arc::clone(store)),
                responder(Arc::clone(store)),
            )
            .unwrap();
        if matches!(
            action,
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
        ) {
            scheduler.run_until_idle_conservative();
            continue;
        }
        return action;
    }
    panic!(
        "expected a non-pipeline live-gate action at pc {:?} with pipeline {:?}",
        core.pc(),
        core.in_order_pipeline_snapshot().in_flight()
    );
}

fn drive_to_live_gate(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
) {
    assert!(matches!(
        drive_non_pipeline_action(core, scheduler, transport, store),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_non_pipeline_action(core, scheduler, transport, store),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(
        drive_non_pipeline_action(core, scheduler, transport, store),
        None
    );
}

fn drive_next_core_action(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
) -> Option<RiscvCoreDriveAction> {
    core.drive_next_action(
        scheduler,
        transport,
        MemoryTrace::new(),
        MemoryTrace::new(),
        responder(Arc::clone(store)),
        responder(Arc::clone(store)),
    )
    .unwrap()
}

fn recorded_execute_wait_cycles(core: &RiscvCore) -> u64 {
    core.in_order_pipeline_cycle_records()
        .iter()
        .filter(|record| record.stall_cause() == Some(InOrderPipelineStallCause::ExecuteWait))
        .map(|record| record.stall_cycle_count())
        .sum()
}

fn recorded_in_order_retirements(core: &RiscvCore) -> usize {
    core.in_order_pipeline_cycle_records()
        .iter()
        .map(|record| record.summary().retired_count())
        .sum()
}

fn drive_to_pending_div_execute_wait_wake(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
) -> u64 {
    const DRIVE_LIMIT: usize = 96;
    const DIV_EXTRA_EXECUTE_WAIT_CYCLES: u64 = 19;
    let x3 = Register::new(3).unwrap();
    let mut previous_wait_cycles = recorded_execute_wait_cycles(core);

    for _ in 0..DRIVE_LIMIT {
        assert_eq!(
            core.read_register(x3),
            0,
            "NORMAL in-order DIV result must stay hidden while ExecuteWait is scheduler-visible"
        );
        assert!(
            core.checkpoint_owned_live_retire_gate_wakes().is_empty(),
            "NORMAL in-order DIV must not use the detailed O3 live retire gate"
        );

        match drive_next_core_action(core, scheduler, transport, store) {
            Some(RiscvCoreDriveAction::FetchIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. }) => {
                assert_eq!(core.checkpoint_owned_in_order_pipeline_wakes().len(), 1);
                scheduler.run_until_idle_conservative();
                assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());

                let wait_cycles = recorded_execute_wait_cycles(core);
                if wait_cycles > previous_wait_cycles {
                    assert_eq!(
                        wait_cycles,
                        previous_wait_cycles + 1,
                        "one scheduler wake should consume exactly one DIV ExecuteWait cycle"
                    );
                    assert!(
                        wait_cycles < DIV_EXTRA_EXECUTE_WAIT_CYCLES,
                        "checkpoint must leave a remaining DIV ExecuteWait tail"
                    );
                    assert_eq!(
                        core.read_register(x3),
                        0,
                        "DIV destination became architectural while only ExecuteWait advanced"
                    );
                    assert!(matches!(
                        drive_next_core_action(core, scheduler, transport, store),
                        Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
                    ));
                    assert_eq!(
                        core.checkpoint_owned_in_order_pipeline_wakes().len(),
                        1,
                        "checkpoint must capture the next pending in-order pipeline wake"
                    );
                    return wait_cycles;
                }
                previous_wait_cycles = wait_cycles;
            }
            Some(RiscvCoreDriveAction::InstructionExecuted(execution)) => panic!(
                "NORMAL in-order DIV became architectural before any scheduler-visible \
                 ExecuteWait cycle: execution={execution:?}, wait_cycles={previous_wait_cycles}"
            ),
            Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                panic!("register-register DIV should not issue a data access")
            }
            None if scheduler.is_idle() => {
                panic!("NORMAL in-order DIV made no progress before scheduler-visible ExecuteWait")
            }
            None => {
                scheduler.run_until_idle_conservative();
            }
        }
    }

    panic!("bounded drive did not reach a pending DIV ExecuteWait wake")
}

fn drive_until_div_architectural_visibility(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    expected_replayed_wait_cycles: u64,
) -> u64 {
    const DRIVE_LIMIT: usize = 96;
    let x3 = Register::new(3).unwrap();
    let start_wait_cycles = recorded_execute_wait_cycles(core);

    for _ in 0..DRIVE_LIMIT {
        assert_eq!(
            core.read_register(x3),
            0,
            "DIV destination became architectural before retirement replay completed"
        );
        assert!(
            core.checkpoint_owned_live_retire_gate_wakes().is_empty(),
            "restored NORMAL in-order DIV must not use the detailed O3 live retire gate"
        );

        match drive_next_core_action(core, scheduler, transport, store) {
            Some(RiscvCoreDriveAction::FetchIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. }) => {
                assert_eq!(core.checkpoint_owned_in_order_pipeline_wakes().len(), 1);
                scheduler.run_until_idle_conservative();
                let replayed_wait_cycles =
                    recorded_execute_wait_cycles(core).saturating_sub(start_wait_cycles);
                assert!(
                    replayed_wait_cycles <= expected_replayed_wait_cycles,
                    "restored DIV replayed too many ExecuteWait cycles"
                );
            }
            Some(RiscvCoreDriveAction::InstructionExecuted(_)) => {
                let replayed_wait_cycles =
                    recorded_execute_wait_cycles(core).saturating_sub(start_wait_cycles);
                assert_eq!(
                    replayed_wait_cycles, expected_replayed_wait_cycles,
                    "restore should replay only the remaining DIV ExecuteWait cycles"
                );
                assert_eq!(core.read_register(x3), 12);
                return replayed_wait_cycles;
            }
            Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                panic!("register-register DIV should not issue a data access")
            }
            None if scheduler.is_idle() => {
                panic!("restored NORMAL in-order DIV stopped before architectural visibility")
            }
            None => {
                scheduler.run_until_idle_conservative();
            }
        }
    }

    panic!("bounded drive did not retire restored DIV")
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
fn attached_scheduler_checkpoint_rejects_live_div_gate_without_losing_wake() {
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

    let (mut executor, cpu_component, _scheduler_component) =
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

    let error = executor.apply(&checkpoint).unwrap_err();
    assert!(matches!(
        error,
        SystemError::Checkpoint(CheckpointError::ComponentNotQuiescent { component })
            if component == cpu_component
    ));
    assert_eq!(
        scheduler.lock().unwrap().snapshot().total_pending_events(),
        1
    );
    assert_eq!(
        core.checkpoint_owned_live_retire_gate_wakes()
            .first()
            .copied()
            .expect("failed checkpoint must preserve the live gate wake")
            .1
            .tick(),
        ready_tick
    );

    let mut scheduler = scheduler.lock().unwrap();
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
fn attached_scheduler_checkpoint_replays_only_remaining_normal_div_execute_wait() {
    const DIV_EXTRA_EXECUTE_WAIT_CYCLES: u64 = 19;

    let (core, transport) = core_and_transport();
    core.write_register(Register::new(1).unwrap(), 84);
    core.write_register(Register::new(2).unwrap(), 7);
    core.set_detailed_live_retire_gate_enabled(false);
    let store = loaded_div_store();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_min_remote_delay(2, 1).unwrap(),
    ));

    let consumed_wait_cycles = {
        let mut scheduler = scheduler.lock().unwrap();
        drive_to_pending_div_execute_wait_wake(&core, &mut scheduler, &transport, &store)
    };
    assert!(
        consumed_wait_cycles >= 1,
        "checkpoint must occur after at least one scheduler-visible DIV ExecuteWait cycle"
    );
    assert_eq!(
        scheduler.lock().unwrap().snapshot().total_pending_events(),
        1
    );

    let (mut executor, cpu_component, scheduler_component) =
        attached_checkpoint_executor(&core, &scheduler);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(61);
    let checkpoint = HostActionRecord::new(
        scheduler.lock().unwrap().now(),
        host,
        host,
        GuestEventId::new(81),
        source,
        HostAction::Checkpoint {
            label: "normal-div-execute-wait".to_string(),
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
                .any(|chunk| chunk.name() == "in-order-pipeline")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &scheduler_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "scheduler")
    }));

    scheduler.lock().unwrap().run_until_idle_conservative();
    core.write_register(Register::new(1).unwrap(), 1);
    core.write_register(Register::new(2).unwrap(), 1);
    core.write_register(Register::new(3).unwrap(), 99);

    let restore = HostActionRecord::new(
        checkpoint.tick().saturating_add(1),
        host,
        host,
        GuestEventId::new(82),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    assert!(matches!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored { .. }
    ));
    assert!(scheduler.lock().unwrap().is_idle());
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
    assert!(core.checkpoint_owned_live_retire_gate_wakes().is_empty());
    assert_eq!(core.read_register(Register::new(1).unwrap()), 84);
    assert_eq!(core.read_register(Register::new(2).unwrap()), 7);
    assert_eq!(core.read_register(Register::new(3).unwrap()), 0);
    assert_eq!(recorded_execute_wait_cycles(&core), consumed_wait_cycles);

    let retirements_before_replay = recorded_in_order_retirements(&core);
    let remaining_wait_cycles = DIV_EXTRA_EXECUTE_WAIT_CYCLES - consumed_wait_cycles;
    let replayed_wait_cycles = {
        let mut scheduler = scheduler.lock().unwrap();
        drive_until_div_architectural_visibility(
            &core,
            &mut scheduler,
            &transport,
            &store,
            remaining_wait_cycles,
        )
    };

    assert_eq!(replayed_wait_cycles, remaining_wait_cycles);
    assert_eq!(core.read_register(Register::new(3).unwrap()), 12);
    assert_eq!(
        recorded_in_order_retirements(&core) - retirements_before_replay,
        1,
        "restored DIV should retire exactly once"
    );
    assert!(scheduler.lock().unwrap().is_idle());
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

#[test]
fn scheduler_restore_discards_and_forgets_pending_pipeline_wake() {
    let (core, transport) = core_and_transport();
    let store = loaded_nop_store();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_min_remote_delay(2, 1).unwrap(),
    ));
    {
        let mut scheduler = scheduler.lock().unwrap();
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
            Some(RiscvCoreDriveAction::FetchIssued { .. })
        ));
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
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
        ));
    }
    assert_eq!(core.checkpoint_owned_in_order_pipeline_wakes().len(), 1);

    let (mut executor, _, _) = attached_checkpoint_executor(&core, &scheduler);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(51);
    let checkpoint = HostActionRecord::new(
        scheduler.lock().unwrap().now(),
        host,
        host,
        GuestEventId::new(71),
        source,
        HostAction::Checkpoint {
            label: "pending-pipeline-wake".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    let restore = HostActionRecord::new(
        checkpoint.tick().saturating_add(1),
        host,
        host,
        GuestEventId::new(72),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    assert!(matches!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored { .. }
    ));
    assert!(scheduler.lock().unwrap().is_idle());
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());

    let second_checkpoint = HostActionRecord::new(
        restore.tick().saturating_add(1),
        host,
        host,
        GuestEventId::new(73),
        source,
        HostAction::Checkpoint {
            label: "after-pipeline-wake-restore".to_string(),
        },
    );
    assert!(matches!(
        executor.apply(&second_checkpoint).unwrap(),
        SystemActionOutcome::Checkpoint { .. }
    ));
}
