use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_interrupt::{
    InterruptController, InterruptError, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptPriority, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};
use rem6_mmio::{MmioRequest, MmioRequestId};
use rem6_stats::{
    StatDumpId, StatDumpRecord, StatSample, StatSnapshot, StatsError, StatsRegistry,
    StatsResetRecord,
};
use rem6_system::{
    ClintCheckpointBank, ClintCheckpointPort, DramMemoryCheckpointBank, DramMemoryCheckpointPort,
    ExecutionMode, ExecutionModeCheckpointError, ExecutionModeTarget, GuestEvent,
    GuestEventDelivery, GuestEventId, GuestEventKind, GuestSourceId, HostAction, HostActionRecord,
    HostEventPolicy, InterruptControllerCheckpointBank, InterruptControllerCheckpointPort,
    MemoryStoreCheckpointBank, MemoryStoreCheckpointPort, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, StopRequest, SystemActionExecutor, SystemActionOutcome, SystemError,
    SystemHostController, SystemHostEventPort, SystemRunController, TimerCheckpointBank,
    TimerCheckpointPort, UartCheckpointBank, UartCheckpointPort,
};
use rem6_timer::{
    ClintHartConfig, ClintHartSnapshot, ClintMmioDevice, ClintSnapshot, ClintTimebase,
    ProgrammableTimer, TimerArm, TimerExpiry, TimerId, TimerSignalError, TimerSnapshot,
};
use rem6_transport::{MemoryRoute, MemoryTransport, TransportEndpointId};
use rem6_uart::{
    UartId, UartInterruptError, UartMmioDevice, UartRxByte, UartSnapshot, UartTxByte,
    UART_MMIO_DATA_OFFSET, UART_MMIO_REGISTER_BYTES,
};

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

fn line_data(base: u8) -> Vec<u8> {
    (0..64).map(|offset| base.wrapping_add(offset)).collect()
}

fn memory_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn dram_geometry() -> DramGeometry {
    DramGeometry::new(4, 256, 64).unwrap()
}

fn dram_timing() -> DramTiming {
    DramTiming::new(3, 5, 7, 2, 4).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(11), sequence)
}

fn execution_mode_checkpoint_payload(manifest: &CheckpointManifest) -> Option<&[u8]> {
    manifest
        .states()
        .iter()
        .find(|state| state.component().as_str() == "host.execution_modes")?
        .chunks()
        .iter()
        .find(|chunk| chunk.name() == "modes")
        .map(|chunk| chunk.payload())
}

fn uart_byte_mask() -> ByteMask {
    ByteMask::full(AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap()).unwrap()
}

fn write_uart_byte(uart: &UartMmioDevice, tick: u64, byte: u8) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let base = uart.base();
    let uart = uart.clone();
    scheduler
        .schedule_at(PartitionId::new(0), tick, move |context| {
            uart.respond(
                context,
                &MmioRequest::write(
                    MmioRequestId::new(300 + tick),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    vec![byte],
                    uart_byte_mask(),
                )
                .unwrap(),
            )
            .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();
}

fn timer_with_interrupt(
    id: TimerId,
    timer_partition: PartitionId,
    target_partition: PartitionId,
    source: InterruptSourceId,
) -> ProgrammableTimer {
    let route = InterruptRoute::new(
        InterruptLineId::new(70),
        InterruptTargetId::new(0),
        target_partition,
    );
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    ProgrammableTimer::new(id, timer_partition, source, port)
}

fn clint_device(base: Address, target_partition: PartitionId) -> ClintMmioDevice {
    let target = InterruptTargetId::new(0);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let software_route = InterruptRoute::new(InterruptLineId::new(90), target, target_partition);
    let timer_route = InterruptRoute::new(InterruptLineId::new(91), target, target_partition);
    controller
        .lock()
        .unwrap()
        .register_route(software_route)
        .unwrap();
    controller
        .lock()
        .unwrap()
        .register_route(timer_route)
        .unwrap();
    let software_port = InterruptLinePort::new(
        InterruptLineChannel::new(software_route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer_port = InterruptLinePort::new(
        InterruptLineChannel::new(timer_route, 2).unwrap(),
        Arc::clone(&controller),
    );

    ClintMmioDevice::with_timebase(
        base,
        [ClintHartConfig::new(
            0,
            software_port,
            InterruptSourceId::new(90),
            timer_port,
            InterruptSourceId::new(91),
        )],
        ClintTimebase::rtc_driven(),
    )
    .unwrap()
}

fn read_uart_byte(uart: &UartMmioDevice, tick: u64) -> u8 {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let base = uart.base();
    let uart = uart.clone();
    let byte = Arc::new(Mutex::new(None));
    let result = Arc::clone(&byte);
    scheduler
        .schedule_at(PartitionId::new(0), tick, move |context| {
            let response = uart
                .respond(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(400 + tick),
                        Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                        AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap();
            *result.lock().unwrap() = Some(response.data().unwrap()[0]);
        })
        .unwrap();
    scheduler.run_until_idle();
    let value = byte.lock().unwrap().unwrap();
    value
}

fn dram_read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        memory_layout(),
    )
    .unwrap()
}

fn dram_write(address: u64, bytes: &[u8], sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes.len() as u64).unwrap(),
        bytes.to_vec(),
        ByteMask::full(AccessSize::new(bytes.len() as u64).unwrap()).unwrap(),
        memory_layout(),
    )
    .unwrap()
}

fn partitioned_memory_store() -> (
    rem6_memory::PartitionedMemoryStore,
    rem6_memory::MemoryTargetId,
) {
    let target = rem6_memory::MemoryTargetId::new(10);
    let mut store = rem6_memory::PartitionedMemoryStore::new();
    store.add_partition(target, memory_layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    store
        .insert_line(target, Address::new(0x1000), line_data(0x10))
        .unwrap();
    (store, target)
}

fn dram_memory_controller() -> (DramMemoryController, rem6_memory::MemoryTargetId) {
    let target = rem6_memory::MemoryTargetId::new(20);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            memory_layout(),
            dram_geometry(),
            dram_timing(),
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(target, Address::new(0x1000), line_data(0x10))
        .unwrap();
    (controller, target)
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
        SystemActionOutcome::StatsDump(StatDumpRecord::new(
            StatDumpId::new(0),
            StatSnapshot::new(
                14,
                1,
                10,
                vec![StatSample::new(insts, "cpu0.committed_insts", "count", 3)],
            ),
        ))
    );
    assert_eq!(
        executor.stats().dump_records(),
        &[StatDumpRecord::new(
            StatDumpId::new(0),
            StatSnapshot::new(
                14,
                1,
                10,
                vec![StatSample::new(insts, "cpu0.committed_insts", "count", 3)],
            ),
        )]
    );
}

#[test]
fn system_action_executor_rejects_out_of_order_stats_reset_actions() {
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
    executor.apply(&reset).unwrap();
    executor.stats_mut().increment(insts, 3).unwrap();

    let out_of_order_reset = HostActionRecord::new(
        9,
        guest,
        host,
        GuestEventId::new(2),
        source,
        HostAction::ResetStats,
    );
    assert_eq!(
        executor.apply(&out_of_order_reset).unwrap_err(),
        SystemError::Stats(StatsError::ResetBeforeLastReset {
            tick: 9,
            reset_tick: 10,
        })
    );

    assert_eq!(executor.stats().epoch(), 1);
    assert_eq!(executor.stats().reset_tick(), 10);
    assert_eq!(
        executor.stats().snapshot(12),
        StatSnapshot::new(
            12,
            1,
            10,
            vec![StatSample::new(insts, "cpu0.committed_insts", "count", 3)],
        )
    );

    let dump = HostActionRecord::new(
        12,
        guest,
        host,
        GuestEventId::new(3),
        source,
        HostAction::DumpStats,
    );
    executor.apply(&dump).unwrap();
    let reset_before_dump = HostActionRecord::new(
        11,
        guest,
        host,
        GuestEventId::new(4),
        source,
        HostAction::ResetStats,
    );
    assert_eq!(
        executor.apply(&reset_before_dump).unwrap_err(),
        SystemError::Stats(StatsError::HistoryTickBeforeLastRecord {
            tick: 11,
            last_history_tick: 12,
        })
    );
    assert_eq!(executor.stats().epoch(), 1);
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
    let fregs = executor.checkpoints().chunk(&component, "fregs").unwrap();
    assert_eq!(fregs.len(), 32 * 8);
    let hart_run_state = executor
        .checkpoints()
        .chunk(&component, "hart-run-state")
        .unwrap();
    assert_eq!(hart_run_state, &[0]);
    let pmp = executor.checkpoints().chunk(&component, "pmp").unwrap();
    assert_eq!(&pmp[0..2], &16_u16.to_le_bytes());
    let in_order_pipeline = executor
        .checkpoints()
        .chunk(&component, "in-order-pipeline")
        .unwrap();
    let bimode_branch_predictor = executor
        .checkpoints()
        .chunk(&component, "bimode-branch-predictor")
        .unwrap();
    let branch_predictor = executor
        .checkpoints()
        .chunk(&component, "branch-predictor")
        .unwrap();
    let gshare_branch_predictor = executor
        .checkpoints()
        .chunk(&component, "gshare-branch-predictor")
        .unwrap();
    let tournament_branch_predictor = executor
        .checkpoints()
        .chunk(&component, "tournament-branch-predictor")
        .unwrap();
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
                        CheckpointChunk::new(
                            "bimode-branch-predictor",
                            bimode_branch_predictor.to_vec(),
                        ),
                        CheckpointChunk::new("branch-predictor", branch_predictor.to_vec()),
                        CheckpointChunk::new("fregs", fregs.to_vec()),
                        CheckpointChunk::new(
                            "gshare-branch-predictor",
                            gshare_branch_predictor.to_vec(),
                        ),
                        CheckpointChunk::new("hart-run-state", hart_run_state.to_vec()),
                        CheckpointChunk::new("in-order-pipeline", in_order_pipeline.to_vec()),
                        CheckpointChunk::new("pc", 0x8040_u64.to_le_bytes().to_vec()),
                        CheckpointChunk::new("pmp", pmp.to_vec()),
                        CheckpointChunk::new(
                            "tournament-branch-predictor",
                            tournament_branch_predictor.to_vec(),
                        ),
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
fn system_action_executor_refreshes_and_restores_live_memory_checkpoint() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(13);
    let component = CheckpointComponentId::new("memory0").unwrap();
    let (store, target) = partitioned_memory_store();
    let store = Arc::new(Mutex::new(store));
    let bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
        component.clone(),
        Arc::clone(&store),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    let mut executor =
        SystemActionExecutor::with_memory_checkpoint_bank(StatsRegistry::new(), checkpoints, bank);

    let checkpoint = HostActionRecord::new(
        84,
        host,
        host,
        GuestEventId::new(9),
        source,
        HostAction::Checkpoint {
            label: "with-memory".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(
        executor
            .checkpoints()
            .chunk(&component, "store")
            .unwrap()
            .len()
            > 128
    );
    store
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x1000), line_data(0xaa))
        .unwrap();

    let restore = HostActionRecord::new(
        96,
        host,
        host,
        GuestEventId::new(10),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 96,
            event: GuestEventId::new(10),
            source,
            manifest,
        }
    );
    assert_eq!(
        store
            .lock()
            .unwrap()
            .line_data(target, Address::new(0x1000))
            .unwrap(),
        line_data(0x10)
    );
}

#[test]
fn system_action_executor_refreshes_and_restores_live_dram_checkpoint() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(15);
    let component = CheckpointComponentId::new("dram0").unwrap();
    let (mut controller, target) = dram_memory_controller();
    let first = controller.accept(0, &dram_read(0x1000, 8, 31)).unwrap();
    assert_eq!(first.ready_cycle(), 8);
    assert!(!first.dram_access().row_hit());
    let controller = Arc::new(Mutex::new(controller));
    let bank = DramMemoryCheckpointBank::new([DramMemoryCheckpointPort::new(
        component.clone(),
        Arc::clone(&controller),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    let mut executor = SystemActionExecutor::with_dram_memory_checkpoint_bank(
        StatsRegistry::new(),
        checkpoints,
        bank,
    );

    let checkpoint = HostActionRecord::new(
        104,
        host,
        host,
        GuestEventId::new(13),
        source,
        HostAction::Checkpoint {
            label: "with-dram-memory".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(
        executor
            .checkpoints()
            .chunk(&component, "dram")
            .unwrap()
            .len()
            > 192
    );
    {
        let mut controller = controller.lock().unwrap();
        controller
            .accept(8, &dram_write(0x1000, &[0xaa, 0xbb, 0xcc, 0xdd], 32))
            .unwrap();
        assert_eq!(
            &controller.line_data(target, Address::new(0x1000)).unwrap()[..4],
            &[0xaa, 0xbb, 0xcc, 0xdd]
        );
    }

    let restore = HostActionRecord::new(
        116,
        host,
        host,
        GuestEventId::new(14),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 116,
            event: GuestEventId::new(14),
            source,
            manifest,
        }
    );
    let mut controller = controller.lock().unwrap();
    assert_eq!(
        &controller.line_data(target, Address::new(0x1000)).unwrap()[..4],
        &[0x10, 0x11, 0x12, 0x13]
    );
    let bank = controller
        .dram_controller(target)
        .unwrap()
        .bank_state(0)
        .unwrap();
    assert_eq!(bank.open_row(), Some(4));
    assert_eq!(bank.available_cycle(), 8);
    let row_hit = controller.accept(8, &dram_read(0x1008, 4, 33)).unwrap();
    assert!(row_hit.dram_access().row_hit());
    assert_eq!(row_hit.ready_cycle(), 13);
}

#[test]
fn system_action_executor_refreshes_and_restores_live_uart_checkpoint() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(16);
    let component = CheckpointComponentId::new("uart0").unwrap();
    let uart = UartMmioDevice::new(UartId::new(0), Address::new(0xa000));
    let captured = UartSnapshot::new(
        vec![UartTxByte::new(4, b'O')],
        vec![UartRxByte::new(120, b'A'), UartRxByte::new(120, b'B')],
        b"AB".to_vec(),
        Vec::new(),
        vec![UartInterruptError::new(
            33,
            InterruptSourceId::new(40),
            InterruptEventKind::Assert,
            InterruptError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: PartitionId::new(2),
                target: PartitionId::new(3),
                source_tick: 40,
                delivery_tick: 44,
                minimum_delivery_tick: 45,
            }),
        )],
    );
    uart.restore(&captured);
    let bank = UartCheckpointBank::new([UartCheckpointPort::new(component.clone(), uart.clone())])
        .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    let mut executor =
        SystemActionExecutor::with_uart_checkpoint_bank(StatsRegistry::new(), checkpoints, bank);

    let checkpoint = HostActionRecord::new(
        124,
        host,
        host,
        GuestEventId::new(15),
        source,
        HostAction::Checkpoint {
            label: "with-uart".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(
        executor
            .checkpoints()
            .chunk(&component, "uart")
            .unwrap()
            .len()
            > 48
    );
    write_uart_byte(&uart, 5, b'X');
    assert_eq!(read_uart_byte(&uart, 6), b'A');
    uart.inject_rx([b'C']).unwrap();
    assert_ne!(uart.snapshot().rx_pending(), b"AB");

    let restore = HostActionRecord::new(
        128,
        host,
        host,
        GuestEventId::new(16),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 128,
            event: GuestEventId::new(16),
            source,
            manifest,
        }
    );
    assert_eq!(uart.snapshot().tx_bytes(), &[UartTxByte::new(4, b'O')]);
    assert_eq!(uart.snapshot().rx_pending(), b"AB");
    assert_eq!(
        uart.snapshot().interrupt_errors(),
        captured.interrupt_errors()
    );
    assert_eq!(read_uart_byte(&uart, 129), b'A');
    assert_eq!(read_uart_byte(&uart, 130), b'B');
}

#[test]
fn system_action_executor_refreshes_and_restores_live_timer_checkpoint() {
    let host = PartitionId::new(1);
    let timer_partition = PartitionId::new(2);
    let target_partition = PartitionId::new(0);
    let source = GuestSourceId::new(17);
    let interrupt_source = InterruptSourceId::new(52);
    let component = CheckpointComponentId::new("timer0").unwrap();
    let timer = timer_with_interrupt(
        TimerId::new(0),
        timer_partition,
        target_partition,
        interrupt_source,
    );
    let captured = TimerSnapshot::new(
        TimerId::new(0),
        timer_partition,
        interrupt_source,
        Some(40),
        vec![TimerArm::new(4, 12, 40)],
        vec![TimerExpiry::new(3, 20)],
        vec![TimerSignalError::new(
            3,
            21,
            InterruptError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: timer_partition,
                target: target_partition,
                source_tick: 21,
                delivery_tick: 22,
                minimum_delivery_tick: 23,
            }),
        )],
    );
    let empty = TimerSnapshot::new(
        TimerId::new(0),
        timer_partition,
        interrupt_source,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    timer.restore(&captured).unwrap();
    let bank =
        TimerCheckpointBank::new([TimerCheckpointPort::new(component.clone(), timer.clone())])
            .unwrap();
    let checkpoints = CheckpointRegistry::new();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);
    executor.attach_timer_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        132,
        host,
        host,
        GuestEventId::new(17),
        source,
        HostAction::Checkpoint {
            label: "with-timer".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(
        executor
            .checkpoints()
            .chunk(&component, "timer")
            .unwrap()
            .len()
            > 72
    );
    timer.restore(&empty).unwrap();
    assert_ne!(timer.snapshot(), captured);

    let restore = HostActionRecord::new(
        136,
        host,
        host,
        GuestEventId::new(18),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 136,
            event: GuestEventId::new(18),
            source,
            manifest,
        }
    );
    assert_eq!(timer.snapshot(), captured);
}

#[test]
fn system_action_executor_refreshes_and_restores_live_clint_checkpoint() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(18);
    let component = CheckpointComponentId::new("clint0").unwrap();
    let clint = clint_device(Address::new(0x200_0000), PartitionId::new(0));
    let captured = ClintSnapshot::with_mtime(
        Address::new(0x200_0000),
        7,
        vec![ClintHartSnapshot::new(0, 1, 44, 2, true)],
    );
    let empty = ClintSnapshot::with_mtime(
        Address::new(0x200_0000),
        0,
        vec![ClintHartSnapshot::new(0, 0, u64::MAX, 0, false)],
    );
    clint.restore(&captured).unwrap();
    let bank =
        ClintCheckpointBank::new([ClintCheckpointPort::new(component.clone(), clint.clone())])
            .unwrap();
    let checkpoints = CheckpointRegistry::new();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);
    executor.attach_clint_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        137,
        host,
        host,
        GuestEventId::new(19),
        source,
        HostAction::Checkpoint {
            label: "with-clint".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(
        executor
            .checkpoints()
            .chunk(&component, "clint")
            .unwrap()
            .len()
            >= 56
    );
    clint.restore(&empty).unwrap();
    assert_ne!(clint.snapshot(), captured);

    let restore = HostActionRecord::new(
        138,
        host,
        host,
        GuestEventId::new(20),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 138,
            event: GuestEventId::new(20),
            source,
            manifest,
        }
    );
    assert_eq!(clint.snapshot(), captured);
}

#[test]
fn system_action_executor_refreshes_and_restores_live_interrupt_controller_checkpoint() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(19);
    let component = CheckpointComponentId::new("interrupt0").unwrap();
    let target = InterruptTargetId::new(0);
    let cpu = PartitionId::new(0);
    let claimed_line = InterruptLineId::new(80);
    let pending_line = InterruptLineId::new(81);
    let extra_line = InterruptLineId::new(82);
    let claimed_source = InterruptSourceId::new(60);
    let pending_source = InterruptSourceId::new(61);
    let claimed =
        rem6_interrupt::InterruptClaim::new(claimed_line, target, cpu, claimed_source, 7, 11);
    let mut controller = InterruptController::new();
    controller
        .register_route(InterruptRoute::new(claimed_line, target, cpu))
        .unwrap();
    controller
        .register_route(InterruptRoute::new(pending_line, target, cpu))
        .unwrap();
    controller
        .set_priority(claimed_line, InterruptPriority::new(9))
        .unwrap();
    controller
        .set_priority(pending_line, InterruptPriority::ZERO)
        .unwrap();
    controller.assert(pending_line, pending_source, 5).unwrap();
    controller.assert(claimed_line, claimed_source, 7).unwrap();
    assert_eq!(controller.claim(target, cpu, 11), Some(claimed));
    let controller = Arc::new(Mutex::new(controller));
    let bank = InterruptControllerCheckpointBank::new([InterruptControllerCheckpointPort::new(
        component.clone(),
        Arc::clone(&controller),
    )])
    .unwrap();
    let checkpoints = CheckpointRegistry::new();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);
    executor
        .attach_interrupt_controller_checkpoint_bank(bank)
        .unwrap();
    assert!(executor.interrupt_controller_checkpoint_bank().is_some());

    let checkpoint = HostActionRecord::new(
        140,
        host,
        host,
        GuestEventId::new(19),
        source,
        HostAction::Checkpoint {
            label: "with-interrupt".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let captured = controller.lock().unwrap().snapshot(140);

    assert!(
        executor
            .checkpoints()
            .chunk(&component, "interrupt")
            .unwrap()
            .len()
            > 96
    );
    {
        let mut controller = controller.lock().unwrap();
        controller.complete(target, cpu, claimed_line, 141).unwrap();
        controller
            .set_priority(pending_line, InterruptPriority::new(4))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(extra_line, target, cpu))
            .unwrap();
        controller
            .assert(extra_line, InterruptSourceId::new(62), 142)
            .unwrap();
        assert_ne!(controller.snapshot(140), captured);
    }

    let restore = HostActionRecord::new(
        144,
        host,
        host,
        GuestEventId::new(20),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 144,
            event: GuestEventId::new(20),
            source,
            manifest,
        }
    );
    let controller = controller.lock().unwrap();
    assert_eq!(controller.snapshot(140), captured);
    assert_eq!(controller.claimed(), vec![claimed]);
    assert_eq!(
        controller.priority(pending_line).unwrap(),
        InterruptPriority::ZERO
    );
    assert_eq!(
        controller.priority(extra_line),
        Err(InterruptError::UnknownLine { line: extra_line })
    );
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
fn system_run_controller_executes_delivered_checkpoint_restore_by_label() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(11);
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    checkpoints.register(cpu.clone()).unwrap();
    checkpoints.write_chunk(&cpu, "pc", vec![0x10]).unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);
    let mut controller = SystemRunController::new(HostEventPolicy);

    let checkpoint = controller
        .execute_delivery(
            GuestEventDelivery::new(
                40,
                host,
                host,
                GuestEvent::new(
                    GuestEventId::new(7),
                    source,
                    GuestEventKind::Checkpoint {
                        label: "resume".to_string(),
                    },
                ),
            ),
            &mut executor,
        )
        .unwrap();
    let manifest = match &checkpoint[0] {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest.clone(),
        outcome => panic!("unexpected checkpoint outcome: {outcome:?}"),
    };
    executor
        .checkpoints_mut()
        .write_chunk(&cpu, "pc", vec![0x44])
        .unwrap();

    let restored = controller
        .execute_delivery(
            GuestEventDelivery::new(
                64,
                host,
                host,
                GuestEvent::new(
                    GuestEventId::new(8),
                    source,
                    GuestEventKind::RestoreCheckpoint {
                        label: "resume".to_string(),
                    },
                ),
            ),
            &mut executor,
        )
        .unwrap();

    assert_eq!(
        restored,
        vec![SystemActionOutcome::CheckpointRestored {
            tick: 64,
            event: GuestEventId::new(8),
            source,
            manifest,
        }]
    );
    assert_eq!(executor.checkpoints().chunk(&cpu, "pc"), Some(&[0x10][..]));
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
        vec![SystemActionOutcome::StatsDump(StatDumpRecord::new(
            StatDumpId::new(0),
            StatSnapshot::new(
                48,
                1,
                40,
                vec![StatSample::new(insts, "cpu0.committed_insts", "count", 5)],
            ),
        ))]
    );
    assert_eq!(
        controller.action_outcomes(),
        &[
            SystemActionOutcome::StatsReset(StatsResetRecord::new(40, 1, vec![(insts, 11)])),
            SystemActionOutcome::StatsDump(StatDumpRecord::new(
                StatDumpId::new(0),
                StatSnapshot::new(
                    48,
                    1,
                    40,
                    vec![StatSample::new(insts, "cpu0.committed_insts", "count", 5)],
                ),
            )),
        ]
    );
    assert_eq!(
        executor.stats().dump_records(),
        &[StatDumpRecord::new(
            StatDumpId::new(0),
            StatSnapshot::new(
                48,
                1,
                40,
                vec![StatSample::new(insts, "cpu0.committed_insts", "count", 5)],
            ),
        )]
    );
}

#[test]
fn system_run_controller_executes_delivered_execution_mode_switches() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(12);
    let target = ExecutionModeTarget::new("cpu0");
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    let mut controller = SystemRunController::new(HostEventPolicy);

    let first = controller
        .execute_delivery(
            GuestEventDelivery::new(
                52,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(10),
                    source,
                    GuestEventKind::ExecutionModeSwitch {
                        target: target.clone(),
                        mode: ExecutionMode::Functional,
                    },
                ),
            ),
            &mut executor,
        )
        .unwrap();

    assert_eq!(
        first,
        vec![SystemActionOutcome::ExecutionModeSwitched {
            tick: 52,
            event: GuestEventId::new(10),
            source,
            target: target.clone(),
            previous_mode: None,
            mode: ExecutionMode::Functional,
            stats_epoch: 0,
            stats_reset_tick: 0,
        }]
    );
    assert_eq!(
        executor.execution_mode(&target),
        Some(ExecutionMode::Functional)
    );

    let second = controller
        .execute_delivery(
            GuestEventDelivery::new(
                60,
                guest,
                host,
                GuestEvent::new(
                    GuestEventId::new(11),
                    source,
                    GuestEventKind::ExecutionModeSwitch {
                        target: target.clone(),
                        mode: ExecutionMode::Detailed,
                    },
                ),
            ),
            &mut executor,
        )
        .unwrap();

    assert_eq!(
        second,
        vec![SystemActionOutcome::ExecutionModeSwitched {
            tick: 60,
            event: GuestEventId::new(11),
            source,
            target: target.clone(),
            previous_mode: Some(ExecutionMode::Functional),
            mode: ExecutionMode::Detailed,
            stats_epoch: 0,
            stats_reset_tick: 0,
        }]
    );
    assert_eq!(
        executor.execution_mode(&target),
        Some(ExecutionMode::Detailed)
    );
    assert_eq!(
        controller.action_outcomes(),
        &[
            SystemActionOutcome::ExecutionModeSwitched {
                tick: 52,
                event: GuestEventId::new(10),
                source,
                target: target.clone(),
                previous_mode: None,
                mode: ExecutionMode::Functional,
                stats_epoch: 0,
                stats_reset_tick: 0,
            },
            SystemActionOutcome::ExecutionModeSwitched {
                tick: 60,
                event: GuestEventId::new(11),
                source,
                target,
                previous_mode: Some(ExecutionMode::Functional),
                mode: ExecutionMode::Detailed,
                stats_epoch: 0,
                stats_reset_tick: 0,
            },
        ]
    );
}

#[test]
fn execution_mode_switch_outcome_records_stats_scope() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(13);
    let target = ExecutionModeTarget::new("cpu0");
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    stats.increment(insts, 17).unwrap();
    let mut executor = SystemActionExecutor::new(stats);

    assert_eq!(
        executor
            .apply(&HostActionRecord::new(
                40,
                guest,
                host,
                GuestEventId::new(20),
                source,
                HostAction::ResetStats,
            ))
            .unwrap(),
        SystemActionOutcome::StatsReset(StatsResetRecord::new(40, 1, vec![(insts, 17)]))
    );

    assert_eq!(
        executor
            .apply(&HostActionRecord::new(
                52,
                guest,
                host,
                GuestEventId::new(21),
                source,
                HostAction::SwitchExecutionMode {
                    target: target.clone(),
                    mode: ExecutionMode::Timing,
                },
            ))
            .unwrap(),
        SystemActionOutcome::ExecutionModeSwitched {
            tick: 52,
            event: GuestEventId::new(21),
            source,
            target,
            previous_mode: None,
            mode: ExecutionMode::Timing,
            stats_epoch: 1,
            stats_reset_tick: 40,
        }
    );
}

#[test]
fn execution_mode_switches_are_checkpointed_and_restored() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(13);
    let target = ExecutionModeTarget::new("cpu0");
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());

    executor
        .apply(&HostActionRecord::new(
            10,
            guest,
            host,
            GuestEventId::new(20),
            source,
            HostAction::SwitchExecutionMode {
                target: target.clone(),
                mode: ExecutionMode::Functional,
            },
        ))
        .unwrap();

    let checkpoint = executor
        .apply(&HostActionRecord::new(
            12,
            guest,
            host,
            GuestEventId::new(21),
            source,
            HostAction::Checkpoint {
                label: "mode-functional".to_string(),
            },
        ))
        .unwrap();
    let manifest = match checkpoint {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(execution_mode_checkpoint_payload(&manifest).is_some_and(|payload| !payload.is_empty()));

    executor
        .apply(&HostActionRecord::new(
            14,
            guest,
            host,
            GuestEventId::new(22),
            source,
            HostAction::SwitchExecutionMode {
                target: target.clone(),
                mode: ExecutionMode::Detailed,
            },
        ))
        .unwrap();
    assert_eq!(
        executor.execution_mode(&target),
        Some(ExecutionMode::Detailed)
    );

    assert_eq!(
        executor
            .apply(&HostActionRecord::new(
                18,
                guest,
                host,
                GuestEventId::new(23),
                source,
                HostAction::RestoreCheckpoint {
                    manifest: manifest.clone(),
                },
            ))
            .unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 18,
            event: GuestEventId::new(23),
            source,
            manifest,
        }
    );
    assert_eq!(
        executor.execution_mode(&target),
        Some(ExecutionMode::Functional)
    );
}

#[test]
fn failed_execution_mode_restore_does_not_register_checkpoint_component() {
    let guest = PartitionId::new(0);
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(14);
    let component = CheckpointComponentId::new("host.execution_modes").unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    let bad_manifest = CheckpointManifest::new(
        "bad-mode-restore",
        22,
        vec![CheckpointState::new(
            component.clone(),
            vec![CheckpointChunk::new("modes", vec![1, 0, 0])],
        )],
    );

    let error = executor
        .apply(&HostActionRecord::new(
            24,
            guest,
            host,
            GuestEventId::new(24),
            source,
            HostAction::RestoreCheckpoint {
                manifest: bad_manifest,
            },
        ))
        .unwrap_err();

    assert_eq!(
        error,
        SystemError::ExecutionModeCheckpoint(ExecutionModeCheckpointError::InvalidChunk {
            component,
            name: "modes".to_string(),
        })
    );

    let checkpoint = executor
        .apply(&HostActionRecord::new(
            26,
            guest,
            host,
            GuestEventId::new(25),
            source,
            HostAction::Checkpoint {
                label: "after-bad-mode-restore".to_string(),
            },
        ))
        .unwrap();
    let manifest = match checkpoint {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(execution_mode_checkpoint_payload(&manifest).is_none());
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
            SystemActionOutcome::StatsDump(StatDumpRecord::new(
                StatDumpId::new(0),
                StatSnapshot::new(
                    11,
                    1,
                    7,
                    vec![StatSample::new(insts, "cpu0.committed_insts", "count", 4)],
                ),
            )),
        ]
    );
}
