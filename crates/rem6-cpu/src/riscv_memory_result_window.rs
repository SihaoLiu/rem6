use std::collections::BTreeSet;

use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, RiscvInstruction};
use rem6_memory::{Address, MemoryRequestId};

use crate::{
    o3_runtime::{
        o3_memory_result_window_destination, o3_memory_result_younger_buffered_effect_destination,
        o3_memory_result_younger_read_destination,
    },
    riscv_data_issue::{access_address, access_size, masked_vector_memory_request_span},
    riscv_fetch_ahead::{O3MemoryResultWindowRole, O3MemoryResultWindowRoute},
    riscv_live_retire_window::{
        completed_fetch_instruction_from_events, completed_fetch_instruction_starting_with,
    },
    CpuFetchEvent, CpuFetchEventKind, RiscvCoreState, RiscvCpuExecutionEvent,
};

impl RiscvCoreState {
    pub(crate) fn has_exact_translated_result_pair_window(
        &self,
        fetch_events: &[CpuFetchEvent],
        head_fetch_request: MemoryRequestId,
        head_o3_sequence: u64,
    ) -> bool {
        if self.data_translation.is_none()
            || !self.buffered_o3_effects.is_empty()
            || self.memory_result_window_authorizations.len() != 1
            || (!self.pending_data_translations.is_empty()
                && !self.ready_translated_data.is_empty())
        {
            return false;
        }
        let Some((&younger_fetch_request, authorization)) =
            self.memory_result_window_authorizations.iter().next()
        else {
            return false;
        };
        let authorization = *authorization;
        let executed_fetches = BTreeSet::new();
        let Some(head_event) = fetch_events.iter().find(|event| {
            event.request_id() == head_fetch_request && event.kind() == CpuFetchEventKind::Completed
        }) else {
            return false;
        };
        let Some(head) =
            completed_fetch_instruction_starting_with(&executed_fetches, fetch_events, head_event)
        else {
            return false;
        };
        let Some(sequential_pc) = head
            .pc()
            .get()
            .checked_add(u64::from(head.decoded().bytes()))
            .map(Address::new)
        else {
            return false;
        };
        let Some(younger) = completed_fetch_instruction_from_events(
            &executed_fetches,
            fetch_events,
            head.last_consumed_request(),
            sequential_pc,
        ) else {
            return false;
        };
        if authorization.role() != O3MemoryResultWindowRole::YoungerRead
            || !authorization.is_translated()
            || authorization.integer_destination().is_none()
            || head.first_consumed_request() != head_fetch_request
            || younger.first_consumed_request() != younger_fetch_request
            || !matches!(
                younger.decoded().instruction(),
                RiscvInstruction::Load {
                    rd,
                    width: MemoryWidth::Doubleword,
                    ..
                } if Some(rd) == authorization.integer_destination()
            )
        {
            return false;
        }
        let exact_younger = |fetch_request, access: &MemoryAccessKind, address, size| {
            fetch_request == younger_fetch_request
                && matches!(
                    access,
                    MemoryAccessKind::Load {
                        rd,
                        width: MemoryWidth::Doubleword,
                        ..
                    } if Some(*rd) == authorization.integer_destination()
                )
                && authorization.matches_virtual_range(address, size)
        };
        if self.pending_data_translations.len() > 1
            || self.pending_data_translations.iter().next().is_some_and(
                |(translation_id, pending)| {
                    translation_id.agent() != pending.request_id.agent()
                        || translation_id.sequence() != pending.request_id.sequence()
                        || !exact_younger(
                            pending.fetch_request,
                            &pending.access,
                            pending.virtual_address,
                            pending.size,
                        )
                },
            )
            || self.ready_translated_data.len() > 1
            || self
                .ready_translated_data
                .iter()
                .next()
                .is_some_and(|(fetch_request, ready)| {
                    *fetch_request != ready.fetch_request
                        || !exact_younger(
                            ready.fetch_request,
                            &ready.access,
                            ready.virtual_address,
                            ready.size,
                        )
                })
        {
            return false;
        }
        let snapshot = self.o3_runtime.snapshot();
        snapshot
            .reorder_buffer()
            .iter()
            .filter(|entry| entry.sequence() == head_o3_sequence)
            .count()
            == 1
            && snapshot
                .load_store_queue()
                .iter()
                .filter(|entry| entry.sequence() == head_o3_sequence)
                .count()
                == 1
            && snapshot.reorder_buffer().len() < self.o3_runtime.scalar_live_window_limit()
            && snapshot.load_store_queue().len() < self.o3_runtime.scalar_memory_window_limit()
    }

    pub(crate) fn can_extend_detailed_memory_result_window(&self) -> bool {
        let authorized = self
            .memory_result_window_authorizations
            .values()
            .any(|authorization| authorization.role().is_younger());
        let capacity = self.o3_runtime.can_consider_memory_result_younger();
        self.live_retire_gate.detailed_policy_enabled()
            && !self.outstanding_data.is_empty()
            && self.data_translation.is_none()
            && self.pending_data_translations.is_empty()
            && self.ready_translated_data.is_empty()
            && authorized
            && capacity
    }

    pub(crate) fn can_overlap_detailed_memory_result_event(
        &self,
        event: &RiscvCpuExecutionEvent,
    ) -> bool {
        event.execution().memory_access().is_some_and(|access| {
            self.can_overlap_detailed_memory_result_access(event.fetch().request_id(), access)
        })
    }

    pub(crate) fn can_overlap_detailed_memory_result_instruction(
        &self,
        fetch_request: MemoryRequestId,
        instruction: RiscvInstruction,
    ) -> bool {
        let mut hart = self.hart.clone();
        hart.execute(instruction)
            .ok()
            .and_then(|execution| execution.memory_access().cloned())
            .is_some_and(|access| {
                self.can_overlap_detailed_memory_result_access(fetch_request, &access)
            })
    }

    fn can_overlap_detailed_memory_result_access(
        &self,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
    ) -> bool {
        let Some(authorization) = self
            .memory_result_window_authorizations
            .get(&fetch_request)
            .copied()
            .filter(|authorization| authorization.role().is_younger())
        else {
            return false;
        };
        let expected_role = if o3_memory_result_younger_read_destination(access).is_some() {
            O3MemoryResultWindowRole::YoungerRead
        } else if o3_memory_result_younger_buffered_effect_destination(access).is_some() {
            O3MemoryResultWindowRole::YoungerBufferedEffect
        } else {
            return false;
        };
        if authorization.role() != expected_role {
            return false;
        }
        if authorization.dependent_source().is_some() {
            return false;
        }
        let Some(integer_destination) = o3_memory_result_window_destination(access) else {
            return false;
        };
        if integer_destination != authorization.integer_destination() {
            return false;
        }
        let Ok(base_size) = access_size(access) else {
            return false;
        };
        let base_address = Address::new(access_address(access));
        let Ok(span) = masked_vector_memory_request_span(access, base_address, base_size) else {
            return false;
        };
        let authorized_range = if authorization.is_translated() {
            self.data_translation.is_some()
                && authorization.matches_virtual_range(span.address, span.size)
        } else {
            self.data_translation.is_none()
                && authorization.matches_resolved_range(
                    O3MemoryResultWindowRoute::Memory,
                    span.address,
                    span.size,
                )
                && matches!(
                    self.pma
                        .is_uncacheable(span.address.get(), span.size.bytes()),
                    Ok(false)
                )
        };
        authorized_range
            && self
                .o3_runtime
                .can_stage_memory_result_window_access(access)
    }
}
