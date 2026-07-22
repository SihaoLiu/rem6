use rem6_isa_riscv::{MemoryAccessKind, RiscvDecodedInstruction};
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

    #[cfg(test)]
    pub(super) fn first_mut(&mut self) -> Option<&mut O3PendingDataAddress> {
        self.rows.first_mut()
    }

    pub(super) fn last(&self) -> Option<&O3PendingDataAddress> {
        self.rows.last()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &O3PendingDataAddress> {
        self.rows.iter()
    }

    pub(super) fn find_sequence(&self, sequence: u64) -> Option<&O3PendingDataAddress> {
        self.rows.iter().find(|row| row.sequence == sequence)
    }
    pub(super) fn find_sequence_mut(&mut self, sequence: u64) -> Option<&mut O3PendingDataAddress> {
        self.rows.iter_mut().find(|row| row.sequence == sequence)
    }

    pub(super) fn find_fetch(&self, request: MemoryRequestId) -> Option<&O3PendingDataAddress> {
        self.rows.iter().find(|row| {
            row.fetch.request_id() == request || row.consumed_requests.contains(&request)
        })
    }

    pub(super) fn find_primary_fetch(
        &self,
        request: MemoryRequestId,
    ) -> Option<&O3PendingDataAddress> {
        self.rows
            .iter()
            .find(|row| row.fetch.request_id() == request)
    }

    pub(super) fn find_primary_fetch_mut(
        &mut self,
        request: MemoryRequestId,
    ) -> Option<&mut O3PendingDataAddress> {
        self.rows
            .iter_mut()
            .find(|row| row.fetch.request_id() == request)
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
                .iter()
                .any(|existing| existing.sequence == row.sequence)
            || self
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

    fn take_fetch(&mut self, request: MemoryRequestId) -> O3PendingDataAddress {
        let index = self
            .rows
            .iter()
            .position(|row| row.fetch.request_id() == request)
            .expect("pending fetch row");
        self.rows.remove(index)
    }

    fn take_from(&mut self, sequence: u64) -> Vec<O3PendingDataAddress> {
        let first_removed = self.rows.partition_point(|row| row.sequence < sequence);
        self.rows.drain(first_removed..).collect()
    }

    fn is_consistent_with(&self, runtime: &O3RuntimeState) -> bool {
        self.rows.len() <= O3_PENDING_DATA_ADDRESS_CAPACITY
            && self.iter().all(|row| row.is_consistent_with(runtime))
            && self.iter().enumerate().all(|(index, row)| {
                self.rows[..index].iter().all(|older| {
                    older.destination.architectural() != row.destination.architectural()
                })
            })
            && self.rows.windows(2).all(|rows| {
                let (older, younger) = (&rows[0], &rows[1]);
                older.sequence < younger.sequence
                    && older.root_head == younger.root_head
                    && older.consumed_requests.last().copied()
                        == Some(younger.fetch_predecessor_request)
                    && (younger.producer_sequence == younger.root_head.sequence
                        || younger.producer_sequence == older.sequence)
            })
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

    pub(crate) fn oldest_pending_data_address_execution(&self) -> Option<&RiscvCpuExecutionEvent> {
        self.pending_data_addresses
            .iter()
            .find_map(|pending| pending.materialized.as_ref())
    }

    pub(crate) fn pending_data_address_execution_for_fetch(
        &self,
        fetch_request: MemoryRequestId,
    ) -> Option<&RiscvCpuExecutionEvent> {
        self.pending_data_addresses
            .find_primary_fetch(fetch_request)?
            .materialized
            .as_ref()
    }

    pub(crate) fn pending_data_address_execution_for_fetch_mut(
        &mut self,
        fetch_request: MemoryRequestId,
    ) -> Option<&mut RiscvCpuExecutionEvent> {
        self.pending_data_addresses
            .find_primary_fetch_mut(fetch_request)?
            .materialized
            .as_mut()
    }

    pub(crate) fn pending_data_address_decoded(
        &self,
        fetch_request: MemoryRequestId,
    ) -> Option<RiscvDecodedInstruction> {
        self.pending_data_addresses
            .find_primary_fetch(fetch_request)
            .map(|pending| pending.decoded)
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
        let Some(pending) = self
            .pending_data_addresses
            .find_primary_fetch(fetch_request)
        else {
            return false;
        };
        let Some(execution) = pending.materialized.as_ref() else {
            return false;
        };
        pending.issue_matches(
            self,
            execution,
            fetch_request,
            access,
            physical_address,
            size,
            request_tick,
        )
    }

    fn discard_pending_data_address_at_internal(&mut self, sequence: u64, now: Option<u64>) {
        let removed = self.pending_data_addresses.take_from(sequence);
        let Some(first_removed) = removed.first().map(O3PendingDataAddress::sequence) else {
            return;
        };
        self.snapshot
            .load_store_queue
            .retain(|entry| !removed.iter().any(|row| row.sequence == entry.sequence()));
        match now {
            Some(now) => self.discard_live_staged_window_from_at(first_removed, now),
            None => self.discard_live_staged_window_from(first_removed),
        }
    }

    pub(super) fn discard_pending_data_address_from(&mut self, sequence: u64) {
        self.discard_pending_data_address_at_internal(sequence, None);
    }

    pub(crate) fn discard_pending_data_address_for_fetch(
        &mut self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        let Some(sequence) = self
            .pending_data_addresses
            .find_primary_fetch(fetch_request)
            .map(O3PendingDataAddress::sequence)
        else {
            return false;
        };
        self.discard_pending_data_address_from(sequence);
        true
    }

    pub(crate) fn discard_pending_data_address(&mut self) {
        let Some(sequence) = self
            .pending_data_addresses
            .first()
            .map(O3PendingDataAddress::sequence)
        else {
            return;
        };
        self.discard_pending_data_address_at_internal(sequence, None);
    }

    pub(crate) fn discard_pending_data_address_at(&mut self, now: u64) {
        let Some(sequence) = self
            .pending_data_addresses
            .first()
            .map(O3PendingDataAddress::sequence)
        else {
            return;
        };
        self.discard_pending_data_address_at_internal(sequence, Some(now));
        self.discard_live_data_access_lifecycle_at(now);
    }

    pub(crate) fn bind_pending_data_address_issue(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        physical_address: Address,
        request_tick: u64,
    ) -> Option<Vec<MemoryRequestId>> {
        let fetch_request = execution.fetch().request_id();
        self.pending_data_addresses
            .find_primary_fetch(fetch_request)?;
        let access = execution
            .execution()
            .memory_access()
            .expect("pending address execution carries a load");
        let size = AccessSize::new(u64::from(PENDING_DATA_ADDRESS_LSQ_BYTES))
            .expect("pending data address size");
        assert!(self.pending_data_address_issue_matches(
            fetch_request,
            access,
            physical_address,
            size,
            request_tick,
        ));
        let pending = self
            .pending_data_addresses
            .find_primary_fetch(fetch_request)?
            .clone();
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

        let removed = self.pending_data_addresses.take_fetch(fetch_request);
        debug_assert_eq!(removed.sequence, sequence);
        self.live_data_access_younger_sequences.remove(&sequence);
        self.live_data_accesses.push(O3LiveDataAccess {
            fetch_request,
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
        self.pending_data_addresses.is_consistent_with(self)
    }
}
