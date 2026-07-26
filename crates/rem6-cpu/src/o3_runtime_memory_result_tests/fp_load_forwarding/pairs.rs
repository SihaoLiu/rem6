use rem6_isa_riscv::{RiscvFloatRoundingMode, RiscvVectorScalarMoveInstruction};

use super::*;

fn float_load_event_at(
    pc: u64,
    sequence: u64,
    address: u64,
    destination: u8,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::FloatLoad {
        rd: freg(destination),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
    };
    execution_event(
        pc,
        sequence,
        instruction,
        MemoryAccessKind::FloatLoad {
            rd: freg(destination),
            address,
            width: MemoryWidth::Doubleword,
        },
    )
}

fn scalar_load_event_at(
    pc: u64,
    sequence: u64,
    address: u64,
    destination: u8,
) -> RiscvCpuExecutionEvent {
    execution_event(
        pc,
        sequence,
        load_instruction(destination),
        load_access(destination, address),
    )
}

fn float_mul_d(source: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulD {
        rd: freg(5),
        rs1: freg(source),
        rs2: freg(4),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn addi(source: u8) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(7),
        rs1: reg(source),
        imm: Immediate::new(1),
    }
}

fn vector_move_to_scalar(source: u8) -> RiscvInstruction {
    RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
        rd: reg(7),
        vs2: vreg(source),
    })
}

fn result_pair_runtime(
    first: &RiscvCpuExecutionEvent,
    second: &RiscvCpuExecutionEvent,
) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    for (event, request_sequence, issue_tick) in [(first, 20, 31), (second, 21, 32)] {
        assert!(runtime.stage_live_data_access_issue(
            event,
            request(request_sequence),
            issue_tick,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
    }
    runtime
}

fn stage_pair_suffix(
    first: &RiscvCpuExecutionEvent,
    second: &RiscvCpuExecutionEvent,
    consumer: RiscvInstruction,
) -> usize {
    let mut runtime = result_pair_runtime(first, second);
    runtime.stage_live_data_access_younger_window(
        second.fetch().request_id(),
        [
            (Address::new(0x8008), consumer),
            (Address::new(0x800c), addi(0)),
        ],
    )
}

#[test]
fn memory_result_runtime_pair_blocks_fp_head_and_younger_fp_destinations() {
    for (label, first, second) in [
        (
            "fp head",
            float_load_event_at(0x8000, 1, 0x9000, 3),
            scalar_load_event_at(0x8004, 2, 0x9010, 13),
        ),
        (
            "fp younger",
            scalar_load_event_at(0x8000, 1, 0x9000, 13),
            float_load_event_at(0x8004, 2, 0x9010, 3),
        ),
    ] {
        assert_eq!(
            stage_pair_suffix(&first, &second, float_mul_d(3)),
            1,
            "{label} matching"
        );
        assert_eq!(
            stage_pair_suffix(&first, &second, float_mul_d(6)),
            2,
            "{label} nonmatching"
        );
    }
}

#[test]
fn memory_result_runtime_pair_retains_same_fp_destination_rows_by_sequence() {
    let first = float_load_event_at(0x8000, 1, 0x9000, 3);
    let second = float_load_event_at(0x8004, 2, 0x9010, 3);
    let runtime = result_pair_runtime(&first, &second);
    let sequences = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .filter(|entry| {
            staged_rename_entry(**entry).is_some_and(|destination| {
                destination.register_class() == O3RegisterClass::FloatingPoint
                    && destination.architectural() == 3
            })
        })
        .map(|entry| entry.sequence())
        .collect::<Vec<_>>();

    assert_eq!(sequences.len(), 2);
    assert_ne!(sequences[0], sequences[1]);
    assert_eq!(stage_pair_suffix(&first, &second, float_mul_d(3)), 1);
    assert_eq!(stage_pair_suffix(&first, &second, float_mul_d(6)), 2);
}

#[test]
fn memory_result_runtime_pair_keeps_same_index_fp_and_integer_destinations_distinct() {
    let first = float_load_event_at(0x8000, 1, 0x9000, 3);
    let second = scalar_load_event_at(0x8004, 2, 0x9010, 3);
    let runtime = result_pair_runtime(&first, &second);
    let destinations = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .filter_map(|entry| staged_rename_entry(*entry))
        .map(|entry| (entry.register_class(), entry.architectural()))
        .collect::<Vec<_>>();

    assert_eq!(
        destinations,
        vec![
            (O3RegisterClass::FloatingPoint, 3),
            (O3RegisterClass::Integer, 3),
        ]
    );
    assert_eq!(stage_pair_suffix(&first, &second, float_mul_d(3)), 1);
    assert_eq!(stage_pair_suffix(&first, &second, addi(3)), 1);
}

#[test]
fn memory_result_runtime_pair_rejects_matching_vector_to_scalar_consumer() {
    let first = vector_unit_event(0x8000, 1, 0x9000, None);
    let second = scalar_load_event_at(0x8004, 2, 0x9010, 13);

    assert_eq!(
        stage_pair_suffix(&first, &second, vector_move_to_scalar(2)),
        0
    );
    assert_eq!(
        stage_pair_suffix(&first, &second, vector_move_to_scalar(3)),
        2
    );
}
