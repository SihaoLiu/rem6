use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerCheckpointAccess};
use rem6_workload::{
    HostEventIntent, WorkloadExecutionMode, WorkloadGuestHostCallResponse, WorkloadHostEvent,
    WorkloadReplayPlan,
};

use crate::workload_replay::RiscvWorkloadReplayError;
use crate::{
    ExecutionMode, ExecutionModeTarget, GuestEvent, GuestEventDelivery, GuestEventId,
    GuestEventKind, GuestHostCallResponse, GuestSourceId, SystemActionOutcome,
    SystemHostController,
};

const PLANNED_HOST_EVENT_ID_BASE: u64 = 10_000;

pub(crate) enum PlannedHostDeliveryContext<'a> {
    Plain,
    SchedulerCheckpoint {
        component: CheckpointComponentId,
        scheduler: SchedulerCheckpointAccess<'a>,
    },
}

impl PlannedHostDeliveryContext<'_> {
    pub(crate) fn deliver(
        self,
        delivery: GuestEventDelivery,
        controller: &Arc<Mutex<SystemHostController>>,
    ) -> Vec<SystemActionOutcome> {
        let mut controller = controller.lock().expect("system host controller lock");
        match self {
            Self::Plain => controller.handle_delivery(delivery),
            Self::SchedulerCheckpoint {
                component,
                scheduler,
            } => {
                controller.handle_delivery_with_scheduler_checkpoint(delivery, component, scheduler)
            }
        }
    }
}

pub(crate) fn schedule_planned_host_events_with_handler<H>(
    plan: &WorkloadReplayPlan,
    scheduler: &mut PartitionedScheduler,
    controller: &Arc<Mutex<SystemHostController>>,
    host_partition: PartitionId,
    host_source: GuestSourceId,
    scheduler_checkpoint_component: CheckpointComponentId,
    handle_delivery: H,
) -> Result<(), RiscvWorkloadReplayError>
where
    H: for<'a> Fn(
            &WorkloadHostEvent,
            GuestEventDelivery,
            &Arc<Mutex<SystemHostController>>,
            PlannedHostDeliveryContext<'a>,
        ) + Clone
        + Send
        + Sync
        + 'static,
{
    register_planned_guest_host_call_responses(plan, controller);

    for (index, event) in plan.host_events().iter().enumerate() {
        let Some(guest_event) = planned_host_guest_event(event, index, host_source) else {
            continue;
        };
        let delivery_controller = Arc::clone(controller);
        let event = event.clone();
        let handle_delivery = handle_delivery.clone();
        if planned_host_event_requires_scheduler_checkpoint(&event) {
            let component = scheduler_checkpoint_component.clone();
            let scheduled = scheduler
                .schedule_at(host_partition, event.tick(), move |context| {
                    let delivery = GuestEventDelivery::new(
                        context.now(),
                        host_partition,
                        host_partition,
                        guest_event,
                    );
                    handle_delivery(
                        &event,
                        delivery,
                        &delivery_controller,
                        PlannedHostDeliveryContext::SchedulerCheckpoint {
                            component,
                            scheduler: context.checkpoint_access(),
                        },
                    );
                })
                .map_err(RiscvWorkloadReplayError::Scheduler)?;
            let event = scheduler
                .pending_event_snapshot(scheduled)
                .expect("newly scheduled planned host control is pending");
            controller
                .lock()
                .expect("system host controller lock")
                .executor_mut()
                .register_scheduler_checkpoint_control_event(scheduler.instance_id(), event);
        } else {
            scheduler
                .schedule_parallel_at(host_partition, event.tick(), move |context| {
                    let delivery = GuestEventDelivery::new(
                        context.now(),
                        host_partition,
                        host_partition,
                        guest_event,
                    );
                    handle_delivery(
                        &event,
                        delivery,
                        &delivery_controller,
                        PlannedHostDeliveryContext::Plain,
                    );
                })
                .map_err(RiscvWorkloadReplayError::Scheduler)?;
        }
    }

    Ok(())
}

fn planned_host_event_requires_scheduler_checkpoint(event: &WorkloadHostEvent) -> bool {
    matches!(
        event.intent(),
        HostEventIntent::SwitchExecutionMode { .. }
            | HostEventIntent::Checkpoint { .. }
            | HostEventIntent::RestoreCheckpoint { .. }
    )
}

fn register_planned_guest_host_call_responses(
    plan: &WorkloadReplayPlan,
    controller: &Arc<Mutex<SystemHostController>>,
) {
    let mut controller = controller.lock().expect("system host controller lock");
    let executor = controller.executor_mut();
    for event in plan.host_events() {
        if let HostEventIntent::GuestHostCall {
            selector,
            response: Some(response),
            ..
        } = event.intent()
        {
            executor
                .register_guest_host_call_response(*selector, guest_host_call_response(response));
        }
    }
}

fn planned_host_guest_event(
    event: &WorkloadHostEvent,
    index: usize,
    source: GuestSourceId,
) -> Option<GuestEvent> {
    let kind = match event.intent() {
        HostEventIntent::WorkBegin { work_id, thread_id } => GuestEventKind::WorkBegin {
            work_id: *work_id,
            thread_id: *thread_id,
        },
        HostEventIntent::WorkEnd { work_id, thread_id } => GuestEventKind::WorkEnd {
            work_id: *work_id,
            thread_id: *thread_id,
        },
        HostEventIntent::RoiBegin { .. } => GuestEventKind::RoiBegin,
        HostEventIntent::RoiEnd { .. } => GuestEventKind::RoiEnd,
        HostEventIntent::StatsReset { .. } => GuestEventKind::StatsReset,
        HostEventIntent::StatsDump { .. } => GuestEventKind::StatsDump,
        HostEventIntent::SwitchExecutionMode { target, mode } => {
            GuestEventKind::ExecutionModeSwitch {
                target: ExecutionModeTarget::new(target.clone()),
                mode: workload_execution_mode(mode),
            }
        }
        HostEventIntent::GuestHostCall {
            selector,
            arguments,
            payload,
            ..
        } => GuestEventKind::GuestHostCall {
            selector: *selector,
            arguments: arguments.clone(),
            payload: payload.clone(),
        },
        HostEventIntent::Checkpoint { label } => GuestEventKind::Checkpoint {
            label: label.clone(),
        },
        HostEventIntent::RestoreCheckpoint { label } => GuestEventKind::RestoreCheckpoint {
            label: label.clone(),
        },
        HostEventIntent::Stop { .. } => return None,
    };
    Some(GuestEvent::new(
        GuestEventId::new(PLANNED_HOST_EVENT_ID_BASE + index as u64),
        source,
        kind,
    ))
}

fn workload_execution_mode(mode: &WorkloadExecutionMode) -> ExecutionMode {
    match mode {
        WorkloadExecutionMode::Functional => ExecutionMode::Functional,
        WorkloadExecutionMode::Timing => ExecutionMode::Timing,
        WorkloadExecutionMode::Detailed => ExecutionMode::Detailed,
    }
}

fn guest_host_call_response(response: &WorkloadGuestHostCallResponse) -> GuestHostCallResponse {
    GuestHostCallResponse::new(
        response.status(),
        response.return_values().to_vec(),
        response.payload().to_vec(),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rem6_boot::BootImage;
    use rem6_memory::Address;
    use rem6_stats::StatsRegistry;
    use rem6_workload::{WorkloadId, WorkloadManifest};

    use super::*;
    use crate::{
        HostEventPolicy, SchedulerCheckpointBank, SchedulerCheckpointPort, SystemActionOutcome,
    };

    #[test]
    fn planned_checkpoint_preserves_future_scheduler_control() {
        let boot = BootImage::new(Address::new(0x1000))
            .add_segment(Address::new(0x1000), vec![0; 4])
            .unwrap();
        let manifest = WorkloadManifest::builder(
            WorkloadId::new("planned-checkpoint-controls").unwrap(),
            boot,
        )
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::Checkpoint {
                label: "planned".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            2,
            HostEventIntent::RestoreCheckpoint {
                label: "planned".to_string(),
            },
        ))
        .build()
        .unwrap();
        let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        controller
            .lock()
            .unwrap()
            .executor_mut()
            .attach_scheduler_checkpoint_bank(
                SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
                    component.clone(),
                    Arc::clone(&scheduler),
                )])
                .unwrap(),
            )
            .unwrap();
        let mut scheduler = scheduler.lock().unwrap();

        schedule_planned_host_events_with_handler(
            &plan,
            &mut scheduler,
            &controller,
            PartitionId::new(0),
            GuestSourceId::new(1),
            component,
            |_, delivery, controller, delivery_context| {
                delivery_context.deliver(delivery, controller);
            },
        )
        .unwrap();
        scheduler.run_until_idle();
        drop(scheduler);

        let controller = controller.lock().unwrap();
        assert!(controller.action_errors().is_empty());
        assert!(matches!(
            controller.run().action_outcomes(),
            [
                SystemActionOutcome::Checkpoint { manifest, .. },
                SystemActionOutcome::CheckpointRestored {
                    manifest: restored,
                    ..
                }
            ] if manifest.label() == "planned" && restored.label() == "planned"
        ));
    }
}
