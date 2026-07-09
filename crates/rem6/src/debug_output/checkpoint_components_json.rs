use crate::{
    formatting::json_escape, Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary,
    Rem6HostO3RuntimeCheckpointChunkSummary,
};

pub(in crate::debug_output) fn checkpoint_components_to_json(
    components: &[Rem6HostCheckpointComponentSummary],
) -> String {
    let components = components
        .iter()
        .map(component_to_json)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{components}]")
}

fn component_to_json(component: &Rem6HostCheckpointComponentSummary) -> String {
    let chunks = component
        .chunks
        .iter()
        .map(chunk_to_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"component\":\"{}\",\"chunk_count\":{},\"payload_bytes\":{},\"chunks\":[{}]}}",
        json_escape(&component.component),
        component.chunk_count,
        component.payload_bytes,
        chunks,
    )
}

fn chunk_to_json(chunk: &Rem6HostCheckpointChunkSummary) -> String {
    let o3_runtime = chunk
        .o3_runtime
        .as_ref()
        .map(|summary| format!(",\"o3_runtime\":{}", o3_runtime_to_json(summary)))
        .unwrap_or_default();
    format!(
        "{{\"name\":\"{}\",\"payload_bytes\":{},\"payload_checksum\":\"0x{:016x}\"{}}}",
        json_escape(&chunk.name),
        chunk.payload_bytes,
        chunk.payload_checksum,
        o3_runtime,
    )
}

fn o3_runtime_to_json(summary: &Rem6HostO3RuntimeCheckpointChunkSummary) -> String {
    format!(
        "{{\"decode_error\":{},\"snapshot_rob_entries\":{},\"snapshot_lsq_entries\":{},\"snapshot_rename_map_entries\":{},\"stats_max_rob_occupancy\":{},\"stats_max_lsq_occupancy\":{},\"stats_rename_map_entries\":{}}}",
        summary.decode_error,
        optional_u64_json(summary.snapshot_rob_entries),
        optional_u64_json(summary.snapshot_lsq_entries),
        optional_u64_json(summary.snapshot_rename_map_entries),
        optional_u64_json(summary.stats_max_rob_occupancy),
        optional_u64_json(summary.stats_max_lsq_occupancy),
        optional_u64_json(summary.stats_rename_map_entries),
    )
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}
