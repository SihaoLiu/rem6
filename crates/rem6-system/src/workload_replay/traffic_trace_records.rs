use std::sync::{Arc, Mutex};

use crate::{RiscvTraceDiagnosticRecord, RiscvTraceErrorRecord, RiscvTraceHtmAccessRecord};

use super::traffic_trace::{
    RiscvWorkloadTraceHtmAbortRecord, RiscvWorkloadTraceHtmBeginRecord,
    RiscvWorkloadTraceMemoryFailureRecord, RiscvWorkloadTraceMemoryResponseRecord,
    RiscvWorkloadTraceMemoryWriteCompletionRecord,
};
use super::traffic_trace_sideband_records::{
    RiscvWorkloadTraceCacheFlushRecord, RiscvWorkloadTraceTlbSyncRecord,
};
use super::traffic_trace_sync::{
    RiscvWorkloadTraceL1InvalidationRecord, RiscvWorkloadTraceSyncRecord,
};

#[derive(Clone, Debug, Default)]
pub(super) struct RiscvWorkloadTraceReplayRecords {
    memory_response_records: Arc<Mutex<Vec<RiscvWorkloadTraceMemoryResponseRecord>>>,
    memory_write_completion_records: Arc<Mutex<Vec<RiscvWorkloadTraceMemoryWriteCompletionRecord>>>,
    memory_failure_records: Arc<Mutex<Vec<RiscvWorkloadTraceMemoryFailureRecord>>>,
    trace_tlb_sync_records: Arc<Mutex<Vec<RiscvWorkloadTraceTlbSyncRecord>>>,
    trace_cache_flush_records: Arc<Mutex<Vec<RiscvWorkloadTraceCacheFlushRecord>>>,
    trace_l1_invalidation_records: Arc<Mutex<Vec<RiscvWorkloadTraceL1InvalidationRecord>>>,
    trace_error_records: Arc<Mutex<Vec<RiscvTraceErrorRecord>>>,
    trace_htm_access_records: Arc<Mutex<Vec<RiscvTraceHtmAccessRecord>>>,
    trace_diagnostic_records: Arc<Mutex<Vec<RiscvTraceDiagnosticRecord>>>,
    sync_records: Arc<Mutex<Vec<RiscvWorkloadTraceSyncRecord>>>,
    htm_begin_records: Arc<Mutex<Vec<RiscvWorkloadTraceHtmBeginRecord>>>,
    htm_abort_records: Arc<Mutex<Vec<RiscvWorkloadTraceHtmAbortRecord>>>,
}

impl RiscvWorkloadTraceReplayRecords {
    pub(super) fn memory_response_snapshot(&self) -> Vec<RiscvWorkloadTraceMemoryResponseRecord> {
        let mut records = self
            .memory_response_records
            .lock()
            .expect("traffic trace replay memory response lock")
            .clone();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn memory_write_completion_snapshot(
        &self,
    ) -> Vec<RiscvWorkloadTraceMemoryWriteCompletionRecord> {
        let mut records = self
            .memory_write_completion_records
            .lock()
            .expect("traffic trace replay memory write completion lock")
            .clone();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn memory_failure_snapshot(&self) -> Vec<RiscvWorkloadTraceMemoryFailureRecord> {
        let mut records = self
            .memory_failure_records
            .lock()
            .expect("traffic trace replay memory failure lock")
            .clone();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn trace_tlb_sync_snapshot(&self) -> Vec<RiscvWorkloadTraceTlbSyncRecord> {
        let mut records = self
            .trace_tlb_sync_records
            .lock()
            .expect("traffic trace replay tlb sync lock")
            .clone();
        records.sort_by_key(|record| (record.tick(), record.sequence()));
        records
    }

    pub(super) fn trace_cache_flush_snapshot(&self) -> Vec<RiscvWorkloadTraceCacheFlushRecord> {
        let mut records = self
            .trace_cache_flush_records
            .lock()
            .expect("traffic trace replay cache flush lock")
            .clone();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn trace_l1_invalidation_snapshot(
        &self,
    ) -> Vec<RiscvWorkloadTraceL1InvalidationRecord> {
        let mut records = self
            .trace_l1_invalidation_records
            .lock()
            .expect("traffic trace replay l1 invalidation lock")
            .clone();
        records.sort_by_key(|record| (record.completion_tick(), record.trace_sequence()));
        records
    }

    pub(super) fn trace_error_snapshot(&self) -> Vec<RiscvTraceErrorRecord> {
        let mut records = self
            .trace_error_records
            .lock()
            .expect("traffic trace replay error lock")
            .clone();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn trace_htm_access_snapshot(&self) -> Vec<RiscvTraceHtmAccessRecord> {
        let mut records = self
            .trace_htm_access_records
            .lock()
            .expect("traffic trace replay htm access lock")
            .clone();
        records.sort_by_key(|record| {
            (
                record.tick(),
                record.sequence(),
                record.line().get(),
                record.kind(),
            )
        });
        records
    }

    pub(super) fn trace_diagnostic_snapshot(&self) -> Vec<RiscvTraceDiagnosticRecord> {
        let mut records = self
            .trace_diagnostic_records
            .lock()
            .expect("traffic trace replay diagnostic lock")
            .clone();
        records.sort_by_key(|record| {
            (
                record.tick(),
                record.target().get(),
                record.line().get(),
                record.address().get(),
            )
        });
        records
    }

    pub(super) fn sync_snapshot(&self) -> Vec<RiscvWorkloadTraceSyncRecord> {
        self.sync_records
            .lock()
            .expect("traffic trace replay sync lock")
            .clone()
    }

    pub(super) fn htm_begin_snapshot(&self) -> Vec<RiscvWorkloadTraceHtmBeginRecord> {
        self.htm_begin_records
            .lock()
            .expect("traffic trace replay htm begin lock")
            .clone()
    }

    pub(super) fn htm_abort_snapshot(&self) -> Vec<RiscvWorkloadTraceHtmAbortRecord> {
        self.htm_abort_records
            .lock()
            .expect("traffic trace replay htm abort lock")
            .clone()
    }

    pub(super) fn record_memory_response(&self, record: RiscvWorkloadTraceMemoryResponseRecord) {
        self.memory_response_records
            .lock()
            .expect("workload trace memory response lock")
            .push(record);
    }

    pub(super) fn record_memory_write_completion(
        &self,
        record: RiscvWorkloadTraceMemoryWriteCompletionRecord,
    ) {
        self.memory_write_completion_records
            .lock()
            .expect("workload trace memory write completion lock")
            .push(record);
    }

    pub(super) fn record_memory_failure(&self, record: RiscvWorkloadTraceMemoryFailureRecord) {
        self.memory_failure_records
            .lock()
            .expect("workload trace memory failure lock")
            .push(record);
    }

    pub(super) fn record_trace_tlb_sync(&self, record: RiscvWorkloadTraceTlbSyncRecord) {
        self.trace_tlb_sync_records
            .lock()
            .expect("workload trace tlb sync lock")
            .push(record);
    }

    pub(super) fn record_trace_cache_flush(&self, record: RiscvWorkloadTraceCacheFlushRecord) {
        self.trace_cache_flush_records
            .lock()
            .expect("workload trace cache flush lock")
            .push(record);
    }

    pub(super) fn record_trace_l1_invalidation(
        &self,
        record: RiscvWorkloadTraceL1InvalidationRecord,
    ) {
        self.trace_l1_invalidation_records
            .lock()
            .expect("workload trace l1 invalidation lock")
            .push(record);
    }

    pub(super) fn record_trace_error(&self, record: RiscvTraceErrorRecord) {
        self.trace_error_records
            .lock()
            .expect("workload trace error lock")
            .push(record);
    }

    pub(super) fn record_trace_htm_access(&self, record: RiscvTraceHtmAccessRecord) {
        self.trace_htm_access_records
            .lock()
            .expect("workload trace htm access lock")
            .push(record);
    }

    pub(super) fn record_trace_diagnostic(&self, record: RiscvTraceDiagnosticRecord) {
        self.trace_diagnostic_records
            .lock()
            .expect("workload trace diagnostic lock")
            .push(record);
    }

    pub(super) fn record_sync(&self, record: RiscvWorkloadTraceSyncRecord) {
        self.sync_records
            .lock()
            .expect("workload trace sync lock")
            .push(record);
    }

    pub(super) fn record_htm_begin(&self, record: RiscvWorkloadTraceHtmBeginRecord) {
        self.htm_begin_records
            .lock()
            .expect("workload trace htm begin lock")
            .push(record);
    }

    pub(super) fn record_htm_abort(&self, record: RiscvWorkloadTraceHtmAbortRecord) {
        self.htm_abort_records
            .lock()
            .expect("workload trace htm abort lock")
            .push(record);
    }
}
