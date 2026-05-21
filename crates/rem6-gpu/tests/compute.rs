use rem6_gpu::{
    GpuComputeConfig, GpuDevice, GpuDeviceId, GpuDeviceSnapshot, GpuError, GpuKernelId,
    GpuKernelLaunch, GpuQueuedWorkgroupSnapshot, GpuSlotSnapshot, GpuTraceEvent, GpuTraceKind,
    GpuWorkgroupCompletion, GpuWorkgroupId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};

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
    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.final_tick(), 16);
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
        GpuError::Scheduler(SchedulerError::RemoteDelayBelowLookahead {
            source: cpu_partition,
            target: gpu_partition,
            delay: 2,
            minimum: 3,
        }),
    );
    assert_eq!(scheduler.now(), 0);
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
    let queued =
        GpuQueuedWorkgroupSnapshot::new(GpuKernelId::new(33), GpuWorkgroupId::new(4), 2, 1, 13, 21);
    assert_eq!(queued.kernel(), GpuKernelId::new(33));
    assert_eq!(queued.workgroup(), GpuWorkgroupId::new(4));
    assert_eq!(queued.compute_unit(), 2);
    assert_eq!(queued.slot(), 1);
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
