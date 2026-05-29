use rem6_fabric::{FabricLinkId, VirtualNetworkId};
use rem6_kernel::Tick;

use crate::{WorkloadError, WorkloadParallelRemoteFlowScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadCheckpointComponentChunkSummaryBelowMinimumError {
    pub(crate) label: String,
    pub(crate) component: String,
    pub(crate) chunk: String,
    pub(crate) minimum_payload_bytes: usize,
    pub(crate) actual_payload_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelRemoteFlowMergeSummaryError {
    pub(crate) scope: WorkloadParallelRemoteFlowScope,
    pub(crate) source: u32,
    pub(crate) target: u32,
    pub(crate) merged_send_count: usize,
    pub(crate) scoped_send_count: usize,
    pub(crate) merged_first_tick: Option<Tick>,
    pub(crate) scoped_first_tick: Tick,
    pub(crate) merged_last_tick: Option<Tick>,
    pub(crate) scoped_last_tick: Tick,
    pub(crate) merged_minimum_delay: Option<Tick>,
    pub(crate) scoped_minimum_delay: Option<Tick>,
    pub(crate) merged_maximum_delay: Option<Tick>,
    pub(crate) scoped_maximum_delay: Option<Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadFabricHopActivityBelowMinimumError {
    pub(crate) hop_index: usize,
    pub(crate) link: FabricLinkId,
    pub(crate) virtual_network: VirtualNetworkId,
    pub(crate) minimum_transfer_count: usize,
    pub(crate) actual_transfer_count: usize,
    pub(crate) minimum_byte_count: u64,
    pub(crate) actual_byte_count: u64,
    pub(crate) minimum_occupied_ticks: Tick,
    pub(crate) actual_occupied_ticks: Tick,
    pub(crate) minimum_queue_delay_ticks: Tick,
    pub(crate) actual_queue_delay_ticks: Tick,
    pub(crate) required_first_tick: Option<Tick>,
    pub(crate) actual_first_tick: Tick,
    pub(crate) required_last_tick: Option<Tick>,
    pub(crate) actual_last_tick: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadFabricLaneActivityBelowMinimumError {
    pub(crate) link: FabricLinkId,
    pub(crate) virtual_network: VirtualNetworkId,
    pub(crate) minimum_transfer_count: usize,
    pub(crate) actual_transfer_count: usize,
    pub(crate) minimum_byte_count: u64,
    pub(crate) actual_byte_count: u64,
    pub(crate) minimum_occupied_ticks: Tick,
    pub(crate) actual_occupied_ticks: Tick,
    pub(crate) minimum_queue_delay_ticks: Tick,
    pub(crate) actual_queue_delay_ticks: Tick,
    pub(crate) minimum_max_queue_delay_ticks: Tick,
    pub(crate) actual_max_queue_delay_ticks: Tick,
    pub(crate) required_first_tick: Option<Tick>,
    pub(crate) actual_first_tick: Tick,
    pub(crate) required_last_tick: Option<Tick>,
    pub(crate) actual_last_tick: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadFabricLinkActivityBelowMinimumError {
    pub(crate) link: FabricLinkId,
    pub(crate) minimum_transfer_count: usize,
    pub(crate) actual_transfer_count: usize,
    pub(crate) minimum_active_virtual_network_count: usize,
    pub(crate) actual_active_virtual_network_count: usize,
    pub(crate) minimum_queue_delay_ticks: Tick,
    pub(crate) actual_queue_delay_ticks: Tick,
    pub(crate) minimum_contended_virtual_network_count: usize,
    pub(crate) actual_contended_virtual_network_count: usize,
    pub(crate) required_first_tick: Option<Tick>,
    pub(crate) actual_first_tick: Tick,
    pub(crate) required_last_tick: Option<Tick>,
    pub(crate) actual_last_tick: Tick,
}

impl WorkloadError {
    pub fn checkpoint_component_chunk_summary_below_minimum(
        label: impl Into<String>,
        component: impl Into<String>,
        chunk: impl Into<String>,
        minimum_payload_bytes: usize,
        actual_payload_bytes: usize,
    ) -> Self {
        Self::CheckpointComponentChunkSummaryBelowMinimum(Box::new(
            WorkloadCheckpointComponentChunkSummaryBelowMinimumError {
                label: label.into(),
                component: component.into(),
                chunk: chunk.into(),
                minimum_payload_bytes,
                actual_payload_bytes,
            },
        ))
    }

    pub fn checkpoint_restore_component_chunk_summary_below_minimum(
        label: impl Into<String>,
        component: impl Into<String>,
        chunk: impl Into<String>,
        minimum_payload_bytes: usize,
        actual_payload_bytes: usize,
    ) -> Self {
        Self::CheckpointRestoreComponentChunkSummaryBelowMinimum(Box::new(
            WorkloadCheckpointComponentChunkSummaryBelowMinimumError {
                label: label.into(),
                component: component.into(),
                chunk: chunk.into(),
                minimum_payload_bytes,
                actual_payload_bytes,
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn invalid_parallel_remote_flow_merge_summary(
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        merged_send_count: usize,
        scoped_send_count: usize,
        merged_first_tick: Option<Tick>,
        scoped_first_tick: Tick,
        merged_last_tick: Option<Tick>,
        scoped_last_tick: Tick,
        merged_minimum_delay: Option<Tick>,
        scoped_minimum_delay: Option<Tick>,
        merged_maximum_delay: Option<Tick>,
        scoped_maximum_delay: Option<Tick>,
    ) -> Self {
        Self::InvalidParallelRemoteFlowMergeSummary(Box::new(
            WorkloadParallelRemoteFlowMergeSummaryError {
                scope,
                source,
                target,
                merged_send_count,
                scoped_send_count,
                merged_first_tick,
                scoped_first_tick,
                merged_last_tick,
                scoped_last_tick,
                merged_minimum_delay,
                scoped_minimum_delay,
                merged_maximum_delay,
                scoped_maximum_delay,
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn expected_fabric_hop_activity_below_minimum(
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_byte_count: u64,
        actual_byte_count: u64,
        minimum_occupied_ticks: Tick,
        actual_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    ) -> Self {
        Self::ExpectedFabricHopActivityBelowMinimum(Box::new(
            WorkloadFabricHopActivityBelowMinimumError {
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
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn expected_fabric_lane_activity_below_minimum(
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_byte_count: u64,
        actual_byte_count: u64,
        minimum_occupied_ticks: Tick,
        actual_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        minimum_max_queue_delay_ticks: Tick,
        actual_max_queue_delay_ticks: Tick,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    ) -> Self {
        Self::ExpectedFabricLaneActivityBelowMinimum(Box::new(
            WorkloadFabricLaneActivityBelowMinimumError {
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
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn expected_fabric_link_activity_below_minimum(
        link: FabricLinkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_active_virtual_network_count: usize,
        actual_active_virtual_network_count: usize,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        minimum_contended_virtual_network_count: usize,
        actual_contended_virtual_network_count: usize,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    ) -> Self {
        Self::ExpectedFabricLinkActivityBelowMinimum(Box::new(
            WorkloadFabricLinkActivityBelowMinimumError {
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
            },
        ))
    }
}
