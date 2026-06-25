use crate::error::RiscvError;

pub(crate) fn mulh_signed(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as i64 as i128)) >> 64) as u64
}

pub(crate) fn mulh_signed_unsigned(lhs: u64, rhs: u64) -> u64 {
    (((lhs as i64 as i128) * (rhs as i128)) >> 64) as u64
}

pub(crate) fn mulh_unsigned(lhs: u64, rhs: u64) -> u64 {
    (((lhs as u128) * (rhs as u128)) >> 64) as u64
}

pub(crate) fn div_signed(lhs: u64, rhs: u64) -> u64 {
    let lhs = lhs as i64;
    let rhs = rhs as i64;
    if rhs == 0 {
        -1i64 as u64
    } else if lhs == i64::MIN && rhs == -1 {
        lhs as u64
    } else {
        (lhs / rhs) as u64
    }
}

pub(crate) fn div_unsigned(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        u64::MAX
    } else {
        lhs / rhs
    }
}

pub(crate) fn rem_signed(lhs: u64, rhs: u64) -> u64 {
    let lhs_signed = lhs as i64;
    let rhs_signed = rhs as i64;
    if rhs_signed == 0 {
        lhs
    } else if lhs_signed == i64::MIN && rhs_signed == -1 {
        0
    } else {
        (lhs_signed % rhs_signed) as u64
    }
}

pub(crate) fn rem_unsigned(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

pub(crate) const fn sign_extend_word(value: u32) -> u64 {
    value as i32 as i64 as u64
}

pub(crate) fn div_signed_word(lhs: u64, rhs: u64) -> u64 {
    let lhs = lhs as u32 as i32;
    let rhs = rhs as u32 as i32;
    let value = if rhs == 0 {
        -1i32 as u32
    } else if lhs == i32::MIN && rhs == -1 {
        lhs as u32
    } else {
        (lhs / rhs) as u32
    };
    sign_extend_word(value)
}

pub(crate) fn div_unsigned_word(lhs: u64, rhs: u64) -> u64 {
    let lhs = lhs as u32;
    let rhs = rhs as u32;
    let value = if rhs == 0 { u32::MAX } else { lhs / rhs };
    sign_extend_word(value)
}

pub(crate) fn rem_signed_word(lhs: u64, rhs: u64) -> u64 {
    let lhs_signed = lhs as u32 as i32;
    let rhs_signed = rhs as u32 as i32;
    let value = if rhs_signed == 0 {
        lhs as u32
    } else if lhs_signed == i32::MIN && rhs_signed == -1 {
        0
    } else {
        (lhs_signed % rhs_signed) as u32
    };
    sign_extend_word(value)
}

pub(crate) fn rem_unsigned_word(lhs: u64, rhs: u64) -> u64 {
    let lhs = lhs as u32;
    let rhs = rhs as u32;
    let value = if rhs == 0 { lhs } else { lhs % rhs };
    sign_extend_word(value)
}

pub(crate) fn add_signed(value: u64, offset: i64) -> Result<u64, RiscvError> {
    if offset >= 0 {
        value
            .checked_add(offset as u64)
            .ok_or(RiscvError::AddressOverflow { value, offset })
    } else {
        value
            .checked_sub(offset.unsigned_abs())
            .ok_or(RiscvError::AddressOverflow { value, offset })
    }
}
