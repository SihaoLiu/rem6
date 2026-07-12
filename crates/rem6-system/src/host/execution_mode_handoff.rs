use crate::{ExecutionMode, ExecutionModeTarget};

use super::ExecutionModeSwitchStateTransfer;

pub(super) const fn supports_live_data_handoff(
    previous: Option<ExecutionMode>,
    requested: ExecutionMode,
) -> bool {
    matches!(previous, Some(ExecutionMode::Detailed)) && matches!(requested, ExecutionMode::Timing)
}

impl ExecutionModeSwitchStateTransfer {
    pub(super) fn from_live_data_handoff_manifest(
        manifest: &rem6_checkpoint::CheckpointManifest,
        target: &ExecutionModeTarget,
        checker: Option<crate::riscv_checkpoint::RiscvCoreCheckerSnapshotSummary>,
    ) -> Self {
        let mut transfer = Self::from_manifest(manifest, target, checker);
        transfer.restorable = false;
        transfer.live_data_handoff = true;
        transfer.quiescence_gate.validated = false;
        transfer
    }

    pub const fn restorable(&self) -> bool {
        self.restorable
    }

    pub const fn live_data_handoff(&self) -> bool {
        self.live_data_handoff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_data_handoff_only_supports_detailed_to_timing() {
        assert!(supports_live_data_handoff(
            Some(ExecutionMode::Detailed),
            ExecutionMode::Timing
        ));
        for (previous, requested) in [
            (None, ExecutionMode::Timing),
            (Some(ExecutionMode::Functional), ExecutionMode::Timing),
            (Some(ExecutionMode::Timing), ExecutionMode::Detailed),
            (Some(ExecutionMode::Detailed), ExecutionMode::Detailed),
            (Some(ExecutionMode::Detailed), ExecutionMode::Functional),
        ] {
            assert!(!supports_live_data_handoff(previous, requested));
        }
    }
}
