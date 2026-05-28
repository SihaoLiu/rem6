use rem6_kernel::PartitionId;

use crate::{
    WorkloadError, WorkloadParallelBatchPartitionScope, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelExecutionSummary,
};

pub(crate) fn validate_partition_scope_batch_partition_set_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
    partitions: &[PartitionId],
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchPartitionScope::FullSystem {
        return Ok(());
    }
    validate_raw_full_system_batch_partition_sets(summary)?;
    validate_raw_full_system_batch_partition_streaks(summary)?;
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
    validate_raw_full_system_batch_partition_streaks(summary)?;
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

fn validate_raw_full_system_batch_partition_sets(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    let mut seen_partition_sets = Vec::new();
    for set in summary.raw_full_system_parallel_scheduler_batch_partition_sets() {
        validate_raw_full_system_batch_partition_set(
            set,
            set.batch_count(),
            &mut seen_partition_sets,
        )?;
    }
    Ok(())
}

fn validate_raw_full_system_batch_partition_streaks(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    let mut seen_partition_sets = Vec::new();
    for streak in summary.raw_full_system_parallel_scheduler_batch_partition_streaks() {
        validate_raw_full_system_batch_partition_set(
            streak,
            streak.consecutive_batch_count(),
            &mut seen_partition_sets,
        )?;
    }
    Ok(())
}

fn validate_raw_full_system_batch_partition_set(
    set: impl RawBatchPartitionSummary,
    count: usize,
    seen_partition_sets: &mut Vec<Vec<PartitionId>>,
) -> Result<(), WorkloadError> {
    let partitions = set.partitions();
    if partitions.len() < 2
        || count == 0
        || seen_partition_sets.iter().any(|seen| seen == partitions)
    {
        return Err(WorkloadError::UnexpectedParallelBatchPartitionSummary {
            scope: WorkloadParallelBatchPartitionScope::FullSystem,
            partitions: partition_indexes(partitions),
            count,
        });
    }
    seen_partition_sets.push(partitions.to_vec());
    Ok(())
}

trait RawBatchPartitionSummary {
    fn partitions(&self) -> &[PartitionId];
}

impl RawBatchPartitionSummary for &WorkloadParallelBatchPartitionSet {
    fn partitions(&self) -> &[PartitionId] {
        WorkloadParallelBatchPartitionSet::partitions(self)
    }
}

impl RawBatchPartitionSummary for &WorkloadParallelBatchPartitionStreak {
    fn partitions(&self) -> &[PartitionId] {
        WorkloadParallelBatchPartitionStreak::partitions(self)
    }
}

fn partition_indexes(partitions: &[PartitionId]) -> Vec<u32> {
    partitions
        .iter()
        .map(|partition| partition.index())
        .collect()
}
