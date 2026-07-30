use super::*;

#[path = "source_local_checkpoint/restore_fence.rs"]
mod restore_fence;

use restore_fence::SourceLocalRestoreFenceOwnership;

impl RiscvTrapEventPort {
    fn emit_guest_event_kind_with_source_local_scheduler_checkpoint(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEventId,
        kind: GuestEventKind,
        capture_prepared: bool,
        restore_fence_ownership: Option<Arc<SourceLocalRestoreFenceOwnership>>,
    ) -> Result<PartitionEventId, SystemError> {
        let component =
            self.scheduler_checkpoint_component
                .clone()
                .ok_or(SystemError::SchedulerCheckpoint(
                    SchedulerCheckpointError::BorrowedSchedulerContextRequired,
                ))?;
        self.host.emit_with_scheduler_checkpoint_on_source(
            context,
            GuestEvent::new(event, self.source, kind),
            component,
            capture_prepared,
            restore_fence_ownership,
        )
    }

    pub fn schedule_host_checkpoint_event_on_source_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        label: String,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_source_local_host_control_event_kind(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::Checkpoint { label },
        )
    }

    pub fn schedule_host_checkpoint_restore_event_on_source_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        label: String,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_source_local_host_control_event_kind(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::RestoreCheckpoint { label },
        )
    }

    pub(super) fn schedule_source_local_host_control_event_kind(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        kind: GuestEventKind,
    ) -> Result<PartitionEventId, SystemError> {
        self.validate_parallel_scheduled_emit(scheduler, source, source_tick)?;
        if self.scheduler_checkpoint_component.is_none() {
            return Err(SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerContextRequired,
            ));
        }
        let restore_fence_ownership = matches!(kind, GuestEventKind::RestoreCheckpoint { .. })
            .then(|| Arc::new(SourceLocalRestoreFenceOwnership::default()));
        let preparation = if matches!(kind, GuestEventKind::RestoreCheckpoint { .. }) {
            let deadline = source_tick
                .checked_add(self.host.channel.host_latency())
                .ok_or(SystemError::Scheduler(SchedulerError::TickOverflow {
                    now: source_tick,
                    delay: self.host.channel.host_latency(),
                }))?;
            let cleanup_controller = Arc::clone(&self.host.controller);
            let cleanup_ownership = Arc::clone(
                restore_fence_ownership
                    .as_ref()
                    .expect("restore scheduling owns a fence release token"),
            );
            let cleanup = scheduler
                .schedule_at(source, deadline, move |_| {
                    if !cleanup_ownership.is_activated() {
                        cleanup_ownership.release_capture_once(&cleanup_controller, deadline);
                    }
                })
                .map_err(SystemError::Scheduler)?;
            let preflight = match source_tick
                .checked_sub(1)
                .filter(|prepare_tick| *prepare_tick >= scheduler.now())
            {
                Some(prepare_tick) => {
                    let prepare_controller = Arc::clone(&self.host.controller);
                    let prepared = scheduler.schedule_at(source, prepare_tick, move |_| {
                        let controller = prepare_controller
                            .lock()
                            .expect("system host controller lock");
                        controller
                            .executor()
                            .prepare_source_local_checkpoint_capture(deadline);
                    });
                    match prepared {
                        Ok(prepared) => Some(prepared),
                        Err(error) => {
                            scheduler
                                .cancel_event(cleanup)
                                .expect("new restore cleanup event remains pending");
                            return Err(SystemError::Scheduler(error));
                        }
                    }
                }
                None => {
                    let controller = self
                        .host
                        .controller
                        .lock()
                        .expect("system host controller lock");
                    controller
                        .executor()
                        .prepare_source_local_checkpoint_capture(deadline);
                    None
                }
            };
            Some((preflight, cleanup, deadline))
        } else {
            None
        };
        let port = self.clone();
        let capture_prepared = preparation.is_some();
        let preflight = preparation.and_then(|(preflight, _, _)| preflight);
        let restore_deadline = preparation.map(|(_, _, deadline)| deadline);
        let source_fence_ownership = restore_fence_ownership.clone();
        let scheduler_event = match scheduler.schedule_at(source, source_tick, move |context| {
            let result = port.emit_guest_event_kind_with_source_local_scheduler_checkpoint(
                context,
                event,
                kind,
                capture_prepared,
                source_fence_ownership.clone(),
            );
            if let (Ok(_), Some(ownership), Some(deadline)) =
                (&result, source_fence_ownership.as_ref(), restore_deadline)
            {
                let fallback_ownership = Arc::clone(ownership);
                let fallback_controller = Arc::clone(&port.host.controller);
                let fallback = context
                    .schedule_local_after(port.host.channel.host_latency(), move |_| {
                        fallback_ownership.release_restore_once(
                            &fallback_controller,
                            source_tick,
                            deadline,
                        );
                    })
                    .expect("validated restore fallback cleanup scheduling");
                let scheduler = context.checkpoint_access();
                let event = scheduler
                    .pending_event_snapshot(fallback)
                    .expect("new restore fallback cleanup is pending");
                port.host
                    .controller
                    .lock()
                    .expect("system host controller lock")
                    .executor_mut()
                    .register_scheduler_checkpoint_control_event(scheduler.instance_id(), event);
                ownership.activate();
            }
            if result.is_err() {
                if let Some(preflight) = preflight.filter(|preflight| {
                    context
                        .checkpoint_access()
                        .pending_event_snapshot(*preflight)
                        .is_some()
                }) {
                    context
                        .checkpoint_access()
                        .cancel_event(preflight)
                        .expect("restore preflight remains pending after source emit failure");
                }
            }
            result.expect("validated source-local scheduler checkpoint control scheduling");
        }) {
            Ok(event) => event,
            Err(error) => {
                if let Some((preparation, cleanup, deadline)) = preparation {
                    if let Some(preparation) = preparation {
                        scheduler
                            .cancel_event(preparation)
                            .expect("new restore preparation event remains pending");
                    } else {
                        let controller = self
                            .host
                            .controller
                            .lock()
                            .expect("system host controller lock");
                        controller
                            .executor()
                            .release_source_local_checkpoint_capture(deadline);
                    }
                    scheduler
                        .cancel_event(cleanup)
                        .expect("new restore cleanup event remains pending");
                }
                return Err(SystemError::Scheduler(error));
            }
        };
        if let Some((preparation, cleanup, _)) = preparation {
            if let Some(preparation) = preparation {
                self.register_scheduler_checkpoint_control_event(scheduler, preparation);
            }
            self.register_scheduler_checkpoint_control_event(scheduler, cleanup);
        }
        self.register_scheduler_checkpoint_control_event(scheduler, scheduler_event);
        Ok(scheduler_event)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use rem6_checkpoint::CheckpointComponentId;
    use rem6_cpu::{
        CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEventKind, CpuId, CpuResetState,
        RiscvCluster, RiscvCore, RiscvCoreDriveAction, RiscvDataAccessEventKind,
    };
    use rem6_kernel::{PartitionId, PartitionedScheduler};
    use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryResponse};
    use rem6_stats::{CommMonitorConfig, ProbePayload, StackDistProbeConfig, StatsRegistry};
    use rem6_transport::{
        MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
    };

    use crate::scheduler_checkpoint::{SchedulerCheckpointBank, SchedulerCheckpointPort};
    use crate::{
        GuestEventId, GuestSourceId, HostEventPolicy, RiscvCoreCheckpointBank,
        RiscvCoreCheckpointPort, RiscvDataAccessStats, RiscvSystemRunDriver, SystemActionOutcome,
        SystemError, SystemHostController,
    };

    use super::*;

    #[path = "restore_fences.rs"]
    mod restore_fences;

    struct SourceLocalFixture {
        controller: Arc<Mutex<SystemHostController>>,
        trap: RiscvTrapEventPort,
        scheduler: Arc<Mutex<PartitionedScheduler>>,
        core: RiscvCore,
        transport: MemoryTransport,
    }

    fn endpoint(name: &str) -> TransportEndpointId {
        TransportEndpointId::new(name).unwrap()
    }

    fn fixture(host_latency: u64, route_delay: u64) -> SourceLocalFixture {
        let source = PartitionId::new(0);
        let target = PartitionId::new(1);
        let fetch_endpoint = endpoint("cpu0.ifetch");
        let mut transport = MemoryTransport::new();
        let route = transport
            .add_route(
                MemoryRoute::new(
                    fetch_endpoint.clone(),
                    source,
                    endpoint("l1i"),
                    target,
                    route_delay,
                    route_delay,
                )
                .unwrap(),
            )
            .unwrap();
        let core = RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(CpuId::new(0), source, AgentId::new(0), Address::new(0x8000)),
                CpuFetchConfig::new(
                    fetch_endpoint,
                    route,
                    CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
        );
        let scheduler_component = CheckpointComponentId::new("scheduler0").unwrap();
        let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(2).unwrap()));
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        {
            let mut controller = controller.lock().unwrap();
            controller
                .executor_mut()
                .attach_riscv_checkpoint_bank(
                    RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
                        cpu_component,
                        core.clone(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            controller
                .executor_mut()
                .attach_scheduler_checkpoint_bank(
                    SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
                        scheduler_component.clone(),
                        Arc::clone(&scheduler),
                    )])
                    .unwrap(),
                )
                .unwrap();
        }
        let host =
            SystemHostEventPort::with_controller(source, host_latency, Arc::clone(&controller))
                .unwrap();
        let trap = RiscvTrapEventPort::new(host, GuestSourceId::new(1))
            .with_scheduler_checkpoint_component(scheduler_component);
        SourceLocalFixture {
            controller,
            trap,
            scheduler,
            core,
            transport,
        }
    }

    fn drive_fetch(
        fixture: &SourceLocalFixture,
        scheduler: &mut PartitionedScheduler,
    ) -> Option<RiscvCoreDriveAction> {
        fixture
            .core
            .drive_next_action(
                scheduler,
                &fixture.transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_delivery, _context| TargetOutcome::NoResponse,
                |_delivery, _context| TargetOutcome::NoResponse,
            )
            .unwrap()
    }

    fn run_until_next_host_result(
        fixture: &SourceLocalFixture,
        scheduler: &mut PartitionedScheduler,
    ) {
        let initial = {
            let controller = fixture.controller.lock().unwrap();
            controller.run().action_outcomes().len() + controller.action_errors().len()
        };
        for _ in 0..16 {
            scheduler.run_next_epoch();
            let observed = {
                let controller = fixture.controller.lock().unwrap();
                controller.run().action_outcomes().len() + controller.action_errors().len()
            };
            if observed > initial {
                return;
            }
        }
        panic!("source-local host delivery did not complete");
    }

    #[test]
    fn successful_source_local_checkpoint_releases_prepare_at_delivery() {
        let fixture = fixture(3, 2);
        let mut scheduler = fixture.scheduler.lock().unwrap();
        fixture
            .trap
            .schedule_host_checkpoint_event_on_source_parallel(
                &mut scheduler,
                GuestEventId::new(1),
                PartitionId::new(0),
                0,
                "success".to_string(),
            )
            .unwrap();

        run_until_next_host_result(&fixture, &mut scheduler);

        assert_eq!(scheduler.now(), 3);
        assert!(fixture
            .controller
            .lock()
            .unwrap()
            .action_errors()
            .is_empty());
        assert!(matches!(
            drive_fetch(&fixture, &mut scheduler),
            Some(RiscvCoreDriveAction::FetchIssued { .. })
        ));
    }

    #[test]
    fn failed_source_local_restore_preserves_foreign_prepare_references() {
        let fixture = fixture(3, 2);
        fixture.core.prepare_source_local_checkpoint_capture(100);
        fixture.core.prepare_source_local_checkpoint_restore(100);
        let mut scheduler = fixture.scheduler.lock().unwrap();
        fixture
            .trap
            .schedule_host_checkpoint_restore_event_on_source_parallel(
                &mut scheduler,
                GuestEventId::new(1),
                PartitionId::new(0),
                0,
                "missing".to_string(),
            )
            .unwrap();

        run_until_next_host_result(&fixture, &mut scheduler);

        assert_eq!(scheduler.now(), 3);
        assert!(matches!(
            fixture.controller.lock().unwrap().action_errors(),
            [SystemError::MissingCheckpointManifest { label }] if label == "missing"
        ));
        assert!(drive_fetch(&fixture, &mut scheduler).is_none());
        fixture.core.release_source_local_checkpoint_capture(100);
        assert!(drive_fetch(&fixture, &mut scheduler).is_none());
        fixture.core.release_source_local_checkpoint_restore(100);
        assert!(matches!(
            drive_fetch(&fixture, &mut scheduler),
            Some(RiscvCoreDriveAction::FetchIssued { .. })
        ));
    }

    #[test]
    fn successful_source_local_restore_preserves_foreign_prepare_references() {
        let fixture = fixture(3, 2);
        let mut scheduler = fixture.scheduler.lock().unwrap();
        fixture
            .trap
            .schedule_host_checkpoint_event_on_source_parallel(
                &mut scheduler,
                GuestEventId::new(1),
                PartitionId::new(0),
                0,
                "restore".to_string(),
            )
            .unwrap();
        run_until_next_host_result(&fixture, &mut scheduler);
        fixture.core.prepare_source_local_checkpoint_capture(100);
        fixture.core.prepare_source_local_checkpoint_restore(100);
        let restore_source_tick = scheduler.now() + 1;
        fixture
            .trap
            .schedule_host_checkpoint_restore_event_on_source_parallel(
                &mut scheduler,
                GuestEventId::new(2),
                PartitionId::new(0),
                restore_source_tick,
                "restore".to_string(),
            )
            .unwrap();

        run_until_next_host_result(&fixture, &mut scheduler);

        assert!(fixture
            .controller
            .lock()
            .unwrap()
            .action_errors()
            .is_empty());
        assert!(drive_fetch(&fixture, &mut scheduler).is_none());
        fixture.core.release_source_local_checkpoint_capture(100);
        assert!(drive_fetch(&fixture, &mut scheduler).is_none());
        fixture.core.release_source_local_checkpoint_restore(100);
        assert!(matches!(
            drive_fetch(&fixture, &mut scheduler),
            Some(RiscvCoreDriveAction::FetchIssued { .. })
        ));
    }

    #[test]
    fn rejected_checkpoint_does_not_cancel_outstanding_fetch() {
        let fixture = fixture(2, 4);
        let mut scheduler = fixture.scheduler.lock().unwrap();
        fixture
            .core
            .issue_next_fetch(
                &mut scheduler,
                &fixture.transport,
                MemoryTrace::new(),
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(
                            delivery.request(),
                            Some(0x0000_0013_u32.to_le_bytes().to_vec()),
                        )
                        .unwrap(),
                    )
                },
            )
            .unwrap();
        fixture
            .trap
            .schedule_host_checkpoint_event_on_source_parallel(
                &mut scheduler,
                GuestEventId::new(1),
                PartitionId::new(0),
                1,
                "outstanding".to_string(),
            )
            .unwrap();

        run_until_next_host_result(&fixture, &mut scheduler);

        assert_eq!(scheduler.now(), 3);
        assert_eq!(fixture.controller.lock().unwrap().action_errors().len(), 1);
        assert_eq!(
            fixture
                .core
                .inner()
                .fetch_events()
                .iter()
                .map(|event| event.kind())
                .collect::<Vec<_>>(),
            vec![CpuFetchEventKind::Issued]
        );
        assert_eq!(scheduler.snapshot().total_pending_events(), 1);

        scheduler.run_until_idle_conservative();

        assert_eq!(
            fixture
                .core
                .inner()
                .fetch_events()
                .iter()
                .map(|event| event.kind())
                .collect::<Vec<_>>(),
            vec![CpuFetchEventKind::Issued, CpuFetchEventKind::Completed]
        );
    }

    #[test]
    fn serial_checkpoint_captures_unsynchronized_data_completion_before_restore_replay() {
        let source = PartitionId::new(0);
        let target = PartitionId::new(1);
        let cpu = CpuId::new(0);
        let fetch_endpoint = endpoint("cpu0.ifetch");
        let data_endpoint = endpoint("cpu0.dmem");
        let mut transport = MemoryTransport::new();
        let fetch_route = transport
            .add_route(
                MemoryRoute::new(
                    fetch_endpoint.clone(),
                    source,
                    endpoint("l1i"),
                    target,
                    2,
                    3,
                )
                .unwrap(),
            )
            .unwrap();
        let data_route = transport
            .add_route(
                MemoryRoute::new(data_endpoint.clone(), source, endpoint("l1d"), target, 2, 3)
                    .unwrap(),
            )
            .unwrap();
        let core = RiscvCore::with_data(
            CpuCore::new(
                CpuResetState::new(cpu, source, AgentId::new(7), Address::new(0x8000)),
                CpuFetchConfig::new(
                    fetch_endpoint,
                    fetch_route,
                    CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
            CpuDataConfig::new(data_endpoint, data_route, CacheLineLayout::new(16).unwrap()),
        );
        core.write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9000);
        let cluster = RiscvCluster::new([core.clone()]).unwrap();
        let scheduler_component = CheckpointComponentId::new("scheduler0").unwrap();
        let scheduler = Arc::new(Mutex::new(
            PartitionedScheduler::with_min_remote_delay(2, 2).unwrap(),
        ));
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        {
            let mut controller = controller.lock().unwrap();
            controller
                .executor_mut()
                .attach_riscv_checkpoint_bank(
                    RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
                        CheckpointComponentId::new("cpu0").unwrap(),
                        core.clone(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            controller
                .executor_mut()
                .attach_scheduler_checkpoint_bank(
                    SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
                        scheduler_component.clone(),
                        Arc::clone(&scheduler),
                    )])
                    .unwrap(),
                )
                .unwrap();
        }
        let trap = RiscvTrapEventPort::new(
            SystemHostEventPort::with_controller(source, 1, Arc::clone(&controller)).unwrap(),
            GuestSourceId::new(1),
        )
        .with_scheduler_checkpoint_component(scheduler_component);
        let driver = RiscvSystemRunDriver::new(trap.clone()).with_data_access_stats(
            RiscvDataAccessStats::with_stack_distance(
                StackDistProbeConfig::builder(16, 16).build().unwrap(),
            )
            .with_comm_monitor(CommMonitorConfig::builder(100).build().unwrap()),
        );
        let data_requests = Arc::new(AtomicUsize::new(0));
        let mut scheduler = scheduler.lock().unwrap();

        let run = driver
            .drive_until_host_stop(
                &cluster,
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    move |delivery, _context| {
                        let instruction: u32 = match delivery.request().range().start().get() {
                            0x8000 => 0x0001_2283,
                            0x8004 => 0x0041_2303,
                            0x8008 => 0x0000_0073,
                            address => panic!("unexpected fetch address {address:#x}"),
                        };
                        TargetOutcome::Respond(
                            MemoryResponse::completed(
                                delivery.request(),
                                Some(instruction.to_le_bytes().to_vec()),
                            )
                            .unwrap(),
                        )
                    }
                },
                |_cpu| {
                    let data_requests = Arc::clone(&data_requests);
                    let trap = trap.clone();
                    move |delivery, context| {
                        let request_index = data_requests.fetch_add(1, Ordering::SeqCst);
                        let control = match request_index {
                            0 => Some((
                                GuestEventId::new(1),
                                GuestEventKind::Checkpoint {
                                    label: "data-boundary".to_string(),
                                },
                            )),
                            1 => Some((
                                GuestEventId::new(2),
                                GuestEventKind::RestoreCheckpoint {
                                    label: "data-boundary".to_string(),
                                },
                            )),
                            _ => None,
                        };
                        if let Some((event, kind)) = control {
                            context
                                .schedule_remote_after(source, 2, move |context| {
                                    trap
                                        .emit_guest_event_kind_with_source_local_scheduler_checkpoint(
                                            context, event, kind, false, None,
                                        )
                                        .unwrap();
                                })
                                .unwrap();
                        }
                        TargetOutcome::Respond(
                            MemoryResponse::completed(
                                delivery.request(),
                                Some(vec![request_index as u8 + 1, 0, 0, 0]),
                            )
                            .unwrap(),
                        )
                    }
                },
                100,
                |_cpu| GuestEventId::new(100),
            )
            .unwrap();

        assert_eq!(data_requests.load(Ordering::SeqCst), 3);
        let history = core.data_access_events();
        assert_eq!(
            history
                .iter()
                .filter(|event| event.kind() == RiscvDataAccessEventKind::Issued)
                .count(),
            3
        );
        assert_eq!(
            history
                .iter()
                .filter(|event| event.kind() == RiscvDataAccessEventKind::Completed)
                .count(),
            3
        );
        let checkpoint_tick = controller
            .lock()
            .unwrap()
            .run()
            .action_outcomes()
            .iter()
            .find_map(|outcome| match outcome {
                SystemActionOutcome::Checkpoint { tick, .. } => Some(*tick),
                _ => None,
            })
            .expect("checkpoint outcome");
        assert_eq!(
            history
                .iter()
                .find(|event| event.kind() == RiscvDataAccessEventKind::Completed)
                .unwrap()
                .tick(),
            checkpoint_tick
        );
        assert!(run
            .turns()
            .iter()
            .filter_map(rem6_cpu::RiscvClusterTurn::serial_scheduler_summary)
            .any(
                |summary| summary.final_tick() == checkpoint_tick && summary.executed_events() >= 2
            ));
        assert!(controller.lock().unwrap().action_errors().is_empty());

        let probes = driver
            .data_access_stats()
            .unwrap()
            .data_access_probe_snapshot();
        let request_point = probes.request_point();
        let response_point = probes.response_point().unwrap();
        let observed = probes
            .probes()
            .events()
            .iter()
            .map(|event| {
                let ProbePayload::MemoryPacket(packet) = event.payload() else {
                    panic!("unexpected data probe payload: {:?}", event.payload());
                };
                (event.point(), packet.packet_id())
            })
            .collect::<Vec<_>>();
        let issued_packet_ids = history
            .iter()
            .filter(|event| event.kind() == RiscvDataAccessEventKind::Issued)
            .map(|event| {
                (u64::from(event.request_id().agent().get()) << 32)
                    | (event.request_id().sequence() & u64::from(u32::MAX))
            })
            .collect::<Vec<_>>();
        assert_eq!(observed.len(), 4, "{observed:?}");
        assert_eq!(observed[0], (request_point, issued_packet_ids[0]));
        assert_eq!(observed[1], (response_point, issued_packet_ids[0]));
        assert!(!observed
            .iter()
            .any(|(_point, packet_id)| *packet_id == issued_packet_ids[1]));
        assert_eq!(observed[2], (request_point, issued_packet_ids[2]));
        assert_eq!(observed[3], (response_point, issued_packet_ids[2]));
    }
}
