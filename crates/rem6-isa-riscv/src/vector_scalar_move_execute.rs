use crate::{
    vector_group::lane_bytes_to_u64, write_register, Register, RegisterWrite, RiscvHartState,
    RiscvVectorScalarMoveInstruction, VectorRegister,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    register_writes: &mut Vec<RegisterWrite>,
    instruction: RiscvVectorScalarMoveInstruction,
) -> bool {
    match instruction {
        RiscvVectorScalarMoveInstruction::MoveToScalar { rd, vs2 } => {
            execute_move_to_scalar(hart, register_writes, rd, vs2)
        }
        RiscvVectorScalarMoveInstruction::MoveFromScalar { vd, rs1 } => {
            execute_move_from_scalar(hart, vd, hart.read(rs1))
        }
    }
}

fn execute_move_to_scalar(
    hart: &mut RiscvHartState,
    register_writes: &mut Vec<RegisterWrite>,
    rd: Register,
    vs2: VectorRegister,
) -> bool {
    let Some(element_bytes) = hart.vector_config().element_width_bytes() else {
        return false;
    };

    let source = hart.read_vector(vs2);
    let value = sign_extend_lane(lane_bytes_to_u64(&source[..element_bytes]), element_bytes);
    write_register(hart, register_writes, rd, value);
    true
}

fn execute_move_from_scalar(hart: &mut RiscvHartState, vd: VectorRegister, scalar: u64) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    if config.vl() == 0 {
        return true;
    }

    let mut destination = hart.read_vector(vd);
    destination[..element_bytes].copy_from_slice(&scalar.to_le_bytes()[..element_bytes]);
    hart.write_vector(vd, destination);
    true
}

fn sign_extend_lane(value: u64, element_bytes: usize) -> u64 {
    let bits = element_bytes * 8;
    if bits == u64::BITS as usize {
        value
    } else {
        ((value << (u64::BITS as usize - bits)) as i64 >> (u64::BITS as usize - bits)) as u64
    }
}
