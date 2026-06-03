use crate::{RiscvCsrError, RiscvPrivilegeMode};

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
pub enum RiscvMachineTrapCsr {
    Mtvec,
    Mepc,
    Mcause,
    Mtval,
}

impl RiscvMachineTrapCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Mtvec => 0x305,
            Self::Mepc => 0x341,
            Self::Mcause => 0x342,
            Self::Mtval => 0x343,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x305 => Some(Self::Mtvec),
            0x341 => Some(Self::Mepc),
            0x342 => Some(Self::Mcause),
            0x343 => Some(Self::Mtval),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvSupervisorTrapCsr {
    Stvec,
    Sepc,
    Scause,
    Stval,
}

impl RiscvSupervisorTrapCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Stvec => 0x105,
            Self::Sepc => 0x141,
            Self::Scause => 0x142,
            Self::Stval => 0x143,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x105 => Some(Self::Stvec),
            0x141 => Some(Self::Sepc),
            0x142 => Some(Self::Scause),
            0x143 => Some(Self::Stval),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvStatusCsr {
    Mstatus,
    Sstatus,
}

impl RiscvStatusCsr {
    const S_MODE_MASK: u64 = (1 << 1) | (1 << 5) | (1 << 8) | (1 << 18) | (1 << 19);

    pub const fn address(self) -> u16 {
        match self {
            Self::Mstatus => 0x300,
            Self::Sstatus => 0x100,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x300 => Some(Self::Mstatus),
            0x100 => Some(Self::Sstatus),
            _ => None,
        }
    }

    pub const fn read(self, status: RiscvStatusWord) -> u64 {
        match self {
            Self::Mstatus => status.bits(),
            Self::Sstatus => status.bits() & Self::S_MODE_MASK,
        }
    }

    pub const fn write(self, status: RiscvStatusWord, value: u64) -> RiscvStatusWord {
        match self {
            Self::Mstatus => RiscvStatusWord::new(value),
            Self::Sstatus => RiscvStatusWord::new(
                (status.bits() & !Self::S_MODE_MASK) | (value & Self::S_MODE_MASK),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvStatusWord {
    bits: u64,
}

impl RiscvStatusWord {
    const SIE_BIT: u32 = 1;
    const MIE_BIT: u32 = 3;
    const SPIE_BIT: u32 = 5;
    const MPIE_BIT: u32 = 7;
    const SPP_BIT: u32 = 8;
    const MPP_SHIFT: u32 = 11;
    const MPP_MASK: u64 = 0b11 << Self::MPP_SHIFT;
    const MPRV_BIT: u32 = 17;
    const SUM_BIT: u32 = 18;
    const MXR_BIT: u32 = 19;

    pub const fn new(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn sie(self) -> bool {
        status_bit(self.bits, Self::SIE_BIT)
    }

    pub const fn with_sie(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::SIE_BIT, enabled);
        self
    }

    pub const fn mie(self) -> bool {
        status_bit(self.bits, Self::MIE_BIT)
    }

    pub const fn with_mie(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::MIE_BIT, enabled);
        self
    }

    pub const fn spie(self) -> bool {
        status_bit(self.bits, Self::SPIE_BIT)
    }

    pub const fn with_spie(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::SPIE_BIT, enabled);
        self
    }

    pub const fn mpie(self) -> bool {
        status_bit(self.bits, Self::MPIE_BIT)
    }

    pub const fn with_mpie(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::MPIE_BIT, enabled);
        self
    }

    pub const fn mprv(self) -> bool {
        status_bit(self.bits, Self::MPRV_BIT)
    }

    pub const fn with_mprv(mut self, enabled: bool) -> Self {
        self.bits = set_status_bit(self.bits, Self::MPRV_BIT, enabled);
        self
    }

    pub const fn spp(self) -> RiscvPrivilegeMode {
        if status_bit(self.bits, Self::SPP_BIT) {
            RiscvPrivilegeMode::Supervisor
        } else {
            RiscvPrivilegeMode::User
        }
    }

    pub const fn with_spp(mut self, privilege: RiscvPrivilegeMode) -> Self {
        self.bits = set_status_bit(
            self.bits,
            Self::SPP_BIT,
            matches!(
                privilege,
                RiscvPrivilegeMode::Supervisor | RiscvPrivilegeMode::Machine
            ),
        );
        self
    }

    pub const fn mpp(self) -> RiscvPrivilegeMode {
        match (self.bits & Self::MPP_MASK) >> Self::MPP_SHIFT {
            0 => RiscvPrivilegeMode::User,
            1 => RiscvPrivilegeMode::Supervisor,
            _ => RiscvPrivilegeMode::Machine,
        }
    }

    pub const fn with_mpp(mut self, privilege: RiscvPrivilegeMode) -> Self {
        let bits = match privilege {
            RiscvPrivilegeMode::User => 0,
            RiscvPrivilegeMode::Supervisor => 1,
            RiscvPrivilegeMode::Machine => 3,
        };
        self.bits = (self.bits & !Self::MPP_MASK) | (bits << Self::MPP_SHIFT);
        self
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
