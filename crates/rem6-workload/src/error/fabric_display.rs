use std::fmt;

use rem6_kernel::Tick;

use super::WorkloadError;

pub(super) fn format_fabric_activity_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
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
        WorkloadError::MissingFabricLinkActivitySummary {
            link,
            minimum_transfer_count,
            minimum_active_virtual_network_count,
            minimum_queue_delay_ticks,
            minimum_contended_virtual_network_count,
        } => write!(
            formatter,
            "missing parallel summary for expected fabric link {} activity with at least {minimum_transfer_count} transfers, {minimum_active_virtual_network_count} active virtual networks, {minimum_queue_delay_ticks} queue delay ticks, and {minimum_contended_virtual_network_count} contended virtual networks",
            link.as_str()
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
        } => write!(
            formatter,
            "expected fabric link {} activity to reach at least {minimum_transfer_count} transfers, {minimum_active_virtual_network_count} active virtual networks, {minimum_queue_delay_ticks} queue delay ticks, and {minimum_contended_virtual_network_count} contended virtual networks, got {actual_transfer_count} transfers, {actual_active_virtual_network_count} active virtual networks, {actual_queue_delay_ticks} queue delay ticks, and {actual_contended_virtual_network_count} contended virtual networks",
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
        WorkloadError::MissingFabricVirtualNetworkActivitySummary {
            virtual_network,
            minimum_transfer_count,
            minimum_active_lane_count,
            minimum_queue_delay_ticks,
            minimum_contended_lane_count,
        } => write!(
            formatter,
            "missing parallel summary for expected fabric virtual network {} activity with at least {minimum_transfer_count} transfers, {minimum_active_lane_count} active lanes, {minimum_queue_delay_ticks} queue delay ticks, and {minimum_contended_lane_count} contended lanes",
            virtual_network.get()
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
        } => write!(
            formatter,
            "expected fabric virtual network {} activity to reach at least {minimum_transfer_count} transfers, {minimum_active_lane_count} active lanes, {minimum_queue_delay_ticks} queue delay ticks, and {minimum_contended_lane_count} contended lanes, got {actual_transfer_count} transfers, {actual_active_lane_count} active lanes, {actual_queue_delay_ticks} queue delay ticks, and {actual_contended_lane_count} contended lanes",
            virtual_network.get()
        ),
        _ => unreachable!("fabric activity formatter called for non-fabric activity error"),
    }
}

fn format_optional_tick(tick: &Option<Tick>) -> String {
    tick.map_or_else(|| "none".to_owned(), |tick| tick.to_string())
}
