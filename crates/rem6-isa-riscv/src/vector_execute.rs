use crate::{RiscvHartState, RiscvInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES};

const MAX_VECTOR_GROUP_REGISTERS: usize = 8;
const MAX_VECTOR_GROUP_BYTES: usize = RISCV_VECTOR_REGISTER_BYTES * MAX_VECTOR_GROUP_REGISTERS;

pub(crate) fn execute_vector_integer_binary(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
) -> bool {
    match instruction {
        RiscvInstruction::VectorAddVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorAddVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorAddVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorSubVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Sub)
        }
        RiscvInstruction::VectorSubVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Sub)
        }
        RiscvInstruction::VectorMinUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MinUnsigned)
        }
        RiscvInstruction::VectorMinUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MinUnsigned)
        }
        RiscvInstruction::VectorMinSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MinSigned)
        }
        RiscvInstruction::VectorMinSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MinSigned)
        }
        RiscvInstruction::VectorMaxUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MaxUnsigned)
        }
        RiscvInstruction::VectorMaxUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MaxUnsigned)
        }
        RiscvInstruction::VectorMaxSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MaxSigned)
        }
        RiscvInstruction::VectorMaxSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MaxSigned)
        }
        RiscvInstruction::VectorMultiplyLowVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyLow)
        }
        RiscvInstruction::VectorMultiplyLowVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MultiplyLow)
        }
        RiscvInstruction::VectorMultiplyHighUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyHighUnsigned)
        }
        RiscvInstruction::VectorMultiplyHighUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                LaneBinaryOp::MultiplyHighUnsigned,
            )
        }
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyHighSignedUnsigned)
        }
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                LaneBinaryOp::MultiplyHighSignedUnsigned,
            )
        }
        RiscvInstruction::VectorMultiplyHighSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyHighSigned)
        }
        RiscvInstruction::VectorMultiplyHighSignedVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::MultiplyHighSigned,
        ),
        RiscvInstruction::VectorDivideUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::DivideUnsigned)
        }
        RiscvInstruction::VectorDivideUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::DivideUnsigned)
        }
        RiscvInstruction::VectorDivideSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::DivideSigned)
        }
        RiscvInstruction::VectorDivideSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::DivideSigned)
        }
        RiscvInstruction::VectorRemainderUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::RemainderUnsigned)
        }
        RiscvInstruction::VectorRemainderUnsignedVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::RemainderUnsigned,
        ),
        RiscvInstruction::VectorRemainderSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::RemainderSigned)
        }
        RiscvInstruction::VectorRemainderSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::RemainderSigned)
        }
        RiscvInstruction::VectorAndVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::And)
        }
        RiscvInstruction::VectorAndVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::And)
        }
        RiscvInstruction::VectorAndVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::And)
        }
        RiscvInstruction::VectorOrVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Or)
        }
        RiscvInstruction::VectorOrVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Or)
        }
        RiscvInstruction::VectorOrVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::Or)
        }
        RiscvInstruction::VectorXorVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Xor)
        }
        RiscvInstruction::VectorXorVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Xor)
        }
        RiscvInstruction::VectorXorVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::Xor)
        }
        RiscvInstruction::VectorShiftLeftLogicalVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::ShiftLeftLogical)
        }
        RiscvInstruction::VectorShiftLeftLogicalVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::ShiftLeftLogical,
        ),
        RiscvInstruction::VectorShiftLeftLogicalVi { vd, vs2, shamt } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            u64::from(shamt),
            LaneBinaryOp::ShiftLeftLogical,
        ),
        RiscvInstruction::VectorShiftRightLogicalVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::ShiftRightLogical)
        }
        RiscvInstruction::VectorShiftRightLogicalVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::ShiftRightLogical,
        ),
        RiscvInstruction::VectorShiftRightLogicalVi { vd, vs2, shamt } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            u64::from(shamt),
            LaneBinaryOp::ShiftRightLogical,
        ),
        RiscvInstruction::VectorShiftRightArithmeticVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::ShiftRightArithmetic)
        }
        RiscvInstruction::VectorShiftRightArithmeticVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                LaneBinaryOp::ShiftRightArithmetic,
            )
        }
        RiscvInstruction::VectorShiftRightArithmeticVi { vd, vs2, shamt } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                u64::from(shamt),
                LaneBinaryOp::ShiftRightArithmetic,
            )
        }
        _ => false,
    }
}

fn execute_vector_binary_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
    operation: LaneBinaryOp,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, imm as i64 as u64, operation)
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
    MinUnsigned,
    MinSigned,
    MaxUnsigned,
    MaxSigned,
    MultiplyLow,
    MultiplyHighUnsigned,
    MultiplyHighSignedUnsigned,
    MultiplyHighSigned,
    DivideUnsigned,
    DivideSigned,
    RemainderUnsigned,
    RemainderSigned,
    And,
    Or,
    Xor,
    ShiftLeftLogical,
    ShiftRightLogical,
    ShiftRightArithmetic,
}

impl LaneBinaryOp {
    fn apply_u8(self, left: u8, right: u8) -> u8 {
        let shift = shift_amount(u64::from(right), 8);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i8).min(right as i8) as u8,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i8).max(right as i8) as u8,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u8::BITS) as u8
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i8), u128::from(right), u8::BITS)
                    as u8
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i8), i128::from(right as i8), u8::BITS)
                    as u8
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u8::MAX.into()) as u8
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i8),
                i128::from(right as i8),
                i128::from(i8::MIN),
            ) as u8,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u8
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i8),
                i128::from(right as i8),
                i128::from(i8::MIN),
            ) as u8,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i8) >> shift) as u8,
        }
    }

    fn apply_u16(self, left: u16, right: u16) -> u16 {
        let shift = shift_amount(u64::from(right), 16);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i16).min(right as i16) as u16,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i16).max(right as i16) as u16,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u16::BITS) as u16
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i16), u128::from(right), u16::BITS)
                    as u16
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i16), i128::from(right as i16), u16::BITS)
                    as u16
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u16::MAX.into()) as u16
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i16),
                i128::from(right as i16),
                i128::from(i16::MIN),
            ) as u16,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u16
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i16),
                i128::from(right as i16),
                i128::from(i16::MIN),
            ) as u16,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i16) >> shift) as u16,
        }
    }

    fn apply_u32(self, left: u32, right: u32) -> u32 {
        let shift = shift_amount(u64::from(right), 32);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i32).min(right as i32) as u32,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i32).max(right as i32) as u32,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u32::BITS) as u32
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i32), u128::from(right), u32::BITS)
                    as u32
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i32), i128::from(right as i32), u32::BITS)
                    as u32
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u32::MAX.into()) as u32
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i32),
                i128::from(right as i32),
                i128::from(i32::MIN),
            ) as u32,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u32
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i32),
                i128::from(right as i32),
                i128::from(i32::MIN),
            ) as u32,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i32) >> shift) as u32,
        }
    }

    fn apply_u64(self, left: u64, right: u64) -> u64 {
        let shift = shift_amount(right, 64);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i64).min(right as i64) as u64,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i64).max(right as i64) as u64,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u64::BITS) as u64
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i64), u128::from(right), u64::BITS)
                    as u64
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i64), i128::from(right as i64), u64::BITS)
                    as u64
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u64::MAX.into()) as u64
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i64),
                i128::from(right as i64),
                i128::from(i64::MIN),
            ) as u64,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u64
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i64),
                i128::from(right as i64),
                i128::from(i64::MIN),
            ) as u64,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i64) >> shift) as u64,
        }
    }
}

fn multiply_high_unsigned(left: u128, right: u128, element_bits: u32) -> u128 {
    (left * right) >> element_bits
}

fn multiply_high_signed(left: i128, right: i128, element_bits: u32) -> u128 {
    ((left * right) >> element_bits) as u128
}

fn multiply_high_signed_unsigned(left: i128, right: u128, element_bits: u32) -> u128 {
    ((left * right as i128) >> element_bits) as u128
}

fn divide_unsigned(left: u128, right: u128, division_by_zero_result: u128) -> u128 {
    if right == 0 {
        division_by_zero_result
    } else {
        left / right
    }
}

fn divide_signed(left: i128, right: i128, min_value: i128) -> u128 {
    if right == 0 {
        u128::MAX
    } else if left == min_value && right == -1 {
        min_value as u128
    } else {
        (left / right) as u128
    }
}

fn remainder_unsigned(left: u128, right: u128) -> u128 {
    if right == 0 {
        left
    } else {
        left % right
    }
}

fn remainder_signed(left: i128, right: i128, min_value: i128) -> u128 {
    if right == 0 {
        left as u128
    } else if left == min_value && right == -1 {
        0
    } else {
        (left % right) as u128
    }
}

fn shift_amount(raw: u64, element_bits: u32) -> u32 {
    (raw & u64::from(element_bits - 1)) as u32
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
