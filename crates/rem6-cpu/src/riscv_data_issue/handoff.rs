use rem6_isa_riscv::MemoryAccessKind;

use super::IssuedDataAccess;
use crate::riscv_execution_mode_handoff::{
    RiscvIssuedScalarLoadHandoff, RiscvO3LiveDataHandoffTarget,
};
use crate::RiscvDataAccessTarget;

impl IssuedDataAccess {
    pub(crate) fn scalar_load_handoff(&self) -> Option<RiscvIssuedScalarLoadHandoff> {
        if !matches!(self.access, MemoryAccessKind::Load { .. }) {
            return None;
        }
        let target = match &self.target {
            RiscvDataAccessTarget::Memory { route, .. } => {
                RiscvO3LiveDataHandoffTarget::Memory { route: *route }
            }
            RiscvDataAccessTarget::Mmio { route } => {
                RiscvO3LiveDataHandoffTarget::Mmio { route: *route }
            }
        };
        Some(RiscvIssuedScalarLoadHandoff {
            fetch_request: self.fetch_request,
            data_request: self.request,
            issue_tick: self.tick,
            partition: self.partition,
            target,
            address: self.physical_address,
            bytes: u32::try_from(self.size.bytes()).ok()?,
        })
    }
}
