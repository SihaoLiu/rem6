use rem6_isa_riscv::{Register, RiscvInstruction};

use crate::{
    o3_runtime::{
        o3_live_control_operands, o3_predicted_scalar_descendant_operands,
        o3_scalar_integer_destination, o3_speculative_scalar_alu_operands,
    },
    o3_runtime_trace::O3RuntimeFuLatencyClass,
    riscv_fu_latency::riscv_o3_fu_latency_class,
    BranchTargetKind, MAX_RISCV_BRANCH_LOOKAHEAD,
};

pub(crate) const O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS: usize = 4;
const MAX_PREDICTED_CONTROL_DEPTH: usize = MAX_RISCV_BRANCH_LOOKAHEAD;

const fn scalar_integer_fu_live_window_head(instruction: RiscvInstruction) -> bool {
    matches!(
        riscv_o3_fu_latency_class(instruction),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ForwardableRasPushKind {
    Call,
    CoroutineReplacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ForwardableRasPush {
    destination: Register,
    sequence: Option<u64>,
    kind: ForwardableRasPushKind,
}

pub(crate) struct RiscvScalarIntegerLiveWindow {
    unresolved_destinations: Vec<Register>,
    live_destinations: Vec<Register>,
    forwardable_ras_push: Option<ForwardableRasPush>,
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
    AdmitPredictedRasControl,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvSequencedScalarIntegerYoungerDecision {
    decision: RiscvScalarIntegerYoungerDecision,
    ras_push_sequence: Option<u64>,
}

impl RiscvSequencedScalarIntegerYoungerDecision {
    pub(crate) const fn decision(self) -> RiscvScalarIntegerYoungerDecision {
        self.decision
    }

    pub(crate) const fn ras_push_sequence(self) -> Option<u64> {
        self.ras_push_sequence
    }
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

    pub(crate) fn from_data_access_result(
        integer_destination: Option<Register>,
        row_limit: usize,
    ) -> Self {
        Self::new(
            integer_destination
                .filter(|destination| !destination.is_zero())
                .into_iter()
                .collect(),
            1,
            row_limit,
            false,
        )
    }

    fn new(
        unresolved_destinations: Vec<Register>,
        rows: usize,
        row_limit: usize,
        admits_terminal_control: bool,
    ) -> Self {
        let live_destinations = unresolved_destinations.clone();
        Self {
            unresolved_destinations,
            live_destinations,
            forwardable_ras_push: None,
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
        self.classify_younger_with_sequence(instruction, None)
            .decision()
    }

    pub(crate) fn classify_sequenced_younger(
        &mut self,
        instruction: RiscvInstruction,
        sequence: u64,
    ) -> RiscvSequencedScalarIntegerYoungerDecision {
        self.classify_younger_with_sequence(instruction, Some(sequence))
    }

    fn classify_younger_with_sequence(
        &mut self,
        instruction: RiscvInstruction,
        instruction_sequence: Option<u64>,
    ) -> RiscvSequencedScalarIntegerYoungerDecision {
        if self.is_full() || self.control_closed {
            return RiscvSequencedScalarIntegerYoungerDecision {
                decision: RiscvScalarIntegerYoungerDecision::Reject,
                ras_push_sequence: None,
            };
        }
        if self.admits_terminal_control {
            let Some(control) = o3_live_control_operands(instruction) else {
                return RiscvSequencedScalarIntegerYoungerDecision {
                    decision: self.classify_scalar_younger(instruction),
                    ras_push_sequence: None,
                };
            };
            if self.control_depth >= MAX_PREDICTED_CONTROL_DEPTH {
                return RiscvSequencedScalarIntegerYoungerDecision {
                    decision: RiscvScalarIntegerYoungerDecision::Reject,
                    ras_push_sequence: None,
                };
            }
            self.rows += 1;
            let depends_on_unresolved = control
                .sources()
                .iter()
                .any(|source| self.unresolved_destinations.contains(source));
            let frontend_sensitive_indirect_target = matches!(
                control.kind(),
                BranchTargetKind::IndirectUnconditional
                    | BranchTargetKind::CallIndirect
                    | BranchTargetKind::Return
            );
            let indirect_target_is_live = frontend_sensitive_indirect_target
                && control
                    .sources()
                    .iter()
                    .any(|source| self.live_destinations.contains(source));
            let consumer_writes_link = control
                .destination()
                .is_some_and(|destination| !destination.is_zero());
            let forwardable_ras_push = (control.kind() == BranchTargetKind::Return
                && control.sources().len() == 1)
                .then(|| self.forwardable_ras_push)
                .flatten()
                .filter(|push| {
                    push.destination == control.sources()[0]
                        && match push.kind {
                            ForwardableRasPushKind::Call => true,
                            ForwardableRasPushKind::CoroutineReplacement => !consumer_writes_link,
                        }
                });
            let forwardable_live_return = forwardable_ras_push.is_some();
            if depends_on_unresolved || (indirect_target_is_live && !forwardable_live_return) {
                self.forwardable_ras_push = None;
                self.control_closed = true;
                return RiscvSequencedScalarIntegerYoungerDecision {
                    decision: RiscvScalarIntegerYoungerDecision::AdmitTerminalControl,
                    ras_push_sequence: None,
                };
            }
            let ras_push_sequence = forwardable_ras_push.and_then(|push| push.sequence);
            let destination = control
                .destination()
                .filter(|destination| !destination.is_zero());
            if let Some(destination) = destination {
                self.record_shadowing_destination(destination);
            }
            match control.kind() {
                BranchTargetKind::CallDirect | BranchTargetKind::CallIndirect => {
                    if let Some(destination) = destination {
                        self.record_forwardable_ras_push(
                            destination,
                            instruction_sequence,
                            ForwardableRasPushKind::Call,
                        );
                    }
                }
                BranchTargetKind::Return if forwardable_live_return => {
                    if let Some(destination) = destination {
                        self.record_forwardable_ras_push(
                            destination,
                            instruction_sequence,
                            ForwardableRasPushKind::CoroutineReplacement,
                        );
                    } else {
                        self.forwardable_ras_push = None;
                    }
                }
                BranchTargetKind::Return => {
                    self.forwardable_ras_push = None;
                }
                _ => {
                    self.forwardable_ras_push = None;
                }
            }
            self.control_depth += 1;
            let decision = if forwardable_live_return {
                RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            } else {
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            };
            return RiscvSequencedScalarIntegerYoungerDecision {
                decision,
                ras_push_sequence,
            };
        }
        RiscvSequencedScalarIntegerYoungerDecision {
            decision: self.classify_scalar_younger(instruction),
            ras_push_sequence: None,
        }
    }

    fn classify_scalar_younger(
        &mut self,
        instruction: RiscvInstruction,
    ) -> RiscvScalarIntegerYoungerDecision {
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
            self.record_shadowing_destination(destination);
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
            self.record_live_destination(destination);
            return RiscvScalarIntegerYoungerDecision::AdmitStop;
        }
        self.record_shadowing_destination(destination);
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    }

    fn record_shadowing_destination(&mut self, destination: Register) {
        self.unresolved_destinations
            .retain(|unresolved| *unresolved != destination);
        if self
            .forwardable_ras_push
            .is_some_and(|push| push.destination == destination)
        {
            self.forwardable_ras_push = None;
        }
        self.record_live_destination(destination);
    }

    fn record_forwardable_ras_push(
        &mut self,
        destination: Register,
        sequence: Option<u64>,
        kind: ForwardableRasPushKind,
    ) {
        self.forwardable_ras_push = Some(ForwardableRasPush {
            destination,
            sequence,
            kind,
        });
    }

    fn record_live_destination(&mut self, destination: Register) {
        if !self.live_destinations.contains(&destination) {
            self.live_destinations.push(destination);
        }
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

    fn jal_with_destination(rd: u8) -> RiscvInstruction {
        RiscvInstruction::Jal {
            rd: Register::new(rd).unwrap(),
            offset: Immediate::new(8),
        }
    }

    fn jalr_with_registers(rd: u8, rs1: u8) -> RiscvInstruction {
        RiscvInstruction::Jalr {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
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
    fn scalar_load_window_admits_independent_div_and_stops_load_dependent_div() {
        let mut independent = scalar_load_window(4);
        assert_eq!(
            independent.classify_younger(div_x3()),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );

        let mut dependent = scalar_load_window(4);
        let load_dependent_div = RiscvInstruction::Div {
            rd: Register::new(3).unwrap(),
            rs1: Register::new(4).unwrap(),
            rs2: Register::new(2).unwrap(),
        };
        assert_eq!(
            dependent.classify_younger(load_dependent_div),
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
    fn scalar_memory_prefix_opens_three_predicted_control_paths() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(bne()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(blt()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(bgeu()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert!(window.is_full());
        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }

    #[test]
    fn scalar_memory_prefix_opens_mixed_no_link_control_paths() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(jal_with_destination(0)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(0, 9)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert!(window.is_full());
    }

    #[test]
    fn live_producer_keeps_no_link_jalr_terminal() {
        let mut load_dependent = scalar_load_window(4);
        assert_eq!(
            load_dependent.classify_younger(jalr_with_registers(0, 4)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );

        let mut alu_dependent = scalar_load_window(4);
        assert_eq!(
            alu_dependent.classify_younger(addi(9, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            alu_dependent.classify_younger(jalr_with_registers(0, 9)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
    }

    #[test]
    fn scalar_memory_prefix_admits_linked_controls_with_committed_targets() {
        for instruction in [
            jal_with_destination(1),
            jal_with_destination(5),
            jalr_with_registers(1, 9),
            jalr_with_registers(5, 9),
            jalr_with_registers(0, 1),
            jalr_with_registers(0, 5),
        ] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl,
                "{instruction:?}"
            );
        }
    }

    #[test]
    fn scalar_memory_prefix_admits_coroutines_with_committed_targets() {
        for (destination, source) in [(5, 1), (1, 5)] {
            let mut window = scalar_load_window(4);
            assert_eq!(
                window.classify_younger(jalr_with_registers(destination, source)),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );
            assert_eq!(
                window.classify_younger(addi(8, destination)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue
            );
        }
    }

    #[test]
    fn linked_control_destination_shadows_unresolved_load_for_descendants() {
        for (destination, instruction) in [
            (1, jal_with_destination(1)),
            (5, jal_with_destination(5)),
            (1, jalr_with_registers(1, 9)),
            (5, jalr_with_registers(5, 9)),
        ] {
            let mut window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
                [Register::new(destination).unwrap()],
                1,
                4,
            )
            .unwrap();

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl,
                "{instruction:?}"
            );
            assert_eq!(
                window.classify_younger(addi(8, destination)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue,
                "{instruction:?}"
            );
        }
    }

    #[test]
    fn frontend_sensitive_indirect_controls_remain_terminal_when_target_is_unresolved() {
        for instruction in [jalr_with_registers(0, 4), jalr_with_registers(1, 4)] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::AdmitTerminalControl,
                "{instruction:?}"
            );
        }

        let mut return_window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(1).unwrap()],
            1,
            4,
        )
        .unwrap();
        assert_eq!(
            return_window.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
    }

    #[test]
    fn frontend_sensitive_indirect_controls_remain_terminal_when_target_is_live() {
        for instruction in [jalr_with_registers(0, 9), jalr_with_registers(1, 9)] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(addi(9, 0)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue
            );
            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::AdmitTerminalControl,
                "{instruction:?}"
            );
        }

        let mut return_window = scalar_load_window(4);
        assert_eq!(
            return_window.classify_younger(addi(1, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            return_window.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
    }

    #[test]
    fn same_window_link_return_requires_ras_prediction() {
        for (call, return_jump) in [
            (jal_with_destination(1), jalr_with_registers(0, 1)),
            (jalr_with_registers(5, 9), jalr_with_registers(0, 5)),
        ] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(call),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl,
                "{call:?}"
            );
            assert_eq!(
                window.classify_younger(return_jump),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl,
                "{return_jump:?}"
            );
            assert_eq!(
                window.classify_younger(addi(8, 0)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue
            );
            assert!(window.is_full());
        }
    }

    #[test]
    fn same_window_coroutine_requires_exact_call_ras_prediction() {
        for (call, coroutine, destination) in [
            (jal_with_destination(1), jalr_with_registers(5, 1), 5),
            (jalr_with_registers(5, 9), jalr_with_registers(1, 5), 1),
        ] {
            let mut window = scalar_load_window(4);
            assert_eq!(
                window.classify_sequenced_younger(call, 51).decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );
            let coroutine = window.classify_sequenced_younger(coroutine, 52);
            assert_eq!(
                coroutine.decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            );
            assert_eq!(coroutine.ras_push_sequence(), Some(51));
            assert_eq!(
                window.classify_younger(addi(8, destination)),
                RiscvScalarIntegerYoungerDecision::AdmitContinue
            );
            assert!(window.is_full());
        }
    }

    #[test]
    fn same_window_link_return_consumes_forwardable_provenance() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(jal_with_destination(1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert!(window.is_full());
    }

    #[test]
    fn same_window_return_must_match_latest_link_call() {
        let mut older_link = scalar_load_window(4);

        assert_eq!(
            older_link.classify_younger(jal_with_destination(1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            older_link.classify_younger(jal_with_destination(5)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            older_link.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert!(older_link.is_full());

        let mut latest_link = scalar_load_window(4);
        assert_eq!(
            latest_link.classify_younger(jal_with_destination(1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            latest_link.classify_younger(jal_with_destination(5)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            latest_link.classify_younger(jalr_with_registers(0, 5)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert!(latest_link.is_full());
    }

    #[test]
    fn sequenced_same_window_return_reports_latest_call_push() {
        for (older, latest) in [(1, 5), (5, 1)] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window
                    .classify_sequenced_younger(jal_with_destination(older), 41)
                    .decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );
            assert_eq!(
                window
                    .classify_sequenced_younger(jal_with_destination(latest), 42)
                    .decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );
            let return_decision =
                window.classify_sequenced_younger(jalr_with_registers(0, latest), 43);
            assert_eq!(
                return_decision.decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            );
            assert_eq!(return_decision.ras_push_sequence(), Some(42));
        }
    }

    #[test]
    fn unrelated_scalar_write_preserves_sequenced_call_owner() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window
                .classify_sequenced_younger(jal_with_destination(1), 51)
                .decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_sequenced_younger(addi(9, 0), 52).decision(),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        let return_decision = window.classify_sequenced_younger(jalr_with_registers(0, 1), 53);
        assert_eq!(
            return_decision.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(return_decision.ras_push_sequence(), Some(51));
    }

    #[test]
    fn same_window_admitted_return_consumes_pending_link_owner() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(jal_with_destination(1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(0, 5)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert!(window.is_full());
    }

    #[test]
    fn scalar_overwrite_after_call_keeps_same_window_return_terminal() {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(jal_with_destination(1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(addi(1, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(0, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert!(window.is_full());
    }

    #[test]
    fn overwritten_coroutine_source_remains_terminal() {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(jal_with_destination(1)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(addi(1, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(jalr_with_registers(5, 1)),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert!(window.is_full());
    }

    #[test]
    fn admitted_coroutine_publishes_replacement_push_to_one_adjacent_return() {
        for (call, coroutine, return_jump, expected_destination) in [
            (
                jal_with_destination(1),
                jalr_with_registers(5, 1),
                jalr_with_registers(0, 5),
                5,
            ),
            (
                jalr_with_registers(5, 9),
                jalr_with_registers(1, 5),
                jalr_with_registers(0, 1),
                1,
            ),
        ] {
            let mut window = scalar_load_window(4);
            let call = window.classify_sequenced_younger(call, 51);
            assert_eq!(
                call.decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            );

            let coroutine = window.classify_sequenced_younger(coroutine, 52);
            assert_eq!(
                coroutine.decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            );
            assert_eq!(coroutine.ras_push_sequence(), Some(51));
            assert_eq!(
                window.forwardable_ras_push,
                Some(ForwardableRasPush {
                    destination: Register::new(expected_destination).unwrap(),
                    sequence: Some(52),
                    kind: ForwardableRasPushKind::CoroutineReplacement,
                })
            );

            let return_jump = window.classify_sequenced_younger(return_jump, 53);
            assert_eq!(
                return_jump.decision(),
                RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            );
            assert_eq!(return_jump.ras_push_sequence(), Some(52));
            assert_eq!(window.forwardable_ras_push, None);
            assert!(window.is_full());
        }
    }

    #[test]
    fn committed_source_coroutine_does_not_publish_replacement() {
        let mut window = scalar_load_window(4);
        let coroutine = window.classify_sequenced_younger(jalr_with_registers(1, 5), 52);
        assert_eq!(
            coroutine.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(coroutine.ras_push_sequence(), None);
        let return_jump = window.classify_sequenced_younger(jalr_with_registers(0, 1), 53);
        assert_eq!(
            return_jump.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert_eq!(return_jump.ras_push_sequence(), None);
        assert_eq!(window.forwardable_ras_push, None);
    }

    #[test]
    fn intervening_control_clears_coroutine_replacement() {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window
                .classify_sequenced_younger(jal_with_destination(1), 51)
                .decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window
                .classify_sequenced_younger(jalr_with_registers(5, 1), 52)
                .decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(
            window
                .classify_sequenced_younger(jalr_with_registers(0, 9), 53)
                .decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(window.forwardable_ras_push, None);
    }

    #[test]
    fn coroutine_replacement_rejects_linked_consumer() {
        let mut window = scalar_load_window(4);
        window.classify_sequenced_younger(jal_with_destination(1), 51);
        window.classify_sequenced_younger(jalr_with_registers(5, 1), 52);
        let linked_consumer = window.classify_sequenced_younger(jalr_with_registers(1, 5), 53);
        assert_eq!(
            linked_consumer.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert_eq!(linked_consumer.ras_push_sequence(), None);
        assert_eq!(window.forwardable_ras_push, None);
    }

    #[test]
    fn ordinary_return_consumes_coroutine_replacement_once() {
        let mut window = scalar_load_window(4);
        window.classify_sequenced_younger(jal_with_destination(1), 51);
        window.classify_sequenced_younger(jalr_with_registers(5, 1), 52);
        let return_jump = window.classify_sequenced_younger(jalr_with_registers(0, 5), 53);
        assert_eq!(
            return_jump.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(return_jump.ras_push_sequence(), Some(52));
        assert_eq!(window.forwardable_ras_push, None);
    }

    #[test]
    fn scalar_memory_prefix_rejects_unsupported_link_forms() {
        for instruction in [
            jal_with_destination(2),
            jalr_with_registers(2, 9),
            jalr_with_registers(2, 1),
            jalr_with_registers(2, 5),
            jalr_with_registers(1, 1),
            jalr_with_registers(5, 5),
        ] {
            let mut window = scalar_load_window(4);

            assert_eq!(
                window.classify_younger(instruction),
                RiscvScalarIntegerYoungerDecision::Reject,
                "{instruction:?}"
            );
        }
    }

    #[test]
    fn nested_control_rejects_unsupported_rows_after_two_controls() {
        for instruction in [
            scalar_load(),
            jal_with_destination(2),
            RiscvInstruction::Ecall,
        ] {
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
        for instruction in [
            jal_with_destination(2),
            jalr_with_registers(2, 1),
            jalr_with_registers(2, 5),
            jalr_with_registers(1, 1),
            jalr_with_registers(5, 5),
            RiscvInstruction::Ecall,
            scalar_load(),
        ] {
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
