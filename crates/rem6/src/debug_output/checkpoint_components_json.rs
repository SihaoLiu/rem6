use crate::{
    formatting::json_escape, Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary,
    Rem6HostO3LiveDataHandoffChunkSummary, Rem6HostO3RuntimeCheckpointChunkSummary,
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
        .map(|summary| {
            format!(
                ",\"o3_live_data_handoff\":{}",
                o3_live_data_handoff_to_json(summary)
            )
        })
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

fn o3_live_data_handoff_to_json(summary: &Rem6HostO3LiveDataHandoffChunkSummary) -> String {
    let first_address = summary
        .first_address
        .map(|address| format!("\"0x{address:x}\""))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"decode_error\":{},\"outstanding_requests\":{},\"resident_rows\":{},\"younger_rows\":{},\"first_fetch_request_agent\":{},\"first_fetch_request_sequence\":{},\"first_data_request_agent\":{},\"first_data_request_sequence\":{},\"first_issue_tick\":{},\"last_issue_tick\":{},\"first_partition\":{},\"first_route\":{},\"first_address\":{},\"first_bytes\":{},\"first_o3_sequence\":{},\"first_trace_sequence\":{}}}",
        summary.decode_error,
        optional_u64_json(summary.outstanding_requests),
        optional_u64_json(summary.resident_rows),
        optional_u64_json(summary.younger_rows),
        optional_u64_json(summary.first_fetch_request_agent),
        optional_u64_json(summary.first_fetch_request_sequence),
        optional_u64_json(summary.first_data_request_agent),
        optional_u64_json(summary.first_data_request_sequence),
        optional_u64_json(summary.first_issue_tick),
        optional_u64_json(summary.last_issue_tick),
        optional_u64_json(summary.first_partition),
        optional_u64_json(summary.first_route),
        first_address,
        optional_u64_json(summary.first_bytes),
        optional_u64_json(summary.first_o3_sequence),
        optional_u64_json(summary.first_trace_sequence),
    )
}
