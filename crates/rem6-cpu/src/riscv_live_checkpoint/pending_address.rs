use rem6_isa_riscv::Register;
use rem6_kernel::Tick;
use rem6_memory::{AddressRange, MemoryRequestId};

use crate::{CpuFetchEvent, O3LoadStoreQueueKind, O3RenameMapEntry};

use super::{
    invalid, RiscvO3LiveCheckpointError, RiscvO3LiveCheckpointIssueRow,
    RiscvO3LiveCheckpointPayload,
};

#[path = "pending_address/graph.rs"]
mod graph;
#[path = "pending_address/store.rs"]
mod store;

pub(crate) const MAX_PENDING_ADDRESSES: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointPendingDataAddress {
    pub sequence: u64,
    pub fetch: CpuFetchEvent,
    pub consumed_requests: Vec<MemoryRequestId>,
    pub fetch_predecessor_request: MemoryRequestId,
    pub producer_register: Register,
    pub destination: Option<O3RenameMapEntry>,
    pub producer_sequence: u64,
    pub root_sequence: u64,
    pub root_fetch_request: MemoryRequestId,
    pub root_range: AddressRange,
    pub root_atomic: bool,
    pub lsq_kind: O3LoadStoreQueueKind,
    pub expected_lsq_bytes: u32,
    pub published_producer_ready_tick: Option<Tick>,
    pub requested_wake_tick: Option<Tick>,
}

pub(super) fn validate_pending_profile(
    value: &RiscvO3LiveCheckpointPayload,
) -> Result<(), RiscvO3LiveCheckpointError> {
    let rows = value.pending_addresses.as_slice();
    if rows.is_empty() {
        return Err(invalid("pending-address profile lacks its pending rows"));
    }
    let expected_issue = rows
        .iter()
        .map(|pending| RiscvO3LiveCheckpointIssueRow {
            sequence: pending.sequence,
            fetch_request: pending.fetch.request_id(),
        })
        .collect::<Vec<_>>();
    let expected_resident = rows
        .iter()
        .map(|pending| pending.sequence)
        .collect::<Vec<_>>();
    if !value.events.is_empty()
        || value.issue_rows != expected_issue
        || !value.rename_rows.is_empty()
        || value.resident_sequences != expected_resident
        || !value.executed_fetch_requests.is_empty()
        || !value.issued_fetch_requests.is_empty()
        || !value.writeback_counted_sequences.is_empty()
        || !value.writeback_published_sequences.is_empty()
        || value.reservation.is_some()
        || value.completed_result.is_some()
        || value.service.telemetry.current_occupancy != rows.len() as u64
        || value.service.telemetry.peak_occupancy < rows.len() as u64
        || !value
            .finalized_writeback
            .is_valid_without_live_calendar_at(value.captured_tick)
        || value.service.requested_tick != value.wake.tick
        || rows
            .iter()
            .any(|pending| value.wake.partition != pending.fetch.partition())
    {
        return Err(invalid("pending-address ownership is inconsistent"));
    }
    match rows {
        [pending] if pending.destination.is_none() => store::validate(value, pending),
        rows => graph::validate(value, rows),
    }
}
