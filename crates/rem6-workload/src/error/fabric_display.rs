use std::fmt;

use rem6_fabric::FabricLinkId;
use rem6_kernel::Tick;

use super::WorkloadError;

pub(super) fn format_fabric_activity_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::ZeroExpectedFabricHopActivity {
            hop_index,
            link,
            virtual_network,
        } => write!(
            formatter,
            "expected fabric hop {hop_index} on link {} virtual network {} activity must require a positive transfer, byte, occupancy, or queue delay count",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::DuplicateExpectedFabricHopActivity {
            hop_index,
            link,
            virtual_network,
        } => write!(
            formatter,
            "expected fabric hop {hop_index} on link {} virtual network {} activity is already declared",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::InvalidExpectedFabricHopActivityWindow {
            hop_index,
            link,
            virtual_network,
            first_tick,
            last_tick,
        } => write!(
            formatter,
            "expected fabric hop {hop_index} on link {} virtual network {} activity window first tick {first_tick} is after last tick {last_tick}",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::MissingFabricHopActivitySummary {
            hop_index,
            link,
            virtual_network,
            minimum_transfer_count,
            minimum_byte_count,
            minimum_occupied_ticks,
            minimum_queue_delay_ticks,
            required_first_tick,
            required_last_tick,
        } => write!(
            formatter,
            "missing parallel summary for expected fabric hop {hop_index} on link {} virtual network {} activity with at least {minimum_transfer_count} transfers, {minimum_byte_count} bytes, {minimum_occupied_ticks} occupied ticks, {minimum_queue_delay_ticks} queue delay ticks, first tick {}, and last tick {}",
            link.as_str(),
            virtual_network.get(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ExpectedFabricHopActivityBelowMinimum {
            hop_index,
            link,
            virtual_network,
            minimum_transfer_count,
            actual_transfer_count,
            minimum_byte_count,
            actual_byte_count,
            minimum_occupied_ticks,
            actual_occupied_ticks,
            minimum_queue_delay_ticks,
            actual_queue_delay_ticks,
            required_first_tick,
            actual_first_tick,
            required_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected fabric hop {hop_index} on link {} virtual network {} activity to reach at least {minimum_transfer_count} transfers, {minimum_byte_count} bytes, {minimum_occupied_ticks} occupied ticks, {minimum_queue_delay_ticks} queue delay ticks, first tick {}, and last tick {}, got {actual_transfer_count} transfers, {actual_byte_count} bytes, {actual_occupied_ticks} occupied ticks, {actual_queue_delay_ticks} queue delay ticks, first tick {actual_first_tick}, and last tick {actual_last_tick}",
            link.as_str(),
            virtual_network.get(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ZeroExpectedFabricLaneActivity {
            link,
            virtual_network,
        } => write!(
            formatter,
            "expected fabric lane {} virtual network {} activity must require a positive transfer, byte, occupancy, or queue delay count",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::DuplicateExpectedFabricLaneActivity {
            link,
            virtual_network,
        } => write!(
            formatter,
            "expected fabric lane {} virtual network {} activity is already declared",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::InvalidExpectedFabricLaneActivityWindow {
            link,
            virtual_network,
            first_tick,
            last_tick,
        } => write!(
            formatter,
            "expected fabric lane {} virtual network {} activity window first tick {first_tick} is after last tick {last_tick}",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::InvalidExpectedFabricLaneActivityQueueDelayBudget {
            link,
            virtual_network,
            maximum_queue_delay_ticks,
            maximum_max_queue_delay_ticks,
        } => write!(
            formatter,
            "expected fabric lane {} virtual network {} activity queue-delay budget peak {maximum_max_queue_delay_ticks} is above total {maximum_queue_delay_ticks}",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::MissingFabricLaneActivitySummary {
            link,
            virtual_network,
            minimum_transfer_count,
            minimum_byte_count,
            minimum_occupied_ticks,
            minimum_queue_delay_ticks,
            minimum_max_queue_delay_ticks,
            required_first_tick,
            required_last_tick,
        } => write!(
            formatter,
            "missing parallel summary for expected fabric lane {} virtual network {} activity with at least {minimum_transfer_count} transfers, {minimum_byte_count} bytes, {minimum_occupied_ticks} occupied ticks, {minimum_queue_delay_ticks} queue delay ticks, maximum queue delay {minimum_max_queue_delay_ticks}, first tick {}, and last tick {}",
            link.as_str(),
            virtual_network.get(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ExpectedFabricLaneActivityBelowMinimum {
            link,
            virtual_network,
            minimum_transfer_count,
            actual_transfer_count,
            minimum_byte_count,
            actual_byte_count,
            minimum_occupied_ticks,
            actual_occupied_ticks,
            minimum_queue_delay_ticks,
            actual_queue_delay_ticks,
            minimum_max_queue_delay_ticks,
            actual_max_queue_delay_ticks,
            required_first_tick,
            actual_first_tick,
            required_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected fabric lane {} virtual network {} activity to reach at least {minimum_transfer_count} transfers, {minimum_byte_count} bytes, {minimum_occupied_ticks} occupied ticks, {minimum_queue_delay_ticks} queue delay ticks, maximum queue delay {minimum_max_queue_delay_ticks}, first tick {}, and last tick {}, got {actual_transfer_count} transfers, {actual_byte_count} bytes, {actual_occupied_ticks} occupied ticks, {actual_queue_delay_ticks} queue delay ticks, maximum queue delay {actual_max_queue_delay_ticks}, first tick {actual_first_tick}, and last tick {actual_last_tick}",
            link.as_str(),
            virtual_network.get(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ExpectedFabricLaneActivityAboveMaximum {
            link,
            virtual_network,
            maximum_queue_delay_ticks,
            actual_queue_delay_ticks,
            maximum_max_queue_delay_ticks,
            actual_max_queue_delay_ticks,
        } => write!(
            formatter,
            "expected fabric lane {} virtual network {} activity to stay within {maximum_queue_delay_ticks} queue delay ticks and {maximum_max_queue_delay_ticks} maximum queue delay ticks, got {actual_queue_delay_ticks} queue delay ticks and {actual_max_queue_delay_ticks} maximum queue delay ticks",
            link.as_str(),
            virtual_network.get()
        ),
        WorkloadError::ZeroExpectedFabricLinkActivity { link } => write!(
            formatter,
            "expected fabric link {} activity must require a positive transfer, active virtual network, queue delay, or contended virtual network count",
            link.as_str()
        ),
        WorkloadError::DuplicateExpectedFabricLinkActivity { link } => write!(
            formatter,
            "expected fabric link {} activity is already declared",
            link.as_str()
        ),
        WorkloadError::InvalidExpectedFabricLinkActivityWindow {
            link,
            first_tick,
            last_tick,
        } => write!(
            formatter,
            "expected fabric link {} activity window first tick {first_tick} is after last tick {last_tick}",
            link.as_str()
        ),
        WorkloadError::InvalidExpectedFabricLinkActivityQueueDelayBudget {
            link,
            maximum_queue_delay_ticks,
            maximum_max_queue_delay_ticks,
        } => write!(
            formatter,
            "expected fabric link {} activity queue-delay budget peak {maximum_max_queue_delay_ticks} is above total {maximum_queue_delay_ticks}",
            link.as_str()
        ),
        WorkloadError::MissingFabricLinkActivitySummary {
            link,
            minimum_transfer_count,
            minimum_active_virtual_network_count,
            minimum_queue_delay_ticks,
            minimum_contended_virtual_network_count,
            required_first_tick,
            required_last_tick,
        } => write!(
            formatter,
            "missing parallel summary for expected fabric link {} activity with at least {minimum_transfer_count} transfers, {minimum_active_virtual_network_count} active virtual networks, {minimum_queue_delay_ticks} queue delay ticks, {minimum_contended_virtual_network_count} contended virtual networks, first tick {}, and last tick {}",
            link.as_str(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ExpectedFabricLinkActivityBelowMinimum {
            link,
            minimum_transfer_count,
            actual_transfer_count,
            minimum_active_virtual_network_count,
            actual_active_virtual_network_count,
            minimum_queue_delay_ticks,
            actual_queue_delay_ticks,
            minimum_contended_virtual_network_count,
            actual_contended_virtual_network_count,
            required_first_tick,
            actual_first_tick,
            required_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected fabric link {} activity to reach at least {minimum_transfer_count} transfers, {minimum_active_virtual_network_count} active virtual networks, {minimum_queue_delay_ticks} queue delay ticks, {minimum_contended_virtual_network_count} contended virtual networks, first tick {}, and last tick {}, got {actual_transfer_count} transfers, {actual_active_virtual_network_count} active virtual networks, {actual_queue_delay_ticks} queue delay ticks, {actual_contended_virtual_network_count} contended virtual networks, first tick {actual_first_tick}, and last tick {actual_last_tick}",
            link.as_str(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ExpectedFabricLinkActivityAboveMaximum {
            link,
            maximum_queue_delay_ticks,
            actual_queue_delay_ticks,
            maximum_max_queue_delay_ticks,
            actual_max_queue_delay_ticks,
        } => write!(
            formatter,
            "expected fabric link {} activity to stay within {maximum_queue_delay_ticks} queue delay ticks and {maximum_max_queue_delay_ticks} maximum queue delay ticks, got {actual_queue_delay_ticks} queue delay ticks and {actual_max_queue_delay_ticks} maximum queue delay ticks",
            link.as_str()
        ),
        WorkloadError::ZeroExpectedFabricVirtualNetworkActivity { virtual_network } => write!(
            formatter,
            "expected fabric virtual network {} activity must require a positive transfer, active lane, queue delay, or contended lane count",
            virtual_network.get()
        ),
        WorkloadError::DuplicateExpectedFabricVirtualNetworkActivity { virtual_network } => {
            write!(
                formatter,
                "expected fabric virtual network {} activity is already declared",
                virtual_network.get()
            )
        }
        WorkloadError::DuplicateExpectedFabricVirtualNetworkActivityCoverageLink {
            virtual_network,
            link,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity coverage link {} is already declared",
            virtual_network.get(),
            link.as_str()
        ),
        WorkloadError::InvalidExpectedFabricVirtualNetworkActivityWindow {
            virtual_network,
            first_tick,
            last_tick,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity window first tick {first_tick} is after last tick {last_tick}",
            virtual_network.get()
        ),
        WorkloadError::InvalidExpectedFabricVirtualNetworkActivityQueueDelayBudget {
            virtual_network,
            maximum_queue_delay_ticks,
            maximum_max_queue_delay_ticks,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity queue-delay budget peak {maximum_max_queue_delay_ticks} is above total {maximum_queue_delay_ticks}",
            virtual_network.get()
        ),
        WorkloadError::InvalidExpectedFabricVirtualNetworkActivityLaneBudget {
            virtual_network,
            maximum_active_lane_count,
            maximum_contended_lane_count,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity lane budget contended lane count {maximum_contended_lane_count} is above active lane count {maximum_active_lane_count}",
            virtual_network.get()
        ),
        WorkloadError::MissingFabricVirtualNetworkActivitySummary {
            virtual_network,
            minimum_transfer_count,
            minimum_active_lane_count,
            minimum_queue_delay_ticks,
            minimum_contended_lane_count,
            required_first_tick,
            required_last_tick,
        } => write!(
            formatter,
            "missing parallel summary for expected fabric virtual network {} activity with at least {minimum_transfer_count} transfers, {minimum_active_lane_count} active lanes, {minimum_queue_delay_ticks} queue delay ticks, {minimum_contended_lane_count} contended lanes, first tick {}, and last tick {}",
            virtual_network.get(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::MissingFabricVirtualNetworkLinkCoverage {
            virtual_network,
            required_links,
        } => write!(
            formatter,
            "missing fabric lane summary for expected fabric virtual network {} link coverage {}",
            virtual_network.get(),
            format_links(required_links)
        ),
        WorkloadError::ExpectedFabricVirtualNetworkActivityBelowMinimum {
            virtual_network,
            minimum_transfer_count,
            actual_transfer_count,
            minimum_active_lane_count,
            actual_active_lane_count,
            minimum_queue_delay_ticks,
            actual_queue_delay_ticks,
            minimum_contended_lane_count,
            actual_contended_lane_count,
            required_first_tick,
            actual_first_tick,
            required_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity to reach at least {minimum_transfer_count} transfers, {minimum_active_lane_count} active lanes, {minimum_queue_delay_ticks} queue delay ticks, {minimum_contended_lane_count} contended lanes, first tick {}, and last tick {}, got {actual_transfer_count} transfers, {actual_active_lane_count} active lanes, {actual_queue_delay_ticks} queue delay ticks, {actual_contended_lane_count} contended lanes, first tick {actual_first_tick}, and last tick {actual_last_tick}",
            virtual_network.get(),
            format_optional_tick(required_first_tick),
            format_optional_tick(required_last_tick)
        ),
        WorkloadError::ExpectedFabricVirtualNetworkActivityAboveMaximum {
            virtual_network,
            maximum_queue_delay_ticks,
            actual_queue_delay_ticks,
            maximum_max_queue_delay_ticks,
            actual_max_queue_delay_ticks,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity to stay within {maximum_queue_delay_ticks} queue delay ticks and {maximum_max_queue_delay_ticks} maximum queue delay ticks, got {actual_queue_delay_ticks} queue delay ticks and {actual_max_queue_delay_ticks} maximum queue delay ticks",
            virtual_network.get()
        ),
        WorkloadError::ExpectedFabricVirtualNetworkActivityAboveLaneBudget {
            virtual_network,
            maximum_active_lane_count,
            actual_active_lane_count,
            maximum_contended_lane_count,
            actual_contended_lane_count,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity to stay within {maximum_active_lane_count} active lanes and {maximum_contended_lane_count} contended lanes, got {actual_active_lane_count} active lanes and {actual_contended_lane_count} contended lanes",
            virtual_network.get()
        ),
        WorkloadError::ExpectedFabricVirtualNetworkLinkCoverageMissing {
            virtual_network,
            required_links,
            actual_links,
            missing_links,
        } => write!(
            formatter,
            "expected fabric virtual network {} activity to cover links {}, got links {}, missing links {}",
            virtual_network.get(),
            format_links(required_links),
            format_links(actual_links),
            format_links(missing_links)
        ),
        _ => unreachable!("fabric activity formatter called for non-fabric activity error"),
    }
}

fn format_optional_tick(tick: &Option<Tick>) -> String {
    tick.map_or_else(|| "none".to_owned(), |tick| tick.to_string())
}

fn format_links(links: &[FabricLinkId]) -> String {
    let names = links
        .iter()
        .map(|link| link.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{names}]")
}
