use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction, RiscvExecutionRecord,
    RiscvInstruction,
};
use rem6_memory::{AccessSize, Address, AddressRange, MemoryRequestId};

use super::*;
use crate::{CpuFetchEvent, CpuFetchEventKind};

const PENDING_DATA_ADDRESS_LSQ_BYTES: u32 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3PendingDataAddress {
    pub(super) sequence: u64,
    pub(super) fetch: CpuFetchEvent,
    pub(super) consumed_requests: Vec<MemoryRequestId>,
    pub(super) decoded: RiscvDecodedInstruction,
    pub(super) producer_register: Register,
    pub(super) producer_sequence: u64,
    pub(super) producer_fetch: MemoryRequestId,
    pub(super) destination: O3RenameMapEntry,
    pub(super) expected_lsq_bytes: u32,
    pub(super) head_range: AddressRange,
    pub(super) atomic_head: bool,
    pub(super) requested_wake_tick: Option<u64>,
    pub(super) selected_issue_tick: Option<u64>,
    pub(super) materialized: Option<RiscvCpuExecutionEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3PendingDataAddressRequest {
    pub(crate) fetch: CpuFetchEvent,
    pub(crate) consumed_requests: Vec<MemoryRequestId>,
    pub(crate) decoded: RiscvDecodedInstruction,
    pub(crate) producer_register: Register,
}

impl O3PendingDataAddress {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }
}

impl O3PendingDataAddressRequest {
    pub(crate) fn new(
        fetch: CpuFetchEvent,
        consumed_requests: Vec<MemoryRequestId>,
        decoded: RiscvDecodedInstruction,
        producer_register: Register,
    ) -> Self {
        Self {
            fetch,
            consumed_requests,
            decoded,
            producer_register,
        }
    }

    #[cfg(test)]
    pub(crate) const fn fetch(&self) -> &CpuFetchEvent {
        &self.fetch
    }
}

impl O3RuntimeState {
    pub(crate) const fn has_pending_data_address(&self) -> bool {
        self.pending_data_address.is_some()
    }

    pub(crate) fn pending_data_address_owns_fetch(&self, fetch_request: MemoryRequestId) -> bool {
        self.pending_data_address.as_ref().is_some_and(|pending| {
            pending.fetch.request_id() == fetch_request
                || pending.consumed_requests.contains(&fetch_request)
        })
    }

    pub(crate) fn pending_data_address_execution(&self) -> Option<&RiscvCpuExecutionEvent> {
        self.pending_data_address
            .as_ref()
            .and_then(|pending| pending.materialized.as_ref())
    }

    pub(crate) fn pending_data_address_execution_mut(
        &mut self,
    ) -> Option<&mut RiscvCpuExecutionEvent> {
        self.pending_data_address
            .as_mut()
            .and_then(|pending| pending.materialized.as_mut())
    }

    pub(crate) fn pending_data_address_decoded(
        &self,
        fetch_request: MemoryRequestId,
    ) -> Option<RiscvDecodedInstruction> {
        let pending = self.pending_data_address.as_ref()?;
        (pending.fetch.request_id() == fetch_request).then_some(pending.decoded)
    }

    pub(crate) fn pending_data_address_issue_matches(
        &self,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
        physical_address: Address,
        size: AccessSize,
        request_tick: u64,
    ) -> bool {
        if !self.pending_data_address_can_issue(fetch_request, access) {
            return false;
        }
        let Some(pending) = self.pending_data_address.as_ref() else {
            return false;
        };
        let Some(execution) = pending.materialized.as_ref() else {
            return false;
        };
        let Some(issue_tick) = pending.selected_issue_tick else {
            return false;
        };
        let RiscvInstruction::Load {
            rd,
            rs1,
            width: MemoryWidth::Doubleword,
            ..
        } = pending.decoded.instruction()
        else {
            return false;
        };
        let Some(MemoryAccessKind::Load {
            rd: access_rd,
            address,
            width: MemoryWidth::Doubleword,
            ..
        }) = execution.execution().memory_access()
        else {
            return false;
        };
        let Ok(range) = AddressRange::new(physical_address, size) else {
            return false;
        };
        pending.fetch.request_id() == fetch_request
            && pending.fetch.pc() == execution.fetch().pc()
            && pending.decoded.instruction() == execution.instruction()
            && pending.producer_register == rs1
            && pending.destination.register_class() == O3RegisterClass::Integer
            && pending.destination.architectural() == u32::from(rd.index())
            && *access_rd == rd
            && execution.execution().memory_access() == Some(access)
            && physical_address.get() == *address
            && size.bytes() == u64::from(pending.expected_lsq_bytes)
            && request_tick >= issue_tick
            && (!pending.atomic_head || !pending.head_range.overlaps(range))
            && self.live_staged_fetch_identity_matches(
                pending.sequence,
                pending.decoded.instruction(),
                &pending.consumed_requests,
            )
    }

    pub(crate) fn stage_pending_data_address_window(
        &mut self,
        head_fetch: MemoryRequestId,
        pending: O3PendingDataAddressRequest,
        suffix: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) -> usize {
        if self.pending_data_address.is_some() {
            return 0;
        }
        let Some((producer_sequence, head_range, atomic_head)) =
            self.pending_data_address_head_metadata(head_fetch, pending.producer_register)
        else {
            return 0;
        };
        let Some(pending_rd) = self.validate_pending_data_address_request(&pending) else {
            return 0;
        };
        let Some(mut window) =
            crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_memory_results(
                [pending.producer_register, pending_rd],
                2,
                self.scalar_memory_window_limit,
            )
        else {
            return 0;
        };
        let Some((sequence, Some(physical_destination))) = self
            .stage_live_instruction_with_rename_destination(
                pending.fetch.pc(),
                pending.decoded.instruction(),
                0,
                Some((O3RegisterClass::Integer, u32::from(pending_rd.index()))),
            )
        else {
            return 0;
        };
        let destination = O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            u32::from(pending_rd.index()),
            physical_destination,
        );
        if !self.bind_live_staged_fetch_identity_at_sequence(
            sequence,
            pending.decoded.instruction(),
            &pending.consumed_requests,
        ) {
            self.discard_live_staged_window_from(sequence);
            return 0;
        }
        self.snapshot
            .load_store_queue
            .push(O3LoadStoreQueueEntry::load(
                sequence,
                None,
                PENDING_DATA_ADDRESS_LSQ_BYTES,
            ));
        self.live_data_access_younger_sequences.insert(sequence);
        self.pending_data_address = Some(O3PendingDataAddress {
            sequence,
            fetch: pending.fetch,
            consumed_requests: pending.consumed_requests,
            decoded: pending.decoded,
            producer_register: pending.producer_register,
            producer_sequence,
            producer_fetch: head_fetch,
            destination,
            expected_lsq_bytes: PENDING_DATA_ADDRESS_LSQ_BYTES,
            head_range,
            atomic_head,
            requested_wake_tick: None,
            selected_issue_tick: None,
            materialized: None,
        });

        let mut staged = 1;
        for (pc, instruction) in suffix.into_iter().take(2) {
            let decision = window.classify_younger(instruction);
            if decision == crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::Reject
            {
                break;
            }
            let Some(sequence) = self.stage_live_instruction(pc, instruction, 0) else {
                break;
            };
            self.live_data_access_younger_sequences.insert(sequence);
            staged += 1;
        }
        debug_assert!(self.pending_data_address_owner_is_consistent());
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .observe_lsq_occupancy(self.snapshot.load_store_queue.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        staged
    }

    pub(super) fn record_pending_data_address_materialization(
        &mut self,
        candidate: O3LiveSpeculativeIssueCandidate,
        consumed_requests: &[MemoryRequestId],
        issue_tick: u64,
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        let Some(pending) = self.pending_data_address.as_ref().cloned() else {
            return Ok(false);
        };
        let producer = candidate.data_producers();
        let valid_producer = producer.len() == 1
            && producer[0].sequence() == pending.producer_sequence
            && producer[0].source() == pending.producer_register
            && candidate.producer_sequences() == [pending.producer_sequence];
        let valid_lsq = pending.expected_lsq_bytes == PENDING_DATA_ADDRESS_LSQ_BYTES
            && self.snapshot.load_store_queue.iter().any(|entry| {
                entry.sequence() == pending.sequence
                    && entry.kind() == O3LoadStoreQueueKind::Load
                    && entry.address().is_none()
                    && entry.bytes() == pending.expected_lsq_bytes
            });
        let expected_destination = u32::from(match execution.memory_access() {
            Some(MemoryAccessKind::Load {
                rd,
                width: MemoryWidth::Doubleword,
                ..
            }) if !rd.is_zero() => rd.index(),
            _ => return Ok(false),
        });
        if candidate.sequence() != pending.sequence
            || candidate.instruction() != pending.decoded.instruction()
            || candidate.pending_data_address_destination() != Some(pending.destination)
            || pending.destination.register_class() != O3RegisterClass::Integer
            || pending.destination.architectural() != expected_destination
            || pending.consumed_requests != consumed_requests
            || !valid_producer
            || !valid_lsq
            || !self.live_staged_fetch_identity_matches(
                pending.sequence,
                pending.decoded.instruction(),
                consumed_requests,
            )
            || execution.pc() != pending.fetch.pc().get()
            || execution.instruction() != pending.decoded.instruction()
            || execution.instruction_bytes() != pending.decoded.bytes()
            || execution.next_pc()
                != execution
                    .pc()
                    .wrapping_add(u64::from(execution.instruction_bytes()))
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || !execution.register_writes().is_empty()
            || !execution.float_register_writes().is_empty()
        {
            return Ok(false);
        }
        let event =
            RiscvCpuExecutionEvent::new(pending.fetch, pending.decoded.instruction(), execution);
        let Some(stored) = self.pending_data_address.as_mut() else {
            return Ok(false);
        };
        if let Some(materialized) = &stored.materialized {
            return Ok(stored.selected_issue_tick == Some(issue_tick) && materialized == &event);
        }
        stored.requested_wake_tick = None;
        stored.selected_issue_tick = Some(issue_tick);
        stored.materialized = Some(event);
        Ok(true)
    }

    pub(super) fn pending_data_address_materialization_matches(
        &self,
        request: &O3LiveIssueRequest,
    ) -> bool {
        self.pending_data_address.as_ref().is_some_and(|pending| {
            pending.materialized.is_some()
                && pending.fetch.pc() == request.pc()
                && pending.decoded == request.decoded()
                && pending.consumed_requests == request.consumed_requests()
        })
    }

    fn discard_pending_data_address_at_internal(&mut self, now: Option<u64>) {
        let Some(pending) = self.pending_data_address.take() else {
            return;
        };
        self.snapshot
            .load_store_queue
            .retain(|entry| entry.sequence() != pending.sequence);
        match now {
            Some(now) => self.discard_live_staged_window_from_at(pending.sequence, now),
            None => self.discard_live_staged_window_from(pending.sequence),
        }
    }

    pub(super) fn discard_pending_data_address_from(&mut self, sequence: u64) {
        if self
            .pending_data_address
            .as_ref()
            .is_some_and(|pending| pending.sequence >= sequence)
        {
            self.discard_pending_data_address_at_internal(None);
        }
    }

    pub(crate) fn discard_pending_data_address(&mut self) {
        self.discard_pending_data_address_at_internal(None);
    }

    pub(crate) fn discard_pending_data_address_at(&mut self, now: u64) {
        self.discard_pending_data_address_at_internal(Some(now));
    }

    pub(crate) fn bind_pending_data_address_issue(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        physical_address: Address,
        request_tick: u64,
    ) -> Option<Vec<MemoryRequestId>> {
        let access = execution
            .execution()
            .memory_access()
            .expect("pending address execution carries a load");
        let size = AccessSize::new(u64::from(PENDING_DATA_ADDRESS_LSQ_BYTES))
            .expect("pending data address size");
        let owns_fetch = self.pending_data_address_owns_fetch(execution.fetch().request_id());
        if !owns_fetch {
            return None;
        }
        assert!(self.pending_data_address_issue_matches(
            execution.fetch().request_id(),
            access,
            physical_address,
            size,
            request_tick,
        ));
        let pending = self.pending_data_address.as_ref()?;
        let issue_tick = pending
            .selected_issue_tick
            .expect("materialized pending address has a selected issue tick");
        assert!(!self
            .live_data_accesses
            .iter()
            .any(|live| live.data_request == data_request));
        assert!(self.snapshot.reorder_buffer.iter().any(|entry| {
            entry.sequence() == pending.sequence
                && entry.destination() == Some(pending.destination.physical())
        }));

        let sequence = pending.sequence;
        let consumed_requests = pending.consumed_requests.clone();
        let issue_rob_occupancy = self.snapshot.reorder_buffer.len();
        let issue_lsq_occupancy = self.snapshot.load_store_queue.len();
        let lsq = self
            .snapshot
            .load_store_queue
            .iter_mut()
            .find(|entry| entry.sequence() == sequence)
            .expect("pending address LSQ row");
        assert_eq!(lsq.kind(), O3LoadStoreQueueKind::Load);
        assert_eq!(lsq.bytes(), PENDING_DATA_ADDRESS_LSQ_BYTES);
        assert!(lsq.resolve_address(physical_address));

        self.pending_data_address = None;
        self.live_data_access_younger_sequences.remove(&sequence);
        self.live_data_accesses.push(O3LiveDataAccess {
            fetch_request: execution.fetch().request_id(),
            data_request,
            execution: execution.clone(),
            sequence,
            lsq_sequence_span: 1,
            issue_tick,
            issue_rob_occupancy,
            issue_lsq_occupancy,
            younger_window_policy: O3DataAccessWindowPolicy::MemoryResultWindow,
            response_tick: None,
            latency_ticks: None,
            commit_tick: None,
            load_data: None,
            memory_result: None,
            forwarding_plan: None,
            outcome: O3LiveDataAccessOutcome::Resident,
            event_taken: false,
        });
        Some(consumed_requests)
    }

    fn pending_data_address_head_metadata_for_sequence(
        &self,
        producer_sequence: u64,
        producer_register: Register,
    ) -> Option<(AddressRange, bool)> {
        let live = self.live_data_accesses.iter().find(|live| {
            live.sequence == producer_sequence
                && live.outcome == O3LiveDataAccessOutcome::Resident
                && live.younger_window_policy == O3DataAccessWindowPolicy::MemoryResultWindow
        })?;
        let access = live.execution.execution().memory_access()?;
        let (destination, atomic_head) = match access {
            MemoryAccessKind::Load {
                rd,
                width: MemoryWidth::Doubleword,
                ..
            } if !rd.is_zero() => (*rd, false),
            MemoryAccessKind::AtomicMemory {
                rd,
                acquire: false,
                release: false,
                ..
            } if !rd.is_zero() => (*rd, true),
            _ => return None,
        };
        if destination != producer_register {
            return None;
        }
        Some((o3_memory_result_range(access)?, atomic_head))
    }

    fn pending_data_address_head_metadata(
        &self,
        head_fetch: MemoryRequestId,
        producer_register: Register,
    ) -> Option<(u64, AddressRange, bool)> {
        let live = self.live_data_accesses.iter().find(|live| {
            live.fetch_request == head_fetch
                && live.outcome == O3LiveDataAccessOutcome::Resident
                && live.younger_window_policy == O3DataAccessWindowPolicy::MemoryResultWindow
        })?;
        if !self.live_data_access_younger_sequences.is_empty()
            || self.live_data_accesses.len() != 1
            || self
                .snapshot
                .reorder_buffer
                .iter()
                .any(|entry| entry.is_live_staged() && entry.sequence() > live.sequence)
        {
            return None;
        }
        let access = live.execution.execution().memory_access()?;
        let (destination, atomic_head) = match access {
            MemoryAccessKind::Load {
                rd,
                width: MemoryWidth::Doubleword,
                ..
            } if !rd.is_zero() => (*rd, false),
            MemoryAccessKind::AtomicMemory {
                rd,
                acquire: false,
                release: false,
                ..
            } if !rd.is_zero() => (*rd, true),
            _ => return None,
        };
        if destination != producer_register {
            return None;
        }
        Some((live.sequence, o3_memory_result_range(access)?, atomic_head))
    }

    fn validate_pending_data_address_request(
        &self,
        pending: &O3PendingDataAddressRequest,
    ) -> Option<Register> {
        if pending.fetch.kind() != CpuFetchEventKind::Completed
            || pending.consumed_requests.first().copied() != Some(pending.fetch.request_id())
            || !valid_live_speculative_fetch_identity(&pending.consumed_requests)
            || !self.pending_fetch_decodes_to_request(&pending.fetch, pending.decoded)
            || self.live_staged_fetch_identities.values().any(|identity| {
                pending
                    .consumed_requests
                    .iter()
                    .copied()
                    .any(|request| identity.owns_fetch_request(request))
            })
            || self
                .invalidated_live_staged_fetch_identities
                .values()
                .any(|identity| {
                    pending
                        .consumed_requests
                        .iter()
                        .copied()
                        .any(|request| identity.owns_fetch_request(request))
                })
        {
            return None;
        }
        let RiscvInstruction::Load {
            rd,
            rs1,
            width: MemoryWidth::Doubleword,
            ..
        } = pending.decoded.instruction()
        else {
            return None;
        };
        (pending.decoded.bytes() == 4 && !rd.is_zero() && rs1 == pending.producer_register)
            .then_some(rd)
    }

    fn pending_fetch_decodes_to_request(
        &self,
        fetch: &CpuFetchEvent,
        decoded: RiscvDecodedInstruction,
    ) -> bool {
        let Some([a, b, c, d]) = fetch.data() else {
            return false;
        };
        let raw = u32::from_le_bytes([*a, *b, *c, *d]);
        RiscvInstruction::decode_with_length(raw).ok() == Some(decoded)
    }

    fn pending_data_address_owner_is_consistent(&self) -> bool {
        let Some(pending) = &self.pending_data_address else {
            return true;
        };
        let Some((head_range, atomic_head)) = self.pending_data_address_head_metadata_for_sequence(
            pending.producer_sequence,
            pending.producer_register,
        ) else {
            return false;
        };
        head_range == pending.head_range
            && atomic_head == pending.atomic_head
            && pending.expected_lsq_bytes == PENDING_DATA_ADDRESS_LSQ_BYTES
            && pending.materialized.is_some() == pending.selected_issue_tick.is_some()
            && !(pending.materialized.is_some() && pending.requested_wake_tick.is_some())
            && self.snapshot.reorder_buffer.iter().any(|entry| {
                entry.sequence() == pending.sequence
                    && entry.destination() == Some(pending.destination.physical())
                    && entry.rename_destination()
                        == Some((
                            pending.destination.register_class(),
                            pending.destination.architectural(),
                        ))
            })
            && self
                .snapshot_with_live_rename_map()
                .rename_map()
                .contains(&pending.destination)
            && self.snapshot.load_store_queue.iter().any(|entry| {
                entry.sequence() == pending.sequence
                    && entry.kind() == O3LoadStoreQueueKind::Load
                    && entry.address().is_none()
                    && entry.bytes() == pending.expected_lsq_bytes
            })
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_sequence_for_test(&self) -> Option<u64> {
        self.pending_data_address
            .as_ref()
            .map(O3PendingDataAddress::sequence)
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_owner_count_for_test(&self) -> usize {
        usize::from(self.pending_data_address.is_some())
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_selected_issue_tick_for_test(&self) -> Option<u64> {
        self.pending_data_address
            .as_ref()
            .and_then(|pending| pending.selected_issue_tick)
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_materialized_execution_for_test(
        &self,
    ) -> Option<&RiscvCpuExecutionEvent> {
        self.pending_data_address
            .as_ref()
            .and_then(|pending| pending.materialized.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn corrupt_pending_data_address_lsq_bytes_for_test(&mut self, bytes: u32) {
        if let Some(pending) = self.pending_data_address.as_mut() {
            pending.expected_lsq_bytes = bytes;
        }
    }

    #[cfg(test)]
    pub(crate) fn corrupt_pending_data_address_producer_sequence_for_test(
        &mut self,
        sequence: u64,
    ) {
        if let Some(pending) = self.pending_data_address.as_mut() {
            pending.producer_sequence = sequence;
        }
    }
}
