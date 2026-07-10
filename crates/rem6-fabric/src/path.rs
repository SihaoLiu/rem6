use rem6_kernel::{ClockDomain, Cycles, Tick};

use crate::types::{FabricError, FabricLinkId, FabricRouterId, VirtualNetworkId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FabricSerialLinkRate {
    BitsPerCycle {
        lane_speed_bits_per_cycle: u64,
        bits_per_cycle: u64,
    },
    BitsPerNanosecond {
        lane_speed_bits_per_nanosecond: u64,
        ticks_per_nanosecond: u64,
        bits_per_nanosecond: u64,
    },
}

impl FabricSerialLinkRate {
    fn from_bits_per_cycle(
        lanes: u64,
        lane_speed_bits_per_cycle: u64,
    ) -> Result<Self, FabricError> {
        if lane_speed_bits_per_cycle == 0 {
            return Err(FabricError::ZeroSerialLinkLaneSpeed);
        }
        let bits_per_cycle = lanes.checked_mul(lane_speed_bits_per_cycle).ok_or(
            FabricError::SerialLinkBandwidthOverflow {
                lanes,
                lane_speed_bits_per_cycle,
            },
        )?;

        Ok(Self::BitsPerCycle {
            lane_speed_bits_per_cycle,
            bits_per_cycle,
        })
    }

    fn from_bits_per_nanosecond(
        lanes: u64,
        lane_speed_bits_per_nanosecond: u64,
        ticks_per_nanosecond: u64,
    ) -> Result<Self, FabricError> {
        if lane_speed_bits_per_nanosecond == 0 {
            return Err(FabricError::ZeroSerialLinkLaneSpeed);
        }
        if ticks_per_nanosecond == 0 {
            return Err(FabricError::ZeroSerialLinkTicksPerNanosecond);
        }
        let bits_per_nanosecond = lanes.checked_mul(lane_speed_bits_per_nanosecond).ok_or(
            FabricError::SerialLinkNanosecondBandwidthOverflow {
                lanes,
                lane_speed_bits_per_nanosecond,
            },
        )?;

        Ok(Self::BitsPerNanosecond {
            lane_speed_bits_per_nanosecond,
            ticks_per_nanosecond,
            bits_per_nanosecond,
        })
    }

    pub const fn lane_speed_bits_per_cycle(self) -> Option<u64> {
        match self {
            Self::BitsPerCycle {
                lane_speed_bits_per_cycle,
                ..
            } => Some(lane_speed_bits_per_cycle),
            Self::BitsPerNanosecond { .. } => None,
        }
    }

    pub const fn bits_per_cycle(self) -> Option<u64> {
        match self {
            Self::BitsPerCycle { bits_per_cycle, .. } => Some(bits_per_cycle),
            Self::BitsPerNanosecond { .. } => None,
        }
    }

    pub const fn lane_speed_bits_per_nanosecond(self) -> Option<u64> {
        match self {
            Self::BitsPerCycle { .. } => None,
            Self::BitsPerNanosecond {
                lane_speed_bits_per_nanosecond,
                ..
            } => Some(lane_speed_bits_per_nanosecond),
        }
    }

    pub const fn ticks_per_nanosecond(self) -> Option<u64> {
        match self {
            Self::BitsPerCycle { .. } => None,
            Self::BitsPerNanosecond {
                ticks_per_nanosecond,
                ..
            } => Some(ticks_per_nanosecond),
        }
    }

    pub const fn bits_per_nanosecond(self) -> Option<u64> {
        match self {
            Self::BitsPerCycle { .. } => None,
            Self::BitsPerNanosecond {
                bits_per_nanosecond,
                ..
            } => Some(bits_per_nanosecond),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricSerialLinkTiming {
    clock: ClockDomain,
    latency_cycles: Cycles,
    lanes: u64,
    rate: FabricSerialLinkRate,
}

impl FabricSerialLinkTiming {
    pub fn new(
        clock: ClockDomain,
        latency_cycles: Cycles,
        lanes: u64,
        lane_speed_bits_per_cycle: u64,
    ) -> Result<Self, FabricError> {
        if lanes == 0 {
            return Err(FabricError::ZeroSerialLinkLanes);
        }
        let rate = FabricSerialLinkRate::from_bits_per_cycle(lanes, lane_speed_bits_per_cycle)?;

        Ok(Self {
            clock,
            latency_cycles,
            lanes,
            rate,
        })
    }

    pub fn with_lane_speed_bits_per_nanosecond(
        clock: ClockDomain,
        latency_cycles: Cycles,
        lanes: u64,
        lane_speed_bits_per_nanosecond: u64,
        ticks_per_nanosecond: u64,
    ) -> Result<Self, FabricError> {
        if lanes == 0 {
            return Err(FabricError::ZeroSerialLinkLanes);
        }
        let rate = FabricSerialLinkRate::from_bits_per_nanosecond(
            lanes,
            lane_speed_bits_per_nanosecond,
            ticks_per_nanosecond,
        )?;

        Ok(Self {
            clock,
            latency_cycles,
            lanes,
            rate,
        })
    }

    pub const fn clock(self) -> ClockDomain {
        self.clock
    }

    pub const fn latency_cycles(self) -> Cycles {
        self.latency_cycles
    }

    pub const fn lanes(self) -> u64 {
        self.lanes
    }

    pub const fn rate(self) -> FabricSerialLinkRate {
        self.rate
    }

    pub const fn lane_speed_bits_per_cycle(self) -> Option<u64> {
        self.rate.lane_speed_bits_per_cycle()
    }

    pub const fn bits_per_cycle(self) -> Option<u64> {
        self.rate.bits_per_cycle()
    }

    pub const fn lane_speed_bits_per_nanosecond(self) -> Option<u64> {
        self.rate.lane_speed_bits_per_nanosecond()
    }

    pub const fn ticks_per_nanosecond(self) -> Option<u64> {
        self.rate.ticks_per_nanosecond()
    }

    pub const fn bits_per_nanosecond(self) -> Option<u64> {
        self.rate.bits_per_nanosecond()
    }

    pub fn latency_ticks(self) -> Result<Tick, FabricError> {
        self.clock
            .cycles_to_ticks(self.latency_cycles)
            .map_err(FabricError::from)
    }

    pub fn serialization_ticks(self, bytes: u64) -> Result<Tick, FabricError> {
        let bits = bytes
            .checked_mul(8)
            .ok_or(FabricError::SerialLinkPacketBitOverflow { bytes })?;
        match self.rate {
            FabricSerialLinkRate::BitsPerCycle { bits_per_cycle, .. } => self
                .clock
                .cycles_to_ticks(Cycles::new(ceil_div(bits, bits_per_cycle)))
                .map_err(FabricError::from),
            FabricSerialLinkRate::BitsPerNanosecond {
                bits_per_nanosecond,
                ticks_per_nanosecond,
                ..
            } => {
                let nanoseconds = ceil_div(bits, bits_per_nanosecond);
                let ticks = nanoseconds.checked_mul(ticks_per_nanosecond).ok_or(
                    FabricError::SerialLinkNanosecondTickOverflow {
                        nanoseconds,
                        ticks_per_nanosecond,
                    },
                )?;
                let cycles = self.clock.ticks_to_cycles_ceil(ticks);
                self.clock
                    .cycles_to_ticks(cycles)
                    .map_err(FabricError::from)
            }
        }
    }

    fn flit_count(self, bytes: u64) -> Result<u64, FabricError> {
        let bits = bytes
            .checked_mul(8)
            .ok_or(FabricError::SerialLinkPacketBitOverflow { bytes })?;
        Ok(match self.rate {
            FabricSerialLinkRate::BitsPerCycle { bits_per_cycle, .. } => {
                ceil_div(bits, bits_per_cycle)
            }
            FabricSerialLinkRate::BitsPerNanosecond {
                bits_per_nanosecond,
                ..
            } => ceil_div(bits, bits_per_nanosecond),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricRouterStage {
    router: FabricRouterId,
    input_port: u32,
    output_port: u32,
    virtual_channel: u16,
    latency: Tick,
}

impl FabricRouterStage {
    pub fn new(
        router: FabricRouterId,
        input_port: u32,
        output_port: u32,
        virtual_channel: u16,
        latency: Tick,
    ) -> Result<Self, FabricError> {
        if latency == 0 {
            return Err(FabricError::ZeroRouterLatency);
        }

        Ok(Self {
            router,
            input_port,
            output_port,
            virtual_channel,
            latency,
        })
    }

    pub fn router(&self) -> &FabricRouterId {
        &self.router
    }

    pub const fn input_port(&self) -> u32 {
        self.input_port
    }

    pub const fn output_port(&self) -> u32 {
        self.output_port
    }

    pub const fn virtual_channel(&self) -> u16 {
        self.virtual_channel
    }

    pub const fn latency(&self) -> Tick {
        self.latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricPathHop {
    link: FabricLinkId,
    latency: Tick,
    bandwidth_bytes_per_tick: u64,
    serial_link: Option<FabricSerialLinkTiming>,
    credit_depth: Option<u32>,
    virtual_network: Option<VirtualNetworkId>,
    router_stage: Option<FabricRouterStage>,
}

impl FabricPathHop {
    pub fn new(
        link: FabricLinkId,
        latency: Tick,
        bandwidth_bytes_per_tick: u64,
    ) -> Result<Self, FabricError> {
        if latency == 0 {
            return Err(FabricError::ZeroLinkLatency);
        }
        if bandwidth_bytes_per_tick == 0 {
            return Err(FabricError::ZeroLinkBandwidth);
        }

        Ok(Self {
            link,
            latency,
            bandwidth_bytes_per_tick,
            serial_link: None,
            credit_depth: None,
            virtual_network: None,
            router_stage: None,
        })
    }

    pub fn serial_link(
        link: FabricLinkId,
        clock: ClockDomain,
        latency_cycles: Cycles,
        lanes: u64,
        lane_speed_bits_per_cycle: u64,
    ) -> Result<Self, FabricError> {
        let serial_link =
            FabricSerialLinkTiming::new(clock, latency_cycles, lanes, lane_speed_bits_per_cycle)?;
        Ok(Self {
            link,
            latency: serial_link.latency_ticks()?,
            bandwidth_bytes_per_tick: 1,
            serial_link: Some(serial_link),
            credit_depth: None,
            virtual_network: None,
            router_stage: None,
        })
    }

    pub fn serial_link_bits_per_nanosecond(
        link: FabricLinkId,
        clock: ClockDomain,
        latency_cycles: Cycles,
        lanes: u64,
        lane_speed_bits_per_nanosecond: u64,
        ticks_per_nanosecond: u64,
    ) -> Result<Self, FabricError> {
        let serial_link = FabricSerialLinkTiming::with_lane_speed_bits_per_nanosecond(
            clock,
            latency_cycles,
            lanes,
            lane_speed_bits_per_nanosecond,
            ticks_per_nanosecond,
        )?;
        Ok(Self {
            link,
            latency: serial_link.latency_ticks()?,
            bandwidth_bytes_per_tick: 1,
            serial_link: Some(serial_link),
            credit_depth: None,
            virtual_network: None,
            router_stage: None,
        })
    }

    pub fn with_credit_depth(mut self, credit_depth: u32) -> Result<Self, FabricError> {
        if credit_depth == 0 {
            return Err(FabricError::ZeroCreditDepth);
        }

        self.credit_depth = Some(credit_depth);
        Ok(self)
    }

    pub const fn with_virtual_network(mut self, virtual_network: VirtualNetworkId) -> Self {
        self.virtual_network = Some(virtual_network);
        self
    }

    pub fn with_router_stage(mut self, router_stage: FabricRouterStage) -> Self {
        self.router_stage = Some(router_stage);
        self
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn latency(&self) -> Tick {
        self.latency
    }

    pub const fn bandwidth_bytes_per_tick(&self) -> u64 {
        self.bandwidth_bytes_per_tick
    }

    pub const fn serial_link_timing(&self) -> Option<FabricSerialLinkTiming> {
        self.serial_link
    }

    pub const fn credit_depth(&self) -> Option<u32> {
        self.credit_depth
    }

    pub const fn virtual_network(&self) -> Option<VirtualNetworkId> {
        self.virtual_network
    }

    pub fn router_stage(&self) -> Option<&FabricRouterStage> {
        self.router_stage.as_ref()
    }

    pub(crate) fn serialization_ticks(&self, bytes: u64) -> Result<Tick, FabricError> {
        if let Some(serial_link) = self.serial_link {
            return serial_link.serialization_ticks(bytes);
        }

        Ok(serialization_ticks(bytes, self.bandwidth_bytes_per_tick()))
    }

    pub(crate) fn flit_count(&self, bytes: u64) -> Result<u64, FabricError> {
        if let Some(serial_link) = self.serial_link {
            return serial_link.flit_count(bytes);
        }

        Ok(ceil_div(bytes, self.bandwidth_bytes_per_tick()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricPath {
    hops: Vec<FabricPathHop>,
}

impl FabricPath {
    pub fn new<I>(hops: I) -> Result<Self, FabricError>
    where
        I: IntoIterator<Item = FabricPathHop>,
    {
        let hops: Vec<_> = hops.into_iter().collect();
        if hops.is_empty() {
            return Err(FabricError::EmptyPath);
        }

        Ok(Self { hops })
    }

    pub fn hops(&self) -> &[FabricPathHop] {
        &self.hops
    }
}

fn serialization_ticks(bytes: u64, bandwidth_bytes_per_tick: u64) -> Tick {
    ceil_div(bytes, bandwidth_bytes_per_tick)
}

fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    ((numerator - 1) / denominator) + 1
}
