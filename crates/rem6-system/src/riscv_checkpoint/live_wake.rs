use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::RiscvO3LiveCheckpointWake;
use rem6_kernel::{PartitionId, ScheduledEventKind};

use super::RiscvCoreCheckpointError;

pub(super) const O3_LIVE_WAKE_AUTHORITY_CHUNK: &str = "o3-live-wake-authority";

const MAGIC: &[u8; 4] = b"O3WA";
const VERSION: u8 = 1;
const ENCODED_BYTES: usize = 34;

pub(super) fn encode(wake: RiscvO3LiveCheckpointWake) -> Vec<u8> {
    let mut payload = Vec::with_capacity(ENCODED_BYTES);
    payload.extend_from_slice(MAGIC);
    payload.push(VERSION);
    payload.extend_from_slice(&wake.scheduler_instance_raw.to_le_bytes());
    payload.extend_from_slice(&wake.partition.index().to_le_bytes());
    payload.extend_from_slice(&wake.tick.to_le_bytes());
    payload.extend_from_slice(&wake.scheduler_order.to_le_bytes());
    payload.push(match wake.kind {
        ScheduledEventKind::Serial => 0,
        ScheduledEventKind::Parallel => 1,
    });
    payload
}

pub(super) fn validate(
    component: &CheckpointComponentId,
    payload: &[u8],
    expected: RiscvO3LiveCheckpointWake,
) -> Result<(), RiscvCoreCheckpointError> {
    let invalid = |reason: &str| RiscvCoreCheckpointError::InvalidO3LiveWakeAuthority {
        component: component.clone(),
        reason: reason.to_string(),
    };
    if payload.len() != ENCODED_BYTES {
        return Err(invalid("invalid encoded size"));
    }
    if &payload[..4] != MAGIC {
        return Err(invalid("invalid magic"));
    }
    if payload[4] != VERSION {
        return Err(invalid("unsupported version"));
    }
    let kind = match payload[33] {
        0 => ScheduledEventKind::Serial,
        1 => ScheduledEventKind::Parallel,
        _ => return Err(invalid("invalid event kind")),
    };
    let authority = RiscvO3LiveCheckpointWake {
        scheduler_instance_raw: u64::from_le_bytes(payload[5..13].try_into().unwrap()),
        partition: PartitionId::new(u32::from_le_bytes(payload[13..17].try_into().unwrap())),
        tick: u64::from_le_bytes(payload[17..25].try_into().unwrap()),
        scheduler_order: u64::from_le_bytes(payload[25..33].try_into().unwrap()),
        kind,
    };
    if authority != expected {
        return Err(RiscvCoreCheckpointError::MismatchedO3LiveWakeAuthority {
            component: component.clone(),
        });
    }
    Ok(())
}
