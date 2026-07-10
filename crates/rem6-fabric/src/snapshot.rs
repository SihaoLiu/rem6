use rem6_kernel::Tick;

use crate::types::{FabricLinkId, FabricRouterId, VirtualNetworkId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricSnapshot {
    lanes: Vec<FabricLaneSnapshot>,
    router_input_vcs: Vec<FabricRouterInputVcSnapshot>,
    router_output_ports: Vec<FabricRouterOutputPortSnapshot>,
}

impl FabricSnapshot {
    pub fn new(
        lanes: Vec<FabricLaneSnapshot>,
        router_input_vcs: Vec<FabricRouterInputVcSnapshot>,
        router_output_ports: Vec<FabricRouterOutputPortSnapshot>,
    ) -> Self {
        Self {
            lanes,
            router_input_vcs,
            router_output_ports,
        }
    }

    pub fn lanes(&self) -> &[FabricLaneSnapshot] {
        &self.lanes
    }

    pub fn router_input_vcs(&self) -> &[FabricRouterInputVcSnapshot] {
        &self.router_input_vcs
    }

    pub fn router_output_ports(&self) -> &[FabricRouterOutputPortSnapshot] {
        &self.router_output_ports
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Vec<FabricLaneSnapshot>,
        Vec<FabricRouterInputVcSnapshot>,
        Vec<FabricRouterOutputPortSnapshot>,
    ) {
        (self.lanes, self.router_input_vcs, self.router_output_ports)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricRouterInputVcSnapshot {
    router: FabricRouterId,
    input_port: u32,
    virtual_channel: u16,
    next_available_tick: Tick,
}

impl FabricRouterInputVcSnapshot {
    pub fn new(
        router: FabricRouterId,
        input_port: u32,
        virtual_channel: u16,
        next_available_tick: Tick,
    ) -> Self {
        Self {
            router,
            input_port,
            virtual_channel,
            next_available_tick,
        }
    }

    pub fn router(&self) -> &FabricRouterId {
        &self.router
    }

    pub const fn input_port(&self) -> u32 {
        self.input_port
    }

    pub const fn virtual_channel(&self) -> u16 {
        self.virtual_channel
    }

    pub const fn next_available_tick(&self) -> Tick {
        self.next_available_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricRouterOutputPortSnapshot {
    router: FabricRouterId,
    output_port: u32,
    next_available_tick: Tick,
}

impl FabricRouterOutputPortSnapshot {
    pub fn new(router: FabricRouterId, output_port: u32, next_available_tick: Tick) -> Self {
        Self {
            router,
            output_port,
            next_available_tick,
        }
    }

    pub fn router(&self) -> &FabricRouterId {
        &self.router
    }

    pub const fn output_port(&self) -> u32 {
        self.output_port
    }

    pub const fn next_available_tick(&self) -> Tick {
        self.next_available_tick
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
