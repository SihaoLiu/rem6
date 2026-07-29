use std::collections::{BTreeMap, BTreeSet};

use rem6_cpu::{
    CpuFetchEvent, CpuFetchRecord, RiscvO3LiveCheckpointEvent,
    RiscvO3LiveCheckpointFinalizedWriteback, RiscvO3LiveCheckpointIssueRow,
    RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointProfile, RiscvO3LiveCheckpointService,
    RiscvO3LiveCheckpointTelemetry, RiscvO3LiveCheckpointWake,
};
use rem6_isa_riscv::RegisterWrite;
use rem6_kernel::ScheduledEventKind;
use rem6_memory::MemoryRequestId;
use rem6_transport::MemoryRouteId;

use super::*;
use crate::live_o3_support::{
    core, seed_live_core, seed_live_core_at, SeededLiveCore, LIVE_TICK, O3LC, O3LH, O3RT,
};
use crate::pending_address_support::{seed_pending_address_core, STORE_SEQUENCE};

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_rejects_malformed_o3lc_before_mutating_destination() {
    let (destination, error) = restore_with_o3lc(b"O3LC\x01".to_vec());
    assert!(error.to_string().contains("O3 live checkpoint profile"), "unexpected checkpoint error: {error}");
    assert_sentinel_state(&destination);
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_rejects_o3lc_without_source_hart_state() {
    let (destination, error) = restore_with_o3lc(valid_compute_o3lc());
    assert!(matches!(error, RiscvCoreCheckpointError::O3LiveCheckpointRequiresHartState { .. }));
    assert_sentinel_state(&destination);
}

#[rustfmt::skip]
fn captured_live(kind: ScheduledEventKind) -> (SeededLiveCore, CheckpointComponentId, CheckpointRegistry) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let source = seed_live_core(0, &mut scheduler, kind);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), source.core.clone());
    let mut registry = CheckpointRegistry::new(); port.register(&mut registry).unwrap(); port.capture_into_at(&mut registry, LIVE_TICK).unwrap();
    (source, component, registry)
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_writes_o3lc_only_for_supported_live_state() {
    let (source, component, registry) = captured_live(ScheduledEventKind::Parallel);
    assert_eq!(source.wake.kind(), ScheduledEventKind::Parallel);
    assert!(registry.chunk(&component, O3LC).is_some());
    assert!(registry.chunk(&component, O3LH).is_some());
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(registry.chunk(&component, O3LC).unwrap()).unwrap(), source.live);
    let destination = compatible_core();
    RiscvCoreCheckpointPort::new(component, destination.clone()).restore_from(&registry).unwrap();
    assert_eq!(destination.checkpoint_hart_state(), source.core.checkpoint_hart_state());
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_keeps_legacy_drained_record_without_o3lc() {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core(0));
    let mut registry = CheckpointRegistry::new(); port.register(&mut registry).unwrap(); port.capture_into(&mut registry).unwrap();
    let before = registry.capture("before", 0).unwrap().states()[0].chunks().to_vec();
    registry.write_chunk(&component, O3LC, vec![1]).unwrap(); registry.write_chunk(&component, O3LH, vec![2]).unwrap();
    port.capture_into(&mut registry).unwrap();
    assert_eq!(registry.capture("after", 0).unwrap().states()[0].chunks(), before);
    assert!(registry.chunk(&component, O3LC).is_none()); assert!(registry.chunk(&component, O3LH).is_none());
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_rejects_o3lc_without_o3rt_or_with_o3dh() {
    let (_, component, mut registry) = captured_live(ScheduledEventKind::Serial);
    let destination = RiscvCoreCheckpointPort::new(component.clone(), core(0));
    let mut missing_runtime = registry.clone(); missing_runtime.remove_chunk(&component, O3RT);
    assert!(matches!(destination.restore_from(&missing_runtime).unwrap_err(), RiscvCoreCheckpointError::O3LiveCheckpointRequiresRuntime { .. }));
    registry.write_chunk(&component, RISCV_O3_LIVE_DATA_HANDOFF_CHUNK, vec![0]).unwrap();
    assert!(matches!(destination.restore_from(&registry).unwrap_err(), RiscvCoreCheckpointError::O3LiveCheckpointConflictsWithDataHandoff { .. }));
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_bank_prepares_all_cores_before_first_install() {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let live = seed_live_core(0, &mut scheduler, ScheduledEventKind::Serial);
    let cpu0 = CheckpointComponentId::new("cpu0").unwrap(); let cpu1 = CheckpointComponentId::new("cpu1").unwrap();
    let source_bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(cpu0.clone(), core(1)),
        RiscvCoreCheckpointPort::new(cpu1.clone(), live.core),
    ]).unwrap();
    let mut registry = CheckpointRegistry::new(); source_bank.register_all(&mut registry).unwrap(); source_bank.capture_all_into_at(&mut registry, LIVE_TICK).unwrap();
    let mut invalid = RiscvO3LiveCheckpointPayload::decode(registry.chunk(&cpu1, O3LC).unwrap()).unwrap(); invalid.issue_rows[0].sequence = 999;
    registry.write_chunk(&cpu1, O3LC, invalid.encode().unwrap()).unwrap();
    let first = core(1); first.write_register(reg(7), 0xdead_beef);
    let second = compatible_core(); second.write_register(reg(8), 0xcafe_babe);
    let before = [core_restore_sentinel(&first), core_restore_sentinel(&second)];
    let destination = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(cpu0, first.clone()),
        RiscvCoreCheckpointPort::new(cpu1, second.clone()),
    ]).unwrap();
    assert!(destination.restore_all_from(&registry).is_err());
    assert_eq!([core_restore_sentinel(&first), core_restore_sentinel(&second)], before);
    assert_eq!(first.read_register(reg(7)), 0xdead_beef); assert_eq!(second.read_register(reg(8)), 0xcafe_babe);
}

#[test]
#[rustfmt::skip]
fn pending_address_second_bank_corruption_mutates_no_core_or_scheduler() {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let source0 = seed_pending_address_core(0, &mut scheduler, ScheduledEventKind::Serial);
    let source1 = seed_pending_address_core(1, &mut scheduler, ScheduledEventKind::Parallel);
    let cpu0 = CheckpointComponentId::new("cpu0").unwrap(); let cpu1 = CheckpointComponentId::new("cpu1").unwrap();
    let source_bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(cpu0.clone(), source0.core),
        RiscvCoreCheckpointPort::new(cpu1.clone(), source1.core),
    ]).unwrap();
    let mut registry = CheckpointRegistry::new(); source_bank.register_all(&mut registry).unwrap(); source_bank.capture_all_into_at(&mut registry, LIVE_TICK).unwrap();
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(registry.chunk(&cpu0, O3LC).unwrap()).unwrap(), source0.live);
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(registry.chunk(&cpu1, O3LC).unwrap()).unwrap(), source1.live);
    assert_eq!(O3RuntimeCheckpointPayload::decode(registry.chunk(&cpu0, O3RT).unwrap()).unwrap(), source0.stable);
    assert_eq!(O3RuntimeCheckpointPayload::decode(registry.chunk(&cpu1, O3RT).unwrap()).unwrap(), source1.stable);
    let mut invalid = RiscvO3LiveCheckpointPayload::decode(registry.chunk(&cpu1, O3LC).unwrap()).unwrap();
    let corrupt_sequence = STORE_SEQUENCE + 1;
    invalid.pending_address.as_mut().unwrap().sequence = corrupt_sequence;
    invalid.issue_rows[0].sequence = corrupt_sequence;
    invalid.resident_sequences[0] = corrupt_sequence;
    registry.write_chunk(&cpu1, O3LC, invalid.encode().unwrap()).unwrap();

    scheduler.checkpoint_access().discard_exact_events(&[source0.wake, source1.wake]).unwrap();
    let destination0 = seed_pending_address_core(0, &mut scheduler, ScheduledEventKind::Parallel);
    let destination1 = seed_pending_address_core(1, &mut scheduler, ScheduledEventKind::Serial);
    destination0.core.write_register(reg(7), 0xdead_beef); destination1.core.write_register(reg(8), 0xcafe_babe);
    let cores_before = [core_restore_sentinel(&destination0.core), core_restore_sentinel(&destination1.core)];
    let scheduler_before = scheduler.snapshot(); let registry_before = registry.clone();
    let destination_bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(cpu0, destination0.core.clone()),
        RiscvCoreCheckpointPort::new(cpu1, destination1.core.clone()),
    ]).unwrap();

    assert!(destination_bank.restore_all_from(&registry).is_err());

    assert_eq!([core_restore_sentinel(&destination0.core), core_restore_sentinel(&destination1.core)], cores_before);
    assert_eq!(destination0.core.read_register(reg(7)), 0xdead_beef); assert_eq!(destination1.core.read_register(reg(8)), 0xcafe_babe);
    assert_eq!(scheduler.snapshot(), scheduler_before); assert_eq!(registry, registry_before);
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_bank_rejects_aliased_core_storage() {
    let cpu0 = CheckpointComponentId::new("cpu0").unwrap(); let cpu1 = CheckpointComponentId::new("cpu1").unwrap();
    let shared = core(0);
    let error = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(cpu0, shared.clone()),
        RiscvCoreCheckpointPort::new(cpu1.clone(), shared),
    ]).unwrap_err();
    assert_eq!(error, CheckpointError::DuplicateComponent { component: cpu1 });
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_rejects_live_capture_without_explicit_tick() {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let source = seed_live_core_at(0, &mut scheduler, ScheduledEventKind::Serial, 0, 0);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), source.core);
    let bank = RiscvCoreCheckpointBank::new([port.clone()]).unwrap();
    let mut registry = CheckpointRegistry::new(); port.register(&mut registry).unwrap();
    assert!(matches!(port.capture_into(&mut registry), Err(CheckpointError::ComponentNotQuiescent { .. })));
    assert!(matches!(bank.capture_all_into(&mut registry), Err(CheckpointError::ComponentNotQuiescent { .. })));
    assert!(registry.chunk(&component, O3LC).is_none());
}

#[test]
fn riscv_checkpoint_rejects_noncanonical_and_malformed_o3lh() {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let source = seed_live_core(0, &mut scheduler, ScheduledEventKind::Serial);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), source.core);
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();
    port.capture_into_at(&mut registry, LIVE_TICK).unwrap();
    let canonical = registry.chunk(&component, O3LH).unwrap().to_vec();
    let mut nonzero_x0 = canonical.clone();
    nonzero_x0[216..224].copy_from_slice(&1_u64.to_le_bytes());
    let mut noncanonical_mcountinhibit = canonical.clone();
    noncanonical_mcountinhibit[128..136].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut trailing = canonical.clone();
    trailing.push(0);

    for payload in [nonzero_x0, noncanonical_mcountinhibit] {
        let mut corrupted = registry.clone();
        corrupted.write_chunk(&component, O3LH, payload).unwrap();
        let destination = compatible_core();
        destination.write_register(reg(7), 0xfeed);
        assert!(matches!(
            RiscvCoreCheckpointPort::new(component.clone(), destination.clone())
                .restore_from(&corrupted),
            Err(RiscvCoreCheckpointError::InvalidO3LiveHartState { .. })
        ));
        assert_eq!(destination.read_register(reg(7)), 0xfeed);
    }
    for payload in [canonical[..canonical.len() - 1].to_vec(), trailing] {
        let mut corrupted = registry.clone();
        corrupted.write_chunk(&component, O3LH, payload).unwrap();
        let destination = compatible_core();
        destination.write_register(reg(7), 0xfeed);
        assert!(
            RiscvCoreCheckpointPort::new(component.clone(), destination.clone())
                .restore_from(&corrupted)
                .is_err()
        );
        assert_eq!(destination.read_register(reg(7)), 0xfeed);
    }
}

#[rustfmt::skip]
fn compatible_core() -> RiscvCore {
    RiscvCore::new(CpuCore::new(
        CpuResetState::new(CpuId::new(9), PartitionId::new(0), AgentId::new(7), Address::new(0x8000)),
        CpuFetchConfig::new(endpoint("cpu0.ifetch"), MemoryRouteId::new(0), layout(), AccessSize::new(4).unwrap()),
    ).unwrap())
}

#[derive(Debug, Eq, PartialEq)]
struct CoreRestoreSentinel {
    hart: rem6_isa_riscv::RiscvHartState,
    riscv_pc: Address,
    cpu_pc: Address,
    next_sequence: u64,
    fetch_events: Vec<CpuFetchEvent>,
    runtime: O3RuntimeSnapshot,
    stats: rem6_cpu::O3RuntimeStats,
    wakes: Vec<(
        rem6_kernel::SchedulerInstanceId,
        rem6_kernel::PendingEventSnapshot,
    )>,
}

fn core_restore_sentinel(core: &RiscvCore) -> CoreRestoreSentinel {
    let cpu = core.inner();
    CoreRestoreSentinel {
        hart: core.checkpoint_hart_state(),
        riscv_pc: core.pc(),
        cpu_pc: cpu.pc(),
        next_sequence: cpu.next_sequence(),
        fetch_events: cpu.fetch_events(),
        runtime: core.o3_runtime_snapshot(),
        stats: core.o3_runtime_stats(),
        wakes: core.owned_o3_writeback_wakes(),
    }
}

fn restore_with_o3lc(payload: Vec<u8>) -> (RiscvCore, RiscvCoreCheckpointError) {
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let source_port = RiscvCoreCheckpointPort::new(component.clone(), riscv_core());
    let mut registry = CheckpointRegistry::new();
    source_port.register(&mut registry).unwrap();
    source_port.capture_into(&mut registry).unwrap();
    registry
        .write_chunk(&component, "o3-live-checkpoint", payload)
        .unwrap();

    let destination = riscv_core();
    destination.redirect_pc(Address::new(0xdead_beef));
    destination.write_register(reg(7), 0x1122_3344_5566_7788);
    destination.write_float_register(freg(9), 0x8877_6655_4433_2211);
    let port = RiscvCoreCheckpointPort::new(component, destination.clone());
    let error = port.restore_from(&registry).unwrap_err();
    (destination, error)
}

#[rustfmt::skip]
fn assert_sentinel_state(destination: &RiscvCore) {
    assert_eq!(destination.pc(), Address::new(0xdead_beef));
    assert_eq!(destination.read_register(reg(7)), 0x1122_3344_5566_7788);
    assert_eq!(destination.read_float_register(freg(9)), 0x8877_6655_4433_2211);
}

#[rustfmt::skip]
fn valid_compute_o3lc() -> Vec<u8> {
    let request = MemoryRequestId::new(AgentId::new(7), 1);
    let fetch = CpuFetchRecord::new(
        20, PartitionId::new(0), MemoryRouteId::new(0), endpoint("cpu0.ifetch"),
        request, Address::new(0x8000), AccessSize::new(4).unwrap(),
    );
    let event = RiscvO3LiveCheckpointEvent {
        fetch: CpuFetchEvent::completed(fetch, 0x0020_81b3_u32.to_le_bytes().to_vec()),
        execution_pc: 0x8000, next_pc: 0x8004, instruction_bytes: 4,
        register_writes: vec![RegisterWrite::new(reg(3), 13)],
        float_register_writes: Vec::new(),
        memory_access: None, data_access_event_kind: None,
        counts_as_retired_instruction: true,
    };
    RiscvO3LiveCheckpointPayload {
        profile: RiscvO3LiveCheckpointProfile::ComputeQueue,
        captured_tick: 20, next_fetch_pc: Address::new(0x8004), next_fetch_request_sequence: 2,
        events: vec![event],
        issue_rows: vec![RiscvO3LiveCheckpointIssueRow { sequence: 1, fetch_request: request }],
        rename_rows: Vec::new(), resident_sequences: vec![1],
        executed_fetch_requests: vec![request], issued_fetch_requests: Vec::new(),
        service: RiscvO3LiveCheckpointService {
            requested_tick: 21, mutation_generation: 1, last_service_generation: None,
            telemetry: RiscvO3LiveCheckpointTelemetry {
                enqueued_rows: 1, service_turns: 0, wake_requests: 1,
                current_occupancy: 1, peak_occupancy: 1,
                scalar_integer_issued_rows: 0, integer_mul_div_issued_rows: 0,
                memory_agu_issued_rows: 0, control_issued_rows: 0,
                scalar_float_issued_rows: 0, vector_to_scalar_issued_rows: 0,
            },
        },
        finalized_writeback: RiscvO3LiveCheckpointFinalizedWriteback {
            cycles: 0, admitted_rows: 0, deferred_rows: 0, deferred_row_cycles: 0,
            max_ready_rows_per_cycle: 0, max_deferred_rows: 0,
            partial_cycle_ticks: BTreeSet::new(),
            partial_ready_rows_by_tick: BTreeMap::new(),
            partial_deferred_rows_by_tick: BTreeMap::new(),
            closed_before_tick: 20,
        },
        writeback_counted_sequences: Vec::new(), writeback_published_sequences: Vec::new(),
        reservation: None, completed_result: None,
        pending_address: None,
        wake: RiscvO3LiveCheckpointWake {
            scheduler_instance_raw: 1, partition: PartitionId::new(0),
            tick: 21, scheduler_order: 1, kind: ScheduledEventKind::Parallel,
        },
    }.encode().unwrap()
}
