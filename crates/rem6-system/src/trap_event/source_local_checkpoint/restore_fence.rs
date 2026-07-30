use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[derive(Debug, Default)]
pub(super) struct SourceLocalRestoreFenceOwnership {
    activated: AtomicBool,
    released: AtomicBool,
}

impl SourceLocalRestoreFenceOwnership {
    pub(super) fn activate(&self) {
        self.activated.store(true, Ordering::SeqCst);
    }

    pub(super) fn is_activated(&self) -> bool {
        self.activated.load(Ordering::SeqCst)
    }

    pub(super) fn release_capture_once(
        &self,
        controller: &Arc<Mutex<SystemHostController>>,
        deadline: Tick,
    ) {
        if self.released.swap(true, Ordering::SeqCst) {
            return;
        }
        controller
            .lock()
            .expect("system host controller lock")
            .executor()
            .release_source_local_checkpoint_capture(deadline);
    }

    pub(super) fn release_restore_once(
        &self,
        controller: &Arc<Mutex<SystemHostController>>,
        source_tick: Tick,
        deadline: Tick,
    ) {
        if self.released.swap(true, Ordering::SeqCst) {
            return;
        }
        let controller = controller.lock().expect("system host controller lock");
        controller
            .executor()
            .release_source_local_checkpoint_capture(deadline);
        controller
            .executor()
            .release_source_local_checkpoint_restore_after(source_tick, deadline);
    }
}

impl SystemHostEventPort {
    pub(super) fn emit_with_scheduler_checkpoint_on_source(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
        component: CheckpointComponentId,
        capture_prepared: bool,
        restore_fence_ownership: Option<Arc<SourceLocalRestoreFenceOwnership>>,
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
        let restore_deadline = capture_deadline
            .filter(|_| matches!(event.kind(), GuestEventKind::RestoreCheckpoint { .. }));
        let source_tick = context.now();
        if let Some(deadline) = capture_deadline.filter(|_| !capture_prepared) {
            self.controller
                .lock()
                .expect("system host controller lock")
                .executor()
                .prepare_source_local_checkpoint_capture(deadline);
        }
        if let Some(deadline) = restore_deadline {
            self.controller
                .lock()
                .expect("system host controller lock")
                .executor()
                .prepare_source_local_checkpoint_restore_after(source_tick, deadline);
        }
        let delivery_controller = Arc::clone(&self.controller);
        let release_controller = Arc::clone(&self.controller);
        let registration_controller = Arc::clone(&self.controller);
        let delivery_fence_ownership = restore_fence_ownership.clone();
        let scheduler_event = self.channel.emit_with_scheduler_checkpoint_on_source(
            context,
            event,
            move |delivery, context| {
                super::super::scheduler_checkpoint_delivery::handle_host_delivery_with_scheduler_checkpoint(
                    context,
                    delivery,
                    0,
                    component,
                    delivery_controller,
                    delivery_fence_ownership.is_none(),
                    delivery_fence_ownership
                        .is_none()
                        .then_some(source_tick)
                        .filter(|_| restore_deadline.is_some()),
                );
                if let (Some(deadline), Some(ownership)) =
                    (restore_deadline, delivery_fence_ownership)
                {
                    ownership.release_restore_once(
                        &release_controller,
                        source_tick,
                        deadline,
                    );
                }
            },
        );
        let scheduler_event = match scheduler_event {
            Ok(event) => event,
            Err(error) => {
                if let (Some(deadline), Some(ownership)) =
                    (restore_deadline, restore_fence_ownership.as_ref())
                {
                    ownership.release_restore_once(&self.controller, source_tick, deadline);
                    return Err(error);
                }
                if let Some(deadline) = capture_deadline {
                    self.controller
                        .lock()
                        .expect("system host controller lock")
                        .executor()
                        .release_source_local_checkpoint_capture(deadline);
                }
                if let Some(deadline) = restore_deadline {
                    self.controller
                        .lock()
                        .expect("system host controller lock")
                        .executor()
                        .release_source_local_checkpoint_restore_after(source_tick, deadline);
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
