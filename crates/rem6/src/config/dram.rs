use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliDramMemoryProfile {
    Ddr,
    Ddr4_2400_8Gb,
    Ddr5_4800_16Gb,
    Hbm,
    Hbm2_2000_2Gb,
    Lpddr,
    Lpddr4_3200_16Gb,
    Nvm,
}

impl CliDramMemoryProfile {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "ddr" => Ok(Self::Ddr),
            "ddr4-2400-8gb" => Ok(Self::Ddr4_2400_8Gb),
            "ddr5-4800-16gb" => Ok(Self::Ddr5_4800_16Gb),
            "hbm" => Ok(Self::Hbm),
            "hbm2-2000-2gb" => Ok(Self::Hbm2_2000_2Gb),
            "lpddr" => Ok(Self::Lpddr),
            "lpddr4-3200-16gb" => Ok(Self::Lpddr4_3200_16Gb),
            "nvm" => Ok(Self::Nvm),
            _ => Err(Rem6CliError::UnsupportedDramMemoryProfile {
                profile: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ddr => "ddr",
            Self::Ddr4_2400_8Gb => "ddr4-2400-8gb",
            Self::Ddr5_4800_16Gb => "ddr5-4800-16gb",
            Self::Hbm => "hbm",
            Self::Hbm2_2000_2Gb => "hbm2-2000-2gb",
            Self::Lpddr => "lpddr",
            Self::Lpddr4_3200_16Gb => "lpddr4-3200-16gb",
            Self::Nvm => "nvm",
        }
    }
}
