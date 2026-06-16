use rem6_kernel::{PartitionedScheduler, Tick};

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
    let recorded = scheduler
        .run_next_epoch_parallel_recorded()
        .map_err(RiscvClusterError::Scheduler)?;
    Ok(RiscvClusterTurn::parallel_scheduler(plan, recorded))
}

pub(crate) fn drive_parallel_scheduler_turn_until_tick(
    scheduler: &mut PartitionedScheduler,
    tick_limit: Tick,
) -> Result<Option<RiscvClusterTurn>, RiscvClusterError> {
    let Some((plan, recorded)) = scheduler
        .run_next_epoch_parallel_recorded_until(tick_limit)
        .map_err(RiscvClusterError::Scheduler)?
    else {
        return Ok(None);
    };
    Ok(Some(RiscvClusterTurn::parallel_scheduler(plan, recorded)))
}
