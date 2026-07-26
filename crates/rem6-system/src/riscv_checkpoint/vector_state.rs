use rem6_checkpoint::CheckpointComponentId;
use rem6_isa_riscv::{
    RiscvVectorArchitecturalState, RiscvVectorConfig, RiscvVectorFixedPointState,
    RiscvVectorFixedRoundingMode, RISCV_VECTOR_REGISTER_BYTES, RISCV_VECTOR_REGISTER_COUNT,
};

use super::RiscvCoreCheckpointError;

pub(super) const RISCV_STATE_VERSION_CHUNK: &str = "riscv-state-version";
pub(super) const VECTOR_STATE_CHUNK: &str = "vector-state";
pub(super) const RISCV_STATE_VERSION: u8 = 1;
const VECTOR_STATE_VERSION: u8 = 1;
const RISCV_STATE_VERSION_BYTES: usize = 1;
const VECTOR_STATE_BYTES: usize = 526;
const VECTOR_VERSION_OFFSET: usize = 0;
const VECTOR_VL_OFFSET: usize = 1;
const VECTOR_VL_BYTES: usize = 4;
const VECTOR_VTYPE_OFFSET: usize = VECTOR_VL_OFFSET + VECTOR_VL_BYTES;
const VECTOR_VTYPE_BYTES: usize = 8;
const VECTOR_VCSR_OFFSET: usize = VECTOR_VTYPE_OFFSET + VECTOR_VTYPE_BYTES;
const VECTOR_REGISTERS_OFFSET: usize = VECTOR_VCSR_OFFSET + 1;
const _: () = assert!(
    VECTOR_STATE_BYTES
        == VECTOR_REGISTERS_OFFSET + RISCV_VECTOR_REGISTER_COUNT * RISCV_VECTOR_REGISTER_BYTES
);

pub(super) fn encode_vector_architectural_state(state: &RiscvVectorArchitecturalState) -> Vec<u8> {
    let config = state.config();
    let fixed_point = state.fixed_point();
    let registers = state.registers();
    let mut payload = vec![0; VECTOR_STATE_BYTES];
    payload[VECTOR_VERSION_OFFSET] = VECTOR_STATE_VERSION;
    payload[VECTOR_VL_OFFSET..VECTOR_VL_OFFSET + VECTOR_VL_BYTES]
        .copy_from_slice(&config.vl().to_le_bytes());
    payload[VECTOR_VTYPE_OFFSET..VECTOR_VTYPE_OFFSET + VECTOR_VTYPE_BYTES]
        .copy_from_slice(&config.vtype().to_le_bytes());
    payload[VECTOR_VCSR_OFFSET] = fixed_point.vcsr_bits();
    for (index, register) in registers.iter().enumerate() {
        let offset = VECTOR_REGISTERS_OFFSET + index * RISCV_VECTOR_REGISTER_BYTES;
        payload[offset..offset + RISCV_VECTOR_REGISTER_BYTES].copy_from_slice(register);
    }
    payload
}

pub(super) fn decode_vector_architectural_state(
    component: &CheckpointComponentId,
    state_version: Option<&[u8]>,
    vector_state: Option<&[u8]>,
) -> Result<RiscvVectorArchitecturalState, RiscvCoreCheckpointError> {
    match (state_version, vector_state) {
        (None, None) => Ok(RiscvVectorArchitecturalState::default()),
        (None, Some(_)) => Err(
            RiscvCoreCheckpointError::UnexpectedVectorStateWithoutVersion {
                component: component.clone(),
            },
        ),
        (Some(state_version), vector_state) => {
            if state_version.len() != RISCV_STATE_VERSION_BYTES {
                return Err(RiscvCoreCheckpointError::InvalidChunkSize {
                    component: component.clone(),
                    name: RISCV_STATE_VERSION_CHUNK.to_string(),
                    expected: RISCV_STATE_VERSION_BYTES,
                    actual: state_version.len(),
                });
            }
            let state_version = state_version[0];
            if state_version != RISCV_STATE_VERSION {
                return Err(RiscvCoreCheckpointError::UnsupportedRiscvStateVersion {
                    component: component.clone(),
                    version: state_version,
                });
            }
            let vector_state =
                vector_state.ok_or_else(|| RiscvCoreCheckpointError::MissingChunk {
                    component: component.clone(),
                    name: VECTOR_STATE_CHUNK.to_string(),
                })?;
            if vector_state.len() != VECTOR_STATE_BYTES {
                return Err(RiscvCoreCheckpointError::InvalidChunkSize {
                    component: component.clone(),
                    name: VECTOR_STATE_CHUNK.to_string(),
                    expected: VECTOR_STATE_BYTES,
                    actual: vector_state.len(),
                });
            }
            let vector_version = vector_state[VECTOR_VERSION_OFFSET];
            if vector_version != VECTOR_STATE_VERSION {
                return Err(RiscvCoreCheckpointError::UnsupportedVectorStateVersion {
                    component: component.clone(),
                    version: vector_version,
                });
            }
            let vcsr = vector_state[VECTOR_VCSR_OFFSET];
            if vcsr & !0b111 != 0 {
                return Err(RiscvCoreCheckpointError::InvalidVectorStateVcsr {
                    component: component.clone(),
                    value: vcsr,
                });
            }

            let vl = u32::from_le_bytes(
                vector_state[VECTOR_VL_OFFSET..VECTOR_VL_OFFSET + VECTOR_VL_BYTES]
                    .try_into()
                    .expect("validated vector state vl width"),
            );
            let vtype = u64::from_le_bytes(
                vector_state[VECTOR_VTYPE_OFFSET..VECTOR_VTYPE_OFFSET + VECTOR_VTYPE_BYTES]
                    .try_into()
                    .expect("validated vector state vtype width"),
            );
            let mut fixed_point =
                RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundNearestUp);
            fixed_point.write_vcsr_bits(vcsr);
            let mut registers = [[0; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT];
            for (index, register) in registers.iter_mut().enumerate() {
                let offset = VECTOR_REGISTERS_OFFSET + index * RISCV_VECTOR_REGISTER_BYTES;
                register
                    .copy_from_slice(&vector_state[offset..offset + RISCV_VECTOR_REGISTER_BYTES]);
            }
            Ok(RiscvVectorArchitecturalState::new(
                RiscvVectorConfig::new(vl, vtype),
                fixed_point,
                registers,
            ))
        }
    }
}
