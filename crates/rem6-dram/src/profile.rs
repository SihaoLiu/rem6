use rem6_memory::{CacheLineLayout, MemoryTargetId};

use crate::{DramControllerConfig, DramError, DramGeometry, DramTiming};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramMemoryTechnology {
    Ddr,
    Hbm,
    Lpddr,
    Nvm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramProfileField {
    Channels,
    RanksPerChannel,
    Stacks,
    PseudoChannelsPerStack,
    DiesPerChannel,
    Controllers,
    MediaBanksPerController,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalMemoryTopology {
    Ddr {
        channels: u32,
        ranks_per_channel: u32,
    },
    Hbm {
        stacks: u32,
        pseudo_channels_per_stack: u32,
    },
    Lpddr {
        channels: u32,
        dies_per_channel: u32,
    },
    Nvm {
        controllers: u32,
        media_banks_per_controller: u32,
    },
}

impl ExternalMemoryTopology {
    pub const fn parallel_port_count(self) -> u32 {
        match self {
            Self::Ddr { channels, .. } | Self::Lpddr { channels, .. } => channels,
            Self::Nvm { controllers, .. } => controllers,
            Self::Hbm {
                stacks,
                pseudo_channels_per_stack,
            } => stacks * pseudo_channels_per_stack,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalMemoryProfile {
    target: MemoryTargetId,
    line_layout: CacheLineLayout,
    geometry: DramGeometry,
    timing: DramTiming,
    technology: DramMemoryTechnology,
    topology: ExternalMemoryTopology,
}

impl ExternalMemoryProfile {
    pub fn ddr(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        channels: u32,
        ranks_per_channel: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(
            DramMemoryTechnology::Ddr,
            DramProfileField::Channels,
            channels,
        )?;
        validate_profile_count(
            DramMemoryTechnology::Ddr,
            DramProfileField::RanksPerChannel,
            ranks_per_channel,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Ddr,
            ExternalMemoryTopology::Ddr {
                channels,
                ranks_per_channel,
            },
        ))
    }

    pub fn hbm(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        stacks: u32,
        pseudo_channels_per_stack: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(DramMemoryTechnology::Hbm, DramProfileField::Stacks, stacks)?;
        validate_profile_count(
            DramMemoryTechnology::Hbm,
            DramProfileField::PseudoChannelsPerStack,
            pseudo_channels_per_stack,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Hbm,
            ExternalMemoryTopology::Hbm {
                stacks,
                pseudo_channels_per_stack,
            },
        ))
    }

    pub fn lpddr(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        channels: u32,
        dies_per_channel: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(
            DramMemoryTechnology::Lpddr,
            DramProfileField::Channels,
            channels,
        )?;
        validate_profile_count(
            DramMemoryTechnology::Lpddr,
            DramProfileField::DiesPerChannel,
            dies_per_channel,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Lpddr,
            ExternalMemoryTopology::Lpddr {
                channels,
                dies_per_channel,
            },
        ))
    }

    pub fn nvm(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        controllers: u32,
        media_banks_per_controller: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(
            DramMemoryTechnology::Nvm,
            DramProfileField::Controllers,
            controllers,
        )?;
        validate_profile_count(
            DramMemoryTechnology::Nvm,
            DramProfileField::MediaBanksPerController,
            media_banks_per_controller,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Nvm,
            ExternalMemoryTopology::Nvm {
                controllers,
                media_banks_per_controller,
            },
        ))
    }

    const fn new(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
        technology: DramMemoryTechnology,
        topology: ExternalMemoryTopology,
    ) -> Self {
        Self {
            target,
            line_layout,
            geometry,
            timing,
            technology,
            topology,
        }
    }

    pub const fn target(self) -> MemoryTargetId {
        self.target
    }

    pub const fn line_layout(self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn geometry(self) -> DramGeometry {
        self.geometry
    }

    pub const fn timing(self) -> DramTiming {
        self.timing
    }

    pub const fn technology(self) -> DramMemoryTechnology {
        self.technology
    }

    pub const fn topology(self) -> ExternalMemoryTopology {
        self.topology
    }

    pub const fn parallel_port_count(self) -> u32 {
        self.topology.parallel_port_count()
    }

    pub const fn controller_config(self) -> DramControllerConfig {
        DramControllerConfig::new(self.target, self.line_layout, self.geometry, self.timing)
            .with_profile_parallel_ports(self.parallel_port_count())
    }
}

fn validate_profile_count(
    technology: DramMemoryTechnology,
    field: DramProfileField,
    value: u32,
) -> Result<(), DramError> {
    if value == 0 {
        return Err(DramError::ZeroProfileTopology { technology, field });
    }

    Ok(())
}
