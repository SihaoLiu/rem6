use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::{
    ClockDomain, ClockError, Cycles, Tick, WaitForEdgeKind, WaitForGraph, WaitForNode,
};

mod activity;
mod qos;

pub use activity::{
    FabricActivityProfile, FabricLaneActivity, FabricLinkActivity, FabricVirtualNetworkActivity,
};
pub use qos::{
    FabricQosRequest, QosError, QosFixedPriorityPolicy, QosGrant, QosPriority, QosPriorityPolicy,
    QosProportionalFairPolicy, QosProportionalFairPolicySnapshot, QosProportionalFairScoreSnapshot,
    QosQueueArbiter, QosQueueArbiterSnapshot, QosQueuePolicyKind, QosQueuedRequest, QosRequestId,
    QosRequestorId,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FabricLinkId(String);

impl FabricLinkId {
    pub fn new(value: impl Into<String>) -> Result<Self, FabricError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FabricError::EmptyLinkId);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FabricPacketId(u64);

impl FabricPacketId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtualNetworkId(u16);

impl VirtualNetworkId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FabricError {
    EmptyLinkId,
    EmptyPath,
    ZeroPacketBytes,
    ZeroLinkLatency,
    ZeroLinkBandwidth,
    ZeroCreditDepth,
    ZeroSerialLinkLanes,
    ZeroSerialLinkLaneSpeed,
    ZeroSerialLinkTicksPerNanosecond,
    SerialLinkBandwidthOverflow {
        lanes: u64,
        lane_speed_bits_per_cycle: u64,
    },
    SerialLinkNanosecondBandwidthOverflow {
        lanes: u64,
        lane_speed_bits_per_nanosecond: u64,
    },
    SerialLinkPacketBitOverflow {
        bytes: u64,
    },
    SerialLinkNanosecondTickOverflow {
        nanoseconds: u64,
        ticks_per_nanosecond: u64,
    },
    Clock(ClockError),
    DuplicatePacketInBatch {
        packet: FabricPacketId,
    },
    DuplicateLaneSnapshot {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
    },
    QosNoGrant,
    TickOverflow,
}

impl fmt::Display for FabricError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLinkId => write!(formatter, "fabric link id must not be empty"),
            Self::EmptyPath => write!(formatter, "fabric path must contain a hop"),
            Self::ZeroPacketBytes => write!(formatter, "fabric packet must contain bytes"),
            Self::ZeroLinkLatency => write!(formatter, "fabric link latency must be positive"),
            Self::ZeroLinkBandwidth => write!(formatter, "fabric link bandwidth must be positive"),
            Self::ZeroCreditDepth => write!(formatter, "fabric credit depth must be positive"),
            Self::ZeroSerialLinkLanes => {
                write!(formatter, "fabric serial link lane count must be positive")
            }
            Self::ZeroSerialLinkLaneSpeed => {
                write!(formatter, "fabric serial link lane speed must be positive")
            }
            Self::ZeroSerialLinkTicksPerNanosecond => write!(
                formatter,
                "fabric serial link nanosecond timebase must be positive"
            ),
            Self::SerialLinkBandwidthOverflow {
                lanes,
                lane_speed_bits_per_cycle,
            } => write!(
                formatter,
                "fabric serial link bandwidth overflows for {lanes} lanes at {lane_speed_bits_per_cycle} bits per cycle"
            ),
            Self::SerialLinkNanosecondBandwidthOverflow {
                lanes,
                lane_speed_bits_per_nanosecond,
            } => write!(
                formatter,
                "fabric serial link bandwidth overflows for {lanes} lanes at {lane_speed_bits_per_nanosecond} bits per nanosecond"
            ),
            Self::SerialLinkPacketBitOverflow { bytes } => write!(
                formatter,
                "fabric serial link packet bit count overflows for {bytes} bytes"
            ),
            Self::SerialLinkNanosecondTickOverflow {
                nanoseconds,
                ticks_per_nanosecond,
            } => write!(
                formatter,
                "fabric serial link {nanoseconds} nanoseconds overflows at {ticks_per_nanosecond} ticks per nanosecond"
            ),
            Self::Clock(error) => write!(formatter, "{error}"),
            Self::DuplicatePacketInBatch { packet } => {
                write!(formatter, "packet {} appears more than once", packet.get())
            }
            Self::DuplicateLaneSnapshot {
                link,
                virtual_network,
            } => write!(
                formatter,
                "fabric lane {} virtual network {} appears more than once in snapshot",
                link.as_str(),
                virtual_network.get()
            ),
            Self::QosNoGrant => write!(formatter, "QoS arbiter did not select a queued request"),
            Self::TickOverflow => write!(formatter, "fabric tick calculation overflowed"),
        }
    }
}

impl Error for FabricError {}

impl From<ClockError> for FabricError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricPacket {
    id: FabricPacketId,
    bytes: u64,
    virtual_network: VirtualNetworkId,
}

impl FabricPacket {
    pub fn new(
        id: FabricPacketId,
        bytes: u64,
        virtual_network: VirtualNetworkId,
    ) -> Result<Self, FabricError> {
        if bytes == 0 {
            return Err(FabricError::ZeroPacketBytes);
        }

        Ok(Self {
            id,
            bytes,
            virtual_network,
        })
    }

    pub const fn id(&self) -> FabricPacketId {
        self.id
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }
}

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
pub struct FabricPathHop {
    link: FabricLinkId,
    latency: Tick,
    bandwidth_bytes_per_tick: u64,
    serial_link: Option<FabricSerialLinkTiming>,
    credit_depth: Option<u32>,
    virtual_network: Option<VirtualNetworkId>,
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

    fn serialization_ticks(&self, bytes: u64) -> Result<Tick, FabricError> {
        if let Some(serial_link) = self.serial_link {
            return serial_link.serialization_ticks(bytes);
        }

        Ok(serialization_ticks(bytes, self.bandwidth_bytes_per_tick()))
    }

    fn flit_count(&self, bytes: u64) -> Result<u64, FabricError> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricHopTiming {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    ready_tick: Tick,
    start_tick: Tick,
    serialization_ticks: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
}

impl FabricHopTiming {
    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub const fn ready_tick(&self) -> Tick {
        self.ready_tick
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn serialization_ticks(&self) -> Tick {
        self.serialization_ticks
    }

    pub const fn depart_tick(&self) -> Tick {
        self.depart_tick
    }

    pub const fn arrival_tick(&self) -> Tick {
        self.arrival_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricTransfer {
    packet: FabricPacket,
    injection_tick: Tick,
    arrival_tick: Tick,
    hops: Vec<FabricHopTiming>,
}

impl FabricTransfer {
    pub fn packet(&self) -> &FabricPacket {
        &self.packet
    }

    pub const fn injection_tick(&self) -> Tick {
        self.injection_tick
    }

    pub const fn arrival_tick(&self) -> Tick {
        self.arrival_tick
    }

    pub fn hops(&self) -> &[FabricHopTiming] {
        &self.hops
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricHopActivity {
    packet: FabricPacketId,
    hop_index: usize,
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    bytes: u64,
    flits: u64,
    ready_tick: Tick,
    start_tick: Tick,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    credit_delay_ticks: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
}

impl FabricHopActivity {
    pub const fn packet(&self) -> FabricPacketId {
        self.packet
    }

    pub const fn hop_index(&self) -> usize {
        self.hop_index
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn flits(&self) -> u64 {
        self.flits
    }

    pub const fn ready_tick(&self) -> Tick {
        self.ready_tick
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn occupied_ticks(&self) -> Tick {
        self.occupied_ticks
    }

    pub const fn queue_delay_ticks(&self) -> Tick {
        self.queue_delay_ticks
    }

    pub const fn credit_delay_ticks(&self) -> Tick {
        self.credit_delay_ticks
    }

    pub const fn depart_tick(&self) -> Tick {
        self.depart_tick
    }

    pub const fn arrival_tick(&self) -> Tick {
        self.arrival_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricActivityMarker {
    offset: usize,
}

impl FabricActivityMarker {
    const fn new(offset: usize) -> Self {
        Self { offset }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricWaitForMarker {
    offset: usize,
}

impl FabricWaitForMarker {
    const fn new(offset: usize) -> Self {
        Self { offset }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FabricLaneKey {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
}

impl FabricLaneKey {
    fn new(link: FabricLinkId, virtual_network: VirtualNetworkId) -> Self {
        Self {
            link,
            virtual_network,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricLaneSnapshot {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    next_available_tick: Tick,
    credit_return_ticks: Vec<Tick>,
}

impl FabricLaneSnapshot {
    pub fn new(
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        next_available_tick: Tick,
        credit_return_ticks: Vec<Tick>,
    ) -> Self {
        Self {
            link,
            virtual_network,
            next_available_tick,
            credit_return_ticks,
        }
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub const fn next_available_tick(&self) -> Tick {
        self.next_available_tick
    }

    pub fn credit_return_ticks(&self) -> &[Tick] {
        &self.credit_return_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FabricLaneActivityRecord {
    packet: FabricPacketId,
    hop_index: usize,
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    bytes: u64,
    flits: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    credit_delay_ticks: Tick,
    ready_tick: Tick,
    start_tick: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
}

impl FabricLaneActivityRecord {
    #[allow(clippy::too_many_arguments)]
    fn new(
        packet: FabricPacketId,
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        bytes: u64,
        flits: u64,
        occupied_ticks: Tick,
        queue_delay_ticks: Tick,
        credit_delay_ticks: Tick,
        ready_tick: Tick,
        start_tick: Tick,
        depart_tick: Tick,
        arrival_tick: Tick,
    ) -> Self {
        Self {
            packet,
            hop_index,
            link,
            virtual_network,
            bytes,
            flits,
            occupied_ticks,
            queue_delay_ticks,
            credit_delay_ticks,
            ready_tick,
            start_tick,
            depart_tick,
            arrival_tick,
        }
    }

    fn key(&self) -> FabricLaneKey {
        FabricLaneKey::new(self.link.clone(), self.virtual_network)
    }

    fn activity(&self) -> FabricLaneActivity {
        FabricLaneActivity::new(
            self.link.clone(),
            self.virtual_network,
            1,
            self.bytes,
            self.occupied_ticks,
            self.queue_delay_ticks,
            self.queue_delay_ticks,
            self.ready_tick,
            self.arrival_tick,
        )
        .with_flit_count(self.flits)
        .with_credit_delay(self.credit_delay_ticks, self.credit_delay_ticks)
    }

    fn hop_activity(&self) -> FabricHopActivity {
        FabricHopActivity {
            packet: self.packet,
            hop_index: self.hop_index,
            link: self.link.clone(),
            virtual_network: self.virtual_network,
            bytes: self.bytes,
            flits: self.flits,
            ready_tick: self.ready_tick,
            start_tick: self.start_tick,
            occupied_ticks: self.occupied_ticks,
            queue_delay_ticks: self.queue_delay_ticks,
            credit_delay_ticks: self.credit_delay_ticks,
            depart_tick: self.depart_tick,
            arrival_tick: self.arrival_tick,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FabricWaitKind {
    Queue,
    Credit,
}

impl FabricWaitKind {
    const fn edge_kind(self) -> WaitForEdgeKind {
        match self {
            Self::Queue => WaitForEdgeKind::Queue,
            Self::Credit => WaitForEdgeKind::Credit,
        }
    }

    const fn resource_suffix(self) -> &'static str {
        match self {
            Self::Queue => "lane",
            Self::Credit => "credit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FabricWaitRecord {
    packet: FabricPacketId,
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    kind: FabricWaitKind,
    first_blocked_tick: Tick,
    unblocked_tick: Tick,
}

impl FabricWaitRecord {
    fn new(
        packet: FabricPacketId,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        kind: FabricWaitKind,
        first_blocked_tick: Tick,
        unblocked_tick: Tick,
    ) -> Self {
        Self {
            packet,
            link,
            virtual_network,
            kind,
            first_blocked_tick,
            unblocked_tick,
        }
    }

    const fn is_active_at(&self, tick: Tick) -> bool {
        self.first_blocked_tick <= tick && tick < self.unblocked_tick
    }
}

#[derive(Clone, Debug, Default)]
struct FabricLaneState {
    next_available: Tick,
    credit_returns: Vec<Tick>,
}

impl FabricLaneState {
    fn from_snapshot(snapshot: &FabricLaneSnapshot) -> Self {
        let mut credit_returns = snapshot.credit_return_ticks.clone();
        credit_returns.sort_unstable();
        Self {
            next_available: snapshot.next_available_tick,
            credit_returns,
        }
    }

    fn reserve(
        &mut self,
        arrival_tick: Tick,
        serialization_ticks: Tick,
        link_latency: Tick,
        credit_depth: Option<u32>,
    ) -> Result<FabricLaneReservation, FabricError> {
        let mut start_tick = arrival_tick.max(self.next_available);
        let mut wait_kind = (start_tick > arrival_tick).then_some(FabricWaitKind::Queue);
        let mut credit_delay_ticks: Tick = 0;
        if let Some(credit_depth) = credit_depth {
            loop {
                self.credit_returns
                    .retain(|credit_return| *credit_return > start_tick);
                if self.credit_returns.len() < credit_depth as usize {
                    break;
                }

                let credit_ready_tick = self.credit_returns[0];
                if credit_ready_tick <= start_tick {
                    break;
                }
                wait_kind = Some(FabricWaitKind::Credit);
                let previous_start_tick = start_tick;
                start_tick = credit_ready_tick.max(self.next_available);
                let credit_delay = start_tick
                    .checked_sub(previous_start_tick)
                    .ok_or(FabricError::TickOverflow)?;
                credit_delay_ticks = credit_delay_ticks
                    .checked_add(credit_delay)
                    .ok_or(FabricError::TickOverflow)?;
            }
        }

        let ready_tick = start_tick;
        let depart_tick = start_tick
            .checked_add(serialization_ticks)
            .ok_or(FabricError::TickOverflow)?;
        let next_arrival_tick = depart_tick
            .checked_add(link_latency)
            .ok_or(FabricError::TickOverflow)?;

        self.next_available = depart_tick;
        if credit_depth.is_some() {
            self.credit_returns.push(next_arrival_tick);
            self.credit_returns.sort_unstable();
        }

        Ok(FabricLaneReservation {
            ready_tick,
            start_tick,
            depart_tick,
            arrival_tick: next_arrival_tick,
            wait_kind,
            credit_delay_ticks,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FabricLaneReservation {
    ready_tick: Tick,
    start_tick: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
    wait_kind: Option<FabricWaitKind>,
    credit_delay_ticks: Tick,
}

#[derive(Clone, Debug, Default)]
pub struct FabricModel {
    lanes: BTreeMap<FabricLaneKey, FabricLaneState>,
    activity_log: Vec<FabricLaneActivityRecord>,
    wait_log: Vec<FabricWaitRecord>,
}

impl FabricModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transmit(
        &mut self,
        injection_tick: Tick,
        packet: FabricPacket,
        path: FabricPath,
    ) -> Result<FabricTransfer, FabricError> {
        self.reserve_transfer(injection_tick, packet, &path)
    }

    pub fn transmit_batch<I>(
        &mut self,
        injection_tick: Tick,
        requests: I,
    ) -> Result<Vec<FabricTransfer>, FabricError>
    where
        I: IntoIterator<Item = (FabricPacket, FabricPath)>,
    {
        let mut requests: Vec<_> = requests.into_iter().collect();
        let mut seen = BTreeSet::new();
        for (packet, _) in &requests {
            if !seen.insert(packet.id()) {
                return Err(FabricError::DuplicatePacketInBatch {
                    packet: packet.id(),
                });
            }
        }

        requests.sort_by_key(|(packet, _)| packet.id());
        requests
            .into_iter()
            .map(|(packet, path)| self.reserve_transfer(injection_tick, packet, &path))
            .collect()
    }

    pub fn transmit_qos_batch<I>(
        &mut self,
        injection_tick: Tick,
        requests: I,
        arbiter: &mut QosQueueArbiter,
    ) -> Result<Vec<FabricTransfer>, FabricError>
    where
        I: IntoIterator<Item = FabricQosRequest>,
    {
        let mut pending: Vec<_> = requests.into_iter().collect();
        let mut seen = BTreeSet::new();
        for request in &pending {
            if !seen.insert(request.packet().id()) {
                return Err(FabricError::DuplicatePacketInBatch {
                    packet: request.packet().id(),
                });
            }
        }

        let mut transfers = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let queue: Vec<_> = pending
                .iter()
                .map(FabricQosRequest::queued_request)
                .collect();
            let Some(grant) = arbiter.grant(&queue) else {
                return Err(FabricError::QosNoGrant);
            };
            let request = pending.remove(grant.queue_index());
            let (packet, path) = request.into_packet_path();
            transfers.push(self.reserve_transfer(injection_tick, packet, &path)?);
        }

        Ok(transfers)
    }

    pub fn lane_snapshots(&self) -> Vec<FabricLaneSnapshot> {
        self.lanes
            .iter()
            .map(|(lane, state)| {
                FabricLaneSnapshot::new(
                    lane.link.clone(),
                    lane.virtual_network,
                    state.next_available,
                    state.credit_returns.clone(),
                )
            })
            .collect()
    }

    pub fn restore_lane_snapshots<I>(&mut self, snapshots: I) -> Result<(), FabricError>
    where
        I: IntoIterator<Item = FabricLaneSnapshot>,
    {
        let mut lanes = BTreeMap::new();
        for snapshot in snapshots {
            let key = FabricLaneKey::new(snapshot.link.clone(), snapshot.virtual_network);
            if lanes.contains_key(&key) {
                return Err(FabricError::DuplicateLaneSnapshot {
                    link: snapshot.link,
                    virtual_network: snapshot.virtual_network,
                });
            }
            lanes.insert(key, FabricLaneState::from_snapshot(&snapshot));
        }

        self.lanes = lanes;
        Ok(())
    }

    pub fn mark_activity(&self) -> FabricActivityMarker {
        FabricActivityMarker::new(self.activity_log.len())
    }

    pub fn mark_wait_for(&self) -> FabricWaitForMarker {
        FabricWaitForMarker::new(self.wait_log.len())
    }

    pub fn lane_activities(&self) -> Vec<FabricLaneActivity> {
        collect_lane_activities(&self.activity_log)
    }

    pub fn lane_activities_since(&self, marker: FabricActivityMarker) -> Vec<FabricLaneActivity> {
        let Some(records) = self.activity_log.get(marker.offset..) else {
            return Vec::new();
        };
        collect_lane_activities(records)
    }

    pub fn hop_activities(&self) -> Vec<FabricHopActivity> {
        collect_hop_activities(&self.activity_log)
    }

    pub fn hop_activities_since(&self, marker: FabricActivityMarker) -> Vec<FabricHopActivity> {
        let Some(records) = self.activity_log.get(marker.offset..) else {
            return Vec::new();
        };
        collect_hop_activities(records)
    }

    pub fn link_activities(&self) -> Vec<FabricLinkActivity> {
        FabricLinkActivity::from_lanes(self.lane_activities().iter())
    }

    pub fn link_activities_since(&self, marker: FabricActivityMarker) -> Vec<FabricLinkActivity> {
        FabricLinkActivity::from_lanes(self.lane_activities_since(marker).iter())
    }

    pub fn virtual_network_activities(&self) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities().iter())
    }

    pub fn virtual_network_activities_since(
        &self,
        marker: FabricActivityMarker,
    ) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities_since(marker).iter())
    }

    pub fn activity_profile(&self) -> FabricActivityProfile {
        FabricActivityProfile::from_lanes(self.lane_activities().iter())
    }

    pub fn activity_profile_since(&self, marker: FabricActivityMarker) -> FabricActivityProfile {
        FabricActivityProfile::from_lanes(self.lane_activities_since(marker).iter())
    }

    pub fn lane_activity(
        &self,
        link: &FabricLinkId,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricLaneActivity> {
        self.lane_activities().into_iter().find(|activity| {
            activity.link() == link && activity.virtual_network() == virtual_network
        })
    }

    pub fn link_activity(&self, link: &FabricLinkId) -> Option<FabricLinkActivity> {
        self.link_activities()
            .into_iter()
            .find(|activity| activity.link() == link)
    }

    pub fn virtual_network_activity(
        &self,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricVirtualNetworkActivity> {
        self.virtual_network_activities()
            .into_iter()
            .find(|activity| activity.virtual_network() == virtual_network)
    }

    pub fn active_lane_count(&self) -> usize {
        self.lane_activities().len()
    }

    pub fn total_transfer_count(&self) -> usize {
        self.lane_activities()
            .iter()
            .map(FabricLaneActivity::transfer_count)
            .sum()
    }

    pub fn total_queue_delay_ticks(&self) -> Tick {
        self.lane_activities()
            .iter()
            .map(FabricLaneActivity::queue_delay_ticks)
            .sum()
    }

    pub fn wait_for_graph_at(&self, tick: Tick) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        for wait in self.wait_log.iter().filter(|wait| wait.is_active_at(tick)) {
            record_wait_interval(&mut graph, wait, tick, tick);
        }
        graph
    }

    pub fn wait_for_graph_since(&self, marker: FabricWaitForMarker) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        let Some(records) = self.wait_log.get(marker.offset..) else {
            return graph;
        };
        for wait in records {
            record_wait_interval(
                &mut graph,
                wait,
                wait.first_blocked_tick,
                wait.unblocked_tick.saturating_sub(1),
            );
        }
        graph
    }

    pub fn clear_activity(&mut self) {
        self.activity_log.clear();
        self.wait_log.clear();
    }

    fn reserve_transfer(
        &mut self,
        injection_tick: Tick,
        packet: FabricPacket,
        path: &FabricPath,
    ) -> Result<FabricTransfer, FabricError> {
        let mut arrival_tick = injection_tick;
        let mut timings = Vec::with_capacity(path.hops().len());

        for (hop_index, hop) in path.hops().iter().enumerate() {
            let ready_tick = arrival_tick;
            let virtual_network = hop.virtual_network().unwrap_or(packet.virtual_network());
            let lane = FabricLaneKey::new(hop.link().clone(), virtual_network);
            let serialization_ticks = hop.serialization_ticks(packet.bytes())?;
            let flits = hop.flit_count(packet.bytes())?;
            let reservation = self.lanes.entry(lane).or_default().reserve(
                ready_tick,
                serialization_ticks,
                hop.latency(),
                hop.credit_depth(),
            )?;
            let queue_delay_ticks = reservation
                .start_tick
                .checked_sub(ready_tick)
                .ok_or(FabricError::TickOverflow)?;
            let credit_delay_ticks = reservation.credit_delay_ticks;
            if queue_delay_ticks != 0 {
                if credit_delay_ticks == 0 {
                    let wait_kind = reservation.wait_kind.unwrap_or(FabricWaitKind::Queue);
                    self.wait_log.push(FabricWaitRecord::new(
                        packet.id(),
                        hop.link().clone(),
                        virtual_network,
                        wait_kind,
                        ready_tick,
                        reservation.start_tick,
                    ));
                } else {
                    let credit_wait_start_tick = reservation
                        .start_tick
                        .checked_sub(credit_delay_ticks)
                        .ok_or(FabricError::TickOverflow)?;
                    if credit_wait_start_tick > ready_tick {
                        self.wait_log.push(FabricWaitRecord::new(
                            packet.id(),
                            hop.link().clone(),
                            virtual_network,
                            FabricWaitKind::Queue,
                            ready_tick,
                            credit_wait_start_tick,
                        ));
                    }
                    self.wait_log.push(FabricWaitRecord::new(
                        packet.id(),
                        hop.link().clone(),
                        virtual_network,
                        FabricWaitKind::Credit,
                        credit_wait_start_tick,
                        reservation.start_tick,
                    ));
                }
            }

            timings.push(FabricHopTiming {
                link: hop.link().clone(),
                virtual_network,
                ready_tick: reservation.ready_tick,
                start_tick: reservation.start_tick,
                serialization_ticks,
                depart_tick: reservation.depart_tick,
                arrival_tick: reservation.arrival_tick,
            });
            self.activity_log.push(FabricLaneActivityRecord::new(
                packet.id(),
                hop_index,
                hop.link().clone(),
                virtual_network,
                packet.bytes(),
                flits,
                serialization_ticks,
                queue_delay_ticks,
                credit_delay_ticks,
                ready_tick,
                reservation.start_tick,
                reservation.depart_tick,
                reservation.arrival_tick,
            ));
            arrival_tick = reservation.arrival_tick;
        }

        Ok(FabricTransfer {
            packet,
            injection_tick,
            arrival_tick,
            hops: timings,
        })
    }
}

fn collect_lane_activities(records: &[FabricLaneActivityRecord]) -> Vec<FabricLaneActivity> {
    let mut activities = BTreeMap::<FabricLaneKey, FabricLaneActivity>::new();
    for record in records {
        activities
            .entry(record.key())
            .and_modify(|stored| *stored = stored.clone().merge_window(record.activity()))
            .or_insert_with(|| record.activity());
    }
    activities.into_values().collect()
}

fn collect_hop_activities(records: &[FabricLaneActivityRecord]) -> Vec<FabricHopActivity> {
    records
        .iter()
        .map(FabricLaneActivityRecord::hop_activity)
        .collect()
}

fn serialization_ticks(bytes: u64, bandwidth_bytes_per_tick: u64) -> Tick {
    ceil_div(bytes, bandwidth_bytes_per_tick)
}

fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    ((numerator - 1) / denominator) + 1
}

fn fabric_packet_node(packet: FabricPacketId) -> WaitForNode {
    WaitForNode::transaction(format!("fabric.packet.{}", packet.get()))
        .expect("fabric packet wait-for label is generated from numeric ids")
}

fn fabric_resource_node(
    link: &FabricLinkId,
    virtual_network: VirtualNetworkId,
    suffix: &str,
) -> WaitForNode {
    WaitForNode::resource(format!(
        "fabric.{}.vn.{}.{}",
        wait_for_label_segment(link.as_str()),
        virtual_network.get(),
        suffix
    ))
    .expect("fabric resource wait-for label is sanitized")
}

fn record_wait_interval(
    graph: &mut WaitForGraph,
    wait: &FabricWaitRecord,
    first_tick: Tick,
    last_tick: Tick,
) {
    let source = fabric_packet_node(wait.packet);
    let target = fabric_resource_node(
        &wait.link,
        wait.virtual_network,
        wait.kind.resource_suffix(),
    );
    graph
        .record_wait(
            source.clone(),
            target.clone(),
            wait.kind.edge_kind(),
            first_tick,
        )
        .expect("fabric wait-for labels are generated from typed ids");
    if last_tick != first_tick {
        graph
            .record_wait(source, target, wait.kind.edge_kind(), last_tick)
            .expect("fabric wait-for labels are generated from typed ids");
    }
}

fn wait_for_label_segment(label: &str) -> String {
    label
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':') {
                byte as char
            } else {
                '_'
            }
        })
        .collect()
}
