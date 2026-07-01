#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvFloatCsr {
    Fflags,
    Frm,
    Fcsr,
}

impl RiscvFloatCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Fflags => 0x001,
            Self::Frm => 0x002,
            Self::Fcsr => 0x003,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x001 => Some(Self::Fflags),
            0x002 => Some(Self::Frm),
            0x003 => Some(Self::Fcsr),
            _ => None,
        }
    }

    pub const fn read(self, status: RiscvFloatStatus) -> u64 {
        match self {
            Self::Fflags => status.fflags(),
            Self::Frm => status.frm(),
            Self::Fcsr => status.bits(),
        }
    }

    pub const fn write(self, status: RiscvFloatStatus, value: u64) -> RiscvFloatStatus {
        match self {
            Self::Fflags => status.with_fflags(value),
            Self::Frm => status.with_frm(value),
            Self::Fcsr => RiscvFloatStatus::new(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvFloatStatus {
    bits: u8,
}

impl RiscvFloatStatus {
    const FFLAGS_MASK: u8 = 0x1f;
    const FRM_MASK: u8 = 0xe0;
    const FCSR_MASK: u64 = 0xff;

    pub const fn new(bits: u64) -> Self {
        Self {
            bits: (bits & Self::FCSR_MASK) as u8,
        }
    }

    pub const fn bits(self) -> u64 {
        self.bits as u64
    }

    pub const fn fflags(self) -> u64 {
        (self.bits & Self::FFLAGS_MASK) as u64
    }

    pub const fn frm(self) -> u64 {
        ((self.bits & Self::FRM_MASK) >> 5) as u64
    }

    pub const fn with_fflags(mut self, value: u64) -> Self {
        self.bits = (self.bits & !Self::FFLAGS_MASK) | ((value as u8) & Self::FFLAGS_MASK);
        self
    }

    pub const fn with_frm(mut self, value: u64) -> Self {
        self.bits = (self.bits & !Self::FRM_MASK) | (((value as u8) & 0x7) << 5);
        self
    }

    pub fn raise_exception_flags(&mut self, flags: u64) {
        self.bits |= (flags as u8) & Self::FFLAGS_MASK;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvFloatRoundingMode {
    RoundNearestEven,
    RoundTowardZero,
    RoundDown,
    RoundUp,
    RoundNearestMaxMagnitude,
    Dynamic,
}

impl RiscvFloatRoundingMode {
    pub const fn from_rm_bits(bits: u8) -> Option<Self> {
        match bits {
            0..=4 => Self::from_frm_bits(bits),
            7 => Some(Self::Dynamic),
            _ => None,
        }
    }

    pub const fn from_frm_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::RoundNearestEven),
            1 => Some(Self::RoundTowardZero),
            2 => Some(Self::RoundDown),
            3 => Some(Self::RoundUp),
            4 => Some(Self::RoundNearestMaxMagnitude),
            _ => None,
        }
    }

    pub const fn resolve(self, frm: u64) -> Option<Self> {
        match self {
            Self::Dynamic => Self::from_frm_bits(frm as u8),
            mode @ (Self::RoundNearestEven
            | Self::RoundTowardZero
            | Self::RoundDown
            | Self::RoundUp
            | Self::RoundNearestMaxMagnitude) => Some(mode),
        }
    }
}
