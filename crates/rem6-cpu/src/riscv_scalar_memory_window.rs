use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_memory::{
    AccessSize, Address, AddressRange, MemoryRequestId, TranslationAccessKind,
    TranslationAddressSpaceId, TranslationRequest, TranslationRequestId,
};

use crate::RiscvCoreState;

impl RiscvCoreState {
    pub(super) fn can_extend_detailed_scalar_memory_window(&self) -> bool {
        !self.outstanding_data.is_empty()
            && self.data_translation.is_none()
            && self.pending_data_translations.is_empty()
            && self.ready_translated_data.is_empty()
            && self.outstanding_data.values().all(|outstanding| {
                outstanding.memory_range().is_some_and(|range| {
                    matches!(
                        self.pma
                            .is_uncacheable(range.start().get(), range.size().bytes()),
                        Ok(false)
                    )
                })
            })
            && self.o3_runtime.can_consider_scalar_memory_younger()
    }

    pub(super) fn can_overlap_detailed_scalar_memory_instruction(
        &self,
        instruction: RiscvInstruction,
    ) -> bool {
        if !self.can_extend_detailed_scalar_memory_window() {
            return false;
        }
        let Some(range) = self.cacheable_scalar_memory_instruction_range(instruction) else {
            return false;
        };
        self.o3_runtime
            .can_defer_scalar_memory_instruction(instruction, range)
    }

    pub(super) fn cacheable_scalar_memory_instruction_range(
        &self,
        instruction: RiscvInstruction,
    ) -> Option<AddressRange> {
        let range = self.scalar_memory_instruction_range(instruction)?;
        match self
            .pma
            .is_uncacheable(range.start().get(), range.size().bytes())
        {
            Ok(false) => Some(range),
            Ok(true) | Err(_) => None,
        }
    }

    pub(super) fn cacheable_cached_translated_scalar_load(
        &self,
        instruction: RiscvInstruction,
        fetch_request: MemoryRequestId,
    ) -> bool {
        if !matches!(instruction, RiscvInstruction::Load { .. }) {
            return false;
        }
        let Some(virtual_range) = self.scalar_memory_instruction_range(instruction) else {
            return false;
        };
        let Ok(request) = TranslationRequest::new(
            TranslationRequestId::new(fetch_request.agent(), fetch_request.sequence()),
            virtual_range.start(),
            virtual_range.size(),
            TranslationAccessKind::Load,
        ) else {
            return false;
        };
        let address_space = TranslationAddressSpaceId::new(self.hart.translation_address_space());
        let Some(tlb) = self
            .data_translation
            .as_ref()
            .and_then(|frontend| frontend.tlb())
        else {
            return false;
        };
        let Some(physical_address) = tlb
            .peek_cached_in_address_space(address_space, &request)
            .ok()
            .flatten()
            .and_then(|lookup| lookup.physical_address())
        else {
            return false;
        };
        matches!(
            self.pma
                .is_uncacheable(physical_address.get(), virtual_range.size().bytes()),
            Ok(false)
        )
    }

    fn scalar_memory_instruction_range(
        &self,
        instruction: RiscvInstruction,
    ) -> Option<AddressRange> {
        let (rs1, offset, width) = match instruction {
            RiscvInstruction::Load {
                rs1, offset, width, ..
            }
            | RiscvInstruction::Store {
                rs1, offset, width, ..
            } => (rs1, offset, width),
            _ => return None,
        };
        let base = self.hart.read(rs1);
        let address = if offset.value() >= 0 {
            base.checked_add(offset.value() as u64)?
        } else {
            base.checked_sub(offset.value().unsigned_abs())?
        };
        AddressRange::new(
            Address::new(address),
            AccessSize::new(width.bytes() as u64).ok()?,
        )
        .ok()
    }
}

pub(crate) fn independent_scalar_load_destination<I>(
    instruction: RiscvInstruction,
    older_destinations: I,
) -> Option<Register>
where
    I: IntoIterator<Item = Register>,
{
    let RiscvInstruction::Load { rd, rs1, .. } = instruction else {
        return None;
    };
    if rd.is_zero()
        || older_destinations
            .into_iter()
            .any(|older| rd == older || rs1 == older)
    {
        return None;
    }
    Some(rd)
}
