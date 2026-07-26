use rem6_isa_riscv::{FloatRegister, RiscvFloatRoundingMode, RiscvInstruction};

use super::*;
use crate::o3_dependency::O3RegisterClass;

struct TypedForwardingFixture {
    runtime: O3RuntimeState,
}

impl TypedForwardingFixture {
    fn new() -> Self {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.set_issue_width(4));
        Self { runtime }
    }

    fn stage(&mut self, pc: u64, instruction: RiscvInstruction, request_sequence: u64) -> u64 {
        let sequence = self
            .runtime
            .stage_live_instruction(Address::new(pc), instruction, 0)
            .unwrap();
        assert!(self.runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            typed_decoded(instruction),
            &[request(request_sequence)],
            20,
        ));
        sequence
    }

    fn queue(&self) -> O3LiveIssueQueue {
        super::materialized_queue(&self.runtime)
    }
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn float_add_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatAddS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn float_mul_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn typed_decoded(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let raw = match instruction {
        RiscvInstruction::FloatAddS { rd, rs1, rs2, .. } => {
            fp_raw(0, rs2.index(), rs1.index(), rd.index())
        }
        RiscvInstruction::FloatMulS { rd, rs1, rs2, .. } => {
            fp_raw(0b0001000, rs2.index(), rs1.index(), rd.index())
        }
        _ => panic!("typed forwarding fixture received unsupported instruction"),
    };
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x53
}

#[test]
fn typed_live_forwarding_discovers_fp_source_producer() {
    let mut fixture = TypedForwardingFixture::new();
    let producer = fixture.stage(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let consumer_instruction = float_mul_s(5, 4, 3);
    let consumer = fixture.stage(SECOND_PC, consumer_instruction, 12);
    let queue = fixture.queue();
    let producers = queue.entry(consumer).unwrap().scheduling().data_producers();

    assert_eq!(producers.len(), 1);
    assert_eq!(producers[0].sequence(), producer);
    assert_eq!(
        producers[0].source(),
        O3ArchitecturalRegister::floating_point(f(4)),
    );
    assert!(fixture
        .runtime
        .live_speculative_issue_candidate(Address::new(SECOND_PC), consumer_instruction)
        .is_none());
}

#[test]
fn typed_live_forwarding_selects_nearest_fp_waw_producer() {
    let mut fixture = TypedForwardingFixture::new();
    let older = fixture.stage(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let nearest = fixture.stage(SECOND_PC, float_add_s(4, 2, 3), 12);
    let consumer = fixture.stage(THIRD_PC, float_mul_s(5, 4, 3), 13);
    let queue = fixture.queue();
    let producers = queue.entry(consumer).unwrap().scheduling().data_producers();

    assert_ne!(older, nearest);
    assert_eq!(producers.len(), 1);
    assert_eq!(producers[0].sequence(), nearest);
}

#[test]
fn typed_live_forwarding_keeps_two_fp_fanin_producers() {
    let mut fixture = TypedForwardingFixture::new();
    let left = fixture.stage(BRANCH_PC, float_add_s(4, 1, 2), 11);
    let right = fixture.stage(SECOND_PC, float_add_s(6, 2, 3), 12);
    let consumer = fixture.stage(THIRD_PC, float_mul_s(5, 4, 6), 13);
    let queue = fixture.queue();

    assert_eq!(
        queue
            .entry(consumer)
            .unwrap()
            .scheduling()
            .data_producers()
            .iter()
            .map(|producer| producer.sequence())
            .collect::<Vec<_>>(),
        [left, right],
    );
}

#[test]
fn typed_live_forwarding_filters_integer_x0_without_filtering_fp_f0() {
    let mut integer_fixture = TypedForwardingFixture::new();
    let zero_producer_instruction = addi(0, 1, 1);
    integer_fixture
        .runtime
        .stage_live_instruction_with_rename_destination(
            Address::new(BRANCH_PC),
            zero_producer_instruction,
            0,
            Some((O3RegisterClass::Integer, 0)),
        )
        .unwrap();
    assert!(integer_fixture.runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(zero_producer_instruction),
        &[request(11)],
        20,
    ));
    let integer_consumer = integer_fixture
        .runtime
        .stage_live_instruction(Address::new(SECOND_PC), addi(5, 0, 1), 0)
        .unwrap();
    assert!(integer_fixture.runtime.bind_live_staged_issue_packet(
        Address::new(SECOND_PC),
        decoded(addi(5, 0, 1)),
        &[request(12)],
        20,
    ));

    assert!(integer_fixture
        .queue()
        .entry(integer_consumer)
        .unwrap()
        .scheduling()
        .data_producers()
        .is_empty());

    let mut floating_point_fixture = TypedForwardingFixture::new();
    let producer = floating_point_fixture.stage(BRANCH_PC, float_add_s(0, 1, 2), 11);
    let consumer = floating_point_fixture.stage(SECOND_PC, float_mul_s(5, 0, 3), 12);
    let queue = floating_point_fixture.queue();
    let producers = queue.entry(consumer).unwrap().scheduling().data_producers();

    assert_eq!(producers.len(), 1);
    assert_eq!(producers[0].sequence(), producer);
    assert_eq!(
        producers[0].source(),
        O3ArchitecturalRegister::floating_point(f(0)),
    );
}
