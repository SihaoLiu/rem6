use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_workload::{
    HostEventIntent, WorkloadExecutionMode, WorkloadGuestHostCallResponse, WorkloadHostEvent,
    WorkloadReplayPlan,
};

use crate::workload_replay::RiscvWorkloadReplayError;
use crate::{
    ExecutionMode, ExecutionModeTarget, GuestEvent, GuestEventDelivery, GuestEventId,
    GuestEventKind, GuestHostCallResponse, GuestSourceId, SystemHostController,
};

const PLANNED_HOST_EVENT_ID_BASE: u64 = 10_000;

pub(crate) fn schedule_planned_host_events(
    plan: &WorkloadReplayPlan,
    scheduler: &mut PartitionedScheduler,
    controller: &Arc<Mutex<SystemHostController>>,
    host_partition: PartitionId,
    host_source: GuestSourceId,
) -> Result<(), RiscvWorkloadReplayError> {
    register_planned_guest_host_call_responses(plan, controller);

    for (index, event) in plan.host_events().iter().enumerate() {
        let Some(guest_event) = planned_host_guest_event(event, index, host_source) else {
            continue;
        };
        let controller = Arc::clone(controller);
        scheduler
            .schedule_parallel_at(host_partition, event.tick(), move |context| {
                let delivery = GuestEventDelivery::new(
                    context.now(),
                    host_partition,
                    host_partition,
                    guest_event,
                );
                controller
                    .lock()
                    .expect("system host controller lock")
                    .handle_delivery(delivery);
            })
            .map_err(RiscvWorkloadReplayError::Scheduler)?;
    }

    Ok(())
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
