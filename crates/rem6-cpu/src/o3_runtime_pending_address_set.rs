use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, RiscvDecodedInstruction, RiscvInstruction};
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use super::o3_runtime_pending_address::{O3PendingDataAddress, PENDING_DATA_ADDRESS_LSQ_BYTES};
use super::*;

pub(super) const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 2;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct O3PendingDataAddresses {
    rows: Vec<O3PendingDataAddress>,
}

impl O3PendingDataAddresses {
    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(super) fn first(&self) -> Option<&O3PendingDataAddress> {
        self.rows.first()
    }

    pub(super) fn first_mut(&mut self) -> Option<&mut O3PendingDataAddress> {
        self.rows.first_mut()
    }

    pub(super) fn find_sequence(&self, sequence: u64) -> Option<&O3PendingDataAddress> {
        self.rows.iter().find(|row| row.sequence == sequence)
    }

    pub(super) fn find_fetch(&self, request: MemoryRequestId) -> Option<&O3PendingDataAddress> {
        self.rows.iter().find(|row| {
            row.fetch.request_id() == request || row.consumed_requests.contains(&request)
        })
    }

    pub(super) fn try_push(&mut self, row: O3PendingDataAddress) -> bool {
        let duplicate_fetch = self.rows.iter().any(|existing| {
            existing.fetch.request_id() == row.fetch.request_id()
                || existing.consumed_requests.contains(&row.fetch.request_id())
                || row.consumed_requests.contains(&existing.fetch.request_id())
                || existing
                    .consumed_requests
                    .iter()
                    .any(|request| row.consumed_requests.contains(request))
        });
        if self.rows.len() >= O3_PENDING_DATA_ADDRESS_CAPACITY
            || self
                .rows
                .iter()
                .any(|existing| existing.sequence == row.sequence)
            || self
                .rows
                .last()
                .is_some_and(|existing| existing.sequence >= row.sequence)
            || duplicate_fetch
        {
            return false;
        }
        let sequence = row.sequence;
        self.rows.push(row);
        debug_assert!(self.find_sequence(sequence).is_some());
        true
    }

    fn take_first(&mut self) -> O3PendingDataAddress {
        self.rows.remove(0)
    }
}

impl O3RuntimeState {
    pub(crate) fn pending_data_address_count(&self) -> usize {
        self.pending_data_addresses.len()
    }

    pub(crate) fn has_pending_data_address(&self) -> bool {
        self.pending_data_address_count() != 0
    }

    pub(crate) fn pending_data_address_owns_fetch(&self, fetch_request: MemoryRequestId) -> bool {
        self.pending_data_addresses
            .find_fetch(fetch_request)
            .is_some()
    }

    pub(crate) fn pending_data_address_execution(&self) -> Option<&RiscvCpuExecutionEvent> {
        self.pending_data_addresses
            .first()
            .and_then(|pending| pending.materialized.as_ref())
    }

    pub(crate) fn pending_data_address_execution_mut(
        &mut self,
    ) -> Option<&mut RiscvCpuExecutionEvent> {
        self.pending_data_addresses
            .first_mut()
            .and_then(|pending| pending.materialized.as_mut())
    }

    pub(crate) fn pending_data_address_decoded(
        &self,
        fetch_request: MemoryRequestId,
    ) -> Option<RiscvDecodedInstruction> {
        let pending = self.pending_data_addresses.first()?;
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
        let Some(pending) = self.pending_data_addresses.first() else {
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
        let Ok(range) = rem6_memory::AddressRange::new(physical_address, size) else {
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

    fn discard_pending_data_address_at_internal(&mut self, now: Option<u64>) {
        if self.pending_data_addresses.is_empty() {
            return;
        }
        let pending = self.pending_data_addresses.take_first();
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
            .pending_data_addresses
            .first()
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
        if !self.pending_data_address_owns_fetch(execution.fetch().request_id()) {
            return None;
        }
        assert!(self.pending_data_address_issue_matches(
            execution.fetch().request_id(),
            access,
            physical_address,
            size,
            request_tick,
        ));
        let pending = self.pending_data_addresses.first()?;
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

        let removed = self.pending_data_addresses.take_first();
        debug_assert_eq!(removed.sequence, sequence);
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

    pub(super) fn pending_data_address_owner_is_consistent(&self) -> bool {
        self.pending_data_addresses
            .first()
            .map_or(true, |pending| pending.is_consistent_with(self))
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_sequence_for_test(&self) -> Option<u64> {
        self.pending_data_addresses
            .first()
            .map(O3PendingDataAddress::sequence)
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_owner_count_for_test(&self) -> usize {
        self.pending_data_address_count()
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_selected_issue_tick_for_test(&self) -> Option<u64> {
        self.pending_data_addresses
            .first()
            .and_then(|pending| pending.selected_issue_tick)
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_materialized_execution_for_test(
        &self,
    ) -> Option<&RiscvCpuExecutionEvent> {
        self.pending_data_addresses
            .first()
            .and_then(|pending| pending.materialized.as_ref())
    }
}
