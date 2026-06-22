use rem6_boot::BootElfArchitecture;

use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestedIsa {
    Riscv,
    X86,
}

impl RequestedIsa {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "riscv" => Ok(Self::Riscv),
            "x86" => Ok(Self::X86),
            _ => Err(Rem6CliError::UnsupportedIsa {
                isa: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Riscv => "riscv",
            Self::X86 => "x86",
        }
    }

    pub(crate) const fn matches_architecture(self, architecture: BootElfArchitecture) -> bool {
        matches!(
            (self, architecture),
            (
                Self::Riscv,
                BootElfArchitecture::Riscv32 | BootElfArchitecture::Riscv64
            ) | (
                Self::X86,
                BootElfArchitecture::I386 | BootElfArchitecture::X8664
            )
        )
    }
}
