use rem6_gpu::{
    GpuComputeConfig, GpuDevice, GpuDeviceId, GpuDeviceSnapshot, GpuDmaCompletion, GpuDmaCopy,
    GpuDmaId, GpuError, GpuKernelId, GpuKernelLaunch, GpuPendingDmaWrite,
    GpuQueuedWorkgroupSnapshot, GpuSlotSnapshot, GpuTraceEvent, GpuTraceKind,
    GpuWorkgroupCompletion, GpuWorkgroupId,
};
use rem6_kernel::{
    ParallelRunProfile, PartitionId, PartitionedScheduler, SchedulerError, WaitForEdgeKind,
    WaitForNode,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, LineMemoryStore, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryRequest, MemoryRequestId,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome, TransportEndpointId,
};
use std::sync::{Arc, Mutex};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

#[test]
fn gpu_launch_runs_workgroups_on_compute_units_deterministically() {
    let cpu_partition = PartitionId::new(0);
    let gpu_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(3), gpu_partition, 2, 1).unwrap());
    let launch = GpuKernelLaunch::new(GpuKernelId::new(10), 5, 4).unwrap();

    gpu.submit_kernel_from_partition(&mut scheduler, cpu_partition, 3, launch.clone())
        .unwrap();
    let summary = gpu
        .run_until_idle_parallel_recorded(&mut scheduler)
        .unwrap();

    assert_eq!(summary.final_tick(), 16);
    assert_eq!(summary.workgroup_completion_count(), 5);
    assert_eq!(summary.dma_completion_count(), 0);
    assert_eq!(summary.pending_dma_write_count(), 0);
    assert_eq!(summary.trace_event_count(), 12);
    assert_eq!(summary.scheduler_run().profile(), summary.profile());
    assert_eq!(summary.profile().dispatch_count(), summary.dispatch_count());
    assert_eq!(
        summary.profile(),
        ParallelRunProfile::new(
            summary.epoch_count(),
            summary.empty_epoch_count(),
            summary.batch_count(),
            summary.dispatch_count(),
            summary.total_parallel_workers(),
            summary.max_parallel_workers(),
        )
    );
    assert!(summary.has_device_activity());
    assert!(summary.has_compute_activity());
    assert!(!summary.has_dma_activity());
    let gpu_activity = summary.partition_activity(gpu_partition).unwrap();
    assert!(gpu_activity.worker_count() >= 1);
    assert!(gpu_activity.dispatch_count() >= 1);
    assert!(gpu_activity.max_pending_events() >= 1);
    assert!(summary.has_partition_activity(gpu_partition));
    assert!(!summary.has_partition_activity(PartitionId::new(3)));
    assert!(summary.active_partition_count() >= 1);
    assert_eq!(
        gpu.completions(),
        vec![
            GpuWorkgroupCompletion::new(GpuKernelId::new(10), GpuWorkgroupId::new(0), 0, 0, 3, 7,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(10), GpuWorkgroupId::new(1), 1, 0, 3, 7,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(10), GpuWorkgroupId::new(2), 0, 0, 7, 11,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(10), GpuWorkgroupId::new(3), 1, 0, 7, 11,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(10), GpuWorkgroupId::new(4), 0, 0, 11, 15,),
        ],
    );
    assert_eq!(
        gpu.trace(),
        vec![
            GpuTraceEvent::new(
                0,
                GpuTraceKind::LaunchSubmitted {
                    kernel: GpuKernelId::new(10),
                    source: cpu_partition,
                    target: gpu_partition,
                },
            ),
            GpuTraceEvent::new(
                3,
                GpuTraceKind::LaunchAccepted {
                    kernel: GpuKernelId::new(10),
                    workgroups: 5,
                },
            ),
            GpuTraceEvent::new(
                3,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(0),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 7,
                },
            ),
            GpuTraceEvent::new(
                3,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(1),
                    compute_unit: 1,
                    slot: 0,
                    complete_at: 7,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(0),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(1),
                    compute_unit: 1,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(2),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 11,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(3),
                    compute_unit: 1,
                    slot: 0,
                    complete_at: 11,
                },
            ),
            GpuTraceEvent::new(
                11,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(2),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                11,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(3),
                    compute_unit: 1,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                11,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(4),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 15,
                },
            ),
            GpuTraceEvent::new(
                15,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(10),
                    workgroup: GpuWorkgroupId::new(4),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
        ],
    );
}

#[test]
fn gpu_wait_for_graph_tracks_queued_workgroups_until_slot_starts() {
    let cpu_partition = PartitionId::new(0);
    let gpu_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(8), gpu_partition, 1, 1).unwrap());
    let launch = GpuKernelLaunch::new(GpuKernelId::new(40), 3, 4).unwrap();
    let marker = gpu.mark_wait_for();

    gpu.submit_kernel_from_partition(&mut scheduler, cpu_partition, 2, launch)
        .unwrap();
    let first_epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(first_epoch.summary().final_tick(), 2);
    assert_eq!(gpu.snapshot().slots()[0].queued()[0].queued_at(), 2);
    let slot = WaitForNode::resource("gpu.8.cu.0.slot.0").unwrap();
    let second_workgroup = WaitForNode::transaction("gpu.8.kernel.40.wg.1").unwrap();
    let third_workgroup = WaitForNode::transaction("gpu.8.kernel.40.wg.2").unwrap();
    let wait_for = gpu.wait_for_graph().snapshot();

    assert_eq!(wait_for.edge_count(), 2);
    assert_eq!(wait_for.first_observed_tick(), Some(2));
    assert_eq!(wait_for.last_observed_tick(), Some(2));
    assert!(wait_for.contains_edge(&second_workgroup, &slot, WaitForEdgeKind::Queue));
    assert!(wait_for.contains_edge(&third_workgroup, &slot, WaitForEdgeKind::Queue));
    assert_eq!(
        wait_for.dependencies(&second_workgroup)[0].observation_count(),
        1
    );

    while scheduler.now() < 6 {
        scheduler.run_next_epoch_parallel_recorded().unwrap();
    }
    let after_second_start = gpu.wait_for_graph().snapshot();
    assert_eq!(after_second_start.edge_count(), 1);
    assert!(!after_second_start.contains_edge(&second_workgroup, &slot, WaitForEdgeKind::Queue));
    assert!(after_second_start.contains_edge(&third_workgroup, &slot, WaitForEdgeKind::Queue));

    scheduler.run_until_idle_parallel_recorded().unwrap();
    let history = gpu.wait_for_graph_since(marker).snapshot();
    assert_eq!(history.edge_count(), 2);
    assert_eq!(history.first_observed_tick(), Some(2));
    assert_eq!(history.last_observed_tick(), Some(9));
    assert!(history.contains_edge(&second_workgroup, &slot, WaitForEdgeKind::Queue));
    assert!(history.contains_edge(&third_workgroup, &slot, WaitForEdgeKind::Queue));
    assert!(gpu.wait_for_graph().is_empty());
}

#[test]
fn gpu_launch_rejects_invalid_config_and_submission_before_enqueueing() {
    let gpu_partition = PartitionId::new(1);
    assert_eq!(
        GpuComputeConfig::new(GpuDeviceId::new(4), gpu_partition, 0, 1).unwrap_err(),
        GpuError::ZeroComputeUnits {
            device: GpuDeviceId::new(4),
        },
    );
    assert_eq!(
        GpuComputeConfig::new(GpuDeviceId::new(4), gpu_partition, 1, 0).unwrap_err(),
        GpuError::ZeroWaveSlots {
            device: GpuDeviceId::new(4),
        },
    );
    assert_eq!(
        GpuKernelLaunch::new(GpuKernelId::new(11), 0, 4).unwrap_err(),
        GpuError::ZeroWorkgroups {
            kernel: GpuKernelId::new(11),
        },
    );
    assert_eq!(
        GpuKernelLaunch::new(GpuKernelId::new(11), 1, 0).unwrap_err(),
        GpuError::ZeroWorkgroupLatency {
            kernel: GpuKernelId::new(11),
        },
    );

    let cpu_partition = PartitionId::new(0);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();
    scheduler
        .schedule_parallel_at(cpu_partition, 7, |_| {})
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    let source_tick = scheduler.partition_now(cpu_partition).unwrap();
    let delivery_tick = source_tick.checked_add(2).unwrap();
    let minimum_delivery_tick = source_tick.checked_add(3).unwrap();
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(5), gpu_partition, 1, 1).unwrap());
    let error = gpu
        .submit_kernel_from_partition(
            &mut scheduler,
            cpu_partition,
            2,
            GpuKernelLaunch::new(GpuKernelId::new(12), 1, 2).unwrap(),
        )
        .unwrap_err();

    assert_eq!(
        error,
        GpuError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: cpu_partition,
            target: gpu_partition,
            source_tick,
            delivery_tick,
            minimum_delivery_tick,
        }),
    );
    assert_eq!(scheduler.now(), source_tick);
    assert!(gpu.trace().is_empty());
    assert!(gpu.completions().is_empty());
}

#[test]
fn gpu_device_restores_snapshot_state_and_slot_reservations() {
    let cpu_partition = PartitionId::new(0);
    let gpu_partition = PartitionId::new(1);
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(6), gpu_partition, 2, 1).unwrap());
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    gpu.submit_kernel_from_partition(
        &mut scheduler,
        cpu_partition,
        2,
        GpuKernelLaunch::new(GpuKernelId::new(30), 3, 4).unwrap(),
    )
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    let snapshot = gpu.snapshot();
    assert_eq!(snapshot.slot_count(), 2);
    assert!(!snapshot.has_queued_workgroups());
    assert!(!snapshot.has_pending_dma_writes());
    assert!(snapshot.slots().iter().all(GpuSlotSnapshot::is_idle));
    let rebuilt_slots = snapshot
        .slots()
        .iter()
        .map(|slot| {
            GpuSlotSnapshot::new(
                slot.available_at(),
                slot.pump_scheduled(),
                slot.queued().to_vec(),
            )
        })
        .collect();
    let rebuilt = GpuDeviceSnapshot::new(
        rebuilt_slots,
        snapshot.trace().to_vec(),
        snapshot.completions().to_vec(),
        snapshot.pending_dma_writes().to_vec(),
        snapshot.dma_completions().to_vec(),
    );
    assert_eq!(rebuilt, snapshot);
    let queued = GpuQueuedWorkgroupSnapshot::new(
        GpuKernelId::new(33),
        GpuWorkgroupId::new(4),
        2,
        1,
        11,
        13,
        21,
    );
    assert_eq!(queued.kernel(), GpuKernelId::new(33));
    assert_eq!(queued.workgroup(), GpuWorkgroupId::new(4));
    assert_eq!(queued.compute_unit(), 2);
    assert_eq!(queued.slot(), 1);
    assert_eq!(queued.queued_at(), 11);
    assert_eq!(queued.started_at(), 13);
    assert_eq!(queued.completed_at(), 21);

    let mut mutation_scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    gpu.submit_kernel_from_partition(
        &mut mutation_scheduler,
        cpu_partition,
        2,
        GpuKernelLaunch::new(GpuKernelId::new(31), 2, 3).unwrap(),
    )
    .unwrap();
    mutation_scheduler.run_until_idle_parallel().unwrap();
    assert_ne!(gpu.snapshot(), snapshot);

    gpu.restore(&snapshot);
    assert_eq!(gpu.snapshot(), snapshot);

    let mut restored_scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    gpu.submit_kernel_from_partition(
        &mut restored_scheduler,
        cpu_partition,
        2,
        GpuKernelLaunch::new(GpuKernelId::new(32), 1, 5).unwrap(),
    )
    .unwrap();
    restored_scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        gpu.completions(),
        vec![
            GpuWorkgroupCompletion::new(GpuKernelId::new(30), GpuWorkgroupId::new(0), 0, 0, 2, 6,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(30), GpuWorkgroupId::new(1), 1, 0, 2, 6,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(30), GpuWorkgroupId::new(2), 0, 0, 6, 10,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(32), GpuWorkgroupId::new(0), 1, 0, 6, 11,),
        ],
    );
}

#[test]
fn gpu_dma_copy_reports_recorded_parallel_activity() {
    let gpu_partition = PartitionId::new(0);
    let memory_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let gpu_endpoint = endpoint("gpu0.dma");
    let memory_endpoint = endpoint("memory0.port");
    let route = transport
        .add_route(
            MemoryRoute::new(
                gpu_endpoint,
                gpu_partition,
                memory_endpoint,
                memory_partition,
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(8), gpu_partition, 1, 1).unwrap());
    let mut store = LineMemoryStore::new(line_layout());
    let mut source_line = vec![0; 64];
    source_line[8..12].copy_from_slice(&[0x10, 0x20, 0x30, 0x40]);
    store
        .insert_line(Address::new(0x1000), source_line)
        .unwrap();
    store
        .insert_line(Address::new(0x2000), vec![0; 64])
        .unwrap();
    let store = Arc::new(Mutex::new(store));
    let read_request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(8), 1),
        Address::new(0x1008),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let copy = GpuDmaCopy::new(
        GpuDmaId::new(30),
        route,
        read_request.clone(),
        route,
        MemoryRequestId::new(AgentId::new(8), 2),
        Address::new(0x2004),
    )
    .unwrap();

    let read_store = Arc::clone(&store);
    gpu.submit_dma_copy_read(
        &mut scheduler,
        &transport,
        copy.clone(),
        trace.clone(),
        move |delivery: RequestDelivery, _context| {
            let response = read_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap();
    let read_run = gpu
        .run_until_idle_parallel_recorded(&mut scheduler)
        .unwrap();

    assert_eq!(read_run.dma_completion_count(), 0);
    assert_eq!(read_run.pending_dma_write_count(), 1);
    assert_eq!(read_run.trace_event_count(), 1);
    assert!(read_run.has_dma_activity());
    assert!(!read_run.has_compute_activity());
    assert_eq!(
        gpu.pending_dma_writes(),
        vec![GpuPendingDmaWrite::new(
            copy.clone(),
            vec![0x10, 0x20, 0x30, 0x40],
            4,
        )]
    );

    let write_store = Arc::clone(&store);
    assert!(gpu
        .issue_next_dma_write(
            &mut scheduler,
            &transport,
            trace.clone(),
            move |delivery: RequestDelivery, _context| {
                let response = write_store
                    .lock()
                    .unwrap()
                    .respond(delivery.request())
                    .unwrap()
                    .unwrap();
                TargetOutcome::Respond(response)
            },
        )
        .unwrap()
        .is_some());
    let write_run = gpu
        .run_until_idle_parallel_recorded(&mut scheduler)
        .unwrap();

    let destination = store
        .lock()
        .unwrap()
        .line_data(Address::new(0x2000))
        .unwrap();
    assert_eq!(&destination[4..8], &[0x10, 0x20, 0x30, 0x40]);
    assert!(gpu.pending_dma_writes().is_empty());
    assert_eq!(write_run.dma_completion_count(), 1);
    assert_eq!(write_run.pending_dma_write_count(), 0);
    assert_eq!(write_run.trace_event_count(), 1);
    assert!(write_run.has_dma_activity());
    assert!(!write_run.has_compute_activity());
    assert_eq!(
        gpu.dma_completions(),
        vec![GpuDmaCompletion::new(
            GpuDmaId::new(30),
            read_request.id(),
            MemoryRequestId::new(AgentId::new(8), 2),
            4,
            8,
        )]
    );
}

#[test]
fn gpu_dma_write_preserves_read_request_ordering() {
    let gpu_partition = PartitionId::new(0);
    let memory_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("gpu0.dma"),
                gpu_partition,
                endpoint("memory0.port"),
                memory_partition,
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(8), gpu_partition, 1, 1).unwrap());
    let ordering = MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        Some(MemoryBarrierSet::new(false, true)),
    );
    let read_request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(8), 11),
        Address::new(0x1008),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap()
    .with_ordering(ordering);
    let copy = GpuDmaCopy::new(
        GpuDmaId::new(31),
        route,
        read_request,
        route,
        MemoryRequestId::new(AgentId::new(8), 12),
        Address::new(0x2004),
    )
    .unwrap();
    gpu.restore(&GpuDeviceSnapshot::new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![GpuPendingDmaWrite::new(
            copy,
            vec![0x10, 0x20, 0x30, 0x40],
            4,
        )],
        Vec::new(),
    ));
    let observed = Arc::new(Mutex::new(None));
    let write_observed = Arc::clone(&observed);

    assert!(gpu
        .issue_next_dma_write(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            move |delivery: RequestDelivery, _context| {
                *write_observed.lock().unwrap() = Some(delivery.request().ordering());
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .is_some());
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(*observed.lock().unwrap(), Some(ordering));
}
