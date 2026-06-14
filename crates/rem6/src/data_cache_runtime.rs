use std::sync::{Arc, Mutex};

use rem6_coherence::{
    CpuResponseRecord, MsiBankCycleRun, MsiBankDirectoryHarness, ParallelCoherenceRunSummary,
    ParallelCoherenceWaitForGraphs,
};
use rem6_kernel::{RecordedConservativeRunSummary, WaitForGraph};
use rem6_memory::{
    AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse, ResponseStatus,
};
use rem6_system::{RiscvDataCacheProtocol, RiscvDataCacheRunRecord, RiscvSystemRun};
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::runtime_memory::{cli_memory_response, CliMemoryRuntime};
use crate::{execute_error, Rem6CliError};

#[derive(Clone)]
pub(super) struct CliDataCacheRuntime {
    layout: CacheLineLayout,
    harness: Arc<Mutex<MsiBankDirectoryHarness>>,
    records: Arc<Mutex<Vec<RiscvDataCacheRunRecord>>>,
    error: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CliDataCacheSummary {
    pub(crate) runs: u64,
    pub(crate) msi_runs: u64,
    pub(crate) mesi_runs: u64,
    pub(crate) moesi_runs: u64,
    pub(crate) chi_runs: u64,
    pub(crate) cpu_responses: u64,
    pub(crate) directory_decisions: u64,
    pub(crate) dram_accesses: u64,
}

impl CliDataCacheSummary {
    pub(crate) fn from_run(run: &RiscvSystemRun) -> Self {
        Self {
            runs: run.data_cache_run_count() as u64,
            msi_runs: run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Msi) as u64,
            mesi_runs: run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Mesi) as u64,
            moesi_runs: run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Moesi) as u64,
            chi_runs: run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Chi) as u64,
            cpu_responses: run
                .data_cache_runs()
                .iter()
                .map(|run| run.cpu_response_count() as u64)
                .sum(),
            directory_decisions: run
                .data_cache_runs()
                .iter()
                .map(|run| run.directory_decision_count() as u64)
                .sum(),
            dram_accesses: run
                .data_cache_runs()
                .iter()
                .map(|run| run.dram_access_count() as u64)
                .sum(),
        }
    }
}

pub(super) fn cli_data_memory_response(
    data_cache: Option<&CliDataCacheRuntime>,
    memory: &CliMemoryRuntime,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    if let Some(data_cache) = data_cache {
        if let Some(outcome) = data_cache.respond(memory, delivery) {
            return outcome;
        }
    }
    cli_memory_response(memory, delivery)
}

impl CliDataCacheRuntime {
    pub(super) fn new_msi_bank<I>(layout: CacheLineLayout, agents: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        Ok(Self {
            layout,
            harness: Arc::new(Mutex::new(
                MsiBankDirectoryHarness::new(layout, agents).map_err(execute_error)?,
            )),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn respond(
        &self,
        memory: &CliMemoryRuntime,
        delivery: &RequestDelivery,
    ) -> Option<TargetOutcome> {
        self.respond_for_request(memory, delivery.tick(), delivery.request())
    }

    fn respond_for_request(
        &self,
        memory: &CliMemoryRuntime,
        tick: u64,
        request: &MemoryRequest,
    ) -> Option<TargetOutcome> {
        if request.line_layout() != self.layout {
            return None;
        }
        if !self.ensure_backing(memory, request) {
            return None;
        }

        Some(
            self.respond_inner(memory, tick, request)
                .unwrap_or_else(|error| {
                    self.record_error(error);
                    TargetOutcome::Respond(MemoryResponse::retry(request))
                }),
        )
    }

    pub(super) fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.records
            .lock()
            .expect("CLI data cache record lock")
            .clone()
    }

    pub(super) fn take_error(&self) -> Option<Rem6CliError> {
        self.error
            .lock()
            .expect("CLI data cache error lock")
            .take()
            .map(execute_error)
    }

    fn ensure_backing(&self, memory: &CliMemoryRuntime, request: &MemoryRequest) -> bool {
        let line = self.layout.line_address(request.line_address());
        {
            let harness = self.harness.lock().expect("CLI MSI data cache lock");
            if harness.backing_line(line).is_some()
                || harness.directory_line_addresses().contains(&line)
                || harness.cache_agents().into_iter().any(|agent| {
                    harness
                        .cache_line_addresses(agent)
                        .map(|lines| lines.contains(&line))
                        .unwrap_or(false)
                })
            {
                return true;
            }
        }

        let Some(data) =
            memory.read_guest_memory(line.get(), self.layout.bytes() as usize, self.layout)
        else {
            return false;
        };

        self.harness
            .lock()
            .expect("CLI MSI data cache lock")
            .insert_backing_line(line, data)
            .is_ok()
    }

    fn respond_inner(
        &self,
        memory: &CliMemoryRuntime,
        start_tick: u64,
        request: &MemoryRequest,
    ) -> Result<TargetOutcome, Rem6CliError> {
        let mut harness = self.harness.lock().expect("CLI MSI data cache lock");
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        let cycle_run = harness
            .submit_parallel_cycle(start_tick, [(request.id().agent(), request.clone())])
            .map_err(execute_error)?;
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record =
            response_record(&harness.cpu_responses(), responses_before, request.id());
        let outcome = match response_record.as_ref() {
            Some(record) => response_record_to_target_outcome(request, start_tick, record)?,
            None if request.requires_response() && !request.returns_data() => {
                TargetOutcome::Respond(
                    MemoryResponse::completed(request, None).map_err(execute_error)?,
                )
            }
            None if !request.requires_response() => TargetOutcome::NoResponse,
            None => return Err(execute_error("CLI MSI data cache response missing")),
        };
        let line_data = harness
            .functional_read_line(self.layout.line_address(request.line_address()))
            .map_err(execute_error)?
            .data()
            .map(<[u8]>::to_vec);
        drop(harness);

        if let Some(line_data) = line_data {
            self.sync_request_range(memory, request, &line_data)?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Msi,
                msi_bank_cycle_run_summary(&cycle_run, directory_decision_count),
            ));
        Ok(outcome)
    }

    fn sync_request_range(
        &self,
        memory: &CliMemoryRuntime,
        request: &MemoryRequest,
        line_data: &[u8],
    ) -> Result<(), Rem6CliError> {
        let range = request.range();
        let offset = usize::try_from(self.layout.line_offset(range.start()))
            .map_err(|_| execute_error("CLI MSI data cache request offset is too large"))?;
        let bytes = usize::try_from(range.size().bytes())
            .map_err(|_| execute_error("CLI MSI data cache request size is too large"))?;
        let end = offset
            .checked_add(bytes)
            .ok_or_else(|| execute_error("CLI MSI data cache request range overflows"))?;
        let Some(data) = line_data.get(offset..end) else {
            return Err(execute_error(
                "CLI MSI data cache request range exceeds cache line",
            ));
        };
        if memory.write_guest_memory(range.start().get(), data, self.layout) {
            Ok(())
        } else {
            Err(execute_error("failed to sync CLI MSI data cache request"))
        }
    }

    fn record_error(&self, error: Rem6CliError) {
        let mut slot = self.error.lock().expect("CLI data cache error lock");
        if slot.is_none() {
            *slot = Some(error.to_string());
        }
    }
}

fn response_record(
    responses: &[CpuResponseRecord],
    responses_before: usize,
    request: MemoryRequestId,
) -> Option<CpuResponseRecord> {
    responses
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == request)
        .cloned()
}

fn response_record_to_target_outcome(
    request: &MemoryRequest,
    start_tick: u64,
    record: &CpuResponseRecord,
) -> Result<TargetOutcome, Rem6CliError> {
    let response = match record.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(request, record.data().map(<[u8]>::to_vec))
                .map_err(execute_error)?
        }
        ResponseStatus::Retry => MemoryResponse::retry(request),
        ResponseStatus::StoreConditionalFailed => {
            MemoryResponse::store_conditional_failed(request).map_err(execute_error)?
        }
    };
    let delay = record.tick().saturating_sub(start_tick);
    if delay == 0 {
        Ok(TargetOutcome::Respond(response))
    } else {
        Ok(TargetOutcome::RespondAfter { delay, response })
    }
}

fn msi_bank_cycle_run_summary(
    run: &MsiBankCycleRun,
    directory_decision_count: usize,
) -> ParallelCoherenceRunSummary {
    ParallelCoherenceRunSummary::new(
        RecordedConservativeRunSummary::empty(run.tick()),
        run.response_count(),
        directory_decision_count,
        0,
        Vec::new(),
        Vec::new(),
        ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new()),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rem6_memory::{
        AccessSize, Address, MemoryRequestId, MemoryTargetId, PartitionedMemoryStore,
        ResponseStatus,
    };

    use super::*;

    const TARGET: MemoryTargetId = MemoryTargetId::new(7);

    #[test]
    fn respond_for_request_records_internal_error_before_retry() {
        let layout = CacheLineLayout::new(32).unwrap();
        let memory = memory_with_line(layout);
        let runtime = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)]).unwrap();
        let request = MemoryRequest::read_shared(
            MemoryRequestId::new(AgentId::new(0), 1),
            Address::new(0x1018),
            AccessSize::new(16).unwrap(),
            layout,
        )
        .unwrap();

        let outcome = runtime
            .respond_for_request(&memory, 4, &request)
            .expect("matching layout should be handled by the data cache runtime");

        let TargetOutcome::Respond(response) = outcome else {
            panic!("internal data-cache errors should return a retry response");
        };
        assert_eq!(response.status(), ResponseStatus::Retry);
        let error = runtime
            .take_error()
            .expect("internal data-cache error should be recorded");
        let error = error.to_string();
        assert!(error.contains("outside cache line"), "{error}");
    }

    fn memory_with_line(layout: CacheLineLayout) -> CliMemoryRuntime {
        let mut store = PartitionedMemoryStore::new();
        store.add_partition(TARGET, layout).unwrap();
        store
            .map_region(
                TARGET,
                Address::new(0x1000),
                AccessSize::new(layout.bytes()).unwrap(),
            )
            .unwrap();
        store
            .insert_line(
                TARGET,
                Address::new(0x1000),
                vec![0xa5; layout.bytes() as usize],
            )
            .unwrap();
        CliMemoryRuntime::Store(Arc::new(Mutex::new(store)))
    }
}
