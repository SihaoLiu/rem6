use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn recorded_parallel_runs_report_exact_batch_worker_count_buckets() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);
    let memory = PartitionId::new(3);

    scheduler.schedule_parallel_at(core0, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core1, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core2, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(memory, 5, |_| {}).unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let first_epoch = &run.epochs()[0];
    let second_epoch = &run.epochs()[1];

    assert_eq!(
        first_epoch.batch_worker_count_summaries(),
        vec![(1, 1), (2, 1)]
    );
    assert_eq!(first_epoch.batch_count_for_worker_count(1), 1);
    assert_eq!(first_epoch.batch_count_for_worker_count(2), 1);
    assert_eq!(first_epoch.batch_count_at_or_above(1), 2);
    assert_eq!(first_epoch.batch_count_at_or_above(2), 1);
    assert_eq!(first_epoch.batch_count_at_or_above(3), 0);
    assert_eq!(
        first_epoch.batch_worker_count_tick_summaries(),
        vec![(1, 4), (2, 4)]
    );
    assert_eq!(first_epoch.batch_ticks_for_worker_count(1), 4);
    assert_eq!(first_epoch.batch_ticks_for_worker_count(2), 4);
    assert_eq!(first_epoch.batch_ticks_at_or_above(1), 8);
    assert_eq!(first_epoch.batch_ticks_at_or_above(2), 4);
    assert_eq!(first_epoch.batch_worker_ticks(), 12);
    assert_eq!(first_epoch.batch_worker_ticks_at_or_above(1), 12);
    assert_eq!(first_epoch.batch_worker_ticks_at_or_above(2), 8);
    assert_eq!(first_epoch.batch_worker_ticks_at_or_above(3), 0);

    assert_eq!(second_epoch.batch_worker_count_summaries(), vec![(1, 1)]);
    assert_eq!(
        second_epoch.batch_worker_count_tick_summaries(),
        vec![(1, 4)]
    );
    assert_eq!(second_epoch.batch_worker_ticks(), 4);
    assert_eq!(second_epoch.batch_worker_ticks_at_or_above(2), 0);
    assert_eq!(run.batch_worker_count_summaries(), vec![(1, 2), (2, 1)]);
    assert_eq!(run.batch_count_for_worker_count(1), 2);
    assert_eq!(run.batch_count_for_worker_count(2), 1);
    assert_eq!(run.batch_count_at_or_above(1), 3);
    assert_eq!(run.batch_count_at_or_above(2), 1);
    assert_eq!(run.batch_count_at_or_above(3), 0);
    assert_eq!(
        run.batch_worker_count_tick_summaries(),
        vec![(1, 8), (2, 4)]
    );
    assert_eq!(run.batch_ticks_for_worker_count(1), 8);
    assert_eq!(run.batch_ticks_for_worker_count(2), 4);
    assert_eq!(run.batch_ticks_at_or_above(1), 12);
    assert_eq!(run.batch_ticks_at_or_above(2), 4);
    assert_eq!(run.batch_worker_ticks(), 16);
    assert_eq!(run.batch_worker_ticks_at_or_above(1), 16);
    assert_eq!(run.batch_worker_ticks_at_or_above(2), 8);
    assert_eq!(run.batch_worker_ticks_at_or_above(3), 0);
}
