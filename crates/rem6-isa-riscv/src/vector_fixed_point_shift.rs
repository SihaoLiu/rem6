use crate::{
    vector::{RiscvVectorError, RiscvVectorFixedRoundingMode},
    Register, VectorRegister,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorFixedPointShiftInstruction {
    Vv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        operation: RiscvVectorFixedPointShiftOperation,
    },
    Vx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        operation: RiscvVectorFixedPointShiftOperation,
    },
    Vi {
        vd: VectorRegister,
        vs2: VectorRegister,
        shamt: u8,
        operation: RiscvVectorFixedPointShiftOperation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorFixedPointShiftOperation {
    ShiftRightLogical,
    ShiftRightArithmetic,
}

impl RiscvVectorFixedPointShiftInstruction {
    pub const fn shift_right_logical_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::Vv {
            vd,
            vs2,
            vs1,
            operation: RiscvVectorFixedPointShiftOperation::ShiftRightLogical,
        }
    }

    pub const fn shift_right_arithmetic_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::Vv {
            vd,
            vs2,
            vs1,
            operation: RiscvVectorFixedPointShiftOperation::ShiftRightArithmetic,
        }
    }

    pub const fn shift_right_logical_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    ) -> Self {
        Self::Vx {
            vd,
            vs2,
            rs1,
            operation: RiscvVectorFixedPointShiftOperation::ShiftRightLogical,
        }
    }

    pub const fn shift_right_arithmetic_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    ) -> Self {
        Self::Vx {
            vd,
            vs2,
            rs1,
            operation: RiscvVectorFixedPointShiftOperation::ShiftRightArithmetic,
        }
    }

    pub const fn shift_right_logical_vi(
        vd: VectorRegister,
        vs2: VectorRegister,
        shamt: u8,
    ) -> Self {
        Self::Vi {
            vd,
            vs2,
            shamt,
            operation: RiscvVectorFixedPointShiftOperation::ShiftRightLogical,
        }
    }

    pub const fn shift_right_arithmetic_vi(
        vd: VectorRegister,
        vs2: VectorRegister,
        shamt: u8,
    ) -> Self {
        Self::Vi {
            vd,
            vs2,
            shamt,
            operation: RiscvVectorFixedPointShiftOperation::ShiftRightArithmetic,
        }
    }
}

pub(crate) fn validate_fixed_point_shift(shift: u32) -> Result<(), RiscvVectorError> {
    if shift >= 128 {
        return Err(RiscvVectorError::InvalidFixedPointShift { shift });
    }
    Ok(())
}

pub(crate) fn round_unsigned(
    value: u128,
    shift: u32,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> Result<u128, RiscvVectorError> {
    if shift == 0 {
        return Ok(value);
    }

    let lsb = 1_u128 << shift;
    let lsb_half = lsb >> 1;
    match rounding_mode {
        RiscvVectorFixedRoundingMode::RoundNearestUp => value
            .checked_add(lsb_half)
            .ok_or(RiscvVectorError::FixedPointRoundingOverflow),
        RiscvVectorFixedRoundingMode::RoundNearestEven => {
            let round =
                (value & lsb_half) != 0 && ((value & (lsb_half - 1)) != 0 || (value & lsb) != 0);
            if round {
                value
                    .checked_add(lsb)
                    .ok_or(RiscvVectorError::FixedPointRoundingOverflow)
            } else {
                Ok(value)
            }
        }
        RiscvVectorFixedRoundingMode::RoundDown => Ok(value),
        RiscvVectorFixedRoundingMode::RoundToOdd => {
            if value & (lsb - 1) != 0 {
                Ok(value | lsb)
            } else {
                Ok(value)
            }
        }
    }
}

pub(crate) fn round_signed(
    value: i128,
    shift: u32,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> Result<i128, RiscvVectorError> {
    if shift == 0 {
        return Ok(value);
    }

    let value_bits = value as u128;
    let lsb = 1_u128 << shift;
    let lsb_half = lsb >> 1;
    match rounding_mode {
        RiscvVectorFixedRoundingMode::RoundNearestUp => {
            let increment = i128::try_from(lsb_half)
                .map_err(|_| RiscvVectorError::FixedPointRoundingOverflow)?;
            value
                .checked_add(increment)
                .ok_or(RiscvVectorError::FixedPointRoundingOverflow)
        }
        RiscvVectorFixedRoundingMode::RoundNearestEven => {
            let round = (value_bits & lsb_half) != 0
                && ((value_bits & (lsb_half - 1)) != 0 || (value_bits & lsb) != 0);
            if round {
                let increment = i128::try_from(lsb)
                    .map_err(|_| RiscvVectorError::FixedPointRoundingOverflow)?;
                value
                    .checked_add(increment)
                    .ok_or(RiscvVectorError::FixedPointRoundingOverflow)
            } else {
                Ok(value)
            }
        }
        RiscvVectorFixedRoundingMode::RoundDown => Ok(value),
        RiscvVectorFixedRoundingMode::RoundToOdd => {
            if value_bits & (lsb - 1) != 0 {
                Ok((value_bits | lsb) as i128)
            } else {
                Ok(value)
            }
        }
    }
}
