use rem6_isa_riscv::{MemoryAccessKind, RiscvInstruction};
use rem6_memory::{Address, MemoryRequestId};

use crate::{
    o3_runtime::{
        o3_memory_result_window_destination, o3_memory_result_younger_buffered_effect_destination,
        o3_memory_result_younger_read_destination,
    },
    riscv_data_issue::{access_address, access_size, masked_vector_memory_request_span},
    riscv_fetch_ahead::{O3MemoryResultWindowRole, O3MemoryResultWindowRoute},
    RiscvCoreState, RiscvCpuExecutionEvent,
};

impl RiscvCoreState {
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
        if self.data_translation.is_some() {
            return false;
        }
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
        authorization.matches_resolved_range(
            O3MemoryResultWindowRoute::Memory,
            span.address,
            span.size,
        ) && matches!(
            self.pma
                .is_uncacheable(span.address.get(), span.size.bytes()),
            Ok(false)
        ) && self
            .o3_runtime
            .can_stage_memory_result_window_access(access)
    }
}
