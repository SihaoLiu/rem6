use rem6_isa_riscv::MemoryAccessKind;

use super::{IssuedDataAccess, OutstandingDataAccess};
use crate::riscv_execution_mode_handoff::{
    RiscvIssuedScalarMemoryHandoff, RiscvO3LiveDataHandoffOperation, RiscvO3LiveDataHandoffTarget,
    RiscvPendingPartialScalarLoadHandoff,
};
use crate::RiscvDataAccessTarget;

impl IssuedDataAccess {
    pub(crate) fn scalar_memory_handoff(&self) -> Option<RiscvIssuedScalarMemoryHandoff> {
        let (operation, store_data) = match self.access {
            MemoryAccessKind::Load { .. } => (RiscvO3LiveDataHandoffOperation::Load, None),
            MemoryAccessKind::Store { value, .. } => (
                RiscvO3LiveDataHandoffOperation::Store,
                Some(value.to_le_bytes()),
            ),
            _ => return None,
        };
        let partial_overlay = match self.store_load_forwarding_plan {
            Some(plan)
                if operation == RiscvO3LiveDataHandoffOperation::Load && plan.is_partial() =>
            {
                Some(RiscvPendingPartialScalarLoadHandoff {
                    address: plan.load_range().start(),
                    bytes: plan.bytes(),
                    forwarded_mask: plan.forwarded_mask(),
                    data: plan.forwarded_data(),
                })
            }
            Some(_) => return None,
            None => None,
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
            partial_overlay,
        })
    }
}

impl OutstandingDataAccess {
    pub(crate) fn scalar_memory_handoff(&self) -> Option<RiscvIssuedScalarMemoryHandoff> {
        self.clone_without_layout().scalar_memory_handoff()
    }
}
