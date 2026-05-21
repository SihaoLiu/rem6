use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorCompletion,
    AcceleratorDmaCompletion, AcceleratorDmaCopy, AcceleratorEngine, AcceleratorEngineConfig,
    AcceleratorEngineId, AcceleratorEngineSnapshot, AcceleratorError, AcceleratorPendingDmaWrite,
    AcceleratorTraceEvent, AcceleratorTraceKind,
};
use rem6_kernel::{
    ParallelRunProfile, PartitionId, PartitionedScheduler, SchedulerError, WaitForEdgeKind,
    WaitForNode,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, LineMemoryStore, MemoryRequest,
    MemoryRequestId,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, RequestDelivery,
    TargetOutcome, TransportEndpointId,
};
use std::sync::{Arc, Mutex};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

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

    let summary = engine
        .run_until_idle_parallel_recorded(&mut scheduler)
        .unwrap();

    assert_eq!(summary.executed_events(), 6);
    assert_eq!(summary.final_tick(), 8);
    assert_eq!(summary.command_completion_count(), 2);
    assert_eq!(summary.dma_completion_count(), 0);
    assert_eq!(summary.pending_dma_write_count(), 0);
    assert_eq!(summary.trace_event_count(), 6);
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
    assert!(summary.has_command_activity());
    assert!(!summary.has_dma_activity());
    let accelerator_activity = summary.partition_activity(accelerator_partition).unwrap();
    assert!(accelerator_activity.worker_count() >= 1);
    assert!(accelerator_activity.dispatch_count() >= 1);
    assert!(accelerator_activity.max_pending_events() >= 1);
    assert!(summary.has_partition_activity(accelerator_partition));
    assert!(!summary.has_partition_activity(PartitionId::new(3)));
    assert!(summary.active_partition_count() >= 1);
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
fn accelerator_wait_for_graph_tracks_queued_commands_until_lane_starts() {
    let cpu_partition = PartitionId::new(0);
    let accelerator_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let engine = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(14), accelerator_partition, 1)
            .unwrap(),
    );
    let marker = engine.mark_wait_for();

    engine
        .submit_from_partition(
            &mut scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(50),
                AcceleratorCommandKind::NpuInference { tiles: 4 },
                5,
            )
            .unwrap(),
        )
        .unwrap();
    engine
        .submit_from_partition(
            &mut scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(51),
                AcceleratorCommandKind::NpuInference { tiles: 8 },
                3,
            )
            .unwrap(),
        )
        .unwrap();

    let first_epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(first_epoch.summary().final_tick(), 2);
    let lane = WaitForNode::resource("accelerator.14.lane.0").unwrap();
    let queued_command = WaitForNode::transaction("accelerator.14.command.51").unwrap();
    let wait_for = engine.wait_for_graph().snapshot();
    assert_eq!(wait_for.edge_count(), 1);
    assert_eq!(wait_for.first_observed_tick(), Some(2));
    assert_eq!(wait_for.last_observed_tick(), Some(2));
    assert!(wait_for.contains_edge(&queued_command, &lane, WaitForEdgeKind::Queue));
    assert_eq!(
        wait_for.dependencies(&queued_command)[0].observation_count(),
        1
    );
    let snapshot = engine.snapshot();
    assert!(snapshot.has_queued_commands());
    assert_eq!(snapshot.queued_commands().len(), 1);
    assert_eq!(
        snapshot.queued_commands()[0].command().id(),
        AcceleratorCommandId::new(51)
    );
    assert_eq!(snapshot.queued_commands()[0].lane(), 0);
    assert_eq!(snapshot.queued_commands()[0].queued_at(), 2);
    assert_eq!(snapshot.queued_commands()[0].started_at(), 7);

    let restored = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(14), accelerator_partition, 1)
            .unwrap(),
    );
    restored.restore(&snapshot);
    assert!(restored.wait_for_graph().snapshot().contains_edge(
        &queued_command,
        &lane,
        WaitForEdgeKind::Queue
    ));

    while scheduler.now() < 7 {
        scheduler.run_next_epoch_parallel_recorded().unwrap();
    }
    assert!(engine.wait_for_graph().is_empty());

    scheduler.run_until_idle_parallel_recorded().unwrap();
    let history = engine.wait_for_graph_since(marker).snapshot();
    assert_eq!(history.edge_count(), 1);
    assert_eq!(history.first_observed_tick(), Some(2));
    assert_eq!(history.last_observed_tick(), Some(6));
    assert!(history.contains_edge(&queued_command, &lane, WaitForEdgeKind::Queue));
    assert!(engine.wait_for_graph().is_empty());
}

#[test]
fn accelerator_dma_copy_uses_parallel_memory_transport() {
    let accelerator_partition = PartitionId::new(0);
    let memory_partition = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let accelerator = endpoint("accelerator0.dma");
    let memory = endpoint("memory0.port");
    let route = transport
        .add_route(
            MemoryRoute::new(
                accelerator.clone(),
                accelerator_partition,
                memory.clone(),
                memory_partition,
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let engine = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(9), accelerator_partition, 1)
            .unwrap(),
    );
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
        MemoryRequestId::new(AgentId::new(9), 1),
        Address::new(0x1008),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let copy = AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(30),
        route,
        read_request.clone(),
        route,
        MemoryRequestId::new(AgentId::new(9), 2),
        Address::new(0x2004),
    )
    .unwrap();

    let read_store = Arc::clone(&store);
    engine
        .submit_dma_copy_read(
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
    let read_run = engine
        .run_until_idle_parallel_recorded(&mut scheduler)
        .unwrap();
    assert_eq!(read_run.dma_completion_count(), 0);
    assert_eq!(read_run.pending_dma_write_count(), 1);
    assert_eq!(read_run.trace_event_count(), 1);
    assert!(read_run.has_dma_activity());
    assert!(!read_run.has_command_activity());

    assert_eq!(
        engine.pending_dma_writes(),
        vec![AcceleratorPendingDmaWrite::new(
            copy.clone(),
            vec![0x10, 0x20, 0x30, 0x40],
            4,
        )]
    );

    let write_store = Arc::clone(&store);
    assert!(engine
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
    let write_run = engine
        .run_until_idle_parallel_recorded(&mut scheduler)
        .unwrap();
    assert_eq!(write_run.dma_completion_count(), 1);
    assert_eq!(write_run.pending_dma_write_count(), 0);
    assert_eq!(write_run.trace_event_count(), 1);
    assert!(write_run.has_dma_activity());
    assert!(!write_run.has_command_activity());

    let destination = store
        .lock()
        .unwrap()
        .line_data(Address::new(0x2000))
        .unwrap();
    assert_eq!(&destination[4..8], &[0x10, 0x20, 0x30, 0x40]);
    assert!(engine.pending_dma_writes().is_empty());
    assert_eq!(
        engine.dma_completions(),
        vec![AcceleratorDmaCompletion::new(
            AcceleratorCommandId::new(30),
            read_request.id(),
            MemoryRequestId::new(AgentId::new(9), 2),
            4,
            8,
        )]
    );
    assert_eq!(
        engine.trace(),
        vec![
            AcceleratorTraceEvent::new(
                0,
                AcceleratorTraceKind::DmaReadIssued {
                    command: AcceleratorCommandId::new(30),
                    request: read_request.id(),
                },
            ),
            AcceleratorTraceEvent::new(
                4,
                AcceleratorTraceKind::DmaReadCompleted {
                    command: AcceleratorCommandId::new(30),
                    request: read_request.id(),
                    bytes: 4,
                },
            ),
            AcceleratorTraceEvent::new(
                4,
                AcceleratorTraceKind::DmaWriteIssued {
                    command: AcceleratorCommandId::new(30),
                    request: MemoryRequestId::new(AgentId::new(9), 2),
                },
            ),
            AcceleratorTraceEvent::new(
                8,
                AcceleratorTraceKind::DmaWriteCompleted {
                    command: AcceleratorCommandId::new(30),
                    request: MemoryRequestId::new(AgentId::new(9), 2),
                },
            ),
        ],
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                accelerator.clone(),
                MemoryTraceKind::RequestSent,
                read_request.id(),
            ),
            MemoryTraceEvent::request(
                2,
                route,
                memory.clone(),
                MemoryTraceKind::RequestArrived,
                read_request.id(),
            ),
            MemoryTraceEvent::response(
                4,
                route,
                accelerator.clone(),
                read_request.id(),
                rem6_memory::ResponseStatus::Completed,
            ),
            MemoryTraceEvent::request(
                4,
                route,
                accelerator,
                MemoryTraceKind::RequestSent,
                MemoryRequestId::new(AgentId::new(9), 2),
            ),
            MemoryTraceEvent::request(
                6,
                route,
                memory,
                MemoryTraceKind::RequestArrived,
                MemoryRequestId::new(AgentId::new(9), 2),
            ),
            MemoryTraceEvent::response(
                8,
                route,
                endpoint("accelerator0.dma"),
                MemoryRequestId::new(AgentId::new(9), 2),
                rem6_memory::ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn accelerator_dma_copy_rejects_requests_without_return_data() {
    let request = MemoryRequest::write(
        MemoryRequestId::new(AgentId::new(4), 1),
        Address::new(0x3000),
        AccessSize::new(4).unwrap(),
        vec![1, 2, 3, 4],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(
        AcceleratorDmaCopy::new(
            AcceleratorCommandId::new(31),
            rem6_transport::MemoryRouteId::new(0),
            request.clone(),
            rem6_transport::MemoryRouteId::new(0),
            MemoryRequestId::new(AgentId::new(4), 2),
            Address::new(0x4000),
        ),
        Err(AcceleratorError::DmaReadRequiresData {
            command: AcceleratorCommandId::new(31),
            request: request.id(),
        }),
    );
}

#[test]
fn accelerator_engine_restores_snapshot_state_and_lane_reservations() {
    let cpu_partition = PartitionId::new(0);
    let accelerator_partition = PartitionId::new(1);
    let engine = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(12), accelerator_partition, 1)
            .unwrap(),
    );
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    engine
        .submit_from_partition(
            &mut scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(40),
                AcceleratorCommandKind::NpuInference { tiles: 3 },
                5,
            )
            .unwrap(),
        )
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.lane_count(), 1);
    assert!(!snapshot.has_pending_dma_writes());
    let rebuilt = AcceleratorEngineSnapshot::new(
        snapshot.lane_busy_until().to_vec(),
        snapshot.trace().to_vec(),
        snapshot.completed().to_vec(),
        snapshot.pending_dma_writes().to_vec(),
        snapshot.dma_completions().to_vec(),
    );
    assert_eq!(rebuilt, snapshot);

    let mut mutation_scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    engine
        .submit_from_partition(
            &mut mutation_scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(41),
                AcceleratorCommandKind::GpuKernel { workgroups: 1 },
                3,
            )
            .unwrap(),
        )
        .unwrap();
    mutation_scheduler.run_until_idle_parallel().unwrap();
    assert_ne!(engine.snapshot(), snapshot);

    engine.restore(&snapshot);
    assert_eq!(engine.snapshot(), snapshot);

    let mut restored_scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    engine
        .submit_from_partition(
            &mut restored_scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(42),
                AcceleratorCommandKind::DmaCopy { bytes: 16 },
                4,
            )
            .unwrap(),
        )
        .unwrap();
    restored_scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        engine.completed(),
        vec![
            AcceleratorCompletion::new(
                AcceleratorCommandId::new(40),
                AcceleratorCommandKind::NpuInference { tiles: 3 },
                0,
                2,
                7,
            ),
            AcceleratorCompletion::new(
                AcceleratorCommandId::new(42),
                AcceleratorCommandKind::DmaCopy { bytes: 16 },
                0,
                7,
                11,
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
