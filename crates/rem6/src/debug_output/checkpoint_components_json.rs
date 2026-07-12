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
    let o3_live_data_handoff = chunk
        .o3_live_data_handoff
        .as_ref()
        .map(|summary| format!(",\"o3_live_data_handoff\":{}", summary.to_json()))
        .unwrap_or_default();
    format!(
        "{{\"name\":\"{}\",\"payload_bytes\":{},\"payload_checksum\":\"0x{:016x}\"{}{}}}",
        json_escape(&chunk.name),
        chunk.payload_bytes,
        chunk.payload_checksum,
        o3_runtime,
        o3_live_data_handoff,
    )
}

fn o3_runtime_to_json(summary: &Rem6HostO3RuntimeCheckpointChunkSummary) -> String {
    let numeric_fields = summary
        .numeric_fields()
        .into_iter()
        .map(|(name, value)| format!("\"{}\":{}", json_escape(name), optional_u64_json(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"decode_error\":{},{}}}",
        summary.decode_error, numeric_fields
    )
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}
