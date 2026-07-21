use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction, RiscvExecutionRecord,
};
use rem6_memory::{AddressRange, MemoryRequestId};

use super::*;
use crate::CpuFetchEvent;

pub(super) const PENDING_DATA_ADDRESS_LSQ_BYTES: u32 = 8;

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

    pub(super) fn is_consistent_with(&self, runtime: &O3RuntimeState) -> bool {
        let Some((head_range, atomic_head)) = runtime
            .pending_data_address_head_metadata_for_sequence(
                self.producer_sequence,
                self.producer_register,
            )
        else {
            return false;
        };
        head_range == self.head_range
            && atomic_head == self.atomic_head
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
    pub(super) fn record_pending_data_address_materialization(
        &mut self,
        candidate: O3LiveSpeculativeIssueCandidate,
        consumed_requests: &[MemoryRequestId],
        issue_tick: u64,
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        let Some(pending) = self.pending_data_addresses.first().cloned() else {
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
        let Some(stored) = self.pending_data_addresses.first_mut() else {
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
        self.pending_data_addresses.first().is_some_and(|pending| {
            pending.materialized.is_some()
                && pending.fetch.pc() == request.pc()
                && pending.decoded == request.decoded()
                && pending.consumed_requests == request.consumed_requests()
        })
    }

    pub(super) fn pending_data_address_head_metadata_for_sequence(
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
}
