use std::collections::{BTreeMap, BTreeSet};

use rem6_cpu::{
    CpuFetchEvent, CpuFetchRecord, RiscvO3LiveCheckpointEvent,
    RiscvO3LiveCheckpointFinalizedWriteback, RiscvO3LiveCheckpointPayload,
    RiscvO3LiveCheckpointProfile, RiscvO3LiveCheckpointService, RiscvO3LiveCheckpointTelemetry,
    RiscvO3LiveCheckpointWake,
};
use rem6_isa_riscv::RegisterWrite;
use rem6_kernel::ScheduledEventKind;
use rem6_memory::MemoryRequestId;
use rem6_transport::MemoryRouteId;

use super::*;

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_rejects_malformed_o3lc_before_mutating_destination() {
    let (destination, error) = restore_with_o3lc(b"O3LC\x01".to_vec());
    assert!(error.to_string().contains("O3 live checkpoint profile"), "unexpected checkpoint error: {error}");
    assert_sentinel_state(&destination);
}

#[test]
#[rustfmt::skip]
fn riscv_checkpoint_rejects_valid_o3lc_until_live_restore_is_supported() {
    let (destination, error) = restore_with_o3lc(valid_compute_o3lc());
    assert!(matches!(error, RiscvCoreCheckpointError::O3LiveCheckpointNotRestorable { .. }));
    assert_sentinel_state(&destination);
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
        issue_rows: Vec::new(), rename_rows: Vec::new(), resident_sequences: Vec::new(),
        executed_fetch_requests: Vec::new(), issued_fetch_requests: Vec::new(),
        service: RiscvO3LiveCheckpointService {
            requested_tick: 21, mutation_generation: 1, last_service_generation: 0,
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
        wake: RiscvO3LiveCheckpointWake {
            scheduler_instance_raw: 1, partition: PartitionId::new(0),
            tick: 21, scheduler_order: 1, kind: ScheduledEventKind::Parallel,
        },
    }.encode().unwrap()
}
