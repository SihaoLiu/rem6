use crate::encoding::{csr, funct3, rd, rs1};
use crate::{
    RiscvCounterCsr, RiscvCounterCsrWord, RiscvCounterEnableCsr, RiscvCounterEnableCsrInstruction,
    RiscvCsrOp, RiscvEnvironmentConfigCsr, RiscvEnvironmentConfigCsrInstruction, RiscvError,
    RiscvFloatCsr, RiscvInstruction, RiscvInterruptCsr, RiscvMachineInformationCsr,
    RiscvMachineInformationCsrInstruction, RiscvMachineTrapCsr, RiscvStatusCsr,
    RiscvSupervisorTrapCsr, RiscvTranslationCsr, RiscvTranslationCsrInstruction,
    RiscvVectorFixedPointCsr, RiscvVectorFixedPointCsrInstruction,
};

pub(crate) fn decode_csr(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let csr_address = csr(raw);
    if is_csr_no_write_read(raw) {
        return RiscvMachineInformationCsr::from_address(csr_address)
            .map(|csr| {
                RiscvInstruction::MachineInformationCsr(
                    RiscvMachineInformationCsrInstruction::read(rd(raw), csr),
                )
            })
            .or_else(|| {
                RiscvCounterCsr::from_user_address(csr_address)
                    .ok()
                    .map(|csr| RiscvInstruction::ReadCounterCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvCounterCsrWord::from_user_address(csr_address)
                    .ok()
                    .map(|csr| RiscvInstruction::ReadCounterCsrWord { rd: rd(raw), csr })
            })
            .or_else(|| {
                machine_counter_csr(csr_address)
                    .map(|csr| RiscvInstruction::ReadMachineCounterCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                machine_counter_csr_word(csr_address)
                    .map(|csr| RiscvInstruction::ReadMachineCounterCsrWord { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvFloatCsr::from_address(csr_address)
                    .map(|csr| RiscvInstruction::ReadFloatCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvVectorFixedPointCsr::from_address(csr_address).map(|csr| {
                    RiscvInstruction::VectorFixedPointCsr(
                        RiscvVectorFixedPointCsrInstruction::read(rd(raw), csr),
                    )
                })
            })
            .or_else(|| {
                RiscvStatusCsr::from_address(csr_address)
                    .map(|csr| RiscvInstruction::ReadStatusCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvEnvironmentConfigCsr::from_address(csr_address).map(|csr| {
                    RiscvInstruction::EnvironmentConfigCsr(
                        RiscvEnvironmentConfigCsrInstruction::read(rd(raw), csr),
                    )
                })
            })
            .or_else(|| {
                RiscvCounterEnableCsr::from_address(csr_address).map(|csr| {
                    RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::read(
                        rd(raw),
                        csr,
                    ))
                })
            })
            .or_else(|| {
                RiscvInterruptCsr::from_address(csr_address)
                    .map(|csr| RiscvInstruction::ReadInterruptCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvMachineTrapCsr::from_address(csr_address)
                    .map(|csr| RiscvInstruction::ReadMachineTrapCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvSupervisorTrapCsr::from_address(csr_address)
                    .map(|csr| RiscvInstruction::ReadSupervisorTrapCsr { rd: rd(raw), csr })
            })
            .or_else(|| {
                RiscvTranslationCsr::from_address(csr_address).map(|csr| {
                    RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::read(
                        rd(raw),
                        csr,
                    ))
                })
            })
            .ok_or(RiscvError::UnknownEncoding { raw });
    }

    if let Some(csr) = RiscvMachineInformationCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(decode_machine_information_csr(raw, csr, RiscvCsrOp::Write)),
            0x2 => Ok(decode_machine_information_csr(raw, csr, RiscvCsrOp::Set)),
            0x3 => Ok(decode_machine_information_csr(raw, csr, RiscvCsrOp::Clear)),
            0x5 => Ok(decode_machine_information_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Write,
            )),
            0x6 => Ok(decode_machine_information_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Set,
            )),
            0x7 => Ok(decode_machine_information_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Clear,
            )),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvFloatCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteFloatCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetFloatCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearFloatCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteFloatCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetFloatCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearFloatCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvVectorFixedPointCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(decode_vector_fixed_point_csr(raw, csr, RiscvCsrOp::Write)),
            0x2 => Ok(decode_vector_fixed_point_csr(raw, csr, RiscvCsrOp::Set)),
            0x3 => Ok(decode_vector_fixed_point_csr(raw, csr, RiscvCsrOp::Clear)),
            0x5 => Ok(decode_vector_fixed_point_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Write,
            )),
            0x6 => Ok(decode_vector_fixed_point_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Set,
            )),
            0x7 => Ok(decode_vector_fixed_point_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Clear,
            )),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvInterruptCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteInterruptCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetInterruptCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearInterruptCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteInterruptCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetInterruptCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearInterruptCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
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

    if let Some(csr) = RiscvEnvironmentConfigCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(decode_environment_config_csr(raw, csr, RiscvCsrOp::Write)),
            0x2 => Ok(decode_environment_config_csr(raw, csr, RiscvCsrOp::Set)),
            0x3 => Ok(decode_environment_config_csr(raw, csr, RiscvCsrOp::Clear)),
            0x5 => Ok(decode_environment_config_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Write,
            )),
            0x6 => Ok(decode_environment_config_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Set,
            )),
            0x7 => Ok(decode_environment_config_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Clear,
            )),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvCounterEnableCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(decode_counter_enable_csr(raw, csr, RiscvCsrOp::Write)),
            0x2 => Ok(decode_counter_enable_csr(raw, csr, RiscvCsrOp::Set)),
            0x3 => Ok(decode_counter_enable_csr(raw, csr, RiscvCsrOp::Clear)),
            0x5 => Ok(decode_counter_enable_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Write,
            )),
            0x6 => Ok(decode_counter_enable_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Set,
            )),
            0x7 => Ok(decode_counter_enable_csr_immediate(
                raw,
                csr,
                RiscvCsrOp::Clear,
            )),
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

    if let Some(csr) = RiscvSupervisorTrapCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteSupervisorTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetSupervisorTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearSupervisorTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteSupervisorTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetSupervisorTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearSupervisorTrapCsrImmediate {
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
        0x1 => Ok(decode_translation_csr(raw, csr, RiscvCsrOp::Write)),
        0x2 => Ok(decode_translation_csr(raw, csr, RiscvCsrOp::Set)),
        0x3 => Ok(decode_translation_csr(raw, csr, RiscvCsrOp::Clear)),
        0x5 => Ok(decode_translation_csr_immediate(
            raw,
            csr,
            RiscvCsrOp::Write,
        )),
        0x6 => Ok(decode_translation_csr_immediate(raw, csr, RiscvCsrOp::Set)),
        0x7 => Ok(decode_translation_csr_immediate(
            raw,
            csr,
            RiscvCsrOp::Clear,
        )),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn is_csr_no_write_read(raw: u32) -> bool {
    matches!((funct3(raw), rs1(raw).index()), (0x2 | 0x3 | 0x6 | 0x7, 0))
}

fn decode_machine_information_csr(
    raw: u32,
    csr: RiscvMachineInformationCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::register(
        rd(raw),
        csr,
        op,
        rs1(raw),
    ))
}

fn decode_machine_information_csr_immediate(
    raw: u32,
    csr: RiscvMachineInformationCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::MachineInformationCsr(RiscvMachineInformationCsrInstruction::immediate(
        rd(raw),
        csr,
        op,
        rs1(raw).index(),
    ))
}

fn decode_vector_fixed_point_csr(
    raw: u32,
    csr: RiscvVectorFixedPointCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::VectorFixedPointCsr(RiscvVectorFixedPointCsrInstruction::register(
        rd(raw),
        csr,
        op,
        rs1(raw),
    ))
}

fn decode_vector_fixed_point_csr_immediate(
    raw: u32,
    csr: RiscvVectorFixedPointCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::VectorFixedPointCsr(RiscvVectorFixedPointCsrInstruction::immediate(
        rd(raw),
        csr,
        op,
        rs1(raw).index(),
    ))
}

fn decode_environment_config_csr(
    raw: u32,
    csr: RiscvEnvironmentConfigCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::register(
        rd(raw),
        csr,
        op,
        rs1(raw),
    ))
}

fn decode_environment_config_csr_immediate(
    raw: u32,
    csr: RiscvEnvironmentConfigCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::EnvironmentConfigCsr(RiscvEnvironmentConfigCsrInstruction::immediate(
        rd(raw),
        csr,
        op,
        rs1(raw).index(),
    ))
}

fn decode_counter_enable_csr(
    raw: u32,
    csr: RiscvCounterEnableCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::register(
        rd(raw),
        csr,
        op,
        rs1(raw),
    ))
}

fn decode_counter_enable_csr_immediate(
    raw: u32,
    csr: RiscvCounterEnableCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::CounterEnableCsr(RiscvCounterEnableCsrInstruction::immediate(
        rd(raw),
        csr,
        op,
        rs1(raw).index(),
    ))
}

fn decode_translation_csr(raw: u32, csr: RiscvTranslationCsr, op: RiscvCsrOp) -> RiscvInstruction {
    RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::register(
        rd(raw),
        csr,
        op,
        rs1(raw),
    ))
}

fn decode_translation_csr_immediate(
    raw: u32,
    csr: RiscvTranslationCsr,
    op: RiscvCsrOp,
) -> RiscvInstruction {
    RiscvInstruction::TranslationCsr(RiscvTranslationCsrInstruction::immediate(
        rd(raw),
        csr,
        op,
        rs1(raw).index(),
    ))
}

fn machine_counter_csr(address: u16) -> Option<RiscvCounterCsr> {
    RiscvCounterCsr::from_machine_address(address).ok()
}

fn machine_counter_csr_word(address: u16) -> Option<RiscvCounterCsrWord> {
    RiscvCounterCsrWord::from_machine_address(address).ok()
}
