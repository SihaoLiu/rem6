use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{
    O3PendingStateCheckpointPayload, O3RuntimeCheckpointPayload, O3RuntimeSnapshot, RiscvCore,
};

use super::RiscvCoreCheckpointError;

pub(super) const O3_PENDING_STATE_CHUNK: &str = "o3-pending-state";
pub(super) const O3_RUNTIME_STATE_CHUNK: &str = "o3-runtime-state";

pub(super) fn decode_o3_runtime_authority(
    component: &CheckpointComponentId,
    runtime: Option<&[u8]>,
    pending: Option<&[u8]>,
) -> Result<O3RuntimeCheckpointPayload, RiscvCoreCheckpointError> {
    let runtime = runtime
        .map(|payload| decode_runtime(component, payload))
        .transpose()?;
    match (runtime, pending) {
        (Some(runtime), Some(payload)) => {
            let pending = decode_pending(component, payload)?;
            if pending != pending_from_runtime(&runtime) {
                return Err(RiscvCoreCheckpointError::MismatchedO3PendingStateSnapshot {
                    component: component.clone(),
                });
            }
            Ok(runtime)
        }
        (Some(runtime), None) => Ok(runtime),
        (None, Some(payload)) => {
            let pending = decode_pending(component, payload)?;
            runtime_from_legacy_pending(component, pending)
        }
        (None, None) => Ok(RiscvCore::default_o3_runtime_checkpoint_payload()),
    }
}

fn decode_pending(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<O3PendingStateCheckpointPayload, RiscvCoreCheckpointError> {
    O3PendingStateCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidO3PendingStateSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_runtime(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<O3RuntimeCheckpointPayload, RiscvCoreCheckpointError> {
    O3RuntimeCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidO3RuntimeSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn pending_from_runtime(runtime: &O3RuntimeCheckpointPayload) -> O3PendingStateCheckpointPayload {
    O3PendingStateCheckpointPayload::from_snapshot(runtime.snapshot().pending_state().clone())
        .expect("validated O3 runtime payload has valid pending state")
}

fn runtime_from_legacy_pending(
    component: &CheckpointComponentId,
    pending: O3PendingStateCheckpointPayload,
) -> Result<O3RuntimeCheckpointPayload, RiscvCoreCheckpointError> {
    let default = RiscvCore::default_o3_runtime_checkpoint_payload().into_snapshot();
    let snapshot = O3RuntimeSnapshot::new(
        default.reorder_buffer().iter().copied(),
        default.load_store_queue().iter().copied(),
        default.rename_map().iter().copied(),
        pending.into_snapshot(),
    )
    .map_err(|error| RiscvCoreCheckpointError::InvalidO3RuntimeSnapshot {
        component: component.clone(),
        error,
    })?;
    O3RuntimeCheckpointPayload::from_snapshot(snapshot).map_err(|error| {
        RiscvCoreCheckpointError::InvalidO3RuntimeSnapshot {
            component: component.clone(),
            error,
        }
    })
}
