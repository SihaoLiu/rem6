use std::error::Error;
use std::fmt;

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
