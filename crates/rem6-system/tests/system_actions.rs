use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_kernel::PartitionedScheduler;
use rem6_stats::{StatSample, StatSnapshot, StatsRegistry, StatsResetRecord};
use rem6_system::{
    GuestEvent, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId, HostAction,
    HostActionRecord, HostEventPolicy, StopRequest, SystemActionExecutor, SystemActionOutcome,
    SystemHostController, SystemHostEventPort, SystemRunController,
};

#[test]
fn system_action_executor_applies_stats_reset_and_dump_actions() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(4);
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    stats.increment(insts, 9).unwrap();
    let mut executor = SystemActionExecutor::new(stats);

    let reset = HostActionRecord::new(
        10,
        guest,
        host,
        GuestEventId::new(1),
        source,
        HostAction::ResetStats,
    );
    assert_eq!(
        executor.apply(&reset).unwrap(),
        SystemActionOutcome::StatsReset(StatsResetRecord::new(10, 1, vec![(insts, 9)]))
    );

    executor.stats_mut().increment(insts, 3).unwrap();

    let dump = HostActionRecord::new(
        14,
        guest,
        host,
        GuestEventId::new(2),
        source,
        HostAction::DumpStats,
    );
    assert_eq!(
        executor.apply(&dump).unwrap(),
        SystemActionOutcome::StatsSnapshot(StatSnapshot::new(
            14,
            1,
            10,
            vec![StatSample::new(insts, "cpu0.committed_insts", "count", 3)],
        ))
    );
}

#[test]
fn system_action_executor_records_non_stats_control_outcomes() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(8);
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());

    let command = HostActionRecord::new(
        20,
        guest,
        host,
        GuestEventId::new(3),
        source,
        HostAction::InjectCommand {
            command: "dump-device-tree".to_string(),
        },
    );
    assert_eq!(
        executor.apply(&command).unwrap(),
        SystemActionOutcome::InjectedCommand {
            tick: 20,
            event: GuestEventId::new(3),
            source,
            command: "dump-device-tree".to_string(),
        }
    );

    let checkpoint = HostActionRecord::new(
        24,
        guest,
        host,
        GuestEventId::new(4),
        source,
        HostAction::Checkpoint {
            label: "after-boot".to_string(),
        },
    );
    assert_eq!(
        executor.apply(&checkpoint).unwrap(),
        SystemActionOutcome::Checkpoint {
            tick: 24,
            event: GuestEventId::new(4),
            source,
            manifest: CheckpointManifest::new("after-boot", 24, Vec::new()),
        }
    );

    let stop = HostActionRecord::new(
        30,
        guest,
        host,
        GuestEventId::new(5),
        source,
        HostAction::Stop { code: 0 },
    );
    assert_eq!(
        executor.apply(&stop).unwrap(),
        SystemActionOutcome::Stop(StopRequest::new(30, GuestEventId::new(5), source, 0))
    );
}

#[test]
fn system_action_executor_captures_checkpoint_manifest() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(10);
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let memory = CheckpointComponentId::new("memory0").unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    checkpoints.register(memory.clone()).unwrap();
    checkpoints.register(cpu.clone()).unwrap();
    checkpoints.write_chunk(&cpu, "pc", vec![0x40]).unwrap();
    checkpoints
        .write_chunk(&memory, "lines", vec![0xaa])
        .unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);

    let checkpoint = HostActionRecord::new(
        32,
        guest,
        host,
        GuestEventId::new(6),
        source,
        HostAction::Checkpoint {
            label: "roi-ready".to_string(),
        },
    );

    assert_eq!(
        executor.apply(&checkpoint).unwrap(),
        SystemActionOutcome::Checkpoint {
            tick: 32,
            event: GuestEventId::new(6),
            source,
            manifest: CheckpointManifest::new(
                "roi-ready",
                32,
                vec![
                    CheckpointState::new(cpu, vec![CheckpointChunk::new("pc", vec![0x40])]),
                    CheckpointState::new(memory, vec![CheckpointChunk::new("lines", vec![0xaa])],),
                ],
            ),
        }
    );
}

#[test]
fn system_run_controller_executes_delivered_stats_events() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(12);
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    stats.increment(insts, 11).unwrap();
    let mut executor = SystemActionExecutor::new(stats);
    let mut controller = SystemRunController::new(HostEventPolicy);

    let reset_outcomes = controller
        .execute_delivery(
            GuestEventDelivery::new(
                40,
                guest,
                host,
                GuestEvent::new(GuestEventId::new(6), source, GuestEventKind::RoiBegin),
            ),
            &mut executor,
        )
        .unwrap();
    assert_eq!(
        reset_outcomes,
        vec![SystemActionOutcome::StatsReset(StatsResetRecord::new(
            40,
            1,
            vec![(insts, 11)],
        ))]
    );

    executor.stats_mut().increment(insts, 5).unwrap();
    let dump_outcomes = controller
        .execute_delivery(
            GuestEventDelivery::new(
                48,
                guest,
                host,
                GuestEvent::new(GuestEventId::new(7), source, GuestEventKind::RoiEnd),
            ),
            &mut executor,
        )
        .unwrap();
    assert_eq!(
        dump_outcomes,
        vec![SystemActionOutcome::StatsSnapshot(StatSnapshot::new(
            48,
            1,
            40,
            vec![StatSample::new(insts, "cpu0.committed_insts", "count", 5)],
        ))]
    );
    assert_eq!(
        controller.action_outcomes(),
        &[
            SystemActionOutcome::StatsReset(StatsResetRecord::new(40, 1, vec![(insts, 11)])),
            SystemActionOutcome::StatsSnapshot(StatSnapshot::new(
                48,
                1,
                40,
                vec![StatSample::new(insts, "cpu0.committed_insts", "count", 5)],
            )),
        ]
    );
}

#[test]
fn system_host_event_port_delivers_and_executes_actions() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(20);
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    stats.increment(insts, 9).unwrap();
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        stats,
    )));
    let port = SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(guest, 5, move |context| {
            port.emit(
                context,
                GuestEvent::new(GuestEventId::new(8), source, GuestEventKind::RoiBegin),
            )
            .unwrap();
        })
        .unwrap();

    let controller_for_stats = Arc::clone(&controller);
    scheduler
        .schedule_at(host, 8, move |_context| {
            controller_for_stats
                .lock()
                .unwrap()
                .executor_mut()
                .stats_mut()
                .increment(insts, 4)
                .unwrap();
        })
        .unwrap();

    let controller_for_dump = Arc::clone(&controller);
    scheduler
        .schedule_at(guest, 9, move |context| {
            SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller_for_dump))
                .unwrap()
                .emit(
                    context,
                    GuestEvent::new(GuestEventId::new(9), source, GuestEventKind::RoiEnd),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 11);
    let controller = controller.lock().unwrap();
    assert!(controller.action_errors().is_empty());
    assert_eq!(
        controller.run().action_outcomes(),
        &[
            SystemActionOutcome::StatsReset(StatsResetRecord::new(7, 1, vec![(insts, 9)])),
            SystemActionOutcome::StatsSnapshot(StatSnapshot::new(
                11,
                1,
                7,
                vec![StatSample::new(insts, "cpu0.committed_insts", "count", 4)],
            )),
        ]
    );
}
