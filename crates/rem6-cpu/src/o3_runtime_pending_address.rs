use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction, RiscvExecutionRecord,
    RiscvInstruction,
};
use rem6_memory::{AccessSize, Address, AddressRange, MemoryRequestId};

use super::*;
use crate::CpuFetchEvent;

pub(super) const PENDING_DATA_ADDRESS_LSQ_BYTES: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3PendingDataAddressRootHead {
    pub(super) sequence: u64,
    pub(super) fetch_request: MemoryRequestId,
    pub(super) range: AddressRange,
    pub(super) atomic_head: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3PendingDataAddress {
    pub(super) sequence: u64,
    pub(super) fetch: CpuFetchEvent,
    pub(super) consumed_requests: Vec<MemoryRequestId>,
    pub(super) decoded: RiscvDecodedInstruction,
    pub(super) fetch_predecessor_request: MemoryRequestId,
    pub(super) producer_register: Register,
    pub(super) producer_sequence: u64,
    pub(super) root_head: O3PendingDataAddressRootHead,
    pub(super) destination: O3RenameMapEntry,
    pub(super) expected_lsq_bytes: u32,
    pub(super) requested_wake_tick: Option<u64>,
    pub(super) selected_issue_tick: Option<u64>,
    pub(super) materialized: Option<RiscvCpuExecutionEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3PendingDataAddressRequest {
    pub(crate) fetch_predecessor_request: MemoryRequestId,
    pub(crate) fetch: CpuFetchEvent,
    pub(crate) consumed_requests: Vec<MemoryRequestId>,
    pub(crate) decoded: RiscvDecodedInstruction,
    pub(crate) producer_register: Register,
}

impl O3PendingDataAddress {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) fn is_consistent_with(&self, runtime: &O3RuntimeState) -> bool {
        self.root_head.sequence <= self.producer_sequence
            && self.producer_sequence < self.sequence
            && runtime.pending_data_address_producer_is_consistent(self)
            && self.expected_lsq_bytes == PENDING_DATA_ADDRESS_LSQ_BYTES
            && self.materialized.is_some() == self.selected_issue_tick.is_some()
            && !(self.materialized.is_some() && self.requested_wake_tick.is_some())
            && runtime.snapshot.reorder_buffer.iter().any(|entry| {
                entry.sequence() == self.sequence
                    && entry.destination() == Some(self.destination.physical())
                    && entry.rename_destination()
                        == Some((
                            self.destination.register_class(),
                            self.destination.architectural(),
                        ))
            })
            && runtime
                .snapshot_with_live_rename_map()
                .rename_map()
                .contains(&self.destination)
            && runtime.snapshot.load_store_queue.iter().any(|entry| {
                entry.sequence() == self.sequence
                    && entry.kind() == O3LoadStoreQueueKind::Load
                    && entry.address().is_none()
                    && entry.bytes() == self.expected_lsq_bytes
            })
    }

    pub(super) fn issue_matches(
        &self,
        runtime: &O3RuntimeState,
        execution: &RiscvCpuExecutionEvent,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
        physical_address: Address,
        size: AccessSize,
        request_tick: u64,
    ) -> bool {
        let Some(issue_tick) = self.selected_issue_tick else {
            return false;
        };
        let RiscvInstruction::Load {
            rd,
            rs1,
            width: MemoryWidth::Doubleword,
            ..
        } = self.decoded.instruction()
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
        self.fetch.request_id() == fetch_request
            && self.fetch.pc() == execution.fetch().pc()
            && self.decoded.instruction() == execution.instruction()
            && self.producer_register == rs1
            && self.destination.register_class() == O3RegisterClass::Integer
            && self.destination.architectural() == u32::from(rd.index())
            && *access_rd == rd
            && execution.execution().memory_access() == Some(access)
            && physical_address.get() == *address
            && size.bytes() == u64::from(self.expected_lsq_bytes)
            && request_tick >= issue_tick
            && (!self.root_head.atomic_head || !self.root_head.range.overlaps(range))
            && runtime.live_staged_fetch_identity_matches(
                self.sequence,
                self.decoded.instruction(),
                &self.consumed_requests,
            )
    }
}

impl O3PendingDataAddressRequest {
    pub(crate) fn new(
        fetch_predecessor_request: MemoryRequestId,
        fetch: CpuFetchEvent,
        consumed_requests: Vec<MemoryRequestId>,
        decoded: RiscvDecodedInstruction,
        producer_register: Register,
    ) -> Self {
        Self {
            fetch_predecessor_request,
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
    #[cfg(test)]
    pub(crate) fn set_pending_data_address_materialized_for_test(
        &mut self,
        issue_tick: u64,
        execution: RiscvCpuExecutionEvent,
    ) {
        let fetch_request = self
            .pending_data_addresses
            .first()
            .expect("pending address owner")
            .fetch
            .request_id();
        self.set_pending_data_address_materialized_for_fetch_for_test(
            fetch_request,
            issue_tick,
            execution,
        );
    }

    #[cfg(test)]
    pub(crate) fn set_pending_data_address_materialized_for_fetch_for_test(
        &mut self,
        fetch_request: MemoryRequestId,
        issue_tick: u64,
        execution: RiscvCpuExecutionEvent,
    ) {
        let pending = self
            .pending_data_addresses
            .find_primary_fetch_mut(fetch_request)
            .expect("pending address fetch owner");
        pending.selected_issue_tick = Some(issue_tick);
        pending.materialized = Some(execution);
    }

    pub(super) fn record_pending_data_address_materialization(
        &mut self,
        candidate: O3LiveSpeculativeIssueCandidate,
        consumed_requests: &[MemoryRequestId],
        issue_tick: u64,
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        let sequence = candidate.sequence();
        let Some(pending) = self.pending_data_addresses.find_sequence(sequence).cloned() else {
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
        if sequence != pending.sequence
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
        let pc = pending.fetch.pc();
        let event =
            RiscvCpuExecutionEvent::new(pending.fetch, pending.decoded.instruction(), execution);
        {
            let Some(stored) = self.pending_data_addresses.find_sequence_mut(sequence) else {
                return Ok(false);
            };
            if let Some(materialized) = &stored.materialized {
                return Ok(stored.selected_issue_tick == Some(issue_tick) && materialized == &event);
            }
            stored.requested_wake_tick = None;
            stored.selected_issue_tick = Some(issue_tick);
            stored.materialized = Some(event);
        }
        self.live_issue.remove_exact_at(
            sequence,
            O3LiveIssueTraceAction::Selected,
            pc,
            O3LiveIssueTraceClass::MemoryAgu,
            issue_tick,
        );
        Ok(true)
    }

    fn pending_data_address_producer_is_consistent(&self, pending: &O3PendingDataAddress) -> bool {
        if self
            .pending_data_addresses
            .find_sequence(pending.producer_sequence)
            .is_some_and(|producer| {
                producer.sequence < pending.sequence
                    && producer.destination.register_class() == O3RegisterClass::Integer
                    && producer.destination.architectural()
                        == u32::from(pending.producer_register.index())
                    && producer.root_head == pending.root_head
            })
        {
            return true;
        }
        self.live_data_accesses.iter().any(|live| {
            if live.sequence != pending.producer_sequence
                || matches!(
                    live.outcome,
                    O3LiveDataAccessOutcome::Retried | O3LiveDataAccessOutcome::Failed
                )
                || live.younger_window_policy != O3DataAccessWindowPolicy::MemoryResultWindow
            {
                return false;
            }
            let Some(access) = live.execution.execution().memory_access() else {
                return false;
            };
            let producer_matches = o3_memory_result_destination(access).is_some_and(
                |(register_class, architectural)| {
                    register_class == O3RegisterClass::Integer
                        && architectural == u32::from(pending.producer_register.index())
                },
            );
            if pending.producer_sequence != pending.root_head.sequence {
                return producer_matches;
            }
            let atomic_head = matches!(
                access,
                MemoryAccessKind::AtomicMemory {
                    acquire: false,
                    release: false,
                    ..
                }
            );
            producer_matches
                && live.fetch_request == pending.root_head.fetch_request
                && o3_memory_result_range(access) == Some(pending.root_head.range)
                && atomic_head == pending.root_head.atomic_head
        })
    }

    #[cfg(test)]
    pub(crate) fn corrupt_pending_data_address_lsq_bytes_for_test(&mut self, bytes: u32) {
        if let Some(pending) = self.pending_data_addresses.first_mut() {
            pending.expected_lsq_bytes = bytes;
        }
    }

    #[cfg(test)]
    pub(crate) fn corrupt_pending_data_address_producer_sequence_for_test(
        &mut self,
        sequence: u64,
    ) {
        if let Some(pending) = self.pending_data_addresses.first_mut() {
            pending.producer_sequence = sequence;
        }
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

    #[cfg(test)]
    pub(super) fn pending_data_address_rows_for_test(&self) -> Vec<O3PendingDataAddress> {
        self.pending_data_addresses.iter().cloned().collect()
    }

    #[cfg(test)]
    pub(super) fn pending_data_address_sequences_for_test(&self) -> Vec<u64> {
        self.pending_data_addresses
            .iter()
            .map(O3PendingDataAddress::sequence)
            .collect()
    }

    #[cfg(test)]
    pub(super) fn bind_oldest_pending_data_address_for_test(
        &mut self,
        data_request: MemoryRequestId,
        address: Address,
        tick: u64,
    ) {
        let execution = self
            .oldest_pending_data_address_execution()
            .cloned()
            .expect("oldest pending execution");
        assert!(self
            .bind_pending_data_address_issue(&execution, data_request, address, tick)
            .is_some());
    }

    #[cfg(test)]
    pub(super) fn corrupt_pending_data_address_lsq_bytes_for_fetch_for_test(
        &mut self,
        fetch_request: MemoryRequestId,
        bytes: u32,
    ) {
        self.pending_data_addresses
            .find_primary_fetch_mut(fetch_request)
            .expect("pending address fetch owner")
            .expected_lsq_bytes = bytes;
    }

    #[cfg(test)]
    pub(crate) fn append_pending_data_address_consumed_request_for_test(
        &mut self,
        fetch_request: MemoryRequestId,
        consumed_request: MemoryRequestId,
    ) {
        self.pending_data_addresses
            .find_primary_fetch_mut(fetch_request)
            .expect("pending address fetch owner")
            .consumed_requests
            .push(consumed_request);
    }

    #[cfg(test)]
    pub(super) fn complete_pending_data_address_for_test(
        &mut self,
        fetch_request: MemoryRequestId,
        data_request: MemoryRequestId,
        response_tick: u64,
        data: &[u8],
    ) -> (RiscvCpuExecutionEvent, u64) {
        let live = self
            .live_data_accesses
            .iter()
            .find(|live| live.fetch_request == fetch_request)
            .expect("pending address is live");
        let sequence = live.sequence;
        let mut completed = live.execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(self
            .complete_live_data_access_response(
                &completed,
                data_request,
                response_tick,
                9,
                Some(data),
            )
            .unwrap());
        let admitted = self
            .writeback_reservation(sequence)
            .expect("pending writeback reservation")
            .admitted_tick();
        (completed, admitted)
    }
}
