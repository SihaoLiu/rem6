use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction, RiscvInstruction,
};
use rem6_memory::{Address, MemoryRequestId};

use super::o3_runtime_pending_address::{
    O3PendingDataAddress, O3PendingDataAddressRootHead, PENDING_DATA_ADDRESS_LSQ_BYTES,
};
use super::o3_runtime_pending_address_set::O3_PENDING_DATA_ADDRESS_CAPACITY;
use super::*;
use crate::CpuFetchEventKind;

impl O3RuntimeState {
    pub(crate) fn stage_pending_data_address_window(
        &mut self,
        head_fetch: MemoryRequestId,
        pending: impl IntoIterator<Item = O3PendingDataAddressRequest>,
        suffix: impl IntoIterator<Item = (Address, RiscvInstruction)>,
        admission_tick: u64,
    ) -> usize {
        if !self.pending_data_addresses.is_empty() {
            return 0;
        }
        let pending = pending.into_iter().collect::<Vec<_>>();
        if pending.is_empty()
            || pending.len() > O3_PENDING_DATA_ADDRESS_CAPACITY
            || (pending.len() >= 2 && self.scalar_memory_window_limit < 4)
        {
            return 0;
        }
        let suffix_limit = self
            .scalar_memory_window_limit
            .saturating_sub(1 + pending.len());
        let suffix = suffix.into_iter().take(suffix_limit).collect::<Vec<_>>();
        let mut staged = self.clone();
        let Some(staged_count) = staged.try_stage_pending_data_address_window(
            head_fetch,
            pending,
            suffix,
            admission_tick,
        ) else {
            return 0;
        };
        *self = staged;
        staged_count
    }

    fn try_stage_pending_data_address_window(
        &mut self,
        head_fetch: MemoryRequestId,
        pending: Vec<O3PendingDataAddressRequest>,
        suffix: Vec<(Address, RiscvInstruction)>,
        admission_tick: u64,
    ) -> Option<usize> {
        let (head_destination, _) = self.pending_data_address_head_metadata(head_fetch)?;
        let pending_count = pending.len();
        let mut result_destinations = Vec::with_capacity(O3_PENDING_DATA_ADDRESS_CAPACITY + 1);
        result_destinations.push(head_destination);
        let mut previous_consumed_request = None;
        for pending in pending {
            if previous_consumed_request.is_some()
                && previous_consumed_request != Some(pending.fetch_predecessor_request)
            {
                return None;
            }
            let next_predecessor = pending.consumed_requests.last().copied()?;
            let pending_rd = self.validate_pending_data_address_request(&pending)?;
            if result_destinations.contains(&pending_rd) {
                return None;
            }
            let valid_source = pending.producer_register == head_destination
                || result_destinations.last().copied() == Some(pending.producer_register);
            if !valid_source {
                return None;
            }
            let (producer_sequence, root_head) =
                self.pending_data_address_producer_metadata(head_fetch, pending.producer_register)?;
            let Some((sequence, Some(physical_destination))) = self
                .stage_live_instruction_with_rename_destination(
                    pending.fetch.pc(),
                    pending.decoded.instruction(),
                    0,
                    Some((O3RegisterClass::Integer, u32::from(pending_rd.index()))),
                )
            else {
                return None;
            };
            if producer_sequence >= sequence {
                return None;
            }
            let destination = O3RenameMapEntry::new(
                O3RegisterClass::Integer,
                u32::from(pending_rd.index()),
                physical_destination,
            );
            if !self.pending_data_addresses.try_push(O3PendingDataAddress {
                sequence,
                fetch: pending.fetch.clone(),
                consumed_requests: pending.consumed_requests.clone(),
                decoded: pending.decoded,
                fetch_predecessor_request: pending.fetch_predecessor_request,
                producer_register: pending.producer_register,
                producer_sequence,
                root_head,
                destination,
                expected_lsq_bytes: PENDING_DATA_ADDRESS_LSQ_BYTES,
                requested_wake_tick: None,
                selected_issue_tick: None,
                materialized: None,
            }) {
                return None;
            }
            if !self.bind_live_staged_issue_packet_at_sequence(
                sequence,
                pending.decoded,
                &pending.consumed_requests,
                admission_tick,
            ) {
                return None;
            }
            self.snapshot
                .load_store_queue
                .push(O3LoadStoreQueueEntry::load(
                    sequence,
                    None,
                    PENDING_DATA_ADDRESS_LSQ_BYTES,
                ));
            self.live_data_access_younger_sequences.insert(sequence);
            result_destinations.push(pending_rd);
            previous_consumed_request = Some(next_predecessor);
        }
        let Some(mut window) =
            crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_memory_results(
                result_destinations,
                1 + pending_count,
                self.scalar_memory_window_limit,
            )
        else {
            return None;
        };
        let mut staged = pending_count;
        for (pc, instruction) in suffix {
            let decision = window.classify_younger(instruction);
            if decision == crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::Reject
            {
                return None;
            }
            let sequence = self.stage_live_instruction(pc, instruction, 0)?;
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
        Some(staged)
    }

    fn pending_data_address_head_metadata(
        &self,
        head_fetch: MemoryRequestId,
    ) -> Option<(Register, O3PendingDataAddressRootHead)> {
        let metadata = self.pending_data_address_live_head_metadata(head_fetch)?;
        if !self.live_data_access_younger_sequences.is_empty()
            || self.live_data_accesses.len() != 1
            || self
                .snapshot
                .reorder_buffer
                .iter()
                .any(|entry| entry.is_live_staged() && entry.sequence() > metadata.1.sequence)
        {
            return None;
        }
        Some(metadata)
    }

    fn pending_data_address_producer_metadata(
        &self,
        head_fetch: MemoryRequestId,
        producer_register: Register,
    ) -> Option<(u64, O3PendingDataAddressRootHead)> {
        if let Some((head_destination, root_head)) =
            self.pending_data_address_live_head_metadata(head_fetch)
        {
            if producer_register == head_destination {
                return Some((root_head.sequence, root_head));
            }
        }
        let pending = self.pending_data_addresses.last()?;
        (pending.destination.register_class() == O3RegisterClass::Integer
            && pending.destination.architectural() == u32::from(producer_register.index()))
        .then_some((pending.sequence, pending.root_head))
    }

    fn pending_data_address_live_head_metadata(
        &self,
        head_fetch: MemoryRequestId,
    ) -> Option<(Register, O3PendingDataAddressRootHead)> {
        let live = self.live_data_accesses.iter().find(|live| {
            live.fetch_request == head_fetch
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
        Some((
            destination,
            O3PendingDataAddressRootHead {
                sequence: live.sequence,
                fetch_request: live.fetch_request,
                range: o3_memory_result_range(access)?,
                atomic_head,
            },
        ))
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
        fetch: &crate::CpuFetchEvent,
        decoded: RiscvDecodedInstruction,
    ) -> bool {
        let Some([a, b, c, d]) = fetch.data() else {
            return false;
        };
        let raw = u32::from_le_bytes([*a, *b, *c, *d]);
        RiscvInstruction::decode_with_length(raw).ok() == Some(decoded)
    }
}
