use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction, RiscvInstruction,
};
use rem6_memory::{Address, AddressRange, MemoryRequestId};

use super::o3_runtime_pending_address::{O3PendingDataAddress, PENDING_DATA_ADDRESS_LSQ_BYTES};
use super::*;
use crate::CpuFetchEventKind;

impl O3RuntimeState {
    pub(crate) fn stage_pending_data_address_window(
        &mut self,
        head_fetch: MemoryRequestId,
        pending: O3PendingDataAddressRequest,
        suffix: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) -> usize {
        if !self.pending_data_addresses.is_empty() {
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
        if !self.pending_data_addresses.try_push(O3PendingDataAddress {
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
        }) {
            self.snapshot
                .load_store_queue
                .retain(|entry| entry.sequence() != sequence);
            self.discard_live_staged_window_from(sequence);
            return 0;
        }

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
