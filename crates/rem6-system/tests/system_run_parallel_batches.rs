use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_cpu::RiscvClusterTurn;
use rem6_kernel::{PartitionId, PartitionedScheduler, WaitForGraph};
use rem6_system::{RiscvSystemRun, RiscvSystemRunStopReason};

fn empty_wait_for_graphs() -> ParallelCoherenceWaitForGraphs {
    ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new())
}

fn cpu_scheduler_turn(
    partitions: u32,
    worker_limit: usize,
    scheduled_partitions: &[PartitionId],
) -> RiscvClusterTurn {
    let mut scheduler =
        PartitionedScheduler::with_parallel_worker_limit(partitions, 4, worker_limit).unwrap();
    for partition in scheduled_partitions {
        scheduler
            .schedule_parallel_at(*partition, 0, |_| {})
            .unwrap();
    }
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    RiscvClusterTurn::parallel_scheduler(plan, recorded)
}

fn data_cache_run(
    partitions: u32,
    worker_limit: usize,
    scheduled_partitions: &[PartitionId],
) -> ParallelCoherenceRunSummary {
    let mut scheduler =
        PartitionedScheduler::with_parallel_worker_limit(partitions, 4, worker_limit).unwrap();
    for partition in scheduled_partitions {
        scheduler
            .schedule_parallel_at(*partition, 0, |_| {})
            .unwrap();
    }
    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        empty_wait_for_graphs(),
    )
}

#[test]
fn system_run_summarizes_parallel_batch_worker_and_partition_sets() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn(3, 2, &[cpu0, cpu1, cache])],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run(3, 2, &[cpu1, cache])]);

    assert_eq!(
        run.parallel_scheduler_batch_worker_count_summaries(),
        vec![(1, 1), (2, 1)],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_batch_worker_count_summaries(),
        vec![(2, 1)],
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batch_worker_count_summaries(),
        vec![(1, 1), (2, 2)],
    );
    assert_eq!(run.parallel_scheduler_batch_count_at_or_above(2), 1);
    assert_eq!(
        run.data_cache_parallel_scheduler_batch_count_at_or_above(2),
        1,
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batch_count_at_or_above(2),
        2
    );

    assert_eq!(
        run.parallel_scheduler_batch_partition_set_summaries(),
        vec![(vec![cpu0, cpu1], 1), (vec![cache], 1)],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_batch_partition_set_summaries(),
        vec![(vec![cpu1, cache], 1)],
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batch_partition_set_summaries(),
        vec![
            (vec![cpu0, cpu1], 1),
            (vec![cpu1, cache], 1),
            (vec![cache], 1),
        ],
    );
    assert_eq!(
        run.parallel_scheduler_batch_count_for_partition_set([cpu0, cpu1]),
        1,
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_batch_count_for_partition_set([cpu1, cache]),
        1,
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batch_count_for_partition_set([cache]),
        1,
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batch_count_for_partition_set([cpu0, cache]),
        0,
    );

    assert_eq!(
        run.parallel_scheduler_batch_partition_streak_summaries(),
        vec![(vec![cpu0, cpu1], 1), (vec![cache], 1)],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_batch_partition_streak_summaries(),
        vec![(vec![cpu1, cache], 1)],
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batch_partition_streak_summaries(),
        vec![
            (vec![cpu0, cpu1], 1),
            (vec![cpu1, cache], 1),
            (vec![cache], 1),
        ],
    );
    assert_eq!(
        run.full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([
            cpu1, cache,
        ]),
        1,
    );
}

#[test]
fn full_system_batch_streaks_cross_cpu_and_data_cache_batch_boundaries() {
    let cpu1 = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn(3, 2, &[cpu1, cache])],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run(3, 2, &[cpu1, cache])]);

    assert_eq!(
        run.full_system_parallel_scheduler_batch_partition_streak_summaries(),
        vec![(vec![cpu1, cache], 2)],
    );
    assert_eq!(
        run.full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([
            cpu1, cache,
        ]),
        2,
    );
}
