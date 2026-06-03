use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction};
use rem6_isa_riscv::{RiscvTrap, RiscvTrapKind};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    guest_trap_from_riscv, guest_trap_kind_from_riscv, ExecutionMode, ExecutionModeTarget,
    GuestEvent, GuestEventChannel, GuestEventDelivery, GuestEventId, GuestEventKind,
    GuestHostCallResponse, GuestSourceId, GuestTrap, GuestTrapKind, HostAction, HostActionRecord,
    HostEventPolicy, RiscvTrapEventPort, ScheduledRiscvTrap, StopRequest, SystemActionOutcome,
    SystemError, SystemEventPort, SystemHostController, SystemHostEventPort, SystemRunController,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

#[test]
fn riscv_trap_mapping_preserves_illegal_instruction_kind() {
    assert_eq!(
        guest_trap_kind_from_riscv(RiscvTrapKind::IllegalInstruction),
        GuestTrapKind::IllegalInstruction
    );
    assert_eq!(
        guest_trap_from_riscv(RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x7400)),
        GuestTrap::new(GuestTrapKind::IllegalInstruction, 0x7400)
    );
    assert_eq!(GuestTrapKind::IllegalInstruction.default_stop_code(), 2);
}

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
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
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_program_store(instructions: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
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

    let mut image = BootImage::new(Address::new(instructions[0].0));
    for (address, instruction) in instructions {
        image = image
            .add_segment(Address::new(*address), word(*instruction))
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn cpu_route() -> (PartitionedScheduler, MemoryTransport, MemoryRouteId) {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    (
        PartitionedScheduler::with_min_remote_delay(3, 2).unwrap(),
        transport,
        route,
    )
}

fn riscv_core(fetch_route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn riscv_core_on(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn drive_one_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> Option<RiscvCoreDriveAction> {
    let fetch_store = Arc::clone(&store);
    core.drive_next_action(
        scheduler,
        transport,
        MemoryTrace::new(),
        MemoryTrace::new(),
        move |delivery, _context| {
            let response = fetch_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
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
        },
    )
    .unwrap()
}

#[test]
fn guest_events_route_from_guest_to_host_partition() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let channel = GuestEventChannel::new(host, 4).unwrap();
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let first_log = Arc::clone(&delivered);
    let second_log = Arc::clone(&delivered);
    scheduler
        .schedule_at(guest, 3, move |context| {
            channel
                .emit(
                    context,
                    GuestEvent::new(
                        GuestEventId::new(10),
                        GuestSourceId::new(7),
                        GuestEventKind::RoiBegin,
                    ),
                    move |delivery| first_log.lock().unwrap().push(delivery),
                )
                .unwrap();
            channel
                .emit(
                    context,
                    GuestEvent::new(
                        GuestEventId::new(11),
                        GuestSourceId::new(7),
                        GuestEventKind::Terminate { code: 0 },
                    ),
                    move |delivery| second_log.lock().unwrap().push(delivery),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 7);
    assert_eq!(
        delivered.lock().unwrap().as_slice(),
        &[
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(10),
                    GuestSourceId::new(7),
                    GuestEventKind::RoiBegin,
                ),
            ),
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(11),
                    GuestSourceId::new(7),
                    GuestEventKind::Terminate { code: 0 },
                ),
            ),
        ]
    );
}

#[test]
fn guest_event_channel_rejects_zero_host_latency() {
    assert_eq!(
        GuestEventChannel::new(PartitionId::new(1), 0).unwrap_err(),
        SystemError::ZeroHostLatency
    );
}

#[test]
fn guest_host_call_records_typed_outcome_without_stopping() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(31);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(guest, 5, move |context| {
            port.emit(
                context,
                GuestEvent::new(
                    GuestEventId::new(80),
                    source,
                    GuestEventKind::GuestHostCall {
                        selector: 0x1000,
                        arguments: vec![1, 2, 3],
                        payload: vec![0xaa, 0xbb],
                    },
                ),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 7);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().deliveries(),
        &[GuestEventDelivery::new(
            7,
            guest,
            host,
            GuestEvent::new(
                GuestEventId::new(80),
                source,
                GuestEventKind::GuestHostCall {
                    selector: 0x1000,
                    arguments: vec![1, 2, 3],
                    payload: vec![0xaa, 0xbb],
                },
            ),
        )]
    );
    assert_eq!(
        controller.run().action_records(),
        &[HostActionRecord::new(
            7,
            guest,
            host,
            GuestEventId::new(80),
            source,
            HostAction::RecordGuestHostCall {
                selector: 0x1000,
                arguments: vec![1, 2, 3],
                payload: vec![0xaa, 0xbb],
            },
        )]
    );
    assert_eq!(
        controller.run().action_outcomes(),
        &[SystemActionOutcome::GuestHostCall {
            tick: 7,
            event: GuestEventId::new(80),
            source,
            selector: 0x1000,
            arguments: vec![1, 2, 3],
            payload: vec![0xaa, 0xbb],
            response: GuestHostCallResponse::unhandled(),
        }]
    );
    assert_eq!(controller.run().stop_request(), None);
}

#[test]
fn guest_host_call_uses_registered_typed_response() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(32);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    controller
        .lock()
        .unwrap()
        .executor_mut()
        .register_guest_host_call_response(
            0x2200,
            GuestHostCallResponse::ok(vec![55, 89], vec![0xca, 0xfe]),
        );
    let port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(guest, 5, move |context| {
            port.emit(
                context,
                GuestEvent::new(
                    GuestEventId::new(81),
                    source,
                    GuestEventKind::GuestHostCall {
                        selector: 0x2200,
                        arguments: vec![34],
                        payload: Vec::new(),
                    },
                ),
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().action_outcomes(),
        &[SystemActionOutcome::GuestHostCall {
            tick: 7,
            event: GuestEventId::new(81),
            source,
            selector: 0x2200,
            arguments: vec![34],
            payload: Vec::new(),
            response: GuestHostCallResponse::ok(vec![55, 89], vec![0xca, 0xfe]),
        }]
    );
}

#[test]
fn host_event_policy_maps_structured_events_to_actions() {
    let policy = HostEventPolicy;

    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(1),
            GuestSourceId::new(3),
            GuestEventKind::RoiBegin,
        )),
        vec![HostAction::ResetStats]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(2),
            GuestSourceId::new(3),
            GuestEventKind::RoiEnd,
        )),
        vec![HostAction::DumpStats]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(3),
            GuestSourceId::new(3),
            GuestEventKind::Checkpoint {
                label: "warm-boot".to_string(),
            },
        )),
        vec![HostAction::Checkpoint {
            label: "warm-boot".to_string(),
        }]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(33),
            GuestSourceId::new(3),
            GuestEventKind::RestoreCheckpoint {
                label: "warm-boot".to_string(),
            },
        )),
        vec![HostAction::RestoreCheckpointByLabel {
            label: "warm-boot".to_string(),
        }]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(4),
            GuestSourceId::new(3),
            GuestEventKind::Terminate { code: 12 },
        )),
        vec![HostAction::Stop { code: 12 }]
    );
    assert_eq!(
        policy.actions_for(&GuestEvent::new(
            GuestEventId::new(5),
            GuestSourceId::new(3),
            GuestEventKind::ExecutionModeSwitch {
                target: ExecutionModeTarget::new("cpu0"),
                mode: ExecutionMode::Functional,
            },
        )),
        vec![HostAction::SwitchExecutionMode {
            target: ExecutionModeTarget::new("cpu0"),
            mode: ExecutionMode::Functional,
        }]
    );
}

#[test]
fn system_run_controller_records_actions_and_stop_request() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(7);
    let mut controller = SystemRunController::new(HostEventPolicy);

    let roi_records = controller.handle_delivery(GuestEventDelivery::new(
        9,
        guest,
        host,
        GuestEvent::new(GuestEventId::new(30), source, GuestEventKind::RoiBegin),
    ));
    assert_eq!(
        roi_records,
        vec![HostActionRecord::new(
            9,
            guest,
            host,
            GuestEventId::new(30),
            source,
            HostAction::ResetStats,
        )]
    );
    assert_eq!(controller.stop_request(), None);

    let stop_records = controller.handle_delivery(GuestEventDelivery::new(
        12,
        guest,
        host,
        GuestEvent::new(
            GuestEventId::new(31),
            source,
            GuestEventKind::Terminate { code: 5 },
        ),
    ));
    assert_eq!(
        stop_records,
        vec![HostActionRecord::new(
            12,
            guest,
            host,
            GuestEventId::new(31),
            source,
            HostAction::Stop { code: 5 },
        )]
    );
    assert_eq!(
        controller.stop_request(),
        Some(&StopRequest::new(12, GuestEventId::new(31), source, 5))
    );
    assert_eq!(controller.deliveries().len(), 2);
    assert_eq!(
        controller.action_records(),
        &[
            HostActionRecord::new(
                9,
                guest,
                host,
                GuestEventId::new(30),
                source,
                HostAction::ResetStats,
            ),
            HostActionRecord::new(
                12,
                guest,
                host,
                GuestEventId::new(31),
                source,
                HostAction::Stop { code: 5 },
            ),
        ]
    );
}

#[test]
fn system_event_port_delivers_guest_events_into_controller() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(9);
    let controller = Arc::new(Mutex::new(SystemRunController::new(HostEventPolicy)));
    let port = SystemEventPort::new(
        GuestEventChannel::new(host, 2).unwrap(),
        Arc::clone(&controller),
    );
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(guest, 5, move |context| {
            port.emit(
                context,
                GuestEvent::new(GuestEventId::new(40), source, GuestEventKind::RoiBegin),
            )
            .unwrap();
            port.emit(
                context,
                GuestEvent::new(
                    GuestEventId::new(41),
                    source,
                    GuestEventKind::Checkpoint {
                        label: "booted".to_string(),
                    },
                ),
            )
            .unwrap();
            port.emit(
                context,
                GuestEvent::new(
                    GuestEventId::new(42),
                    source,
                    GuestEventKind::Terminate { code: 0 },
                ),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 4);
    assert_eq!(summary.final_tick(), 7);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.deliveries(),
        &[
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(GuestEventId::new(40), source, GuestEventKind::RoiBegin),
            ),
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(41),
                    source,
                    GuestEventKind::Checkpoint {
                        label: "booted".to_string(),
                    },
                ),
            ),
            GuestEventDelivery::new(
                7,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(42),
                    source,
                    GuestEventKind::Terminate { code: 0 },
                ),
            ),
        ]
    );
    assert_eq!(
        controller.action_records(),
        &[
            HostActionRecord::new(
                7,
                guest,
                host,
                GuestEventId::new(40),
                source,
                HostAction::ResetStats,
            ),
            HostActionRecord::new(
                7,
                guest,
                host,
                GuestEventId::new(41),
                source,
                HostAction::Checkpoint {
                    label: "booted".to_string(),
                },
            ),
            HostActionRecord::new(
                7,
                guest,
                host,
                GuestEventId::new(42),
                source,
                HostAction::Stop { code: 0 },
            ),
        ]
    );
    assert_eq!(
        controller.stop_request(),
        Some(&StopRequest::new(7, GuestEventId::new(42), source, 0))
    );
}

#[test]
fn riscv_trap_event_port_routes_traps_into_host_controller() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(13);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(guest, 5, move |context| {
            trap_port
                .emit(
                    context,
                    GuestEventId::new(50),
                    RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 7);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().deliveries(),
        &[GuestEventDelivery::new(
            7,
            guest,
            host,
            GuestEvent::new(
                GuestEventId::new(50),
                source,
                GuestEventKind::Trap {
                    trap: GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000),
                },
            ),
        )]
    );
    assert_eq!(
        controller.run().action_records(),
        &[HostActionRecord::new(
            7,
            guest,
            host,
            GuestEventId::new(50),
            source,
            HostAction::Stop { code: 0 },
        )]
    );
    assert_eq!(
        controller.run().action_outcomes(),
        &[SystemActionOutcome::Stop(StopRequest::new(
            7,
            GuestEventId::new(50),
            source,
            0,
        ))]
    );
}

#[test]
fn riscv_trap_event_port_emits_pending_core_trap_into_host_controller() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(2);
    let source = GuestSourceId::new(21);
    let (mut scheduler, transport, fetch_route) = cpu_route();
    let core = riscv_core(fetch_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(event) = action else {
        panic!("expected trap execution event");
    };
    assert_eq!(
        event.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );
    assert!(core.has_pending_trap());

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let emit_tick = scheduler.now();
    let pending_core = core.clone();
    scheduler
        .schedule_at(guest, emit_tick, move |context| {
            let event = trap_port
                .emit_pending_core_trap(context, GuestEventId::new(60), &pending_core)
                .unwrap();
            assert!(event.is_some());
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), emit_tick + 2);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().deliveries(),
        &[GuestEventDelivery::new(
            emit_tick + 2,
            guest,
            host,
            GuestEvent::new(
                GuestEventId::new(60),
                source,
                GuestEventKind::Trap {
                    trap: GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000),
                },
            ),
        )]
    );
    assert_eq!(
        controller.run().action_records(),
        &[HostActionRecord::new(
            emit_tick + 2,
            guest,
            host,
            GuestEventId::new(60),
            source,
            HostAction::Stop { code: 0 },
        )]
    );
    assert_eq!(
        controller.run().action_outcomes(),
        &[SystemActionOutcome::Stop(StopRequest::new(
            emit_tick + 2,
            GuestEventId::new(60),
            source,
            0,
        ))]
    );
}

#[test]
fn riscv_trap_event_port_schedules_pending_core_trap_from_core_partition() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(2);
    let source = GuestSourceId::new(22);
    let (mut scheduler, transport, fetch_route) = cpu_route();
    let core = riscv_core(fetch_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    assert!(matches!(
        action,
        RiscvCoreDriveAction::InstructionExecuted(_)
    ));
    assert!(core.has_pending_trap());

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let emit_tick = scheduler.partition_now(guest).unwrap();
    let event = trap_port
        .schedule_pending_core_trap(&mut scheduler, GuestEventId::new(61), &core)
        .unwrap()
        .unwrap();

    assert_eq!(event.partition(), guest);

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), emit_tick + 2);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().deliveries(),
        &[GuestEventDelivery::new(
            emit_tick + 2,
            guest,
            host,
            GuestEvent::new(
                GuestEventId::new(61),
                source,
                GuestEventKind::Trap {
                    trap: GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000),
                },
            ),
        )]
    );
    assert_eq!(
        controller.run().stop_request(),
        Some(&StopRequest::new(
            emit_tick + 2,
            GuestEventId::new(61),
            source,
            0,
        ))
    );
}

#[test]
fn riscv_trap_event_port_schedules_pending_core_trap_on_parallel_scheduler() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(2);
    let source = GuestSourceId::new(25);
    let (mut scheduler, transport, fetch_route) = cpu_route();
    let core = riscv_core(fetch_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    assert!(matches!(
        action,
        RiscvCoreDriveAction::InstructionExecuted(_)
    ));
    assert!(core.has_pending_trap());

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let emit_tick = scheduler.partition_now(guest).unwrap();
    let event = trap_port
        .schedule_pending_core_trap_parallel(&mut scheduler, GuestEventId::new(63), &core)
        .unwrap()
        .unwrap();

    assert_eq!(event.partition(), guest);

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 2);
    assert!(summary.final_tick() >= emit_tick + 2);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().deliveries(),
        &[GuestEventDelivery::new(
            emit_tick + 2,
            guest,
            host,
            GuestEvent::new(
                GuestEventId::new(63),
                source,
                GuestEventKind::Trap {
                    trap: GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000),
                },
            ),
        )]
    );
    assert_eq!(
        controller.run().stop_request(),
        Some(&StopRequest::new(
            emit_tick + 2,
            GuestEventId::new(63),
            source,
            0,
        ))
    );
}

#[test]
fn riscv_trap_event_port_reports_serial_trap_lookahead_boundary() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(2);
    let source = GuestSourceId::new(25);
    let (mut scheduler, transport, fetch_route) = cpu_route();
    let core = riscv_core(fetch_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    assert!(matches!(
        action,
        RiscvCoreDriveAction::InstructionExecuted(_)
    ));
    assert!(core.has_pending_trap());

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 1, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let source_tick = scheduler.partition_now(guest).unwrap();

    assert_eq!(
        trap_port
            .schedule_pending_core_trap(&mut scheduler, GuestEventId::new(65), &core)
            .unwrap_err(),
        SystemError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: guest,
            target: host,
            source_tick,
            delivery_tick: source_tick + 1,
            minimum_delivery_tick: source_tick + 2,
        })
    );
    assert_eq!(scheduler.next_pending_tick(guest).unwrap(), None);
    assert!(controller.lock().unwrap().run().deliveries().is_empty());
}

#[test]
fn riscv_trap_event_port_reports_parallel_trap_lookahead_boundary() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(2);
    let source = GuestSourceId::new(26);
    let (mut scheduler, transport, fetch_route) = cpu_route();
    let core = riscv_core(fetch_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    assert!(matches!(
        action,
        RiscvCoreDriveAction::InstructionExecuted(_)
    ));
    assert!(core.has_pending_trap());

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 1, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let source_tick = scheduler.partition_now(guest).unwrap();

    assert_eq!(
        trap_port
            .schedule_pending_core_trap_parallel(&mut scheduler, GuestEventId::new(64), &core)
            .unwrap_err(),
        SystemError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: guest,
            target: host,
            source_tick,
            delivery_tick: source_tick + 1,
            minimum_delivery_tick: source_tick + 2,
        })
    );
    assert_eq!(scheduler.next_pending_tick(guest).unwrap(), None);
    assert!(controller.lock().unwrap().run().deliveries().is_empty());
}

#[test]
fn riscv_trap_event_port_rejects_scheduled_core_trap_for_unknown_host_partition() {
    let guest = PartitionId::new(0);
    let missing_host = PartitionId::new(9);
    let source = GuestSourceId::new(23);
    let (mut scheduler, transport, fetch_route) = cpu_route();
    let core = riscv_core(fetch_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    assert!(matches!(
        action,
        RiscvCoreDriveAction::InstructionExecuted(_)
    ));

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port =
        SystemHostEventPort::with_controller(missing_host, 2, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);

    assert_eq!(
        trap_port
            .schedule_pending_core_trap(&mut scheduler, GuestEventId::new(62), &core)
            .unwrap_err(),
        SystemError::Scheduler(SchedulerError::UnknownPartition {
            partition: missing_host,
            partitions: 3,
        })
    );
    assert_eq!(scheduler.next_pending_tick(guest).unwrap(), None);
    assert!(controller.lock().unwrap().run().deliveries().is_empty());
}

#[test]
fn riscv_trap_event_port_schedules_pending_traps_for_multiple_cores() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(24);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core_on(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core_on(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    let store = loaded_program_store(&[(0x8000, 0x0000_0073), (0x9000, 0x0010_0073)]);

    assert!(matches!(
        drive_one_action(&core0, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert!(matches!(
        drive_one_action(&core1, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_action(&core0, Arc::clone(&store), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_action(&core1, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let host_port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let trap_port = RiscvTrapEventPort::new(host_port, source);
    let emit_tick = scheduler.now();
    let scheduled = trap_port
        .schedule_pending_core_traps(&mut scheduler, [core0.clone(), core1.clone()], |cpu| {
            GuestEventId::new(70 + u64::from(cpu.get()))
        })
        .unwrap();

    assert_eq!(scheduled.len(), 2);
    assert_eq!(scheduled[0].cpu(), CpuId::new(0));
    assert_eq!(scheduled[0].event(), GuestEventId::new(70));
    assert_eq!(scheduled[0].source_partition(), PartitionId::new(0));
    assert_eq!(
        scheduled[0].scheduler_event().partition(),
        PartitionId::new(0)
    );
    assert_eq!(
        scheduled[0].trap(),
        GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000)
    );
    assert_eq!(
        scheduled[1],
        ScheduledRiscvTrap::new(
            CpuId::new(1),
            GuestEventId::new(71),
            PartitionId::new(1),
            scheduled[1].scheduler_event(),
            GuestTrap::new(GuestTrapKind::Breakpoint, 0x9000),
        )
    );

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 4);
    assert_eq!(summary.final_tick(), emit_tick + 2);

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().deliveries(),
        &[
            GuestEventDelivery::new(
                emit_tick + 2,
                PartitionId::new(0),
                host,
                GuestEvent::new(
                    GuestEventId::new(70),
                    source,
                    GuestEventKind::Trap {
                        trap: GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000),
                    },
                ),
            ),
            GuestEventDelivery::new(
                emit_tick + 2,
                PartitionId::new(1),
                host,
                GuestEvent::new(
                    GuestEventId::new(71),
                    source,
                    GuestEventKind::Trap {
                        trap: GuestTrap::new(GuestTrapKind::Breakpoint, 0x9000),
                    },
                ),
            ),
        ]
    );
    assert_eq!(
        controller.run().action_outcomes(),
        &[
            SystemActionOutcome::Stop(StopRequest::new(
                emit_tick + 2,
                GuestEventId::new(70),
                source,
                0,
            )),
            SystemActionOutcome::Stop(StopRequest::new(
                emit_tick + 2,
                GuestEventId::new(71),
                source,
                1,
            )),
        ]
    );
}
