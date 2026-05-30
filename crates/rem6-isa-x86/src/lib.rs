use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86PrivilegeLevel(u8);

impl X86PrivilegeLevel {
    pub fn new(level: u8) -> Result<Self, X86PrivilegeError> {
        if level <= 3 {
            Ok(Self(level))
        } else {
            Err(X86PrivilegeError::InvalidPrivilegeLevel { level })
        }
    }

    pub const fn ring0() -> Self {
        Self(0)
    }

    pub const fn ring3() -> Self {
        Self(3)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86PrivilegeError {
    InvalidPrivilegeLevel { level: u8 },
}

impl fmt::Display for X86PrivilegeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPrivilegeLevel { level } => {
                write!(formatter, "invalid x86 privilege level {level}")
            }
        }
    }
}

impl Error for X86PrivilegeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86Rflags {
    bits: u64,
}

impl X86Rflags {
    pub const CARRY: u64 = 1 << 0;
    pub const RESERVED_BIT_1: u64 = 1 << 1;
    pub const IF: u64 = 1 << 9;
    pub const IOPL_MASK: u64 = 0b11 << 12;
    pub const VIF: u64 = 1 << 19;
    pub const VIP: u64 = 1 << 20;

    pub const fn new(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn carry(self) -> bool {
        self.bits & Self::CARRY != 0
    }

    pub const fn interrupt_flag(self) -> bool {
        self.bits & Self::IF != 0
    }

    pub const fn iopl(self) -> u8 {
        ((self.bits & Self::IOPL_MASK) >> 12) as u8
    }

    pub const fn virtual_interrupt_flag(self) -> bool {
        self.bits & Self::VIF != 0
    }

    pub const fn virtual_interrupt_pending(self) -> bool {
        self.bits & Self::VIP != 0
    }

    pub const fn with_iopl(self, iopl: u8) -> Self {
        Self {
            bits: (self.bits & !Self::IOPL_MASK) | (((iopl as u64) & 0b11) << 12),
        }
    }

    pub const fn with_carry(self, enabled: bool) -> Self {
        self.with_bit(Self::CARRY, enabled)
    }

    pub const fn with_interrupt_flag(self, enabled: bool) -> Self {
        self.with_bit(Self::IF, enabled)
    }

    pub const fn with_virtual_interrupt_flag(self, enabled: bool) -> Self {
        self.with_bit(Self::VIF, enabled)
    }

    pub const fn with_virtual_interrupt_pending(self, enabled: bool) -> Self {
        self.with_bit(Self::VIP, enabled)
    }

    const fn with_bit(self, mask: u64, enabled: bool) -> Self {
        if enabled {
            Self {
                bits: self.bits | mask,
            }
        } else {
            Self {
                bits: self.bits & !mask,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86ControlRegister4 {
    bits: u64,
}

impl X86ControlRegister4 {
    pub const PVI: u64 = 1 << 1;

    pub const fn new(bits: u64) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn pvi(self) -> bool {
        self.bits & Self::PVI != 0
    }

    pub const fn with_pvi(self, enabled: bool) -> Self {
        if enabled {
            Self {
                bits: self.bits | Self::PVI,
            }
        } else {
            Self {
                bits: self.bits & !Self::PVI,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X86InterruptFlagOperation {
    Sti,
    Cli,
}

impl X86InterruptFlagOperation {
    pub fn apply_protected(
        self,
        cpl: X86PrivilegeLevel,
        cr4: X86ControlRegister4,
        rflags: X86Rflags,
    ) -> Result<X86InterruptFlagOutcome, X86InterruptFlagError> {
        if cpl.get() <= rflags.iopl() {
            return Ok(match self {
                Self::Sti => X86InterruptFlagOutcome::InterruptFlagSet {
                    rflags: rflags.with_interrupt_flag(true),
                },
                Self::Cli => X86InterruptFlagOutcome::InterruptFlagCleared {
                    rflags: rflags.with_interrupt_flag(false),
                },
            });
        }

        if !cr4.pvi() || cpl != X86PrivilegeLevel::ring3() {
            return Err(X86InterruptFlagError::GeneralProtection { code: 0 });
        }

        match self {
            Self::Sti => {
                if rflags.virtual_interrupt_pending() {
                    Err(X86InterruptFlagError::GeneralProtection { code: 0 })
                } else {
                    Ok(X86InterruptFlagOutcome::VirtualInterruptFlagSet {
                        rflags: rflags.with_virtual_interrupt_flag(true),
                    })
                }
            }
            Self::Cli => Ok(X86InterruptFlagOutcome::VirtualInterruptFlagCleared {
                rflags: rflags.with_virtual_interrupt_flag(false),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X86InterruptFlagOutcome {
    InterruptFlagSet { rflags: X86Rflags },
    InterruptFlagCleared { rflags: X86Rflags },
    VirtualInterruptFlagSet { rflags: X86Rflags },
    VirtualInterruptFlagCleared { rflags: X86Rflags },
}

impl X86InterruptFlagOutcome {
    pub const fn rflags(self) -> X86Rflags {
        match self {
            Self::InterruptFlagSet { rflags }
            | Self::InterruptFlagCleared { rflags }
            | Self::VirtualInterruptFlagSet { rflags }
            | Self::VirtualInterruptFlagCleared { rflags } => rflags,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86InterruptFlagError {
    GeneralProtection { code: u16 },
}

impl fmt::Display for X86InterruptFlagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneralProtection { code } => {
                write!(formatter, "x86 general protection fault with code {code}")
            }
        }
    }
}

impl Error for X86InterruptFlagError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X86InstructionMode {
    Long64,
    Protected32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X86SegmentOverride {
    ES,
    CS,
    SS,
    DS,
    FS,
    GS,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct X86LegacyPrefixes {
    segment: Option<X86SegmentOverride>,
    operand_size_override: bool,
    address_size_override: bool,
    lock: bool,
    rep: bool,
    repne: bool,
}

impl X86LegacyPrefixes {
    pub const fn segment(self) -> Option<X86SegmentOverride> {
        self.segment
    }

    pub const fn operand_size_override(self) -> bool {
        self.operand_size_override
    }

    pub const fn address_size_override(self) -> bool {
        self.address_size_override
    }

    pub const fn lock(self) -> bool {
        self.lock
    }

    pub const fn rep(self) -> bool {
        self.rep
    }

    pub const fn repne(self) -> bool {
        self.repne
    }

    fn apply(&mut self, prefix: X86LegacyPrefixKind) {
        match prefix {
            X86LegacyPrefixKind::Segment(segment) => self.segment = Some(segment),
            X86LegacyPrefixKind::OperandSizeOverride => self.operand_size_override = true,
            X86LegacyPrefixKind::AddressSizeOverride => self.address_size_override = true,
            X86LegacyPrefixKind::Lock => self.lock = true,
            X86LegacyPrefixKind::Rep => self.rep = true,
            X86LegacyPrefixKind::Repne => self.repne = true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86RexPrefix {
    byte: u8,
    offset: usize,
}

impl X86RexPrefix {
    const fn new(byte: u8, offset: usize) -> Self {
        Self { byte, offset }
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn w(self) -> bool {
        self.byte & 0x08 != 0
    }

    pub const fn r(self) -> bool {
        self.byte & 0x04 != 0
    }

    pub const fn x(self) -> bool {
        self.byte & 0x02 != 0
    }

    pub const fn b(self) -> bool {
        self.byte & 0x01 != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X86IgnoredRexReason {
    InterruptedByLegacyPrefix,
    SupersededByLaterRex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86IgnoredRexPrefix {
    byte: u8,
    offset: usize,
    reason: X86IgnoredRexReason,
}

impl X86IgnoredRexPrefix {
    const fn new(rex: X86RexPrefix, reason: X86IgnoredRexReason) -> Self {
        Self {
            byte: rex.byte,
            offset: rex.offset,
            reason,
        }
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn reason(self) -> X86IgnoredRexReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X86OpcodeMap {
    OneByte,
    TwoByte,
    ThreeByte0F38,
    ThreeByte0F3A,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X86PrefixScan {
    mode: X86InstructionMode,
    legacy_prefixes: X86LegacyPrefixes,
    rex: Option<X86RexPrefix>,
    ignored_rex_prefixes: Vec<X86IgnoredRexPrefix>,
    opcode_start: usize,
    opcode_map: X86OpcodeMap,
    opcode: u8,
}

impl X86PrefixScan {
    pub fn scan(mode: X86InstructionMode, bytes: &[u8]) -> Result<Self, X86DecodeError> {
        if bytes.is_empty() {
            return Err(X86DecodeError::EmptyInstruction);
        }

        let mut legacy_prefixes = X86LegacyPrefixes::default();
        let mut ignored_rex_prefixes = Vec::new();
        let mut pending_rex = None;
        let mut index = 0;

        while let Some(&byte) = bytes.get(index) {
            if let Some(prefix) = legacy_prefix_kind(byte) {
                if let Some(rex) = pending_rex.take() {
                    ignored_rex_prefixes.push(X86IgnoredRexPrefix::new(
                        rex,
                        X86IgnoredRexReason::InterruptedByLegacyPrefix,
                    ));
                }
                legacy_prefixes.apply(prefix);
                index += 1;
                continue;
            }

            if mode == X86InstructionMode::Long64 && is_rex_prefix(byte) {
                if let Some(rex) = pending_rex.replace(X86RexPrefix::new(byte, index)) {
                    ignored_rex_prefixes.push(X86IgnoredRexPrefix::new(
                        rex,
                        X86IgnoredRexReason::SupersededByLaterRex,
                    ));
                }
                index += 1;
                continue;
            }

            break;
        }

        let (opcode_map, opcode) = decode_opcode_map(bytes, index)?;
        Ok(Self {
            mode,
            legacy_prefixes,
            rex: pending_rex,
            ignored_rex_prefixes,
            opcode_start: index,
            opcode_map,
            opcode,
        })
    }

    pub const fn mode(&self) -> X86InstructionMode {
        self.mode
    }

    pub const fn legacy_prefixes(&self) -> X86LegacyPrefixes {
        self.legacy_prefixes
    }

    pub const fn rex(&self) -> Option<X86RexPrefix> {
        self.rex
    }

    pub fn ignored_rex_prefixes(&self) -> &[X86IgnoredRexPrefix] {
        &self.ignored_rex_prefixes
    }

    pub const fn opcode_start(&self) -> usize {
        self.opcode_start
    }

    pub const fn opcode_map(&self) -> X86OpcodeMap {
        self.opcode_map
    }

    pub const fn opcode(&self) -> u8 {
        self.opcode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum X86DecodeError {
    EmptyInstruction,
    MissingOpcode { after_prefix_bytes: usize },
    MissingOpcodeAfterEscape { escape_offset: usize, escape: u8 },
}

impl fmt::Display for X86DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInstruction => formatter.write_str("empty x86 instruction byte stream"),
            Self::MissingOpcode { after_prefix_bytes } => write!(
                formatter,
                "x86 instruction has no opcode after {after_prefix_bytes} prefix bytes"
            ),
            Self::MissingOpcodeAfterEscape {
                escape_offset,
                escape,
            } => write!(
                formatter,
                "x86 instruction has no opcode after escape byte {escape:#04x} at offset {escape_offset}"
            ),
        }
    }
}

impl Error for X86DecodeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum X86LegacyPrefixKind {
    Segment(X86SegmentOverride),
    OperandSizeOverride,
    AddressSizeOverride,
    Lock,
    Rep,
    Repne,
}

fn legacy_prefix_kind(byte: u8) -> Option<X86LegacyPrefixKind> {
    match byte {
        0x26 => Some(X86LegacyPrefixKind::Segment(X86SegmentOverride::ES)),
        0x2e => Some(X86LegacyPrefixKind::Segment(X86SegmentOverride::CS)),
        0x36 => Some(X86LegacyPrefixKind::Segment(X86SegmentOverride::SS)),
        0x3e => Some(X86LegacyPrefixKind::Segment(X86SegmentOverride::DS)),
        0x64 => Some(X86LegacyPrefixKind::Segment(X86SegmentOverride::FS)),
        0x65 => Some(X86LegacyPrefixKind::Segment(X86SegmentOverride::GS)),
        0x66 => Some(X86LegacyPrefixKind::OperandSizeOverride),
        0x67 => Some(X86LegacyPrefixKind::AddressSizeOverride),
        0xf0 => Some(X86LegacyPrefixKind::Lock),
        0xf2 => Some(X86LegacyPrefixKind::Repne),
        0xf3 => Some(X86LegacyPrefixKind::Rep),
        _ => None,
    }
}

const fn is_rex_prefix(byte: u8) -> bool {
    byte >= 0x40 && byte <= 0x4f
}

fn decode_opcode_map(
    bytes: &[u8],
    opcode_start: usize,
) -> Result<(X86OpcodeMap, u8), X86DecodeError> {
    let first = *bytes
        .get(opcode_start)
        .ok_or(X86DecodeError::MissingOpcode {
            after_prefix_bytes: opcode_start,
        })?;

    if first != 0x0f {
        return Ok((X86OpcodeMap::OneByte, first));
    }

    let second = *bytes
        .get(opcode_start + 1)
        .ok_or(X86DecodeError::MissingOpcodeAfterEscape {
            escape_offset: opcode_start,
            escape: first,
        })?;

    match second {
        0x38 => {
            let opcode =
                *bytes
                    .get(opcode_start + 2)
                    .ok_or(X86DecodeError::MissingOpcodeAfterEscape {
                        escape_offset: opcode_start + 1,
                        escape: second,
                    })?;
            Ok((X86OpcodeMap::ThreeByte0F38, opcode))
        }
        0x3a => {
            let opcode =
                *bytes
                    .get(opcode_start + 2)
                    .ok_or(X86DecodeError::MissingOpcodeAfterEscape {
                        escape_offset: opcode_start + 1,
                        escape: second,
                    })?;
            Ok((X86OpcodeMap::ThreeByte0F3A, opcode))
        }
        opcode => Ok((X86OpcodeMap::TwoByte, opcode)),
    }
}
