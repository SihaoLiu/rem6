use std::collections::BTreeSet;

use rem6_isa_riscv::{MemoryWidth, RiscvInstruction};

use crate::{CpuFetchEventKind, O3LoadStoreQueueKind};

use super::super::{invalid, RiscvO3LiveCheckpointError, RiscvO3LiveCheckpointPayload};
use super::RiscvO3LiveCheckpointPendingDataAddress;

pub(super) fn validate(
    value: &RiscvO3LiveCheckpointPayload,
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<(), RiscvO3LiveCheckpointError> {
    let (Some(published_tick), Some(requested_wake_tick)) = (
        pending.published_producer_ready_tick,
        pending.requested_wake_tick,
    ) else {
        return Err(invalid("pending store lacks publication or wake"));
    };
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
        || published_tick > value.captured_tick
        || value.captured_tick > requested_wake_tick
        || published_tick > requested_wake_tick
        || value.service.requested_tick != requested_wake_tick
        || value.wake.tick != requested_wake_tick
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
