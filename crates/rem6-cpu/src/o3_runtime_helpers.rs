use std::collections::BTreeSet;

use crate::o3_pipeline::{
    O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PipelineStage,
    O3WritebackTransferPolicy, O3WritebackTransferSnapshot,
};
use crate::O3RegisterClass;

use super::{O3RuntimeError, O3RuntimeSnapshot, O3_RUNTIME_U32_MAX};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum O3RuntimeUniqueKey {
    Sequence(u64),
    Rename(O3RegisterClass, u32),
}

pub(super) fn default_o3_runtime_snapshot() -> O3RuntimeSnapshot {
    O3RuntimeSnapshot::new(
        [],
        [],
        [],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0)
                    .expect("default O3 writeback policy is valid"),
                [],
            ),
        )
        .expect("default O3 pending-state snapshot is valid"),
    )
    .expect("default O3 runtime snapshot is valid")
}

pub(super) fn validate_runtime_snapshot(
    snapshot: &O3RuntimeSnapshot,
) -> Result<(), O3RuntimeError> {
    encode_u32("reorder_buffer_count", snapshot.reorder_buffer.len())?;
    encode_u32("load_store_queue_count", snapshot.load_store_queue.len())?;
    encode_u32("rename_map_count", snapshot.rename_map.len())?;
    let pending_payload =
        O3PendingStateCheckpointPayload::from_snapshot(snapshot.pending_state.clone())
            .map_err(|error| O3RuntimeError::InvalidPendingState { error })?
            .encode();
    encode_u32("pending_payload_length", pending_payload.len())?;
    Ok(())
}

pub(super) fn validate_unique<I>(kind: &'static str, keys: I) -> Result<(), O3RuntimeError>
where
    I: IntoIterator<Item = O3RuntimeUniqueKey>,
{
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            return match (kind, key) {
                ("ROB", O3RuntimeUniqueKey::Sequence(sequence)) => {
                    Err(O3RuntimeError::DuplicateReorderBufferSequence { sequence })
                }
                ("LSQ", O3RuntimeUniqueKey::Sequence(sequence)) => {
                    Err(O3RuntimeError::DuplicateLoadStoreQueueSequence { sequence })
                }
                ("rename_map", O3RuntimeUniqueKey::Rename(register_class, architectural)) => {
                    Err(O3RuntimeError::DuplicateRenameMapEntry {
                        register_class,
                        architectural,
                    })
                }
                _ => unreachable!("O3 runtime unique key kind is known"),
            };
        }
    }
    Ok(())
}

pub(super) fn encode_u32(field: &'static str, value: usize) -> Result<u32, O3RuntimeError> {
    u32::try_from(value).map_err(|_| O3RuntimeError::CheckpointValueTooLarge {
        field,
        value,
        maximum: O3_RUNTIME_U32_MAX,
    })
}

pub(super) const fn encode_register_class(register_class: O3RegisterClass) -> u8 {
    match register_class {
        O3RegisterClass::Integer => 0,
        O3RegisterClass::FloatingPoint => 1,
        O3RegisterClass::Vector => 2,
        O3RegisterClass::ConditionCode => 3,
        O3RegisterClass::Misc => 4,
    }
}
