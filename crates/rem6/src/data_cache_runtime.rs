use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_coherence::{
    ChiCpuResponseRecord, ChiDirectoryLineHarness, CpuResponseRecord, LineBackingStore,
    MesiCpuResponseRecord, MesiDirectoryLineHarness, MoesiCpuResponseRecord,
    MoesiDirectoryLineHarness, MsiBankCycleRun, MsiBankDirectoryHarness,
    ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs,
};
use rem6_kernel::{RecordedConservativeRunSummary, WaitForGraph};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_system::{
    RiscvDataCacheProtocol, RiscvDataCacheRunRecord, RiscvGuestMemoryMapRequest,
    RiscvGuestMemoryMapResult, RiscvSystemRun, RiscvSystemRunDriver,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::runtime_memory::{cli_memory_response, CliMemoryRuntime};
use crate::{execute_error, Rem6CliError};

#[derive(Clone)]
pub(super) struct CliDataCacheRuntime {
    layout: CacheLineLayout,
    harness: Arc<Mutex<CliDataCacheHarness>>,
    records: Arc<Mutex<Vec<RiscvDataCacheRunRecord>>>,
    error: Arc<Mutex<Option<String>>>,
}

enum CliDataCacheHarness {
    Msi(MsiBankDirectoryHarness),
    Mesi(CliMesiLineHarnesses),
    Moesi(CliMoesiLineHarnesses),
    Chi(CliChiLineHarnesses),
}

struct CliMesiLineHarnesses {
    agents: Vec<AgentId>,
    lines: BTreeMap<Address, MesiDirectoryLineHarness>,
}

struct CliMoesiLineHarnesses {
    agents: Vec<AgentId>,
    lines: BTreeMap<Address, MoesiDirectoryLineHarness>,
}

struct CliChiLineHarnesses {
    agents: Vec<AgentId>,
    lines: BTreeMap<Address, ChiDirectoryLineHarness>,
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
        Self::from_records(&run.data_cache_run_records())
    }

    pub(crate) fn from_records(records: &[RiscvDataCacheRunRecord]) -> Self {
        Self {
            runs: records.len() as u64,
            msi_runs: records
                .iter()
                .filter(|record| record.protocol() == Some(RiscvDataCacheProtocol::Msi))
                .count() as u64,
            mesi_runs: records
                .iter()
                .filter(|record| record.protocol() == Some(RiscvDataCacheProtocol::Mesi))
                .count() as u64,
            moesi_runs: records
                .iter()
                .filter(|record| record.protocol() == Some(RiscvDataCacheProtocol::Moesi))
                .count() as u64,
            chi_runs: records
                .iter()
                .filter(|record| record.protocol() == Some(RiscvDataCacheProtocol::Chi))
                .count() as u64,
            cpu_responses: records
                .iter()
                .map(|record| record.summary().cpu_response_count() as u64)
                .sum(),
            directory_decisions: records
                .iter()
                .map(|record| record.summary().directory_decision_count() as u64)
                .sum(),
            dram_accesses: records
                .iter()
                .map(|record| record.summary().dram_access_count() as u64)
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

pub(super) fn with_riscv_syscall_data_cache_memory_io(
    driver: RiscvSystemRunDriver,
    memory: CliMemoryRuntime,
    data_cache: Option<CliDataCacheRuntime>,
    line_layout: CacheLineLayout,
) -> RiscvSystemRunDriver {
    let read_memory = memory.clone();
    let write_memory = memory.clone();
    let write_data_cache = data_cache.clone();
    driver.with_riscv_syscall_emulation_and_guest_memory_io_map_handler(
        move |address, bytes| read_memory.read_guest_memory(address, bytes, line_layout),
        move |address, bytes| {
            write_guest_memory_with_data_cache_invalidation(
                &write_memory,
                write_data_cache.as_ref(),
                address,
                bytes,
                line_layout,
            )
        },
        move |request| {
            map_guest_memory_with_data_cache_invalidation(
                &memory,
                data_cache.as_ref(),
                request,
                line_layout,
            )
        },
    )
}

fn write_guest_memory_with_data_cache_invalidation(
    memory: &CliMemoryRuntime,
    data_cache: Option<&CliDataCacheRuntime>,
    address: u64,
    bytes: &[u8],
    line_layout: CacheLineLayout,
) -> bool {
    if !memory.write_guest_memory(address, bytes, line_layout) {
        return false;
    }
    data_cache
        .map(CliDataCacheRuntime::invalidate_cached_guest_memory)
        .unwrap_or(true)
}

fn map_guest_memory_with_data_cache_invalidation(
    memory: &CliMemoryRuntime,
    data_cache: Option<&CliDataCacheRuntime>,
    request: RiscvGuestMemoryMapRequest,
    line_layout: CacheLineLayout,
) -> RiscvGuestMemoryMapResult {
    let result = memory.map_guest_memory(
        request.address(),
        request.bytes(),
        line_layout,
        request.replace_existing(),
    );
    match data_cache {
        Some(data_cache) if !data_cache.invalidate_cached_guest_memory() => {
            RiscvGuestMemoryMapResult::Failed
        }
        _ => result,
    }
}

impl CliDataCacheRuntime {
    pub(super) fn new_msi_bank<I>(layout: CacheLineLayout, agents: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        Ok(Self {
            layout,
            harness: Arc::new(Mutex::new(CliDataCacheHarness::Msi(
                MsiBankDirectoryHarness::new(layout, agents).map_err(execute_error)?,
            ))),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn new_mesi_lines<I>(
        layout: CacheLineLayout,
        agents: I,
    ) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let agents = agents.into_iter().collect::<Vec<_>>();
        if agents.is_empty() {
            return Err(execute_error(
                "CLI MESI data cache requires at least one agent",
            ));
        }
        Ok(Self {
            layout,
            harness: Arc::new(Mutex::new(CliDataCacheHarness::Mesi(
                CliMesiLineHarnesses {
                    agents,
                    lines: BTreeMap::new(),
                },
            ))),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn new_moesi_lines<I>(
        layout: CacheLineLayout,
        agents: I,
    ) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let agents = agents.into_iter().collect::<Vec<_>>();
        if agents.is_empty() {
            return Err(execute_error(
                "CLI MOESI data cache requires at least one agent",
            ));
        }
        Ok(Self {
            layout,
            harness: Arc::new(Mutex::new(CliDataCacheHarness::Moesi(
                CliMoesiLineHarnesses {
                    agents,
                    lines: BTreeMap::new(),
                },
            ))),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn new_chi_lines<I>(layout: CacheLineLayout, agents: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let agents = agents.into_iter().collect::<Vec<_>>();
        if agents.is_empty() {
            return Err(execute_error(
                "CLI CHI data cache requires at least one agent",
            ));
        }
        Ok(Self {
            layout,
            harness: Arc::new(Mutex::new(CliDataCacheHarness::Chi(CliChiLineHarnesses {
                agents,
                lines: BTreeMap::new(),
            }))),
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
        let backing_dram_access_count = self.ensure_backing(memory, request)?;

        Some(
            self.respond_inner(memory, tick, request, backing_dram_access_count)
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

    pub(super) fn invalidate_cached_guest_memory(&self) -> bool {
        match self.invalidate_cached_guest_memory_inner() {
            Ok(()) => true,
            Err(error) => {
                self.record_error(error);
                false
            }
        }
    }

    fn invalidate_cached_guest_memory_inner(&self) -> Result<(), Rem6CliError> {
        let mut harness = self.harness.lock().expect("CLI data cache lock");
        match &mut *harness {
            CliDataCacheHarness::Msi(harness) => {
                let agents = harness.cache_agents();
                *harness =
                    MsiBankDirectoryHarness::new(self.layout, agents).map_err(execute_error)?;
            }
            CliDataCacheHarness::Mesi(harnesses) => {
                harnesses.lines.clear();
            }
            CliDataCacheHarness::Moesi(harnesses) => {
                harnesses.lines.clear();
            }
            CliDataCacheHarness::Chi(harnesses) => {
                harnesses.lines.clear();
            }
        }
        Ok(())
    }

    fn ensure_backing(&self, memory: &CliMemoryRuntime, request: &MemoryRequest) -> Option<usize> {
        let line = self.layout.line_address(request.line_address());
        {
            let harness = self.harness.lock().expect("CLI data cache lock");
            match &*harness {
                CliDataCacheHarness::Msi(harness) => {
                    if harness.backing_line(line).is_some()
                        || harness.directory_line_addresses().contains(&line)
                        || harness.cache_agents().into_iter().any(|agent| {
                            harness
                                .cache_line_addresses(agent)
                                .map(|lines| lines.contains(&line))
                                .unwrap_or(false)
                        })
                    {
                        return Some(0);
                    }
                }
                CliDataCacheHarness::Mesi(harnesses) => {
                    if harnesses.lines.contains_key(&line) {
                        return Some(0);
                    }
                }
                CliDataCacheHarness::Moesi(harnesses) => {
                    if harnesses.lines.contains_key(&line) {
                        return Some(0);
                    }
                }
                CliDataCacheHarness::Chi(harnesses) => {
                    if harnesses.lines.contains_key(&line) {
                        return Some(0);
                    }
                }
            }
        }

        let data =
            memory.read_guest_memory(line.get(), self.layout.bytes() as usize, self.layout)?;

        let mut harness = self.harness.lock().expect("CLI data cache lock");
        let inserted = match &mut *harness {
            CliDataCacheHarness::Msi(harness) => harness.insert_backing_line(line, data).is_ok(),
            CliDataCacheHarness::Mesi(harnesses) => {
                let Ok(backing) = LineBackingStore::new(self.layout, line, data) else {
                    return None;
                };
                let Ok(line_harness) = MesiDirectoryLineHarness::new(
                    self.layout,
                    line,
                    backing,
                    harnesses.agents.iter().copied(),
                ) else {
                    return None;
                };
                harnesses.lines.insert(line, line_harness);
                true
            }
            CliDataCacheHarness::Moesi(harnesses) => {
                let Ok(backing) = LineBackingStore::new(self.layout, line, data) else {
                    return None;
                };
                let Ok(line_harness) = MoesiDirectoryLineHarness::new(
                    self.layout,
                    line,
                    backing,
                    harnesses.agents.iter().copied(),
                ) else {
                    return None;
                };
                harnesses.lines.insert(line, line_harness);
                true
            }
            CliDataCacheHarness::Chi(harnesses) => {
                let Ok(backing) = LineBackingStore::new(self.layout, line, data) else {
                    return None;
                };
                let Ok(line_harness) = ChiDirectoryLineHarness::new(
                    self.layout,
                    line,
                    backing,
                    harnesses.agents.iter().copied(),
                ) else {
                    return None;
                };
                harnesses.lines.insert(line, line_harness);
                true
            }
        };
        if inserted {
            Some(if memory.uses_dram() { 1 } else { 0 })
        } else {
            None
        }
    }

    fn respond_inner(
        &self,
        memory: &CliMemoryRuntime,
        start_tick: u64,
        request: &MemoryRequest,
        backing_dram_access_count: usize,
    ) -> Result<TargetOutcome, Rem6CliError> {
        let mut harness = self.harness.lock().expect("CLI data cache lock");
        match &mut *harness {
            CliDataCacheHarness::Msi(harness) => self.respond_msi_inner(
                memory,
                start_tick,
                request,
                harness,
                backing_dram_access_count,
            ),
            CliDataCacheHarness::Mesi(harnesses) => self.respond_mesi_inner(
                memory,
                start_tick,
                request,
                harnesses,
                backing_dram_access_count,
            ),
            CliDataCacheHarness::Moesi(harnesses) => self.respond_moesi_inner(
                memory,
                start_tick,
                request,
                harnesses,
                backing_dram_access_count,
            ),
            CliDataCacheHarness::Chi(harnesses) => self.respond_chi_inner(
                memory,
                start_tick,
                request,
                harnesses,
                backing_dram_access_count,
            ),
        }
    }

    fn respond_msi_inner(
        &self,
        memory: &CliMemoryRuntime,
        start_tick: u64,
        request: &MemoryRequest,
        harness: &mut MsiBankDirectoryHarness,
        backing_dram_access_count: usize,
    ) -> Result<TargetOutcome, Rem6CliError> {
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

        if let Some(line_data) = line_data {
            self.sync_request_range(memory, request, &line_data)?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Msi,
                msi_bank_cycle_run_summary(
                    &cycle_run,
                    directory_decision_count,
                    backing_dram_access_count,
                ),
            ));
        Ok(outcome)
    }

    fn respond_mesi_inner(
        &self,
        memory: &CliMemoryRuntime,
        start_tick: u64,
        request: &MemoryRequest,
        harnesses: &mut CliMesiLineHarnesses,
        backing_dram_access_count: usize,
    ) -> Result<TargetOutcome, Rem6CliError> {
        let line = self.layout.line_address(request.line_address());
        let harness = harnesses
            .lines
            .get_mut(&line)
            .ok_or_else(|| execute_error("CLI MESI data cache backing line missing"))?;
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        harness
            .submit_cpu_request(request.id().agent(), request.clone())
            .map_err(execute_error)?;
        let responses = harness.cpu_responses();
        let cpu_response_count = responses.len().saturating_sub(responses_before);
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(&responses, responses_before, request.id());
        let outcome = match response_record.as_ref() {
            Some(record) => response_record_to_target_outcome(request, start_tick, record)?,
            None if request.requires_response() && !request.returns_data() => {
                TargetOutcome::Respond(
                    MemoryResponse::completed(request, None).map_err(execute_error)?,
                )
            }
            None if !request.requires_response() => TargetOutcome::NoResponse,
            None => return Err(execute_error("CLI MESI data cache response missing")),
        };
        if let Some(line_data) = harness
            .cache_data(request.id().agent())
            .map_err(execute_error)?
        {
            self.sync_request_range(memory, request, &line_data)?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Mesi,
                data_cache_run_summary(
                    start_tick,
                    cpu_response_count,
                    directory_decision_count,
                    backing_dram_access_count,
                ),
            ));
        Ok(outcome)
    }

    fn respond_moesi_inner(
        &self,
        memory: &CliMemoryRuntime,
        start_tick: u64,
        request: &MemoryRequest,
        harnesses: &mut CliMoesiLineHarnesses,
        backing_dram_access_count: usize,
    ) -> Result<TargetOutcome, Rem6CliError> {
        let line = self.layout.line_address(request.line_address());
        let harness = harnesses
            .lines
            .get_mut(&line)
            .ok_or_else(|| execute_error("CLI MOESI data cache backing line missing"))?;
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        harness
            .submit_cpu_request(request.id().agent(), request.clone())
            .map_err(execute_error)?;
        let responses = harness.cpu_responses();
        let cpu_response_count = responses.len().saturating_sub(responses_before);
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(&responses, responses_before, request.id());
        let outcome = match response_record.as_ref() {
            Some(record) => response_record_to_target_outcome(request, start_tick, record)?,
            None if request.requires_response() && !request.returns_data() => {
                TargetOutcome::Respond(
                    MemoryResponse::completed(request, None).map_err(execute_error)?,
                )
            }
            None if !request.requires_response() => TargetOutcome::NoResponse,
            None => return Err(execute_error("CLI MOESI data cache response missing")),
        };
        if let Some(line_data) = harness
            .cache_data(request.id().agent())
            .map_err(execute_error)?
        {
            self.sync_request_range(memory, request, &line_data)?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Moesi,
                data_cache_run_summary(
                    start_tick,
                    cpu_response_count,
                    directory_decision_count,
                    backing_dram_access_count,
                ),
            ));
        Ok(outcome)
    }

    fn respond_chi_inner(
        &self,
        memory: &CliMemoryRuntime,
        start_tick: u64,
        request: &MemoryRequest,
        harnesses: &mut CliChiLineHarnesses,
        backing_dram_access_count: usize,
    ) -> Result<TargetOutcome, Rem6CliError> {
        let line = self.layout.line_address(request.line_address());
        let harness = harnesses
            .lines
            .get_mut(&line)
            .ok_or_else(|| execute_error("CLI CHI data cache backing line missing"))?;
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        harness
            .submit_cpu_request(request.id().agent(), request.clone())
            .map_err(execute_error)?;
        let responses = harness.cpu_responses();
        let cpu_response_count = responses.len().saturating_sub(responses_before);
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(&responses, responses_before, request.id());
        let outcome = match response_record.as_ref() {
            Some(record) => response_record_to_target_outcome(request, start_tick, record)?,
            None if request.requires_response() && !request.returns_data() => {
                TargetOutcome::Respond(
                    MemoryResponse::completed(request, None).map_err(execute_error)?,
                )
            }
            None if !request.requires_response() => TargetOutcome::NoResponse,
            None => return Err(execute_error("CLI CHI data cache response missing")),
        };
        if let Some(line_data) = harness
            .cache_data(request.id().agent())
            .map_err(execute_error)?
        {
            self.sync_request_range(memory, request, &line_data)?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Chi,
                data_cache_run_summary(
                    start_tick,
                    cpu_response_count,
                    directory_decision_count,
                    backing_dram_access_count,
                ),
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
            .map_err(|_| execute_error("CLI data cache request offset is too large"))?;
        let bytes = usize::try_from(range.size().bytes())
            .map_err(|_| execute_error("CLI data cache request size is too large"))?;
        let end = offset
            .checked_add(bytes)
            .ok_or_else(|| execute_error("CLI data cache request range overflows"))?;
        let Some(data) = line_data.get(offset..end) else {
            return Err(execute_error(
                "CLI data cache request range exceeds cache line",
            ));
        };
        if memory.write_guest_memory(range.start().get(), data, self.layout) {
            Ok(())
        } else {
            Err(execute_error("failed to sync CLI data cache request"))
        }
    }

    fn record_error(&self, error: Rem6CliError) {
        let mut slot = self.error.lock().expect("CLI data cache error lock");
        if slot.is_none() {
            *slot = Some(error.to_string());
        }
    }
}

trait CliDataCacheResponseRecord: Clone {
    fn request(&self) -> MemoryRequestId;
    fn status(&self) -> ResponseStatus;
    fn data(&self) -> Option<&[u8]>;
    fn tick(&self) -> u64;
}

impl CliDataCacheResponseRecord for CpuResponseRecord {
    fn request(&self) -> MemoryRequestId {
        CpuResponseRecord::request(self)
    }

    fn status(&self) -> ResponseStatus {
        CpuResponseRecord::status(self)
    }

    fn data(&self) -> Option<&[u8]> {
        CpuResponseRecord::data(self)
    }

    fn tick(&self) -> u64 {
        CpuResponseRecord::tick(self)
    }
}

impl CliDataCacheResponseRecord for MesiCpuResponseRecord {
    fn request(&self) -> MemoryRequestId {
        MesiCpuResponseRecord::request(self)
    }

    fn status(&self) -> ResponseStatus {
        MesiCpuResponseRecord::status(self)
    }

    fn data(&self) -> Option<&[u8]> {
        MesiCpuResponseRecord::data(self)
    }

    fn tick(&self) -> u64 {
        MesiCpuResponseRecord::tick(self)
    }
}

impl CliDataCacheResponseRecord for MoesiCpuResponseRecord {
    fn request(&self) -> MemoryRequestId {
        MoesiCpuResponseRecord::request(self)
    }

    fn status(&self) -> ResponseStatus {
        MoesiCpuResponseRecord::status(self)
    }

    fn data(&self) -> Option<&[u8]> {
        MoesiCpuResponseRecord::data(self)
    }

    fn tick(&self) -> u64 {
        MoesiCpuResponseRecord::tick(self)
    }
}

impl CliDataCacheResponseRecord for ChiCpuResponseRecord {
    fn request(&self) -> MemoryRequestId {
        ChiCpuResponseRecord::request(self)
    }

    fn status(&self) -> ResponseStatus {
        ChiCpuResponseRecord::status(self)
    }

    fn data(&self) -> Option<&[u8]> {
        ChiCpuResponseRecord::data(self)
    }

    fn tick(&self) -> u64 {
        ChiCpuResponseRecord::tick(self)
    }
}

fn response_record<R>(
    responses: &[R],
    responses_before: usize,
    request: MemoryRequestId,
) -> Option<R>
where
    R: CliDataCacheResponseRecord,
{
    responses
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == request)
        .cloned()
}

fn response_record_to_target_outcome<R>(
    request: &MemoryRequest,
    start_tick: u64,
    record: &R,
) -> Result<TargetOutcome, Rem6CliError>
where
    R: CliDataCacheResponseRecord,
{
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
    dram_access_count: usize,
) -> ParallelCoherenceRunSummary {
    data_cache_run_summary(
        run.tick(),
        run.response_count(),
        directory_decision_count,
        dram_access_count,
    )
}

fn data_cache_run_summary(
    tick: u64,
    cpu_response_count: usize,
    directory_decision_count: usize,
    dram_access_count: usize,
) -> ParallelCoherenceRunSummary {
    ParallelCoherenceRunSummary::new(
        RecordedConservativeRunSummary::empty(tick),
        cpu_response_count,
        directory_decision_count,
        dram_access_count,
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
