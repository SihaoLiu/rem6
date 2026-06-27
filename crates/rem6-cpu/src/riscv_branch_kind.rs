use rem6_isa_riscv::{Register, RiscvInstruction};

use crate::BranchTargetKind;

pub(crate) const fn riscv_branch_target_kind(instruction: RiscvInstruction) -> BranchTargetKind {
    match instruction {
        RiscvInstruction::Beq { .. }
        | RiscvInstruction::Bne { .. }
        | RiscvInstruction::Blt { .. }
        | RiscvInstruction::Bge { .. }
        | RiscvInstruction::Bltu { .. }
        | RiscvInstruction::Bgeu { .. } => BranchTargetKind::DirectConditional,
        RiscvInstruction::Jal { rd, .. } => {
            if is_riscv_link_register(rd) {
                BranchTargetKind::CallDirect
            } else {
                BranchTargetKind::DirectUnconditional
            }
        }
        RiscvInstruction::Jalr { rd, rs1, .. } => {
            let rd_link = is_riscv_link_register(rd);
            let rs1_link = is_riscv_link_register(rs1);
            if (!rd_link && rs1_link) || (rd_link && rs1_link && rd.index() != rs1.index()) {
                BranchTargetKind::Return
            } else if rd_link {
                BranchTargetKind::CallIndirect
            } else {
                BranchTargetKind::IndirectUnconditional
            }
        }
        _ => BranchTargetKind::NoBranch,
    }
}

pub(crate) const fn is_riscv_link_register(register: Register) -> bool {
    matches!(register.index(), 1 | 5)
}
