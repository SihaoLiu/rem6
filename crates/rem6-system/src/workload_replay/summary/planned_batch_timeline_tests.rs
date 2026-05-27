use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_kernel::{PartitionId, PartitionedScheduler, WaitForGraph};
use rem6_workload::WorkloadTopology;

use super::{
    parallel_execution_summary, workload_parallel_batch_timeline_record,
    WorkloadAcceleratorDmaActivity, WorkloadReplayActivityRefs,
};
use crate::workload_replay::WorkloadGpuDmaActivity;
use crate::workload_replay_heterogeneous::{WorkloadAcceleratorActivity, WorkloadGpuActivity};
use crate::{RiscvClusterTurn, RiscvSystemRun, RiscvSystemRunStopReason};

fn empty_coherence_wait_for_graphs() -> ParallelCoherenceWaitForGraphs {
    ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new())
}

fn data_cache_batch_run_at(
    partitions: u32,
    worker_limit: usize,
    tick: u64,
    scheduled_partitions: &[PartitionId],
) -> ParallelCoherenceRunSummary {
    let mut scheduler =
        PartitionedScheduler::with_parallel_worker_limit(partitions, 4, worker_limit).unwrap();
    for partition in scheduled_partitions {
        scheduler
            .schedule_parallel_at(*partition, tick, |_| {})
            .unwrap();
    }
    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        empty_coherence_wait_for_graphs(),
    )
}

#[test]
fn parallel_execution_summary_copies_planned_batch_timeline() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let cpu2 = PartitionId::new(2);
    let memory = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 5, 2).unwrap();
    scheduler
        .schedule_parallel_at(cpu0, 0, move |context| {
            context.schedule_remote_after(memory, 5, |_| {}).unwrap();
        })
        .unwrap();
    scheduler.schedule_parallel_at(cpu1, 1, |_| {}).unwrap();
    scheduler.schedule_parallel_at(cpu2, 3, |_| {}).unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvSystemRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_batch_run_at(4, 2, 2, &[cpu2, memory])]);
    let expected_scheduler = run
        .parallel_scheduler_planned_batch_timeline()
        .into_iter()
        .map(workload_parallel_batch_timeline_record)
        .collect::<Vec<_>>();
    let expected_data_cache = run
        .data_cache_parallel_scheduler_planned_batch_timeline()
        .into_iter()
        .map(workload_parallel_batch_timeline_record)
        .collect::<Vec<_>>();
    let expected_full_system = run
        .full_system_parallel_scheduler_planned_batch_timeline()
        .into_iter()
        .map(workload_parallel_batch_timeline_record)
        .collect::<Vec<_>>();
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );

    assert_ne!(
        summary.parallel_scheduler_batch_timeline(),
        expected_scheduler.as_slice(),
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_timeline(),
        expected_scheduler.as_slice(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_timeline(),
        expected_data_cache.as_slice(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_timeline(),
        expected_full_system,
    );
}
