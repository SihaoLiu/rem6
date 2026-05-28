use rem6_fabric::{FabricHopActivity, FabricLinkId, VirtualNetworkId};
use rem6_kernel::Tick;

use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedFabricHopActivity {
    hop_index: usize,
    link: FabricLinkId,
    virtual_network: VirtualNetworkId,
    minimum_transfer_count: usize,
    minimum_byte_count: u64,
    minimum_occupied_ticks: Tick,
    minimum_queue_delay_ticks: Tick,
    required_first_tick: Option<Tick>,
    required_last_tick: Option<Tick>,
}

impl WorkloadExpectedFabricHopActivity {
    pub fn new(
        hop_index: usize,
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
            return Err(WorkloadError::ZeroExpectedFabricHopActivity {
                hop_index,
                link,
                virtual_network,
            });
        }
        Ok(Self {
            hop_index,
            link,
            virtual_network,
            minimum_transfer_count,
            minimum_byte_count,
            minimum_occupied_ticks,
            minimum_queue_delay_ticks,
            required_first_tick: None,
            required_last_tick: None,
        })
    }

    pub fn with_required_tick_window(
        mut self,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if first_tick > last_tick {
            return Err(WorkloadError::InvalidExpectedFabricHopActivityWindow {
                hop_index: self.hop_index,
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

    pub const fn hop_index(&self) -> usize {
        self.hop_index
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

    pub(crate) fn sort_key(&self) -> (usize, &str, u16) {
        (
            self.hop_index,
            self.link.as_str(),
            self.virtual_network.get(),
        )
    }

    fn actual_activity(&self, activities: &[FabricHopActivity]) -> Option<ActualFabricHopActivity> {
        let mut actual = ActualFabricHopActivity::default();
        for activity in activities {
            if activity.hop_index() != self.hop_index
                || activity.link() != &self.link
                || activity.virtual_network() != self.virtual_network
            {
                continue;
            }
            actual.record(activity);
        }
        (actual.transfer_count != 0).then_some(actual)
    }

    fn below_minimum(&self, actual: &ActualFabricHopActivity) -> bool {
        actual.transfer_count < self.minimum_transfer_count
            || actual.byte_count < self.minimum_byte_count
            || actual.occupied_ticks < self.minimum_occupied_ticks
            || actual.queue_delay_ticks < self.minimum_queue_delay_ticks
            || self
                .required_first_tick
                .is_some_and(|required| actual.first_tick > required)
            || self
                .required_last_tick
                .is_some_and(|required| actual.last_tick < required)
    }
}

#[derive(Default)]
struct ActualFabricHopActivity {
    transfer_count: usize,
    byte_count: u64,
    occupied_ticks: Tick,
    queue_delay_ticks: Tick,
    first_tick: Tick,
    last_tick: Tick,
}

impl ActualFabricHopActivity {
    fn record(&mut self, activity: &FabricHopActivity) {
        if self.transfer_count == 0 {
            self.first_tick = activity.ready_tick();
            self.last_tick = activity.arrival_tick();
        } else {
            self.first_tick = self.first_tick.min(activity.ready_tick());
            self.last_tick = self.last_tick.max(activity.arrival_tick());
        }
        self.transfer_count += 1;
        self.byte_count += activity.bytes();
        self.occupied_ticks += activity.occupied_ticks();
        self.queue_delay_ticks += activity.queue_delay_ticks();
    }
}

pub(crate) fn verify_expected_fabric_hop_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_fabric_hop_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_activity[0];
        return Err(missing_fabric_hop_activity_summary(expected));
    };

    for expected in expected_activity {
        let Some(actual) = expected.actual_activity(summary.fabric_hop_activities()) else {
            return Err(missing_fabric_hop_activity_summary(expected));
        };
        if expected.below_minimum(&actual) {
            return Err(WorkloadError::expected_fabric_hop_activity_below_minimum(
                expected.hop_index(),
                expected.link().clone(),
                expected.virtual_network(),
                expected.minimum_transfer_count(),
                actual.transfer_count,
                expected.minimum_byte_count(),
                actual.byte_count,
                expected.minimum_occupied_ticks(),
                actual.occupied_ticks,
                expected.minimum_queue_delay_ticks(),
                actual.queue_delay_ticks,
                expected.required_first_tick(),
                actual.first_tick,
                expected.required_last_tick(),
                actual.last_tick,
            ));
        }
    }
    Ok(())
}

fn missing_fabric_hop_activity_summary(
    expected: &WorkloadExpectedFabricHopActivity,
) -> WorkloadError {
    WorkloadError::MissingFabricHopActivitySummary {
        hop_index: expected.hop_index(),
        link: expected.link().clone(),
        virtual_network: expected.virtual_network(),
        minimum_transfer_count: expected.minimum_transfer_count(),
        minimum_byte_count: expected.minimum_byte_count(),
        minimum_occupied_ticks: expected.minimum_occupied_ticks(),
        minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
        required_first_tick: expected.required_first_tick(),
        required_last_tick: expected.required_last_tick(),
    }
}
