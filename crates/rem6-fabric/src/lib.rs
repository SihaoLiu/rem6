use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

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
    DuplicatePacketInBatch { packet: FabricPacketId },
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
            Self::DuplicatePacketInBatch { packet } => {
                write!(formatter, "packet {} appears more than once", packet.get())
            }
            Self::TickOverflow => write!(formatter, "fabric tick calculation overflowed"),
        }
    }
}

impl Error for FabricError {}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricPathHop {
    link: FabricLinkId,
    latency: Tick,
    bandwidth_bytes_per_tick: u64,
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
        })
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

#[derive(Clone, Debug, Default)]
pub struct FabricModel {
    next_available: BTreeMap<FabricLaneKey, Tick>,
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

    fn reserve_transfer(
        &mut self,
        injection_tick: Tick,
        packet: FabricPacket,
        path: &FabricPath,
    ) -> Result<FabricTransfer, FabricError> {
        let mut arrival_tick = injection_tick;
        let mut timings = Vec::with_capacity(path.hops().len());

        for hop in path.hops() {
            let lane = FabricLaneKey::new(hop.link().clone(), packet.virtual_network());
            let ready_tick = *self.next_available.get(&lane).unwrap_or(&0);
            let start_tick = arrival_tick.max(ready_tick);
            let serialization_ticks =
                serialization_ticks(packet.bytes(), hop.bandwidth_bytes_per_tick());
            let depart_tick = start_tick
                .checked_add(serialization_ticks)
                .ok_or(FabricError::TickOverflow)?;
            let next_arrival_tick = depart_tick
                .checked_add(hop.latency())
                .ok_or(FabricError::TickOverflow)?;

            self.next_available.insert(lane, depart_tick);
            timings.push(FabricHopTiming {
                link: hop.link().clone(),
                virtual_network: packet.virtual_network(),
                ready_tick,
                start_tick,
                serialization_ticks,
                depart_tick,
                arrival_tick: next_arrival_tick,
            });
            arrival_tick = next_arrival_tick;
        }

        Ok(FabricTransfer {
            packet,
            injection_tick,
            arrival_tick,
            hops: timings,
        })
    }
}

fn serialization_ticks(bytes: u64, bandwidth_bytes_per_tick: u64) -> Tick {
    ((bytes - 1) / bandwidth_bytes_per_tick) + 1
}
