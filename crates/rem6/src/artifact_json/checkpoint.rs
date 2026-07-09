use crate::formatting::json_escape;
use crate::{
    Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary, Rem6HostCheckpointSummary,
    Rem6HostExecutionModeSummary, Rem6HostO3RuntimeCheckpointChunkSummary,
};

impl Rem6HostCheckpointSummary {
    pub(crate) fn to_json(&self) -> String {
        let components = self
            .components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let execution_modes = self
            .execution_modes
            .iter()
            .map(Rem6HostExecutionModeSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"tick\":{},\"event\":{},\"source\":{},\"label\":\"{}\",\"manifest_tick\":{},\"component_count\":{},\"chunk_count\":{},\"payload_bytes\":{},\"execution_mode_authority_present\":{},\"execution_mode_authority_cleared\":{},\"execution_mode_authority_decode_error\":{},\"execution_modes\":[{}],\"components\":[{}]}}",
            self.tick,
            self.event,
            self.source,
            json_escape(&self.label),
            self.manifest_tick,
            self.component_count,
            self.chunk_count,
            self.payload_bytes,
            self.execution_mode_authority_present,
            self.execution_mode_authority_cleared,
            self.execution_mode_authority_decode_error,
            execution_modes,
            components,
        )
    }
}

impl Rem6HostCheckpointComponentSummary {
    pub(super) fn to_json(&self) -> String {
        let chunks = self
            .chunks
            .iter()
            .map(Rem6HostCheckpointChunkSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"component\":\"{}\",\"chunk_count\":{},\"payload_bytes\":{},\"chunks\":[{}]}}",
            json_escape(&self.component),
            self.chunk_count,
            self.payload_bytes,
            chunks,
        )
    }
}

impl Rem6HostCheckpointChunkSummary {
    fn to_json(&self) -> String {
        let o3_runtime = self
            .o3_runtime
            .as_ref()
            .map(|summary| format!(",\"o3_runtime\":{}", summary.to_json()))
            .unwrap_or_default();
        format!(
            "{{\"name\":\"{}\",\"payload_bytes\":{},\"payload_checksum\":\"0x{:016x}\"{}}}",
            json_escape(&self.name),
            self.payload_bytes,
            self.payload_checksum,
            o3_runtime,
        )
    }
}

impl Rem6HostO3RuntimeCheckpointChunkSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"decode_error\":{},\"snapshot_rob_entries\":{},\"snapshot_lsq_entries\":{},\"snapshot_rename_map_entries\":{},\"stats_max_rob_occupancy\":{},\"stats_max_lsq_occupancy\":{},\"stats_rename_map_entries\":{},\"stats_lsq_operation_load\":{},\"stats_lsq_operation_store\":{},\"stats_lsq_data_latency_samples\":{},\"stats_lsq_data_latency_ticks\":{},\"stats_lsq_data_latency_max_ticks\":{},\"stats_lsq_data_latency_min_ticks\":{},\"stats_lsq_data_latency_avg_ticks\":{},\"stats_lsq_operation_load_latency_samples\":{},\"stats_lsq_operation_load_latency_ticks\":{},\"stats_lsq_operation_store_latency_samples\":{},\"stats_lsq_operation_store_latency_ticks\":{}}}",
            self.decode_error,
            optional_u64_json(self.snapshot_rob_entries),
            optional_u64_json(self.snapshot_lsq_entries),
            optional_u64_json(self.snapshot_rename_map_entries),
            optional_u64_json(self.stats_max_rob_occupancy),
            optional_u64_json(self.stats_max_lsq_occupancy),
            optional_u64_json(self.stats_rename_map_entries),
            optional_u64_json(self.stats_lsq_operation_load),
            optional_u64_json(self.stats_lsq_operation_store),
            optional_u64_json(self.stats_lsq_data_latency_samples),
            optional_u64_json(self.stats_lsq_data_latency_ticks),
            optional_u64_json(self.stats_lsq_data_latency_max_ticks),
            optional_u64_json(self.stats_lsq_data_latency_min_ticks),
            optional_u64_json(self.stats_lsq_data_latency_avg_ticks),
            optional_u64_json(self.stats_lsq_operation_load_latency_samples),
            optional_u64_json(self.stats_lsq_operation_load_latency_ticks),
            optional_u64_json(self.stats_lsq_operation_store_latency_samples),
            optional_u64_json(self.stats_lsq_operation_store_latency_ticks),
        )
    }
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}
