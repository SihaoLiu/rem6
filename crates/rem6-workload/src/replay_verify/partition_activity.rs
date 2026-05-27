use rem6_kernel::ParallelPartitionActivity;

use crate::{
    WorkloadError, WorkloadParallelBatchPartitionScope, WorkloadParallelExecutionSummary,
    WorkloadParallelPartitionActivityMergeSummary,
};

pub(crate) fn validate_partition_scope_activity_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
) -> Result<(), WorkloadError> {
    if scope == WorkloadParallelBatchPartitionScope::FullSystem {
        validate_full_system_partition_activity_merge_summary(summary)?;
    }
    Ok(())
}

fn validate_full_system_partition_activity_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    let explicit_activities = summary.raw_full_system_parallel_scheduler_partition_activities();
    if explicit_activities.is_empty() {
        return Ok(());
    }

    for (partition, merged_activity) in explicit_activities.iter().copied() {
        let Some(lower_bound) =
            summary.full_system_parallel_scheduler_partition_activity_lower_bound(partition)
        else {
            continue;
        };
        if partition_activity_covers_lower_bound(merged_activity, lower_bound) {
            continue;
        }
        return Err(WorkloadError::InvalidParallelPartitionActivityMergeSummary(
            Box::new(WorkloadParallelPartitionActivityMergeSummary {
                scope: WorkloadParallelBatchPartitionScope::FullSystem,
                partition: partition.index(),
                merged_worker_count: merged_activity.worker_count(),
                lower_bound_worker_count: lower_bound.worker_count(),
                merged_dispatch_count: merged_activity.dispatch_count(),
                lower_bound_dispatch_count: lower_bound.dispatch_count(),
                merged_remote_send_count: merged_activity.remote_send_count(),
                lower_bound_remote_send_count: lower_bound.remote_send_count(),
                merged_remote_receive_count: merged_activity.remote_receive_count(),
                lower_bound_remote_receive_count: lower_bound.remote_receive_count(),
                merged_max_pending_events: merged_activity.max_pending_events(),
                lower_bound_max_pending_events: lower_bound.max_pending_events(),
            }),
        ));
    }
    Ok(())
}

fn partition_activity_covers_lower_bound(
    merged_activity: ParallelPartitionActivity,
    lower_bound: ParallelPartitionActivity,
) -> bool {
    merged_activity.worker_count() >= lower_bound.worker_count()
        && merged_activity.dispatch_count() >= lower_bound.dispatch_count()
        && merged_activity.remote_send_count() >= lower_bound.remote_send_count()
        && merged_activity.remote_receive_count() >= lower_bound.remote_receive_count()
        && merged_activity.max_pending_events() >= lower_bound.max_pending_events()
}
