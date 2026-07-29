use std::collections::BTreeSet;

use rem6_isa_riscv::{MemoryWidth, Register, RiscvInstruction};

use crate::{CpuFetchEventKind, O3LoadStoreQueueKind, O3RegisterClass, O3RenameMapEntry};

use super::super::{invalid, RiscvO3LiveCheckpointError, RiscvO3LiveCheckpointPayload};
use super::{RiscvO3LiveCheckpointPendingDataAddress, MAX_PENDING_ADDRESSES};

pub(super) fn validate(
    value: &RiscvO3LiveCheckpointPayload,
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) -> Result<(), RiscvO3LiveCheckpointError> {
    if rows.len() > MAX_PENDING_ADDRESSES {
        return Err(invalid("pending load graph has too many rows"));
    }
    if rows.windows(2).any(|window| {
        window[0].sequence >= window[1].sequence
            || window[0].fetch.request_id().sequence() >= window[1].fetch.request_id().sequence()
    }) {
        return Err(invalid("pending load graph is not ordered"));
    }
    let root = &rows[0];
    if root.producer_sequence != root.root_sequence
        || root.producer_register.is_zero()
        || root.root_fetch_request.agent() != root.fetch.request_id().agent()
        || root.root_fetch_request.sequence() >= root.fetch.request_id().sequence()
        || root.root_fetch_request.sequence() >= value.next_fetch_request_sequence
    {
        return Err(invalid("pending load root is inconsistent"));
    }
    if rows
        .iter()
        .any(|pending| pending.root_sequence >= pending.sequence)
    {
        return Err(invalid("pending load root is not older than every row"));
    }

    let mut consumed = BTreeSet::new();
    let mut destination_architectural = BTreeSet::new();
    let mut destination_physical = BTreeSet::new();
    let mut requested_wakes = Vec::new();
    for (index, pending) in rows.iter().enumerate() {
        let destination = pending
            .destination
            .ok_or(invalid("pending load lacks a destination"))?;
        let destination_register = row_destination_register(destination)?;
        let destination_architectural_register = destination.architectural();
        if destination_architectural_register == u32::from(root.producer_register.index())
            || !destination_architectural.insert(destination_architectural_register)
        {
            return Err(invalid(
                "pending load destinations repeat an architectural register",
            ));
        }
        if !destination_physical.insert(destination.physical().get()) {
            return Err(invalid(
                "pending load destinations repeat a physical register",
            ));
        }
        for request in &pending.consumed_requests {
            if !consumed.insert(*request) {
                return Err(invalid(
                    "pending-address consumed requests repeat an identity",
                ));
            }
        }

        let fetch_request = pending.fetch.request_id();
        let predecessor_request = if index == 0 {
            pending.root_fetch_request
        } else {
            rows[index - 1].fetch.request_id()
        };
        let expected_pc = root
            .fetch
            .pc()
            .get()
            .checked_add(4 * index as u64)
            .ok_or(invalid("pending load PC overflows"))?;
        if pending.fetch.kind() != CpuFetchEventKind::Completed
            || pending.fetch.size().bytes() != 4
            || pending.fetch.data().is_none_or(|bytes| bytes.len() != 4)
            || pending.consumed_requests != [fetch_request]
            || pending.fetch_predecessor_request != predecessor_request
            || pending.root_sequence != root.root_sequence
            || pending.root_fetch_request != root.root_fetch_request
            || pending.root_range != root.root_range
            || pending.root_atomic != root.root_atomic
            || pending.fetch.partition() != root.fetch.partition()
            || pending.fetch.route() != root.fetch.route()
            || pending.fetch.endpoint() != root.fetch.endpoint()
            || fetch_request.agent() != root.fetch.request_id().agent()
            || pending.sequence != value.resident_sequences[index]
            || fetch_request.sequence() >= value.next_fetch_request_sequence
            || pending.fetch.tick() > value.captured_tick
            || pending.root_range.size().bytes() != 8
            || pending.lsq_kind != O3LoadStoreQueueKind::Load
            || pending.expected_lsq_bytes != 8
            || pending.fetch.pc().get() != expected_pc
            || pending.producer_register.is_zero()
        {
            return Err(invalid("pending load row is inconsistent"));
        }

        let root_dependent = pending.producer_sequence == pending.root_sequence;
        let previous_dependent = index > 0 && pending.producer_sequence == rows[index - 1].sequence;
        if !root_dependent && !previous_dependent {
            return Err(invalid("pending load producer lineage is invalid"));
        }
        if root_dependent {
            let (Some(published_tick), Some(requested_wake_tick)) = (
                pending.published_producer_ready_tick,
                pending.requested_wake_tick,
            ) else {
                return Err(invalid("root-dependent pending load lacks wake"));
            };
            if pending.producer_register != root.producer_register
                || published_tick > value.captured_tick
                || value.captured_tick > requested_wake_tick
                || published_tick > requested_wake_tick
            {
                return Err(invalid("root-dependent pending load wake is invalid"));
            }
            requested_wakes.push(requested_wake_tick);
        } else {
            let prior_destination = rows[index - 1]
                .destination
                .expect("validated previous pending load destination");
            if pending.producer_register != row_destination_register(prior_destination)?
                || pending.published_producer_ready_tick.is_some()
                || pending.requested_wake_tick.is_some()
            {
                return Err(invalid("internal pending load dependency is invalid"));
            }
        }

        validate_load_instruction(pending, destination_register)?;
    }

    if requested_wakes.iter().min().copied() != Some(value.service.requested_tick)
        || value.wake.tick != value.service.requested_tick
        || rows
            .last()
            .and_then(|pending| pending.fetch.pc().get().checked_add(4))
            != Some(value.next_fetch_pc.get())
    {
        return Err(invalid("pending load wake or fetch tail is inconsistent"));
    }
    Ok(())
}

fn validate_load_instruction(
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
    destination_register: Register,
) -> Result<(), RiscvO3LiveCheckpointError> {
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
            RiscvInstruction::Load {
                rd,
                rs1,
                width: MemoryWidth::Doubleword,
                ..
            } if rd == destination_register && rs1 == pending.producer_register && !rd.is_zero()
        )
    {
        return Err(invalid(
            "pending-address instruction is not a destinationful doubleword load",
        ));
    }
    Ok(())
}

fn row_destination_register(
    value: O3RenameMapEntry,
) -> Result<Register, RiscvO3LiveCheckpointError> {
    if value.register_class() != O3RegisterClass::Integer
        || value.architectural() >= 32
        || value.physical().is_invalid()
    {
        return Err(invalid("pending load destination is not a scalar register"));
    }
    Register::new(value.architectural() as u8)
        .map_err(|_| invalid("pending load destination is not a scalar register"))
}
