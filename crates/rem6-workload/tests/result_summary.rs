use rem6_boot::BootImage;
use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::{ParallelRemoteFlowRecord, PartitionId};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadId, WorkloadManifest,
    WorkloadParallelExecutionSummary, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
        .add_segment(Address::new(0x8010), vec![0x73, 0x00, 0x00, 0x00])
        .unwrap()
}

fn kernel_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        "sha256:kernel",
        "resources/kernel.elf",
    )
    .unwrap()
}

#[test]
fn workload_result_records_parallel_execution_summary() {
    let manifest = WorkloadManifest::builder(id("result-parallel-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(3, 1, 7, 5)
        .with_scheduler_partitions(4, 2)
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 2, 5, 11),
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 3, 3, 17),
            ParallelRemoteFlowRecord::new(PartitionId::new(1), PartitionId::new(3), 0, 9, 9),
        ])
        .with_riscv_core_counts(2, 2, 4, 3, 1, 2)
        .with_data_cache_parallel_counts(7, 9, 11, 13, 3)
        .with_data_cache_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(5), 7, 19, 23),
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(5), 1, 13, 29),
        ])
        .with_data_cache_run_attribution(6, 1)
        .with_data_cache_protocol_counts([
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Moesi, 3),
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Msi, 2),
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Chi, 1),
        ])
        .with_data_cache_diagnostics(17, 19)
        .with_fabric_activity(2, 7, 224, 31, 13, 8, 1)
        .with_dram_activity(1, 2, 3, 5, 4, 1, 2, 3, 11, 1, 83, 21)
        .with_dram_qos_activity(
            3,
            24,
            1,
            [
                WorkloadDramQosPrioritySummary::new(QosPriority::new(1), 1, 8),
                WorkloadDramQosPrioritySummary::new(QosPriority::new(0), 2, 16),
            ],
            [
                WorkloadDramQosRequestorSummary::new(QosRequestorId::new(8), 1, 8),
                WorkloadDramQosRequestorSummary::new(QosRequestorId::new(7), 2, 16),
            ],
        );

    let result = WorkloadResult::new(manifest.identity(), 96)
        .with_parallel_execution_summary(summary.clone());

    assert_eq!(result.parallel_execution_summary(), Some(&summary));
    assert_eq!(summary.scheduler_epoch_count(), 3);
    assert_eq!(summary.scheduler_empty_epoch_count(), 1);
    assert_eq!(summary.scheduler_dispatch_count(), 7);
    assert_eq!(summary.scheduler_batch_count(), 5);
    assert_eq!(summary.active_scheduler_partition_count(), 4);
    assert_eq!(summary.max_parallel_scheduler_workers(), 2);
    assert_eq!(
        summary.parallel_scheduler_remote_flows(),
        &[ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(2),
            5,
            3,
            17,
        )],
    );
    assert_eq!(
        summary.parallel_scheduler_remote_flow_count(PartitionId::new(0), PartitionId::new(2)),
        5,
    );
    assert_eq!(
        summary.parallel_scheduler_remote_flow_count(PartitionId::new(2), PartitionId::new(0)),
        0,
    );
    assert!(summary.has_parallel_scheduler_remote_flows());
    assert_eq!(summary.riscv_core_count(), 2);
    assert_eq!(summary.active_riscv_core_count(), 2);
    assert_eq!(summary.riscv_fetch_issue_count(), 4);
    assert_eq!(summary.riscv_committed_instruction_count(), 3);
    assert_eq!(summary.riscv_data_access_issue_count(), 1);
    assert_eq!(summary.riscv_scheduled_trap_count(), 2);
    assert!(summary.has_riscv_core_activity());
    assert_eq!(summary.data_cache_parallel_run_count(), 7);
    assert_eq!(summary.data_cache_parallel_scheduler_epoch_count(), 9);
    assert_eq!(summary.data_cache_parallel_scheduler_dispatch_count(), 11);
    assert_eq!(summary.data_cache_parallel_scheduler_batch_count(), 13);
    assert_eq!(summary.data_cache_parallel_scheduler_max_workers(), 3);
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_flows(),
        &[ParallelRemoteFlowRecord::new(
            PartitionId::new(4),
            PartitionId::new(5),
            8,
            13,
            29,
        )],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_flow_count(
            PartitionId::new(4),
            PartitionId::new(5),
        ),
        8,
    );
    assert!(summary.has_data_cache_parallel_scheduler_remote_flows());
    assert_eq!(summary.attributed_data_cache_parallel_run_count(), 6);
    assert_eq!(summary.unattributed_data_cache_parallel_run_count(), 1);
    assert_eq!(
        summary.data_cache_protocol_counts(),
        &[
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Msi, 2),
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Moesi, 3),
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Chi, 1),
        ]
    );
    assert_eq!(
        summary.data_cache_protocols(),
        vec![
            WorkloadDataCacheProtocol::Msi,
            WorkloadDataCacheProtocol::Moesi,
            WorkloadDataCacheProtocol::Chi,
        ],
    );
    assert_eq!(WorkloadDataCacheProtocol::Msi.as_str(), "msi");
    assert_eq!(WorkloadDataCacheProtocol::Mesi.as_str(), "mesi");
    assert_eq!(WorkloadDataCacheProtocol::Moesi.as_str(), "moesi");
    assert_eq!(WorkloadDataCacheProtocol::Chi.as_str(), "chi");
    assert!(!summary.data_cache_protocol_counts()[0].is_empty());
    assert!(WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Mesi, 0).is_empty());
    assert_eq!(summary.attributed_data_cache_protocol_run_count(), 6);
    assert_eq!(
        summary
            .data_cache_protocol_count_map()
            .get(&WorkloadDataCacheProtocol::Msi),
        Some(&2),
    );
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Mesi),
        0,
    );
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Moesi),
        3,
    );
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Chi),
        1,
    );
    assert!(summary.has_data_cache_protocol(WorkloadDataCacheProtocol::Msi));
    assert!(summary.has_data_cache_protocol(WorkloadDataCacheProtocol::Chi));
    assert!(!summary.has_data_cache_protocol(WorkloadDataCacheProtocol::Mesi));
    assert_eq!(summary.data_cache_wait_for_edge_count(), 17);
    assert_eq!(summary.data_cache_deadlock_diagnostic_count(), 19);
    assert!(summary.has_unattributed_data_cache_parallel_runs());
    assert!(summary.has_data_cache_diagnostics());
    assert_eq!(summary.active_fabric_lane_count(), 2);
    assert_eq!(summary.fabric_transfer_count(), 7);
    assert_eq!(summary.fabric_byte_count(), 224);
    assert_eq!(summary.fabric_occupied_ticks(), 31);
    assert_eq!(summary.fabric_queue_delay_ticks(), 13);
    assert_eq!(summary.fabric_max_queue_delay_ticks(), 8);
    assert_eq!(summary.contended_fabric_lane_count(), 1);
    assert!(summary.has_fabric_activity());
    assert!(summary.has_fabric_contention());
    assert_eq!(summary.active_dram_target_count(), 1);
    assert_eq!(summary.active_dram_port_count(), 2);
    assert_eq!(summary.active_dram_bank_count(), 3);
    assert_eq!(summary.dram_access_count(), 5);
    assert_eq!(summary.dram_read_count(), 4);
    assert_eq!(summary.dram_write_count(), 1);
    assert_eq!(summary.dram_row_hit_count(), 2);
    assert_eq!(summary.dram_row_miss_count(), 3);
    assert_eq!(summary.dram_command_count(), 11);
    assert_eq!(summary.dram_turnaround_count(), 1);
    assert_eq!(summary.dram_total_ready_latency_cycles(), 83);
    assert_eq!(summary.dram_max_ready_latency_cycles(), 21);
    assert!(summary.has_dram_activity());
    assert!(summary.has_dram_row_misses());
    assert_eq!(summary.dram_qos_access_count(), 3);
    assert_eq!(summary.dram_qos_byte_count(), 24);
    assert_eq!(summary.dram_qos_escalated_access_count(), 1);
    assert_eq!(
        summary.dram_qos_priority_summaries(),
        &[
            WorkloadDramQosPrioritySummary::new(QosPriority::new(0), 2, 16),
            WorkloadDramQosPrioritySummary::new(QosPriority::new(1), 1, 8),
        ]
    );
    assert_eq!(
        summary.dram_qos_requestor_summaries(),
        &[
            WorkloadDramQosRequestorSummary::new(QosRequestorId::new(7), 2, 16),
            WorkloadDramQosRequestorSummary::new(QosRequestorId::new(8), 1, 8),
        ]
    );
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
    assert!(summary.has_dram_qos_activity());
    assert_eq!(
        summary.resource_activity_count(),
        summary.fabric_transfer_count()
            + summary.dram_access_count()
            + summary.resource_wait_for_edge_count(),
    );
    assert!(summary.has_resource_activity());
    assert_eq!(summary.full_system_parallel_scheduler_epoch_count(), 12);
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 18);
    assert_eq!(summary.full_system_parallel_scheduler_batch_count(), 18);
    assert_eq!(summary.full_system_parallel_scheduler_max_workers(), 3);
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_flows(),
        vec![
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 5, 3, 17),
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(5), 8, 13, 29),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_flow_count(
            PartitionId::new(0),
            PartitionId::new(2),
        ),
        5,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_flow_count(
            PartitionId::new(4),
            PartitionId::new(5),
        ),
        8,
    );
    assert!(summary.has_full_system_parallel_scheduler_remote_flows());
    assert!(summary.has_full_system_parallel_scheduler_work());
    assert!(summary.has_parallel_scheduler_work());
    assert!(summary.has_data_cache_parallel_work());
}
