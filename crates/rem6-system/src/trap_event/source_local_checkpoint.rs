use super::*;

impl SystemHostEventPort {
    fn emit_with_scheduler_checkpoint_on_source(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
        component: CheckpointComponentId,
    ) -> Result<PartitionEventId, SystemError> {
        let capture_deadline = matches!(
            event.kind(),
            GuestEventKind::Checkpoint { .. } | GuestEventKind::RestoreCheckpoint { .. }
        )
        .then(|| {
            context
                .now()
                .checked_add(self.channel.host_latency())
                .ok_or(SystemError::Scheduler(SchedulerError::TickOverflow {
                    now: context.now(),
                    delay: self.channel.host_latency(),
                }))
        })
        .transpose()?;
        if let Some(deadline) = capture_deadline {
            self.controller
                .lock()
                .expect("system host controller lock")
                .executor()
                .prepare_source_local_checkpoint_capture(deadline);
        }
        let delivery_controller = Arc::clone(&self.controller);
        let registration_controller = Arc::clone(&self.controller);
        let scheduler_event = self.channel.emit_with_scheduler_checkpoint_on_source(
            context,
            event,
            move |delivery, context| {
                handle_host_delivery_with_scheduler_checkpoint(
                    context,
                    delivery,
                    0,
                    component,
                    delivery_controller,
                );
            },
        );
        let scheduler_event = match scheduler_event {
            Ok(event) => event,
            Err(error) => {
                if let Some(deadline) = capture_deadline {
                    self.controller
                        .lock()
                        .expect("system host controller lock")
                        .executor()
                        .release_source_local_checkpoint_capture(deadline);
                }
                return Err(error);
            }
        };
        let scheduler = context.checkpoint_access();
        let event = scheduler
            .pending_event_snapshot(scheduler_event)
            .expect("new source-local scheduler checkpoint control delivery is pending");
        registration_controller
            .lock()
            .expect("system host controller lock")
            .executor_mut()
            .register_scheduler_checkpoint_control_event(scheduler.instance_id(), event);
        Ok(scheduler_event)
    }
}

impl RiscvTrapEventPort {
    fn emit_guest_event_kind_with_source_local_scheduler_checkpoint(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEventId,
        kind: GuestEventKind,
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
        let port = self.clone();
        let scheduler_event = scheduler
            .schedule_at(source, source_tick, move |context| {
                port.emit_guest_event_kind_with_source_local_scheduler_checkpoint(
                    context, event, kind,
                )
                .expect("validated source-local scheduler checkpoint control scheduling");
            })
            .map_err(SystemError::Scheduler)?;
        self.register_scheduler_checkpoint_control_event(scheduler, scheduler_event);
        Ok(scheduler_event)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rem6_checkpoint::CheckpointComponentId;
    use rem6_cpu::{
        CpuCore, CpuFetchConfig, CpuFetchEventKind, CpuId, CpuResetState, RiscvCore,
        RiscvCoreDriveAction,
    };
    use rem6_kernel::{PartitionId, PartitionedScheduler};
    use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryResponse};
    use rem6_stats::StatsRegistry;
    use rem6_transport::{
        MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
    };

    use crate::scheduler_checkpoint::{SchedulerCheckpointBank, SchedulerCheckpointPort};
    use crate::{
        GuestEventId, GuestSourceId, HostEventPolicy, RiscvCoreCheckpointBank,
        RiscvCoreCheckpointPort, SystemError, SystemHostController,
    };

    use super::*;

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
    fn failed_source_local_restore_releases_only_its_prepare_reference() {
        let fixture = fixture(3, 2);
        fixture.core.prepare_source_local_checkpoint_capture(100);
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
        assert!(matches!(
            drive_fetch(&fixture, &mut scheduler),
            Some(RiscvCoreDriveAction::FetchIssued { .. })
        ));
    }

    #[test]
    fn successful_source_local_restore_scrubs_all_prepare_references() {
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
}
