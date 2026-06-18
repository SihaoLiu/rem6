use crate::{instruction::RiscvInstruction, RiscvPrivilegeMode};

impl RiscvInstruction {
    pub(crate) const fn required_csr_privilege(self) -> Option<RiscvPrivilegeMode> {
        match self {
            Self::ReadMachineIdentityCsr { csr, .. } | Self::WriteMachineIdentityCsr { csr } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadCounterCsr { .. } => Some(RiscvPrivilegeMode::User),
            Self::ReadMachineCounterCsr { .. }
            | Self::WriteCounterCsr { .. }
            | Self::SetCounterCsr { .. }
            | Self::ClearCounterCsr { .. }
            | Self::WriteCounterCsrImmediate { .. }
            | Self::SetCounterCsrImmediate { .. }
            | Self::ClearCounterCsrImmediate { .. } => Some(RiscvPrivilegeMode::Machine),
            Self::ReadFloatCsr { csr, .. }
            | Self::WriteFloatCsr { csr, .. }
            | Self::SetFloatCsr { csr, .. }
            | Self::ClearFloatCsr { csr, .. }
            | Self::WriteFloatCsrImmediate { csr, .. }
            | Self::SetFloatCsrImmediate { csr, .. }
            | Self::ClearFloatCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::VectorFixedPointCsr(instruction) => {
                Some(required_csr_privilege(instruction.csr().address()))
            }
            Self::ReadStatusCsr { csr, .. }
            | Self::WriteStatusCsr { csr, .. }
            | Self::SetStatusCsr { csr, .. }
            | Self::ClearStatusCsr { csr, .. }
            | Self::WriteStatusCsrImmediate { csr, .. }
            | Self::SetStatusCsrImmediate { csr, .. }
            | Self::ClearStatusCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadInterruptCsr { csr, .. }
            | Self::WriteInterruptCsr { csr, .. }
            | Self::SetInterruptCsr { csr, .. }
            | Self::ClearInterruptCsr { csr, .. }
            | Self::WriteInterruptCsrImmediate { csr, .. }
            | Self::SetInterruptCsrImmediate { csr, .. }
            | Self::ClearInterruptCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadMachineTrapCsr { csr, .. }
            | Self::WriteMachineTrapCsr { csr, .. }
            | Self::SetMachineTrapCsr { csr, .. }
            | Self::ClearMachineTrapCsr { csr, .. }
            | Self::WriteMachineTrapCsrImmediate { csr, .. }
            | Self::SetMachineTrapCsrImmediate { csr, .. }
            | Self::ClearMachineTrapCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadSupervisorTrapCsr { csr, .. }
            | Self::WriteSupervisorTrapCsr { csr, .. }
            | Self::SetSupervisorTrapCsr { csr, .. }
            | Self::ClearSupervisorTrapCsr { csr, .. }
            | Self::WriteSupervisorTrapCsrImmediate { csr, .. }
            | Self::SetSupervisorTrapCsrImmediate { csr, .. }
            | Self::ClearSupervisorTrapCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            Self::ReadTranslationCsr { csr, .. }
            | Self::WriteTranslationCsr { csr, .. }
            | Self::SetTranslationCsr { csr, .. }
            | Self::ClearTranslationCsr { csr, .. }
            | Self::WriteTranslationCsrImmediate { csr, .. }
            | Self::SetTranslationCsrImmediate { csr, .. }
            | Self::ClearTranslationCsrImmediate { csr, .. } => {
                Some(required_csr_privilege(csr.address()))
            }
            _ => None,
        }
    }
}

const fn required_csr_privilege(address: u16) -> RiscvPrivilegeMode {
    match (address >> 8) & 0b11 {
        0 => RiscvPrivilegeMode::User,
        1 => RiscvPrivilegeMode::Supervisor,
        _ => RiscvPrivilegeMode::Machine,
    }
}

pub(crate) fn csr_privilege_allowed(
    current: RiscvPrivilegeMode,
    required: RiscvPrivilegeMode,
) -> bool {
    privilege_rank(current) >= privilege_rank(required)
}

const fn privilege_rank(privilege: RiscvPrivilegeMode) -> u8 {
    match privilege {
        RiscvPrivilegeMode::User => 0,
        RiscvPrivilegeMode::Supervisor => 1,
        RiscvPrivilegeMode::Machine => 3,
    }
}
