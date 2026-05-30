use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rem6_cpu::{RiscvCluster, RiscvCore};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_transport::{MemoryTrace, MemoryTransport, TargetOutcome};

#[test]
fn tick_bounded_parallel_turn_runs_ready_events_when_epoch_horizon_exceeds_limit() {
    let cluster = RiscvCluster::new(Vec::<RiscvCore>::new()).unwrap();
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(1, 4, 1).unwrap();
    let executed_tick = Arc::new(AtomicU64::new(u64::MAX));
    let observed_tick = Arc::clone(&executed_tick);
    scheduler
        .schedule_parallel_at(PartitionId::new(0), 2, move |context| {
            observed_tick.store(context.now(), Ordering::SeqCst);
        })
        .unwrap();
    let transport = MemoryTransport::new();

    let turn = cluster
        .drive_turn_parallel_until_tick(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            2,
        )
        .unwrap()
        .expect("event at the tick limit should be runnable");

    assert_eq!(executed_tick.load(Ordering::SeqCst), 2);
    assert_eq!(scheduler.now(), 2);
    let epoch = turn.parallel_scheduler_epoch().unwrap();
    assert_eq!(epoch.summary().final_tick(), 2);
    let worker = epoch.batches().first().unwrap().workers().first().unwrap();
    assert_eq!(worker.start_tick(), 0);
    assert_eq!(worker.safe_until(), 2);
    assert_eq!(worker.duration_ticks(), 2);
}
