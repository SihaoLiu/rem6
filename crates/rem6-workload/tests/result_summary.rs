use rem6_boot::BootImage;
use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::{
    LivelockTransitionKind, ParallelPartitionActivity, ParallelProgressTransitionRecord,
    ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionFrontier, PartitionId,
    WaitForEdgeKind, WaitForNode,
};
use rem6_memory::Address;
use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadError, WorkloadId, WorkloadManifest,
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchPartitionStreak,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerCount, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadResult, WorkloadWaitForEdgeKindWindow, WorkloadWaitForTargetNodeWindow,
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

fn wait_subject(value: &str) -> WaitForNode {
    WaitForNode::component(value).unwrap()
}

fn wait_resource(value: &str) -> WaitForNode {
    WaitForNode::resource(value).unwrap()
}

fn progress_transition(
    partition: u32,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
) -> ParallelProgressTransitionRecord {
    ParallelProgressTransitionRecord::new(PartitionId::new(partition), subject, kind, tick, order)
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
        .with_scheduler_worker_count(15)
        .with_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(1, 2),
            WorkloadParallelBatchWorkerCount::new(2, 3),
        ])
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 3),
        ])
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 2, 5, 11),
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 3, 3, 17),
            ParallelRemoteFlowRecord::new(PartitionId::new(1), PartitionId::new(3), 0, 9, 9),
        ])
        .with_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                5,
                11,
                1,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                3,
                17,
                0,
            ),
        ])
        .with_parallel_scheduler_frontiers(
            [
                PartitionFrontier::new(PartitionId::new(0), 0, 8, Some(2), 1),
                PartitionFrontier::new(PartitionId::new(1), 0, 8, Some(4), 1),
            ],
            [
                PartitionFrontier::new(PartitionId::new(0), 8, 16, None, 0),
                PartitionFrontier::new(PartitionId::new(1), 4, 16, Some(12), 1),
            ],
        )
        .with_riscv_core_counts(2, 2, 4, 3, 1, 2)
        .with_data_cache_parallel_counts(7, 9, 11, 13, 3)
        .with_data_cache_parallel_empty_epoch_count(2)
        .with_data_cache_parallel_partitions(6)
        .with_data_cache_parallel_worker_count(21)
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 4),
            WorkloadParallelBatchWorkerCount::new(3, 9),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 4),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(1),
                    PartitionId::new(2),
                    PartitionId::new(3),
                ],
                9,
            ),
        ])
        .with_full_system_parallel_partitions(8)
        .with_data_cache_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(5), 7, 19, 23),
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(5), 1, 13, 29),
        ])
        .with_data_cache_parallel_scheduler_remote_sends([
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(4),
                PartitionId::new(5),
                19,
                23,
                3,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(4),
                PartitionId::new(5),
                13,
                29,
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_frontiers(
            [PartitionFrontier::new(
                PartitionId::new(4),
                13,
                21,
                Some(19),
                2,
            )],
            [PartitionFrontier::new(PartitionId::new(4), 21, 29, None, 0)],
        )
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
    assert_eq!(summary.scheduler_dispatch_count(), 10);
    assert_eq!(summary.scheduler_batch_count(), 5);
    assert_eq!(summary.active_scheduler_partition_count(), 4);
    assert_eq!(summary.max_parallel_scheduler_workers(), 2);
    assert_eq!(summary.total_parallel_scheduler_workers(), 15);
    assert_eq!(
        summary.parallel_scheduler_batch_worker_counts(),
        &[WorkloadParallelBatchWorkerCount::new(2, 3)],
    );
    assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(2), 5);
    assert_eq!(
        summary.parallel_scheduler_batch_partition_sets(),
        &[
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 3),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_batch_count_for_partition_set([
            PartitionId::new(0),
            PartitionId::new(2),
        ]),
        3,
    );
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
    assert_eq!(
        summary.parallel_scheduler_remote_sends(),
        &[
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                3,
                17,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                5,
                11,
                1,
            ),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_remote_send_count(PartitionId::new(0), PartitionId::new(2)),
        2,
    );
    assert!(summary.has_parallel_scheduler_remote_sends());
    assert_eq!(summary.parallel_scheduler_remote_sends()[0].delay(), 14);
    assert_eq!(
        summary.parallel_scheduler_initial_frontiers(),
        &[
            PartitionFrontier::new(PartitionId::new(0), 0, 8, Some(2), 1),
            PartitionFrontier::new(PartitionId::new(1), 0, 8, Some(4), 1),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_final_frontiers(),
        &[
            PartitionFrontier::new(PartitionId::new(0), 8, 16, None, 0),
            PartitionFrontier::new(PartitionId::new(1), 4, 16, Some(12), 1),
        ],
    );
    assert_eq!(summary.parallel_scheduler_initial_frontier_count(), 2);
    assert_eq!(summary.parallel_scheduler_final_frontier_count(), 2);
    assert!(summary.has_parallel_scheduler_frontiers());
    assert_eq!(summary.riscv_core_count(), 2);
    assert_eq!(summary.active_riscv_core_count(), 2);
    assert_eq!(summary.riscv_fetch_issue_count(), 4);
    assert_eq!(summary.riscv_committed_instruction_count(), 3);
    assert_eq!(summary.riscv_data_access_issue_count(), 1);
    assert_eq!(summary.riscv_scheduled_trap_count(), 2);
    assert!(summary.has_riscv_core_activity());
    assert_eq!(summary.data_cache_parallel_run_count(), 7);
    assert_eq!(summary.data_cache_parallel_scheduler_epoch_count(), 9);
    assert_eq!(summary.data_cache_parallel_scheduler_empty_epoch_count(), 2);
    assert_eq!(summary.data_cache_parallel_scheduler_dispatch_count(), 35);
    assert_eq!(summary.data_cache_parallel_scheduler_batch_count(), 13);
    assert_eq!(
        summary.active_data_cache_parallel_scheduler_partition_count(),
        6
    );
    assert_eq!(summary.data_cache_parallel_scheduler_max_workers(), 3);
    assert_eq!(summary.data_cache_parallel_scheduler_total_workers(), 35);
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_worker_counts(),
        &[
            WorkloadParallelBatchWorkerCount::new(2, 4),
            WorkloadParallelBatchWorkerCount::new(3, 9),
        ],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_at_or_above(3),
        9,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_partition_sets(),
        &[
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 4),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(1),
                    PartitionId::new(2),
                    PartitionId::new(3)
                ],
                9,
            ),
        ],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_for_partition_set([
            PartitionId::new(1),
            PartitionId::new(2),
            PartitionId::new(3),
        ]),
        9,
    );
    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        8
    );
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
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_sends(),
        &[
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(4),
                PartitionId::new(5),
                13,
                29,
                2,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(4),
                PartitionId::new(5),
                19,
                23,
                3,
            ),
        ],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_send_count(
            PartitionId::new(4),
            PartitionId::new(5),
        ),
        2,
    );
    assert!(summary.has_data_cache_parallel_scheduler_remote_sends());
    assert_eq!(
        summary.data_cache_parallel_scheduler_initial_frontiers(),
        &[PartitionFrontier::new(
            PartitionId::new(4),
            13,
            21,
            Some(19),
            2,
        )],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_final_frontiers(),
        &[PartitionFrontier::new(PartitionId::new(4), 21, 29, None, 0,)],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_initial_frontier_count(),
        1
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_final_frontier_count(),
        1
    );
    assert!(summary.has_data_cache_parallel_scheduler_frontiers());
    assert_eq!(
        summary.full_system_parallel_scheduler_initial_frontiers(),
        vec![
            PartitionFrontier::new(PartitionId::new(0), 0, 8, Some(2), 1),
            PartitionFrontier::new(PartitionId::new(1), 0, 8, Some(4), 1),
            PartitionFrontier::new(PartitionId::new(4), 13, 21, Some(19), 2),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_final_frontiers(),
        vec![
            PartitionFrontier::new(PartitionId::new(0), 8, 16, None, 0),
            PartitionFrontier::new(PartitionId::new(1), 4, 16, Some(12), 1),
            PartitionFrontier::new(PartitionId::new(4), 21, 29, None, 0),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_initial_frontier_count(),
        3
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_final_frontier_count(),
        3
    );
    assert!(summary.has_full_system_parallel_scheduler_frontiers());
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
    assert_eq!(
        summary.full_system_parallel_scheduler_empty_epoch_count(),
        3
    );
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 45);
    assert_eq!(summary.full_system_parallel_scheduler_batch_count(), 18);
    assert_eq!(summary.full_system_parallel_scheduler_max_workers(), 3);
    assert_eq!(summary.full_system_parallel_scheduler_total_workers(), 50);
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_counts(),
        vec![
            WorkloadParallelBatchWorkerCount::new(2, 7),
            WorkloadParallelBatchWorkerCount::new(3, 9),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_at_or_above(2),
        18,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_partition_sets(),
        vec![
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 7),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(1),
                    PartitionId::new(2),
                    PartitionId::new(3)
                ],
                9,
            ),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_partition_set([
            PartitionId::new(0),
            PartitionId::new(2),
        ]),
        7,
    );
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
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_sends(),
        vec![
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                3,
                17,
                0,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(0),
                PartitionId::new(2),
                5,
                11,
                1,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(4),
                PartitionId::new(5),
                13,
                29,
                2,
            ),
            ParallelRemoteSendRecord::with_timing(
                PartitionId::new(4),
                PartitionId::new(5),
                19,
                23,
                3,
            ),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_send_count(
            PartitionId::new(4),
            PartitionId::new(5),
        ),
        2,
    );
    assert!(summary.has_full_system_parallel_scheduler_remote_sends());
    assert!(summary.has_full_system_parallel_scheduler_work());
    assert!(summary.has_parallel_scheduler_work());
    assert!(summary.has_data_cache_parallel_work());
}

#[test]
fn workload_result_records_scoped_parallel_batch_timeline() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let scheduler_early = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [cpu0, cpu1],
        2,
    );
    let scheduler_late = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::Scheduler,
        8,
        12,
        [cache],
        1,
    );
    let data_cache = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::DataCacheScheduler,
        4,
        8,
        [cpu1, cache],
        2,
    );
    let gpu_dma_early = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        2,
        3,
        [cpu0, cache],
        3,
    );
    let gpu_dma_late = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::GpuDmaScheduler,
        3,
        6,
        [cpu0, cpu1, cache],
        3,
    );
    let accelerator_dma = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::AcceleratorDmaScheduler,
        6,
        11,
        [cpu1, cache],
        4,
    );
    let empty = WorkloadParallelBatchTimelineRecord::new(
        WorkloadParallelBatchScope::Scheduler,
        12,
        16,
        [cpu0],
        0,
    );
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([
            scheduler_late.clone(),
            empty,
            scheduler_early.clone(),
        ])
        .with_data_cache_parallel_scheduler_batch_timeline([data_cache.clone()])
        .with_gpu_dma_scheduler_counts(1, 2, 2, [(3, 4)])
        .with_gpu_dma_scheduler_batch_worker_counts([WorkloadParallelBatchWorkerCount::new(3, 2)])
        .with_gpu_dma_scheduler_batch_timeline([gpu_dma_late.clone(), gpu_dma_early.clone()])
        .with_accelerator_dma_scheduler_counts(1, 1, 1, [(4, 5)])
        .with_accelerator_dma_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(4, 1),
        ])
        .with_accelerator_dma_scheduler_batch_timeline([accelerator_dma.clone()]);

    assert_eq!(WorkloadParallelBatchScope::Scheduler.as_str(), "scheduler");
    assert_eq!(
        WorkloadParallelBatchScope::DataCacheScheduler.as_str(),
        "data-cache-scheduler",
    );
    assert_eq!(
        WorkloadParallelBatchScope::GpuDmaScheduler.as_str(),
        "gpu-dma-scheduler",
    );
    assert_eq!(
        WorkloadParallelBatchScope::AcceleratorDmaScheduler.as_str(),
        "accelerator-dma-scheduler",
    );
    assert_eq!(
        summary.parallel_scheduler_batch_timeline(),
        &[scheduler_early.clone(), scheduler_late.clone()],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_timeline(),
        std::slice::from_ref(&data_cache),
    );
    assert_eq!(
        summary.gpu_dma_scheduler_batch_timeline(),
        &[gpu_dma_early.clone(), gpu_dma_late.clone()],
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_batch_timeline(),
        std::slice::from_ref(&accelerator_dma),
    );
    assert_eq!(
        summary.dma_scheduler_batch_timeline(),
        vec![
            gpu_dma_early.clone(),
            gpu_dma_late.clone(),
            accelerator_dma.clone(),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_timeline(),
        vec![
            scheduler_early.clone(),
            gpu_dma_early.clone(),
            gpu_dma_late.clone(),
            data_cache.clone(),
            accelerator_dma.clone(),
            scheduler_late.clone()
        ],
    );
    assert_eq!(scheduler_early.duration_ticks(), 4);
    assert_eq!(data_cache.duration_ticks(), 4);
    assert_eq!(summary.scheduler_batch_count(), 2);
    assert_eq!(summary.data_cache_parallel_scheduler_batch_count(), 1);
    assert_eq!(summary.gpu_dma_scheduler_batch_count(), 2);
    assert_eq!(summary.accelerator_dma_scheduler_batch_count(), 1);
    assert_eq!(summary.dma_scheduler_batch_count(), 3);
    assert_eq!(summary.dma_scheduler_max_workers(), 4);
    assert_eq!(summary.dma_scheduler_total_workers(), 10);
    assert_eq!(summary.full_system_parallel_scheduler_batch_count(), 6);
    assert_eq!(summary.full_system_parallel_scheduler_max_workers(), 4);
    assert_eq!(summary.full_system_parallel_scheduler_total_workers(), 14);
    assert_eq!(
        summary.gpu_dma_scheduler_batch_worker_counts(),
        &[WorkloadParallelBatchWorkerCount::new(3, 2)],
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_batch_worker_counts(),
        &[WorkloadParallelBatchWorkerCount::new(4, 1)],
    );
    assert_eq!(
        summary.dma_scheduler_batch_worker_counts(),
        vec![
            WorkloadParallelBatchWorkerCount::new(3, 2),
            WorkloadParallelBatchWorkerCount::new(4, 1),
        ],
    );
    assert_eq!(summary.dma_scheduler_batch_count_for_worker_count(3), 2,);
    assert_eq!(summary.dma_scheduler_batch_count_for_worker_count(4), 1,);
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_counts(),
        vec![
            WorkloadParallelBatchWorkerCount::new(2, 2),
            WorkloadParallelBatchWorkerCount::new(3, 2),
            WorkloadParallelBatchWorkerCount::new(4, 1),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_worker_count(3),
        2,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_worker_count(4),
        1,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_at_or_above(3),
        3,
    );
    assert_eq!(
        summary.parallel_scheduler_batch_worker_count_tick_summaries(),
        vec![(2, 4)],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_worker_count_tick_summaries(),
        vec![(2, 4)],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_count_tick_summaries(),
        vec![(2, 8), (3, 4), (4, 5)],
    );
    assert_eq!(
        summary.parallel_scheduler_batch_ticks_for_worker_count(1),
        0
    );
    assert_eq!(
        summary.parallel_scheduler_batch_ticks_for_worker_count(2),
        4
    );
    assert_eq!(summary.parallel_scheduler_batch_ticks_at_or_above(1), 4);
    assert_eq!(summary.parallel_scheduler_batch_ticks_at_or_above(2), 4);
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_ticks_for_worker_count(2),
        4,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_ticks_at_or_above(2),
        4,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_for_worker_count(1),
        0,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_for_worker_count(2),
        8,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_for_worker_count(3),
        4,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_for_worker_count(4),
        5,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_at_or_above(1),
        17,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_at_or_above(2),
        17,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_ticks_at_or_above(3),
        9,
    );
    assert_eq!(summary.parallel_scheduler_batch_worker_ticks(), 8);
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_worker_ticks(),
        8,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_ticks(),
        48
    );
    assert_eq!(
        summary.parallel_scheduler_batch_worker_ticks_at_or_above(2),
        8,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_worker_ticks_at_or_above(2),
        8,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_ticks_at_or_above(2),
        48,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_ticks_at_or_above(3),
        32,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_ticks_at_or_above(4),
        20,
    );
    assert_eq!(
        summary.parallel_scheduler_longest_batch_tick_streak_at_or_above(1),
        4,
    );
    assert_eq!(
        summary.parallel_scheduler_longest_batch_tick_streak_at_or_above(2),
        4,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_longest_batch_tick_streak_at_or_above(2),
        4,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(1),
        11,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(2),
        11,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(3),
        9,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(4),
        5,
    );
    assert_eq!(
        summary.gpu_dma_scheduler_batch_partition_sets(),
        vec![
            WorkloadParallelBatchPartitionSet::new([cpu0, cpu1, cache], 1),
            WorkloadParallelBatchPartitionSet::new([cpu0, cache], 1),
        ],
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_batch_partition_sets(),
        vec![WorkloadParallelBatchPartitionSet::new([cpu1, cache], 1)],
    );
    assert_eq!(
        summary.dma_scheduler_batch_partition_sets(),
        vec![
            WorkloadParallelBatchPartitionSet::new([cpu0, cpu1, cache], 1),
            WorkloadParallelBatchPartitionSet::new([cpu0, cache], 1),
            WorkloadParallelBatchPartitionSet::new([cpu1, cache], 1),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_partition_sets(),
        vec![
            WorkloadParallelBatchPartitionSet::new([cpu0, cpu1], 1),
            WorkloadParallelBatchPartitionSet::new([cpu0, cpu1, cache], 1),
            WorkloadParallelBatchPartitionSet::new([cpu0, cache], 1),
            WorkloadParallelBatchPartitionSet::new([cpu1, cache], 2),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_partition_set([cpu1, cache]),
        2,
    );
    assert!(summary.has_full_system_parallel_scheduler_work());
}

#[test]
fn workload_result_counts_parallel_progress_transition_evidence() {
    let cpu_subject = wait_subject("cpu-scheduler");
    let data_cache_subject = wait_subject("data-cache-scheduler");
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([
            progress_transition(
                0,
                cpu_subject.clone(),
                LivelockTransitionKind::SchedulerEpoch,
                3,
                0,
            ),
            progress_transition(
                0,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                4,
                1,
            ),
        ])
        .with_data_cache_parallel_scheduler_progress_transitions([
            progress_transition(
                2,
                data_cache_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                5,
                0,
            ),
            progress_transition(
                2,
                data_cache_subject.clone(),
                LivelockTransitionKind::QueueRotation,
                6,
                1,
            ),
        ]);

    assert_eq!(
        summary.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::SchedulerEpoch,
        ),
        1,
    );
    assert_eq!(
        summary.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::ProtocolRetry
        ),
        1,
    );
    assert_eq!(
        summary.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::QueueRotation
        ),
        0,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        1,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::QueueRotation,
        ),
        1,
    );
    assert_eq!(
        summary
            .full_system_progress_transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        2,
    );
    assert_eq!(
        summary
            .full_system_progress_transition_count_by_kind(LivelockTransitionKind::SchedulerEpoch),
        1,
    );
    assert_eq!(
        summary
            .full_system_progress_transition_count_by_kind(LivelockTransitionKind::QueueRotation),
        1,
    );

    assert_eq!(
        summary.parallel_scheduler_progress_transition_count_by_partition(PartitionId::new(0)),
        2,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count_by_partition(
            PartitionId::new(2)
        ),
        2,
    );
    assert_eq!(
        summary.full_system_progress_transition_count_by_partition(PartitionId::new(0)),
        2,
    );
    assert_eq!(
        summary.full_system_progress_transition_count_by_partition(PartitionId::new(2)),
        2,
    );

    assert_eq!(
        summary.parallel_scheduler_progress_transition_count_by_subject(&cpu_subject),
        2,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count_by_subject(
            &data_cache_subject,
        ),
        2,
    );
    assert_eq!(
        summary.full_system_progress_transition_count_by_subject(&cpu_subject),
        2,
    );
    assert_eq!(
        summary.full_system_progress_transition_count_by_subject(&data_cache_subject),
        2,
    );
}

#[test]
fn workload_result_preserves_wait_for_edge_kind_counts() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_wait_for_edge_kind_counts([
            (WaitForEdgeKind::Protocol, 2),
            (WaitForEdgeKind::Barrier, 1),
        ])
        .with_resource_wait_for_edge_kind_counts(
            [(WaitForEdgeKind::Queue, 3)],
            [(WaitForEdgeKind::Barrier, 2), (WaitForEdgeKind::Credit, 1)],
        )
        .with_gpu_compute_wait_for_edge_kind_counts([(WaitForEdgeKind::Resource, 5)])
        .with_accelerator_compute_wait_for_edge_kind_counts([(WaitForEdgeKind::HostAction, 7)])
        .with_gpu_dma_wait_for_edge_kind_counts([(WaitForEdgeKind::Message, 11)])
        .with_accelerator_dma_wait_for_edge_kind_counts([(WaitForEdgeKind::Credit, 13)]);

    assert_eq!(
        summary.data_cache_wait_for_edge_count_by_kind(WaitForEdgeKind::Protocol),
        2,
    );
    assert_eq!(
        summary.resource_wait_for_edge_count_by_kind(WaitForEdgeKind::Barrier),
        2,
    );
    assert_eq!(
        summary.compute_wait_for_edge_count_by_kind(WaitForEdgeKind::HostAction),
        7,
    );
    assert_eq!(
        summary.dma_wait_for_edge_count_by_kind(WaitForEdgeKind::Credit),
        13,
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Credit),
        14,
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Protocol),
        2,
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 45);
    assert_eq!(
        summary
            .full_system_wait_for_edge_kind_counts()
            .get(&WaitForEdgeKind::Barrier),
        Some(&3),
    );
}

#[test]
fn workload_result_preserves_wait_for_edge_kind_tick_windows() {
    let summary =
        WorkloadParallelExecutionSummary::default()
            .with_data_cache_wait_for_edge_kind_windows([
                WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Protocol, 2, 4, 9),
                WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Barrier, 1, 7, 7),
            ])
            .with_resource_wait_for_edge_kind_windows(
                [WorkloadWaitForEdgeKindWindow::new(
                    WaitForEdgeKind::Queue,
                    3,
                    5,
                    11,
                )],
                [WorkloadWaitForEdgeKindWindow::new(
                    WaitForEdgeKind::Barrier,
                    2,
                    3,
                    13,
                )],
            )
            .with_gpu_compute_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
                WaitForEdgeKind::Resource,
                5,
                2,
                14,
            )])
            .with_accelerator_compute_wait_for_edge_kind_windows([
                WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::HostAction, 7, 6, 18),
            ])
            .with_gpu_dma_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
                WaitForEdgeKind::Message,
                11,
                8,
                21,
            )])
            .with_accelerator_dma_wait_for_edge_kind_windows([WorkloadWaitForEdgeKindWindow::new(
                WaitForEdgeKind::Credit,
                13,
                10,
                22,
            )]);

    assert_eq!(
        summary.data_cache_wait_for_edge_kind_window(WaitForEdgeKind::Protocol),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Protocol,
            2,
            4,
            9,
        )),
    );
    assert_eq!(
        summary.resource_wait_for_edge_kind_window(WaitForEdgeKind::Barrier),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            2,
            3,
            13,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_kind_window(WaitForEdgeKind::Barrier),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Barrier,
            3,
            3,
            13,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_kind_window(WaitForEdgeKind::Credit),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Credit,
            13,
            10,
            22,
        )),
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 44);
    assert_eq!(
        summary.full_system_wait_for_edge_kind_windows(),
        vec![
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Resource, 5, 2, 14),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Message, 11, 8, 21),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Protocol, 2, 4, 9),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Queue, 3, 5, 11),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Credit, 13, 10, 22),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::HostAction, 7, 6, 18),
            WorkloadWaitForEdgeKindWindow::new(WaitForEdgeKind::Barrier, 3, 3, 13),
        ],
    );
}

#[test]
fn workload_result_preserves_wait_for_target_node_tick_windows() {
    let cache_mshr = wait_resource("cache.mshr");
    let fabric_credit = wait_resource("fabric.credit");
    let dram_bank = wait_resource("dram.bank");
    let gpu_queue = wait_resource("gpu.queue");
    let accelerator_mailbox = wait_resource("accelerator.mailbox");
    let gpu_dma_packet = wait_resource("gpu.dma.packet");
    let accelerator_dma_credit = wait_resource("accelerator.dma.credit");
    let summary =
        WorkloadParallelExecutionSummary::default()
            .with_data_cache_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
                cache_mshr.clone(),
                2,
                4,
                9,
            )])
            .with_resource_wait_for_target_node_windows(
                [WorkloadWaitForTargetNodeWindow::new(
                    fabric_credit.clone(),
                    3,
                    5,
                    11,
                )],
                [WorkloadWaitForTargetNodeWindow::new(
                    dram_bank.clone(),
                    2,
                    3,
                    13,
                )],
            )
            .with_gpu_compute_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
                gpu_queue.clone(),
                5,
                2,
                14,
            )])
            .with_accelerator_compute_wait_for_target_node_windows([
                WorkloadWaitForTargetNodeWindow::new(accelerator_mailbox.clone(), 7, 6, 18),
            ])
            .with_gpu_dma_wait_for_target_node_windows([WorkloadWaitForTargetNodeWindow::new(
                gpu_dma_packet.clone(),
                11,
                8,
                21,
            )])
            .with_accelerator_dma_wait_for_target_node_windows([
                WorkloadWaitForTargetNodeWindow::new(accelerator_dma_credit.clone(), 13, 10, 22),
            ]);

    assert_eq!(
        summary.data_cache_wait_for_target_node_window(&cache_mshr),
        Some(WorkloadWaitForTargetNodeWindow::new(
            cache_mshr.clone(),
            2,
            4,
            9,
        )),
    );
    assert_eq!(
        summary.resource_wait_for_target_node_window(&dram_bank),
        Some(WorkloadWaitForTargetNodeWindow::new(
            dram_bank.clone(),
            2,
            3,
            13,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_target_node_window(&accelerator_dma_credit),
        Some(WorkloadWaitForTargetNodeWindow::new(
            accelerator_dma_credit.clone(),
            13,
            10,
            22,
        )),
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 43);
    assert_eq!(
        summary.full_system_wait_for_target_node_windows(),
        vec![
            WorkloadWaitForTargetNodeWindow::new(accelerator_dma_credit, 13, 10, 22),
            WorkloadWaitForTargetNodeWindow::new(accelerator_mailbox, 7, 6, 18),
            WorkloadWaitForTargetNodeWindow::new(cache_mshr, 2, 4, 9),
            WorkloadWaitForTargetNodeWindow::new(dram_bank, 2, 3, 13),
            WorkloadWaitForTargetNodeWindow::new(fabric_credit, 3, 5, 11),
            WorkloadWaitForTargetNodeWindow::new(gpu_dma_packet, 11, 8, 21),
            WorkloadWaitForTargetNodeWindow::new(gpu_queue, 5, 2, 14),
        ],
    );
}

#[test]
fn workload_result_batch_counts_use_stronger_batch_evidence_than_aggregate_counts() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_sets([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 3),
        ])
        .with_scheduler_counts(1, 0, 1, 1)
        .with_data_cache_parallel_scheduler_batch_worker_counts([
            WorkloadParallelBatchWorkerCount::new(2, 4),
            WorkloadParallelBatchWorkerCount::new(3, 1),
        ])
        .with_data_cache_parallel_counts(1, 1, 1, 2, 1);

    assert_eq!(summary.scheduler_batch_count(), 5);
    assert_eq!(summary.data_cache_parallel_scheduler_batch_count(), 5);
    assert_eq!(summary.full_system_parallel_scheduler_batch_count(), 10);
}

#[test]
fn workload_result_marks_typed_parallel_evidence_as_work() {
    let scheduler_flow =
        WorkloadParallelExecutionSummary::default().with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(1), 2, 3, 11),
        ]);
    assert!(scheduler_flow.has_parallel_scheduler_work());
    assert!(!scheduler_flow.has_data_cache_parallel_work());
    assert!(scheduler_flow.has_full_system_parallel_scheduler_work());

    let scheduler_send = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(0),
            PartitionId::new(1),
            3,
            11,
            0,
        )]);
    assert!(scheduler_send.has_parallel_scheduler_work());
    assert!(!scheduler_send.has_data_cache_parallel_work());
    assert!(scheduler_send.has_full_system_parallel_scheduler_work());

    let data_cache_frontier = WorkloadParallelExecutionSummary::default()
        .with_data_cache_parallel_scheduler_frontiers(
            [PartitionFrontier::new(
                PartitionId::new(2),
                5,
                13,
                Some(8),
                1,
            )],
            [PartitionFrontier::new(PartitionId::new(2), 13, 21, None, 0)],
        );
    assert!(!data_cache_frontier.has_parallel_scheduler_work());
    assert!(data_cache_frontier.has_data_cache_parallel_work());
    assert!(data_cache_frontier.has_full_system_parallel_scheduler_work());

    let active_partitions =
        WorkloadParallelExecutionSummary::default().with_full_system_parallel_partitions(3);
    assert!(active_partitions.has_full_system_parallel_scheduler_work());
}

#[test]
fn workload_result_ignores_local_remote_traffic_as_parallel_evidence() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([ParallelRemoteFlowRecord::new(
            PartitionId::new(0),
            PartitionId::new(0),
            2,
            3,
            7,
        )])
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(1),
            PartitionId::new(1),
            5,
            11,
            0,
        )]);

    assert!(summary.parallel_scheduler_remote_flow_evidence().is_empty());
    assert_eq!(
        summary.parallel_scheduler_remote_send_count(PartitionId::new(1), PartitionId::new(1)),
        0,
    );
    assert!(summary
        .parallel_scheduler_remote_source_partitions()
        .is_empty());
    assert!(summary
        .parallel_scheduler_remote_target_partitions()
        .is_empty());
    assert_eq!(summary.active_scheduler_partition_count(), 0);
    assert!(!summary.has_parallel_scheduler_remote_flows());
    assert!(!summary.has_parallel_scheduler_remote_sends());
    assert!(!summary.has_parallel_scheduler_work());
    assert!(!summary.has_full_system_parallel_scheduler_work());
}

#[test]
fn workload_result_reports_accelerator_command_kind_counts() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_accelerator_compute_counts(4, 9, 3, 1)
        .with_accelerator_command_kind_counts(1, 2, 1)
        .with_accelerator_completion_kind_counts(1, 2, 0);

    assert_eq!(summary.accelerator_gpu_kernel_command_count(), 1);
    assert_eq!(summary.accelerator_npu_inference_command_count(), 2);
    assert_eq!(summary.accelerator_dma_command_count(), 1);
    assert_eq!(summary.accelerator_gpu_kernel_completion_count(), 1);
    assert_eq!(summary.accelerator_npu_inference_completion_count(), 2);
    assert_eq!(summary.accelerator_dma_command_completion_count(), 0);
    assert!(summary.has_accelerator_npu_activity());
    assert!(summary.has_accelerator_compute_activity());
}

#[test]
fn workload_result_full_system_frontiers_merge_partitions_conservatively() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_frontiers(
            [
                PartitionFrontier::new(PartitionId::new(0), 8, 16, Some(12), 1),
                PartitionFrontier::new(PartitionId::new(1), 4, 12, None, 0),
            ],
            [
                PartitionFrontier::new(PartitionId::new(0), 16, 24, None, 0),
                PartitionFrontier::new(PartitionId::new(1), 12, 20, Some(18), 2),
            ],
        )
        .with_data_cache_parallel_scheduler_frontiers(
            [
                PartitionFrontier::new(PartitionId::new(0), 6, 14, Some(10), 3),
                PartitionFrontier::new(PartitionId::new(2), 3, 9, Some(7), 1),
            ],
            [
                PartitionFrontier::new(PartitionId::new(0), 15, 22, Some(21), 4),
                PartitionFrontier::new(PartitionId::new(2), 9, 17, None, 0),
            ],
        );

    assert_eq!(
        summary.full_system_parallel_scheduler_initial_frontiers(),
        vec![
            PartitionFrontier::new(PartitionId::new(0), 6, 14, Some(10), 3),
            PartitionFrontier::new(PartitionId::new(1), 4, 12, None, 0),
            PartitionFrontier::new(PartitionId::new(2), 3, 9, Some(7), 1),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_final_frontiers(),
        vec![
            PartitionFrontier::new(PartitionId::new(0), 15, 22, Some(21), 4),
            PartitionFrontier::new(PartitionId::new(1), 12, 20, Some(18), 2),
            PartitionFrontier::new(PartitionId::new(2), 9, 17, None, 0),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_initial_frontier_count(),
        3
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_final_frontier_count(),
        3
    );
}

#[test]
fn workload_result_records_parallel_batch_partition_streaks_from_ordered_batches() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(1), PartitionId::new(0)], 1),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 1),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 1),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(2)], 2),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(1),
                    PartitionId::new(2),
                    PartitionId::new(3),
                ],
                1,
            ),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(3),
                    PartitionId::new(2),
                    PartitionId::new(1),
                ],
                3,
            ),
        ]);

    assert_eq!(
        summary.parallel_scheduler_batch_partition_streaks(),
        &[
            WorkloadParallelBatchPartitionStreak::new(
                [PartitionId::new(0), PartitionId::new(1)],
                3,
            ),
            WorkloadParallelBatchPartitionStreak::new(
                [PartitionId::new(0), PartitionId::new(2)],
                1,
            ),
        ],
    );
    assert_eq!(
        summary.parallel_scheduler_max_consecutive_batch_count_for_partition_set([
            PartitionId::new(0),
            PartitionId::new(1),
        ]),
        3,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_partition_streaks(),
        &[
            WorkloadParallelBatchPartitionStreak::new(
                [PartitionId::new(0), PartitionId::new(2)],
                2,
            ),
            WorkloadParallelBatchPartitionStreak::new(
                [
                    PartitionId::new(1),
                    PartitionId::new(2),
                    PartitionId::new(3)
                ],
                4,
            ),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_partition_streaks(),
        vec![
            WorkloadParallelBatchPartitionStreak::new(
                [PartitionId::new(0), PartitionId::new(1)],
                3,
            ),
            WorkloadParallelBatchPartitionStreak::new(
                [PartitionId::new(0), PartitionId::new(2)],
                2,
            ),
            WorkloadParallelBatchPartitionStreak::new(
                [
                    PartitionId::new(1),
                    PartitionId::new(2),
                    PartitionId::new(3)
                ],
                4,
            ),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([
            PartitionId::new(0),
            PartitionId::new(2),
        ]),
        2,
    );
}

#[test]
fn workload_result_records_explicit_full_system_parallel_batch_partition_streaks() {
    let cpu = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([cpu, cache], 1),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([cpu, cache], 1),
        ])
        .with_full_system_parallel_scheduler_batch_partition_streaks([
            WorkloadParallelBatchPartitionStreak::new([cache, cpu], 2),
        ]);

    assert_eq!(
        summary.full_system_parallel_scheduler_batch_partition_streaks(),
        vec![WorkloadParallelBatchPartitionStreak::new([cpu, cache], 2)],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([
            cpu, cache,
        ]),
        2,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_partition_set([cpu, cache]),
        2,
    );
}

#[test]
fn workload_result_reports_remote_endpoint_partitions() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(0), PartitionId::new(2), 2, 3, 7),
            ParallelRemoteFlowRecord::new(PartitionId::new(1), PartitionId::new(2), 1, 4, 8),
        ])
        .with_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(1),
            PartitionId::new(3),
            5,
            11,
            0,
        )])
        .with_data_cache_parallel_scheduler_remote_flows([
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(2), 3, 13, 19),
            ParallelRemoteFlowRecord::new(PartitionId::new(4), PartitionId::new(5), 1, 17, 23),
        ])
        .with_data_cache_parallel_scheduler_remote_sends([ParallelRemoteSendRecord::with_timing(
            PartitionId::new(6),
            PartitionId::new(5),
            19,
            29,
            0,
        )]);

    assert_eq!(
        summary.parallel_scheduler_remote_source_partitions(),
        vec![PartitionId::new(0), PartitionId::new(1)],
    );
    assert_eq!(
        summary.parallel_scheduler_remote_source_partition_count(),
        2
    );
    assert_eq!(
        summary.parallel_scheduler_remote_target_partitions(),
        vec![PartitionId::new(2), PartitionId::new(3)],
    );
    assert_eq!(
        summary.parallel_scheduler_remote_target_partition_count(),
        2
    );

    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_source_partitions(),
        vec![PartitionId::new(4), PartitionId::new(6)],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_source_partition_count(),
        2,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_target_partitions(),
        vec![PartitionId::new(2), PartitionId::new(5)],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_remote_target_partition_count(),
        2,
    );

    assert_eq!(
        summary.full_system_parallel_scheduler_remote_source_partitions(),
        vec![
            PartitionId::new(0),
            PartitionId::new(1),
            PartitionId::new(4),
            PartitionId::new(6),
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_source_partition_count(),
        4,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_target_partitions(),
        vec![
            PartitionId::new(2),
            PartitionId::new(3),
            PartitionId::new(5)
        ],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_target_partition_count(),
        3,
    );
}

#[test]
fn workload_result_derives_parallel_activity_from_batch_partition_streaks() {
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(0), PartitionId::new(1)], 2),
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(1), PartitionId::new(0)], 1),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(2),
                    PartitionId::new(3),
                    PartitionId::new(4),
                ],
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_batch_partition_streak_sequence([
            WorkloadParallelBatchPartitionSet::new([PartitionId::new(10), PartitionId::new(11)], 4),
            WorkloadParallelBatchPartitionSet::new(
                [
                    PartitionId::new(11),
                    PartitionId::new(12),
                    PartitionId::new(13),
                ],
                1,
            ),
        ]);

    assert_eq!(summary.scheduler_batch_count(), 5);
    assert_eq!(summary.scheduler_dispatch_count(), 12);
    assert_eq!(summary.max_parallel_scheduler_workers(), 3);
    assert_eq!(summary.total_parallel_scheduler_workers(), 12);
    assert_eq!(summary.active_scheduler_partition_count(), 5);
    assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(2), 5);
    assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(3), 2);
    assert_eq!(
        summary.parallel_scheduler_batch_count_for_worker_count(2),
        3
    );
    assert_eq!(
        summary.parallel_scheduler_batch_count_for_worker_count(3),
        2
    );
    assert_eq!(
        summary.parallel_scheduler_batch_count_for_worker_count(4),
        0
    );
    summary
        .verify_minimum_parallel_batch_count_for_worker_count(
            WorkloadParallelRemoteFlowScope::Scheduler,
            2,
            3,
        )
        .unwrap();
    assert_eq!(
        summary.parallel_scheduler_partition_activity(PartitionId::new(0)),
        Some(ParallelPartitionActivity::with_remote_counts(3, 3, 0, 0, 0)),
    );

    assert_eq!(summary.data_cache_parallel_scheduler_batch_count(), 5);
    assert_eq!(summary.data_cache_parallel_scheduler_dispatch_count(), 11);
    assert_eq!(summary.data_cache_parallel_scheduler_max_workers(), 3);
    assert_eq!(summary.data_cache_parallel_scheduler_total_workers(), 11);
    assert_eq!(
        summary.active_data_cache_parallel_scheduler_partition_count(),
        4,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_at_or_above(3),
        1,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_for_worker_count(2),
        4,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_batch_count_for_worker_count(3),
        1,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_partition_activity(PartitionId::new(11)),
        Some(ParallelPartitionActivity::with_remote_counts(5, 5, 0, 0, 0)),
    );

    assert_eq!(summary.full_system_parallel_scheduler_batch_count(), 10);
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 23);
    assert_eq!(summary.full_system_parallel_scheduler_max_workers(), 3);
    assert_eq!(summary.full_system_parallel_scheduler_total_workers(), 23);
    assert_eq!(
        summary.active_full_system_parallel_scheduler_partition_count(),
        9,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_at_or_above(3),
        3,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_worker_count(2),
        7,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_worker_count(3),
        3,
    );
    let exact_bucket_error = summary
        .verify_minimum_parallel_batch_count_for_worker_count(
            WorkloadParallelRemoteFlowScope::FullSystem,
            3,
            4,
        )
        .unwrap_err();
    assert_eq!(
        exact_bucket_error,
        WorkloadError::ParallelBatchWorkerCountBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            worker_count: 3,
            minimum_batch_count: 4,
            actual_batch_count: 3,
        },
    );
}
