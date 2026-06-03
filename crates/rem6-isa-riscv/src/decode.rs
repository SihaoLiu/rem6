use crate::encoding::{csr, funct3, rd, rs1};
use crate::{
    RiscvCounterCsr, RiscvError, RiscvInstruction, RiscvMachineTrapCsr, RiscvStatusCsr,
    RiscvTranslationCsr,
};

pub(crate) fn decode_csr(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let csr_address = csr(raw);
    if is_csr_no_write_read(raw) {
        return match csr_address {
            0xf14 => Ok(RiscvInstruction::ReadMachineHartId { rd: rd(raw) }),
            csr_address => counter_csr(csr_address)
                .map(|csr| RiscvInstruction::ReadCounterCsr { rd: rd(raw), csr })
                .or_else(|| {
                    RiscvStatusCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadStatusCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvMachineTrapCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadMachineTrapCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvTranslationCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadTranslationCsr { rd: rd(raw), csr })
                })
                .ok_or(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = machine_counter_csr(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteCounterCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetCounterCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearCounterCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteCounterCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetCounterCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearCounterCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvStatusCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteStatusCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetStatusCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearStatusCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteStatusCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetStatusCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearStatusCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvMachineTrapCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteMachineTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetMachineTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearMachineTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteMachineTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetMachineTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearMachineTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    let Some(csr) = RiscvTranslationCsr::from_address(csr_address) else {
        return Err(RiscvError::UnknownEncoding { raw });
    };
    match funct3(raw) {
        0x1 => Ok(RiscvInstruction::WriteTranslationCsr {
            rd: rd(raw),
            csr,
            rs1: rs1(raw),
        }),
        0x2 => Ok(RiscvInstruction::SetTranslationCsr {
            rd: rd(raw),
            csr,
            rs1: rs1(raw),
        }),
        0x3 => Ok(RiscvInstruction::ClearTranslationCsr {
            rd: rd(raw),
            csr,
            rs1: rs1(raw),
        }),
        0x5 => Ok(RiscvInstruction::WriteTranslationCsrImmediate {
            rd: rd(raw),
            csr,
            zimm: rs1(raw).index(),
        }),
        0x6 => Ok(RiscvInstruction::SetTranslationCsrImmediate {
            rd: rd(raw),
            csr,
            zimm: rs1(raw).index(),
        }),
        0x7 => Ok(RiscvInstruction::ClearTranslationCsrImmediate {
            rd: rd(raw),
            csr,
            zimm: rs1(raw).index(),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn is_csr_no_write_read(raw: u32) -> bool {
    matches!((funct3(raw), rs1(raw).index()), (0x2 | 0x3 | 0x6 | 0x7, 0))
}

fn counter_csr(address: u16) -> Option<RiscvCounterCsr> {
    RiscvCounterCsr::from_user_address(address)
        .or_else(|_| RiscvCounterCsr::from_machine_address(address))
        .ok()
}

fn machine_counter_csr(address: u16) -> Option<RiscvCounterCsr> {
    RiscvCounterCsr::from_machine_address(address).ok()
}
