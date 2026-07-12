use rem6_cpu::{
    RiscvO3LiveDataHandoff, RiscvO3LiveDataHandoffOperation, RiscvO3LiveDataHandoffTarget,
};

use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostO3LiveDataHandoffChunkSummary {
    pub(crate) decode_error: bool,
    pub(crate) schema_version: Option<u64>,
    pub(crate) row_count: Option<u64>,
    pub(crate) resident_rows: Option<u64>,
    pub(crate) forwarded_rows: Option<u64>,
    pub(crate) partial_overlay_rows: Option<u64>,
    pub(crate) younger_rows: Option<u64>,
    pub(crate) first_fetch_request_agent: Option<u64>,
    pub(crate) first_fetch_request_sequence: Option<u64>,
    pub(crate) first_data_request_agent: Option<u64>,
    pub(crate) first_data_request_sequence: Option<u64>,
    pub(crate) first_issue_tick: Option<u64>,
    pub(crate) last_issue_tick: Option<u64>,
    pub(crate) first_operation: Option<RiscvO3LiveDataHandoffOperation>,
    pub(crate) first_target: Option<Rem6HostO3LiveDataHandoffTargetSummary>,
    pub(crate) first_address: Option<u64>,
    pub(crate) first_bytes: Option<u64>,
    pub(crate) first_o3_sequence: Option<u64>,
    pub(crate) first_trace_sequence: Option<u64>,
    pub(crate) first_forwarded_fetch_request_agent: Option<u64>,
    pub(crate) first_forwarded_fetch_request_sequence: Option<u64>,
    pub(crate) first_forwarded_data_request_agent: Option<u64>,
    pub(crate) first_forwarded_data_request_sequence: Option<u64>,
    pub(crate) first_forwarding_source_data_request_agent: Option<u64>,
    pub(crate) first_forwarding_source_data_request_sequence: Option<u64>,
    pub(crate) first_forwarded_issue_tick: Option<u64>,
    pub(crate) first_forwarded_response_tick: Option<u64>,
    pub(crate) first_forwarded_address: Option<u64>,
    pub(crate) first_forwarded_bytes: Option<u64>,
    pub(crate) first_forwarded_data: Option<Vec<u8>>,
    pub(crate) first_forwarded_o3_sequence: Option<u64>,
    pub(crate) first_forwarded_trace_sequence: Option<u64>,
    pub(crate) first_partial_overlay_load_data_request_agent: Option<u64>,
    pub(crate) first_partial_overlay_load_data_request_sequence: Option<u64>,
    pub(crate) first_partial_overlay_source_data_request_agent: Option<u64>,
    pub(crate) first_partial_overlay_source_data_request_sequence: Option<u64>,
    pub(crate) first_partial_overlay_source_address: Option<u64>,
    pub(crate) first_partial_overlay_source_bytes: Option<u64>,
    pub(crate) first_partial_overlay_source_data: Option<Vec<u8>>,
    pub(crate) first_partial_overlay_address: Option<u64>,
    pub(crate) first_partial_overlay_bytes: Option<u64>,
    pub(crate) first_partial_overlay_forwarded_mask: Option<u64>,
    pub(crate) first_partial_overlay_response_owned_mask: Option<u64>,
    pub(crate) first_partial_overlay_forwarded_bytes: Option<u64>,
    pub(crate) first_partial_overlay_data: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Rem6HostO3LiveDataHandoffTargetSummary {
    Memory {
        source_partition: u64,
        route: u64,
    },
    Mmio {
        source_partition: u64,
        target_partition: u64,
        request_latency: u64,
        response_latency: u64,
    },
}

pub(super) fn decode_o3_live_data_handoff_chunk(
    name: &str,
    payload: &[u8],
) -> Option<Rem6HostO3LiveDataHandoffChunkSummary> {
    if name != RISCV_O3_LIVE_DATA_HANDOFF_CHUNK {
        return None;
    }
    let Ok((handoff, schema_version)) = RiscvO3LiveDataHandoff::decode_with_version(payload) else {
        return Some(Rem6HostO3LiveDataHandoffChunkSummary::decode_error());
    };
    let first = handoff.entries().first().copied();
    let first_forwarded = handoff.forwarded_rows().first().copied();
    let first_partial_overlay = handoff.partial_overlays().first().copied();
    Some(Rem6HostO3LiveDataHandoffChunkSummary {
        decode_error: false,
        schema_version: Some(u64::from(schema_version)),
        row_count: Some(handoff.entries().len() as u64),
        resident_rows: Some(handoff.resident_rows() as u64),
        forwarded_rows: Some(handoff.forwarded_rows().len() as u64),
        partial_overlay_rows: Some(handoff.partial_overlays().len() as u64),
        younger_rows: Some(u64::from(handoff.younger_rows())),
        first_fetch_request_agent: first
            .map(|entry| u64::from(entry.fetch_request().agent().get())),
        first_fetch_request_sequence: first.map(|entry| entry.fetch_request().sequence()),
        first_data_request_agent: first.map(|entry| u64::from(entry.data_request().agent().get())),
        first_data_request_sequence: first.map(|entry| entry.data_request().sequence()),
        first_issue_tick: first.map(|entry| entry.issue_tick()),
        last_issue_tick: handoff
            .entries()
            .iter()
            .map(|entry| entry.issue_tick())
            .chain(handoff.forwarded_rows().iter().map(|row| row.issue_tick()))
            .max(),
        first_operation: first.map(|entry| entry.operation()),
        first_target: first.map(|entry| match entry.target() {
            RiscvO3LiveDataHandoffTarget::Memory { route } => {
                Rem6HostO3LiveDataHandoffTargetSummary::Memory {
                    source_partition: u64::from(entry.partition().index()),
                    route: route.get(),
                }
            }
            RiscvO3LiveDataHandoffTarget::Mmio { route } => {
                Rem6HostO3LiveDataHandoffTargetSummary::Mmio {
                    source_partition: u64::from(route.source_partition().index()),
                    target_partition: u64::from(route.target_partition().index()),
                    request_latency: route.request_latency(),
                    response_latency: route.response_latency(),
                }
            }
        }),
        first_address: first.map(|entry| entry.address().get()),
        first_bytes: first.map(|entry| u64::from(entry.bytes())),
        first_o3_sequence: first.map(|entry| entry.o3_sequence()),
        first_trace_sequence: first.and_then(|entry| entry.trace_sequence()),
        first_forwarded_fetch_request_agent: first_forwarded
            .map(|row| u64::from(row.fetch_request().agent().get())),
        first_forwarded_fetch_request_sequence: first_forwarded
            .map(|row| row.fetch_request().sequence()),
        first_forwarded_data_request_agent: first_forwarded
            .map(|row| u64::from(row.data_request().agent().get())),
        first_forwarded_data_request_sequence: first_forwarded
            .map(|row| row.data_request().sequence()),
        first_forwarding_source_data_request_agent: first_forwarded
            .map(|row| u64::from(row.source_data_request().agent().get())),
        first_forwarding_source_data_request_sequence: first_forwarded
            .map(|row| row.source_data_request().sequence()),
        first_forwarded_issue_tick: first_forwarded.map(|row| row.issue_tick()),
        first_forwarded_response_tick: first_forwarded.map(|row| row.response_tick()),
        first_forwarded_address: first_forwarded.map(|row| row.address().get()),
        first_forwarded_bytes: first_forwarded.map(|row| u64::from(row.bytes())),
        first_forwarded_data: first_forwarded.map(|row| row.data().to_vec()),
        first_forwarded_o3_sequence: first_forwarded.map(|row| row.o3_sequence()),
        first_forwarded_trace_sequence: first_forwarded.and_then(|row| row.trace_sequence()),
        first_partial_overlay_load_data_request_agent: first_partial_overlay
            .map(|overlay| u64::from(overlay.load_data_request().agent().get())),
        first_partial_overlay_load_data_request_sequence: first_partial_overlay
            .map(|overlay| overlay.load_data_request().sequence()),
        first_partial_overlay_source_data_request_agent: first_partial_overlay
            .map(|overlay| u64::from(overlay.source_data_request().agent().get())),
        first_partial_overlay_source_data_request_sequence: first_partial_overlay
            .map(|overlay| overlay.source_data_request().sequence()),
        first_partial_overlay_source_address: first_partial_overlay
            .map(|overlay| overlay.source_address().get()),
        first_partial_overlay_source_bytes: first_partial_overlay
            .map(|overlay| u64::from(overlay.source_bytes())),
        first_partial_overlay_source_data: first_partial_overlay
            .map(|overlay| overlay.source_data().to_vec()),
        first_partial_overlay_address: first_partial_overlay.map(|overlay| overlay.address().get()),
        first_partial_overlay_bytes: first_partial_overlay
            .map(|overlay| u64::from(overlay.bytes())),
        first_partial_overlay_forwarded_mask: first_partial_overlay
            .map(|overlay| u64::from(overlay.forwarded_mask())),
        first_partial_overlay_response_owned_mask: first_partial_overlay
            .map(|overlay| u64::from(overlay.response_owned_mask())),
        first_partial_overlay_forwarded_bytes: first_partial_overlay
            .map(|overlay| u64::from(overlay.forwarded_bytes())),
        first_partial_overlay_data: first_partial_overlay.map(|overlay| overlay.data().to_vec()),
    })
}

impl Rem6HostO3LiveDataHandoffChunkSummary {
    fn decode_error() -> Self {
        Self {
            decode_error: true,
            schema_version: None,
            row_count: None,
            resident_rows: None,
            forwarded_rows: None,
            partial_overlay_rows: None,
            younger_rows: None,
            first_fetch_request_agent: None,
            first_fetch_request_sequence: None,
            first_data_request_agent: None,
            first_data_request_sequence: None,
            first_issue_tick: None,
            last_issue_tick: None,
            first_operation: None,
            first_target: None,
            first_address: None,
            first_bytes: None,
            first_o3_sequence: None,
            first_trace_sequence: None,
            first_forwarded_fetch_request_agent: None,
            first_forwarded_fetch_request_sequence: None,
            first_forwarded_data_request_agent: None,
            first_forwarded_data_request_sequence: None,
            first_forwarding_source_data_request_agent: None,
            first_forwarding_source_data_request_sequence: None,
            first_forwarded_issue_tick: None,
            first_forwarded_response_tick: None,
            first_forwarded_address: None,
            first_forwarded_bytes: None,
            first_forwarded_data: None,
            first_forwarded_o3_sequence: None,
            first_forwarded_trace_sequence: None,
            first_partial_overlay_load_data_request_agent: None,
            first_partial_overlay_load_data_request_sequence: None,
            first_partial_overlay_source_data_request_agent: None,
            first_partial_overlay_source_data_request_sequence: None,
            first_partial_overlay_source_address: None,
            first_partial_overlay_source_bytes: None,
            first_partial_overlay_source_data: None,
            first_partial_overlay_address: None,
            first_partial_overlay_bytes: None,
            first_partial_overlay_forwarded_mask: None,
            first_partial_overlay_response_owned_mask: None,
            first_partial_overlay_forwarded_bytes: None,
            first_partial_overlay_data: None,
        }
    }

    pub(crate) fn to_json(&self) -> String {
        let first_address = self
            .first_address
            .map(|address| format!("\"0x{address:x}\""))
            .unwrap_or_else(|| "null".to_string());
        let first_target = self
            .first_target
            .map(Rem6HostO3LiveDataHandoffTargetSummary::to_json)
            .unwrap_or_else(|| "null".to_string());
        let first_operation = self
            .first_operation
            .map(|operation| match operation {
                RiscvO3LiveDataHandoffOperation::Load => "\"load\"",
                RiscvO3LiveDataHandoffOperation::Store => "\"store\"",
            })
            .unwrap_or("null");
        let first_forwarded_address = self
            .first_forwarded_address
            .map(|address| format!("\"0x{address:x}\""))
            .unwrap_or_else(|| "null".to_string());
        let first_forwarded_operation = self
            .first_forwarded_data_request_agent
            .map(|_| "\"load\"")
            .unwrap_or("null");
        let first_forwarded_data = self
            .first_forwarded_data
            .as_deref()
            .map(|data| {
                format!(
                    "\"{}\"",
                    data.iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                )
            })
            .unwrap_or_else(|| "null".to_string());
        let first_partial_overlay_address = self
            .first_partial_overlay_address
            .map(|address| format!("\"0x{address:x}\""))
            .unwrap_or_else(|| "null".to_string());
        let first_partial_overlay_operation = self
            .first_partial_overlay_load_data_request_agent
            .map(|_| "\"load\"")
            .unwrap_or("null");
        let first_partial_overlay_data = self
            .first_partial_overlay_data
            .as_deref()
            .map(|data| {
                format!(
                    "\"{}\"",
                    data.iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                )
            })
            .unwrap_or_else(|| "null".to_string());
        let first_partial_overlay_source_address = self
            .first_partial_overlay_source_address
            .map(|address| format!("\"0x{address:x}\""))
            .unwrap_or_else(|| "null".to_string());
        let first_partial_overlay_source_data = self
            .first_partial_overlay_source_data
            .as_deref()
            .map(|data| {
                format!(
                    "\"{}\"",
                    data.iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                )
            })
            .unwrap_or_else(|| "null".to_string());
        let first_partition = self
            .first_target
            .map(Rem6HostO3LiveDataHandoffTargetSummary::source_partition);
        let first_route = self
            .first_target
            .and_then(Rem6HostO3LiveDataHandoffTargetSummary::memory_route);
        format!(
            "{{\"decode_error\":{},\"schema_version\":{},\"outstanding_requests\":{},\"resident_rows\":{},\"transport_owned_rows\":{},\"forwarded_rows\":{},\"partial_overlay_rows\":{},\"younger_rows\":{},\"first_fetch_request_agent\":{},\"first_fetch_request_sequence\":{},\"first_data_request_agent\":{},\"first_data_request_sequence\":{},\"first_issue_tick\":{},\"last_issue_tick\":{},\"first_operation\":{},\"first_partition\":{},\"first_route\":{},\"first_target\":{},\"first_address\":{},\"first_bytes\":{},\"first_o3_sequence\":{},\"first_trace_sequence\":{},\"first_forwarded_operation\":{},\"first_forwarded_fetch_request_agent\":{},\"first_forwarded_fetch_request_sequence\":{},\"first_forwarded_data_request_agent\":{},\"first_forwarded_data_request_sequence\":{},\"first_forwarding_source_data_request_agent\":{},\"first_forwarding_source_data_request_sequence\":{},\"first_forwarded_issue_tick\":{},\"first_forwarded_response_tick\":{},\"first_forwarded_address\":{},\"first_forwarded_bytes\":{},\"first_forwarded_data_hex\":{},\"first_forwarded_o3_sequence\":{},\"first_forwarded_trace_sequence\":{},\"first_partial_overlay_operation\":{},\"first_partial_overlay_load_data_request_agent\":{},\"first_partial_overlay_load_data_request_sequence\":{},\"first_partial_overlay_source_data_request_agent\":{},\"first_partial_overlay_source_data_request_sequence\":{},\"first_partial_overlay_source_address\":{},\"first_partial_overlay_source_bytes\":{},\"first_partial_overlay_source_data_hex\":{},\"first_partial_overlay_address\":{},\"first_partial_overlay_bytes\":{},\"first_partial_overlay_forwarded_mask\":{},\"first_partial_overlay_response_owned_mask\":{},\"first_partial_overlay_forwarded_bytes\":{},\"first_partial_overlay_forwarded_data_hex\":{}}}",
            self.decode_error,
            optional_u64_json(self.schema_version),
            optional_u64_json(self.row_count),
            optional_u64_json(self.resident_rows),
            optional_u64_json(self.row_count),
            optional_u64_json(self.forwarded_rows),
            optional_u64_json(self.partial_overlay_rows),
            optional_u64_json(self.younger_rows),
            optional_u64_json(self.first_fetch_request_agent),
            optional_u64_json(self.first_fetch_request_sequence),
            optional_u64_json(self.first_data_request_agent),
            optional_u64_json(self.first_data_request_sequence),
            optional_u64_json(self.first_issue_tick),
            optional_u64_json(self.last_issue_tick),
            first_operation,
            optional_u64_json(first_partition),
            optional_u64_json(first_route),
            first_target,
            first_address,
            optional_u64_json(self.first_bytes),
            optional_u64_json(self.first_o3_sequence),
            optional_u64_json(self.first_trace_sequence),
            first_forwarded_operation,
            optional_u64_json(self.first_forwarded_fetch_request_agent),
            optional_u64_json(self.first_forwarded_fetch_request_sequence),
            optional_u64_json(self.first_forwarded_data_request_agent),
            optional_u64_json(self.first_forwarded_data_request_sequence),
            optional_u64_json(self.first_forwarding_source_data_request_agent),
            optional_u64_json(self.first_forwarding_source_data_request_sequence),
            optional_u64_json(self.first_forwarded_issue_tick),
            optional_u64_json(self.first_forwarded_response_tick),
            first_forwarded_address,
            optional_u64_json(self.first_forwarded_bytes),
            first_forwarded_data,
            optional_u64_json(self.first_forwarded_o3_sequence),
            optional_u64_json(self.first_forwarded_trace_sequence),
            first_partial_overlay_operation,
            optional_u64_json(self.first_partial_overlay_load_data_request_agent),
            optional_u64_json(self.first_partial_overlay_load_data_request_sequence),
            optional_u64_json(self.first_partial_overlay_source_data_request_agent),
            optional_u64_json(self.first_partial_overlay_source_data_request_sequence),
            first_partial_overlay_source_address,
            optional_u64_json(self.first_partial_overlay_source_bytes),
            first_partial_overlay_source_data,
            first_partial_overlay_address,
            optional_u64_json(self.first_partial_overlay_bytes),
            optional_u64_json(self.first_partial_overlay_forwarded_mask),
            optional_u64_json(self.first_partial_overlay_response_owned_mask),
            optional_u64_json(self.first_partial_overlay_forwarded_bytes),
            first_partial_overlay_data,
        )
    }
}

impl Rem6HostO3LiveDataHandoffTargetSummary {
    const fn source_partition(self) -> u64 {
        match self {
            Self::Memory {
                source_partition, ..
            }
            | Self::Mmio {
                source_partition, ..
            } => source_partition,
        }
    }

    const fn memory_route(self) -> Option<u64> {
        match self {
            Self::Memory { route, .. } => Some(route),
            Self::Mmio { .. } => None,
        }
    }

    fn to_json(self) -> String {
        match self {
            Self::Memory {
                source_partition,
                route,
            } => format!(
                "{{\"kind\":\"memory\",\"source_partition\":{source_partition},\"route\":{route}}}"
            ),
            Self::Mmio {
                source_partition,
                target_partition,
                request_latency,
                response_latency,
            } => format!(
                "{{\"kind\":\"mmio\",\"source_partition\":{source_partition},\"target_partition\":{target_partition},\"request_latency\":{request_latency},\"response_latency\":{response_latency}}}"
            ),
        }
    }
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}
