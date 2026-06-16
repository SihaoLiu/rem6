use crate::{RiscvHartState, RiscvInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES};

const MAX_VECTOR_GROUP_REGISTERS: usize = 8;
const MAX_VECTOR_GROUP_BYTES: usize = RISCV_VECTOR_REGISTER_BYTES * MAX_VECTOR_GROUP_REGISTERS;

pub(crate) fn execute_vector_integer_binary(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
) -> bool {
    match instruction {
        RiscvInstruction::VectorAddVv { vd, vs1, vs2 } => execute_vector_add_vv(hart, vd, vs1, vs2),
        RiscvInstruction::VectorAddVx { vd, vs2, rs1 } => {
            execute_vector_add_vx(hart, vd, vs2, hart.read(rs1))
        }
        RiscvInstruction::VectorAddVi { vd, vs2, imm } => execute_vector_add_vi(hart, vd, vs2, imm),
        RiscvInstruction::VectorSubVv { vd, vs1, vs2 } => execute_vector_sub_vv(hart, vd, vs1, vs2),
        RiscvInstruction::VectorSubVx { vd, vs2, rs1 } => {
            execute_vector_sub_vx(hart, vd, vs2, hart.read(rs1))
        }
        RiscvInstruction::VectorAndVv { vd, vs1, vs2 } => execute_vector_and_vv(hart, vd, vs1, vs2),
        RiscvInstruction::VectorAndVx { vd, vs2, rs1 } => {
            execute_vector_and_vx(hart, vd, vs2, hart.read(rs1))
        }
        RiscvInstruction::VectorAndVi { vd, vs2, imm } => execute_vector_and_vi(hart, vd, vs2, imm),
        RiscvInstruction::VectorOrVv { vd, vs1, vs2 } => execute_vector_or_vv(hart, vd, vs1, vs2),
        RiscvInstruction::VectorOrVx { vd, vs2, rs1 } => {
            execute_vector_or_vx(hart, vd, vs2, hart.read(rs1))
        }
        RiscvInstruction::VectorOrVi { vd, vs2, imm } => execute_vector_or_vi(hart, vd, vs2, imm),
        RiscvInstruction::VectorXorVv { vd, vs1, vs2 } => execute_vector_xor_vv(hart, vd, vs1, vs2),
        RiscvInstruction::VectorXorVx { vd, vs2, rs1 } => {
            execute_vector_xor_vx(hart, vd, vs2, hart.read(rs1))
        }
        RiscvInstruction::VectorXorVi { vd, vs2, imm } => execute_vector_xor_vi(hart, vd, vs2, imm),
        _ => false,
    }
}

fn execute_vector_add_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
) -> bool {
    execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Add)
}

fn execute_vector_add_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, scalar, LaneBinaryOp::Add)
}

fn execute_vector_add_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
) -> bool {
    execute_vector_add_vx(hart, vd, vs2, imm as i64 as u64)
}

fn execute_vector_sub_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
) -> bool {
    execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Sub)
}

fn execute_vector_sub_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, scalar, LaneBinaryOp::Sub)
}

fn execute_vector_and_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
) -> bool {
    execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::And)
}

fn execute_vector_and_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, scalar, LaneBinaryOp::And)
}

fn execute_vector_and_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
) -> bool {
    execute_vector_and_vx(hart, vd, vs2, imm as i64 as u64)
}

fn execute_vector_or_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
) -> bool {
    execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Or)
}

fn execute_vector_or_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, scalar, LaneBinaryOp::Or)
}

fn execute_vector_or_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
) -> bool {
    execute_vector_or_vx(hart, vd, vs2, imm as i64 as u64)
}

fn execute_vector_xor_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
) -> bool {
    execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Xor)
}

fn execute_vector_xor_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, scalar, LaneBinaryOp::Xor)
}

fn execute_vector_xor_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
) -> bool {
    execute_vector_xor_vx(hart, vd, vs2, imm as i64 as u64)
}

fn execute_vector_binary_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: LaneBinaryOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_vector_lanes(&plan, &mut result, &left, &right, operation);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_binary_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    operation: LaneBinaryOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_scalar_lanes(&plan, &mut result, &left, scalar, operation);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

#[derive(Clone, Copy)]
enum LaneBinaryOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
}

impl LaneBinaryOp {
    fn apply_u8(self, left: u8, right: u8) -> u8 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
        }
    }

    fn apply_u16(self, left: u16, right: u16) -> u16 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
        }
    }

    fn apply_u32(self, left: u32, right: u32) -> u32 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
        }
    }

    fn apply_u64(self, left: u64, right: u64) -> u64 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
        }
    }
}

struct VectorBinaryPlan {
    element_bytes: usize,
    group_registers: usize,
    active_bytes: usize,
}

impl VectorBinaryPlan {
    fn new(
        hart: &RiscvHartState,
        destination: VectorRegister,
        sources: &[VectorRegister],
    ) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let group_registers = config.register_group_registers()?;
        if !valid_register_group(destination, group_registers)
            || sources
                .iter()
                .any(|source| !valid_register_group(*source, group_registers))
        {
            return None;
        }

        let active_bytes = (config.vl() as usize).checked_mul(element_bytes)?;
        if active_bytes > group_registers * RISCV_VECTOR_REGISTER_BYTES {
            return None;
        }

        Some(Self {
            element_bytes,
            group_registers,
            active_bytes,
        })
    }
}

fn apply_vector_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right: &[u8; MAX_VECTOR_GROUP_BYTES],
    operation: LaneBinaryOp,
) {
    for offset in (0..plan.active_bytes).step_by(plan.element_bytes) {
        apply_lane(
            &mut result[offset..offset + plan.element_bytes],
            &left[offset..offset + plan.element_bytes],
            &right[offset..offset + plan.element_bytes],
            operation,
        );
    }
}

fn apply_scalar_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    operation: LaneBinaryOp,
) {
    for offset in (0..plan.active_bytes).step_by(plan.element_bytes) {
        apply_lane_scalar(
            &mut result[offset..offset + plan.element_bytes],
            &left[offset..offset + plan.element_bytes],
            scalar,
            operation,
        );
    }
}

fn valid_register_group(register: VectorRegister, group_registers: usize) -> bool {
    let index = register.index() as usize;
    group_registers > 0
        && group_registers <= MAX_VECTOR_GROUP_REGISTERS
        && index.is_multiple_of(group_registers)
        && index + group_registers <= 32
}

fn read_register_group(
    hart: &RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
) -> [u8; MAX_VECTOR_GROUP_BYTES] {
    let mut bytes = [0; MAX_VECTOR_GROUP_BYTES];
    for group_index in 0..group_registers {
        let vector = hart.read_vector(vector_register_at(register, group_index));
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES].copy_from_slice(&vector);
    }
    bytes
}

fn write_register_group(
    hart: &mut RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
    bytes: &[u8; MAX_VECTOR_GROUP_BYTES],
) {
    for group_index in 0..group_registers {
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        let mut vector = [0; RISCV_VECTOR_REGISTER_BYTES];
        vector.copy_from_slice(&bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES]);
        hart.write_vector(vector_register_at(register, group_index), vector);
    }
}

fn vector_register_at(base: VectorRegister, group_index: usize) -> VectorRegister {
    VectorRegister::from_field(u32::from(base.index()) + group_index as u32)
}

fn apply_lane(result: &mut [u8], left: &[u8], right: &[u8], operation: LaneBinaryOp) {
    match result.len() {
        1 => result[0] = operation.apply_u8(left[0], right[0]),
        2 => result.copy_from_slice(
            &operation
                .apply_u16(
                    u16::from_le_bytes([left[0], left[1]]),
                    u16::from_le_bytes([right[0], right[1]]),
                )
                .to_le_bytes(),
        ),
        4 => result.copy_from_slice(
            &operation
                .apply_u32(
                    u32::from_le_bytes([left[0], left[1], left[2], left[3]]),
                    u32::from_le_bytes([right[0], right[1], right[2], right[3]]),
                )
                .to_le_bytes(),
        ),
        8 => result.copy_from_slice(
            &operation
                .apply_u64(
                    u64::from_le_bytes([
                        left[0], left[1], left[2], left[3], left[4], left[5], left[6], left[7],
                    ]),
                    u64::from_le_bytes([
                        right[0], right[1], right[2], right[3], right[4], right[5], right[6],
                        right[7],
                    ]),
                )
                .to_le_bytes(),
        ),
        _ => unreachable!("validated vector element width"),
    }
}

fn apply_lane_scalar(result: &mut [u8], left: &[u8], scalar: u64, operation: LaneBinaryOp) {
    match result.len() {
        1 => result[0] = operation.apply_u8(left[0], scalar as u8),
        2 => result.copy_from_slice(
            &operation
                .apply_u16(u16::from_le_bytes([left[0], left[1]]), scalar as u16)
                .to_le_bytes(),
        ),
        4 => result.copy_from_slice(
            &operation
                .apply_u32(
                    u32::from_le_bytes([left[0], left[1], left[2], left[3]]),
                    scalar as u32,
                )
                .to_le_bytes(),
        ),
        8 => result.copy_from_slice(
            &operation
                .apply_u64(
                    u64::from_le_bytes([
                        left[0], left[1], left[2], left[3], left[4], left[5], left[6], left[7],
                    ]),
                    scalar,
                )
                .to_le_bytes(),
        ),
        _ => unreachable!("validated vector element width"),
    }
}
