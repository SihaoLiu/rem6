use crate::{
    instruction::RiscvInstruction, record::RiscvSystemEvent, Register, RiscvError, RiscvHartState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvPseudoOp {
    Exit,
    Fail,
    ResetStats,
    DumpStats,
    DumpResetStats,
    WorkBegin,
    WorkEnd,
}

pub(crate) fn decode_gem5_pseudo_op(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    if raw & 0x01ff_ffff != 0x0000_007b {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    match raw >> 25 {
        0x21 => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::Exit,
        }),
        0x22 => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::Fail,
        }),
        0x40 => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::ResetStats,
        }),
        0x41 => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::DumpStats,
        }),
        0x42 => Ok(RiscvInstruction::Gem5PseudoOp {
            op: RiscvPseudoOp::DumpResetStats,
        }),
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
    let a0 = hart.read(Register::from_field(10));
    let a1 = hart.read(Register::from_field(11));
    match op {
        RiscvPseudoOp::Exit => RiscvSystemEvent::Gem5Exit { pc, delay: a0 },
        RiscvPseudoOp::Fail => RiscvSystemEvent::Gem5Fail {
            pc,
            delay: a0,
            code: a1,
        },
        RiscvPseudoOp::ResetStats => RiscvSystemEvent::Gem5ResetStats {
            pc,
            delay: a0,
            period: a1,
        },
        RiscvPseudoOp::DumpStats => RiscvSystemEvent::Gem5DumpStats {
            pc,
            delay: a0,
            period: a1,
        },
        RiscvPseudoOp::DumpResetStats => RiscvSystemEvent::Gem5DumpResetStats {
            pc,
            delay: a0,
            period: a1,
        },
        RiscvPseudoOp::WorkBegin => RiscvSystemEvent::Gem5WorkBegin {
            pc,
            work_id: a0,
            thread_id: a1,
        },
        RiscvPseudoOp::WorkEnd => RiscvSystemEvent::Gem5WorkEnd {
            pc,
            work_id: a0,
            thread_id: a1,
        },
    }
}
