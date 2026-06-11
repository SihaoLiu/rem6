use crate::encoding::{aq, funct3, funct5, rd, rl, rs1, rs2};
use crate::{AtomicMemoryOp, MemoryWidth, RiscvError, RiscvInstruction};

pub(crate) fn decode_atomic(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = match funct3(raw) {
        0x2 => MemoryWidth::Word,
        0x3 => MemoryWidth::Doubleword,
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };

    match (funct5(raw), rs2(raw).index()) {
        (0x02, 0) => Ok(RiscvInstruction::LoadReserved {
            rd: rd(raw),
            rs1: rs1(raw),
            width,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x03, _) => Ok(RiscvInstruction::StoreConditional {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (funct5, _) => atomic_memory_op(funct5)
            .map(|op| RiscvInstruction::AtomicMemory {
                rd: rd(raw),
                rs1: rs1(raw),
                rs2: rs2(raw),
                width,
                op,
                acquire: aq(raw),
                release: rl(raw),
            })
            .ok_or(RiscvError::UnknownEncoding { raw }),
    }
}

fn atomic_memory_op(funct5: u32) -> Option<AtomicMemoryOp> {
    match funct5 {
        0x00 => Some(AtomicMemoryOp::Add),
        0x01 => Some(AtomicMemoryOp::Swap),
        0x04 => Some(AtomicMemoryOp::Xor),
        0x08 => Some(AtomicMemoryOp::Or),
        0x0c => Some(AtomicMemoryOp::And),
        0x10 => Some(AtomicMemoryOp::MinSigned),
        0x14 => Some(AtomicMemoryOp::MaxSigned),
        0x18 => Some(AtomicMemoryOp::MinUnsigned),
        0x1c => Some(AtomicMemoryOp::MaxUnsigned),
        _ => None,
    }
}
