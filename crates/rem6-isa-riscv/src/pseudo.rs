use crate::{
    instruction::RiscvInstruction, record::RiscvSystemEvent, Register, RiscvError, RiscvHartState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvPseudoOp {
    WorkBegin,
    WorkEnd,
}

pub(crate) fn decode_gem5_pseudo_op(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    if raw & 0x01ff_ffff != 0x0000_007b {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    match raw >> 25 {
        0x5a => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::WorkBegin,
        }),
        0x5b => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::WorkEnd,
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn gem5_pseudo_system_event(
    op: RiscvPseudoOp,
    pc: u64,
    hart: &RiscvHartState,
) -> RiscvSystemEvent {
    let work_id = hart.read(Register::from_field(10));
    let thread_id = hart.read(Register::from_field(11));
    match op {
        RiscvPseudoOp::WorkBegin => RiscvSystemEvent::Gem5WorkBegin {
            pc,
            work_id,
            thread_id,
        },
        RiscvPseudoOp::WorkEnd => RiscvSystemEvent::Gem5WorkEnd {
            pc,
            work_id,
            thread_id,
        },
    }
}
