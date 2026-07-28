use std::collections::{BTreeMap, BTreeSet};

use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, MemoryAccessKind, MemoryWidth, Register, RegisterWrite,
    RiscvInstruction,
};
use rem6_kernel::{PartitionId, ScheduledEventKind};
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::*;

#[path = "riscv_live_checkpoint_tests/codec.rs"]
mod codec;
#[path = "riscv_live_checkpoint_tests/compute.rs"]
mod compute;
#[path = "riscv_live_checkpoint_tests/fp_result.rs"]
mod fp_result;

#[rustfmt::skip]
fn reg(index: u8) -> Register { Register::new(index).unwrap() }

#[rustfmt::skip]
fn freg(index: u8) -> FloatRegister { FloatRegister::new(index).unwrap() }

#[rustfmt::skip]
fn request(sequence: u64) -> MemoryRequestId { MemoryRequestId::new(AgentId::new(7), sequence) }

#[rustfmt::skip]
fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15)
        | (funct3 << 12) | (u32::from(rd) << 7) | opcode
}

#[rustfmt::skip]
fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20) | (u32::from(rs1) << 15)
        | (funct3 << 12) | (u32::from(rd) << 7) | opcode
}

#[rustfmt::skip]
fn add(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(0, rs2, rs1, 0, rd, 0x33) }

#[rustfmt::skip]
fn mul(rd: u8, rs1: u8, rs2: u8) -> u32 { r_type(1, rs2, rs1, 0, rd, 0x33) }

#[rustfmt::skip]
fn fp_width(width: MemoryWidth, word: u32, doubleword: u32) -> u32 { match width { MemoryWidth::Word => word, MemoryWidth::Doubleword => doubleword, _ => unreachable!() } }

#[rustfmt::skip]
fn float_load(rd: u8, rs1: u8, width: MemoryWidth) -> u32 { i_type(0, rs1, fp_width(width, 2, 3), rd, 0x07) }

#[rustfmt::skip]
fn float_mul(rd: u8, rs1: u8, rs2: u8, width: MemoryWidth) -> u32 { r_type(fp_width(width, 0x08, 0x09), rs2, rs1, 0, rd, 0x53) }

#[rustfmt::skip]
fn event(
    pc: u64, sequence: u64, raw: u32,
    register_writes: Vec<RegisterWrite>, float_register_writes: Vec<FloatRegisterWrite>,
    memory_access: Option<MemoryAccessKind>, data_access_event_kind: Option<RiscvDataAccessEventKind>,
) -> RiscvO3LiveCheckpointEvent {
    let fetch = CpuFetchRecord::new(
        20 + sequence, PartitionId::new(2), MemoryRouteId::new(9),
        TransportEndpointId::new("cpu0.ifetch").unwrap(), request(sequence),
        Address::new(pc), AccessSize::new(4).unwrap(),
    );
    RiscvO3LiveCheckpointEvent {
        fetch: CpuFetchEvent::completed(fetch, raw.to_le_bytes().to_vec()),
        execution_pc: pc, next_pc: pc + 4, instruction_bytes: 4,
        register_writes, float_register_writes,
        memory_access, data_access_event_kind,
        counts_as_retired_instruction: true,
    }
}

#[rustfmt::skip]
fn telemetry() -> RiscvO3LiveCheckpointTelemetry {
    RiscvO3LiveCheckpointTelemetry {
        enqueued_rows: 21, service_turns: 22, wake_requests: 23,
        current_occupancy: 2, peak_occupancy: 4,
        scalar_integer_issued_rows: 24, integer_mul_div_issued_rows: 25,
        memory_agu_issued_rows: 26, control_issued_rows: 0,
        scalar_float_issued_rows: 27, vector_to_scalar_issued_rows: 0,
    }
}

#[rustfmt::skip]
fn finalized_writeback() -> RiscvO3LiveCheckpointFinalizedWriteback {
    RiscvO3LiveCheckpointFinalizedWriteback {
        cycles: 31, admitted_rows: 32, deferred_rows: 33, deferred_row_cycles: 34,
        max_ready_rows_per_cycle: 3, max_deferred_rows: 2,
        partial_cycle_ticks: BTreeSet::from([96, 97]),
        partial_ready_rows_by_tick: BTreeMap::from([(96, 2), (97, 1)]),
        partial_deferred_rows_by_tick: BTreeMap::from([(97, 1)]),
        closed_before_tick: 96,
    }
}

#[rustfmt::skip]
fn base_payload(
    profile: RiscvO3LiveCheckpointProfile,
    events: Vec<RiscvO3LiveCheckpointEvent>,
    reservation: Option<RiscvO3LiveCheckpointReservation>,
    completed_result: Option<RiscvO3LiveCheckpointCompletedFpLoad>,
) -> RiscvO3LiveCheckpointPayload {
    let sequences = events.iter().map(|event| event.fetch.request_id().sequence()).collect::<Vec<_>>();
    let requests = sequences.iter().copied().map(request).collect::<Vec<_>>();
    let issue_rows = sequences.iter().copied().map(|sequence| RiscvO3LiveCheckpointIssueRow {
        sequence, fetch_request: request(sequence),
    }).collect();
    RiscvO3LiveCheckpointPayload {
        profile, captured_tick: 100, next_fetch_pc: Address::new(0x8010),
        next_fetch_request_sequence: 999, issue_rows,
        rename_rows: vec![
            O3RenameMapEntry::new(O3RegisterClass::Integer, 3, O3PhysicalRegisterId::new(43)),
            O3RenameMapEntry::new(O3RegisterClass::FloatingPoint, 5, O3PhysicalRegisterId::new(45)),
        ],
        resident_sequences: sequences, executed_fetch_requests: requests.clone(), issued_fetch_requests: if profile == RiscvO3LiveCheckpointProfile::ComputeQueue { Vec::new() } else { requests },
        service: RiscvO3LiveCheckpointService {
            requested_tick: 108, mutation_generation: 51, last_service_generation: Some((97, 49)), telemetry: telemetry(),
        },
        finalized_writeback: finalized_writeback(),
        writeback_counted_sequences: if profile == RiscvO3LiveCheckpointProfile::ComputeQueue { Vec::new() } else { vec![71] }, writeback_published_sequences: if profile == RiscvO3LiveCheckpointProfile::ComputeQueue { Vec::new() } else { vec![70] },
        reservation, completed_result,
        wake: RiscvO3LiveCheckpointWake {
            scheduler_instance_raw: 0x4455_6677_8899_aabb, partition: PartitionId::new(2),
            tick: 108, scheduler_order: 77, kind: ScheduledEventKind::Parallel,
        },
        events,
    }
}

#[rustfmt::skip]
fn compute_payload() -> RiscvO3LiveCheckpointPayload {
    base_payload(
        RiscvO3LiveCheckpointProfile::ComputeQueue,
        vec![
            event(0x8000, 1, add(3, 1, 2), vec![RegisterWrite::new(reg(3), 13)], Vec::new(), None, None),
            event(0x8004, 2, mul(4, 3, 2), vec![RegisterWrite::new(reg(4), 65)], Vec::new(), None, None),
        ],
        None, None,
    )
}

#[rustfmt::skip]
fn completed_fp_payload(width: MemoryWidth) -> RiscvO3LiveCheckpointPayload {
    let response_bytes = match width {
        MemoryWidth::Word => vec![0x11, 0x22, 0x33, 0x44],
        MemoryWidth::Doubleword => vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
        _ => unreachable!(),
    };
    let access_size = AccessSize::new(response_bytes.len() as u64).unwrap();
    let sequence = if width == MemoryWidth::Word { 10 } else { 20 };
    base_payload(
        RiscvO3LiveCheckpointProfile::CompletedFpLoad,
        vec![
            event(
                0x8000, sequence, float_load(3, 10, width), Vec::new(), Vec::new(),
                Some(MemoryAccessKind::FloatLoad { rd: freg(3), address: 0x9000, width }),
                Some(RiscvDataAccessEventKind::Completed),
            ),
            event(
                0x8004, sequence + 1, float_mul(5, 3, 4, width), Vec::new(),
                vec![FloatRegisterWrite::new(freg(5), 0x3ff0_0000_0000_0000)],
                None, None,
            ),
        ],
        Some(RiscvO3LiveCheckpointReservation {
            sequence, raw_ready_tick: 104, admitted_tick: 106, slot: 1,
            source: RiscvO3LiveCheckpointWritebackSource::MemoryResult, decision_counted: true,
        }),
        Some(RiscvO3LiveCheckpointCompletedFpLoad {
            fetch_request: request(sequence), data_request: MemoryRequestId::new(AgentId::new(8), sequence + 100),
            sequence, lsq_sequence: sequence,
            rob_first_sequence: sequence, rob_last_sequence: sequence + 1,
            issue_tick: 101, response_tick: 103, raw_ready_tick: 104, admitted_tick: 106, latency_ticks: 2,
            physical_address: Address::new(0x9000), access_size, request_byte_offset: 0, response_bytes,
            destination: freg(3), width,
        }),
    )
}
