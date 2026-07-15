use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliCachePrefetcher {
    TaggedNextLine,
}

impl CliCachePrefetcher {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "tagged-next-line" => Some(Self::TaggedNextLine),
            _ => None,
        }
    }

    pub fn parse_data_cache(value: &str) -> Result<Self, Rem6CliError> {
        Self::parse(value).ok_or_else(|| Rem6CliError::InvalidRunDataCachePrefetcher {
            value: value.to_string(),
        })
    }

    pub fn parse_instruction_cache(value: &str) -> Result<Self, Rem6CliError> {
        Self::parse(value).ok_or_else(|| Rem6CliError::InvalidRunInstructionCachePrefetcher {
            value: value.to_string(),
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaggedNextLine => "tagged-next-line",
        }
    }
}
