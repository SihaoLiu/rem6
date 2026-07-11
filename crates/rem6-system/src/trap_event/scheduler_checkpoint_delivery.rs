use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_kernel::{SchedulerContext, Tick};

use crate::{GuestEventDelivery, SystemHostController};

pub(super) fn handle_host_delivery_with_scheduler_checkpoint(
    context: &mut SchedulerContext<'_>,
    delivery: GuestEventDelivery,
    period: Tick,
    component: CheckpointComponentId,
    controller: Arc<Mutex<SystemHostController>>,
) {
    let source_partition = delivery.source_partition();
    let host_partition = delivery.host_partition();
    let event = delivery.event().clone();
    controller
        .lock()
        .expect("system host controller lock")
        .handle_delivery_with_scheduler_checkpoint(
            delivery,
            component.clone(),
            context.checkpoint_access(),
        );

    if period == 0 || context.now().checked_add(period).is_none() {
        return;
    }

    let next_controller = Arc::clone(&controller);
    let next_component = component.clone();
    let Ok(next_event) = context.schedule_local_after(period, move |context| {
        handle_host_delivery_with_scheduler_checkpoint(
            context,
            GuestEventDelivery::new(context.now(), source_partition, host_partition, event),
            period,
            next_component,
            next_controller,
        );
    }) else {
        return;
    };
    let scheduler = context.checkpoint_access();
    let event = scheduler
        .pending_event_snapshot(next_event)
        .expect("new periodic scheduler checkpoint control delivery is pending");
    controller
        .lock()
        .expect("system host controller lock")
        .executor_mut()
        .register_scheduler_checkpoint_control_event(scheduler.instance_id(), event);
}
