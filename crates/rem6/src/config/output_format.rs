use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatsFormat {
    Json,
    Text,
}

impl StatsFormat {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "json" => Ok(Self::Json),
            "text" => Ok(Self::Text),
            _ => Err(Rem6CliError::UnsupportedStatsFormat {
                format: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Text => "text",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerAnalysisFormat {
    McpatXml,
    DsentCsv,
}

impl PowerAnalysisFormat {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "mcpat-xml" => Ok(Self::McpatXml),
            "dsent-csv" => Ok(Self::DsentCsv),
            _ => Err(Rem6CliError::UnsupportedPowerAnalysisFormat {
                format: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::McpatXml => "mcpat-xml",
            Self::DsentCsv => "dsent-csv",
        }
    }
}
