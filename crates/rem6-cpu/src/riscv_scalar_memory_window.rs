use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::{AccessSize, Address, AddressRange};

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
            && self.o3_runtime.has_open_scalar_memory_window_slot()
    }

    pub(super) fn can_overlap_detailed_scalar_memory_instruction(
        &self,
        instruction: RiscvInstruction,
    ) -> bool {
        if !self.can_extend_detailed_scalar_memory_window() {
            return false;
        }
        if !matches!(instruction, RiscvInstruction::Load { .. }) {
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
