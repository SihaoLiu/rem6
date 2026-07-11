use rem6_isa_riscv::{Register, RiscvInstruction};

use crate::{
    o3_fu_latency::{o3_scalar_integer_fu_live_window_head, O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS},
    o3_runtime::{o3_scalar_integer_destination, o3_speculative_scalar_alu_operands},
};

pub(crate) struct RiscvScalarIntegerLiveWindow {
    head_destination: Option<Register>,
    head_destination_shadowed: bool,
    rows: usize,
    row_limit: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvScalarIntegerYoungerDecision {
    AdmitContinue,
    AdmitStop,
    Reject,
}

impl RiscvScalarIntegerLiveWindow {
    pub(crate) fn from_fu_head(head: RiscvInstruction) -> Option<Self> {
        o3_scalar_integer_fu_live_window_head(head).then(|| {
            Self::new(
                o3_scalar_integer_destination(head).filter(|destination| !destination.is_zero()),
                O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
            )
        })
    }

    pub(crate) fn from_scalar_load_head(head: RiscvInstruction, row_limit: usize) -> Option<Self> {
        let RiscvInstruction::Load { rd, .. } = head else {
            return None;
        };
        (!rd.is_zero()).then(|| Self::new(Some(rd), row_limit))
    }

    fn new(head_destination: Option<Register>, row_limit: usize) -> Self {
        Self {
            head_destination,
            head_destination_shadowed: false,
            rows: 1,
            row_limit: row_limit.clamp(1, O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS),
        }
    }

    pub(crate) fn is_full(&self) -> bool {
        self.rows >= self.row_limit
    }

    pub(crate) fn classify_younger(
        &mut self,
        instruction: RiscvInstruction,
    ) -> RiscvScalarIntegerYoungerDecision {
        if self.is_full() {
            return RiscvScalarIntegerYoungerDecision::Reject;
        }
        let Some((destination, sources)) = o3_speculative_scalar_alu_operands(instruction) else {
            return RiscvScalarIntegerYoungerDecision::Reject;
        };
        if destination.is_zero() {
            return RiscvScalarIntegerYoungerDecision::Reject;
        }
        let depends_on_unshadowed_head = self
            .head_destination
            .is_some_and(|head| !self.head_destination_shadowed && sources.contains(&head));
        self.rows += 1;
        if depends_on_unshadowed_head {
            return RiscvScalarIntegerYoungerDecision::AdmitStop;
        }
        if self.head_destination == Some(destination) {
            self.head_destination_shadowed = true;
        }
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{Immediate, MemoryWidth};

    use super::*;

    fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
        RiscvInstruction::Addi {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            imm: Immediate::new(1),
        }
    }

    fn load_x4() -> RiscvInstruction {
        RiscvInstruction::Load {
            rd: Register::new(4).unwrap(),
            rs1: Register::new(10).unwrap(),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        }
    }

    #[test]
    fn load_head_dependency_is_admitted_as_the_terminal_row() {
        let mut window = RiscvScalarIntegerLiveWindow::from_scalar_load_head(load_x4(), 4).unwrap();

        assert_eq!(
            window.classify_younger(addi(5, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(addi(6, 4)),
            RiscvScalarIntegerYoungerDecision::AdmitStop
        );
    }

    #[test]
    fn younger_write_can_shadow_the_load_destination() {
        let mut window = RiscvScalarIntegerLiveWindow::from_scalar_load_head(load_x4(), 4).unwrap();

        assert_eq!(
            window.classify_younger(addi(4, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(addi(5, 4)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
    }

    #[test]
    fn configured_row_limit_is_a_total_window_limit() {
        let mut window = RiscvScalarIntegerLiveWindow::from_scalar_load_head(load_x4(), 2).unwrap();

        assert_eq!(
            window.classify_younger(addi(5, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert!(window.is_full());
        assert_eq!(
            window.classify_younger(addi(6, 5)),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }

    #[test]
    fn unsupported_and_zero_destination_rows_are_rejected() {
        for instruction in [
            RiscvInstruction::Ecall,
            RiscvInstruction::Addi {
                rd: Register::new(0).unwrap(),
                rs1: Register::new(0).unwrap(),
                imm: Immediate::new(0),
            },
        ] {
            let mut window =
                RiscvScalarIntegerLiveWindow::from_scalar_load_head(load_x4(), 4).unwrap();

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::Reject
            );
        }
    }
}
