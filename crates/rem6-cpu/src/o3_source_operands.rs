use rem6_isa_riscv::{Register, RiscvCsrOperand, RiscvInstruction};

pub(super) fn o3_scalar_integer_source_registers(instruction: &RiscvInstruction) -> Vec<Register> {
    match instruction {
        RiscvInstruction::Addi { rs1, .. }
        | RiscvInstruction::Slti { rs1, .. }
        | RiscvInstruction::Sltiu { rs1, .. }
        | RiscvInstruction::Xori { rs1, .. }
        | RiscvInstruction::Ori { rs1, .. }
        | RiscvInstruction::Andi { rs1, .. }
        | RiscvInstruction::Slli { rs1, .. }
        | RiscvInstruction::Srli { rs1, .. }
        | RiscvInstruction::Srai { rs1, .. }
        | RiscvInstruction::Addiw { rs1, .. }
        | RiscvInstruction::Slliw { rs1, .. }
        | RiscvInstruction::Srliw { rs1, .. }
        | RiscvInstruction::Sraiw { rs1, .. }
        | RiscvInstruction::Jalr { rs1, .. }
        | RiscvInstruction::Load { rs1, .. }
        | RiscvInstruction::FloatLoad { rs1, .. }
        | RiscvInstruction::FloatStore { rs1, .. }
        | RiscvInstruction::LoadReserved { rs1, .. }
        | RiscvInstruction::WriteCounterCsr { rs1, .. }
        | RiscvInstruction::SetCounterCsr { rs1, .. }
        | RiscvInstruction::ClearCounterCsr { rs1, .. }
        | RiscvInstruction::WriteFloatCsr { rs1, .. }
        | RiscvInstruction::SetFloatCsr { rs1, .. }
        | RiscvInstruction::ClearFloatCsr { rs1, .. }
        | RiscvInstruction::WriteStatusCsr { rs1, .. }
        | RiscvInstruction::SetStatusCsr { rs1, .. }
        | RiscvInstruction::ClearStatusCsr { rs1, .. }
        | RiscvInstruction::WriteInterruptCsr { rs1, .. }
        | RiscvInstruction::SetInterruptCsr { rs1, .. }
        | RiscvInstruction::ClearInterruptCsr { rs1, .. }
        | RiscvInstruction::WriteMachineTrapCsr { rs1, .. }
        | RiscvInstruction::SetMachineTrapCsr { rs1, .. }
        | RiscvInstruction::ClearMachineTrapCsr { rs1, .. }
        | RiscvInstruction::WriteSupervisorTrapCsr { rs1, .. }
        | RiscvInstruction::SetSupervisorTrapCsr { rs1, .. }
        | RiscvInstruction::ClearSupervisorTrapCsr { rs1, .. } => vec![*rs1],
        RiscvInstruction::Add { rs1, rs2, .. }
        | RiscvInstruction::Sub { rs1, rs2, .. }
        | RiscvInstruction::Sll { rs1, rs2, .. }
        | RiscvInstruction::Slt { rs1, rs2, .. }
        | RiscvInstruction::Sltu { rs1, rs2, .. }
        | RiscvInstruction::Xor { rs1, rs2, .. }
        | RiscvInstruction::Srl { rs1, rs2, .. }
        | RiscvInstruction::Sra { rs1, rs2, .. }
        | RiscvInstruction::Or { rs1, rs2, .. }
        | RiscvInstruction::And { rs1, rs2, .. }
        | RiscvInstruction::Mul { rs1, rs2, .. }
        | RiscvInstruction::Mulh { rs1, rs2, .. }
        | RiscvInstruction::Mulhsu { rs1, rs2, .. }
        | RiscvInstruction::Mulhu { rs1, rs2, .. }
        | RiscvInstruction::Div { rs1, rs2, .. }
        | RiscvInstruction::Divu { rs1, rs2, .. }
        | RiscvInstruction::Rem { rs1, rs2, .. }
        | RiscvInstruction::Remu { rs1, rs2, .. }
        | RiscvInstruction::Mulw { rs1, rs2, .. }
        | RiscvInstruction::Divw { rs1, rs2, .. }
        | RiscvInstruction::Divuw { rs1, rs2, .. }
        | RiscvInstruction::Remw { rs1, rs2, .. }
        | RiscvInstruction::Remuw { rs1, rs2, .. }
        | RiscvInstruction::Addw { rs1, rs2, .. }
        | RiscvInstruction::Subw { rs1, rs2, .. }
        | RiscvInstruction::Sllw { rs1, rs2, .. }
        | RiscvInstruction::Srlw { rs1, rs2, .. }
        | RiscvInstruction::Sraw { rs1, rs2, .. }
        | RiscvInstruction::Beq { rs1, rs2, .. }
        | RiscvInstruction::Bne { rs1, rs2, .. }
        | RiscvInstruction::Blt { rs1, rs2, .. }
        | RiscvInstruction::Bge { rs1, rs2, .. }
        | RiscvInstruction::Bltu { rs1, rs2, .. }
        | RiscvInstruction::Bgeu { rs1, rs2, .. }
        | RiscvInstruction::Store { rs1, rs2, .. }
        | RiscvInstruction::StoreConditional { rs1, rs2, .. }
        | RiscvInstruction::AtomicMemory { rs1, rs2, .. }
        | RiscvInstruction::SfenceVma { rs1, rs2 } => vec![*rs1, *rs2],
        RiscvInstruction::MachineInformationCsr(instruction) => {
            csr_operand_register(instruction.operand())
                .into_iter()
                .collect()
        }
        RiscvInstruction::EnvironmentConfigCsr(instruction) => {
            csr_operand_register(instruction.operand())
                .into_iter()
                .collect()
        }
        RiscvInstruction::CounterEnableCsr(instruction) => {
            csr_operand_register(instruction.operand())
                .into_iter()
                .collect()
        }
        RiscvInstruction::CounterInhibitCsr(instruction) => {
            csr_operand_register(instruction.operand())
                .into_iter()
                .collect()
        }
        RiscvInstruction::VectorFixedPointCsr(instruction) => {
            csr_operand_register(instruction.operand())
                .into_iter()
                .collect()
        }
        RiscvInstruction::TranslationCsr(instruction) => {
            csr_operand_register(instruction.operand())
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn csr_operand_register(operand: RiscvCsrOperand) -> Option<Register> {
    match operand {
        RiscvCsrOperand::Register(register) => Some(register),
        RiscvCsrOperand::Immediate(_) => None,
    }
}
