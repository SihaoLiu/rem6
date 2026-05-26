use rem6_fabric::{FabricLaneActivity, FabricLinkId, VirtualNetworkId};
use rem6_kernel::Tick;

use crate::WorkloadError;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedFabricLaneActivity {
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    minimum_transfer_count: usize,
    minimum_byte_count: u64,
    minimum_occupied_ticks: Tick,
    minimum_queue_delay_ticks: Tick,
    minimum_max_queue_delay_ticks: Tick,
    maximum_queue_delay_ticks: Option<Tick>,
    maximum_max_queue_delay_ticks: Option<Tick>,
    required_first_tick: Option<Tick>,
    required_last_tick: Option<Tick>,
}

impl WorkloadExpectedFabricLaneActivity {
    pub fn new(
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        minimum_byte_count: u64,
        minimum_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        if minimum_transfer_count == 0
            && minimum_byte_count == 0
            && minimum_occupied_ticks == 0
            && minimum_queue_delay_ticks == 0
        {
            return Err(WorkloadError::ZeroExpectedFabricLaneActivity {
                link,
                virtual_network,
            });
        }
        Ok(Self {
            link,
            virtual_network,
            minimum_transfer_count,
            minimum_byte_count,
            minimum_occupied_ticks,
            minimum_queue_delay_ticks,
            minimum_max_queue_delay_ticks: 0,
            maximum_queue_delay_ticks: None,
            maximum_max_queue_delay_ticks: None,
            required_first_tick: None,
            required_last_tick: None,
        })
    }

    pub fn with_minimum_max_queue_delay_ticks(
        mut self,
        minimum_max_queue_delay_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        self.minimum_max_queue_delay_ticks = minimum_max_queue_delay_ticks;
        Ok(self)
    }

    pub fn with_queue_delay_budget(
        mut self,
        maximum_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        if maximum_max_queue_delay_ticks > maximum_queue_delay_ticks {
            return Err(
                WorkloadError::InvalidExpectedFabricLaneActivityQueueDelayBudget {
                    link: self.link,
                    virtual_network: self.virtual_network,
                    maximum_queue_delay_ticks,
                    maximum_max_queue_delay_ticks,
                },
            );
        }
        self.maximum_queue_delay_ticks = Some(maximum_queue_delay_ticks);
        self.maximum_max_queue_delay_ticks = Some(maximum_max_queue_delay_ticks);
        Ok(self)
    }

    pub fn with_required_tick_window(
        mut self,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if first_tick > last_tick {
            return Err(WorkloadError::InvalidExpectedFabricLaneActivityWindow {
                link: self.link,
                virtual_network: self.virtual_network,
                first_tick,
                last_tick,
            });
        }
        self.required_first_tick = Some(first_tick);
        self.required_last_tick = Some(last_tick);
        Ok(self)
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }

    pub const fn minimum_transfer_count(&self) -> usize {
        self.minimum_transfer_count
    }

    pub const fn minimum_byte_count(&self) -> u64 {
        self.minimum_byte_count
    }

    pub const fn minimum_occupied_ticks(&self) -> Tick {
        self.minimum_occupied_ticks
    }

    pub const fn minimum_queue_delay_ticks(&self) -> Tick {
        self.minimum_queue_delay_ticks
    }

    pub const fn minimum_max_queue_delay_ticks(&self) -> Tick {
        self.minimum_max_queue_delay_ticks
    }

    pub const fn queue_delay_budget(&self) -> Option<(Tick, Tick)> {
        match (
            self.maximum_queue_delay_ticks,
            self.maximum_max_queue_delay_ticks,
        ) {
            (Some(queue_delay_ticks), Some(max_queue_delay_ticks)) => {
                Some((queue_delay_ticks, max_queue_delay_ticks))
            }
            _ => None,
        }
    }

    pub const fn maximum_queue_delay_ticks(&self) -> Option<Tick> {
        self.maximum_queue_delay_ticks
    }

    pub const fn maximum_max_queue_delay_ticks(&self) -> Option<Tick> {
        self.maximum_max_queue_delay_ticks
    }

    pub const fn required_tick_window(&self) -> Option<(Tick, Tick)> {
        match (self.required_first_tick, self.required_last_tick) {
            (Some(first_tick), Some(last_tick)) => Some((first_tick, last_tick)),
            _ => None,
        }
    }

    pub const fn required_first_tick(&self) -> Option<Tick> {
        self.required_first_tick
    }

    pub const fn required_last_tick(&self) -> Option<Tick> {
        self.required_last_tick
    }

    pub(crate) fn sort_key(&self) -> (&str, u16) {
        (self.link.as_str(), self.virtual_network.get())
    }

    pub(crate) fn below_minimum(&self, activity: &FabricLaneActivity) -> bool {
        activity.transfer_count() < self.minimum_transfer_count
            || activity.byte_count() < self.minimum_byte_count
            || activity.occupied_ticks() < self.minimum_occupied_ticks
            || activity.queue_delay_ticks() < self.minimum_queue_delay_ticks
            || activity.max_queue_delay_ticks() < self.minimum_max_queue_delay_ticks
            || self
                .required_first_tick
                .is_some_and(|required| activity.first_tick() > required)
            || self
                .required_last_tick
                .is_some_and(|required| activity.last_tick() < required)
    }

    pub(crate) fn above_maximum(&self, activity: &FabricLaneActivity) -> bool {
        self.maximum_queue_delay_ticks
            .is_some_and(|maximum| activity.queue_delay_ticks() > maximum)
            || self
                .maximum_max_queue_delay_ticks
                .is_some_and(|maximum| activity.max_queue_delay_ticks() > maximum)
    }
}
