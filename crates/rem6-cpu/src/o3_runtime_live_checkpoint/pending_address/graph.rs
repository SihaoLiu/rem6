use std::collections::BTreeSet;

use rem6_isa_riscv::{MemoryWidth, Register, RiscvDecodedInstruction, RiscvInstruction};

use super::super::super::o3_runtime_pending_address::O3PendingDataAddress;
use super::*;
use crate::riscv_live_checkpoint::MAX_PENDING_ADDRESSES;

pub(super) fn capture(
    runtime: &O3RuntimeState,
    captured_tick: u64,
    resident_sequences: &[u64],
) -> Result<Vec<RiscvO3LiveCheckpointPendingDataAddress>, Error> {
    let rows = runtime.pending_data_addresses.iter().collect::<Vec<_>>();
    validate_runtime_graph(runtime, captured_tick, resident_sequences, &rows)?;
    Ok(rows
        .into_iter()
        .map(|row| RiscvO3LiveCheckpointPendingDataAddress {
            sequence: row.sequence,
            fetch: row.fetch.clone(),
            consumed_requests: row.consumed_requests.clone(),
            fetch_predecessor_request: row.fetch_predecessor_request,
            producer_register: row.producer_register,
            destination: row.destination,
            producer_sequence: row.producer_sequence,
            root_sequence: row.root_head.sequence,
            root_fetch_request: row.root_head.fetch_request,
            root_range: row.root_head.range,
            root_atomic: row.root_head.atomic_head,
            lsq_kind: row.lsq_kind,
            expected_lsq_bytes: row.expected_lsq_bytes,
            published_producer_ready_tick: row.published_producer_ready_tick,
            requested_wake_tick: row.requested_wake_tick,
        })
        .collect())
}

pub(super) fn validate_restore_payload(live: &RiscvO3LiveCheckpointPayload) -> Result<(), Error> {
    let rows = live.pending_addresses.as_slice();
    validate_payload_shape(live, rows)
}

pub(super) fn validate_stable_owner_set(
    runtime: &O3RuntimeState,
    _live: &RiscvO3LiveCheckpointPayload,
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) -> Result<(), Error> {
    let root = rows
        .first()
        .ok_or(invalid("pending load graph lacks rows"))?;
    if runtime.snapshot.reorder_buffer.len() != rows.len()
        || runtime.snapshot.load_store_queue.len() != rows.len()
        || runtime
            .snapshot
            .reorder_buffer
            .iter()
            .any(|owner| owner.sequence() == root.root_sequence)
        || runtime
            .snapshot
            .load_store_queue
            .iter()
            .any(|owner| owner.sequence() == root.root_sequence)
    {
        return Err(invalid("pending load stable owner set is inconsistent"));
    }
    let committed_root_count = runtime
        .snapshot
        .rename_map
        .iter()
        .filter(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == u32::from(root.producer_register.index())
        })
        .count();
    let live_rename = runtime.snapshot_with_live_rename_map();
    for (index, pending) in rows.iter().enumerate() {
        let destination = pending
            .destination
            .ok_or(invalid("pending load lacks a destination"))?;
        let rob = runtime
            .snapshot
            .reorder_buffer
            .get(index)
            .ok_or(invalid("pending load lacks a ROB owner"))?;
        let lsq = runtime
            .snapshot
            .load_store_queue
            .get(index)
            .ok_or(invalid("pending load lacks an LSQ owner"))?;
        if rob.sequence() != pending.sequence
            || rob.pc() != pending.fetch.pc()
            || rob.destination() != Some(destination.physical())
            || rob.rename_destination()
                != Some((destination.register_class(), destination.architectural()))
            || !rob.is_live_staged()
            || rob.is_ready()
            || lsq.sequence() != pending.sequence
            || lsq.kind() != O3LoadStoreQueueKind::Load
            || lsq.address().is_some()
            || lsq.bytes() != 8
            || lsq.is_completed()
            || !live_rename.rename_map().contains(&destination)
        {
            return Err(invalid("pending load stable owner set is inconsistent"));
        }
    }
    if committed_root_count != 1 {
        return Err(invalid(
            "pending load lacks committed architectural producer authority",
        ));
    }
    Ok(())
}

pub(super) fn restore(
    runtime: &mut O3RuntimeState,
    live: &RiscvO3LiveCheckpointPayload,
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) -> Result<(), Error> {
    let decoded = rows
        .iter()
        .map(decode_pending_load)
        .collect::<Result<Vec<_>, _>>()?;
    for (pending, decoded) in rows.iter().zip(decoded.iter().copied()) {
        runtime.live_staged_fetch_identities.insert(
            pending.sequence,
            O3LiveStagedFetchIdentity::new(decoded.instruction()),
        );
        if !runtime.pending_data_addresses.try_push(
            super::super::super::o3_runtime_pending_address::O3PendingDataAddress {
                sequence: pending.sequence,
                fetch: pending.fetch.clone(),
                consumed_requests: pending.consumed_requests.clone(),
                decoded,
                fetch_predecessor_request: pending.fetch_predecessor_request,
                producer_register: pending.producer_register,
                producer_sequence: pending.producer_sequence,
                root_head: O3PendingDataAddressRootHead {
                    sequence: pending.root_sequence,
                    fetch_request: pending.root_fetch_request,
                    range: pending.root_range.clone(),
                    atomic_head: pending.root_atomic,
                },
                destination: pending.destination,
                lsq_kind: O3LoadStoreQueueKind::Load,
                expected_lsq_bytes: 8,
                published_producer_ready_tick: pending.published_producer_ready_tick,
                requested_wake_tick: pending.requested_wake_tick,
                selected_issue_tick: None,
                materialized: None,
            },
        ) {
            return Err(invalid("pending load owner cannot be restored"));
        }
    }
    runtime.live_data_access_younger_sequences =
        rows.iter().map(|pending| pending.sequence).collect();
    for (pending, decoded) in rows.iter().zip(decoded) {
        if !runtime.bind_live_staged_issue_packet_at_sequence(
            pending.sequence,
            decoded,
            &pending.consumed_requests,
            live.service.requested_tick,
        ) {
            return Err(invalid("production issue binder rejected pending load"));
        }
    }
    runtime.live_issue.install_checkpoint_projection(
        live.resident_sequences.clone(),
        live.service.requested_tick,
        live.service.mutation_generation,
        live.service.last_service_generation,
        restore_telemetry(live.service.telemetry),
    );
    if !runtime.pending_data_address_owner_is_consistent() {
        return Err(invalid("restored pending load ownership is inconsistent"));
    }
    match O3LiveIssueQueue::materialize(runtime, &live.resident_sequences)
        .map_err(|_| invalid("restored pending load queue does not materialize"))?
    {
        O3LiveIssueQueueCapture::Ready(queue)
            if queue
                .entries()
                .iter()
                .map(|row| row.sequence())
                .eq(rows.iter().map(|row| row.sequence)) => {}
        _ => return Err(invalid("restored pending load queue membership changed")),
    }
    Ok(())
}

fn validate_runtime_graph(
    runtime: &O3RuntimeState,
    captured_tick: u64,
    resident_sequences: &[u64],
    rows: &[&O3PendingDataAddress],
) -> Result<(), Error> {
    if rows.is_empty() || rows.len() > MAX_PENDING_ADDRESSES {
        return Err(invalid("pending load graph row count is unsupported"));
    }
    if rows.windows(2).any(|window| {
        window[0].sequence >= window[1].sequence
            || window[0].fetch.request_id().sequence() >= window[1].fetch.request_id().sequence()
    }) {
        return Err(invalid("pending load graph is not ordered"));
    }
    let root = rows[0];
    validate_runtime_authority(runtime, rows)?;
    validate_stable_runtime_owners(runtime, rows)?;
    let mut payload_rows = Vec::with_capacity(rows.len());
    let mut root_wakes = Vec::new();
    let mut destinations = BTreeSet::new();
    let mut physical = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let decoded = decode_runtime_pending_load(row)?;
        let destination = row
            .destination
            .ok_or(invalid("pending load lacks a destination"))?;
        let destination_register = row_destination_register(destination)?;
        let expected_predecessor = if index == 0 {
            row.root_head.fetch_request
        } else {
            rows[index - 1].fetch.request_id()
        };
        let expected_pc = root
            .fetch
            .pc()
            .get()
            .checked_add(4 * index as u64)
            .ok_or(invalid("pending load PC overflows"))?;
        if row.selected_issue_tick.is_some()
            || row.materialized.is_some()
            || row.root_head != root.root_head
            || row.root_head.sequence >= row.sequence
            || row.fetch_predecessor_request != expected_predecessor
            || row.fetch.pc().get() != expected_pc
            || row.consumed_requests != [row.fetch.request_id()]
            || row.root_head.atomic_head
            || row.lsq_kind != O3LoadStoreQueueKind::Load
            || row.expected_lsq_bytes != 8
            || !destinations.insert(destination.architectural())
            || !physical.insert(destination.physical().get())
            || destination.architectural() == u32::from(root.producer_register.index())
            || !runtime.live_staged_fetch_identity_matches(
                row.sequence,
                decoded.instruction(),
                &row.consumed_requests,
            )
        {
            return Err(invalid("pending load row is inconsistent"));
        }
        if row.producer_sequence == row.root_head.sequence {
            let (Some(published), Some(wake)) =
                (row.published_producer_ready_tick, row.requested_wake_tick)
            else {
                return Err(invalid("root-dependent pending load lacks wake"));
            };
            if row.producer_register != root.producer_register
                || published > captured_tick
                || captured_tick > wake
                || published > wake
            {
                return Err(invalid("root-dependent pending load wake is invalid"));
            }
            root_wakes.push(wake);
        } else {
            let valid_previous = index > 0
                && row.producer_sequence == rows[index - 1].sequence
                && row.producer_register
                    == row_destination_register(
                        rows[index - 1]
                            .destination
                            .expect("previous pending load destination"),
                    )?
                && row.published_producer_ready_tick.is_none()
                && row.requested_wake_tick.is_none();
            if !valid_previous {
                return Err(invalid("internal pending load dependency is invalid"));
            }
        }
        payload_rows.push(row.sequence);
        if destination_register.is_zero() {
            return Err(invalid("pending load destination is not a scalar register"));
        }
    }
    if resident_sequences != payload_rows
        || runtime.live_issue.requested_service_tick() != root_wakes.iter().min().copied()
    {
        return Err(invalid("pending load issue ownership is inconsistent"));
    }
    Ok(())
}

fn validate_runtime_authority(
    runtime: &O3RuntimeState,
    rows: &[&O3PendingDataAddress],
) -> Result<(), Error> {
    if !runtime.pending_data_address_owner_is_consistent()
        || !runtime.pending_data_accesses.is_empty()
        || runtime.store_forwarding_window != Default::default()
        || !runtime.live_retired_instructions.is_empty()
        || !runtime.live_speculative_executions.is_empty()
        || !runtime.live_data_accesses.is_empty()
        || runtime.deferred_live_data_access_execution.is_some()
        || !runtime.live_writeback_counted_sequences.is_empty()
        || !runtime.live_control_lineages.is_empty()
        || !runtime.live_serializing_control_sequences.is_empty()
        || !runtime.invalidated_live_staged_fetch_identities.is_empty()
        || !runtime.committed_live_staged_fetch_identities.is_empty()
        || runtime.live_data_access_younger_sequences
            != rows.iter().map(|row| row.sequence).collect::<BTreeSet<_>>()
    {
        return Err(invalid("pending load retains extra transient authority"));
    }
    Ok(())
}

fn validate_stable_runtime_owners(
    runtime: &O3RuntimeState,
    rows: &[&O3PendingDataAddress],
) -> Result<(), Error> {
    let root = rows[0].root_head;
    if runtime.snapshot.reorder_buffer.len() != rows.len()
        || runtime.snapshot.load_store_queue.len() != rows.len()
        || runtime
            .snapshot
            .reorder_buffer
            .iter()
            .any(|owner| owner.sequence() == root.sequence)
        || runtime
            .snapshot
            .load_store_queue
            .iter()
            .any(|owner| owner.sequence() == root.sequence)
    {
        return Err(invalid("pending load stable owner set is inconsistent"));
    }
    let committed_root_count = runtime
        .snapshot
        .rename_map
        .iter()
        .filter(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == u32::from(rows[0].producer_register.index())
        })
        .count();
    for (index, row) in rows.iter().enumerate() {
        let destination = row
            .destination
            .ok_or(invalid("pending load lacks a destination"))?;
        let rob = runtime.snapshot.reorder_buffer[index];
        let lsq = runtime.snapshot.load_store_queue[index];
        let packet = runtime
            .live_staged_issue_packet(row.sequence)
            .ok_or(invalid("resident pending load has no bound issue packet"))?;
        if rob.sequence() != row.sequence
            || rob.pc() != row.fetch.pc()
            || rob.destination() != Some(destination.physical())
            || rob.rename_destination()
                != Some((destination.register_class(), destination.architectural()))
            || !rob.is_live_staged()
            || rob.is_ready()
            || lsq.sequence() != row.sequence
            || lsq.kind() != O3LoadStoreQueueKind::Load
            || lsq.address().is_some()
            || lsq.bytes() != 8
            || lsq.is_completed()
            || packet.decoded() != row.decoded
            || packet.consumed_requests() != row.consumed_requests
        {
            return Err(invalid("pending load stable owner set is inconsistent"));
        }
    }
    if committed_root_count != 1 {
        return Err(invalid(
            "pending load lacks committed architectural producer authority",
        ));
    }
    Ok(())
}

fn validate_payload_shape(
    live: &RiscvO3LiveCheckpointPayload,
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) -> Result<(), Error> {
    let root = rows
        .first()
        .ok_or(invalid("pending load graph lacks rows"))?;
    let expected_issue = rows
        .iter()
        .map(|pending| RiscvO3LiveCheckpointIssueRow {
            sequence: pending.sequence,
            fetch_request: pending.fetch.request_id(),
        })
        .collect::<Vec<_>>();
    let expected_resident = rows
        .iter()
        .map(|pending| pending.sequence)
        .collect::<Vec<_>>();
    if !live.events.is_empty()
        || live.issue_rows != expected_issue
        || !live.rename_rows.is_empty()
        || live.resident_sequences != expected_resident
        || !live.executed_fetch_requests.is_empty()
        || !live.issued_fetch_requests.is_empty()
        || !live.writeback_counted_sequences.is_empty()
        || !live.writeback_published_sequences.is_empty()
        || live.reservation.is_some()
        || live.completed_result.is_some()
        || live.service.telemetry.current_occupancy != rows.len() as u64
        || live.service.telemetry.peak_occupancy < rows.len() as u64
        || !live
            .finalized_writeback
            .is_valid_without_live_calendar_at(live.captured_tick)
        || live.service.requested_tick != live.wake.tick
        || rows
            .iter()
            .any(|pending| live.wake.partition != pending.fetch.partition())
    {
        return Err(invalid("pending-address ownership is inconsistent"));
    }
    if rows.len() > MAX_PENDING_ADDRESSES
        || rows.iter().any(|row| row.destination.is_none())
        || rows.windows(2).any(|window| {
            window[0].sequence >= window[1].sequence
                || window[0].fetch.request_id().sequence()
                    >= window[1].fetch.request_id().sequence()
        })
    {
        return Err(invalid("pending load graph is not ordered"));
    }
    let mut root_wakes = Vec::new();
    let mut destination_arch = BTreeSet::new();
    let mut destination_physical = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let destination = row.destination.expect("checked pending load destination");
        let destination_register = row_destination_register(destination)?;
        let predecessor = if index == 0 {
            row.root_fetch_request
        } else {
            rows[index - 1].fetch.request_id()
        };
        let expected_pc = root
            .fetch
            .pc()
            .get()
            .checked_add(4 * index as u64)
            .ok_or(invalid("pending load PC overflows"))?;
        if row.fetch.kind() != crate::CpuFetchEventKind::Completed
            || row.fetch.size().bytes() != 4
            || row.fetch.data().is_none_or(|bytes| bytes.len() != 4)
            || row.consumed_requests != [row.fetch.request_id()]
            || row.fetch_predecessor_request != predecessor
            || row.root_sequence != root.root_sequence
            || row.root_fetch_request != root.root_fetch_request
            || row.root_range != root.root_range
            || row.root_atomic
            || row.fetch.partition() != root.fetch.partition()
            || row.fetch.route() != root.fetch.route()
            || row.fetch.endpoint() != root.fetch.endpoint()
            || row.sequence != live.resident_sequences[index]
            || row.root_sequence >= row.sequence
            || row.fetch.request_id().sequence() >= live.next_fetch_request_sequence
            || row.fetch.tick() > live.captured_tick
            || row.root_range.size().bytes() != 8
            || row.lsq_kind != O3LoadStoreQueueKind::Load
            || row.expected_lsq_bytes != 8
            || row.fetch.pc().get() != expected_pc
            || row.producer_register.is_zero()
            || destination.architectural() == u32::from(root.producer_register.index())
            || !destination_arch.insert(destination.architectural())
            || !destination_physical.insert(destination.physical().get())
        {
            return Err(invalid("pending load row is inconsistent"));
        }
        if row.producer_sequence == row.root_sequence {
            let (Some(published), Some(wake)) =
                (row.published_producer_ready_tick, row.requested_wake_tick)
            else {
                return Err(invalid("root-dependent pending load lacks wake"));
            };
            if row.producer_register != root.producer_register
                || published > live.captured_tick
                || live.captured_tick > wake
                || published > wake
            {
                return Err(invalid("root-dependent pending load wake is invalid"));
            }
            root_wakes.push(wake);
        } else {
            let valid_previous = index > 0
                && row.producer_sequence == rows[index - 1].sequence
                && row.producer_register
                    == row_destination_register(
                        rows[index - 1]
                            .destination
                            .expect("previous pending load destination"),
                    )?
                && row.published_producer_ready_tick.is_none()
                && row.requested_wake_tick.is_none();
            if !valid_previous {
                return Err(invalid("internal pending load dependency is invalid"));
            }
        }
        validate_load_instruction(row, destination_register)?;
    }
    if root.producer_sequence != root.root_sequence
        || root.root_fetch_request.agent() != root.fetch.request_id().agent()
        || root.root_fetch_request.sequence() >= root.fetch.request_id().sequence()
        || root_wakes.iter().min().copied() != Some(live.service.requested_tick)
        || rows
            .last()
            .and_then(|row| row.fetch.pc().get().checked_add(4))
            != Some(live.next_fetch_pc.get())
    {
        return Err(invalid("pending load wake or root is inconsistent"));
    }
    Ok(())
}

fn decode_runtime_pending_load(
    pending: &O3PendingDataAddress,
) -> Result<RiscvDecodedInstruction, Error> {
    let destination = pending
        .destination
        .ok_or(invalid("pending load lacks a destination"))?;
    let destination_register = row_destination_register(destination)?;
    let payload = RiscvO3LiveCheckpointPendingDataAddress {
        sequence: pending.sequence,
        fetch: pending.fetch.clone(),
        consumed_requests: pending.consumed_requests.clone(),
        fetch_predecessor_request: pending.fetch_predecessor_request,
        producer_register: pending.producer_register,
        destination: pending.destination,
        producer_sequence: pending.producer_sequence,
        root_sequence: pending.root_head.sequence,
        root_fetch_request: pending.root_head.fetch_request,
        root_range: pending.root_head.range,
        root_atomic: pending.root_head.atomic_head,
        lsq_kind: pending.lsq_kind,
        expected_lsq_bytes: pending.expected_lsq_bytes,
        published_producer_ready_tick: pending.published_producer_ready_tick,
        requested_wake_tick: pending.requested_wake_tick,
    };
    validate_load_instruction(&payload, destination_register)
}

fn decode_pending_load(
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<RiscvDecodedInstruction, Error> {
    let destination = pending
        .destination
        .ok_or(invalid("pending load lacks a destination"))?;
    validate_load_instruction(pending, row_destination_register(destination)?)
}

fn validate_load_instruction(
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
    destination_register: Register,
) -> Result<RiscvDecodedInstruction, Error> {
    let raw: [u8; 4] = pending
        .fetch
        .data()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(invalid(
            "pending load fetch is not an uncompressed instruction",
        ))?;
    let decoded = RiscvInstruction::decode_with_length(u32::from_le_bytes(raw))
        .map_err(|_| invalid("pending load instruction does not decode"))?;
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
            "pending load instruction is not a destinationful doubleword load",
        ));
    }
    Ok(decoded)
}

fn row_destination_register(value: O3RenameMapEntry) -> Result<Register, Error> {
    if value.register_class() != O3RegisterClass::Integer
        || value.architectural() >= 32
        || value.physical().is_invalid()
    {
        return Err(invalid("pending load destination is not a scalar register"));
    }
    Register::new(value.architectural() as u8)
        .map_err(|_| invalid("pending load destination is not a scalar register"))
}
