use rem6_isa_riscv::{
    AtomicMemoryOp, MemoryAccessKind, MemoryWidth, RiscvExecutionRecord, RiscvInstruction,
    VectorRegister,
};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AccessSize, Address, TranslationRequestId};
use rem6_mmio::MmioRoute;

use super::*;
use crate::riscv_data_issue::OutstandingDataAccess;
use crate::riscv_translation::PendingDataTranslation;

#[test]
fn duplicate_sequence_and_request_identities_are_rejected_atomically() {
    compute::assert_compute_projection_mutation_rejected(|_, live| {
        live.issue_rows[1].sequence = live.issue_rows[0].sequence;
    });
    compute::assert_compute_projection_mutation_rejected(|_, live| {
        live.issue_rows[1].fetch_request = live.issue_rows[0].fetch_request;
    });

    let mut duplicate_resident = compute_payload();
    duplicate_resident.resident_sequences[1] = duplicate_resident.resident_sequences[0];
    assert!(matches!(
        duplicate_resident.encode(),
        Err(RiscvO3LiveCheckpointError::DuplicateValue {
            field: "resident sequences",
            ..
        })
    ));
}

#[test]
fn missing_rob_lsq_and_rename_owners_are_rejected_atomically() {
    compute::assert_compute_projection_mutation_rejected(|stable, _| {
        let rob = stable.snapshot().reorder_buffer()[1..].to_vec();
        *stable = compute::rebuilt_stable(stable, Some(rob), None, None).unwrap();
    });

    fp_result::assert_completed_fp_projection_mutation_rejected(MemoryWidth::Word, |stable, _| {
        *stable = compute::rebuilt_stable(stable, None, Some(Vec::new()), None).unwrap();
    });

    fp_result::assert_completed_fp_projection_mutation_rejected(
        MemoryWidth::Word,
        |stable, live| {
            let destination = live.completed_result.as_ref().unwrap().destination;
            let rename = stable
                .snapshot()
                .rename_map()
                .iter()
                .copied()
                .filter(|row| {
                    row.register_class() != O3RegisterClass::FloatingPoint
                        || row.architectural() != u32::from(destination.index())
                })
                .collect::<Vec<_>>();
            *stable = compute::rebuilt_stable(stable, None, None, Some(rename.clone())).unwrap();
            live.rename_rows = rename;
        },
    );
}

#[test]
fn completed_fp_calendar_collision_and_o3rt_aggregate_mismatch_are_rejected_atomically() {
    fp_result::assert_completed_fld_occupied_slot_collision_rejected();
    compute::assert_compute_projection_mutation_rejected(|stable, _| {
        let mut stats = stable.stats();
        stats.writeback_port_admitted_rows += 1;
        *stable = O3RuntimeCheckpointPayload::from_snapshot_with_stats_and_dependency_producers(
            stable.snapshot().clone(),
            stats,
            stable.dependency_producers_with_consumers().clone(),
        )
        .unwrap();
    });
}

#[test]
fn completed_fp_closed_through_capture_rejects_malformed_finalized_maps_atomically() {
    fp_result::assert_completed_fp_projection_mutation_rejected(MemoryWidth::Word, |_, live| {
        live.finalized_writeback.closed_before_tick = live.captured_tick + 1;
        live.finalized_writeback
            .partial_cycle_ticks
            .insert(live.captured_tick);
        live.finalized_writeback
            .partial_ready_rows_by_tick
            .insert(live.captured_tick, 1);
    });
}

#[test]
fn completed_fp_target_width_and_response_bytes_are_rejected_atomically() {
    fp_result::assert_completed_fp_projection_mutation_rejected(MemoryWidth::Word, |_, live| {
        live.completed_result.as_mut().unwrap().destination = freg(4);
    });
    fp_result::assert_completed_fp_projection_mutation_rejected(MemoryWidth::Word, |_, live| {
        live.completed_result.as_mut().unwrap().width = MemoryWidth::Doubleword;
    });
    fp_result::assert_completed_fp_projection_mutation_rejected(MemoryWidth::Word, |_, live| {
        live.completed_result.as_mut().unwrap().response_bytes.pop();
    });
}

#[test]
fn translated_pending_resident_transport_and_mmio_authority_remain_rejected() {
    compute::assert_compute_capture_mutation_rejected(|core| {
        let access = scalar_load();
        core.state
            .lock()
            .expect("riscv core lock")
            .pending_data_translations
            .insert(
                TranslationRequestId::new(AgentId::new(7), 80),
                PendingDataTranslation {
                    request_id: request(80),
                    fetch_request: request(1),
                    access,
                    virtual_address: Address::new(0x9000),
                    size: AccessSize::new(4).unwrap(),
                    request_byte_offset: 0,
                },
            );
    });
    for target in [
        RiscvDataAccessTarget::Memory {
            route: MemoryRouteId::new(0),
            endpoint: TransportEndpointId::new("cpu0.dmem").unwrap(),
        },
        RiscvDataAccessTarget::Mmio {
            route: MmioRoute::new(PartitionId::new(0), PartitionId::new(0), 2, 2).unwrap(),
        },
    ] {
        compute::assert_compute_capture_mutation_rejected(|core| {
            let outstanding = outstanding_access(scalar_load(), target.clone());
            let issued = outstanding.issued_for_checkpoint_test();
            core.state
                .lock()
                .expect("riscv core lock")
                .outstanding_data
                .insert(outstanding.request_id, issued);
        });
    }
}

#[test]
fn store_atomic_and_vector_memory_authority_remain_rejected() {
    for event in [store_event(), atomic_event(), vector_load_event()] {
        compute::assert_compute_capture_mutation_rejected(|core| {
            let access = event
                .execution()
                .memory_access()
                .expect("typed memory access")
                .clone();
            let outstanding = outstanding_access(
                access,
                RiscvDataAccessTarget::Memory {
                    route: MemoryRouteId::new(0),
                    endpoint: TransportEndpointId::new("cpu0.dmem").unwrap(),
                },
            );
            let issued = outstanding.issued_for_checkpoint_test();
            core.state
                .lock()
                .expect("riscv core lock")
                .outstanding_data
                .insert(outstanding.request_id, issued);
        });
    }
}

#[test]
fn retry_failure_and_forwarding_overlays_remain_rejected() {
    for kind in [
        RiscvDataAccessEventKind::Retry,
        RiscvDataAccessEventKind::Failed,
    ] {
        fp_result::assert_completed_fp_terminal_capture_rejected(kind);
    }
    compute::assert_compute_capture_mutation_rejected(|core| {
        let store = store_event();
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .stage_store_forwarding_overlay_for_checkpoint_test(&store);
        assert!(matches!(
            state.o3_runtime.compute_checkpoint_projection(100),
            Err(RiscvO3LiveCheckpointError::InvalidProfileShape {
                reason: "unsupported transient O3 authority"
            })
        ));
        assert!(state.o3_force_normal_execute_fetches.is_empty());
    });
}

#[test]
fn split_fetch_authority_remains_rejected() {
    compute::assert_split_fetch_issue_packet_capture_rejected();
}

#[test]
fn producer_forwarded_continuation_after_transport_drain_remains_rejected() {
    compute::assert_producer_forwarded_continuation_capture_rejected();
}

#[test]
fn detached_and_multiple_o3_wake_authority_remain_rejected() {
    compute::assert_compute_capture_mutation_rejected(|core| {
        let mut state = core.state.lock().expect("riscv core lock");
        state.o3_writeback_wake.set_desired_tick(Some(99), 90);
        assert_eq!(state.o3_writeback_wake.owned_wakes().len(), 1);
        assert!(state
            .o3_writeback_wake
            .checkpoint_scheduled_wake()
            .is_none());
    });

    compute::assert_compute_capture_mutation_rejected(|core| {
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let event = scheduler
            .schedule_at(PartitionId::new(0), 99, |_| {})
            .unwrap();
        let snapshot = scheduler.pending_event_snapshot(event).unwrap();
        let mut state = core.state.lock().expect("riscv core lock");
        state.o3_writeback_wake.set_desired_tick(Some(99), 90);
        state
            .o3_writeback_wake
            .mark_scheduled(scheduler.instance_id(), snapshot);
        assert_eq!(state.o3_writeback_wake.owned_wakes().len(), 2);
        assert!(state
            .o3_writeback_wake
            .checkpoint_scheduled_wake()
            .is_none());
    });
}

fn scalar_load() -> MemoryAccessKind {
    MemoryAccessKind::Load {
        rd: reg(6),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: true,
    }
}

fn outstanding_access(
    access: MemoryAccessKind,
    target: RiscvDataAccessTarget,
) -> OutstandingDataAccess {
    OutstandingDataAccess {
        tick: 90,
        partition: PartitionId::new(0),
        target,
        request_id: request(80),
        fetch_request: request(1),
        access,
        size: AccessSize::new(4).unwrap(),
        physical_address: Address::new(0x9000),
        request_byte_offset: 0,
        line_layout: Some(CacheLineLayout::new(16).unwrap()),
        forwarded_load_data: None,
        store_load_forwarding_plan: None,
    }
}

fn store_event() -> RiscvCpuExecutionEvent {
    let raw = ((2_u32 & 0x1f) << 20) | ((1_u32 & 0x1f) << 15) | (2 << 12) | 0x23;
    memory_event(
        71,
        raw,
        MemoryAccessKind::Store {
            address: 0x9000,
            width: MemoryWidth::Word,
            value: 7,
        },
    )
}

fn atomic_event() -> RiscvCpuExecutionEvent {
    let raw = (u32::from(reg(2).index()) << 20)
        | (u32::from(reg(1).index()) << 15)
        | (2 << 12)
        | (u32::from(reg(6).index()) << 7)
        | 0x2f;
    memory_event(
        72,
        raw,
        MemoryAccessKind::AtomicMemory {
            rd: reg(6),
            address: 0x9000,
            width: MemoryWidth::Word,
            op: AtomicMemoryOp::Add,
            value: 7,
            acquire: false,
            release: false,
        },
    )
}

fn vector_load_event() -> RiscvCpuExecutionEvent {
    let raw = 0x0200_6087;
    memory_event(
        73,
        raw,
        MemoryAccessKind::VectorLoadUnitStride {
            vd: VectorRegister::new(1).unwrap(),
            address: 0x9000,
            width: MemoryWidth::Word,
            byte_len: 4,
            byte_mask: None,
            group_registers: 1,
            fault_only_first: false,
        },
    )
}

fn memory_event(
    sequence: u64,
    raw: u32,
    memory_access: MemoryAccessKind,
) -> RiscvCpuExecutionEvent {
    let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
    let instruction = decoded.instruction();
    let pc = 0x9000 + sequence * 4;
    let fetch = completed_fetch(sequence, pc, raw.to_le_bytes().to_vec());
    let execution = RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
        instruction,
        decoded.bytes(),
        pc,
        pc + u64::from(decoded.bytes()),
        Vec::new(),
        Vec::new(),
        Some(memory_access),
    );
    RiscvCpuExecutionEvent::new(fetch, instruction, execution)
}

fn completed_fetch(sequence: u64, pc: u64, bytes: Vec<u8>) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            90,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(bytes.len() as u64).unwrap(),
        ),
        bytes,
    )
}
