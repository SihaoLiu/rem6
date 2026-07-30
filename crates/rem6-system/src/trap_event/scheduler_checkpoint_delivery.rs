use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_kernel::{SchedulerContext, Tick};

use crate::{GuestEventDelivery, GuestEventKind, SystemHostController};

pub(super) fn handle_host_delivery_with_scheduler_checkpoint(
    context: &mut SchedulerContext<'_>,
    delivery: GuestEventDelivery,
    period: Tick,
    component: CheckpointComponentId,
    controller: Arc<Mutex<SystemHostController>>,
    release_source_local_preparation: bool,
    source_local_restore_activation_tick: Option<Tick>,
) {
    let source_partition = delivery.source_partition();
    let host_partition = delivery.host_partition();
    let delivery_tick = delivery.tick();
    let event = delivery.event().clone();
    controller
        .lock()
        .expect("system host controller lock")
        .handle_delivery_with_scheduler_checkpoint(
            delivery,
            component.clone(),
            context.checkpoint_access(),
        );
    if release_source_local_preparation
        && matches!(
            event.kind(),
            GuestEventKind::Checkpoint { .. } | GuestEventKind::RestoreCheckpoint { .. }
        )
    {
        controller
            .lock()
            .expect("system host controller lock")
            .executor()
            .release_source_local_checkpoint_capture(delivery_tick);
    }
    if let Some(source_tick) = source_local_restore_activation_tick
        .filter(|_| matches!(event.kind(), GuestEventKind::RestoreCheckpoint { .. }))
    {
        controller
            .lock()
            .expect("system host controller lock")
            .executor()
            .release_source_local_checkpoint_restore_after(source_tick, delivery_tick);
    }

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
            release_source_local_preparation,
            source_local_restore_activation_tick,
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
