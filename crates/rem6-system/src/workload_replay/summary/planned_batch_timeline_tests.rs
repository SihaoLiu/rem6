use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_kernel::{ParallelBatchUtilizationRatio, PartitionId, PartitionedScheduler, WaitForGraph};
use rem6_workload::{
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerLaneRecord, WorkloadTopology,
};

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

fn workload_batch_record(
    scope: WorkloadParallelBatchScope,
    start_tick: u64,
    horizon: u64,
    partitions: impl IntoIterator<Item = PartitionId>,
    worker_count: usize,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(scope, start_tick, horizon, partitions, worker_count)
}

fn workload_lane_record(
    scope: WorkloadParallelBatchScope,
    lane: usize,
    partition: PartitionId,
    start_tick: u64,
    horizon: u64,
) -> WorkloadParallelBatchWorkerLaneRecord {
    WorkloadParallelBatchWorkerLaneRecord::new(scope, lane, partition, start_tick, horizon)
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
        summary.parallel_scheduler_planned_batch_worker_lanes(),
        &[
            workload_lane_record(WorkloadParallelBatchScope::Scheduler, 0, cpu0, 0, 5),
            workload_lane_record(WorkloadParallelBatchScope::Scheduler, 1, cpu1, 1, 5),
            workload_lane_record(WorkloadParallelBatchScope::Scheduler, 0, cpu2, 3, 5),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_lane_tick_summaries(),
        vec![(0, 7), (1, 4)],
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_lane_partition_ticks(0, cpu2),
        2,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_timeline(),
        expected_data_cache.as_slice(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_worker_lanes(),
        &[
            workload_lane_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                0,
                cpu2,
                2,
                4
            ),
            workload_lane_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                1,
                memory,
                2,
                4
            ),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_timeline(),
        expected_full_system,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_worker_lanes(),
        vec![
            workload_lane_record(WorkloadParallelBatchScope::Scheduler, 0, cpu0, 0, 5),
            workload_lane_record(WorkloadParallelBatchScope::Scheduler, 1, cpu1, 1, 5),
            workload_lane_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                0,
                cpu2,
                2,
                4
            ),
            workload_lane_record(
                WorkloadParallelBatchScope::DataCacheScheduler,
                1,
                memory,
                2,
                4
            ),
            workload_lane_record(WorkloadParallelBatchScope::Scheduler, 0, cpu2, 3, 5),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_ticks(),
        run.parallel_scheduler_planned_batch_worker_ticks(),
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_capacity_ticks(),
        run.parallel_scheduler_planned_batch_worker_capacity_ticks(),
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_idle_worker_ticks(),
        run.parallel_scheduler_planned_batch_idle_worker_ticks(),
    );
    assert_eq!(
        summary
            .parallel_scheduler_planned_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(
            run.parallel_scheduler_planned_batch_worker_ticks(),
            run.parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
        .unwrap(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_worker_ticks(),
        run.data_cache_parallel_scheduler_planned_batch_worker_ticks(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        run.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_idle_worker_ticks(),
        run.data_cache_parallel_scheduler_planned_batch_idle_worker_ticks(),
    );
    assert_eq!(
        summary
            .data_cache_parallel_scheduler_planned_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(
            run.data_cache_parallel_scheduler_planned_batch_worker_ticks(),
            run.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
        .unwrap(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_worker_ticks(),
        run.full_system_parallel_scheduler_planned_batch_worker_ticks(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        run.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_idle_worker_ticks(),
        run.full_system_parallel_scheduler_planned_batch_idle_worker_ticks(),
    );
    assert_eq!(
        summary
            .full_system_parallel_scheduler_planned_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(
            run.full_system_parallel_scheduler_planned_batch_worker_ticks(),
            run.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
        .unwrap(),
    );
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_worker_ticks(),
        run.parallel_scheduler_batch_worker_ticks(),
    );
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        run.parallel_scheduler_batch_worker_capacity_ticks(),
    );
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_idle_worker_ticks(),
        run.parallel_scheduler_batch_idle_worker_ticks(),
    );
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        run.parallel_scheduler_batch_worker_slot_tick_summaries(),
    );
    assert_eq!(
        summary
            .parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        run.parallel_scheduler_batch_utilization_ratio().unwrap(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_worker_ticks(),
        run.data_cache_parallel_scheduler_batch_worker_ticks(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        run.data_cache_parallel_scheduler_batch_worker_capacity_ticks(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_idle_worker_ticks(),
        run.data_cache_parallel_scheduler_batch_idle_worker_ticks(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        run.data_cache_parallel_scheduler_batch_worker_slot_tick_summaries(),
    );
    assert_eq!(
        summary
            .data_cache_parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        run.data_cache_parallel_scheduler_batch_utilization_ratio()
            .unwrap(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_ticks(),
        run.full_system_parallel_scheduler_batch_worker_ticks(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        run.full_system_parallel_scheduler_batch_worker_capacity_ticks(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_idle_worker_ticks(),
        run.full_system_parallel_scheduler_batch_idle_worker_ticks(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        run.full_system_parallel_scheduler_batch_worker_slot_tick_summaries(),
    );
    assert_eq!(
        summary
            .full_system_parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        run.full_system_parallel_scheduler_batch_utilization_ratio()
            .unwrap(),
    );
}

#[test]
fn parallel_execution_summary_copies_dma_recorded_batch_capacity() {
    let cpu = PartitionId::new(0);
    let gpu = PartitionId::new(1);
    let memory = PartitionId::new(2);
    let accelerator = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();
    scheduler.schedule_parallel_at(cpu, 0, |_| {}).unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvSystemRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 16 },
    );
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu_activity = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity {
        scheduler_batch_timeline: vec![workload_batch_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            0,
            4,
            [gpu, memory],
            2,
        )],
        scheduler_recorded_batch_worker_capacity_ticks: 12,
        scheduler_recorded_batch_worker_slot_tick_summaries: vec![(0, 4, 0), (1, 4, 0), (2, 0, 4)],
        ..WorkloadGpuDmaActivity::default()
    };
    let accelerator_activity = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity {
        scheduler_batch_timeline: vec![workload_batch_record(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            4,
            8,
            [accelerator],
            1,
        )],
        scheduler_recorded_batch_worker_capacity_ticks: 8,
        scheduler_recorded_batch_worker_slot_tick_summaries: vec![(0, 4, 0), (1, 0, 4)],
        ..WorkloadAcceleratorDmaActivity::default()
    };
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu_activity,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator_activity,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );

    assert_eq!(summary.gpu_dma_scheduler_recorded_batch_worker_ticks(), 8);
    assert_eq!(
        summary.gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(),
        12,
    );
    assert_eq!(
        summary.gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 4, 0), (1, 4, 0), (2, 0, 4)],
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_recorded_batch_worker_ticks(),
        4,
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(),
        8,
    );
    assert_eq!(
        summary.dma_scheduler_recorded_batch_worker_capacity_ticks(),
        20,
    );
    assert_eq!(
        summary.dma_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 8, 0), (1, 4, 4), (2, 0, 4)],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        28,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 12, 0), (1, 4, 8), (2, 0, 4)],
    );
}
