use std::error::Error;
use std::fmt;

use crate::MemoryWidth;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct RiscvInstructionFlags {
    bits: u32,
}

impl RiscvInstructionFlags {
    pub const EMPTY: Self = Self { bits: 0 };
    pub const SERIALIZE_AFTER: Self = Self { bits: 1 << 0 };
    pub const NON_SPECULATIVE: Self = Self { bits: 1 << 1 };
    pub const DELAYED_COMMIT: Self = Self { bits: 1 << 2 };

    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    pub const fn contains(self, required: Self) -> bool {
        (self.bits & required.bits) == required.bits
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorMicroOpExpansion {
    micro_op_count: usize,
    macro_flags: RiscvInstructionFlags,
    micro_op_flags: RiscvInstructionFlags,
}

impl RiscvVectorMicroOpExpansion {
    pub const fn new(micro_op_count: usize) -> Self {
        Self {
            micro_op_count,
            macro_flags: RiscvInstructionFlags::EMPTY,
            micro_op_flags: RiscvInstructionFlags::EMPTY,
        }
    }

    pub const fn with_macro_flags(mut self, flags: RiscvInstructionFlags) -> Self {
        self.macro_flags = self.macro_flags.union(flags);
        self
    }

    pub const fn with_micro_op_flags(mut self, flags: RiscvInstructionFlags) -> Self {
        self.micro_op_flags = self.micro_op_flags.union(flags);
        self
    }

    pub const fn micro_op_count(self) -> usize {
        self.micro_op_count
    }

    pub const fn macro_flags(self) -> RiscvInstructionFlags {
        self.macro_flags
    }

    pub const fn micro_op_flags(self) -> RiscvInstructionFlags {
        self.micro_op_flags
    }

    pub fn expand(self) -> Result<Vec<RiscvVectorMicroOp>, RiscvVectorError> {
        if self.micro_op_count == 0 {
            return Err(RiscvVectorError::EmptyMicroOpExpansion);
        }

        let flags = self.macro_flags.union(self.micro_op_flags);
        Ok((0..self.micro_op_count)
            .map(|index| {
                RiscvVectorMicroOp::new(index, flags, index == 0, index + 1 == self.micro_op_count)
            })
            .collect())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorMicroOp {
    index: usize,
    flags: RiscvInstructionFlags,
    first: bool,
    last: bool,
}

impl RiscvVectorMicroOp {
    pub const fn new(index: usize, flags: RiscvInstructionFlags, first: bool, last: bool) -> Self {
        Self {
            index,
            flags,
            first,
            last,
        }
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub const fn flags(self) -> RiscvInstructionFlags {
        self.flags
    }

    pub const fn is_first(self) -> bool {
        self.first
    }

    pub const fn is_last(self) -> bool {
        self.last
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorFixedRoundingMode {
    RoundNearestUp,
    RoundNearestEven,
    RoundDown,
    RoundToOdd,
}

impl RiscvVectorFixedRoundingMode {
    pub const fn from_vxrm_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::RoundNearestUp,
            1 => Self::RoundNearestEven,
            2 => Self::RoundDown,
            _ => Self::RoundToOdd,
        }
    }

    pub const fn vxrm_bits(self) -> u8 {
        match self {
            Self::RoundNearestUp => 0,
            Self::RoundNearestEven => 1,
            Self::RoundDown => 2,
            Self::RoundToOdd => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorFixedPointState {
    rounding_mode: RiscvVectorFixedRoundingMode,
    vxsat: bool,
}

impl RiscvVectorFixedPointState {
    pub const fn new(rounding_mode: RiscvVectorFixedRoundingMode) -> Self {
        Self {
            rounding_mode,
            vxsat: false,
        }
    }

    pub const fn rounding_mode(self) -> RiscvVectorFixedRoundingMode {
        self.rounding_mode
    }

    pub const fn vxsat(self) -> bool {
        self.vxsat
    }

    pub const fn vxrm_bits(self) -> u8 {
        self.rounding_mode.vxrm_bits()
    }

    pub const fn vcsr_bits(self) -> u8 {
        (self.vxrm_bits() << 1) | self.vxsat as u8
    }

    pub fn write_vxrm_bits(&mut self, bits: u8) {
        self.rounding_mode = RiscvVectorFixedRoundingMode::from_vxrm_bits(bits);
    }

    pub fn write_vxsat_bit(&mut self, vxsat: bool) {
        self.vxsat = vxsat;
    }

    pub fn write_vcsr_bits(&mut self, bits: u8) {
        self.vxsat = bits & 0b1 != 0;
        self.write_vxrm_bits((bits >> 1) & 0b11);
    }

    pub fn apply_narrow_clip_result(&mut self, result: RiscvVectorNarrowClipResult) {
        self.vxsat |= result.saturated;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorNarrowClipPlan {
    width: MemoryWidth,
    signed: bool,
}

impl RiscvVectorNarrowClipPlan {
    pub const fn unsigned(width: MemoryWidth) -> Self {
        Self {
            width,
            signed: false,
        }
    }

    pub const fn signed(width: MemoryWidth) -> Self {
        Self {
            width,
            signed: true,
        }
    }

    pub const fn width(self) -> MemoryWidth {
        self.width
    }

    pub const fn is_signed(self) -> bool {
        self.signed
    }

    pub fn execute_unsigned(
        self,
        value: u128,
        shift: u32,
        rounding_mode: RiscvVectorFixedRoundingMode,
    ) -> Result<RiscvVectorNarrowClipResult, RiscvVectorError> {
        if self.signed {
            return Err(RiscvVectorError::NarrowClipSignednessMismatch {
                expected_signed: true,
                actual_signed: false,
            });
        }
        validate_fixed_point_shift(shift)?;

        let rounded = round_unsigned(value, shift, rounding_mode)?;
        let shifted = rounded >> shift;
        let max = unsigned_max(self.width);
        let saturated = shifted > max;
        let value = if saturated { max } else { shifted } as i128;

        Ok(RiscvVectorNarrowClipResult { value, saturated })
    }

    pub fn execute_signed(
        self,
        value: i128,
        shift: u32,
        rounding_mode: RiscvVectorFixedRoundingMode,
    ) -> Result<RiscvVectorNarrowClipResult, RiscvVectorError> {
        if !self.signed {
            return Err(RiscvVectorError::NarrowClipSignednessMismatch {
                expected_signed: false,
                actual_signed: true,
            });
        }
        validate_fixed_point_shift(shift)?;

        let rounded = round_signed(value, shift, rounding_mode)?;
        let shifted = rounded >> shift;
        let min = signed_min(self.width);
        let max = signed_max(self.width);
        let saturated = shifted < min || shifted > max;
        let value = shifted.clamp(min, max);

        Ok(RiscvVectorNarrowClipResult { value, saturated })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorNarrowClipResult {
    value: i128,
    saturated: bool,
}

impl RiscvVectorNarrowClipResult {
    pub const fn value(self) -> i128 {
        self.value
    }

    pub const fn saturated(self) -> bool {
        self.saturated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvVectorElements {
    width: MemoryWidth,
    elements: Vec<u64>,
}

impl RiscvVectorElements {
    pub fn new(width: MemoryWidth, elements: Vec<u64>) -> Result<Self, RiscvVectorError> {
        if let Some(value) = elements
            .iter()
            .copied()
            .find(|value| *value & !element_mask(width) != 0)
        {
            return Err(RiscvVectorError::ElementExceedsWidth { width, value });
        }

        Ok(Self { width, elements })
    }

    pub const fn width(&self) -> MemoryWidth {
        self.width
    }

    pub fn as_slice(&self) -> &[u64] {
        &self.elements
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorTailPolicy {
    Undisturbed,
    AgnosticAllOnes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorCompressPlan {
    vl: usize,
    tail_policy: RiscvVectorTailPolicy,
}

impl RiscvVectorCompressPlan {
    pub const fn new(vl: usize, tail_policy: RiscvVectorTailPolicy) -> Self {
        Self { vl, tail_policy }
    }

    pub const fn vl(self) -> usize {
        self.vl
    }

    pub const fn tail_policy(self) -> RiscvVectorTailPolicy {
        self.tail_policy
    }

    pub fn execute(
        self,
        destination: &RiscvVectorElements,
        source: &RiscvVectorElements,
        mask: &[bool],
    ) -> Result<RiscvVectorCompressResult, RiscvVectorError> {
        validate_compress_shape(self.vl, destination, source, mask)?;

        let mut output = destination.elements.clone();
        let mut compressed_count = 0;

        for (source_element, selected) in source.elements.iter().zip(mask.iter()).take(self.vl) {
            if *selected {
                output[compressed_count] = *source_element;
                compressed_count += 1;
            }
        }

        match self.tail_policy {
            RiscvVectorTailPolicy::Undisturbed => {}
            RiscvVectorTailPolicy::AgnosticAllOnes => {
                let ones = element_mask(source.width);
                output
                    .iter_mut()
                    .skip(compressed_count)
                    .for_each(|element| *element = ones);
            }
        }

        Ok(RiscvVectorCompressResult {
            elements: RiscvVectorElements {
                width: destination.width,
                elements: output,
            },
            compressed_count,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvVectorCompressResult {
    elements: RiscvVectorElements,
    compressed_count: usize,
}

impl RiscvVectorCompressResult {
    pub const fn elements(&self) -> &RiscvVectorElements {
        &self.elements
    }

    pub const fn compressed_count(&self) -> usize {
        self.compressed_count
    }
}

fn validate_compress_shape(
    vl: usize,
    destination: &RiscvVectorElements,
    source: &RiscvVectorElements,
    mask: &[bool],
) -> Result<(), RiscvVectorError> {
    if destination.width != source.width {
        return Err(RiscvVectorError::ElementWidthMismatch {
            destination: destination.width,
            source: source.width,
        });
    }

    if destination.len() != source.len() {
        return Err(RiscvVectorError::ElementCountMismatch {
            destination: destination.len(),
            source: source.len(),
        });
    }

    if vl > source.len() {
        return Err(RiscvVectorError::VlExceedsElementCount {
            vl,
            elements: source.len(),
        });
    }

    if vl > mask.len() {
        return Err(RiscvVectorError::VlExceedsMaskLength {
            vl,
            mask: mask.len(),
        });
    }

    Ok(())
}

const fn element_mask(width: MemoryWidth) -> u64 {
    match width {
        MemoryWidth::Byte => 0xff,
        MemoryWidth::Halfword => 0xffff,
        MemoryWidth::Word => 0xffff_ffff,
        MemoryWidth::Doubleword => u64::MAX,
    }
}

const fn element_bits(width: MemoryWidth) -> u32 {
    match width {
        MemoryWidth::Byte => 8,
        MemoryWidth::Halfword => 16,
        MemoryWidth::Word => 32,
        MemoryWidth::Doubleword => 64,
    }
}

const fn unsigned_max(width: MemoryWidth) -> u128 {
    match width {
        MemoryWidth::Byte => u8::MAX as u128,
        MemoryWidth::Halfword => u16::MAX as u128,
        MemoryWidth::Word => u32::MAX as u128,
        MemoryWidth::Doubleword => u64::MAX as u128,
    }
}

fn signed_min(width: MemoryWidth) -> i128 {
    -(1_i128 << (element_bits(width) - 1))
}

fn signed_max(width: MemoryWidth) -> i128 {
    (1_i128 << (element_bits(width) - 1)) - 1
}

fn validate_fixed_point_shift(shift: u32) -> Result<(), RiscvVectorError> {
    if shift >= 128 {
        return Err(RiscvVectorError::InvalidFixedPointShift { shift });
    }
    Ok(())
}

fn round_unsigned(
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

fn round_signed(
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvVectorError {
    ElementExceedsWidth {
        width: MemoryWidth,
        value: u64,
    },
    ElementWidthMismatch {
        destination: MemoryWidth,
        source: MemoryWidth,
    },
    ElementCountMismatch {
        destination: usize,
        source: usize,
    },
    VlExceedsElementCount {
        vl: usize,
        elements: usize,
    },
    VlExceedsMaskLength {
        vl: usize,
        mask: usize,
    },
    EmptyMicroOpExpansion,
    InvalidFixedPointShift {
        shift: u32,
    },
    FixedPointRoundingOverflow,
    NarrowClipSignednessMismatch {
        expected_signed: bool,
        actual_signed: bool,
    },
}

impl fmt::Display for RiscvVectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementExceedsWidth { width, value } => {
                write!(
                    formatter,
                    "RISC-V vector element {value:#x} exceeds {width:?} width"
                )
            }
            Self::ElementWidthMismatch {
                destination,
                source,
            } => {
                write!(
                    formatter,
                    "RISC-V vector destination width {destination:?} does not match source width {source:?}"
                )
            }
            Self::ElementCountMismatch {
                destination,
                source,
            } => {
                write!(
                    formatter,
                    "RISC-V vector destination element count {destination} does not match source count {source}"
                )
            }
            Self::VlExceedsElementCount { vl, elements } => {
                write!(
                    formatter,
                    "RISC-V vector length {vl} exceeds element count {elements}"
                )
            }
            Self::VlExceedsMaskLength { vl, mask } => {
                write!(
                    formatter,
                    "RISC-V vector length {vl} exceeds mask length {mask}"
                )
            }
            Self::EmptyMicroOpExpansion => {
                write!(formatter, "RISC-V vector micro-op expansion is empty")
            }
            Self::InvalidFixedPointShift { shift } => {
                write!(
                    formatter,
                    "RISC-V vector fixed-point shift {shift} exceeds 127 bits"
                )
            }
            Self::FixedPointRoundingOverflow => {
                write!(formatter, "RISC-V vector fixed-point rounding overflowed")
            }
            Self::NarrowClipSignednessMismatch {
                expected_signed,
                actual_signed,
            } => {
                write!(
                    formatter,
                    "RISC-V vector narrow clip signedness mismatch: expected signed {expected_signed}, got signed {actual_signed}"
                )
            }
        }
    }
}

impl Error for RiscvVectorError {}
