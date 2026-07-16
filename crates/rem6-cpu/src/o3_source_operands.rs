use rem6_isa_riscv::{Register, RiscvCsrOperand, RiscvInstruction};

use crate::{
    riscv_branch_kind::{is_riscv_link_register, riscv_branch_target_kind},
    BranchTargetKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveControlOperands {
    kind: BranchTargetKind,
    sources: Vec<Register>,
    destination: Option<Register>,
}

impl O3LiveControlOperands {
    pub(crate) const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub(crate) fn sources(&self) -> &[Register] {
        &self.sources
    }

    pub(crate) const fn destination(&self) -> Option<Register> {
        self.destination
    }
}

pub(crate) fn o3_scalar_integer_source_registers(instruction: &RiscvInstruction) -> Vec<Register> {
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

pub(crate) const fn o3_scalar_integer_destination(
    instruction: RiscvInstruction,
) -> Option<Register> {
    match instruction {
        RiscvInstruction::Lui { rd, .. }
        | RiscvInstruction::Auipc { rd, .. }
        | RiscvInstruction::Addi { rd, .. }
        | RiscvInstruction::Slti { rd, .. }
        | RiscvInstruction::Sltiu { rd, .. }
        | RiscvInstruction::Xori { rd, .. }
        | RiscvInstruction::Ori { rd, .. }
        | RiscvInstruction::Andi { rd, .. }
        | RiscvInstruction::Slli { rd, .. }
        | RiscvInstruction::Srli { rd, .. }
        | RiscvInstruction::Srai { rd, .. }
        | RiscvInstruction::Addiw { rd, .. }
        | RiscvInstruction::Slliw { rd, .. }
        | RiscvInstruction::Srliw { rd, .. }
        | RiscvInstruction::Sraiw { rd, .. }
        | RiscvInstruction::Add { rd, .. }
        | RiscvInstruction::Sub { rd, .. }
        | RiscvInstruction::Sll { rd, .. }
        | RiscvInstruction::Slt { rd, .. }
        | RiscvInstruction::Sltu { rd, .. }
        | RiscvInstruction::Xor { rd, .. }
        | RiscvInstruction::Srl { rd, .. }
        | RiscvInstruction::Sra { rd, .. }
        | RiscvInstruction::Or { rd, .. }
        | RiscvInstruction::And { rd, .. }
        | RiscvInstruction::Mul { rd, .. }
        | RiscvInstruction::Mulh { rd, .. }
        | RiscvInstruction::Mulhsu { rd, .. }
        | RiscvInstruction::Mulhu { rd, .. }
        | RiscvInstruction::Div { rd, .. }
        | RiscvInstruction::Divu { rd, .. }
        | RiscvInstruction::Rem { rd, .. }
        | RiscvInstruction::Remu { rd, .. }
        | RiscvInstruction::Mulw { rd, .. }
        | RiscvInstruction::Divw { rd, .. }
        | RiscvInstruction::Divuw { rd, .. }
        | RiscvInstruction::Remw { rd, .. }
        | RiscvInstruction::Remuw { rd, .. }
        | RiscvInstruction::Addw { rd, .. }
        | RiscvInstruction::Subw { rd, .. }
        | RiscvInstruction::Sllw { rd, .. }
        | RiscvInstruction::Srlw { rd, .. }
        | RiscvInstruction::Sraw { rd, .. }
        | RiscvInstruction::Jal { rd, .. }
        | RiscvInstruction::Jalr { rd, .. }
        | RiscvInstruction::VectorSetVli { rd, .. }
        | RiscvInstruction::VectorSetIvli { rd, .. }
        | RiscvInstruction::VectorSetVl { rd, .. }
        | RiscvInstruction::Load { rd, .. }
        | RiscvInstruction::LoadReserved { rd, .. }
        | RiscvInstruction::StoreConditional { rd, .. }
        | RiscvInstruction::AtomicMemory { rd, .. } => Some(rd),
        _ => None,
    }
}

pub(crate) fn o3_speculative_scalar_alu_operands(
    instruction: RiscvInstruction,
) -> Option<(Register, Vec<Register>)> {
    if !matches!(
        instruction,
        RiscvInstruction::Lui { .. }
            | RiscvInstruction::Auipc { .. }
            | RiscvInstruction::Addi { .. }
            | RiscvInstruction::Slti { .. }
            | RiscvInstruction::Sltiu { .. }
            | RiscvInstruction::Xori { .. }
            | RiscvInstruction::Ori { .. }
            | RiscvInstruction::Andi { .. }
            | RiscvInstruction::Slli { .. }
            | RiscvInstruction::Srli { .. }
            | RiscvInstruction::Srai { .. }
            | RiscvInstruction::Addiw { .. }
            | RiscvInstruction::Slliw { .. }
            | RiscvInstruction::Srliw { .. }
            | RiscvInstruction::Sraiw { .. }
            | RiscvInstruction::Add { .. }
            | RiscvInstruction::Sub { .. }
            | RiscvInstruction::Sll { .. }
            | RiscvInstruction::Slt { .. }
            | RiscvInstruction::Sltu { .. }
            | RiscvInstruction::Xor { .. }
            | RiscvInstruction::Srl { .. }
            | RiscvInstruction::Sra { .. }
            | RiscvInstruction::Or { .. }
            | RiscvInstruction::And { .. }
            | RiscvInstruction::Mul { .. }
            | RiscvInstruction::Mulh { .. }
            | RiscvInstruction::Mulhsu { .. }
            | RiscvInstruction::Mulhu { .. }
            | RiscvInstruction::Div { .. }
            | RiscvInstruction::Divu { .. }
            | RiscvInstruction::Rem { .. }
            | RiscvInstruction::Remu { .. }
            | RiscvInstruction::Mulw { .. }
            | RiscvInstruction::Divw { .. }
            | RiscvInstruction::Divuw { .. }
            | RiscvInstruction::Remw { .. }
            | RiscvInstruction::Remuw { .. }
            | RiscvInstruction::Addw { .. }
            | RiscvInstruction::Subw { .. }
            | RiscvInstruction::Sllw { .. }
            | RiscvInstruction::Srlw { .. }
            | RiscvInstruction::Sraw { .. }
    ) {
        return None;
    }
    Some((
        o3_scalar_integer_destination(instruction)
            .expect("speculative scalar ALU instructions have an integer destination"),
        o3_scalar_integer_source_registers(&instruction),
    ))
}

pub(crate) fn o3_live_control_operands(
    instruction: RiscvInstruction,
) -> Option<O3LiveControlOperands> {
    let supported_destination = match instruction {
        RiscvInstruction::Beq { .. }
        | RiscvInstruction::Bne { .. }
        | RiscvInstruction::Blt { .. }
        | RiscvInstruction::Bge { .. }
        | RiscvInstruction::Bltu { .. }
        | RiscvInstruction::Bgeu { .. } => Some(None),
        RiscvInstruction::Jal { rd, .. } if rd.is_zero() => Some(None),
        RiscvInstruction::Jal { rd, .. } if is_riscv_link_register(rd) => Some(Some(rd)),
        RiscvInstruction::Jalr { rd, .. } if rd.is_zero() => Some(None),
        RiscvInstruction::Jalr { rd, rs1, .. }
            if is_riscv_link_register(rd)
                && (!is_riscv_link_register(rs1) || rd.index() != rs1.index()) =>
        {
            Some(Some(rd))
        }
        _ => None,
    };
    supported_destination.map(|control_destination| O3LiveControlOperands {
        kind: riscv_branch_target_kind(instruction),
        sources: o3_scalar_integer_source_registers(&instruction),
        destination: control_destination,
    })
}

pub(crate) fn o3_predicted_scalar_descendant_operands(
    instruction: RiscvInstruction,
) -> Option<(Register, Vec<Register>)> {
    let supported = o3_speculative_scalar_alu_operands(instruction).is_some()
        || matches!(
            instruction,
            RiscvInstruction::Mul { .. }
                | RiscvInstruction::Mulh { .. }
                | RiscvInstruction::Mulhsu { .. }
                | RiscvInstruction::Mulhu { .. }
                | RiscvInstruction::Mulw { .. }
        );
    supported.then(|| {
        (
            o3_scalar_integer_destination(instruction)
                .expect("predicted scalar descendant has an integer destination"),
            o3_scalar_integer_source_registers(&instruction),
        )
    })
}

fn csr_operand_register(operand: RiscvCsrOperand) -> Option<Register> {
    match operand {
        RiscvCsrOperand::Register(register) => Some(register),
        RiscvCsrOperand::Immediate(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::Immediate;

    use super::*;

    fn register(index: u8) -> Register {
        Register::new(index).unwrap()
    }

    fn jal(rd: u8) -> RiscvInstruction {
        RiscvInstruction::Jal {
            rd: register(rd),
            offset: Immediate::new(8),
        }
    }

    fn jalr(rd: u8, rs1: u8) -> RiscvInstruction {
        RiscvInstruction::Jalr {
            rd: register(rd),
            rs1: register(rs1),
            offset: Immediate::new(0),
        }
    }

    fn assert_live_control(
        instruction: RiscvInstruction,
        kind: BranchTargetKind,
        sources: &[Register],
        destination: Option<Register>,
    ) {
        let control = o3_live_control_operands(instruction).expect("supported live control");

        assert_eq!(control.kind(), kind);
        assert_eq!(control.sources(), sources);
        assert_eq!(control.destination(), destination);
    }

    #[test]
    fn live_control_descriptor_classifies_supported_jal_forms() {
        assert_live_control(jal(0), BranchTargetKind::DirectUnconditional, &[], None);

        for link in [1, 5] {
            assert_live_control(
                jal(link),
                BranchTargetKind::CallDirect,
                &[],
                Some(register(link)),
            );
        }
    }

    #[test]
    fn live_control_descriptor_classifies_supported_jalr_forms() {
        assert_live_control(
            jalr(0, 9),
            BranchTargetKind::IndirectUnconditional,
            &[register(9)],
            None,
        );

        for link in [1, 5] {
            assert_live_control(
                jalr(link, 9),
                BranchTargetKind::CallIndirect,
                &[register(9)],
                Some(register(link)),
            );
            assert_live_control(
                jalr(0, link),
                BranchTargetKind::Return,
                &[register(link)],
                None,
            );
        }
    }

    #[test]
    fn live_control_descriptor_classifies_coroutine_jalr_forms() {
        for (rd, rs1) in [(5, 1), (1, 5)] {
            assert_live_control(
                jalr(rd, rs1),
                BranchTargetKind::Return,
                &[register(rs1)],
                Some(register(rd)),
            );
        }
    }

    #[test]
    fn live_control_descriptor_support_matches_representative_jalr_matrix() {
        for rd in [0, 1, 2, 5, 9] {
            for rs1 in [0, 1, 2, 5, 9] {
                let rd_is_link = is_riscv_link_register(register(rd));
                let rs1_is_link = is_riscv_link_register(register(rs1));
                let expected_supported = rd == 0 || (rd_is_link && (!rs1_is_link || rd != rs1));

                assert_eq!(
                    o3_live_control_operands(jalr(rd, rs1)).is_some(),
                    expected_supported,
                    "rd=x{rd}, rs1=x{rs1}"
                );
            }
        }
    }

    #[test]
    fn live_control_descriptor_rejects_unsupported_link_forms() {
        for instruction in [jal(2), jalr(2, 9), jalr(2, 1), jalr(1, 1), jalr(5, 5)] {
            assert_eq!(
                o3_live_control_operands(instruction),
                None,
                "{instruction:?}"
            );
        }
    }
}
