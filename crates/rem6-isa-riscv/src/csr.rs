use crate::RiscvCsrError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCounterCsr {
    Cycle,
    Instret,
}

impl RiscvCounterCsr {
    pub const fn user_address(self) -> u16 {
        match self {
            Self::Cycle => 0xc00,
            Self::Instret => 0xc02,
        }
    }

    pub const fn machine_address(self) -> u16 {
        match self {
            Self::Cycle => 0xb00,
            Self::Instret => 0xb02,
        }
    }

    pub const fn from_user_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xc00 => Ok(Self::Cycle),
            0xc02 => Ok(Self::Instret),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }

    pub const fn from_machine_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xb00 => Ok(Self::Cycle),
            0xb02 => Ok(Self::Instret),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCounterCsrWord {
    CycleLow,
    CycleHigh,
    InstretLow,
    InstretHigh,
}

impl RiscvCounterCsrWord {
    pub const fn counter(self) -> RiscvCounterCsr {
        match self {
            Self::CycleLow | Self::CycleHigh => RiscvCounterCsr::Cycle,
            Self::InstretLow | Self::InstretHigh => RiscvCounterCsr::Instret,
        }
    }

    pub const fn user_address(self) -> u16 {
        match self {
            Self::CycleLow => 0xc00,
            Self::InstretLow => 0xc02,
            Self::CycleHigh => 0xc80,
            Self::InstretHigh => 0xc82,
        }
    }

    pub const fn machine_address(self) -> u16 {
        match self {
            Self::CycleLow => 0xb00,
            Self::InstretLow => 0xb02,
            Self::CycleHigh => 0xb80,
            Self::InstretHigh => 0xb82,
        }
    }

    pub const fn from_user_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xc00 => Ok(Self::CycleLow),
            0xc02 => Ok(Self::InstretLow),
            0xc80 => Ok(Self::CycleHigh),
            0xc82 => Ok(Self::InstretHigh),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }

    pub const fn from_machine_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xb00 => Ok(Self::CycleLow),
            0xb02 => Ok(Self::InstretLow),
            0xb80 => Ok(Self::CycleHigh),
            0xb82 => Ok(Self::InstretHigh),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvTranslationCsr {
    Satp,
}

impl RiscvTranslationCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Satp => 0x180,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x180 => Some(Self::Satp),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvStatusWord {
    bits: u64,
}

impl RiscvStatusWord {
    const SUM_BIT: u32 = 18;
    const MXR_BIT: u32 = 19;

    pub const fn new(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn sum(self) -> bool {
        status_bit(self.bits, Self::SUM_BIT)
    }

    pub const fn with_sum(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::SUM_BIT, enabled);
        self
    }

    pub const fn mxr(self) -> bool {
        status_bit(self.bits, Self::MXR_BIT)
    }

    pub const fn with_mxr(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::MXR_BIT, enabled);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCounterSnapshot {
    cycle: u64,
    instret: u64,
}

impl RiscvCounterSnapshot {
    pub const fn new(cycle: u64, instret: u64) -> Self {
        Self { cycle, instret }
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub const fn instret(&self) -> u64 {
        self.instret
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCounterBank {
    cycle: u64,
    instret: u64,
}

impl RiscvCounterBank {
    pub const fn new() -> Self {
        Self {
            cycle: 0,
            instret: 0,
        }
    }

    pub const fn read_user(&self, csr: RiscvCounterCsr) -> u64 {
        self.read(csr)
    }

    pub const fn read_machine(&self, csr: RiscvCounterCsr) -> u64 {
        self.read(csr)
    }

    pub const fn read_user_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        self.read_word(csr)
    }

    pub const fn read_machine_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        self.read_word(csr)
    }

    pub fn write_user(&mut self, csr: RiscvCounterCsr, _value: u64) -> Result<(), RiscvCsrError> {
        Err(RiscvCsrError::ReadOnlyCounterAlias { csr })
    }

    pub fn write_user_word(
        &mut self,
        csr: RiscvCounterCsrWord,
        _value: u32,
    ) -> Result<(), RiscvCsrError> {
        Err(RiscvCsrError::ReadOnlyCounterWordAlias { csr })
    }

    pub fn write_machine(&mut self, csr: RiscvCounterCsr, value: u64) -> Result<(), RiscvCsrError> {
        self.set_machine(csr, value);
        Ok(())
    }

    pub fn set_machine(&mut self, csr: RiscvCounterCsr, value: u64) {
        match csr {
            RiscvCounterCsr::Cycle => self.cycle = value,
            RiscvCounterCsr::Instret => self.instret = value,
        }
    }

    pub fn write_machine_word(
        &mut self,
        csr: RiscvCounterCsrWord,
        value: u32,
    ) -> Result<(), RiscvCsrError> {
        match csr {
            RiscvCounterCsrWord::CycleLow => self.cycle = replace_low_word(self.cycle, value),
            RiscvCounterCsrWord::CycleHigh => self.cycle = replace_high_word(self.cycle, value),
            RiscvCounterCsrWord::InstretLow => self.instret = replace_low_word(self.instret, value),
            RiscvCounterCsrWord::InstretHigh => {
                self.instret = replace_high_word(self.instret, value);
            }
        }
        Ok(())
    }

    pub fn add_cycles(&mut self, cycles: u64) {
        self.cycle = self.cycle.wrapping_add(cycles);
    }

    pub fn retire_instructions(&mut self, instructions: u64) {
        self.instret = self.instret.wrapping_add(instructions);
    }

    pub const fn snapshot(&self) -> RiscvCounterSnapshot {
        RiscvCounterSnapshot::new(self.cycle, self.instret)
    }

    pub fn restore(&mut self, snapshot: &RiscvCounterSnapshot) {
        self.cycle = snapshot.cycle;
        self.instret = snapshot.instret;
    }

    const fn read(&self, csr: RiscvCounterCsr) -> u64 {
        match csr {
            RiscvCounterCsr::Cycle => self.cycle,
            RiscvCounterCsr::Instret => self.instret,
        }
    }

    const fn read_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        let counter = self.read(csr.counter());
        match csr {
            RiscvCounterCsrWord::CycleLow | RiscvCounterCsrWord::InstretLow => counter as u32,
            RiscvCounterCsrWord::CycleHigh | RiscvCounterCsrWord::InstretHigh => {
                (counter >> 32) as u32
            }
        }
    }
}

impl Default for RiscvCounterBank {
    fn default() -> Self {
        Self::new()
    }
}

const fn replace_low_word(counter: u64, value: u32) -> u64 {
    (counter & 0xffff_ffff_0000_0000) | value as u64
}

const fn replace_high_word(counter: u64, value: u32) -> u64 {
    (counter & 0x0000_0000_ffff_ffff) | ((value as u64) << 32)
}

const fn status_bit(bits: u64, bit: u32) -> bool {
    bits & (1_u64 << bit) != 0
}

const fn set_status_bit(bits: u64, bit: u32, enabled: bool) -> u64 {
    if enabled {
        bits | (1_u64 << bit)
    } else {
        bits & !(1_u64 << bit)
    }
}
