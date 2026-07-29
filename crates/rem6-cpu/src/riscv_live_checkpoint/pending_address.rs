use std::collections::BTreeSet;

use rem6_isa_riscv::{MemoryWidth, Register, RiscvInstruction};
use rem6_kernel::Tick;
use rem6_memory::{AddressRange, MemoryRequestId};

use crate::{CpuFetchEvent, CpuFetchEventKind, O3LoadStoreQueueKind};

use super::{
    invalid, RiscvO3LiveCheckpointError, RiscvO3LiveCheckpointIssueRow,
    RiscvO3LiveCheckpointPayload,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointPendingDataAddress {
    pub sequence: u64,
    pub fetch: CpuFetchEvent,
    pub consumed_requests: Vec<MemoryRequestId>,
    pub fetch_predecessor_request: MemoryRequestId,
    pub producer_register: Register,
    pub producer_sequence: u64,
    pub root_sequence: u64,
    pub root_fetch_request: MemoryRequestId,
    pub root_range: AddressRange,
    pub root_atomic: bool,
    pub lsq_kind: O3LoadStoreQueueKind,
    pub expected_lsq_bytes: u32,
    pub published_producer_ready_tick: Tick,
    pub requested_wake_tick: Tick,
}

pub(super) fn validate_pending_profile(
    value: &RiscvO3LiveCheckpointPayload,
) -> Result<(), RiscvO3LiveCheckpointError> {
    let Some(pending) = value.pending_address.as_ref() else {
        return Err(invalid("pending-address profile lacks its pending row"));
    };
    let expected_issue = RiscvO3LiveCheckpointIssueRow {
        sequence: pending.sequence,
        fetch_request: pending.fetch.request_id(),
    };
    if !value.events.is_empty()
        || value.issue_rows != [expected_issue]
        || !value.rename_rows.is_empty()
        || value.resident_sequences != [pending.sequence]
        || !value.executed_fetch_requests.is_empty()
        || !value.issued_fetch_requests.is_empty()
        || !value.writeback_counted_sequences.is_empty()
        || !value.writeback_published_sequences.is_empty()
        || value.reservation.is_some()
        || value.completed_result.is_some()
        || value.service.telemetry.current_occupancy != 1
        || value.service.telemetry.peak_occupancy < 1
        || !value
            .finalized_writeback
            .is_valid_without_live_calendar_at(value.captured_tick)
        || value.service.requested_tick != pending.requested_wake_tick
        || value.wake.tick != pending.requested_wake_tick
        || value.wake.partition != pending.fetch.partition()
    {
        return Err(invalid("pending-address ownership is inconsistent"));
    }
    validate_pending_row(value, pending)
}

fn validate_pending_row(
    value: &RiscvO3LiveCheckpointPayload,
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<(), RiscvO3LiveCheckpointError> {
    let mut consumed = BTreeSet::new();
    for request in &pending.consumed_requests {
        if !consumed.insert(*request) {
            return Err(invalid(
                "pending-address consumed requests repeat an identity",
            ));
        }
    }
    let fetch_request = pending.fetch.request_id();
    if pending.fetch.kind() != CpuFetchEventKind::Completed
        || pending.fetch.size().bytes() != 4
        || pending.fetch.data().is_none_or(|bytes| bytes.len() != 4)
        || pending.consumed_requests != [fetch_request]
        || pending.fetch_predecessor_request != pending.root_fetch_request
        || pending.root_fetch_request.agent() != fetch_request.agent()
        || pending.root_fetch_request.sequence() >= fetch_request.sequence()
        || pending.root_fetch_request.sequence() >= value.next_fetch_request_sequence
        || fetch_request.sequence() >= value.next_fetch_request_sequence
        || pending.fetch.tick() > value.captured_tick
        || pending.producer_register.is_zero()
        || pending.root_sequence != pending.producer_sequence
        || pending.producer_sequence >= pending.sequence
        || pending.root_range.size().bytes() != 8
        || pending.lsq_kind != O3LoadStoreQueueKind::Store
        || pending.expected_lsq_bytes != 8
        || pending.published_producer_ready_tick > value.captured_tick
        || value.captured_tick > pending.requested_wake_tick
        || pending.published_producer_ready_tick > pending.requested_wake_tick
        || pending
            .fetch
            .pc()
            .get()
            .checked_add(4)
            .is_none_or(|next_pc| next_pc != value.next_fetch_pc.get())
    {
        return Err(invalid("pending-address row is inconsistent"));
    }

    let raw = u32::from_le_bytes(
        pending
            .fetch
            .data()
            .expect("validated pending fetch bytes")
            .try_into()
            .expect("validated four-byte pending fetch"),
    );
    let decoded = RiscvInstruction::decode_with_length(raw)
        .map_err(|_| invalid("pending-address instruction does not decode"))?;
    if decoded.bytes() != 4
        || !matches!(
            decoded.instruction(),
            RiscvInstruction::Store {
                rs1,
                rs2,
                width: MemoryWidth::Doubleword,
                ..
            } if rs1 == pending.producer_register && !rs2.is_zero() && rs1 != rs2
        )
    {
        return Err(invalid(
            "pending-address instruction is not a destinationless doubleword store",
        ));
    }
    Ok(())
}
