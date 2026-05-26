use rem6_fabric::{FabricLinkActivity, FabricLinkId};
use rem6_kernel::Tick;

use crate::WorkloadError;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedFabricLinkActivity {
    link: FabricLinkId,
    minimum_transfer_count: usize,
    minimum_active_virtual_network_count: usize,
    minimum_queue_delay_ticks: Tick,
    minimum_contended_virtual_network_count: usize,
    maximum_queue_delay_ticks: Option<Tick>,
    maximum_max_queue_delay_ticks: Option<Tick>,
}

impl WorkloadExpectedFabricLinkActivity {
    pub fn new(
        link: FabricLinkId,
        minimum_transfer_count: usize,
        minimum_active_virtual_network_count: usize,
        minimum_queue_delay_ticks: Tick,
        minimum_contended_virtual_network_count: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_transfer_count == 0
            && minimum_active_virtual_network_count == 0
            && minimum_queue_delay_ticks == 0
            && minimum_contended_virtual_network_count == 0
        {
            return Err(WorkloadError::ZeroExpectedFabricLinkActivity { link });
        }
        Ok(Self {
            link,
            minimum_transfer_count,
            minimum_active_virtual_network_count,
            minimum_queue_delay_ticks,
            minimum_contended_virtual_network_count,
            maximum_queue_delay_ticks: None,
            maximum_max_queue_delay_ticks: None,
        })
    }

    pub fn with_queue_delay_budget(
        mut self,
        maximum_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        if maximum_max_queue_delay_ticks > maximum_queue_delay_ticks {
            return Err(
                WorkloadError::InvalidExpectedFabricLinkActivityQueueDelayBudget {
                    link: self.link,
                    maximum_queue_delay_ticks,
                    maximum_max_queue_delay_ticks,
                },
            );
        }
        self.maximum_queue_delay_ticks = Some(maximum_queue_delay_ticks);
        self.maximum_max_queue_delay_ticks = Some(maximum_max_queue_delay_ticks);
        Ok(self)
    }

    pub fn link(&self) -> &FabricLinkId {
        &self.link
    }

    pub const fn minimum_transfer_count(&self) -> usize {
        self.minimum_transfer_count
    }

    pub const fn minimum_active_virtual_network_count(&self) -> usize {
        self.minimum_active_virtual_network_count
    }

    pub const fn minimum_queue_delay_ticks(&self) -> Tick {
        self.minimum_queue_delay_ticks
    }

    pub const fn minimum_contended_virtual_network_count(&self) -> usize {
        self.minimum_contended_virtual_network_count
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

    pub(crate) fn sort_key(&self) -> &str {
        self.link.as_str()
    }

    pub(crate) fn below_minimum(&self, activity: &FabricLinkActivity) -> bool {
        activity.transfer_count() < self.minimum_transfer_count
            || activity.active_virtual_network_count() < self.minimum_active_virtual_network_count
            || activity.queue_delay_ticks() < self.minimum_queue_delay_ticks
            || activity.contended_virtual_network_count()
                < self.minimum_contended_virtual_network_count
    }

    pub(crate) fn above_maximum(&self, activity: &FabricLinkActivity) -> bool {
        self.maximum_queue_delay_ticks
            .is_some_and(|maximum| activity.queue_delay_ticks() > maximum)
            || self
                .maximum_max_queue_delay_ticks
                .is_some_and(|maximum| activity.max_queue_delay_ticks() > maximum)
    }
}
