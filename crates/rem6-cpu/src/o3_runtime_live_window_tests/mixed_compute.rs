use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatRoundingMode, RiscvInstruction,
    RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::*;

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn v(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn float_add_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatAddS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn vector_move_to_scalar(rd: u8, vs2: u8) -> RiscvInstruction {
    RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
        rd: Register::new(rd).unwrap(),
        vs2: v(vs2),
    })
}

#[test]
fn stage_live_instruction_tracks_mixed_compute_destinations() {
    let mut runtime = O3RuntimeState::default();
    runtime.publish_live_rename_entry(O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        10,
        O3PhysicalRegisterId::new(40),
    ));
    runtime.next_physical_register = 41;

    assert!(runtime
        .stage_live_instruction(Address::new(0x8000), div_x3(), 29)
        .is_some());
    assert!(runtime
        .stage_live_instruction(Address::new(0x8004), float_add_s(4, 1, 2), 7)
        .is_some());
    assert!(runtime
        .stage_live_instruction(Address::new(0x8008), vector_move_to_scalar(11, 3), 8)
        .is_some());
    assert!(runtime
        .stage_live_instruction(Address::new(0x800c), addi(0, 1), 9)
        .is_some());

    let snapshot = runtime.snapshot();
    let rob = snapshot.reorder_buffer();
    assert_eq!(
        rob[1].rename_destination(),
        Some((O3RegisterClass::FloatingPoint, 4))
    );
    assert!(rob[1].destination().is_some());
    assert_eq!(
        rob[2].rename_destination(),
        Some((O3RegisterClass::Integer, 11))
    );
    assert!(rob[2].destination().is_some());
    assert_eq!(rob[3].rename_destination(), None);
    assert_eq!(rob[3].destination(), None);
    assert_eq!(
        runtime
            .snapshot()
            .rename_map()
            .iter()
            .map(|entry| (entry.register_class(), entry.architectural()))
            .collect::<Vec<_>>(),
        vec![
            (O3RegisterClass::Integer, 3),
            (O3RegisterClass::Integer, 10),
            (O3RegisterClass::Integer, 11),
            (O3RegisterClass::FloatingPoint, 4),
        ]
    );
}
