use crate::{
    formatting::json_escape, Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary,
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
    format!(
        "{{\"name\":\"{}\",\"payload_bytes\":{},\"payload_checksum\":\"0x{:016x}\"}}",
        json_escape(&chunk.name),
        chunk.payload_bytes,
        chunk.payload_checksum,
    )
}
