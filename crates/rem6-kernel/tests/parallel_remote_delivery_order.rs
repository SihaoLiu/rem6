use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn recorded_parallel_remote_sends_follow_delivery_merge_order() {
    let source0 = PartitionId::new(0);
    let source1 = PartitionId::new(1);
    let target0 = PartitionId::new(0);
    let target2 = PartitionId::new(2);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(source0, 0, move |context| {
            context.schedule_remote_after(target2, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(source1, 0, move |context| {
            context.schedule_remote_after(target0, 4, |_| {}).unwrap();
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let sends = epoch.batches()[0].remote_sends();

    assert_eq!(sends.len(), 2);
    assert_eq!(sends[0].source(), source1);
    assert_eq!(sends[0].target(), target0);
    assert_eq!(sends[0].delivery_tick(), 4);
    assert_eq!(sends[1].source(), source0);
    assert_eq!(sends[1].target(), target2);
    assert_eq!(sends[1].delivery_tick(), 4);
    assert_eq!(epoch.remote_sends(), sends.to_vec());
}

#[test]
fn recorded_epoch_remote_sends_follow_delivery_order_across_batches() {
    let source0 = PartitionId::new(0);
    let source1 = PartitionId::new(1);
    let target0 = PartitionId::new(0);
    let target3 = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 1).unwrap();

    scheduler
        .schedule_parallel_at(source0, 0, move |context| {
            context.schedule_remote_after(target3, 5, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(source1, 0, move |context| {
            context.schedule_remote_after(target0, 4, |_| {}).unwrap();
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let sends = epoch.remote_sends();

    assert_eq!(sends.len(), 2);
    assert_eq!(sends[0].source(), source1);
    assert_eq!(sends[0].target(), target0);
    assert_eq!(sends[0].delivery_tick(), 4);
    assert_eq!(sends[1].source(), source0);
    assert_eq!(sends[1].target(), target3);
    assert_eq!(sends[1].delivery_tick(), 5);
}

#[test]
fn recorded_run_remote_sends_follow_delivery_order_across_epochs() {
    let source0 = PartitionId::new(0);
    let source1 = PartitionId::new(1);
    let target0 = PartitionId::new(0);
    let target3 = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 4).unwrap();

    scheduler
        .schedule_parallel_at(source0, 0, move |context| {
            context.schedule_remote_after(target3, 20, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(source1, 5, move |context| {
            context.schedule_remote_after(target0, 4, |_| {}).unwrap();
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let sends = run.remote_sends();

    assert_eq!(sends.len(), 2);
    assert_eq!(sends[0].source(), source1);
    assert_eq!(sends[0].target(), target0);
    assert_eq!(sends[0].delivery_tick(), 9);
    assert_eq!(sends[1].source(), source0);
    assert_eq!(sends[1].target(), target3);
    assert_eq!(sends[1].delivery_tick(), 20);
}

#[test]
fn recorded_remote_send_order_is_source_local_across_epochs() {
    let source = PartitionId::new(0);
    let target = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 10, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(source, 5, move |context| {
            context.schedule_remote_after(target, 5, |_| {}).unwrap();
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let sends = run.remote_sends();

    assert_eq!(sends.len(), 2);
    assert_eq!(sends[0].source(), source);
    assert_eq!(sends[0].target(), target);
    assert_eq!(sends[0].delivery_tick(), 10);
    assert_eq!(sends[0].order(), 0);
    assert_eq!(sends[1].source(), source);
    assert_eq!(sends[1].target(), target);
    assert_eq!(sends[1].delivery_tick(), 10);
    assert_eq!(sends[1].order(), 1);
}

#[test]
fn quiescent_snapshot_preserves_remote_send_order_identity() {
    let source = PartitionId::new(0);
    let target = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel_recorded().unwrap();

    let snapshot = scheduler.quiescent_snapshot().unwrap();
    let mut restored = PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap();
    restored.restore_quiescent(&snapshot).unwrap();

    restored
        .schedule_parallel_at(source, restored.now(), move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();

    let run = restored.run_until_idle_parallel_recorded().unwrap();
    let sends = run.remote_sends();

    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].source(), source);
    assert_eq!(sends[0].target(), target);
    assert_eq!(sends[0].order(), 1);
}
