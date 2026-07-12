use rem6_isa_riscv::MemoryAccessKind;

use super::IssuedDataAccess;
use crate::riscv_execution_mode_handoff::{
    RiscvIssuedScalarMemoryHandoff, RiscvO3LiveDataHandoffOperation, RiscvO3LiveDataHandoffTarget,
};
use crate::RiscvDataAccessTarget;

impl IssuedDataAccess {
    pub(crate) fn scalar_memory_handoff(&self) -> Option<RiscvIssuedScalarMemoryHandoff> {
        if self.store_load_forwarding_plan.is_some() {
            return None;
        }
        let (operation, store_data) = match self.access {
            MemoryAccessKind::Load { .. } => (RiscvO3LiveDataHandoffOperation::Load, None),
            MemoryAccessKind::Store { value, .. } => (
                RiscvO3LiveDataHandoffOperation::Store,
                Some(value.to_le_bytes()),
            ),
            _ => return None,
        };
        let target = match &self.target {
            RiscvDataAccessTarget::Memory { route, .. } => {
                RiscvO3LiveDataHandoffTarget::Memory { route: *route }
            }
            RiscvDataAccessTarget::Mmio { route } => {
                RiscvO3LiveDataHandoffTarget::Mmio { route: *route }
            }
        };
        Some(RiscvIssuedScalarMemoryHandoff {
            fetch_request: self.fetch_request,
            data_request: self.request,
            issue_tick: self.tick,
            partition: self.partition,
            operation,
            target,
            address: self.physical_address,
            bytes: u32::try_from(self.size.bytes()).ok()?,
            store_data,
        })
    }
}
