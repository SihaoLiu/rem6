use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorCompletion,
    AcceleratorEngine, AcceleratorEngineConfig, AcceleratorEngineId, AcceleratorError,
    AcceleratorTraceEvent, AcceleratorTraceKind,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};

#[test]
fn accelerator_engine_dispatches_remote_commands_on_parallel_scheduler_partition() {
    let cpu_partition = PartitionId::new(0);
    let accelerator_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let engine = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(7), accelerator_partition, 2)
            .unwrap(),
    );
    let kernel = AcceleratorCommand::new(
        AcceleratorCommandId::new(10),
        AcceleratorCommandKind::GpuKernel { workgroups: 4 },
        5,
    )
    .unwrap();
    let inference = AcceleratorCommand::new(
        AcceleratorCommandId::new(11),
        AcceleratorCommandKind::NpuInference { tiles: 8 },
        3,
    )
    .unwrap();

    engine
        .submit_from_partition(&mut scheduler, cpu_partition, 2, kernel)
        .unwrap();
    engine
        .submit_from_partition(&mut scheduler, cpu_partition, 2, inference)
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 6);
    assert_eq!(summary.final_tick(), 8);
    assert_eq!(
        engine.trace(),
        vec![
            AcceleratorTraceEvent::new(
                0,
                AcceleratorTraceKind::Submitted {
                    command: AcceleratorCommandId::new(10),
                    source: cpu_partition,
                    target: accelerator_partition,
                },
            ),
            AcceleratorTraceEvent::new(
                0,
                AcceleratorTraceKind::Submitted {
                    command: AcceleratorCommandId::new(11),
                    source: cpu_partition,
                    target: accelerator_partition,
                },
            ),
            AcceleratorTraceEvent::new(
                2,
                AcceleratorTraceKind::Started {
                    command: AcceleratorCommandId::new(10),
                    lane: 0,
                    complete_at: 7,
                },
            ),
            AcceleratorTraceEvent::new(
                2,
                AcceleratorTraceKind::Started {
                    command: AcceleratorCommandId::new(11),
                    lane: 1,
                    complete_at: 5,
                },
            ),
            AcceleratorTraceEvent::new(
                5,
                AcceleratorTraceKind::Completed {
                    command: AcceleratorCommandId::new(11),
                    lane: 1,
                },
            ),
            AcceleratorTraceEvent::new(
                7,
                AcceleratorTraceKind::Completed {
                    command: AcceleratorCommandId::new(10),
                    lane: 0,
                },
            ),
        ],
    );
    assert_eq!(
        engine.completed(),
        vec![
            AcceleratorCompletion::new(
                AcceleratorCommandId::new(11),
                AcceleratorCommandKind::NpuInference { tiles: 8 },
                1,
                2,
                5,
            ),
            AcceleratorCompletion::new(
                AcceleratorCommandId::new(10),
                AcceleratorCommandKind::GpuKernel { workgroups: 4 },
                0,
                2,
                7,
            ),
        ],
    );
}

#[test]
fn accelerator_engine_rejects_invalid_parallel_submission_before_enqueuing() {
    let cpu_partition = PartitionId::new(0);
    let accelerator_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 4).unwrap();
    let engine = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(3), accelerator_partition, 1)
            .unwrap(),
    );
    let command = AcceleratorCommand::new(
        AcceleratorCommandId::new(21),
        AcceleratorCommandKind::DmaCopy { bytes: 64 },
        2,
    )
    .unwrap();

    assert_eq!(
        engine.submit_from_partition(&mut scheduler, cpu_partition, 2, command),
        Err(AcceleratorError::Scheduler(
            SchedulerError::RemoteDelayBelowLookahead {
                source: cpu_partition,
                target: accelerator_partition,
                delay: 2,
                minimum: 4,
            },
        )),
    );
    assert!(scheduler.is_idle());
    assert!(engine.trace().is_empty());
}

#[test]
fn accelerator_engine_rejects_zero_lanes_and_zero_latency_commands() {
    assert_eq!(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(1), PartitionId::new(0), 0),
        Err(AcceleratorError::ZeroLanes {
            engine: AcceleratorEngineId::new(1),
        }),
    );
    assert_eq!(
        AcceleratorCommand::new(
            AcceleratorCommandId::new(1),
            AcceleratorCommandKind::GpuKernel { workgroups: 1 },
            0,
        ),
        Err(AcceleratorError::ZeroExecutionLatency {
            command: AcceleratorCommandId::new(1),
        }),
    );
}
