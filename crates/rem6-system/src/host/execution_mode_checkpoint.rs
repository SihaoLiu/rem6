use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointManifest};

use crate::{ExecutionMode, ExecutionModeTarget};

const EXECUTION_MODE_CHECKPOINT_COMPONENT: &str = "host.execution_modes";
pub(super) const EXECUTION_MODE_CHECKPOINT_CHUNK: &str = "modes";
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionModeCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        name: String,
    },
    UnknownMode {
        component: CheckpointComponentId,
        name: String,
        code: u8,
    },
    DuplicateTarget {
        component: CheckpointComponentId,
        target: ExecutionModeTarget,
    },
}

impl fmt::Display for ExecutionModeCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "execution mode checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, name } => write!(
                formatter,
                "execution mode checkpoint component {} has invalid chunk {name}",
                component.as_str()
            ),
            Self::UnknownMode {
                component,
                name,
                code,
            } => write!(
                formatter,
                "execution mode checkpoint component {} chunk {name} has unknown mode code {code}",
                component.as_str()
            ),
            Self::DuplicateTarget { component, target } => write!(
                formatter,
                "execution mode checkpoint component {} repeats target {}",
                component.as_str(),
                target.as_str()
            ),
        }
    }
}

impl Error for ExecutionModeCheckpointError {}

pub(super) fn execution_mode_checkpoint_component() -> CheckpointComponentId {
    CheckpointComponentId::new(EXECUTION_MODE_CHECKPOINT_COMPONENT)
        .expect("execution mode checkpoint component id is non-empty")
}

pub(super) fn manifest_has_execution_mode_checkpoint(manifest: &CheckpointManifest) -> bool {
    manifest
        .states()
        .iter()
        .any(|state| state.component().as_str() == EXECUTION_MODE_CHECKPOINT_COMPONENT)
}

pub fn decode_execution_mode_authority_from_manifest(
    manifest: &CheckpointManifest,
) -> Result<Option<BTreeMap<ExecutionModeTarget, ExecutionMode>>, ExecutionModeCheckpointError> {
    let component = execution_mode_checkpoint_component();
    let Some(state) = manifest
        .states()
        .iter()
        .find(|state| state.component() == &component)
    else {
        return Ok(None);
    };
    let payload = state
        .chunks()
        .iter()
        .find(|chunk| chunk.name() == EXECUTION_MODE_CHECKPOINT_CHUNK)
        .map(|chunk| chunk.payload())
        .ok_or_else(|| ExecutionModeCheckpointError::MissingChunk {
            component: component.clone(),
            name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
        })?;
    let modes = decode_execution_modes(&component, payload)?;
    if modes.is_empty() {
        Ok(None)
    } else {
        Ok(Some(modes))
    }
}

pub(super) fn encode_execution_modes(
    modes: &BTreeMap<ExecutionModeTarget, ExecutionMode>,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(modes.len() as u64).to_le_bytes());
    for (target, mode) in modes {
        let target = target.as_str().as_bytes();
        payload.extend_from_slice(&(target.len() as u64).to_le_bytes());
        payload.extend_from_slice(target);
        payload.push(execution_mode_code(*mode));
    }
    payload
}

pub(super) fn decode_execution_modes(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<BTreeMap<ExecutionModeTarget, ExecutionMode>, ExecutionModeCheckpointError> {
    let mut cursor = 0;
    let count = read_u64(component, payload, &mut cursor)?;
    let mut modes = BTreeMap::new();
    for _ in 0..count {
        let target_len = read_u64(component, payload, &mut cursor)? as usize;
        let target_end = cursor.checked_add(target_len).ok_or_else(|| {
            ExecutionModeCheckpointError::InvalidChunk {
                component: component.clone(),
                name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
            }
        })?;
        let target_bytes = payload.get(cursor..target_end).ok_or_else(|| {
            ExecutionModeCheckpointError::InvalidChunk {
                component: component.clone(),
                name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
            }
        })?;
        let target = std::str::from_utf8(target_bytes)
            .map_err(|_| ExecutionModeCheckpointError::InvalidChunk {
                component: component.clone(),
                name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
            })?
            .to_string();
        cursor = target_end;
        let mode_code =
            *payload
                .get(cursor)
                .ok_or_else(|| ExecutionModeCheckpointError::InvalidChunk {
                    component: component.clone(),
                    name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
                })?;
        cursor += 1;
        let target = ExecutionModeTarget::new(target);
        let mode = execution_mode_from_code(component, mode_code)?;
        if modes.insert(target.clone(), mode).is_some() {
            return Err(ExecutionModeCheckpointError::DuplicateTarget {
                component: component.clone(),
                target,
            });
        }
    }

    if cursor != payload.len() {
        return Err(ExecutionModeCheckpointError::InvalidChunk {
            component: component.clone(),
            name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
        });
    }
    Ok(modes)
}

fn read_u64(
    component: &CheckpointComponentId,
    payload: &[u8],
    cursor: &mut usize,
) -> Result<u64, ExecutionModeCheckpointError> {
    let end = cursor.checked_add(U64_BYTES).ok_or_else(|| {
        ExecutionModeCheckpointError::InvalidChunk {
            component: component.clone(),
            name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
        }
    })?;
    let bytes =
        payload
            .get(*cursor..end)
            .ok_or_else(|| ExecutionModeCheckpointError::InvalidChunk {
                component: component.clone(),
                name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
            })?;
    *cursor = end;
    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .expect("checkpoint u64 slice width is fixed"),
    ))
}

const fn execution_mode_code(mode: ExecutionMode) -> u8 {
    match mode {
        ExecutionMode::Functional => 0,
        ExecutionMode::Timing => 1,
        ExecutionMode::Detailed => 2,
    }
}

fn execution_mode_from_code(
    component: &CheckpointComponentId,
    code: u8,
) -> Result<ExecutionMode, ExecutionModeCheckpointError> {
    match code {
        0 => Ok(ExecutionMode::Functional),
        1 => Ok(ExecutionMode::Timing),
        2 => Ok(ExecutionMode::Detailed),
        _ => Err(ExecutionModeCheckpointError::UnknownMode {
            component: component.clone(),
            name: EXECUTION_MODE_CHECKPOINT_CHUNK.to_string(),
            code,
        }),
    }
}

#[cfg(test)]
mod tests {
    use rem6_checkpoint::{CheckpointChunk, CheckpointState};

    use super::*;

    #[test]
    fn manifest_decoder_returns_none_without_execution_mode_authority() {
        let manifest = CheckpointManifest::new("without-authority", 17, Vec::new());

        assert_eq!(
            decode_execution_mode_authority_from_manifest(&manifest),
            Ok(None)
        );
    }

    #[test]
    fn manifest_decoder_returns_none_for_empty_execution_mode_authority() {
        let component = execution_mode_checkpoint_component();
        let manifest = CheckpointManifest::new(
            "empty-authority",
            17,
            vec![CheckpointState::new(
                component,
                vec![CheckpointChunk::new(
                    EXECUTION_MODE_CHECKPOINT_CHUNK,
                    encode_execution_modes(&BTreeMap::new()),
                )],
            )],
        );

        assert_eq!(
            decode_execution_mode_authority_from_manifest(&manifest),
            Ok(None)
        );
    }

    #[test]
    fn manifest_decoder_rejects_duplicate_execution_mode_targets() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&2_u64.to_le_bytes());
        for mode in [2_u8, 1_u8] {
            payload.extend_from_slice(&4_u64.to_le_bytes());
            payload.extend_from_slice(b"cpu0");
            payload.push(mode);
        }
        let component = execution_mode_checkpoint_component();
        let manifest = CheckpointManifest::new(
            "duplicate-authority",
            17,
            vec![CheckpointState::new(
                component.clone(),
                vec![CheckpointChunk::new(
                    EXECUTION_MODE_CHECKPOINT_CHUNK,
                    payload,
                )],
            )],
        );

        assert_eq!(
            decode_execution_mode_authority_from_manifest(&manifest),
            Err(ExecutionModeCheckpointError::DuplicateTarget {
                component,
                target: ExecutionModeTarget::new("cpu0"),
            })
        );
    }
}
