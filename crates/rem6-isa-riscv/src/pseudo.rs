use crate::{
    instruction::RiscvInstruction, record::RiscvSystemEvent, write_register, Register,
    RegisterWrite, RiscvError, RiscvHartState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvPseudoOp {
    Exit,
    Fail,
    Sum,
    ResetStats,
    DumpStats,
    DumpResetStats,
    Checkpoint,
    SwitchCpu,
    Hypercall,
    WorkBegin,
    WorkEnd,
}

pub(crate) fn decode_gem5_pseudo_op(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    if raw & 0x01ff_ffff != 0x0000_007b {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    match raw >> 25 {
        0x21 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Exit)),
        0x22 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Fail)),
        0x23 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Sum)),
        0x40 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::ResetStats)),
        0x41 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::DumpStats)),
        0x42 => Ok(RiscvInstruction::Gem5PseudoOp(
            RiscvPseudoOp::DumpResetStats,
        )),
        0x43 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Checkpoint)),
        0x52 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::SwitchCpu)),
        0x71 => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::Hypercall)),
        0x5a => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::WorkBegin)),
        0x5b => Ok(RiscvInstruction::Gem5PseudoOp(RiscvPseudoOp::WorkEnd)),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn execute_gem5_pseudo_op(
    op: RiscvPseudoOp,
    pc: u64,
    hart: &mut RiscvHartState,
    register_writes: &mut Vec<RegisterWrite>,
) -> Option<RiscvSystemEvent> {
    let a0 = hart.read(Register::from_field(10));
    let a1 = hart.read(Register::from_field(11));
    let event = match op {
        RiscvPseudoOp::Exit => Some(RiscvSystemEvent::Gem5Exit { pc, delay: a0 }),
        RiscvPseudoOp::Fail => Some(RiscvSystemEvent::Gem5Fail {
            pc,
            delay: a0,
            code: a1,
        }),
        RiscvPseudoOp::Sum => {
            let sum = (10..=15)
                .map(|index| hart.read(Register::from_field(index)))
                .fold(0_u64, u64::wrapping_add);
            write_register(hart, register_writes, Register::from_field(10), sum);
            return None;
        }
        RiscvPseudoOp::ResetStats => Some(RiscvSystemEvent::Gem5ResetStats {
            pc,
            delay: a0,
            period: a1,
        }),
        RiscvPseudoOp::DumpStats => Some(RiscvSystemEvent::Gem5DumpStats {
            pc,
            delay: a0,
            period: a1,
        }),
        RiscvPseudoOp::DumpResetStats => Some(RiscvSystemEvent::Gem5DumpResetStats {
            pc,
            delay: a0,
            period: a1,
        }),
        RiscvPseudoOp::Checkpoint => Some(RiscvSystemEvent::Gem5Checkpoint {
            pc,
            delay: a0,
            period: a1,
        }),
        RiscvPseudoOp::SwitchCpu => Some(RiscvSystemEvent::Gem5SwitchCpu { pc }),
        RiscvPseudoOp::Hypercall => Some(RiscvSystemEvent::Gem5Hypercall { pc, selector: a0 }),
        RiscvPseudoOp::WorkBegin => Some(RiscvSystemEvent::Gem5WorkBegin {
            pc,
            work_id: a0,
            thread_id: a1,
        }),
        RiscvPseudoOp::WorkEnd => Some(RiscvSystemEvent::Gem5WorkEnd {
            pc,
            work_id: a0,
            thread_id: a1,
        }),
    };
    write_register(hart, register_writes, Register::from_field(10), 0);
    event
}
