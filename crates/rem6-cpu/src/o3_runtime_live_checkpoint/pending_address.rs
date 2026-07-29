use super::super::o3_runtime_pending_address::O3PendingDataAddressRootHead;
use super::*;

pub(super) fn capture(
    runtime: &O3RuntimeState,
    captured_tick: u64,
    resident_sequences: &[u64],
) -> Result<Option<RiscvO3LiveCheckpointPendingDataAddress>, Error> {
    if runtime.pending_data_addresses.len() != 1 {
        return Ok(None);
    }
    let row = runtime
        .pending_data_addresses
        .first()
        .expect("one pending-address row exists");
    let (Some(published_tick), Some(requested_wake_tick)) =
        (row.published_producer_ready_tick, row.requested_wake_tick)
    else {
        return Err(invalid("pending store lacks publication or wake authority"));
    };
    if row.destination.is_some()
        || row.lsq_kind != O3LoadStoreQueueKind::Store
        || row.expected_lsq_bytes != 8
        || row.selected_issue_tick.is_some()
        || row.materialized.is_some()
        || row.root_head.sequence != row.producer_sequence
        || row.producer_sequence >= row.sequence
        || published_tick > captured_tick
        || captured_tick > requested_wake_tick
    {
        return Err(invalid("pending store shape is unsupported"));
    }

    let [rob] = runtime.snapshot.reorder_buffer.as_slice() else {
        return Err(invalid("pending store does not own exactly one ROB row"));
    };
    let [lsq] = runtime.snapshot.load_store_queue.as_slice() else {
        return Err(invalid("pending store does not own exactly one LSQ row"));
    };
    let Some(packet) = runtime.live_staged_issue_packet(row.sequence) else {
        return Err(invalid("pending store lacks its production issue packet"));
    };
    if resident_sequences != [row.sequence]
        || rob.sequence() != row.sequence
        || rob.pc() != row.fetch.pc()
        || rob.destination().is_some()
        || rob.rename_destination().is_some()
        || !rob.is_live_staged()
        || rob.is_ready()
        || lsq.sequence() != row.sequence
        || lsq.kind() != O3LoadStoreQueueKind::Store
        || lsq.address().is_some()
        || lsq.bytes() != 8
        || lsq.is_completed()
        || packet.decoded() != row.decoded
        || packet.consumed_requests() != row.consumed_requests
        || !runtime.live_staged_fetch_identity_matches(
            row.sequence,
            row.decoded.instruction(),
            &row.consumed_requests,
        )
        || runtime.live_issue.requested_service_tick() != Some(requested_wake_tick)
        || runtime.live_data_access_younger_sequences != BTreeSet::from([row.sequence])
    {
        return Err(invalid("pending store owner set is inconsistent"));
    }

    let committed_producer = runtime.snapshot.rename_map.iter().find(|entry| {
        entry.register_class() == O3RegisterClass::Integer
            && entry.architectural() == u32::from(row.producer_register.index())
    });
    if committed_producer.is_none()
        || runtime.snapshot.reorder_buffer.iter().any(|owner| {
            owner.sequence() == row.root_head.sequence || owner.sequence() == row.producer_sequence
        })
        || runtime.snapshot.load_store_queue.iter().any(|owner| {
            owner.sequence() == row.root_head.sequence || owner.sequence() == row.producer_sequence
        })
    {
        return Err(invalid(
            "pending store lacks committed architectural producer authority",
        ));
    }

    if !runtime.pending_data_address_owner_is_consistent()
        || !runtime.pending_data_accesses.is_empty()
        || runtime.store_forwarding_window != Default::default()
        || !runtime.live_retired_instructions.is_empty()
        || !runtime.live_speculative_executions.is_empty()
        || !runtime.live_data_accesses.is_empty()
        || runtime.deferred_live_data_access_execution.is_some()
        || !runtime.live_writeback_counted_sequences.is_empty()
    {
        return Err(invalid("pending store retains extra transient authority"));
    }

    Ok(Some(RiscvO3LiveCheckpointPendingDataAddress {
        sequence: row.sequence,
        fetch: row.fetch.clone(),
        consumed_requests: row.consumed_requests.clone(),
        fetch_predecessor_request: row.fetch_predecessor_request,
        producer_register: row.producer_register,
        destination: None,
        producer_sequence: row.producer_sequence,
        root_sequence: row.root_head.sequence,
        root_fetch_request: row.root_head.fetch_request,
        root_range: row.root_head.range,
        root_atomic: row.root_head.atomic_head,
        lsq_kind: row.lsq_kind,
        expected_lsq_bytes: row.expected_lsq_bytes,
        published_producer_ready_tick: Some(published_tick),
        requested_wake_tick: Some(requested_wake_tick),
    }))
}

pub(super) fn validate_restore_payload(
    live: &RiscvO3LiveCheckpointPayload,
) -> Result<&RiscvO3LiveCheckpointPendingDataAddress, Error> {
    let [pending] = live.pending_addresses.as_slice() else {
        return Err(invalid(
            "pending-address profile lacks its exact pending row",
        ));
    };
    let (Some(published_tick), Some(requested_wake_tick)) = (
        pending.published_producer_ready_tick,
        pending.requested_wake_tick,
    ) else {
        return Err(invalid("pending store lacks publication or wake"));
    };
    let expected_issue = RiscvO3LiveCheckpointIssueRow {
        sequence: pending.sequence,
        fetch_request: pending.fetch.request_id(),
    };
    if !live.events.is_empty()
        || live.issue_rows != [expected_issue]
        || !live.rename_rows.is_empty()
        || live.resident_sequences != [pending.sequence]
        || !live.executed_fetch_requests.is_empty()
        || !live.issued_fetch_requests.is_empty()
        || !live.writeback_counted_sequences.is_empty()
        || !live.writeback_published_sequences.is_empty()
        || live.reservation.is_some()
        || live.completed_result.is_some()
        || pending.destination.is_some()
        || live.service.requested_tick != requested_wake_tick
        || live.wake.tick != requested_wake_tick
        || live.wake.partition != pending.fetch.partition()
    {
        return Err(invalid("pending-address ownership is inconsistent"));
    }

    let fetch_request = pending.fetch.request_id();
    if pending.fetch.kind() != crate::CpuFetchEventKind::Completed
        || pending.fetch.size().bytes() != 4
        || pending.fetch.data().is_none_or(|bytes| bytes.len() != 4)
        || pending.consumed_requests != [fetch_request]
        || pending.fetch_predecessor_request != pending.root_fetch_request
        || pending.root_fetch_request.agent() != fetch_request.agent()
        || pending.root_fetch_request.sequence() >= fetch_request.sequence()
        || fetch_request.sequence() >= live.next_fetch_request_sequence
        || pending.fetch.tick() > live.captured_tick
        || pending.producer_register.is_zero()
        || pending.root_sequence != pending.producer_sequence
        || pending.producer_sequence >= pending.sequence
        || pending.root_range.size().bytes() != 8
        || pending.lsq_kind != O3LoadStoreQueueKind::Store
        || pending.expected_lsq_bytes != 8
        || published_tick > live.captured_tick
        || live.captured_tick > requested_wake_tick
        || pending
            .fetch
            .pc()
            .get()
            .checked_add(4)
            .is_none_or(|next_pc| next_pc != live.next_fetch_pc.get())
    {
        return Err(invalid("pending-address row is inconsistent"));
    }
    decode_pending_store(pending)?;
    Ok(pending)
}

pub(super) fn validate_stable_owner_set(
    runtime: &O3RuntimeState,
    _live: &RiscvO3LiveCheckpointPayload,
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<(), Error> {
    let [rob] = runtime.snapshot.reorder_buffer.as_slice() else {
        return Err(invalid("pending store does not own one stable ROB row"));
    };
    let [lsq] = runtime.snapshot.load_store_queue.as_slice() else {
        return Err(invalid("pending store does not own one stable LSQ row"));
    };
    let committed_producer_count = runtime
        .snapshot
        .rename_map
        .iter()
        .filter(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == u32::from(pending.producer_register.index())
        })
        .count();
    if rob.sequence() != pending.sequence
        || rob.pc() != pending.fetch.pc()
        || rob.destination().is_some()
        || rob.rename_destination().is_some()
        || !rob.is_live_staged()
        || rob.is_ready()
        || lsq.sequence() != pending.sequence
        || lsq.kind() != O3LoadStoreQueueKind::Store
        || lsq.address().is_some()
        || lsq.bytes() != 8
        || lsq.is_completed()
        || committed_producer_count != 1
    {
        return Err(invalid("pending store stable owner set is inconsistent"));
    }
    Ok(())
}

pub(super) fn restore(
    runtime: &mut O3RuntimeState,
    live: &RiscvO3LiveCheckpointPayload,
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<(), Error> {
    let decoded = decode_pending_store(pending)?;
    runtime.live_staged_fetch_identities.insert(
        pending.sequence,
        O3LiveStagedFetchIdentity::new(decoded.instruction()),
    );
    if !runtime
        .pending_data_addresses
        .try_push(O3PendingDataAddress {
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
            destination: None,
            lsq_kind: O3LoadStoreQueueKind::Store,
            expected_lsq_bytes: 8,
            published_producer_ready_tick: pending.published_producer_ready_tick,
            requested_wake_tick: pending.requested_wake_tick,
            selected_issue_tick: None,
            materialized: None,
        })
    {
        return Err(invalid("pending store owner cannot be restored"));
    }
    runtime.live_data_access_younger_sequences = BTreeSet::from([pending.sequence]);
    if !runtime.bind_live_staged_issue_packet_at_sequence(
        pending.sequence,
        decoded,
        &pending.consumed_requests,
        live.service.requested_tick,
    ) {
        return Err(invalid("production issue binder rejected pending store"));
    }
    runtime.live_issue.install_checkpoint_projection(
        live.resident_sequences.clone(),
        live.service.requested_tick,
        live.service.mutation_generation,
        live.service.last_service_generation,
        restore_telemetry(live.service.telemetry),
    );
    if !runtime.pending_data_address_owner_is_consistent() {
        return Err(invalid("restored pending store ownership is inconsistent"));
    }
    match O3LiveIssueQueue::materialize(runtime, &live.resident_sequences)
        .map_err(|_| invalid("restored pending store queue does not materialize"))?
    {
        O3LiveIssueQueueCapture::Ready(queue)
            if queue.entries().len() == 1 && queue.entries()[0].sequence() == pending.sequence => {}
        _ => return Err(invalid("restored pending store queue membership changed")),
    }
    Ok(())
}

fn decode_pending_store(
    pending: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<rem6_isa_riscv::RiscvDecodedInstruction, Error> {
    let raw: [u8; 4] = pending
        .fetch
        .data()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(invalid(
            "pending store fetch is not an uncompressed instruction",
        ))?;
    let decoded = RiscvInstruction::decode_with_length(u32::from_le_bytes(raw))
        .map_err(|_| invalid("pending store instruction does not decode"))?;
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
            "pending store instruction is not canonical doubleword store",
        ));
    }
    Ok(decoded)
}
