use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadParallelExecutionSummary, WorkloadTopology,
};

use super::workload_replay_dma::WorkloadAcceleratorDmaActivity;
use crate::workload_replay_heterogeneous::{WorkloadAcceleratorActivity, WorkloadGpuActivity};
use crate::{RiscvDataCacheProtocol, RiscvSystemRun};

pub(super) struct WorkloadReplayActivityRefs<'a> {
    pub(super) gpu: &'a WorkloadGpuActivity,
    pub(super) gpu_dma: &'a super::WorkloadGpuDmaActivity,
    pub(super) accelerator: &'a WorkloadAcceleratorActivity,
    pub(super) accelerator_dma: &'a WorkloadAcceleratorDmaActivity,
}

pub(super) fn parallel_execution_summary(
    run: &RiscvSystemRun,
    topology: &WorkloadTopology,
    activities: WorkloadReplayActivityRefs<'_>,
) -> WorkloadParallelExecutionSummary {
    let scheduler = run.parallel_scheduler_profile();
    let fabric = run.fabric_profile();
    let dram = run.dram_profile();
    let cpu_activities = run.cpu_activities();
    let riscv_fetch_issue_count = cpu_activities
        .values()
        .map(|activity| activity.fetch_issue_count())
        .sum();
    let riscv_committed_instruction_count = cpu_activities
        .values()
        .map(|activity| activity.instruction_execution_count())
        .sum();
    let riscv_data_access_issue_count = cpu_activities
        .values()
        .map(|activity| activity.data_access_issue_count())
        .sum();
    let riscv_scheduled_trap_count = cpu_activities
        .values()
        .map(|activity| activity.scheduled_trap_count())
        .sum();
    WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(
            scheduler.epoch_count(),
            scheduler.empty_epoch_count(),
            scheduler.dispatch_count(),
            scheduler.batch_count(),
        )
        .with_scheduler_partitions(
            run.active_parallel_scheduler_partition_count(),
            run.max_parallel_scheduler_workers(),
        )
        .with_riscv_core_counts(
            topology.riscv_cores().len(),
            cpu_activities.len(),
            riscv_fetch_issue_count,
            riscv_committed_instruction_count,
            riscv_data_access_issue_count,
            riscv_scheduled_trap_count,
        )
        .with_data_cache_parallel_counts(
            run.data_cache_run_count(),
            run.data_cache_parallel_scheduler_epoch_count(),
            run.data_cache_parallel_scheduler_dispatch_count(),
            run.data_cache_parallel_scheduler_batch_count(),
            run.data_cache_parallel_scheduler_max_workers(),
        )
        .with_data_cache_run_attribution(
            run.attributed_data_cache_parallel_run_count(),
            run.unattributed_data_cache_parallel_run_count(),
        )
        .with_data_cache_protocol_counts(run.data_cache_protocol_counts().into_iter().map(
            |(protocol, run_count)| {
                WorkloadDataCacheProtocolCount::new(
                    workload_data_cache_protocol(protocol),
                    run_count,
                )
            },
        ))
        .with_data_cache_diagnostics(
            run.data_cache_wait_for_edge_count(),
            run.data_cache_deadlock_diagnostic_count(),
        )
        .with_fabric_activity(
            fabric.active_lane_count(),
            fabric.transfer_count(),
            fabric.byte_count(),
            fabric.occupied_ticks(),
            fabric.queue_delay_ticks(),
            fabric.max_queue_delay_ticks(),
            fabric.contended_lane_count(),
        )
        .with_dram_activity(
            dram.active_target_count(),
            dram.active_port_count(),
            dram.active_bank_count(),
            dram.access_count(),
            dram.read_count(),
            dram.write_count(),
            dram.row_hit_count(),
            dram.row_miss_count(),
            dram.command_count(),
            dram.turnaround_count(),
            dram.total_ready_latency_cycles(),
            dram.max_ready_latency_cycles(),
        )
        .with_dram_qos_activity(
            dram.qos_access_count(),
            dram.qos_byte_count(),
            dram.qos_escalated_access_count(),
            dram.qos_priorities().into_iter().map(|priority| {
                WorkloadDramQosPrioritySummary::new(
                    priority,
                    dram.qos_priority_access_count(priority),
                    dram.qos_priority_byte_count(priority),
                )
            }),
            dram.qos_requestors().into_iter().map(|requestor| {
                WorkloadDramQosRequestorSummary::new(
                    requestor,
                    dram.qos_requestor_access_count(requestor),
                    dram.qos_requestor_byte_count(requestor),
                )
            }),
        )
        .with_resource_diagnostics(
            run.fabric_wait_for_edge_count(),
            run.fabric_deadlock_diagnostic_count(),
            run.dram_wait_for_edge_count(),
            run.dram_deadlock_diagnostic_count(),
        )
        .with_gpu_compute_counts(
            activities.gpu.kernel_launch_count,
            activities.gpu.trace_event_count,
            activities.gpu.workgroup_completion_count,
            activities.gpu.active_device_count,
        )
        .with_gpu_compute_diagnostics(
            activities.gpu.wait_for_edge_count,
            activities.gpu.deadlock_diagnostic_count,
        )
        .with_gpu_dma_counts(
            activities.gpu_dma.copy_count,
            activities.gpu_dma.completion_count,
            activities.gpu_dma.active_device_count,
        )
        .with_gpu_dma_diagnostics(
            activities.gpu_dma.wait_for_edge_count,
            activities.gpu_dma.deadlock_diagnostic_count,
        )
        .with_accelerator_compute_counts(
            activities.accelerator.command_count,
            activities.accelerator.trace_event_count,
            activities.accelerator.completion_count,
            activities.accelerator.active_device_count,
        )
        .with_accelerator_compute_diagnostics(
            activities.accelerator.wait_for_edge_count,
            activities.accelerator.deadlock_diagnostic_count,
        )
        .with_accelerator_dma_counts(
            activities.accelerator_dma.copy_count,
            activities.accelerator_dma.completion_count,
            activities.accelerator_dma.active_device_count,
        )
        .with_accelerator_dma_diagnostics(
            activities.accelerator_dma.wait_for_edge_count,
            activities.accelerator_dma.deadlock_diagnostic_count,
        )
}

fn workload_data_cache_protocol(protocol: RiscvDataCacheProtocol) -> WorkloadDataCacheProtocol {
    match protocol {
        RiscvDataCacheProtocol::Msi => WorkloadDataCacheProtocol::Msi,
        RiscvDataCacheProtocol::Mesi => WorkloadDataCacheProtocol::Mesi,
        RiscvDataCacheProtocol::Moesi => WorkloadDataCacheProtocol::Moesi,
        RiscvDataCacheProtocol::Chi => WorkloadDataCacheProtocol::Chi,
    }
}

#[cfg(test)]
mod tests {
    use rem6_dram::{
        DramController, DramGeometry, DramQosRequest, DramQosSchedulingPolicy,
        DramQosTurnaroundPolicy, DramTargetActivity, DramTiming,
    };
    use rem6_fabric::{QosPriority, QosQueueArbiter, QosQueuePolicyKind, QosRequestorId};
    use rem6_memory::{
        AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId,
        MemoryTargetId,
    };

    use super::*;
    use crate::workload_replay::{WorkloadGpuDmaActivity, WorkloadReplayActivityRefs};
    use crate::workload_replay_heterogeneous::{WorkloadAcceleratorActivity, WorkloadGpuActivity};
    use crate::RiscvSystemRunStopReason;

    fn layout() -> CacheLineLayout {
        CacheLineLayout::new(64).unwrap()
    }

    fn request(agent: u32, address: u64, sequence: u64) -> MemoryRequest {
        MemoryRequest::read_shared(
            MemoryRequestId::new(AgentId::new(agent), sequence),
            Address::new(address),
            AccessSize::new(8).unwrap(),
            layout(),
        )
        .unwrap()
    }

    fn qos_dram_activity(target: MemoryTargetId) -> DramTargetActivity {
        let mut controller = DramController::new(
            DramGeometry::new(4, 256, 64).unwrap(),
            DramTiming::new(3, 5, 7, 2, 4).unwrap(),
        );
        let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
        let low = request(7, 0x0000, 50);
        let other = request(8, 0x0040, 51);
        let high = request(7, 0x0100, 52);

        controller
            .schedule_qos_batch_with_policy(
                0,
                [
                    DramQosRequest::new(&low, QosPriority::new(2), 0),
                    DramQosRequest::new(&other, QosPriority::new(1), 1),
                    DramQosRequest::new(&high, QosPriority::new(0), 2),
                ],
                &mut arbiter,
                DramQosSchedulingPolicy::new()
                    .with_priority_escalation()
                    .with_turnaround(DramQosTurnaroundPolicy::RequestOrder),
            )
            .unwrap();

        DramTargetActivity::new(target, controller.activity_profile())
    }

    #[test]
    fn parallel_execution_summary_copies_dram_qos_activity() {
        let topology = WorkloadTopology::new(
            1,
            1,
            1,
            rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
        )
        .unwrap();
        let run = RiscvSystemRun::new(
            Vec::new(),
            Vec::new(),
            RiscvSystemRunStopReason::Idle { tick: 0 },
        )
        .with_dram_activity(vec![qos_dram_activity(MemoryTargetId::new(2))]);
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
        );

        assert!(summary.has_dram_qos_activity());
        assert_eq!(summary.dram_qos_access_count(), 3);
        assert_eq!(summary.dram_qos_byte_count(), 24);
        assert_eq!(summary.dram_qos_escalated_access_count(), 1);
        assert_eq!(
            summary.dram_qos_priority_access_count(QosPriority::new(0)),
            2,
        );
        assert_eq!(
            summary.dram_qos_priority_byte_count(QosPriority::new(0)),
            16
        );
        assert_eq!(
            summary.dram_qos_requestor_access_count(QosRequestorId::new(7)),
            2,
        );
        assert_eq!(
            summary.dram_qos_requestor_byte_count(QosRequestorId::new(7)),
            16,
        );
    }
}
