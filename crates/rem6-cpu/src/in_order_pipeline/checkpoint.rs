use super::*;

impl InOrderPipelineCheckpointPayload {
    pub fn from_state(state: &InOrderPipelineState) -> Self {
        Self {
            snapshot: state.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: InOrderPipelineSnapshot) -> Result<Self, InOrderPipelineError> {
        let state = InOrderPipelineState::restore(snapshot)?;
        Ok(Self {
            snapshot: state.snapshot(),
        })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, InOrderPipelineError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected: CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(InOrderPipelineError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = payload[offset];
        offset += 1;
        let instruction_bytes = match version {
            CHECKPOINT_VERSION_V1 => CHECKPOINT_V1_INSTRUCTION_BYTES,
            CHECKPOINT_VERSION_V2 => CHECKPOINT_V2_INSTRUCTION_BYTES,
            CHECKPOINT_VERSION_V3 => CHECKPOINT_V3_INSTRUCTION_BYTES,
            _ => return Err(InOrderPipelineError::UnsupportedCheckpointVersion { version }),
        };

        let cycle = read_u64(payload, &mut offset);
        let mut widths = Vec::with_capacity(InOrderPipelineStage::ALL.len());
        for stage in InOrderPipelineStage::ALL {
            let slots = read_u32(payload, &mut offset) as usize;
            widths.push(InOrderPipelineStageWidth::new(stage, slots)?);
        }
        let config = InOrderPipelineConfig::new(widths)?;
        let instruction_count = read_u32(payload, &mut offset) as usize;
        let instruction_bytes = instruction_count.checked_mul(instruction_bytes).ok_or(
            InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected: usize::MAX,
                actual: payload.len(),
            },
        )?;
        let expected = CHECKPOINT_HEADER_BYTES
            .checked_add(instruction_bytes)
            .ok_or(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected: usize::MAX,
                actual: payload.len(),
            })?;
        if payload.len() != expected {
            return Err(InOrderPipelineError::InvalidCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let mut in_flight = Vec::with_capacity(instruction_count);
        for _ in 0..instruction_count {
            let sequence = read_u64(payload, &mut offset);
            let stage = decode_checkpoint_stage(payload[offset])?;
            offset += 1;
            let instruction = InOrderPipelineInstruction::new(sequence, stage);
            let instruction =
                if version == CHECKPOINT_VERSION_V2 || version == CHECKPOINT_VERSION_V3 {
                    decode_execute_wait(
                        instruction,
                        payload,
                        &mut offset,
                        version == CHECKPOINT_VERSION_V3,
                    )?
                } else {
                    instruction
                };
            in_flight.push(instruction);
        }

        Self::from_snapshot(InOrderPipelineSnapshot::with_cycle(
            config, cycle, in_flight,
        ))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("in-order checkpoint payload values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, InOrderPipelineError> {
        let in_flight = self.snapshot.in_flight();
        let version = if in_flight
            .iter()
            .any(|instruction| instruction.execute_wait_key().is_some())
        {
            CHECKPOINT_VERSION_V3
        } else if in_flight
            .iter()
            .any(|instruction| instruction.execute_wait_remaining_cycles().is_some())
        {
            CHECKPOINT_VERSION_V2
        } else {
            CHECKPOINT_VERSION_V1
        };
        let instruction_bytes = match version {
            CHECKPOINT_VERSION_V1 => CHECKPOINT_V1_INSTRUCTION_BYTES,
            CHECKPOINT_VERSION_V2 => CHECKPOINT_V2_INSTRUCTION_BYTES,
            CHECKPOINT_VERSION_V3 => CHECKPOINT_V3_INSTRUCTION_BYTES,
            _ => unreachable!("selected checkpoint version is supported"),
        };
        let mut payload =
            Vec::with_capacity(CHECKPOINT_HEADER_BYTES + in_flight.len() * instruction_bytes);
        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(version);
        payload.extend_from_slice(&self.snapshot.cycle().to_le_bytes());
        for stage in InOrderPipelineStage::ALL {
            let width = encode_checkpoint_u32("stage width", self.snapshot.config().width(stage))?;
            payload.extend_from_slice(&width.to_le_bytes());
        }
        let in_flight_count =
            encode_checkpoint_u32("in-flight instruction count", in_flight.len())?;
        payload.extend_from_slice(&in_flight_count.to_le_bytes());
        for instruction in in_flight {
            payload.extend_from_slice(&instruction.sequence().to_le_bytes());
            payload.push(encode_checkpoint_stage(instruction.stage()));
            if version == CHECKPOINT_VERSION_V2 || version == CHECKPOINT_VERSION_V3 {
                encode_execute_wait(*instruction, &mut payload, version == CHECKPOINT_VERSION_V3);
            }
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &InOrderPipelineSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> InOrderPipelineSnapshot {
        self.snapshot
    }
}

fn encode_execute_wait(
    instruction: InOrderPipelineInstruction,
    payload: &mut Vec<u8>,
    include_key: bool,
) {
    match instruction.execute_wait_cycles {
        Some((total_cycles, remaining_cycles)) => {
            payload.push(1);
            payload.extend_from_slice(&total_cycles.to_le_bytes());
            payload.extend_from_slice(&remaining_cycles.to_le_bytes());
        }
        None => {
            payload.push(0);
            payload.extend_from_slice(&0_u64.to_le_bytes());
            payload.extend_from_slice(&0_u64.to_le_bytes());
        }
    }
    if include_key {
        payload.extend_from_slice(&instruction.execute_wait_key().unwrap_or(0).to_le_bytes());
    }
}

fn decode_execute_wait(
    instruction: InOrderPipelineInstruction,
    payload: &[u8],
    offset: &mut usize,
    includes_key: bool,
) -> Result<InOrderPipelineInstruction, InOrderPipelineError> {
    let code = payload[*offset];
    *offset += 1;
    let total_cycles = read_u64(payload, offset);
    let remaining_cycles = read_u64(payload, offset);
    let key = includes_key.then(|| read_u64(payload, offset));
    if code == 0 && total_cycles == 0 && remaining_cycles == 0 && key.is_some_and(|key| key != 0) {
        return Err(InOrderPipelineError::InvalidCheckpointExecuteWait {
            code,
            total_cycles,
            remaining_cycles,
        });
    }
    match (code, total_cycles, remaining_cycles) {
        (0, 0, 0) => Ok(instruction),
        (1, total_cycles, remaining_cycles) => Ok(match key {
            Some(key) if key > 0 => {
                instruction.with_execute_wait_key(total_cycles, remaining_cycles, key)
            }
            _ => instruction.with_execute_wait(total_cycles, remaining_cycles),
        }),
        _ => Err(InOrderPipelineError::InvalidCheckpointExecuteWait {
            code,
            total_cycles,
            remaining_cycles,
        }),
    }
}

fn encode_checkpoint_stage(stage: InOrderPipelineStage) -> u8 {
    match stage {
        InOrderPipelineStage::Fetch1 => 0,
        InOrderPipelineStage::Fetch2 => 1,
        InOrderPipelineStage::Decode => 2,
        InOrderPipelineStage::Execute => 3,
        InOrderPipelineStage::Commit => 4,
    }
}

fn decode_checkpoint_stage(code: u8) -> Result<InOrderPipelineStage, InOrderPipelineError> {
    match code {
        0 => Ok(InOrderPipelineStage::Fetch1),
        1 => Ok(InOrderPipelineStage::Fetch2),
        2 => Ok(InOrderPipelineStage::Decode),
        3 => Ok(InOrderPipelineStage::Execute),
        4 => Ok(InOrderPipelineStage::Commit),
        _ => Err(InOrderPipelineError::InvalidCheckpointStageCode { code }),
    }
}

fn encode_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, InOrderPipelineError> {
    u32::try_from(value).map_err(|_| InOrderPipelineError::CheckpointValueTooLarge {
        field,
        value,
        maximum: CHECKPOINT_U32_MAX,
    })
}

fn read_u32(payload: &[u8], offset: &mut usize) -> u32 {
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    u32::from_le_bytes(bytes)
}

fn read_u64(payload: &[u8], offset: &mut usize) -> u64 {
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    u64::from_le_bytes(bytes)
}
