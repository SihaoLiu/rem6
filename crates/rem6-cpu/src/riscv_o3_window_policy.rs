use rem6_isa_riscv::{Register, RiscvInstruction};

use crate::{
    o3_runtime::{
        o3_direct_conditional_sources, o3_predicted_scalar_descendant_operands,
        o3_scalar_integer_destination, o3_speculative_scalar_alu_operands,
    },
    o3_runtime_trace::O3RuntimeFuLatencyClass,
    riscv_fu_latency::riscv_o3_fu_latency_class,
};

pub(crate) const O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS: usize = 4;
const MAX_PREDICTED_CONTROL_DEPTH: usize = 2;

const fn scalar_integer_fu_live_window_head(instruction: RiscvInstruction) -> bool {
    matches!(
        riscv_o3_fu_latency_class(instruction),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    )
}

const fn scalar_integer_terminal_control(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Beq { .. }
            | RiscvInstruction::Bne { .. }
            | RiscvInstruction::Blt { .. }
            | RiscvInstruction::Bge { .. }
            | RiscvInstruction::Bltu { .. }
            | RiscvInstruction::Bgeu { .. }
    )
}

pub(crate) struct RiscvScalarIntegerLiveWindow {
    unresolved_destinations: Vec<Register>,
    rows: usize,
    row_limit: usize,
    admits_terminal_control: bool,
    control_depth: usize,
    control_closed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvScalarIntegerYoungerDecision {
    AdmitContinue,
    AdmitStop,
    AdmitTerminalControl,
    AdmitPredictedControl,
    Reject,
}

impl RiscvScalarIntegerLiveWindow {
    pub(crate) fn from_fu_head(head: RiscvInstruction) -> Option<Self> {
        scalar_integer_fu_live_window_head(head).then(|| {
            Self::new(
                o3_scalar_integer_destination(head)
                    .filter(|destination| !destination.is_zero())
                    .into_iter()
                    .collect(),
                1,
                O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
                false,
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
            .then(|| Self::new(unresolved_destinations, occupied_rows, row_limit, true))
    }

    fn new(
        unresolved_destinations: Vec<Register>,
        rows: usize,
        row_limit: usize,
        admits_terminal_control: bool,
    ) -> Self {
        Self {
            unresolved_destinations,
            rows,
            row_limit: row_limit.clamp(1, O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS),
            admits_terminal_control,
            control_depth: 0,
            control_closed: false,
        }
    }

    pub(crate) fn is_full(&self) -> bool {
        self.rows >= self.row_limit
    }

    pub(crate) fn classify_younger(
        &mut self,
        instruction: RiscvInstruction,
    ) -> RiscvScalarIntegerYoungerDecision {
        if self.is_full() || self.control_closed {
            return RiscvScalarIntegerYoungerDecision::Reject;
        }
        if self.admits_terminal_control && scalar_integer_terminal_control(instruction) {
            if self.control_depth >= MAX_PREDICTED_CONTROL_DEPTH {
                return RiscvScalarIntegerYoungerDecision::Reject;
            }
            let sources = o3_direct_conditional_sources(instruction)
                .expect("terminal scalar control has direct conditional sources");
            self.rows += 1;
            if sources
                .iter()
                .any(|source| self.unresolved_destinations.contains(source))
            {
                self.control_closed = true;
                return RiscvScalarIntegerYoungerDecision::AdmitTerminalControl;
            }
            self.control_depth += 1;
            return RiscvScalarIntegerYoungerDecision::AdmitPredictedControl;
        }
        if self.control_depth > 0 {
            let Some((destination, sources)) = o3_predicted_scalar_descendant_operands(instruction)
            else {
                return RiscvScalarIntegerYoungerDecision::Reject;
            };
            if destination.is_zero()
                || sources
                    .iter()
                    .any(|source| self.unresolved_destinations.contains(source))
            {
                return RiscvScalarIntegerYoungerDecision::Reject;
            }
            self.rows += 1;
            self.unresolved_destinations
                .retain(|unresolved| *unresolved != destination);
            return RiscvScalarIntegerYoungerDecision::AdmitContinue;
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
    use rem6_isa_riscv::{Immediate, MemoryWidth};

    use super::*;

    fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
        RiscvInstruction::Addi {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            imm: Immediate::new(1),
        }
    }

    fn div_x3() -> RiscvInstruction {
        RiscvInstruction::Div {
            rd: Register::new(3).unwrap(),
            rs1: Register::new(1).unwrap(),
            rs2: Register::new(2).unwrap(),
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

    fn beq() -> RiscvInstruction {
        RiscvInstruction::Beq {
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn bne() -> RiscvInstruction {
        RiscvInstruction::Bne {
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn blt() -> RiscvInstruction {
        RiscvInstruction::Blt {
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn bge() -> RiscvInstruction {
        RiscvInstruction::Bge {
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn bltu() -> RiscvInstruction {
        RiscvInstruction::Bltu {
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn bgeu() -> RiscvInstruction {
        RiscvInstruction::Bgeu {
            rs1: Register::new(5).unwrap(),
            rs2: Register::new(6).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn beq_with_sources(rs1: u8, rs2: u8) -> RiscvInstruction {
        RiscvInstruction::Beq {
            rs1: Register::new(rs1).unwrap(),
            rs2: Register::new(rs2).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn mul(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
        RiscvInstruction::Mul {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            rs2: Register::new(rs2).unwrap(),
        }
    }

    fn jal() -> RiscvInstruction {
        RiscvInstruction::Jal {
            rd: Register::new(1).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn jalr() -> RiscvInstruction {
        RiscvInstruction::Jalr {
            rd: Register::new(1).unwrap(),
            rs1: Register::new(5).unwrap(),
            offset: Immediate::new(0),
        }
    }

    fn scalar_load() -> RiscvInstruction {
        RiscvInstruction::Load {
            rd: Register::new(7).unwrap(),
            rs1: Register::new(5).unwrap(),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        }
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
    fn scalar_memory_prefix_opens_direct_conditional_predicted_control() {
        for branch in [beq(), bne(), blt(), bge(), bltu(), bgeu()] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(addi(5, 0)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue
            );
            assert_eq!(
                window.classify_younger(addi(6, 0)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue
            );
            assert_eq!(
                window.classify_younger(branch),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl,
                "{branch:?} should open the predicted control path"
            );
            assert!(
                window.is_full(),
                "{branch:?} should fill the four-row window"
            );
        }
    }

    #[test]
    fn independent_branch_opens_one_predicted_control_path() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(mul(7, 5, 6)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(addi(8, 7)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert!(window.is_full());
    }

    #[test]
    fn scalar_memory_prefix_opens_two_predicted_control_paths() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(bne()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(mul(7, 5, 6)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert!(window.is_full());
    }

    #[test]
    fn load_dependent_branch_remains_terminal() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(beq_with_sources(4, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert_eq!(
            window.classify_younger(addi(8, 0)),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }

    #[test]
    fn load_dependent_inner_branch_remains_terminal() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(beq_with_sources(4, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert_eq!(
            window.classify_younger(addi(8, 0)),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }

    #[test]
    fn nested_control_rejects_third_branch_and_memory_descendants() {
        for instruction in [beq(), scalar_load(), jal(), RiscvInstruction::Ecall] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(beq()),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );
            assert_eq!(
                window.classify_younger(bne()),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );
            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::Reject
            );
        }
    }

    #[test]
    fn scalar_fu_head_rejects_direct_conditional_terminal_control() {
        let mut window = RiscvScalarIntegerLiveWindow::from_fu_head(div_x3()).unwrap();

        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }

    #[test]
    fn scalar_memory_prefix_rejects_unsupported_control_and_memory_rows() {
        for instruction in [jal(), jalr(), RiscvInstruction::Ecall, scalar_load()] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::Reject
            );
        }
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
