use crate::{Register, RiscvCsrError, RiscvPrivilegeMode, RiscvVectorFixedPointState};

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvVectorFixedPointCsr {
    Vxsat,
    Vxrm,
    Vcsr,
}

impl RiscvVectorFixedPointCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Vxsat => 0x009,
            Self::Vxrm => 0x00a,
            Self::Vcsr => 0x00f,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x009 => Some(Self::Vxsat),
            0x00a => Some(Self::Vxrm),
            0x00f => Some(Self::Vcsr),
            _ => None,
        }
    }

    pub const fn read(self, state: RiscvVectorFixedPointState) -> u64 {
        match self {
            Self::Vxsat => state.vxsat() as u64,
            Self::Vxrm => state.vxrm_bits() as u64,
            Self::Vcsr => state.vcsr_bits() as u64,
        }
    }

    pub fn write(
        self,
        mut state: RiscvVectorFixedPointState,
        value: u64,
    ) -> RiscvVectorFixedPointState {
        match self {
            Self::Vxsat => state.write_vxsat_bit(value & 0b1 != 0),
            Self::Vxrm => state.write_vxrm_bits(value as u8),
            Self::Vcsr => state.write_vcsr_bits(value as u8),
        }
        state
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCsrOp {
    Read,
    Write,
    Set,
    Clear,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCsrOperand {
    Register(Register),
    Immediate(u8),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvEnvironmentConfigCsr {
    Senvcfg,
}

impl RiscvEnvironmentConfigCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Senvcfg => 0x10a,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x10a => Some(Self::Senvcfg),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvEnvironmentConfigCsrInstruction {
    rd: Register,
    csr: RiscvEnvironmentConfigCsr,
    op: RiscvCsrOp,
    operand: RiscvCsrOperand,
}

impl RiscvEnvironmentConfigCsrInstruction {
    pub const fn read(rd: Register, csr: RiscvEnvironmentConfigCsr) -> Self {
        Self {
            rd,
            csr,
            op: RiscvCsrOp::Read,
            operand: RiscvCsrOperand::Immediate(0),
        }
    }

    pub const fn register(
        rd: Register,
        csr: RiscvEnvironmentConfigCsr,
        op: RiscvCsrOp,
        rs1: Register,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Register(rs1),
        }
    }

    pub const fn immediate(
        rd: Register,
        csr: RiscvEnvironmentConfigCsr,
        op: RiscvCsrOp,
        zimm: u8,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Immediate(zimm),
        }
    }

    pub const fn rd(self) -> Register {
        self.rd
    }

    pub const fn csr(self) -> RiscvEnvironmentConfigCsr {
        self.csr
    }

    pub const fn op(self) -> RiscvCsrOp {
        self.op
    }

    pub const fn operand(self) -> RiscvCsrOperand {
        self.operand
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvVectorFixedPointCsrInstruction {
    rd: Register,
    csr: RiscvVectorFixedPointCsr,
    op: RiscvCsrOp,
    operand: RiscvCsrOperand,
}

impl RiscvVectorFixedPointCsrInstruction {
    pub const fn read(rd: Register, csr: RiscvVectorFixedPointCsr) -> Self {
        Self {
            rd,
            csr,
            op: RiscvCsrOp::Read,
            operand: RiscvCsrOperand::Immediate(0),
        }
    }

    pub const fn register(
        rd: Register,
        csr: RiscvVectorFixedPointCsr,
        op: RiscvCsrOp,
        rs1: Register,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Register(rs1),
        }
    }

    pub const fn immediate(
        rd: Register,
        csr: RiscvVectorFixedPointCsr,
        op: RiscvCsrOp,
        zimm: u8,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Immediate(zimm),
        }
    }

    pub const fn rd(self) -> Register {
        self.rd
    }

    pub const fn csr(self) -> RiscvVectorFixedPointCsr {
        self.csr
    }

    pub const fn op(self) -> RiscvCsrOp {
        self.op
    }

    pub const fn operand(self) -> RiscvCsrOperand {
        self.operand
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCounterCsr {
    Cycle,
    Time,
    Instret,
}

impl RiscvCounterCsr {
    pub const fn user_address(self) -> u16 {
        match self {
            Self::Cycle => 0xc00,
            Self::Time => 0xc01,
            Self::Instret => 0xc02,
        }
    }

    pub const fn machine_address(self) -> Option<u16> {
        match self {
            Self::Cycle => Some(0xb00),
            Self::Time => None,
            Self::Instret => Some(0xb02),
        }
    }

    pub const fn from_user_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xc00 => Ok(Self::Cycle),
            0xc01 => Ok(Self::Time),
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
pub enum RiscvMachineIdentityCsr {
    VendorId,
    ArchitectureId,
    ImplementationId,
    HartId,
}

impl RiscvMachineIdentityCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::VendorId => 0xf11,
            Self::ArchitectureId => 0xf12,
            Self::ImplementationId => 0xf13,
            Self::HartId => 0xf14,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0xf11 => Some(Self::VendorId),
            0xf12 => Some(Self::ArchitectureId),
            0xf13 => Some(Self::ImplementationId),
            0xf14 => Some(Self::HartId),
            _ => None,
        }
    }

    pub const fn read(self, hart_id: u64) -> u64 {
        match self {
            Self::VendorId | Self::ArchitectureId | Self::ImplementationId => 0,
            Self::HartId => hart_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvMachineIsaCsr {
    Misa,
}

impl RiscvMachineIsaCsr {
    const EXTENSIONS: u64 =
        (1 << 0) | (1 << 2) | (1 << 3) | (1 << 5) | (1 << 8) | (1 << 12) | (1 << 18) | (1 << 20);

    pub const RV32_MISA: u64 = (1 << 30) | Self::EXTENSIONS;
    pub const RV64_MISA: u64 = (2 << 62) | Self::EXTENSIONS;

    pub const fn address(self) -> u16 {
        match self {
            Self::Misa => 0x301,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x301 => Some(Self::Misa),
            _ => None,
        }
    }

    pub const fn read_rv64(self) -> u64 {
        match self {
            Self::Misa => Self::RV64_MISA,
        }
    }

    pub const fn read_for_xlen_bits(self, xlen_bits: u8) -> u64 {
        match (self, xlen_bits) {
            (Self::Misa, 32) => Self::RV32_MISA,
            (Self::Misa, _) => Self::RV64_MISA,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvMachineInformationCsr {
    Identity(RiscvMachineIdentityCsr),
    Isa(RiscvMachineIsaCsr),
}

impl RiscvMachineInformationCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Identity(csr) => csr.address(),
            Self::Isa(csr) => csr.address(),
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        if let Some(csr) = RiscvMachineIdentityCsr::from_address(address) {
            return Some(Self::Identity(csr));
        }
        if let Some(csr) = RiscvMachineIsaCsr::from_address(address) {
            return Some(Self::Isa(csr));
        }
        None
    }

    pub const fn read_rv64(self, hart_id: u64) -> u64 {
        match self {
            Self::Identity(csr) => csr.read(hart_id),
            Self::Isa(csr) => csr.read_rv64(),
        }
    }

    pub const fn read_for_xlen_bits(self, hart_id: u64, xlen_bits: u8) -> u64 {
        match self {
            Self::Identity(csr) => csr.read(hart_id),
            Self::Isa(csr) => csr.read_for_xlen_bits(xlen_bits),
        }
    }

    pub const fn write_traps(self) -> bool {
        match self {
            Self::Identity(_) => true,
            Self::Isa(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvMachineInformationCsrInstruction {
    rd: Register,
    csr: RiscvMachineInformationCsr,
    op: RiscvCsrOp,
    operand: RiscvCsrOperand,
}

impl RiscvMachineInformationCsrInstruction {
    pub const fn read(rd: Register, csr: RiscvMachineInformationCsr) -> Self {
        Self {
            rd,
            csr,
            op: RiscvCsrOp::Read,
            operand: RiscvCsrOperand::Immediate(0),
        }
    }

    pub const fn register(
        rd: Register,
        csr: RiscvMachineInformationCsr,
        op: RiscvCsrOp,
        rs1: Register,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Register(rs1),
        }
    }

    pub const fn immediate(
        rd: Register,
        csr: RiscvMachineInformationCsr,
        op: RiscvCsrOp,
        zimm: u8,
    ) -> Self {
        Self {
            rd,
            csr,
            op,
            operand: RiscvCsrOperand::Immediate(zimm),
        }
    }

    pub const fn rd(self) -> Register {
        self.rd
    }

    pub const fn csr(self) -> RiscvMachineInformationCsr {
        self.csr
    }

    pub const fn op(self) -> RiscvCsrOp {
        self.op
    }

    pub const fn operand(self) -> RiscvCsrOperand {
        self.operand
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCounterCsrWord {
    CycleLow,
    CycleHigh,
    TimeLow,
    TimeHigh,
    InstretLow,
    InstretHigh,
}

impl RiscvCounterCsrWord {
    pub const fn counter(self) -> RiscvCounterCsr {
        match self {
            Self::CycleLow | Self::CycleHigh => RiscvCounterCsr::Cycle,
            Self::TimeLow | Self::TimeHigh => RiscvCounterCsr::Time,
            Self::InstretLow | Self::InstretHigh => RiscvCounterCsr::Instret,
        }
    }

    pub const fn user_address(self) -> u16 {
        match self {
            Self::CycleLow => 0xc00,
            Self::TimeLow => 0xc01,
            Self::InstretLow => 0xc02,
            Self::CycleHigh => 0xc80,
            Self::TimeHigh => 0xc81,
            Self::InstretHigh => 0xc82,
        }
    }

    pub const fn machine_address(self) -> Option<u16> {
        match self {
            Self::CycleLow => Some(0xb00),
            Self::TimeLow => None,
            Self::InstretLow => Some(0xb02),
            Self::CycleHigh => Some(0xb80),
            Self::TimeHigh => None,
            Self::InstretHigh => Some(0xb82),
        }
    }

    pub const fn from_user_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xc00 => Ok(Self::CycleLow),
            0xc01 => Ok(Self::TimeLow),
            0xc02 => Ok(Self::InstretLow),
            0xc80 => Ok(Self::CycleHigh),
            0xc81 => Ok(Self::TimeHigh),
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
pub enum RiscvInterruptCsr {
    SupervisorInterruptEnable,
    SupervisorInterruptPending,
    MachineInterruptEnable,
    MachineInterruptPending,
}

impl RiscvInterruptCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::SupervisorInterruptEnable => 0x104,
            Self::SupervisorInterruptPending => 0x144,
            Self::MachineInterruptEnable => 0x304,
            Self::MachineInterruptPending => 0x344,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x104 => Some(Self::SupervisorInterruptEnable),
            0x144 => Some(Self::SupervisorInterruptPending),
            0x304 => Some(Self::MachineInterruptEnable),
            0x344 => Some(Self::MachineInterruptPending),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvMachineTrapCsr {
    Medeleg,
    Mideleg,
    Mtvec,
    Mscratch,
    Mepc,
    Mcause,
    Mtval,
}

impl RiscvMachineTrapCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Medeleg => 0x302,
            Self::Mideleg => 0x303,
            Self::Mtvec => 0x305,
            Self::Mscratch => 0x340,
            Self::Mepc => 0x341,
            Self::Mcause => 0x342,
            Self::Mtval => 0x343,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x302 => Some(Self::Medeleg),
            0x303 => Some(Self::Mideleg),
            0x305 => Some(Self::Mtvec),
            0x340 => Some(Self::Mscratch),
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
    Sscratch,
    Sepc,
    Scause,
    Stval,
}

impl RiscvSupervisorTrapCsr {
    pub const fn address(self) -> u16 {
        match self {
            Self::Stvec => 0x105,
            Self::Sscratch => 0x140,
            Self::Sepc => 0x141,
            Self::Scause => 0x142,
            Self::Stval => 0x143,
        }
    }

    pub const fn from_address(address: u16) -> Option<Self> {
        match address {
            0x105 => Some(Self::Stvec),
            0x140 => Some(Self::Sscratch),
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
    time: u64,
    instret: u64,
}

impl RiscvCounterSnapshot {
    pub const fn new(cycle: u64, instret: u64) -> Self {
        Self {
            cycle,
            time: cycle,
            instret,
        }
    }

    pub const fn with_time(cycle: u64, time: u64, instret: u64) -> Self {
        Self {
            cycle,
            time,
            instret,
        }
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub const fn time(&self) -> u64 {
        self.time
    }

    pub const fn instret(&self) -> u64 {
        self.instret
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCounterBank {
    cycle: u64,
    time: u64,
    instret: u64,
}

impl RiscvCounterBank {
    pub const fn new() -> Self {
        Self {
            cycle: 0,
            time: 0,
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
        self.set_machine(csr, value)
    }

    pub fn set_machine(&mut self, csr: RiscvCounterCsr, value: u64) -> Result<(), RiscvCsrError> {
        match csr {
            RiscvCounterCsr::Cycle => self.cycle = value,
            RiscvCounterCsr::Time => return Err(RiscvCsrError::ReadOnlyCounterAlias { csr }),
            RiscvCounterCsr::Instret => self.instret = value,
        }
        Ok(())
    }

    pub fn write_machine_word(
        &mut self,
        csr: RiscvCounterCsrWord,
        value: u32,
    ) -> Result<(), RiscvCsrError> {
        match csr {
            RiscvCounterCsrWord::CycleLow => self.cycle = replace_low_word(self.cycle, value),
            RiscvCounterCsrWord::CycleHigh => self.cycle = replace_high_word(self.cycle, value),
            RiscvCounterCsrWord::TimeLow | RiscvCounterCsrWord::TimeHigh => {
                return Err(RiscvCsrError::ReadOnlyCounterWordAlias { csr });
            }
            RiscvCounterCsrWord::InstretLow => self.instret = replace_low_word(self.instret, value),
            RiscvCounterCsrWord::InstretHigh => {
                self.instret = replace_high_word(self.instret, value);
            }
        }
        Ok(())
    }

    pub fn add_cycles(&mut self, cycles: u64) {
        self.cycle = self.cycle.wrapping_add(cycles);
        self.time = self.time.wrapping_add(cycles);
    }

    pub fn retire_instructions(&mut self, instructions: u64) {
        self.instret = self.instret.wrapping_add(instructions);
    }

    pub const fn snapshot(&self) -> RiscvCounterSnapshot {
        RiscvCounterSnapshot::with_time(self.cycle, self.time, self.instret)
    }

    pub fn restore(&mut self, snapshot: &RiscvCounterSnapshot) {
        self.cycle = snapshot.cycle;
        self.time = snapshot.time;
        self.instret = snapshot.instret;
    }

    const fn read(&self, csr: RiscvCounterCsr) -> u64 {
        match csr {
            RiscvCounterCsr::Cycle => self.cycle,
            RiscvCounterCsr::Time => self.time,
            RiscvCounterCsr::Instret => self.instret,
        }
    }

    const fn read_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        let counter = self.read(csr.counter());
        match csr {
            RiscvCounterCsrWord::CycleLow
            | RiscvCounterCsrWord::TimeLow
            | RiscvCounterCsrWord::InstretLow => counter as u32,
            RiscvCounterCsrWord::CycleHigh
            | RiscvCounterCsrWord::TimeHigh
            | RiscvCounterCsrWord::InstretHigh => (counter >> 32) as u32,
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
