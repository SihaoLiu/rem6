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
pub enum NvmMediaTimingField {
    ReadMediaLatency,
    WriteMediaLatency,
    SendLatency,
    MaxPendingReads,
    MaxPendingWrites,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NvmMediaTiming {
    read_media_latency: u64,
    write_media_latency: u64,
    send_latency: u64,
    max_pending_reads: u32,
    max_pending_writes: u32,
}

impl NvmMediaTiming {
    pub fn new(
        read_media_latency: u64,
        write_media_latency: u64,
        send_latency: u64,
        max_pending_reads: u32,
        max_pending_writes: u32,
    ) -> Result<Self, DramError> {
        validate_nvm_media_u64(NvmMediaTimingField::ReadMediaLatency, read_media_latency)?;
        validate_nvm_media_u64(NvmMediaTimingField::WriteMediaLatency, write_media_latency)?;
        validate_nvm_media_u64(NvmMediaTimingField::SendLatency, send_latency)?;
        validate_nvm_media_u32(NvmMediaTimingField::MaxPendingReads, max_pending_reads)?;
        validate_nvm_media_u32(NvmMediaTimingField::MaxPendingWrites, max_pending_writes)?;
        Ok(Self {
            read_media_latency,
            write_media_latency,
            send_latency,
            max_pending_reads,
            max_pending_writes,
        })
    }

    pub const fn read_media_latency(self) -> u64 {
        self.read_media_latency
    }

    pub const fn write_media_latency(self) -> u64 {
        self.write_media_latency
    }

    pub const fn send_latency(self) -> u64 {
        self.send_latency
    }

    pub const fn max_pending_reads(self) -> u32 {
        self.max_pending_reads
    }

    pub const fn max_pending_writes(self) -> u32 {
        self.max_pending_writes
    }
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
    nvm_media_timing: Option<NvmMediaTiming>,
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
            None,
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
            None,
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
            None,
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
            None,
        ))
    }

    const fn new(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
        technology: DramMemoryTechnology,
        topology: ExternalMemoryTopology,
        nvm_media_timing: Option<NvmMediaTiming>,
    ) -> Self {
        Self {
            target,
            line_layout,
            geometry,
            timing,
            technology,
            topology,
            nvm_media_timing,
        }
    }

    pub fn with_nvm_media_timing(
        mut self,
        nvm_media_timing: NvmMediaTiming,
    ) -> Result<Self, DramError> {
        if self.technology != DramMemoryTechnology::Nvm {
            return Err(DramError::NvmMediaTimingOnVolatileProfile {
                technology: self.technology,
            });
        }
        self.nvm_media_timing = Some(nvm_media_timing);
        Ok(self)
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

    pub const fn nvm_media_timing(self) -> Option<NvmMediaTiming> {
        self.nvm_media_timing
    }

    pub const fn parallel_port_count(self) -> u32 {
        self.topology.parallel_port_count()
    }

    pub const fn controller_config(self) -> DramControllerConfig {
        let config =
            DramControllerConfig::new(self.target, self.line_layout, self.geometry, self.timing)
                .with_profile_parallel_ports(self.parallel_port_count());
        match self.nvm_media_timing {
            Some(nvm_media_timing) => config.with_nvm_media_timing(nvm_media_timing),
            None => config,
        }
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

fn validate_nvm_media_u64(field: NvmMediaTimingField, value: u64) -> Result<(), DramError> {
    if value == 0 {
        return Err(DramError::ZeroNvmMediaTiming { field });
    }

    Ok(())
}

fn validate_nvm_media_u32(field: NvmMediaTimingField, value: u32) -> Result<(), DramError> {
    if value == 0 {
        return Err(DramError::ZeroNvmMediaTiming { field });
    }

    Ok(())
}
