use std::collections::BTreeMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use rem6_checkpoint::CheckpointState;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvClusterTurn,
    RiscvCore,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{
    PartitionId, PartitionSnapshot, PartitionedScheduler, ScheduledEventKind, SchedulerError,
    SchedulerSnapshot,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId, MemoryResponse,
    PartitionedMemoryStore,
};
use rem6_stats::{
    GlobalInstTrackerSnapshot, PcCountPair, ProbePointId, ProbeSnapshot, StackDistProbeConfig,
    StatsRegistry,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    TargetOutcome, TransportEndpointId,
};

use crate::scheduler_checkpoint::{
    SchedulerCheckpointBank, SchedulerCheckpointOwnedEvent, SchedulerCheckpointPort,
};
use crate::{
    GuestEventId, GuestSourceId, HostAction, MemoryStoreCheckpointBank, MemoryStoreCheckpointPort,
    RiscvCoreCheckpointBank, RiscvCoreCheckpointError, RiscvCoreCheckpointPort,
    RiscvDataAccessStats, RiscvInstructionStats, RiscvO3RuntimeStats,
    RiscvRetiredInstructionProbeSnapshot, RiscvSystemRunDriver, RiscvTrapEventPort,
    SchedulerCheckpointError, SystemHostController, SystemHostEventPort,
};

use super::*;

mod live_o3_support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/live_o3.rs"
    ));
}

#[path = "tests/checkpoint_atomicity_tests.rs"]
mod checkpoint_atomicity_tests;

fn scheduler_component(name: &str) -> CheckpointComponentId {
    CheckpointComponentId::new(name).unwrap()
}

fn checkpoint_record(label: &str) -> HostActionRecord {
    HostActionRecord::new(
        0,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(1),
        GuestSourceId::new(1),
        HostAction::Checkpoint {
            label: label.to_string(),
        },
    )
}

fn checkpoint_test_core(cpu: CpuId) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                cpu,
                PartitionId::new(0),
                AgentId::new(0),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu.ifetch").unwrap(),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn executor_with_data_probe_checkpoint(
    core: RiscvCore,
    data_stats: &RiscvDataAccessStats,
) -> SystemActionExecutor {
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor
        .attach_riscv_checkpoint_bank(
            RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
                CheckpointComponentId::new("cpu0").unwrap(),
                core,
            )])
            .unwrap(),
        )
        .unwrap();
    executor.attach_riscv_data_access_stats(data_stats);
    executor
}

#[test]
fn checkpoint_data_probe_capture_rejects_missing_attached_cpu() {
    let cpu = CpuId::new(0);
    let core = checkpoint_test_core(cpu);
    let data_stats = RiscvDataAccessStats::with_stack_distance(
        StackDistProbeConfig::builder(16, 16).build().unwrap(),
    );
    data_stats.reset_for_run([]);
    let live = data_stats.data_access_probe_checkpoint();
    let mut executor = executor_with_data_probe_checkpoint(core, &data_stats);
    let registry = executor.checkpoints.clone();

    let error = executor
        .apply(&checkpoint_record("missing-cpu"))
        .unwrap_err();

    assert_eq!(
        error,
        SystemError::RiscvCheckpoint(RiscvCoreCheckpointError::MissingDataAccessRecorderCpu {
            cpu
        })
    );
    assert_eq!(data_stats.data_access_probe_checkpoint(), live);
    assert!(executor.captured_manifests.is_empty());
    assert!(executor.riscv_data_access_probe_checkpoints.is_empty());
    assert_eq!(executor.checkpoints, registry);
}

#[test]
fn checkpoint_data_probe_capture_rejects_extra_recorder_cpu() {
    let cpu = CpuId::new(0);
    let extra = CpuId::new(1);
    let core = checkpoint_test_core(cpu);
    let data_stats = RiscvDataAccessStats::with_stack_distance(
        StackDistProbeConfig::builder(16, 16).build().unwrap(),
    );
    data_stats.reset_for_run([(cpu, 0), (extra, 0)]);
    let live = data_stats.data_access_probe_checkpoint();
    let mut executor = executor_with_data_probe_checkpoint(core, &data_stats);

    let error = executor.apply(&checkpoint_record("extra-cpu")).unwrap_err();

    assert_eq!(
        error,
        SystemError::RiscvCheckpoint(RiscvCoreCheckpointError::UnexpectedDataAccessRecorderCpu {
            cpu: extra
        })
    );
    assert_eq!(data_stats.data_access_probe_checkpoint(), live);
    assert!(executor.captured_manifests.is_empty());
    assert!(executor.riscv_data_access_probe_checkpoints.is_empty());
}

#[test]
fn checkpoint_data_probe_capture_rejects_cursor_past_core_history() {
    let cpu = CpuId::new(0);
    let core = checkpoint_test_core(cpu);
    let data_stats = RiscvDataAccessStats::with_stack_distance(
        StackDistProbeConfig::builder(16, 16).build().unwrap(),
    );
    data_stats.reset_for_run([(cpu, 1)]);
    let live = data_stats.data_access_probe_checkpoint();
    let mut executor = executor_with_data_probe_checkpoint(core, &data_stats);

    let error = executor
        .apply(&checkpoint_record("cursor-past-history"))
        .unwrap_err();

    assert_eq!(
        error,
        SystemError::RiscvCheckpoint(
            RiscvCoreCheckpointError::DataAccessRecorderCursorOutOfRange {
                cpu,
                cursor: 1,
                event_count: 0,
            }
        )
    );
    assert_eq!(data_stats.data_access_probe_checkpoint(), live);
    assert!(executor.captured_manifests.is_empty());
    assert!(executor.riscv_data_access_probe_checkpoints.is_empty());
}

#[test]
fn checkpoint_restore_rewinds_shared_retired_instruction_probes() {
    let cpu = CpuId::new(0);
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu])
        .with_retired_inst_thresholds([2, 3])
        .with_pc_count_targets([PcCountPair::new(0x8004, 1)]);
    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    let checkpoint_snapshot = instruction_stats.retired_instruction_probe_snapshot();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_instruction_stats(&instruction_stats);

    let checkpoint = HostActionRecord::new(
        10,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(1),
        GuestSourceId::new(1),
        HostAction::Checkpoint {
            label: "instruction-probes".to_string(),
        },
    );
    executor.apply(&checkpoint).unwrap();
    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();
    instruction_stats
        .record_retired_instruction_probe(cpu, 12, 0x8008)
        .unwrap();
    let restore = HostActionRecord::new(
        13,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(2),
        GuestSourceId::new(1),
        HostAction::RestoreCheckpointByLabel {
            label: "instruction-probes".to_string(),
        },
    );

    executor.apply(&restore).unwrap();

    assert_eq!(
        instruction_stats.retired_instruction_probe_snapshot(),
        checkpoint_snapshot
    );
    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();
    instruction_stats
        .record_retired_instruction_probe(cpu, 12, 0x8008)
        .unwrap();
    assert_eq!(
        instruction_stats
            .retired_instruction_probe_snapshot()
            .probes()
            .events()
            .len(),
        6
    );
}

#[test]
fn checkpoint_restore_rewinds_post_capture_memory_trace_events() {
    let fetch = MemoryTrace::new();
    let data = MemoryTrace::new();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_memory_traces(&fetch, &data);
    let endpoint = TransportEndpointId::new("cpu0.dmem").unwrap();
    let before = MemoryTraceEvent::request(
        289,
        MemoryRouteId::new(1),
        endpoint.clone(),
        MemoryTraceKind::RequestSent,
        MemoryRequestId::new(AgentId::new(0), 15),
    );
    data.record(before.clone());
    executor.apply(&checkpoint_record("memory-traces")).unwrap();
    data.record(MemoryTraceEvent::request(
        291,
        MemoryRouteId::new(1),
        endpoint.clone(),
        MemoryTraceKind::RequestSent,
        MemoryRequestId::new(AgentId::new(0), 16),
    ));

    executor
        .apply(&HostActionRecord::new(
            292,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "memory-traces".to_string(),
            },
        ))
        .unwrap();
    let replay = MemoryTraceEvent::request(
        290,
        MemoryRouteId::new(1),
        endpoint,
        MemoryTraceKind::RequestSent,
        MemoryRequestId::new(AgentId::new(0), 16),
    );
    data.record(replay.clone());

    assert_eq!(data.snapshot(), vec![before, replay]);
}

#[test]
fn stable_checkpoint_restore_does_not_replay_discarded_data_probe_source_history() {
    let cpu = CpuId::new(0);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                PartitionId::new(0),
                TransportEndpointId::new("memory.ifetch").unwrap(),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                TransportEndpointId::new("cpu0.dmem").unwrap(),
                PartitionId::new(0),
                TransportEndpointId::new("memory.dmem").unwrap(),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                cpu,
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000_0000),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                fetch_route,
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(
            TransportEndpointId::new("cpu0.dmem").unwrap(),
            data_route,
            CacheLineLayout::new(16).unwrap(),
        ),
    );
    core.write_register(Register::new(2).unwrap(), 0x9000);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        crate::HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(PartitionId::new(1), 2, Arc::clone(&controller))
            .unwrap(),
        GuestSourceId::new(1),
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_data_access_stats(
        RiscvDataAccessStats::with_stack_distance(
            StackDistProbeConfig::builder(16, 16).build().unwrap(),
        ),
    );
    controller
        .lock()
        .unwrap()
        .executor_mut()
        .attach_riscv_checkpoint_bank(
            RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
                CheckpointComponentId::new("cpu0").unwrap(),
                core.clone(),
            )])
            .unwrap(),
        )
        .unwrap();

    issue_test_load(&core, &mut scheduler, &transport, true);
    driver
        .record_run_stats(
            &cluster,
            scheduler.now(),
            &RiscvClusterTurn::idle(scheduler.now()),
        )
        .unwrap();
    let checkpoint_history = core.data_access_events();
    let checkpoint_probes = driver
        .data_access_stats()
        .unwrap()
        .data_access_probe_snapshot();
    let checkpoint_tick = scheduler.now();
    controller
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&HostActionRecord::new(
            checkpoint_tick,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(1),
            GuestSourceId::new(1),
            HostAction::Checkpoint {
                label: "stable-data-history".to_string(),
            },
        ))
        .unwrap();

    issue_test_load(&core, &mut scheduler, &transport, false);
    assert_eq!(core.data_access_event_count(), checkpoint_history.len() + 1);
    let progressed_history = core.data_access_events();
    driver
        .record_run_stats(
            &cluster,
            scheduler.now(),
            &RiscvClusterTurn::idle(scheduler.now()),
        )
        .unwrap();
    controller
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&HostActionRecord::new(
            scheduler.now(),
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "stable-data-history".to_string(),
            },
        ))
        .unwrap();

    driver
        .record_run_stats(
            &cluster,
            scheduler.now(),
            &RiscvClusterTurn::idle(scheduler.now()),
        )
        .unwrap();

    assert_eq!(
        driver
            .data_access_stats()
            .unwrap()
            .data_access_probe_snapshot(),
        checkpoint_probes
    );
    assert_eq!(core.data_access_events(), progressed_history);

    let replay_history_start = core.data_access_event_count();
    issue_test_load(&core, &mut scheduler, &transport, true);
    assert_eq!(core.data_access_event_count(), replay_history_start + 2);
    driver
        .record_run_stats(
            &cluster,
            scheduler.now(),
            &RiscvClusterTurn::idle(scheduler.now()),
        )
        .unwrap();

    let replayed_probes = driver
        .data_access_stats()
        .unwrap()
        .data_access_probe_snapshot();
    assert_eq!(
        replayed_probes.probes().events().len(),
        checkpoint_probes.probes().events().len() + 1
    );
}

fn issue_test_load(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    complete: bool,
) {
    core.issue_next_fetch(
        scheduler,
        transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(0x0000_2603_u32.to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle();
    core.execute_next_completed_fetch().unwrap().unwrap();
    core.issue_next_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            if complete {
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(vec![0x2a, 0, 0, 0]))
                        .unwrap(),
                )
            } else {
                TargetOutcome::NoResponse
            }
        },
    )
    .unwrap()
    .unwrap();
    if complete {
        scheduler.run_until_idle();
    }
}

#[test]
fn failed_restore_does_not_install_prepared_probe_or_memory_trace_state() {
    let cpu = CpuId::new(0);
    let data_stats = RiscvDataAccessStats::with_stack_distance(
        StackDistProbeConfig::builder(16, 16).build().unwrap(),
    );
    data_stats.reset_for_run([(cpu, 0)]);
    let fetch = MemoryTrace::new();
    let data = MemoryTrace::new();
    let component = CheckpointComponentId::new("late-memory").unwrap();
    let memory = Arc::new(Mutex::new(PartitionedMemoryStore::new()));
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_data_access_stats(&data_stats);
    executor.attach_memory_traces(&fetch, &data);
    executor
        .attach_memory_checkpoint_bank(
            MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                component.clone(),
                memory,
            )])
            .unwrap(),
        )
        .unwrap();
    executor.apply(&checkpoint_record("derived-state")).unwrap();

    data_stats.reset_for_run([(cpu, 7)]);
    let progressed_probe = data_stats.data_access_probe_checkpoint();
    let progressed_trace = MemoryTraceEvent::request(
        291,
        MemoryRouteId::new(1),
        TransportEndpointId::new("cpu0.dmem").unwrap(),
        MemoryTraceKind::RequestSent,
        MemoryRequestId::new(AgentId::new(0), 16),
    );
    data.record(progressed_trace.clone());
    executor.checkpoints.remove_component(&component);

    let error = executor
        .apply(&HostActionRecord::new(
            292,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "derived-state".to_string(),
            },
        ))
        .unwrap_err();

    assert!(matches!(
        error,
        SystemError::Checkpoint(rem6_checkpoint::CheckpointError::UnknownComponent { .. })
    ));
    assert_eq!(data_stats.data_access_probe_checkpoint(), progressed_probe);
    assert_eq!(data.snapshot(), [progressed_trace]);
}

#[test]
fn retained_same_label_manifest_restores_its_exact_instruction_probe_snapshot() {
    let cpu = CpuId::new(0);
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu]);
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_instruction_stats(&instruction_stats);
    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    let first = HostActionRecord::new(
        10,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(1),
        GuestSourceId::new(1),
        HostAction::Checkpoint {
            label: "same-label".to_string(),
        },
    );
    let SystemActionOutcome::Checkpoint {
        manifest: first_manifest,
        ..
    } = executor.apply(&first).unwrap()
    else {
        unreachable!()
    };

    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();
    let second = HostActionRecord::new(
        11,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(2),
        GuestSourceId::new(1),
        HostAction::Checkpoint {
            label: "same-label".to_string(),
        },
    );
    executor.apply(&second).unwrap();
    instruction_stats
        .record_retired_instruction_probe(cpu, 12, 0x8008)
        .unwrap();

    executor
        .apply(&HostActionRecord::new(
            13,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(3),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint {
                manifest: first_manifest,
            },
        ))
        .unwrap();

    assert_eq!(
        instruction_stats
            .retired_instruction_probe_snapshot()
            .probes()
            .events()
            .len(),
        1
    );
}

#[test]
fn pending_probe_finalization_updates_only_the_exact_same_label_manifest() {
    let cpu = CpuId::new(0);
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu]);
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_instruction_stats(&instruction_stats);

    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    let first_snapshot = instruction_stats.retired_instruction_probe_snapshot();
    executor.riscv_instruction_probe_checkpoints.push((
        CheckpointManifest::new("same-label", 10, Vec::new()),
        first_snapshot.clone(),
    ));

    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();
    executor.riscv_instruction_probe_checkpoints.push((
        CheckpointManifest::new("same-label", 11, Vec::new()),
        instruction_stats.retired_instruction_probe_snapshot(),
    ));
    executor
        .pending_riscv_instruction_probe_checkpoint_indices
        .insert(1);
    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8008)
        .unwrap();
    let finalized = instruction_stats.retired_instruction_probe_snapshot();

    executor.finalize_pending_riscv_instruction_probe_checkpoints(11);

    assert_eq!(
        executor.riscv_instruction_probe_checkpoints[0].1,
        first_snapshot
    );
    assert_eq!(executor.riscv_instruction_probe_checkpoints[1].1, finalized);
    assert!(executor
        .pending_riscv_instruction_probe_checkpoint_indices
        .is_empty());
}

#[test]
fn pending_instruction_probe_checkpoint_rejects_restore_before_finalization() {
    let cpu = CpuId::new(0);
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu]);
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_instruction_stats(&instruction_stats);
    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    let manifest = CheckpointManifest::new("pending-probes", 10, Vec::new());
    executor.riscv_instruction_probe_checkpoints.push((
        manifest.clone(),
        instruction_stats.retired_instruction_probe_snapshot(),
    ));
    executor
        .pending_riscv_instruction_probe_checkpoint_indices
        .insert(0);
    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8004)
        .unwrap();
    let before = instruction_stats.retired_instruction_probe_snapshot();
    let restore = HostActionRecord::new(
        10,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(2),
        GuestSourceId::new(1),
        HostAction::RestoreCheckpoint { manifest },
    );

    assert_eq!(
        executor.apply(&restore),
        Err(SystemError::PendingInstructionProbeCheckpoint {
            label: "pending-probes".to_string(),
            tick: 10,
        })
    );
    assert_eq!(
        instruction_stats.retired_instruction_probe_snapshot(),
        before
    );
    assert_eq!(
        executor.pending_riscv_instruction_probe_checkpoint_indices,
        BTreeSet::from([0])
    );
}

#[test]
fn caller_supplied_same_label_manifest_does_not_restore_instruction_probes() {
    let cpu = CpuId::new(0);
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu]);
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_instruction_stats(&instruction_stats);
    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    executor.apply(&checkpoint_record("foreign-label")).unwrap();
    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();

    executor
        .apply(&HostActionRecord::new(
            12,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint {
                manifest: CheckpointManifest::new("foreign-label", 99, Vec::new()),
            },
        ))
        .unwrap();

    assert_eq!(
        instruction_stats
            .retired_instruction_probe_snapshot()
            .probes()
            .events()
            .len(),
        2
    );
}

#[test]
fn instruction_probe_restore_is_prepared_before_architectural_commit() {
    let cpu = CpuId::new(0);
    let core = live_o3_support::core(0);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu]);
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor
        .attach_riscv_checkpoint_bank(
            RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(component, core.clone())])
                .unwrap(),
        )
        .unwrap();
    executor.attach_riscv_instruction_stats(&instruction_stats);
    executor.apply(&checkpoint_record("atomic-probes")).unwrap();
    core.write_register(Register::new(7).unwrap(), 0xcafe);
    let probe_checkpoint = executor
        .riscv_instruction_probe_checkpoints
        .iter_mut()
        .find(|(manifest, _snapshot)| manifest.label() == "atomic-probes")
        .expect("atomic probe checkpoint");
    probe_checkpoint.1 = RiscvRetiredInstructionProbeSnapshot::new(
        ProbeSnapshot::with_cursors(
            vec![(
                "cpu0".to_string(),
                "RetiredInsts".to_string(),
                ProbePointId::new(5),
            )],
            Vec::new(),
            Vec::new(),
            0,
            0,
            0,
        ),
        GlobalInstTrackerSnapshot::new(0, Vec::new()),
        None,
        BTreeMap::from([(cpu, ProbePointId::new(5))]),
        BTreeMap::new(),
    );
    let restore = HostActionRecord::new(
        1,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(2),
        GuestSourceId::new(1),
        HostAction::RestoreCheckpointByLabel {
            label: "atomic-probes".to_string(),
        },
    );

    assert!(matches!(
        executor.apply(&restore),
        Err(SystemError::Stats(_))
    ));
    assert_eq!(core.read_register(Register::new(7).unwrap()), 0xcafe);
}

#[test]
fn cloned_driver_and_controller_share_instruction_probe_restore_timeline() {
    let cpu = CpuId::new(0);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        crate::HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(PartitionId::new(1), 2, Arc::clone(&controller))
            .unwrap(),
        GuestSourceId::new(1),
    );
    let driver = RiscvSystemRunDriver::with_instruction_stats(
        trap_port,
        RiscvInstructionStats::for_cpus([cpu]),
    );
    let cloned_driver = driver.clone();
    let mut cloned_controller = controller.lock().unwrap().clone();
    cloned_driver
        .instruction_stats()
        .unwrap()
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    cloned_controller
        .executor_mut()
        .apply(&checkpoint_record("cloned-probes"))
        .unwrap();
    driver
        .instruction_stats()
        .unwrap()
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();

    cloned_controller
        .executor_mut()
        .apply(&HostActionRecord::new(
            12,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "cloned-probes".to_string(),
            },
        ))
        .unwrap();

    let expected = cloned_driver
        .instruction_stats()
        .unwrap()
        .retired_instruction_probe_snapshot();
    assert_eq!(expected.probes().events().len(), 1);
    assert_eq!(
        driver
            .instruction_stats()
            .unwrap()
            .retired_instruction_probe_snapshot(),
        expected
    );
}

#[test]
fn live_o3_restore_syncs_issue_queue_occupancy_before_replay() {
    let (_scheduler, seeded, mut executor, manifest) = live_o3_action_fixture();
    let cpu = seeded.core.id();
    let o3_stats =
        RiscvO3RuntimeStats::register_for_cpus(executor.stats_mut(), [cpu], false).unwrap();
    executor.attach_riscv_o3_runtime_stats(o3_stats);

    executor
        .apply(&HostActionRecord::new(
            live_o3_support::LIVE_TICK + 1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint { manifest },
        ))
        .unwrap();

    let sample = executor
        .stats()
        .snapshot(live_o3_support::LIVE_TICK + 1)
        .samples()
        .iter()
        .find(|sample| {
            sample.path() == "sim.host_actions.stats_dump.cpu0.o3.issue_queue.current_occupancy"
        })
        .cloned()
        .expect("restored issue queue occupancy stat");
    assert_eq!(sample.value(), 1);
}

#[test]
fn outstanding_fetch_mode_transfer_is_not_restorable_by_label() {
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    let mut transport = MemoryTransport::new();
    let core = live_o3_support::core(0);
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.fetch_endpoint(),
                core.partition(),
                rem6_transport::TransportEndpointId::new("memory.ifetch").unwrap(),
                PartitionId::new(1),
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(route, core.fetch_route());
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap();
    assert!(core.has_pending_fetch());
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor
        .attach_riscv_checkpoint_bank(
            RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
                CheckpointComponentId::new("cpu0").unwrap(),
                core,
            )])
            .unwrap(),
        )
        .unwrap();
    let switch = executor
        .apply(&HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(1),
            GuestSourceId::new(1),
            HostAction::SwitchExecutionMode {
                target: crate::ExecutionModeTarget::new("cpu0"),
                mode: crate::ExecutionMode::Timing,
            },
        ))
        .unwrap();
    let SystemActionOutcome::ExecutionModeSwitched {
        state_transfer: Some(transfer),
        ..
    } = switch
    else {
        panic!("missing outstanding-fetch state transfer: {switch:?}")
    };
    let label = transfer.manifest_label().to_string();
    let error = executor
        .apply(&HostActionRecord::new(
            2,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: label.clone(),
            },
        ))
        .unwrap_err();

    assert_eq!(
        error,
        SystemError::MissingCheckpointManifest {
            label: label.clone()
        }
    );
    assert!(!transfer.restorable());
    assert!(!transfer.live_data_handoff());
}

#[test]
fn restorable_mode_transfer_rewinds_retired_instruction_probes() {
    let cpu = CpuId::new(0);
    let core = live_o3_support::core(0);
    let instruction_stats = RiscvInstructionStats::for_cpus([cpu]);
    instruction_stats
        .record_retired_instruction_probe(cpu, 10, 0x8000)
        .unwrap();
    let expected = instruction_stats.retired_instruction_probe_snapshot();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor
        .attach_riscv_checkpoint_bank(
            RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
                CheckpointComponentId::new("cpu0").unwrap(),
                core,
            )])
            .unwrap(),
        )
        .unwrap();
    executor.attach_riscv_instruction_stats(&instruction_stats);
    let switched = executor
        .apply(&HostActionRecord::new(
            10,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(1),
            GuestSourceId::new(1),
            HostAction::SwitchExecutionMode {
                target: crate::ExecutionModeTarget::new("cpu0"),
                mode: crate::ExecutionMode::Timing,
            },
        ))
        .unwrap();
    let SystemActionOutcome::ExecutionModeSwitched {
        state_transfer: Some(transfer),
        ..
    } = switched
    else {
        panic!("missing restorable state transfer: {switched:?}")
    };
    assert!(transfer.restorable());
    instruction_stats
        .record_retired_instruction_probe(cpu, 11, 0x8004)
        .unwrap();

    executor
        .apply(&HostActionRecord::new(
            12,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: transfer.manifest_label().to_string(),
            },
        ))
        .unwrap();

    assert_eq!(
        instruction_stats.retired_instruction_probe_snapshot(),
        expected
    );
}

mod scheduler;
use scheduler::live_o3_action_fixture;
