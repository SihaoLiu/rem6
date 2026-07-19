use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord,
    RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::riscv_data_completion::RiscvDataCompletion;
use crate::{CpuFetchEvent, CpuFetchRecord, RiscvCpuExecutionEvent};

use super::*;

#[test]
fn fixed_fu_owner_rejects_memory_result_reservation_source() {
    let sequence = 9;
    let raw_ready_tick = 12;
    let instruction = addi(3, 0, 42);
    let mut transaction = O3WritebackReplanTransaction::capture(&O3RuntimeState::default());
    transaction
        .live_speculative_executions
        .push(O3LiveSpeculativeExecution {
            consumed_requests: Vec::new(),
            sequence,
            producer_sequences: Vec::new(),
            issue_tick: 10,
            raw_ready_tick,
            admitted_writeback_tick: raw_ready_tick,
            writeback_slot: Some(0),
            execution: RiscvExecutionRecord::new(
                instruction,
                0x8000,
                0x8004,
                vec![RegisterWrite::new(reg(3), 42)],
                None,
            ),
        });
    let before = transaction.live_speculative_executions.clone();

    let error = transaction
        .sync_live_fixed_fu_writeback_owner(O3WritebackReservation::new(
            sequence,
            raw_ready_tick,
            raw_ready_tick,
            0,
            O3LiveWritebackReadySource::MemoryResult,
            true,
        ))
        .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackOwnerSourceMismatch {
            sequence,
            owner: "fixed-FU speculative execution",
            reservation_source: "MemoryResult",
        }
    );
    assert_eq!(transaction.live_speculative_executions, before);
}

#[test]
fn memory_result_owner_rejects_fixed_fu_reservation_source() {
    let owner = completed_memory_result_owner(7, 39);
    let error = O3WritebackReplanTransaction::validate_live_memory_result_writeback_owner(
        std::slice::from_ref(&owner),
        O3WritebackReservation::new(7, 40, 40, 0, O3LiveWritebackReadySource::FixedFu, true),
    )
    .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackOwnerSourceMismatch {
            sequence: 7,
            owner: "live data access",
            reservation_source: "FixedFu",
        }
    );
}

#[test]
fn memory_result_owner_rejects_changed_raw_ready_tick() {
    let owner = completed_memory_result_owner(7, 39);
    let error = O3WritebackReplanTransaction::validate_live_memory_result_writeback_owner(
        std::slice::from_ref(&owner),
        O3WritebackReservation::new(7, 41, 41, 0, O3LiveWritebackReadySource::MemoryResult, true),
    )
    .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackOwnerReservationMismatch {
            sequence: 7,
            owner: "live data access",
            owner_raw_ready_tick: 40,
            reservation_raw_ready_tick: 41,
        }
    );
}

fn completed_memory_result_owner(sequence: u64, response_tick: u64) -> O3LiveDataAccess {
    let fetch_request = memory_request(10);
    let data_request = memory_request(20);
    let instruction = RiscvInstruction::Load {
        rd: reg(13),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(13),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: false,
    };
    let execution = RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8000,
            0x8004,
            Vec::new(),
            Some(access.clone()),
        ),
    );
    let load_data = vec![0x2a, 0, 0, 0];

    O3LiveDataAccess {
        fetch_request,
        data_request,
        execution,
        sequence,
        lsq_sequence_span: 1,
        issue_tick: 31,
        issue_rob_occupancy: 1,
        issue_lsq_occupancy: 1,
        younger_window_policy: O3DataAccessWindowPolicy::None,
        response_tick: Some(response_tick),
        latency_ticks: Some(response_tick - 31),
        commit_tick: None,
        load_data: Some(load_data.clone()),
        memory_result: Some(RiscvDataCompletion::from_issued_response(
            fetch_request,
            access,
            Address::new(0x9000),
            AccessSize::new(4).unwrap(),
            0,
            Some(load_data),
        )),
        forwarding_plan: None,
        outcome: O3LiveDataAccessOutcome::Completed,
        event_taken: false,
    }
}

fn addi(rd: u8, rs1: u8, immediate: i64) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(immediate),
    }
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            memory_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}
