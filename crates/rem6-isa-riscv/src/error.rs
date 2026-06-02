use std::error::Error;
use std::fmt;

use crate::{RiscvCounterCsr, RiscvCounterCsrWord};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvError {
    InvalidRegister { index: u8 },
    CompressedNotSupported { raw: u32 },
    UnknownEncoding { raw: u32 },
    PcOverflow { pc: u64, offset: u64 },
    AddressOverflow { value: u64, offset: i64 },
}

impl fmt::Display for RiscvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegister { index } => {
                write!(formatter, "register index {index} is outside x0..x31")
            }
            Self::CompressedNotSupported { raw } => {
                write!(
                    formatter,
                    "compressed instruction {raw:#010x} is not supported"
                )
            }
            Self::UnknownEncoding { raw } => {
                write!(formatter, "instruction {raw:#010x} is not supported")
            }
            Self::PcOverflow { pc, offset } => {
                write!(formatter, "pc {pc:#x} overflows by {offset} bytes")
            }
            Self::AddressOverflow { value, offset } => {
                write!(
                    formatter,
                    "address {value:#x} overflows with offset {offset}"
                )
            }
        }
    }
}

impl Error for RiscvError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCsrError {
    UnknownCounterCsr { address: u16 },
    ReadOnlyCounterAlias { csr: RiscvCounterCsr },
    ReadOnlyCounterWordAlias { csr: RiscvCounterCsrWord },
}

impl fmt::Display for RiscvCsrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCounterCsr { address } => {
                write!(
                    formatter,
                    "RISC-V counter CSR {address:#05x} is not supported"
                )
            }
            Self::ReadOnlyCounterAlias { csr } => write!(
                formatter,
                "RISC-V user counter CSR {:#05x} is read-only",
                csr.user_address()
            ),
            Self::ReadOnlyCounterWordAlias { csr } => write!(
                formatter,
                "RISC-V user counter CSR {:#05x} is read-only",
                csr.user_address()
            ),
        }
    }
}

impl Error for RiscvCsrError {}
