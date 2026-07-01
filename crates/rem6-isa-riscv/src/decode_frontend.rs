use crate::encoding::{j_imm, rd, u_imm};
use crate::{atomic, compressed, decode, float, load_store, pseudo};
use crate::{Immediate, RiscvError, RiscvInstruction};

impl RiscvInstruction {
    pub fn decode(raw: u32) -> Result<Self, RiscvError> {
        if raw & 0x3 != 0x3 {
            return Err(RiscvError::CompressedNotSupported { raw });
        }
        Self::decode_with_length(raw).map(RiscvDecodedInstruction::instruction)
    }

    pub fn decode_with_length(raw: u32) -> Result<RiscvDecodedInstruction, RiscvError> {
        if raw & 0x3 != 0x3 {
            return Ok(RiscvDecodedInstruction::new(
                compressed::decode_compressed(raw)?,
                2,
            ));
        }

        let opcode = raw & 0x7f;
        let instruction = match opcode {
            0x03 => load_store::decode_integer_load(raw),
            0x07 if load_store::opcode_uses_vector_memory(raw) => {
                load_store::decode_vector_load(raw)
            }
            0x07 => float::decode_float_load(raw),
            0x0f => decode::decode_fence(raw),
            0x13 => decode::decode_op_imm(raw),
            0x17 => Ok(Self::Auipc {
                rd: rd(raw),
                imm: Immediate::new(u_imm(raw)),
            }),
            0x1b => decode::decode_op_imm_32(raw),
            0x23 => load_store::decode_integer_store(raw),
            0x27 if load_store::opcode_uses_vector_memory(raw) => {
                load_store::decode_vector_store(raw)
            }
            0x27 => float::decode_float_store(raw),
            0x2f => atomic::decode_atomic(raw),
            0x33 => decode::decode_op(raw),
            0x3b => decode::decode_op_32(raw),
            0x37 => Ok(Self::Lui {
                rd: rd(raw),
                imm: Immediate::new(u_imm(raw)),
            }),
            0x63 => decode::decode_branch(raw),
            0x67 => decode::decode_jalr(raw),
            0x6f => Ok(Self::Jal {
                rd: rd(raw),
                offset: Immediate::new(j_imm(raw)),
            }),
            0x73 => decode::decode_system(raw),
            0x43 | 0x47 | 0x4b | 0x4f => float::decode_float_multiply_add(raw),
            0x53 => float::decode_float_op(raw),
            0x57 => decode::decode_vector(raw),
            0x7b => pseudo::decode_gem5_pseudo_op(raw),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        }?;
        Ok(RiscvDecodedInstruction::new(instruction, 4))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvDecodedInstruction {
    instruction: RiscvInstruction,
    bytes: u8,
}

impl RiscvDecodedInstruction {
    pub(crate) const fn new(instruction: RiscvInstruction, bytes: u8) -> Self {
        Self { instruction, bytes }
    }

    pub const fn instruction(self) -> RiscvInstruction {
        self.instruction
    }

    pub const fn bytes(self) -> u8 {
        self.bytes
    }
}
