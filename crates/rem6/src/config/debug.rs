use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CliDebugFlag {
    Data,
    Exec,
    Fetch,
    Memory,
    Power,
    Syscall,
}

impl CliDebugFlag {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        if value.is_empty() {
            return Err(Rem6CliError::EmptyDebugFlag);
        }
        match value {
            "Data" => Ok(Self::Data),
            "Exec" => Ok(Self::Exec),
            "Fetch" => Ok(Self::Fetch),
            "Memory" => Ok(Self::Memory),
            "Power" => Ok(Self::Power),
            "Syscall" => Ok(Self::Syscall),
            _ => Err(Rem6CliError::UnsupportedDebugFlag {
                flag: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Data => "Data",
            Self::Exec => "Exec",
            Self::Fetch => "Fetch",
            Self::Memory => "Memory",
            Self::Power => "Power",
            Self::Syscall => "Syscall",
        }
    }
}

pub(super) fn parse_debug_flags(values: &[String]) -> Result<Vec<CliDebugFlag>, Rem6CliError> {
    let mut flags = Vec::new();
    for value in values {
        flags.extend(parse_debug_flag_list(value)?);
    }
    flags.sort_unstable();
    flags.dedup();
    Ok(flags)
}

pub(super) fn parse_debug_flag_list(value: &str) -> Result<Vec<CliDebugFlag>, Rem6CliError> {
    value.split(',').map(CliDebugFlag::parse).collect()
}
