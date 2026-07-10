use rem6_kernel::Tick;

use crate::types::{FabricLinkId, FabricPacket, FabricPacketId, FabricRouterId, VirtualNetworkId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricRouterTiming {
    router: FabricRouterId,
    input_port: u32,
    output_port: u32,
    virtual_channel: u16,
    ready_tick: Tick,
    start_tick: Tick,
    latency_ticks: Tick,
    depart_tick: Tick,
    queue_delay_ticks: Tick,
}

impl FabricRouterTiming {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        router: FabricRouterId,
        input_port: u32,
        output_port: u32,
        virtual_channel: u16,
        ready_tick: Tick,
        start_tick: Tick,
        latency_ticks: Tick,
        depart_tick: Tick,
        queue_delay_ticks: Tick,
    ) -> Self {
        Self {
            router,
            input_port,
            output_port,
            virtual_channel,
            ready_tick,
            start_tick,
            latency_ticks,
            depart_tick,
            queue_delay_ticks,
        }
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

    pub const fn ready_tick(&self) -> Tick {
        self.ready_tick
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn latency_ticks(&self) -> Tick {
        self.latency_ticks
    }

    pub const fn depart_tick(&self) -> Tick {
        self.depart_tick
    }

    pub const fn queue_delay_ticks(&self) -> Tick {
        self.queue_delay_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricHopTiming {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    router: Option<FabricRouterTiming>,
    ready_tick: Tick,
    start_tick: Tick,
    serialization_ticks: Tick,
    depart_tick: Tick,
    arrival_tick: Tick,
}

impl FabricHopTiming {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        router: Option<FabricRouterTiming>,
        ready_tick: Tick,
        start_tick: Tick,
        serialization_ticks: Tick,
        depart_tick: Tick,
        arrival_tick: Tick,
    ) -> Self {
        Self {
            link,
            virtual_network,
            router,
            ready_tick,
            start_tick,
            serialization_ticks,
            depart_tick,
            arrival_tick,
        }
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub fn router(&self) -> Option<&FabricRouterTiming> {
        self.router.as_ref()
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
    pub(crate) fn new(
        packet: FabricPacket,
        injection_tick: Tick,
        arrival_tick: Tick,
        hops: Vec<FabricHopTiming>,
    ) -> Self {
        Self {
            packet,
            injection_tick,
            arrival_tick,
            hops,
        }
    }

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
pub struct FabricRouterActivity {
    router: FabricRouterId,
    input_port: u32,
    output_port: u32,
    virtual_channel: u16,
    ready_tick: Tick,
    start_tick: Tick,
    latency_ticks: Tick,
    depart_tick: Tick,
    queue_delay_ticks: Tick,
}

impl FabricRouterActivity {
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

    pub const fn ready_tick(&self) -> Tick {
        self.ready_tick
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn latency_ticks(&self) -> Tick {
        self.latency_ticks
    }

    pub const fn depart_tick(&self) -> Tick {
        self.depart_tick
    }

    pub const fn queue_delay_ticks(&self) -> Tick {
        self.queue_delay_ticks
    }
}

impl From<&FabricRouterTiming> for FabricRouterActivity {
    fn from(timing: &FabricRouterTiming) -> Self {
        Self {
            router: timing.router().clone(),
            input_port: timing.input_port(),
            output_port: timing.output_port(),
            virtual_channel: timing.virtual_channel(),
            ready_tick: timing.ready_tick(),
            start_tick: timing.start_tick(),
            latency_ticks: timing.latency_ticks(),
            depart_tick: timing.depart_tick(),
            queue_delay_ticks: timing.queue_delay_ticks(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricHopActivity {
    packet: FabricPacketId,
    hop_index: usize,
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    router: Option<FabricRouterActivity>,
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        packet: FabricPacketId,
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        router: Option<FabricRouterActivity>,
        bytes: u64,
        flits: u64,
        ready_tick: Tick,
        start_tick: Tick,
        occupied_ticks: Tick,
        queue_delay_ticks: Tick,
        credit_delay_ticks: Tick,
        depart_tick: Tick,
        arrival_tick: Tick,
    ) -> Self {
        Self {
            packet,
            hop_index,
            link,
            virtual_network,
            router,
            bytes,
            flits,
            ready_tick,
            start_tick,
            occupied_ticks,
            queue_delay_ticks,
            credit_delay_ticks,
            depart_tick,
            arrival_tick,
        }
    }

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

    pub fn router(&self) -> Option<&FabricRouterActivity> {
        self.router.as_ref()
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
    pub(crate) const fn new(offset: usize) -> Self {
        Self { offset }
    }

    pub(crate) const fn offset(self) -> usize {
        self.offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FabricWaitForMarker {
    offset: usize,
}

impl FabricWaitForMarker {
    pub(crate) const fn new(offset: usize) -> Self {
        Self { offset }
    }

    pub(crate) const fn offset(self) -> usize {
        self.offset
    }
}
