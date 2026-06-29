use super::{
    CliDramLowPowerTiming, CliDramMemoryProfile, CliDramRefreshTiming, CliDramTiming, Rem6RunConfig,
};

impl Rem6RunConfig {
    pub const fn dram_memory(&self) -> bool {
        self.dram_memory
    }

    pub const fn dram_memory_profile(&self) -> CliDramMemoryProfile {
        self.dram_memory_profile
    }

    pub const fn dram_timing(&self) -> CliDramTiming {
        self.dram_timing
    }

    pub const fn dram_low_power_timing(&self) -> CliDramLowPowerTiming {
        self.dram_low_power_timing
    }

    pub const fn dram_refresh_timing(&self) -> Option<CliDramRefreshTiming> {
        self.dram_refresh_timing
    }
}
