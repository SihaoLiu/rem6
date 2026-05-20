use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_isa_riscv::Register;
use rem6_kernel::PartitionId;
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_stats::{StatSample, StatSnapshot, StatsRegistry, StatsResetRecord};
use rem6_system::{
    GuestEvent, GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId, HostAction,
    HostActionRecord, HostEventPolicy, RiscvCoreCheckpointBank, RiscvCoreCheckpointPort,
    StopRequest, SystemActionExecutor, SystemActionOutcome, SystemHostController,
    SystemHostEventPort, SystemRunController,
};
use rem6_transport::{MemoryRoute, MemoryTransport, TransportEndpointId};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn riscv_core(cpu: CpuId, partition: PartitionId, agent: AgentId, entry: u64) -> RiscvCore {
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint(&format!("cpu{}.ifetch", cpu.get())),
                partition,
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(cpu, partition, agent, Address::new(entry)),
            CpuFetchConfig::new(
                endpoint(&format!("cpu{}.ifetch", cpu.get())),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

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
fn system_action_executor_refreshes_live_riscv_core_checkpoint_before_manifest() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(11);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1122);
    let bank =
        RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(component.clone(), core)])
            .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    let mut executor =
        SystemActionExecutor::with_riscv_checkpoint_bank(StatsRegistry::new(), checkpoints, bank);

    let checkpoint = HostActionRecord::new(
        36,
        guest,
        host,
        GuestEventId::new(7),
        source,
        HostAction::Checkpoint {
            label: "live-core".to_string(),
        },
    );

    let outcome = executor.apply(&checkpoint).unwrap();

    assert_eq!(
        executor.checkpoints().chunk(&component, "pc"),
        Some(&0x8040_u64.to_le_bytes()[..])
    );
    let xregs = executor.checkpoints().chunk(&component, "xregs").unwrap();
    assert_eq!(&xregs[8..16], &0x1122_u64.to_le_bytes());
    assert_eq!(
        outcome,
        SystemActionOutcome::Checkpoint {
            tick: 36,
            event: GuestEventId::new(7),
            source,
            manifest: CheckpointManifest::new(
                "live-core",
                36,
                vec![CheckpointState::new(
                    component,
                    vec![
                        CheckpointChunk::new("pc", 0x8040_u64.to_le_bytes().to_vec()),
                        CheckpointChunk::new("xregs", xregs.to_vec()),
                    ],
                )],
            ),
        }
    );
}

#[test]
fn system_action_executor_applies_restored_riscv_core_checkpoint() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(12);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x3344);
    let bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        component.clone(),
        core.clone(),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    bank.capture_all_into(&mut checkpoints).unwrap();
    let manifest = checkpoints.capture("resume-core", 48).unwrap();
    let mut executor =
        SystemActionExecutor::with_riscv_checkpoint_bank(StatsRegistry::new(), checkpoints, bank);
    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0);
    let record = HostActionRecord::new(
        72,
        host,
        host,
        GuestEventId::new(8),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let outcome = executor.apply(&record).unwrap();

    assert_eq!(
        outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 72,
            event: GuestEventId::new(8),
            source,
            manifest,
        }
    );
    assert_eq!(core.pc(), Address::new(0x8040));
    assert_eq!(core.read_register(reg(1)), 0x3344);
}

#[test]
fn system_run_controller_records_and_executes_checkpoint_restore_action() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(11);
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    checkpoints.register(cpu.clone()).unwrap();
    checkpoints.write_chunk(&cpu, "pc", vec![0x10]).unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);
    let mut controller = SystemRunController::new(HostEventPolicy);
    let manifest = CheckpointManifest::new(
        "resume",
        40,
        vec![CheckpointState::new(
            cpu.clone(),
            vec![CheckpointChunk::new("pc", vec![0x80])],
        )],
    );
    let record = HostActionRecord::new(
        64,
        host,
        host,
        GuestEventId::new(7),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    assert_eq!(
        controller
            .execute_record(record.clone(), &mut executor)
            .unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 64,
            event: GuestEventId::new(7),
            source,
            manifest: manifest.clone(),
        }
    );
    assert_eq!(executor.checkpoints().chunk(&cpu, "pc"), Some(&[0x80][..]));
    assert_eq!(controller.action_records(), &[record]);
    assert_eq!(
        controller.action_outcomes(),
        &[SystemActionOutcome::CheckpointRestored {
            tick: 64,
            event: GuestEventId::new(7),
            source,
            manifest,
        }]
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
