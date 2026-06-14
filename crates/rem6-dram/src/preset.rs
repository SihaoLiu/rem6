use rem6_memory::{CacheLineLayout, MemoryTargetId};

use crate::{
    DramError, DramGeometry, DramMemoryTechnology, DramRefreshTiming, DramTiming,
    ExternalMemoryProfile,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramJedecRefreshPreset {
    Ddr4_2400_8Gb,
    Ddr5_4800_16Gb,
    Hbm2_2000_2Gb,
}

impl DramJedecRefreshPreset {
    pub const fn technology(self) -> DramMemoryTechnology {
        self.spec().technology
    }

    pub const fn clock_mhz(self) -> u32 {
        self.spec().clock_mhz
    }

    pub const fn t_refi_ps(self) -> u64 {
        self.spec().t_refi_ps
    }

    pub const fn t_rfc_ps(self) -> u64 {
        self.spec().t_rfc_ps
    }

    pub const fn refresh_timing(self) -> Result<DramRefreshTiming, DramError> {
        self.spec().refresh_timing()
    }

    const fn spec(self) -> DramJedecRefreshSpec {
        match self {
            Self::Ddr4_2400_8Gb => {
                DramJedecRefreshSpec::new(DramMemoryTechnology::Ddr, 1_200, 7_800_000, 350_000)
            }
            Self::Ddr5_4800_16Gb => {
                DramJedecRefreshSpec::new(DramMemoryTechnology::Ddr, 2_400, 3_900_000, 295_000)
            }
            Self::Hbm2_2000_2Gb => {
                DramJedecRefreshSpec::new(DramMemoryTechnology::Hbm, 1_000, 3_900_000, 220_000)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DramJedecRefreshSpec {
    technology: DramMemoryTechnology,
    clock_mhz: u32,
    t_refi_ps: u64,
    t_rfc_ps: u64,
}

impl DramJedecRefreshSpec {
    const fn new(
        technology: DramMemoryTechnology,
        clock_mhz: u32,
        t_refi_ps: u64,
        t_rfc_ps: u64,
    ) -> Self {
        Self {
            technology,
            clock_mhz,
            t_refi_ps,
            t_rfc_ps,
        }
    }

    const fn refresh_timing(self) -> Result<DramRefreshTiming, DramError> {
        DramRefreshTiming::new(
            cycles_from_ps(self.t_refi_ps, self.clock_mhz),
            cycles_from_ps(self.t_rfc_ps, self.clock_mhz),
        )
    }
}

impl DramTiming {
    pub fn with_jedec_refresh_preset(
        self,
        preset: DramJedecRefreshPreset,
    ) -> Result<Self, DramError> {
        self.with_refresh_timing(preset.refresh_timing()?)
    }
}

impl ExternalMemoryProfile {
    pub fn ddr4_2400_8gb(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        channels: u32,
        ranks_per_channel: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        Self::ddr(
            target,
            line_layout,
            channels,
            ranks_per_channel,
            geometry,
            timing.with_jedec_refresh_preset(DramJedecRefreshPreset::Ddr4_2400_8Gb)?,
        )
    }

    pub fn ddr5_4800_16gb(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        channels: u32,
        ranks_per_channel: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        Self::ddr(
            target,
            line_layout,
            channels,
            ranks_per_channel,
            geometry,
            timing.with_jedec_refresh_preset(DramJedecRefreshPreset::Ddr5_4800_16Gb)?,
        )
    }

    pub fn hbm2_2000_2gb(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        stacks: u32,
        pseudo_channels_per_stack: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        Self::hbm(
            target,
            line_layout,
            stacks,
            pseudo_channels_per_stack,
            geometry,
            timing.with_jedec_refresh_preset(DramJedecRefreshPreset::Hbm2_2000_2Gb)?,
        )
    }
}

const fn cycles_from_ps(time_ps: u64, clock_mhz: u32) -> u64 {
    time_ps
        .saturating_mul(clock_mhz as u64)
        .saturating_add(999_999)
        / 1_000_000
}
