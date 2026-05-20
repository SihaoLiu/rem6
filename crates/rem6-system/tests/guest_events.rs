use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction};
use rem6_isa_riscv::{RiscvTrap, RiscvTrapKind};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEvent, GuestEventChannel, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId,
    GuestTrap, GuestTrapKind, HostAction, HostActionRecord, HostEventPolicy, RiscvTrapEventPort,
    StopRequest, SystemActionOutcome, SystemError, SystemEventPort, SystemHostController,
    SystemHostEventPort, SystemRunController,
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
            GuestEventId::new(4),
            GuestSourceId::new(3),
            GuestEventKind::Terminate { code: 12 },
        )),
        vec![HostAction::Stop { code: 12 }]
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
