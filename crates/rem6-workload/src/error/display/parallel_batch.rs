use std::fmt;

use crate::error_support::format_partition_indexes;

use super::super::WorkloadError;

pub(super) fn format_parallel_batch_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::InvalidExpectedParallelBatchWorkerCount {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch activity must require at least 2 workers, got {minimum_worker_count}",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelBatchCount {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch activity with at least {minimum_worker_count} workers must require a positive batch count",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelBatchActivity {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch activity with at least {minimum_worker_count} workers is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelBatchActivitySummary {
            scope,
            minimum_worker_count,
            minimum_batch_count,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch activity with at least {minimum_batch_count} batches at {minimum_worker_count} workers",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelBatchActivityBelowMinimum {
            scope,
            minimum_worker_count,
            minimum_batch_count,
            actual_batch_count,
        } => write!(
            formatter,
            "expected {} batch activity to reach at least {minimum_batch_count} batches at {minimum_worker_count} workers, got {actual_batch_count}",
            scope.as_str()
        ),
        WorkloadError::ParallelBatchWorkerCountBelowMinimum {
            scope,
            worker_count,
            minimum_batch_count,
            actual_batch_count,
        } => write!(
            formatter,
            "{} batch worker-count bucket {worker_count} has {actual_batch_count} batches, below {minimum_batch_count}",
            scope.as_str()
        ),
        WorkloadError::UnexpectedParallelBatchWorkerCount {
            scope,
            worker_count,
            batch_count,
        } => write!(
            formatter,
            "unexpected {} batch worker-count bucket {worker_count} with {batch_count} batches",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelBatchWorkerBucket {
            scope,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count bucket must require at least 2 workers, got {worker_count}",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelBatchWorkerBucket {
            scope,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count bucket {worker_count} must require a positive batch count",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelBatchWorkerBucket {
            scope,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count bucket {worker_count} is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelBatchWorkerBucketSummary {
            scope,
            worker_count,
            minimum_batch_count,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch worker-count bucket {worker_count} with at least {minimum_batch_count} batches",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelBatchWorkerBucketBelowMinimum {
            scope,
            worker_count,
            minimum_batch_count,
            actual_batch_count,
        } => write!(
            formatter,
            "expected {} batch worker-count bucket {worker_count} to reach at least {minimum_batch_count} batches, got {actual_batch_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelBatchWorkerTickBucket {
            scope,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick bucket must require at least 2 workers, got {worker_count}",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelBatchWorkerTickBucket {
            scope,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick bucket {worker_count} must require positive ticks",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelBatchWorkerTickBucket {
            scope,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick bucket {worker_count} is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelBatchWorkerTickBucketSummary {
            scope,
            worker_count,
            minimum_ticks,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch worker-count tick bucket {worker_count} with at least {minimum_ticks} ticks",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelBatchWorkerTickBucketBelowMinimum {
            scope,
            worker_count,
            minimum_ticks,
            actual_ticks,
        } => write!(
            formatter,
            "expected {} batch worker-count tick bucket {worker_count} to reach at least {minimum_ticks} ticks, got {actual_ticks}",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelBatchWorkerTickActivity {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick activity must require at least 2 workers, got {minimum_worker_count}",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelBatchWorkerTickActivity {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick activity at {minimum_worker_count} workers must require positive ticks",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelBatchWorkerTickActivity {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick activity at {minimum_worker_count} workers is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelBatchWorkerTickActivitySummary {
            scope,
            minimum_worker_count,
            minimum_ticks,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch worker-count tick activity with at least {minimum_ticks} ticks at {minimum_worker_count} workers",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelBatchWorkerTickActivityBelowMinimum {
            scope,
            minimum_worker_count,
            minimum_ticks,
            actual_ticks,
        } => write!(
            formatter,
            "expected {} batch worker-count tick activity to reach at least {minimum_ticks} ticks at {minimum_worker_count} workers, got {actual_ticks}",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelBatchWorkerTickStreak {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick streak must require at least 2 workers, got {minimum_worker_count}",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelBatchWorkerTickStreak {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick streak at {minimum_worker_count} workers must require positive consecutive ticks",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelBatchWorkerTickStreak {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-count tick streak at {minimum_worker_count} workers is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelBatchWorkerTickStreakSummary {
            scope,
            minimum_worker_count,
            minimum_consecutive_ticks,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch worker-count tick streak with at least {minimum_consecutive_ticks} consecutive ticks at {minimum_worker_count} workers",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelBatchWorkerTickStreakBelowMinimum {
            scope,
            minimum_worker_count,
            minimum_consecutive_ticks,
            actual_consecutive_ticks,
        } => write!(
            formatter,
            "expected {} batch worker-count tick streak to reach at least {minimum_consecutive_ticks} consecutive ticks at {minimum_worker_count} workers, got {actual_consecutive_ticks}",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelBatchWorkerTicks {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-ticks must require at least 2 workers for thresholded contracts, got {minimum_worker_count}",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelBatchWorkerTicks {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-ticks with minimum worker count {minimum_worker_count} must require positive worker-ticks",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelBatchWorkerTicks {
            scope,
            minimum_worker_count,
        } => write!(
            formatter,
            "expected {} batch worker-ticks with minimum worker count {minimum_worker_count} is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelBatchWorkerTicksSummary {
            scope,
            minimum_worker_count,
            minimum_worker_ticks,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch worker-ticks with at least {minimum_worker_ticks} worker-ticks and minimum worker count {minimum_worker_count}",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelBatchWorkerTicksBelowMinimum {
            scope,
            minimum_worker_count,
            minimum_worker_ticks,
            actual_worker_ticks,
        } => write!(
            formatter,
            "expected {} batch worker-ticks to reach at least {minimum_worker_ticks} with minimum worker count {minimum_worker_count}, got {actual_worker_ticks}",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelBatchPartitionSet { scope, partitions } => write!(
            formatter,
            "expected {} batch partition set {} must include at least 2 partitions",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::ZeroExpectedParallelBatchPartitionSetCount { scope, partitions } => write!(
            formatter,
            "expected {} batch partition set {} must require a positive batch count",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::DuplicateExpectedParallelBatchPartitionSet { scope, partitions } => write!(
            formatter,
            "expected {} batch partition set {} is already declared",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::MissingParallelBatchPartitionSetSummary {
            scope,
            partitions,
            minimum_batch_count,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch partition set {} with at least {minimum_batch_count} batches",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
            scope,
            partitions,
            minimum_batch_count,
            actual_batch_count,
        } => write!(
            formatter,
            "expected {} batch partition set {} to reach at least {minimum_batch_count} batches, got {actual_batch_count}",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::InvalidExpectedParallelBatchPartitionStreak { scope, partitions } => write!(
            formatter,
            "expected {} batch partition streak {} must include at least 2 partitions",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::ZeroExpectedParallelBatchPartitionStreakCount { scope, partitions } => write!(
            formatter,
            "expected {} batch partition streak {} must require a positive consecutive batch count",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::DuplicateExpectedParallelBatchPartitionStreak { scope, partitions } => write!(
            formatter,
            "expected {} batch partition streak {} is already declared",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::MissingParallelBatchPartitionStreakSummary {
            scope,
            partitions,
            minimum_consecutive_batch_count,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch partition streak {} with at least {minimum_consecutive_batch_count} consecutive batches",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::ExpectedParallelBatchPartitionStreakBelowMinimum {
            scope,
            partitions,
            minimum_consecutive_batch_count,
            actual_consecutive_batch_count,
        } => write!(
            formatter,
            "expected {} batch partition streak {} to reach at least {minimum_consecutive_batch_count} consecutive batches, got {actual_consecutive_batch_count}",
            scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch timeline record from {} at {start_tick} to horizon {horizon} for partitions {} must have positive duration, at least 2 workers, and at least 2 partitions, got {worker_count} workers",
            scope.as_str(),
            batch_scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::DuplicateExpectedParallelBatchTimelineRecord {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch timeline record from {} at {start_tick} to horizon {horizon} for partitions {} with {worker_count} workers is already declared",
            scope.as_str(),
            batch_scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::MissingParallelBatchTimelineSummary {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        } => write!(
            formatter,
            "missing parallel summary for expected {} batch timeline record from {} at {start_tick} to horizon {horizon} for partitions {} with {worker_count} workers",
            scope.as_str(),
            batch_scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::ExpectedParallelBatchTimelineRecordMissing {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        } => write!(
            formatter,
            "expected {} batch timeline record from {} at {start_tick} to horizon {horizon} for partitions {} with {worker_count} workers is missing",
            scope.as_str(),
            batch_scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::UnexpectedParallelBatchTimelineRecord {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        } => write!(
            formatter,
            "unexpected {} batch timeline record from {} at {start_tick} to horizon {horizon} for partitions {} with {worker_count} workers",
            scope.as_str(),
            batch_scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::InvalidParallelBatchTimelineMergeSummary {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        } => write!(
            formatter,
            "invalid {} batch timeline merge summary: scoped record from {} at {start_tick} to horizon {horizon} for partitions {} with {worker_count} workers is missing from the merged timeline",
            scope.as_str(),
            batch_scope.as_str(),
            format_partition_indexes(partitions)
        ),
        WorkloadError::ZeroExpectedParallelPartitionCount { scope } => write!(
            formatter,
            "expected {} partition use must require a positive active partition count",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelPartitionCount {
            scope,
            minimum_active_partitions,
        } => write!(
            formatter,
            "expected {} partition use must require at least 2 active partitions, got {minimum_active_partitions}",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelPartitionUse { scope } => write!(
            formatter,
            "expected {} partition use is already declared",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelPartitionActivity { scope, partition } => write!(
            formatter,
            "expected {} partition {partition} activity must require at least one positive activity count",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelPartitionActivity { scope, partition } => write!(
            formatter,
            "expected {} partition {partition} activity is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelPartitionSummary {
            scope,
            minimum_active_partitions,
        } => write!(
            formatter,
            "missing parallel summary for expected {} partition use with at least {minimum_active_partitions} active partitions",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
            scope,
            minimum_active_partitions,
            actual_active_partitions,
        } => write!(
            formatter,
            "expected {} partition use to reach at least {minimum_active_partitions} active partitions, got {actual_active_partitions}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelPartitionCountMergeSummary(summary) => write!(
            formatter,
            "invalid {} partition-count merge summary: merged active partitions {} is below lower-bound active partitions {}",
            summary.scope.as_str(),
            summary.merged_active_partitions,
            summary.lower_bound_active_partitions
        ),
        WorkloadError::MissingParallelPartitionActivitySummary { scope, partition } => write!(
            formatter,
            "missing parallel summary for expected {} partition {partition} activity",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelPartitionActivityBelowMinimum {
            scope,
            partition,
            minimum_worker_count,
            actual_worker_count,
            minimum_dispatch_count,
            actual_dispatch_count,
            minimum_remote_send_count,
            actual_remote_send_count,
            minimum_remote_receive_count,
            actual_remote_receive_count,
        } => write!(
            formatter,
            "expected {} partition {partition} activity to reach workers {minimum_worker_count}, dispatches {minimum_dispatch_count}, remote sends {minimum_remote_send_count}, and remote receives {minimum_remote_receive_count}; got workers {actual_worker_count}, dispatches {actual_dispatch_count}, remote sends {actual_remote_send_count}, and remote receives {actual_remote_receive_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelPartitionActivityMergeSummary(summary) => write!(
            formatter,
            "invalid {} partition {} activity merge summary: merged workers {}, dispatches {}, remote sends {}, remote receives {}, and pending events {}; lower-bound workers {}, dispatches {}, remote sends {}, remote receives {}, and pending events {}",
            summary.scope.as_str(),
            summary.partition,
            summary.merged_worker_count,
            summary.merged_dispatch_count,
            summary.merged_remote_send_count,
            summary.merged_remote_receive_count,
            summary.merged_max_pending_events,
            summary.lower_bound_worker_count,
            summary.lower_bound_dispatch_count,
            summary.lower_bound_remote_send_count,
            summary.lower_bound_remote_receive_count,
            summary.lower_bound_max_pending_events
        ),
        _ => unreachable!("parallel batch display called for unrelated workload error"),
    }
}
