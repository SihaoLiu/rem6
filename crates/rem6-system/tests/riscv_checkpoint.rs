use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointRegistry, CheckpointState,
};
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    BiModeBranchPredictor, BiModeBranchPredictorCheckpointPayload, BiModeBranchPredictorConfig,
    BiModeBranchPredictorError, BranchPredictor, BranchPredictorCheckpointPayload,
    BranchPredictorConfig, BranchPredictorError, CpuCore, CpuFetchConfig, CpuId, CpuResetState,
    GShareBranchPredictor, GShareBranchPredictorCheckpointPayload, GShareBranchPredictorConfig,
    GShareBranchPredictorError, InOrderPipelineSnapshot, RiscvCore, RiscvHartRunState,
};
use rem6_isa_riscv::{FloatRegister, Register, RiscvPmpAddressMode, RiscvPmpConfig};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_system::{RiscvCoreCheckpointBank, RiscvCoreCheckpointPort, RiscvCoreCheckpointRecord};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
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

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn b_type(offset: i32, rs1: u8, rs2: u8, funct3: u32) -> u32 {
    let imm = offset as u32;
    ((imm & 0x1000) << 19)
        | ((imm & 0x07e0) << 20)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x001e) << 7)
        | ((imm & 0x0800) >> 4)
        | 0x63
}

fn tor_config() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
        .with_read(true)
        .with_execute(true)
}

fn riscv_core() -> RiscvCore {
    riscv_core_with(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000)
}

fn riscv_core_with(cpu: CpuId, partition: PartitionId, agent: AgentId, entry: u64) -> RiscvCore {
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

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(entry),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), instruction.to_le_bytes().to_vec())
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(rem6_transport::RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome
       + Send
       + 'static {
    move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    }
}

fn fetch_and_execute_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_fetch(
        scheduler,
        transport,
        MemoryTrace::new(),
        responder(Arc::clone(&store)),
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
    core.execute_next_completed_fetch().unwrap().unwrap();
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_pc_and_integer_registers() {
    let core = riscv_core();
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1122_3344_5566_7788);
    core.write_register(reg(5), 0x55aa);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let expected_pmp = core.pmp_snapshot();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        RiscvCoreCheckpointRecord::new(
            component.clone(),
            Address::new(0x8040),
            (0..32)
                .map(|index| {
                    let register = reg(index);
                    (register, core.read_register(register))
                })
                .collect(),
            expected_pmp,
        )
    );
    assert_eq!(
        registry.chunk(&component, "pc"),
        Some(&0x8040_u64.to_le_bytes()[..])
    );
    let xregs = registry.chunk(&component, "xregs").unwrap();
    assert_eq!(xregs.len(), 32 * 8);
    assert_eq!(&xregs[8..16], &0x1122_3344_5566_7788_u64.to_le_bytes());
    assert_eq!(&xregs[40..48], &0x55aa_u64.to_le_bytes());

    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 1);
    core.write_register(reg(5), 5);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.pc(), Address::new(0x8040));
    assert_eq!(core.read_register(reg(0)), 0);
    assert_eq!(core.read_register(reg(1)), 0x1122_3344_5566_7788);
    assert_eq!(core.read_register(reg(5)), 0x55aa);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_float_registers() {
    let core = riscv_core();
    core.write_float_register(freg(1), 1.5f64.to_bits());
    core.write_float_register(freg(5), 0x55aa);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(captured.float_register(freg(1)), Some(1.5f64.to_bits()));
    assert_eq!(captured.float_register(freg(5)), Some(0x55aa));
    let fregs = registry.chunk(&component, "fregs").unwrap();
    assert_eq!(fregs.len(), 32 * 8);
    assert_eq!(&fregs[8..16], &1.5f64.to_bits().to_le_bytes());
    assert_eq!(&fregs[40..48], &0x55aa_u64.to_le_bytes());

    core.write_float_register(freg(1), 0);
    core.write_float_register(freg(5), 0);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.read_float_register(freg(1)), 1.5f64.to_bits());
    assert_eq!(core.read_float_register(freg(5)), 0x55aa);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_in_order_pipeline_state() {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let mut registry = CheckpointRegistry::new();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let timed_core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let timed_port = RiscvCoreCheckpointPort::new(component.clone(), timed_core.clone());

    fetch_and_execute_one(
        &timed_core,
        loaded_store(0x8000, i_type(5, 0, 0, 1, 0x13)),
        &mut scheduler,
        &transport,
    );
    let captured_pipeline = timed_core.in_order_pipeline_snapshot();

    timed_port.register(&mut registry).unwrap();
    let captured = timed_port.capture_into(&mut registry).unwrap();

    assert_eq!(captured.in_order_pipeline_snapshot(), &captured_pipeline);
    assert!(registry.chunk(&component, "in-order-pipeline").is_some());

    fetch_and_execute_one(
        &timed_core,
        loaded_store(0x8004, i_type(7, 1, 0, 2, 0x13)),
        &mut scheduler,
        &transport,
    );
    assert_ne!(timed_core.in_order_pipeline_snapshot(), captured_pipeline);

    let restored = timed_port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(timed_core.in_order_pipeline_snapshot(), captured_pipeline);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_branch_predictor_state() {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let mut registry = CheckpointRegistry::new();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());

    fetch_and_execute_one(
        &core,
        loaded_store(0x8000, b_type(8, 0, 0, 0x0)),
        &mut scheduler,
        &transport,
    );
    let captured_predictor = core.branch_predictor_snapshot();
    let captured_btb = core.branch_target_buffer_snapshot();
    assert_eq!(captured_predictor.update_count(), 1);
    assert_eq!(captured_btb.update_count(), 1);

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert!(registry.chunk(&component, "branch-predictor").is_some());

    fetch_and_execute_one(
        &core,
        loaded_store(0x8008, b_type(8, 0, 0, 0x0)),
        &mut scheduler,
        &transport,
    );
    assert_ne!(core.branch_predictor_snapshot(), captured_predictor);
    assert_ne!(core.branch_target_buffer_snapshot(), captured_btb);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.branch_predictor_snapshot(), captured_predictor);
    assert_eq!(core.branch_target_buffer_snapshot(), captured_btb);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_gshare_predictor_state() {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let mut registry = CheckpointRegistry::new();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());

    fetch_and_execute_one(
        &core,
        loaded_store(0x8000, b_type(8, 0, 0, 0x0)),
        &mut scheduler,
        &transport,
    );
    let captured_gshare = core.gshare_branch_predictor_snapshot();
    assert_eq!(captured_gshare.update_count(), 1);

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert!(registry
        .chunk(&component, "gshare-branch-predictor")
        .is_some());

    fetch_and_execute_one(
        &core,
        loaded_store(0x8008, b_type(8, 0, 0, 0x0)),
        &mut scheduler,
        &transport,
    );
    assert_ne!(core.gshare_branch_predictor_snapshot(), captured_gshare);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.gshare_branch_predictor_snapshot(), captured_gshare);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_bimode_predictor_state() {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let mut registry = CheckpointRegistry::new();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());

    fetch_and_execute_one(
        &core,
        loaded_store(0x8000, b_type(8, 0, 0, 0x0)),
        &mut scheduler,
        &transport,
    );
    let captured_bimode = core.bimode_branch_predictor_snapshot();
    assert_eq!(captured_bimode.update_count(), 1);

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert!(registry
        .chunk(&component, "bimode-branch-predictor")
        .is_some());

    fetch_and_execute_one(
        &core,
        loaded_store(0x8008, b_type(8, 0, 0, 0x0)),
        &mut scheduler,
        &transport,
    );
    assert_ne!(core.bimode_branch_predictor_snapshot(), captured_bimode);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.bimode_branch_predictor_snapshot(), captured_bimode);
}

#[test]
fn riscv_core_checkpoint_rejects_incompatible_branch_predictor_without_partial_restore() {
    let core = riscv_core();
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();
    let incompatible_payload = BranchPredictorCheckpointPayload::from_snapshot(
        BranchPredictor::new(BranchPredictorConfig::new(8).unwrap()).snapshot(),
        std::iter::empty::<(u64, rem6_cpu::BranchSpeculationId)>(),
    )
    .unwrap()
    .encode();

    port.register(&mut registry).unwrap();
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1111);
    port.capture_into(&mut registry).unwrap();
    registry
        .write_chunk(&component, "branch-predictor", incompatible_payload)
        .unwrap();
    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0x2222);

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        rem6_system::RiscvCoreCheckpointError::InvalidBranchPredictorSnapshot {
            component,
            error: BranchPredictorError::SnapshotTableEntriesMismatch {
                expected: 1024,
                actual: 8,
            },
        }
    );
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.read_register(reg(1)), 0x2222);
}

#[test]
fn riscv_core_checkpoint_rejects_incompatible_gshare_predictor_without_partial_restore() {
    let core = riscv_core();
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();
    let incompatible_payload = GShareBranchPredictorCheckpointPayload::from_snapshot(
        GShareBranchPredictor::new(GShareBranchPredictorConfig::new(1, 8).unwrap()).snapshot(),
    )
    .unwrap()
    .encode();

    port.register(&mut registry).unwrap();
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1111);
    port.capture_into(&mut registry).unwrap();
    registry
        .write_chunk(&component, "gshare-branch-predictor", incompatible_payload)
        .unwrap();
    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0x2222);

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        rem6_system::RiscvCoreCheckpointError::InvalidGShareBranchPredictorSnapshot {
            component,
            error: GShareBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: 1,
                actual_threads: 1,
                expected_entries: 1024,
                actual_entries: 8,
            },
        }
    );
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.read_register(reg(1)), 0x2222);
}

#[test]
fn riscv_core_checkpoint_rejects_incompatible_bimode_predictor_without_partial_restore() {
    let core = riscv_core();
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();
    let incompatible_payload = BiModeBranchPredictorCheckpointPayload::from_snapshot(
        BiModeBranchPredictor::new(BiModeBranchPredictorConfig::new(1, 8, 8).unwrap()).snapshot(),
    )
    .unwrap()
    .encode();

    port.register(&mut registry).unwrap();
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1111);
    port.capture_into(&mut registry).unwrap();
    registry
        .write_chunk(&component, "bimode-branch-predictor", incompatible_payload)
        .unwrap();
    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0x2222);

    let error = port.restore_from(&registry).unwrap_err();

    assert_eq!(
        error,
        rem6_system::RiscvCoreCheckpointError::InvalidBiModeBranchPredictorSnapshot {
            component,
            error: BiModeBranchPredictorError::SnapshotShapeMismatch {
                expected_threads: 1,
                actual_threads: 1,
                expected_choice_entries: 1024,
                actual_choice_entries: 8,
                expected_global_entries: 1024,
                actual_global_entries: 8,
            },
        }
    );
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.read_register(reg(1)), 0x2222);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_hart_run_state() {
    let core = riscv_core();
    core.set_hart_stopped();
    assert_checkpoint_restores_hart_run_state(&core, RiscvHartRunState::Stopped, 1);
    core.set_hart_suspended();
    assert_checkpoint_restores_hart_run_state(&core, RiscvHartRunState::Suspended, 2);
    core.set_hart_start_pending();
    assert_checkpoint_restores_hart_run_state(&core, RiscvHartRunState::StartPending, 3);
    core.set_hart_stop_pending();
    assert_checkpoint_restores_hart_run_state(&core, RiscvHartRunState::StopPending, 4);
    core.set_hart_suspend_pending();
    assert_checkpoint_restores_hart_run_state(&core, RiscvHartRunState::SuspendPending, 5);
    core.set_hart_resume_pending();
    assert_checkpoint_restores_hart_run_state(&core, RiscvHartRunState::ResumePending, 6);
}

fn assert_checkpoint_restores_hart_run_state(
    core: &RiscvCore,
    expected_state: RiscvHartRunState,
    expected_encoding: u8,
) {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(captured.hart_run_state(), expected_state);
    assert_eq!(
        registry.chunk(&component, "hart-run-state"),
        Some(&[expected_encoding][..])
    );

    core.set_hart_started();
    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core.hart_run_state(), expected_state);
}

#[test]
fn riscv_core_checkpoint_restore_without_float_register_chunk_zeros_float_registers() {
    let core = riscv_core();
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();

    registry.register(component.clone()).unwrap();
    registry
        .restore(&rem6_checkpoint::CheckpointManifest::new(
            "legacy-riscv",
            0,
            vec![CheckpointState::new(
                component.clone(),
                vec![
                    CheckpointChunk::new("pc", 0x8040_u64.to_le_bytes().to_vec()),
                    CheckpointChunk::new("xregs", vec![0; 32 * 8]),
                    CheckpointChunk::new("pmp", {
                        let mut pmp = 16_u16.to_le_bytes().to_vec();
                        pmp.resize(2 + 16 * 9, 0);
                        pmp
                    }),
                ],
            )],
        ))
        .unwrap();
    core.write_float_register(freg(1), 0x1122);
    core.set_hart_stopped();
    let default_pipeline = RiscvCore::default_in_order_pipeline_snapshot();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        default_pipeline.config().clone(),
        9,
        [],
    ))
    .unwrap();
    assert_ne!(core.in_order_pipeline_snapshot(), default_pipeline);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored.float_register(freg(1)), Some(0));
    assert_eq!(restored.hart_run_state(), RiscvHartRunState::Started);
    assert_eq!(core.pc(), Address::new(0x8040));
    assert_eq!(core.read_float_register(freg(1)), 0);
    assert_eq!(core.hart_run_state(), RiscvHartRunState::Started);
    assert_eq!(core.in_order_pipeline_snapshot(), default_pipeline);
}

#[test]
fn riscv_core_checkpoint_captures_and_restores_pmp_entries() {
    let core = riscv_core();
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();
    let config = tor_config();
    let raw_addr = 0x2000_u64 >> 2;

    core.write_pmp_addr(0, raw_addr).unwrap();
    core.write_pmp_config(0, config).unwrap();
    let expected_pmp = core.pmp_snapshot();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(captured.pmp_snapshot(), &expected_pmp);
    let pmp = registry.chunk(&component, "pmp").unwrap();
    assert_eq!(pmp.len(), 2 + core.pmp_entry_count() * 9);
    assert_eq!(&pmp[0..2], &(core.pmp_entry_count() as u16).to_le_bytes());
    assert_eq!(&pmp[2..10], &raw_addr.to_le_bytes());
    assert_eq!(pmp[10], config.bits());

    core.write_pmp_config(0, RiscvPmpConfig::default()).unwrap();
    core.write_pmp_addr(0, 0).unwrap();

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored.pmp_snapshot(), &expected_pmp);
    assert_eq!(core.pmp_snapshot(), expected_pmp);
}

#[test]
fn riscv_core_checkpoint_bank_captures_and_restores_cores_in_component_order() {
    let core0 = riscv_core_with(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    let core1 = riscv_core_with(CpuId::new(1), PartitionId::new(1), AgentId::new(8), 0x9000);
    core0.redirect_pc(Address::new(0x8040));
    core0.write_register(reg(1), 0x1111);
    core1.redirect_pc(Address::new(0x9040));
    core1.write_register(reg(2), 0x2222);
    let component0 = CheckpointComponentId::new("cpu0").unwrap();
    let component1 = CheckpointComponentId::new("cpu1").unwrap();
    let bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(component1.clone(), core1.clone()),
        RiscvCoreCheckpointPort::new(component0.clone(), core0.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    let captured = bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(
        bank.components(),
        vec![component0.clone(), component1.clone()]
    );
    assert_eq!(
        captured
            .iter()
            .map(|record| record.component().clone())
            .collect::<Vec<_>>(),
        vec![component0.clone(), component1.clone()]
    );
    let manifest = registry.capture("multi-core", 48).unwrap();
    assert_eq!(
        manifest
            .states()
            .iter()
            .map(|state| state.component().clone())
            .collect::<Vec<_>>(),
        vec![component0.clone(), component1.clone()]
    );
    assert_eq!(
        registry.chunk(&component0, "pc"),
        Some(&0x8040_u64.to_le_bytes()[..])
    );
    assert_eq!(
        registry.chunk(&component1, "pc"),
        Some(&0x9040_u64.to_le_bytes()[..])
    );

    core0.redirect_pc(Address::new(0xa000));
    core0.write_register(reg(1), 0);
    core1.redirect_pc(Address::new(0xb000));
    core1.write_register(reg(2), 0);

    let restored = bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core0.pc(), Address::new(0x8040));
    assert_eq!(core0.read_register(reg(1)), 0x1111);
    assert_eq!(core1.pc(), Address::new(0x9040));
    assert_eq!(core1.read_register(reg(2)), 0x2222);
}

#[test]
fn riscv_core_checkpoint_bank_rejects_truncated_payload_without_partial_restore() {
    let core0 = riscv_core_with(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    let core1 = riscv_core_with(CpuId::new(1), PartitionId::new(1), AgentId::new(8), 0x9000);
    core0.redirect_pc(Address::new(0x8040));
    core0.write_register(reg(1), 0x1111);
    core1.redirect_pc(Address::new(0x9040));
    core1.write_register(reg(2), 0x2222);
    let component0 = CheckpointComponentId::new("cpu0").unwrap();
    let component1 = CheckpointComponentId::new("cpu1").unwrap();
    let bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(component0.clone(), core0.clone()),
        RiscvCoreCheckpointPort::new(component1.clone(), core1.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    registry
        .write_chunk(&component1, "xregs", vec![0xaa, 0xbb, 0xcc])
        .unwrap();
    core0.redirect_pc(Address::new(0xa000));
    core0.write_register(reg(1), 0xa111);
    core1.redirect_pc(Address::new(0xb000));
    core1.write_register(reg(2), 0xb222);

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(
        error,
        rem6_system::RiscvCoreCheckpointError::InvalidChunkSize {
            component: component1,
            name: "xregs".to_string(),
            expected: 32 * 8,
            actual: 3,
        }
    );
    assert_eq!(core0.pc(), Address::new(0xa000));
    assert_eq!(core0.read_register(reg(1)), 0xa111);
    assert_eq!(core1.pc(), Address::new(0xb000));
    assert_eq!(core1.read_register(reg(2)), 0xb222);
}

#[test]
fn riscv_core_checkpoint_bank_rejects_mismatched_pmp_count_without_partial_restore() {
    let core0 = riscv_core_with(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    let core1 = riscv_core_with(CpuId::new(1), PartitionId::new(1), AgentId::new(8), 0x9000);
    let component0 = CheckpointComponentId::new("cpu0").unwrap();
    let component1 = CheckpointComponentId::new("cpu1").unwrap();
    let bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(component0.clone(), core0.clone()),
        RiscvCoreCheckpointPort::new(component1.clone(), core1.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    core0.write_pmp_addr(0, 0x2000 >> 2).unwrap();
    core0.write_pmp_config(0, tor_config()).unwrap();
    core1.write_pmp_addr(0, 0x3000 >> 2).unwrap();
    core1.write_pmp_config(0, tor_config()).unwrap();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();

    let mut mismatched_pmp = 15_u16.to_le_bytes().to_vec();
    mismatched_pmp.resize(2 + 15 * 9, 0);
    registry
        .write_chunk(&component1, "pmp", mismatched_pmp)
        .unwrap();
    core0
        .write_pmp_config(0, RiscvPmpConfig::default())
        .unwrap();
    core0.write_pmp_addr(0, 0).unwrap();
    core1
        .write_pmp_config(0, RiscvPmpConfig::default())
        .unwrap();
    core1.write_pmp_addr(0, 0).unwrap();
    let core0_before = core0.pmp_snapshot();
    let core1_before = core1.pmp_snapshot();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(
        error,
        rem6_system::RiscvCoreCheckpointError::InvalidPmpEntryCount {
            component: component1,
            expected: 16,
            actual: 15,
        }
    );
    assert_eq!(core0.pmp_snapshot(), core0_before);
    assert_eq!(core1.pmp_snapshot(), core1_before);
}
