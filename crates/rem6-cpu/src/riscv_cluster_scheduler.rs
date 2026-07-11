use rem6_kernel::{MixedEventRun, PartitionedScheduler, Tick};

use crate::riscv_cluster_run::RiscvClusterTurn;
use crate::RiscvClusterError;

pub(crate) fn drive_parallel_scheduler_turn(
    scheduler: &mut PartitionedScheduler,
) -> Result<RiscvClusterTurn, RiscvClusterError> {
    let Some(plan) = scheduler
        .plan_next_parallel_epoch()
        .map_err(RiscvClusterError::Scheduler)?
    else {
        return Ok(RiscvClusterTurn::idle(scheduler.now()));
    };
    if !plan.serial_blockers().is_empty() {
        return scheduler
            .run_next_mixed_event(plan)
            .map(cluster_turn_from_mixed_event)
            .map_err(RiscvClusterError::Scheduler);
    }
    let recorded = scheduler
        .run_next_epoch_parallel_recorded()
        .map_err(RiscvClusterError::Scheduler)?;
    Ok(RiscvClusterTurn::parallel_scheduler(plan, recorded))
}

pub(crate) fn drive_parallel_scheduler_turn_until_tick(
    scheduler: &mut PartitionedScheduler,
    tick_limit: Tick,
) -> Result<Option<RiscvClusterTurn>, RiscvClusterError> {
    let Some(plan) = scheduler
        .plan_next_parallel_epoch_until(tick_limit)
        .map_err(RiscvClusterError::Scheduler)?
    else {
        return Ok(None);
    };
    if !plan.serial_blockers().is_empty() {
        return scheduler
            .run_next_mixed_event(plan)
            .map(|run| Some(cluster_turn_from_mixed_event(run)))
            .map_err(RiscvClusterError::Scheduler);
    }
    let Some((plan, recorded)) = scheduler
        .run_next_epoch_parallel_recorded_until(tick_limit)
        .map_err(RiscvClusterError::Scheduler)?
    else {
        return Ok(None);
    };
    Ok(Some(RiscvClusterTurn::parallel_scheduler(plan, recorded)))
}

fn cluster_turn_from_mixed_event(run: MixedEventRun) -> RiscvClusterTurn {
    match run {
        MixedEventRun::Serial(summary) => RiscvClusterTurn::scheduler(summary),
        MixedEventRun::Parallel { plan, recorded } => {
            RiscvClusterTurn::parallel_scheduler(plan, recorded)
        }
    }
}

#[cfg(test)]
mod tests {
    use rem6_kernel::{LivelockTransitionKind, PartitionId, ScheduledEventKind, WaitForNode};

    use super::*;

    #[test]
    fn mixed_parallel_turn_preserves_progress_transitions_before_serial_barrier() {
        let subject = WaitForNode::component("mixed-cluster-scheduler").unwrap();
        let callback_subject = subject.clone();
        let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 4).unwrap();
        scheduler
            .schedule_parallel_at(PartitionId::new(0), 0, move |context| {
                context.record_progress_transition(
                    callback_subject,
                    LivelockTransitionKind::ProtocolRetry,
                );
            })
            .unwrap();
        scheduler
            .schedule_at(PartitionId::new(1), 0, |_| {})
            .unwrap();

        let turn = drive_parallel_scheduler_turn(&mut scheduler).unwrap();

        let epoch = turn
            .parallel_scheduler_epoch()
            .expect("isolated parallel callback should remain a recorded epoch");
        assert_eq!(epoch.progress_transition_count(), 1);
        assert_eq!(epoch.progress_transitions()[0].subject(), &subject);
        assert_eq!(epoch.dispatch_count(), 1);
        assert_eq!(epoch.serial_blocker_count(), 1);
        assert!(!epoch.is_parallel_safe());
        let blocker = epoch
            .first_serial_blocker()
            .expect("mixed epoch should retain its serial barrier");
        assert_eq!(blocker.partition(), PartitionId::new(1));
        assert_eq!(blocker.tick(), 0);
        assert_eq!(blocker.kind(), ScheduledEventKind::Serial);
    }
}
