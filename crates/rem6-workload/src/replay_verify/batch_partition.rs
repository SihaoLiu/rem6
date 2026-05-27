use rem6_kernel::PartitionId;

use crate::{WorkloadError, WorkloadParallelBatchPartitionScope, WorkloadParallelExecutionSummary};

pub(crate) fn validate_partition_scope_batch_partition_set_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
    partitions: &[PartitionId],
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchPartitionScope::FullSystem {
        return Ok(());
    }
    let actual_batch_count = summary
        .explicit_full_system_parallel_scheduler_batch_count_for_partition_set(
            partitions.iter().copied(),
        );
    if actual_batch_count == 0
        && summary
            .explicit_full_system_parallel_scheduler_batch_partition_sets()
            .is_empty()
        && summary
            .explicit_full_system_parallel_scheduler_batch_partition_streaks()
            .is_empty()
    {
        return Ok(());
    }
    let minimum_batch_count = summary
        .scoped_full_system_parallel_scheduler_batch_count_for_partition_set(
            partitions.iter().copied(),
        );
    if actual_batch_count < minimum_batch_count {
        return Err(
            WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
                scope: WorkloadParallelBatchPartitionScope::FullSystem,
                partitions: partition_indexes(partitions),
                minimum_batch_count,
                actual_batch_count,
            },
        );
    }
    Ok(())
}

pub(crate) fn validate_partition_scope_batch_partition_streak_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
    partitions: &[PartitionId],
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchPartitionScope::FullSystem {
        return Ok(());
    }
    let actual_consecutive_batch_count = summary
        .explicit_full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
            partitions.iter().copied(),
        );
    if actual_consecutive_batch_count == 0
        && summary
            .explicit_full_system_parallel_scheduler_batch_partition_streaks()
            .is_empty()
    {
        return Ok(());
    }
    let minimum_consecutive_batch_count = summary
        .scoped_full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
            partitions.iter().copied(),
        );
    if actual_consecutive_batch_count < minimum_consecutive_batch_count {
        return Err(
            WorkloadError::ExpectedParallelBatchPartitionStreakBelowMinimum {
                scope: WorkloadParallelBatchPartitionScope::FullSystem,
                partitions: partition_indexes(partitions),
                minimum_consecutive_batch_count,
                actual_consecutive_batch_count,
            },
        );
    }
    Ok(())
}

fn partition_indexes(partitions: &[PartitionId]) -> Vec<u32> {
    partitions
        .iter()
        .map(|partition| partition.index())
        .collect()
}
