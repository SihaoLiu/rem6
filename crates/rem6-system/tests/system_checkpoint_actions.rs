use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointError, CheckpointManifest,
    CheckpointRegistry, CheckpointState,
};
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, MemoryStoreCheckpointBank,
    MemoryStoreCheckpointError, MemoryStoreCheckpointPort, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, SchedulerCheckpointBank, SchedulerCheckpointError,
    SchedulerCheckpointPort, SystemActionExecutor, SystemActionOutcome, SystemError,
};
use rem6_transport::{MemoryRoute, MemoryTransport, TransportEndpointId};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn memory_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
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

#[test]
fn system_action_executor_checkpoints_and_restores_live_cpu_and_memory_together() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(14);
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let memory_component = CheckpointComponentId::new("memory0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1234);
    let (store, target) = partitioned_memory_store();
    let store = Arc::new(Mutex::new(store));
    let riscv_bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        cpu_component.clone(),
        core.clone(),
    )])
    .unwrap();
    let memory_bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
        memory_component.clone(),
        Arc::clone(&store),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    riscv_bank.register_all(&mut checkpoints).unwrap();
    memory_bank.register_all(&mut checkpoints).unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint_banks(
        StatsRegistry::new(),
        checkpoints,
        riscv_bank,
        memory_bank,
    );

    let checkpoint = HostActionRecord::new(
        108,
        host,
        host,
        GuestEventId::new(11),
        source,
        HostAction::Checkpoint {
            label: "full-system".to_string(),
        },
    );

    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert_eq!(
        manifest
            .states()
            .iter()
            .map(|state| state.component().clone())
            .collect::<Vec<_>>(),
        vec![cpu_component.clone(), memory_component.clone()]
    );
    assert_eq!(
        executor.checkpoints().chunk(&cpu_component, "pc"),
        Some(&0x8040_u64.to_le_bytes()[..])
    );
    assert!(
        executor
            .checkpoints()
            .chunk(&memory_component, "store")
            .unwrap()
            .len()
            > 128
    );

    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0);
    store
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x1000), line_data(0xaa))
        .unwrap();

    let restore = HostActionRecord::new(
        120,
        host,
        host,
        GuestEventId::new(12),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 120,
            event: GuestEventId::new(12),
            source,
            manifest,
        }
    );
    assert_eq!(core.pc(), Address::new(0x8040));
    assert_eq!(core.read_register(reg(1)), 0x1234);
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
fn system_action_executor_rejects_manifest_missing_attached_bank_without_stale_chunk_restore() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(15);
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let memory_component = CheckpointComponentId::new("memory0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x1234);
    let (store, target) = partitioned_memory_store();
    let store = Arc::new(Mutex::new(store));
    let riscv_bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        cpu_component.clone(),
        core.clone(),
    )])
    .unwrap();
    let memory_bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
        memory_component.clone(),
        Arc::clone(&store),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    riscv_bank.register_all(&mut checkpoints).unwrap();
    memory_bank.register_all(&mut checkpoints).unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint_banks(
        StatsRegistry::new(),
        checkpoints,
        riscv_bank,
        memory_bank,
    );
    let checkpoint = HostActionRecord::new(
        121,
        host,
        host,
        GuestEventId::new(13),
        source,
        HostAction::Checkpoint {
            label: "full-system".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let cpu_only_manifest = CheckpointManifest::new(
        "cpu-only-restore",
        manifest.tick(),
        manifest
            .states()
            .iter()
            .filter(|state| state.component() == &cpu_component)
            .cloned()
            .collect(),
    );

    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0);
    store
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x1000), line_data(0xaa))
        .unwrap();
    let restore = HostActionRecord::new(
        122,
        host,
        host,
        GuestEventId::new(14),
        source,
        HostAction::RestoreCheckpoint {
            manifest: cpu_only_manifest,
        },
    );

    let error = executor.apply(&restore).unwrap_err();

    assert_eq!(
        error,
        SystemError::MemoryCheckpoint(MemoryStoreCheckpointError::MissingChunk {
            component: memory_component,
            name: "store".to_string(),
        })
    );
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.read_register(reg(1)), 0);
    assert_eq!(
        store
            .lock()
            .unwrap()
            .line_data(target, Address::new(0x1000))
            .unwrap(),
        line_data(0xaa)
    );
}

#[test]
fn system_action_executor_rejects_cross_bank_invalid_restore_without_partial_live_state() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(16);
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let memory_component = CheckpointComponentId::new("memory0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(1), 0x5678);
    let (store, target) = partitioned_memory_store();
    let store = Arc::new(Mutex::new(store));
    let riscv_bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        cpu_component.clone(),
        core.clone(),
    )])
    .unwrap();
    let memory_bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
        memory_component.clone(),
        Arc::clone(&store),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    riscv_bank.register_all(&mut checkpoints).unwrap();
    memory_bank.register_all(&mut checkpoints).unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint_banks(
        StatsRegistry::new(),
        checkpoints,
        riscv_bank,
        memory_bank,
    );
    let checkpoint = HostActionRecord::new(
        121,
        host,
        host,
        GuestEventId::new(15),
        source,
        HostAction::Checkpoint {
            label: "cross-bank".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let bad_manifest = CheckpointManifest::new(
        manifest.label().to_string(),
        manifest.tick(),
        manifest
            .states()
            .iter()
            .map(|state| {
                if state.component() == &memory_component {
                    CheckpointState::new(
                        memory_component.clone(),
                        vec![CheckpointChunk::new("store", vec![0x99])],
                    )
                } else {
                    state.clone()
                }
            })
            .collect(),
    );

    core.redirect_pc(Address::new(0x9000));
    core.write_register(reg(1), 0);
    store
        .lock()
        .unwrap()
        .insert_line(target, Address::new(0x1000), line_data(0xaa))
        .unwrap();
    let restore = HostActionRecord::new(
        122,
        host,
        host,
        GuestEventId::new(16),
        source,
        HostAction::RestoreCheckpoint {
            manifest: bad_manifest,
        },
    );

    let error = executor.apply(&restore).unwrap_err();

    assert!(matches!(error, SystemError::MemoryCheckpoint(_)));
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.read_register(reg(1)), 0);
    assert_eq!(
        store
            .lock()
            .unwrap()
            .line_data(target, Address::new(0x1000))
            .unwrap(),
        line_data(0xaa)
    );
}

#[test]
fn system_action_executor_rejects_empty_checkpoint_label_without_chunk_writes() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(17);
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    let riscv_bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        cpu_component.clone(),
        core.clone(),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_checkpoint_bank(riscv_bank).unwrap();

    let checkpoint = HostActionRecord::new(
        122,
        host,
        host,
        GuestEventId::new(17),
        source,
        HostAction::Checkpoint {
            label: String::new(),
        },
    );

    let error = executor.apply(&checkpoint).unwrap_err();

    assert_eq!(error, SystemError::Checkpoint(CheckpointError::EmptyLabel));
    assert_eq!(executor.checkpoints().chunk(&cpu_component, "pc"), None);
    assert_eq!(executor.checkpoints().chunk(&cpu_component, "xregs"), None);
}

#[test]
fn system_action_executor_preflights_scheduler_quiescence_before_checkpoint_writes() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(18);
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let scheduler_component = CheckpointComponentId::new("scheduler0").unwrap();
    let core = riscv_core(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    core.redirect_pc(Address::new(0x8040));
    let riscv_bank = RiscvCoreCheckpointBank::new([RiscvCoreCheckpointPort::new(
        cpu_component.clone(),
        core.clone(),
    )])
    .unwrap();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
    ));
    scheduler
        .lock()
        .unwrap()
        .schedule_parallel_at(PartitionId::new(0), 7, |_| {})
        .unwrap();
    let scheduler_bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
        scheduler_component.clone(),
        Arc::clone(&scheduler),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_riscv_checkpoint_bank(riscv_bank).unwrap();
    executor
        .attach_scheduler_checkpoint_bank(scheduler_bank)
        .unwrap();

    let checkpoint = HostActionRecord::new(
        123,
        host,
        host,
        GuestEventId::new(18),
        source,
        HostAction::Checkpoint {
            label: "scheduler-not-ready".to_string(),
        },
    );

    let error = executor.apply(&checkpoint).unwrap_err();

    let SystemError::SchedulerCheckpoint(SchedulerCheckpointError::NonQuiescent { report }) = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(report.component(), &scheduler_component);
    assert_eq!(report.pending_event_count(), 1);
    assert_eq!(report.first_pending_tick(), Some(7));
    assert_eq!(executor.checkpoints().chunk(&cpu_component, "pc"), None);
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&scheduler_component, "scheduler"),
        None
    );
}
