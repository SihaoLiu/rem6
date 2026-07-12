use rem6_cpu::RiscvO3LiveDataHandoff;

use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostO3LiveDataHandoffChunkSummary {
    pub(crate) decode_error: bool,
    pub(crate) outstanding_requests: Option<u64>,
    pub(crate) resident_rows: Option<u64>,
    pub(crate) younger_rows: Option<u64>,
    pub(crate) first_fetch_request_agent: Option<u64>,
    pub(crate) first_fetch_request_sequence: Option<u64>,
    pub(crate) first_data_request_agent: Option<u64>,
    pub(crate) first_data_request_sequence: Option<u64>,
    pub(crate) first_issue_tick: Option<u64>,
    pub(crate) last_issue_tick: Option<u64>,
    pub(crate) first_partition: Option<u64>,
    pub(crate) first_route: Option<u64>,
    pub(crate) first_address: Option<u64>,
    pub(crate) first_bytes: Option<u64>,
    pub(crate) first_o3_sequence: Option<u64>,
    pub(crate) first_trace_sequence: Option<u64>,
}

pub(super) fn decode_o3_live_data_handoff_chunk(
    name: &str,
    payload: &[u8],
) -> Option<Rem6HostO3LiveDataHandoffChunkSummary> {
    if name != RISCV_O3_LIVE_DATA_HANDOFF_CHUNK {
        return None;
    }
    let Ok(handoff) = RiscvO3LiveDataHandoff::decode(payload) else {
        return Some(Rem6HostO3LiveDataHandoffChunkSummary::decode_error());
    };
    let first = handoff.entries().first().copied();
    Some(Rem6HostO3LiveDataHandoffChunkSummary {
        decode_error: false,
        outstanding_requests: Some(handoff.entries().len() as u64),
        resident_rows: Some(handoff.entries().len() as u64),
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
            .max(),
        first_partition: first.map(|entry| u64::from(entry.partition().index())),
        first_route: first.map(|entry| entry.route().get()),
        first_address: first.map(|entry| entry.address().get()),
        first_bytes: first.map(|entry| u64::from(entry.bytes())),
        first_o3_sequence: first.map(|entry| entry.o3_sequence()),
        first_trace_sequence: first.and_then(|entry| entry.trace_sequence()),
    })
}

impl Rem6HostO3LiveDataHandoffChunkSummary {
    fn decode_error() -> Self {
        Self {
            decode_error: true,
            outstanding_requests: None,
            resident_rows: None,
            younger_rows: None,
            first_fetch_request_agent: None,
            first_fetch_request_sequence: None,
            first_data_request_agent: None,
            first_data_request_sequence: None,
            first_issue_tick: None,
            last_issue_tick: None,
            first_partition: None,
            first_route: None,
            first_address: None,
            first_bytes: None,
            first_o3_sequence: None,
            first_trace_sequence: None,
        }
    }
}
