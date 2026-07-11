use rem6_isa_riscv::{Register, RiscvInstruction};

use crate::{
    o3_fu_latency::{o3_scalar_integer_fu_live_window_head, O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS},
    o3_runtime::{o3_scalar_integer_destination, o3_speculative_scalar_alu_operands},
};

pub(crate) struct RiscvScalarIntegerLiveWindow {
    unresolved_destinations: Vec<Register>,
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
                o3_scalar_integer_destination(head)
                    .filter(|destination| !destination.is_zero())
                    .into_iter()
                    .collect(),
                1,
                O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
            )
        })
    }

    pub(crate) fn from_scalar_memory_prefix(
        load_destinations: impl IntoIterator<Item = Register>,
        occupied_rows: usize,
        row_limit: usize,
    ) -> Option<Self> {
        let row_limit = row_limit.clamp(1, O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS);
        if occupied_rows == 0 || occupied_rows > row_limit {
            return None;
        }
        let mut unresolved_destinations = Vec::new();
        for destination in load_destinations
            .into_iter()
            .filter(|destination| !destination.is_zero())
        {
            if !unresolved_destinations.contains(&destination) {
                unresolved_destinations.push(destination);
            }
        }
        (!unresolved_destinations.is_empty())
            .then(|| Self::new(unresolved_destinations, occupied_rows, row_limit))
    }

    fn new(unresolved_destinations: Vec<Register>, rows: usize, row_limit: usize) -> Self {
        Self {
            unresolved_destinations,
            rows,
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
        let depends_on_unresolved_destination = sources
            .iter()
            .any(|source| self.unresolved_destinations.contains(source));
        self.rows += 1;
        if depends_on_unresolved_destination {
            return RiscvScalarIntegerYoungerDecision::AdmitStop;
        }
        self.unresolved_destinations
            .retain(|unresolved| *unresolved != destination);
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::Immediate;

    use super::*;

    fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
        RiscvInstruction::Addi {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            imm: Immediate::new(1),
        }
    }

    fn scalar_load_window(row_limit: usize) -> RiscvScalarIntegerLiveWindow {
        RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(4).unwrap()],
            1,
            row_limit,
        )
        .unwrap()
    }

    #[test]
    fn load_head_dependency_is_admitted_as_the_terminal_row() {
        let mut window = scalar_load_window(4);

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
        let mut window = scalar_load_window(4);

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
        let mut window = scalar_load_window(2);

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
    fn scalar_memory_prefix_protects_every_load_destination() {
        for source in [4, 5] {
            let mut window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
                [Register::new(4).unwrap(), Register::new(5).unwrap()],
                2,
                4,
            )
            .unwrap();

            assert_eq!(
                window.classify_younger(addi(6, source)),
                RiscvScalarIntegerYoungerDecision::AdmitStop
            );
        }
    }

    #[test]
    fn scalar_memory_prefix_shadowing_is_destination_specific() {
        let mut window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(4).unwrap(), Register::new(5).unwrap()],
            2,
            4,
        )
        .unwrap();

        assert_eq!(
            window.classify_younger(addi(4, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(addi(6, 5)),
            RiscvScalarIntegerYoungerDecision::AdmitStop
        );
    }

    #[test]
    fn scalar_memory_prefix_rows_count_toward_total_depth() {
        let mut window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(4).unwrap(), Register::new(5).unwrap()],
            3,
            4,
        )
        .unwrap();

        assert_eq!(
            window.classify_younger(addi(6, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert!(window.is_full());
        assert_eq!(
            window.classify_younger(addi(7, 0)),
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
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::Reject
            );
        }
    }
}
