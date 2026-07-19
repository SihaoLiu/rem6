use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_cache::{
    QueuedPrefetchConfig, QueuedPrefetchIssue, QueuedPrefetchRedundantLine,
    QueuedPrefetchSourceStatus, QueuedPrefetcher, TaggedPrefetchAccess, TaggedPrefetcher,
    TaggedPrefetcherConfig,
};
use rem6_coherence::{
    ChiCpuResponseRecord, ChiDirectoryLineHarness, CpuResponseRecord, LineBackingStore,
    MesiCpuResponseRecord, MesiDirectoryLineHarness, MoesiCpuResponseRecord,
    MoesiDirectoryLineHarness, MsiBankCycleRun, MsiBankDirectoryHarness,
    ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs, SubmitKind,
};
use rem6_kernel::{RecordedConservativeRunSummary, WaitForGraph};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryResponse, ResponseStatus, TranslationResolution,
};
use rem6_system::{
    RiscvDataCacheProtocol, RiscvDataCacheRunRecord, RiscvGuestMemoryMapRequest,
    RiscvGuestMemoryMapResult, RiscvSystemRun, RiscvSystemRunDriver,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::config::CliCachePrefetcher;
use crate::runtime_memory::{
    cli_cross_line_memory_response_with, cli_memory_response, cli_memory_response_for_request,
    CliMemoryRuntime,
};
use crate::{execute_error, Rem6CliError};

mod readiness;

use readiness::{delay_target_outcome_until, CliDataCacheBacking, CliDataCacheLineFill};

const PREFETCH_REQUEST_SEQUENCE_BASE: u64 = 1 << 63;
const LOWER_FILL_REQUEST_SEQUENCE_BASE: u64 = 1 << 62;
const CLI_PREFETCH_TRANSLATION_PAGE_BYTES: u64 = 4096;

#[derive(Clone)]
pub(super) struct CliDataCacheRuntime {
    layout: CacheLineLayout,
    harness: Arc<Mutex<CliDataCacheHarness>>,
    prefetch: Option<Arc<Mutex<CliDataCachePrefetchRuntime>>>,
    prefetch_fills: Arc<Mutex<u64>>,
    line_ready_ticks: Arc<Mutex<BTreeMap<Address, u64>>>,
    records: Arc<Mutex<Vec<RiscvDataCacheRunRecord>>>,
    error: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Default)]
pub(super) struct CliCacheHierarchy {
    levels: Vec<CliDataCacheRuntime>,
}

enum CliDataCacheHarness {
    Msi(MsiBankDirectoryHarness),
    Mesi(CliMesiLineHarnesses),
    Moesi(CliMoesiLineHarnesses),
    Chi(CliChiLineHarnesses),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CliDataCacheResponseMode {
    External,
    Internal,
}

struct CliDataCacheResponseContext<'a> {
    memory: &'a CliMemoryRuntime,
    lower_caches: &'a [CliDataCacheRuntime],
    start_tick: u64,
    request: &'a MemoryRequest,
    backing_dram_access_count: usize,
    response_mode: CliDataCacheResponseMode,
}

struct CliDataCacheResponse {
    outcome: TargetOutcome,
    scheduled_miss: bool,
}

impl CliDataCacheResponse {
    const fn new(outcome: TargetOutcome, scheduled_miss: bool) -> Self {
        Self {
            outcome,
            scheduled_miss,
        }
    }
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

struct CliDataCachePrefetchRuntime {
    tagged: TaggedPrefetcher,
    queue: QueuedPrefetcher,
    issued_lines: BTreeSet<Address>,
    pending_useful_lines: BTreeSet<Address>,
    next_sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CliDataCachePrefetchSummary {
    pub(crate) identified: u64,
    pub(crate) issued: u64,
    pub(crate) useful: u64,
    pub(crate) useful_but_miss: u64,
    pub(crate) unused: u64,
    pub(crate) demand_mshr_misses: u64,
    pub(crate) hit_in_cache: u64,
    pub(crate) hit_in_mshr: u64,
    pub(crate) hit_in_write_buffer: u64,
    pub(crate) late: u64,
    pub(crate) accuracy_ppm: Option<u64>,
    pub(crate) coverage_ppm: Option<u64>,
    pub(crate) span_page: u64,
    pub(crate) useful_span_page: u64,
    pub(crate) in_cache: u64,
    pub(crate) queue_enqueued: u64,
    pub(crate) queue_issued: u64,
    pub(crate) queue_dropped: u64,
    pub(crate) translation_queue_enqueued: u64,
    pub(crate) translation_queue_issued: u64,
    pub(crate) translation_queue_translated: u64,
    pub(crate) translation_queue_dropped: u64,
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
    pub(crate) bank_accepted: u64,
    pub(crate) bank_immediate_hits: u64,
    pub(crate) bank_scheduled_misses: u64,
    pub(crate) bank_coalesced_misses: u64,
    pub(crate) prefetch_identified: u64,
    pub(crate) prefetch_issued: u64,
    pub(crate) prefetch_useful: u64,
    pub(crate) prefetch_useful_but_miss: u64,
    pub(crate) prefetch_unused: u64,
    pub(crate) prefetch_demand_mshr_misses: u64,
    pub(crate) prefetch_hit_in_cache: u64,
    pub(crate) prefetch_hit_in_mshr: u64,
    pub(crate) prefetch_hit_in_write_buffer: u64,
    pub(crate) prefetch_late: u64,
    pub(crate) prefetch_accuracy_ppm: Option<u64>,
    pub(crate) prefetch_coverage_ppm: Option<u64>,
    pub(crate) prefetch_span_page: u64,
    pub(crate) prefetch_useful_span_page: u64,
    pub(crate) prefetch_in_cache: u64,
    pub(crate) prefetch_fills: u64,
    pub(crate) prefetch_queue_enqueued: u64,
    pub(crate) prefetch_queue_issued: u64,
    pub(crate) prefetch_queue_dropped: u64,
    pub(crate) prefetch_translation_queue_enqueued: u64,
    pub(crate) prefetch_translation_queue_issued: u64,
    pub(crate) prefetch_translation_queue_translated: u64,
    pub(crate) prefetch_translation_queue_dropped: u64,
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
            bank_accepted: records
                .iter()
                .map(|record| record.summary().bank_accepted_count() as u64)
                .sum(),
            bank_immediate_hits: records
                .iter()
                .map(|record| record.summary().bank_immediate_hit_count() as u64)
                .sum(),
            bank_scheduled_misses: records
                .iter()
                .map(|record| record.summary().bank_scheduled_miss_count() as u64)
                .sum(),
            bank_coalesced_misses: records
                .iter()
                .map(|record| record.summary().bank_coalesced_miss_count() as u64)
                .sum(),
            prefetch_identified: 0,
            prefetch_issued: 0,
            prefetch_useful: 0,
            prefetch_useful_but_miss: 0,
            prefetch_unused: 0,
            prefetch_demand_mshr_misses: 0,
            prefetch_hit_in_cache: 0,
            prefetch_hit_in_mshr: 0,
            prefetch_hit_in_write_buffer: 0,
            prefetch_late: 0,
            prefetch_accuracy_ppm: None,
            prefetch_coverage_ppm: None,
            prefetch_span_page: 0,
            prefetch_useful_span_page: 0,
            prefetch_in_cache: 0,
            prefetch_fills: 0,
            prefetch_queue_enqueued: 0,
            prefetch_queue_issued: 0,
            prefetch_queue_dropped: 0,
            prefetch_translation_queue_enqueued: 0,
            prefetch_translation_queue_issued: 0,
            prefetch_translation_queue_translated: 0,
            prefetch_translation_queue_dropped: 0,
        }
    }

    pub(crate) const fn with_prefetch_summary(
        mut self,
        summary: CliDataCachePrefetchSummary,
    ) -> Self {
        self.prefetch_identified = summary.identified;
        self.prefetch_issued = summary.issued;
        self.prefetch_useful = summary.useful;
        self.prefetch_useful_but_miss = summary.useful_but_miss;
        self.prefetch_unused = summary.unused;
        self.prefetch_demand_mshr_misses = summary.demand_mshr_misses;
        self.prefetch_hit_in_cache = summary.hit_in_cache;
        self.prefetch_hit_in_mshr = summary.hit_in_mshr;
        self.prefetch_hit_in_write_buffer = summary.hit_in_write_buffer;
        self.prefetch_late = summary.late;
        self.prefetch_accuracy_ppm = summary.accuracy_ppm;
        self.prefetch_coverage_ppm = summary.coverage_ppm;
        self.prefetch_span_page = summary.span_page;
        self.prefetch_useful_span_page = summary.useful_span_page;
        self.prefetch_in_cache = summary.in_cache;
        self.prefetch_queue_enqueued = summary.queue_enqueued;
        self.prefetch_queue_issued = summary.queue_issued;
        self.prefetch_queue_dropped = summary.queue_dropped;
        self.prefetch_translation_queue_enqueued = summary.translation_queue_enqueued;
        self.prefetch_translation_queue_issued = summary.translation_queue_issued;
        self.prefetch_translation_queue_translated = summary.translation_queue_translated;
        self.prefetch_translation_queue_dropped = summary.translation_queue_dropped;
        self
    }

    pub(crate) const fn with_prefetch_fills(mut self, prefetch_fills: u64) -> Self {
        self.prefetch_fills = prefetch_fills;
        self
    }
}

pub(super) fn cli_data_memory_response(
    cache_hierarchy: &CliCacheHierarchy,
    memory: &CliMemoryRuntime,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    if delivery.request().line_span() > 1 {
        return cli_cross_line_memory_response_with(delivery.request(), |request| {
            cache_hierarchy
                .respond_for_request(memory, delivery.tick(), request)
                .unwrap_or_else(|| {
                    cli_memory_response_for_request(memory, delivery.tick(), request)
                })
        })
        .unwrap_or_else(|| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())));
    }
    if let Some(outcome) = cache_hierarchy.respond(memory, delivery) {
        return outcome;
    }
    cli_memory_response(memory, delivery)
}

pub(super) fn cli_cache_runtime_with_prefetcher(
    protocol: Option<RiscvDataCacheProtocol>,
    line_layout: CacheLineLayout,
    core_count: u32,
    prefetcher: Option<CliCachePrefetcher>,
) -> Result<Option<CliDataCacheRuntime>, Rem6CliError> {
    let agents = || (0..core_count).map(AgentId::new);
    match protocol {
        Some(RiscvDataCacheProtocol::Msi) => {
            CliDataCacheRuntime::new_msi_bank(line_layout, agents(), prefetcher).map(Some)
        }
        Some(RiscvDataCacheProtocol::Mesi) => {
            CliDataCacheRuntime::new_mesi_lines(line_layout, agents(), prefetcher).map(Some)
        }
        Some(RiscvDataCacheProtocol::Moesi) => {
            CliDataCacheRuntime::new_moesi_lines(line_layout, agents(), prefetcher).map(Some)
        }
        Some(RiscvDataCacheProtocol::Chi) => {
            CliDataCacheRuntime::new_chi_lines(line_layout, agents(), prefetcher).map(Some)
        }
        None => Ok(None),
    }
}

pub(super) fn with_riscv_syscall_data_cache_memory_io(
    driver: RiscvSystemRunDriver,
    memory: CliMemoryRuntime,
    instruction_cache: CliCacheHierarchy,
    data_cache: CliCacheHierarchy,
    line_layout: CacheLineLayout,
) -> RiscvSystemRunDriver {
    let read_memory = memory.clone();
    let write_memory = memory.clone();
    let probe_memory = memory.clone();
    let map_memory = memory.clone();
    let write_data_cache = data_cache.clone();
    let write_instruction_cache = instruction_cache.clone();
    driver.with_riscv_syscall_emulation_and_guest_memory_io_probe_map_handler(
        move |address, bytes| read_memory.read_guest_memory(address, bytes, line_layout),
        move |address, bytes| {
            write_guest_memory_with_cache_invalidation(
                &write_memory,
                &write_instruction_cache,
                &write_data_cache,
                address,
                bytes,
                line_layout,
            )
        },
        move |address, bytes| probe_memory.can_write_guest_memory(address, bytes, line_layout),
        move |request| {
            map_guest_memory_with_data_cache_invalidation(
                &map_memory,
                &instruction_cache,
                &data_cache,
                request,
                line_layout,
            )
        },
    )
}

pub(super) fn write_guest_memory_with_cache_invalidation(
    memory: &CliMemoryRuntime,
    instruction_cache: &CliCacheHierarchy,
    data_cache: &CliCacheHierarchy,
    address: u64,
    bytes: &[u8],
    line_layout: CacheLineLayout,
) -> bool {
    if !memory.write_guest_memory(address, bytes, line_layout) {
        return false;
    }
    if bytes.is_empty() {
        return true;
    }
    invalidate_cli_cache_hierarchies(instruction_cache, data_cache)
}

fn map_guest_memory_with_data_cache_invalidation(
    memory: &CliMemoryRuntime,
    instruction_cache: &CliCacheHierarchy,
    data_cache: &CliCacheHierarchy,
    request: RiscvGuestMemoryMapRequest,
    line_layout: CacheLineLayout,
) -> RiscvGuestMemoryMapResult {
    let result = memory.map_guest_memory(
        request.address(),
        request.bytes(),
        line_layout,
        request.replace_existing(),
    );
    if invalidate_cli_cache_hierarchies(instruction_cache, data_cache) {
        result
    } else {
        RiscvGuestMemoryMapResult::Failed
    }
}

pub(super) fn invalidate_cli_cache_hierarchies(
    instruction_cache: &CliCacheHierarchy,
    data_cache: &CliCacheHierarchy,
) -> bool {
    instruction_cache.invalidate_cached_guest_memory()
        && data_cache.invalidate_cached_guest_memory()
}

impl CliCacheHierarchy {
    pub(super) fn from_levels<I>(levels: I) -> Self
    where
        I: IntoIterator<Item = Option<CliDataCacheRuntime>>,
    {
        Self {
            levels: levels.into_iter().flatten().collect(),
        }
    }

    fn respond(
        &self,
        memory: &CliMemoryRuntime,
        delivery: &RequestDelivery,
    ) -> Option<TargetOutcome> {
        let (top, lower) = self.levels.split_first()?;
        top.respond_with_lower(memory, lower, delivery)
    }

    fn respond_for_request(
        &self,
        memory: &CliMemoryRuntime,
        tick: u64,
        request: &MemoryRequest,
    ) -> Option<TargetOutcome> {
        let (top, lower) = self.levels.split_first()?;
        top.respond_for_request(memory, lower, tick, request)
    }

    pub(super) fn records(&self, level: usize) -> Vec<RiscvDataCacheRunRecord> {
        self.levels
            .get(level)
            .map(CliDataCacheRuntime::records)
            .unwrap_or_default()
    }

    pub(super) fn prefetch_fills(&self, level: usize) -> u64 {
        self.levels
            .get(level)
            .map(CliDataCacheRuntime::prefetch_fills)
            .unwrap_or_default()
    }

    pub(super) fn top_prefetch_summary(&self) -> CliDataCachePrefetchSummary {
        self.levels
            .first()
            .map(CliDataCacheRuntime::prefetch_summary)
            .unwrap_or_default()
    }

    pub(super) fn take_error(&self) -> Option<Rem6CliError> {
        self.levels.iter().find_map(CliDataCacheRuntime::take_error)
    }

    fn invalidate_cached_guest_memory(&self) -> bool {
        self.levels
            .iter()
            .all(CliDataCacheRuntime::invalidate_cached_guest_memory)
    }
}

impl CliDataCacheRuntime {
    pub(super) fn new_msi_bank<I>(
        layout: CacheLineLayout,
        agents: I,
        prefetcher: Option<CliCachePrefetcher>,
    ) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        Ok(Self {
            layout,
            harness: Arc::new(Mutex::new(CliDataCacheHarness::Msi(
                MsiBankDirectoryHarness::new(layout, agents).map_err(execute_error)?,
            ))),
            prefetch: cli_prefetch_runtime(layout, prefetcher)?,
            prefetch_fills: Arc::new(Mutex::new(0)),
            line_ready_ticks: Arc::new(Mutex::new(BTreeMap::new())),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn new_mesi_lines<I>(
        layout: CacheLineLayout,
        agents: I,
        prefetcher: Option<CliCachePrefetcher>,
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
            prefetch: cli_prefetch_runtime(layout, prefetcher)?,
            prefetch_fills: Arc::new(Mutex::new(0)),
            line_ready_ticks: Arc::new(Mutex::new(BTreeMap::new())),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn new_moesi_lines<I>(
        layout: CacheLineLayout,
        agents: I,
        prefetcher: Option<CliCachePrefetcher>,
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
            prefetch: cli_prefetch_runtime(layout, prefetcher)?,
            prefetch_fills: Arc::new(Mutex::new(0)),
            line_ready_ticks: Arc::new(Mutex::new(BTreeMap::new())),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn new_chi_lines<I>(
        layout: CacheLineLayout,
        agents: I,
        prefetcher: Option<CliCachePrefetcher>,
    ) -> Result<Self, Rem6CliError>
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
            prefetch: cli_prefetch_runtime(layout, prefetcher)?,
            prefetch_fills: Arc::new(Mutex::new(0)),
            line_ready_ticks: Arc::new(Mutex::new(BTreeMap::new())),
            records: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(None)),
        })
    }

    pub(super) fn respond_with_lower(
        &self,
        memory: &CliMemoryRuntime,
        lower_caches: &[CliDataCacheRuntime],
        delivery: &RequestDelivery,
    ) -> Option<TargetOutcome> {
        self.respond_for_request(memory, lower_caches, delivery.tick(), delivery.request())
    }

    fn respond_for_request(
        &self,
        memory: &CliMemoryRuntime,
        lower_caches: &[CliDataCacheRuntime],
        tick: u64,
        request: &MemoryRequest,
    ) -> Option<TargetOutcome> {
        if request.line_layout() != self.layout {
            return None;
        }
        let line = self.layout.line_address(request.line_address());
        let line_was_present = self.contains_line(line);
        let line_was_ready = line_was_present && self.line_is_ready_at(line, tick);
        let should_record_demand_mshr_miss =
            self.prefetch.is_some() && is_cache_prefetch_source(request) && !line_was_ready;
        let backing = self.ensure_backing(memory, lower_caches, tick, request)?;
        if should_record_demand_mshr_miss {
            if let Some(prefetch) = self.prefetch.as_ref() {
                prefetch
                    .lock()
                    .expect("CLI data cache prefetch lock")
                    .record_demand_mshr_miss();
            }
        }

        let response = self
            .respond_inner(CliDataCacheResponseContext {
                memory,
                lower_caches,
                start_tick: tick,
                request,
                backing_dram_access_count: backing.dram_access_count,
                response_mode: CliDataCacheResponseMode::External,
            })
            .unwrap_or_else(|error| {
                self.record_error(error);
                CliDataCacheResponse::new(
                    TargetOutcome::Respond(MemoryResponse::retry(request)),
                    false,
                )
            });
        let missed_usable_state = line_was_present && (!line_was_ready || response.scheduled_miss);
        let source_was_prefetched = match self.record_prefetch_use_from_response(
            request,
            &response.outcome,
            missed_usable_state,
        ) {
            Ok(source_was_prefetched) => source_was_prefetched,
            Err(error) => {
                self.record_error(error);
                false
            }
        };
        if let Err(error) = self.issue_prefetches_from_request(
            memory,
            lower_caches,
            tick,
            request,
            source_was_prefetched,
        ) {
            self.record_error(error);
        }
        Some(delay_target_outcome_until(
            response.outcome,
            tick,
            backing.ready_tick,
        ))
    }

    pub(super) fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.records
            .lock()
            .expect("CLI data cache record lock")
            .clone()
    }

    pub(super) fn prefetch_summary(&self) -> CliDataCachePrefetchSummary {
        self.prefetch
            .as_ref()
            .map(|prefetch| {
                prefetch
                    .lock()
                    .expect("CLI data cache prefetch lock")
                    .summary()
            })
            .unwrap_or_default()
    }

    pub(super) fn prefetch_fills(&self) -> u64 {
        *self
            .prefetch_fills
            .lock()
            .expect("CLI data cache prefetch fill lock")
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
        drop(harness);
        self.line_ready_ticks
            .lock()
            .expect("CLI data cache ready-tick lock")
            .clear();
        if let Some(prefetch) = self.prefetch.as_ref() {
            prefetch
                .lock()
                .expect("CLI data cache prefetch lock")
                .clear_issued_lines();
        }
        Ok(())
    }

    fn ensure_backing(
        &self,
        memory: &CliMemoryRuntime,
        lower_caches: &[CliDataCacheRuntime],
        tick: u64,
        request: &MemoryRequest,
    ) -> Option<CliDataCacheBacking> {
        let line = self.layout.line_address(request.line_address());
        if self.contains_line(line) {
            return Some(CliDataCacheBacking {
                dram_access_count: 0,
                ready_tick: self
                    .line_ready_tick(line)
                    .map_or(tick, |ready_tick| ready_tick.max(tick)),
            });
        }

        let (data, dram_access_count, ready_tick) = match lower_caches.split_first() {
            Some((lower_cache, remaining_lower_caches)) => {
                let fill = lower_cache.read_line_for_upper_fill(
                    memory,
                    remaining_lower_caches,
                    tick,
                    request,
                )?;
                (fill.data, 0, fill.ready_tick)
            }
            None => {
                let fill = memory.read_guest_cache_line_for_fill(line, self.layout, tick)?;
                (fill.data, usize::from(memory.uses_dram()), fill.ready_tick)
            }
        };

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
        drop(harness);
        if !inserted {
            return None;
        }
        self.line_ready_ticks
            .lock()
            .expect("CLI data cache ready-tick lock")
            .insert(line, ready_tick);
        Some(CliDataCacheBacking {
            dram_access_count,
            ready_tick,
        })
    }

    fn read_line_for_upper_fill(
        &self,
        memory: &CliMemoryRuntime,
        lower_caches: &[CliDataCacheRuntime],
        tick: u64,
        upper_request: &MemoryRequest,
    ) -> Option<CliDataCacheLineFill> {
        let line = self.layout.line_address(upper_request.line_address());
        let is_prefetch_fill = upper_request.operation() == MemoryOperation::PrefetchRead;
        let request = match lower_fill_request(upper_request, line, self.layout) {
            Ok(request) => request,
            Err(error) => {
                self.record_error(error);
                return None;
            }
        };
        if request.line_layout() != self.layout {
            return None;
        }
        let backing = self.ensure_backing(memory, lower_caches, tick, &request)?;
        if let Err(error) = self.respond_inner(CliDataCacheResponseContext {
            memory,
            lower_caches,
            start_tick: tick,
            request: &request,
            backing_dram_access_count: backing.dram_access_count,
            response_mode: CliDataCacheResponseMode::Internal,
        }) {
            self.record_error(error);
            return None;
        }
        if is_prefetch_fill {
            self.record_prefetch_fill();
        }
        self.cached_line_data(line, request.id().agent())
            .map(|data| CliDataCacheLineFill {
                data,
                ready_tick: backing.ready_tick,
            })
    }

    fn cached_line_data(&self, line: Address, agent: AgentId) -> Option<Vec<u8>> {
        let harness = self.harness.lock().expect("CLI data cache lock");
        match &*harness {
            CliDataCacheHarness::Msi(harness) => harness
                .functional_read_line(line)
                .ok()?
                .data()
                .map(<[u8]>::to_vec),
            CliDataCacheHarness::Mesi(harnesses) => {
                harnesses.lines.get(&line)?.cache_data(agent).ok().flatten()
            }
            CliDataCacheHarness::Moesi(harnesses) => {
                harnesses.lines.get(&line)?.cache_data(agent).ok().flatten()
            }
            CliDataCacheHarness::Chi(harnesses) => {
                harnesses.lines.get(&line)?.cache_data(agent).ok().flatten()
            }
        }
    }

    fn contains_line(&self, line: Address) -> bool {
        let harness = self.harness.lock().expect("CLI data cache lock");
        match &*harness {
            CliDataCacheHarness::Msi(harness) => {
                harness.backing_line(line).is_some()
                    || harness.directory_line_addresses().contains(&line)
                    || harness.cache_agents().into_iter().any(|agent| {
                        harness
                            .cache_line_addresses(agent)
                            .map(|lines| lines.contains(&line))
                            .unwrap_or(false)
                    })
            }
            CliDataCacheHarness::Mesi(harnesses) => harnesses.lines.contains_key(&line),
            CliDataCacheHarness::Moesi(harnesses) => harnesses.lines.contains_key(&line),
            CliDataCacheHarness::Chi(harnesses) => harnesses.lines.contains_key(&line),
        }
    }

    fn line_ready_tick(&self, line: Address) -> Option<u64> {
        self.line_ready_ticks
            .lock()
            .expect("CLI data cache ready-tick lock")
            .get(&line)
            .copied()
    }

    fn line_is_ready_at(&self, line: Address, tick: u64) -> bool {
        self.line_ready_tick(line)
            .is_none_or(|ready_tick| ready_tick <= tick)
    }

    fn is_prefetch_candidate_backed(&self, memory: &CliMemoryRuntime, address: Address) -> bool {
        let line = self.layout.line_address(address);
        memory.read_guest_cache_line(line, self.layout).is_some()
    }

    fn is_prefetch_candidate_resident(&self, address: Address, tick: u64) -> bool {
        let line = self.layout.line_address(address);
        self.contains_line(line) && self.line_is_ready_at(line, tick)
    }

    fn respond_inner(
        &self,
        context: CliDataCacheResponseContext<'_>,
    ) -> Result<CliDataCacheResponse, Rem6CliError> {
        let mut harness = self.harness.lock().expect("CLI data cache lock");
        match &mut *harness {
            CliDataCacheHarness::Msi(harness) => self.respond_msi_inner(&context, harness),
            CliDataCacheHarness::Mesi(harnesses) => self.respond_mesi_inner(&context, harnesses),
            CliDataCacheHarness::Moesi(harnesses) => self.respond_moesi_inner(&context, harnesses),
            CliDataCacheHarness::Chi(harnesses) => self.respond_chi_inner(&context, harnesses),
        }
    }

    fn respond_msi_inner(
        &self,
        context: &CliDataCacheResponseContext<'_>,
        harness: &mut MsiBankDirectoryHarness,
    ) -> Result<CliDataCacheResponse, Rem6CliError> {
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        let cycle_run = harness
            .submit_parallel_cycle(
                context.start_tick,
                [(context.request.id().agent(), context.request.clone())],
            )
            .map_err(execute_error)?;
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(
            &harness.cpu_responses(),
            responses_before,
            context.request.id(),
        );
        let response_status = response_record.as_ref().map(|record| record.status());
        let outcome = data_cache_target_outcome(
            context.response_mode,
            context.request,
            context.start_tick,
            response_record.as_ref(),
            "CLI MSI data cache response missing",
        )?;
        let line_data = harness
            .functional_read_line(self.layout.line_address(context.request.line_address()))
            .map_err(execute_error)?
            .data()
            .map(<[u8]>::to_vec);

        if let Some(line_data) = line_data {
            self.sync_request_range(
                context.memory,
                context.lower_caches,
                context.request,
                response_status,
                &line_data,
            )?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Msi,
                msi_bank_cycle_run_summary(
                    &cycle_run,
                    directory_decision_count,
                    response_count_for_request(
                        context.response_mode,
                        context.request,
                        cycle_run.response_count(),
                    ),
                    context.backing_dram_access_count,
                ),
            ));
        Ok(CliDataCacheResponse::new(
            outcome,
            cycle_run.scheduled_miss_count() != 0,
        ))
    }

    fn respond_mesi_inner(
        &self,
        context: &CliDataCacheResponseContext<'_>,
        harnesses: &mut CliMesiLineHarnesses,
    ) -> Result<CliDataCacheResponse, Rem6CliError> {
        let line = self.layout.line_address(context.request.line_address());
        let harness = harnesses
            .lines
            .get_mut(&line)
            .ok_or_else(|| execute_error("CLI MESI data cache backing line missing"))?;
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        let submit_result = harness
            .submit_cpu_request(context.request.id().agent(), context.request.clone())
            .map_err(execute_error)?;
        let responses = harness.cpu_responses();
        let cpu_response_count = responses.len().saturating_sub(responses_before);
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(&responses, responses_before, context.request.id());
        let response_status = response_record.as_ref().map(|record| record.status());
        let outcome = data_cache_target_outcome(
            context.response_mode,
            context.request,
            context.start_tick,
            response_record.as_ref(),
            "CLI MESI data cache response missing",
        )?;
        if let Some(line_data) = harness
            .cache_data(context.request.id().agent())
            .map_err(execute_error)?
        {
            self.sync_request_range(
                context.memory,
                context.lower_caches,
                context.request,
                response_status,
                &line_data,
            )?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Mesi,
                data_cache_run_summary(
                    context.start_tick,
                    response_count_for_request(
                        context.response_mode,
                        context.request,
                        cpu_response_count,
                    ),
                    directory_decision_count,
                    context.backing_dram_access_count,
                ),
            ));
        Ok(CliDataCacheResponse::new(
            outcome,
            submit_result.kind() == SubmitKind::ScheduledMiss,
        ))
    }

    fn respond_moesi_inner(
        &self,
        context: &CliDataCacheResponseContext<'_>,
        harnesses: &mut CliMoesiLineHarnesses,
    ) -> Result<CliDataCacheResponse, Rem6CliError> {
        let line = self.layout.line_address(context.request.line_address());
        let harness = harnesses
            .lines
            .get_mut(&line)
            .ok_or_else(|| execute_error("CLI MOESI data cache backing line missing"))?;
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        let submit_result = harness
            .submit_cpu_request(context.request.id().agent(), context.request.clone())
            .map_err(execute_error)?;
        let responses = harness.cpu_responses();
        let cpu_response_count = responses.len().saturating_sub(responses_before);
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(&responses, responses_before, context.request.id());
        let response_status = response_record.as_ref().map(|record| record.status());
        let outcome = data_cache_target_outcome(
            context.response_mode,
            context.request,
            context.start_tick,
            response_record.as_ref(),
            "CLI MOESI data cache response missing",
        )?;
        if let Some(line_data) = harness
            .cache_data(context.request.id().agent())
            .map_err(execute_error)?
        {
            self.sync_request_range(
                context.memory,
                context.lower_caches,
                context.request,
                response_status,
                &line_data,
            )?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Moesi,
                data_cache_run_summary(
                    context.start_tick,
                    response_count_for_request(
                        context.response_mode,
                        context.request,
                        cpu_response_count,
                    ),
                    directory_decision_count,
                    context.backing_dram_access_count,
                ),
            ));
        Ok(CliDataCacheResponse::new(
            outcome,
            submit_result.kind() == SubmitKind::ScheduledMiss,
        ))
    }

    fn respond_chi_inner(
        &self,
        context: &CliDataCacheResponseContext<'_>,
        harnesses: &mut CliChiLineHarnesses,
    ) -> Result<CliDataCacheResponse, Rem6CliError> {
        let line = self.layout.line_address(context.request.line_address());
        let harness = harnesses
            .lines
            .get_mut(&line)
            .ok_or_else(|| execute_error("CLI CHI data cache backing line missing"))?;
        let responses_before = harness.cpu_responses().len();
        let decisions_before = harness.directory_decisions().len();
        let submit_result = harness
            .submit_cpu_request(context.request.id().agent(), context.request.clone())
            .map_err(execute_error)?;
        let responses = harness.cpu_responses();
        let cpu_response_count = responses.len().saturating_sub(responses_before);
        let directory_decision_count = harness
            .directory_decisions()
            .len()
            .saturating_sub(decisions_before);
        let response_record = response_record(&responses, responses_before, context.request.id());
        let response_status = response_record.as_ref().map(|record| record.status());
        let outcome = data_cache_target_outcome(
            context.response_mode,
            context.request,
            context.start_tick,
            response_record.as_ref(),
            "CLI CHI data cache response missing",
        )?;
        if let Some(line_data) = harness
            .cache_data(context.request.id().agent())
            .map_err(execute_error)?
        {
            self.sync_request_range(
                context.memory,
                context.lower_caches,
                context.request,
                response_status,
                &line_data,
            )?;
        }

        self.records
            .lock()
            .expect("CLI data cache record lock")
            .push(RiscvDataCacheRunRecord::new(
                RiscvDataCacheProtocol::Chi,
                data_cache_run_summary(
                    context.start_tick,
                    response_count_for_request(
                        context.response_mode,
                        context.request,
                        cpu_response_count,
                    ),
                    directory_decision_count,
                    context.backing_dram_access_count,
                ),
            ));
        Ok(CliDataCacheResponse::new(
            outcome,
            submit_result.kind() == SubmitKind::ScheduledMiss,
        ))
    }

    fn sync_request_range(
        &self,
        memory: &CliMemoryRuntime,
        lower_caches: &[CliDataCacheRuntime],
        request: &MemoryRequest,
        response_status: Option<ResponseStatus>,
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
        if !data_cache_request_updates_backing(request.operation(), response_status) {
            return Ok(());
        }
        if !memory.write_guest_memory(range.start().get(), data, self.layout) {
            return Err(execute_error("failed to sync CLI data cache request"));
        }
        if !lower_caches
            .iter()
            .all(CliDataCacheRuntime::invalidate_cached_guest_memory)
        {
            return Err(execute_error(
                "failed to invalidate CLI lower data cache after write",
            ));
        }
        Ok(())
    }

    fn issue_prefetches_from_request(
        &self,
        memory: &CliMemoryRuntime,
        lower_caches: &[CliDataCacheRuntime],
        tick: u64,
        request: &MemoryRequest,
        source_was_prefetched: bool,
    ) -> Result<(), Rem6CliError> {
        if !is_cache_prefetch_source(request) {
            return Ok(());
        }
        let Some(prefetch) = self.prefetch.as_ref() else {
            return Ok(());
        };
        let issues = {
            let mut prefetch = prefetch.lock().expect("CLI data cache prefetch lock");
            prefetch.observe_and_issue(
                tick,
                request,
                self.layout,
                source_was_prefetched,
                |address| self.is_prefetch_candidate_backed(memory, address),
                |address| self.is_prefetch_candidate_resident(address, tick),
            )?
        };
        for (issue, sequence) in issues {
            let issue_address = issue.address();
            let prefetch_request = prefetch_issue_request(issue, sequence, self.layout)?;
            if let Some(backing) =
                self.ensure_backing(memory, lower_caches, tick, &prefetch_request)
            {
                let _ = self.respond_inner(CliDataCacheResponseContext {
                    memory,
                    lower_caches,
                    start_tick: tick,
                    request: &prefetch_request,
                    backing_dram_access_count: backing.dram_access_count,
                    response_mode: CliDataCacheResponseMode::Internal,
                })?;
                self.record_prefetch_fill();
                if let Some(prefetch) = self.prefetch.as_ref() {
                    prefetch
                        .lock()
                        .expect("CLI data cache prefetch lock")
                        .mark_issued(issue_address, self.layout);
                }
            }
        }
        Ok(())
    }

    fn record_prefetch_use_from_response(
        &self,
        request: &MemoryRequest,
        outcome: &TargetOutcome,
        missed_usable_state: bool,
    ) -> Result<bool, Rem6CliError> {
        if target_outcome_response_status(outcome) != Some(ResponseStatus::Completed)
            || !is_cache_prefetch_source(request)
        {
            return Ok(false);
        }
        let Some(prefetch) = self.prefetch.as_ref() else {
            return Ok(false);
        };
        prefetch
            .lock()
            .expect("CLI data cache prefetch lock")
            .record_useful_if_issued(request.range().start(), self.layout, missed_usable_state)
    }

    fn record_error(&self, error: Rem6CliError) {
        let mut slot = self.error.lock().expect("CLI data cache error lock");
        if slot.is_none() {
            *slot = Some(error.to_string());
        }
    }

    fn record_prefetch_fill(&self) {
        let mut fills = self
            .prefetch_fills
            .lock()
            .expect("CLI data cache prefetch fill lock");
        *fills = fills.saturating_add(1);
    }
}

impl CliDataCachePrefetchRuntime {
    fn new(kind: CliCachePrefetcher, layout: CacheLineLayout) -> Result<Self, Rem6CliError> {
        match kind {
            CliCachePrefetcher::TaggedNextLine => {
                let tagged = TaggedPrefetcher::new(
                    TaggedPrefetcherConfig::new(layout.bytes(), 1).map_err(execute_error)?,
                );
                let queue_config =
                    QueuedPrefetchConfig::with_line_size(8, 0, 1, true, layout.bytes())
                        .and_then(|config| {
                            config.with_page_size(CLI_PREFETCH_TRANSLATION_PAGE_BYTES)
                        })
                        .and_then(|config| config.with_missing_translation_capacity(8))
                        .map_err(execute_error)?;
                Ok(Self {
                    tagged,
                    queue: QueuedPrefetcher::new(queue_config),
                    issued_lines: BTreeSet::new(),
                    pending_useful_lines: BTreeSet::new(),
                    next_sequence: PREFETCH_REQUEST_SEQUENCE_BASE,
                })
            }
        }
    }

    fn observe_and_issue(
        &mut self,
        tick: u64,
        request: &MemoryRequest,
        layout: CacheLineLayout,
        source_was_prefetched: bool,
        candidate_is_backed: impl Fn(Address) -> bool,
        candidate_is_resident: impl Fn(Address) -> bool,
    ) -> Result<Vec<(QueuedPrefetchIssue, u64)>, Rem6CliError> {
        let access = TaggedPrefetchAccess::new(
            request.id().agent(),
            request.range().start().get(),
            request.range().start(),
            request.attributes().is_secure(),
        );
        let candidates = self.tagged.observe(access).map_err(execute_error)?.to_vec();
        let mut backed_candidates = Vec::new();
        let mut redundant_lines = Vec::new();
        for candidate in &candidates {
            let address = candidate.address();
            if !candidate_is_backed(address) {
                continue;
            }
            let line = layout.line_address(address);
            if candidate_is_resident(address) {
                redundant_lines.push(QueuedPrefetchRedundantLine::in_cache(
                    line,
                    candidate.secure(),
                ));
            } else if self.issued_lines.contains(&line) {
                redundant_lines.push(QueuedPrefetchRedundantLine::in_miss_queue(
                    line,
                    candidate.secure(),
                ));
            }
            backed_candidates.push(candidate.clone());
        }
        let source_status = if source_was_prefetched {
            QueuedPrefetchSourceStatus::prefetched()
        } else {
            QueuedPrefetchSourceStatus::demand()
        };
        self.queue
            .enqueue_candidates_filtered_with_source(
                tick,
                &backed_candidates,
                &redundant_lines,
                source_status,
            )
            .map_err(execute_error)?;
        self.process_identity_translations(tick, &redundant_lines)?;
        let issues = self.queue.issue_ready(tick);
        let mut sequenced_issues = Vec::with_capacity(issues.len());
        for issue in issues {
            let sequence = self.next_sequence;
            self.next_sequence = sequence
                .checked_add(1)
                .ok_or_else(|| execute_error("CLI data cache prefetch request id overflow"))?;
            sequenced_issues.push((issue, sequence));
        }
        Ok(sequenced_issues)
    }

    fn process_identity_translations(
        &mut self,
        tick: u64,
        redundant_lines: &[QueuedPrefetchRedundantLine],
    ) -> Result<(), Rem6CliError> {
        let translations = self
            .queue
            .process_missing_translations(usize::MAX)
            .map_err(execute_error)?;
        for translation in translations {
            self.queue
                .complete_translation(
                    tick,
                    translation.request().id(),
                    TranslationResolution::mapped(translation.request().virtual_address()),
                    redundant_lines,
                )
                .map_err(execute_error)?;
        }
        Ok(())
    }

    fn mark_issued(&mut self, address: Address, layout: CacheLineLayout) {
        let line = layout.line_address(address);
        self.issued_lines.insert(line);
        self.pending_useful_lines.insert(line);
    }

    fn record_useful_if_issued(
        &mut self,
        address: Address,
        layout: CacheLineLayout,
        missed_usable_state: bool,
    ) -> Result<bool, Rem6CliError> {
        let line = layout.line_address(address);
        if self.pending_useful_lines.remove(&line) {
            self.queue
                .record_useful_prefetch(missed_usable_state)
                .map_err(execute_error)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn record_demand_mshr_miss(&mut self) {
        self.queue.record_demand_mshr_miss();
    }

    fn clear_issued_lines(&mut self) {
        self.issued_lines.clear();
        self.pending_useful_lines.clear();
    }

    fn summary(&self) -> CliDataCachePrefetchSummary {
        let stats = self.queue.stats();
        CliDataCachePrefetchSummary {
            identified: stats.identified_prefetches(),
            issued: stats.issued_prefetches(),
            useful: stats.useful_prefetches(),
            useful_but_miss: stats.useful_but_miss_prefetches(),
            unused: stats.unused_prefetches(),
            demand_mshr_misses: stats.demand_mshr_misses(),
            hit_in_cache: stats.prefetch_hits_in_cache(),
            hit_in_mshr: stats.prefetch_hits_in_mshr(),
            hit_in_write_buffer: stats.prefetch_hits_in_write_buffer(),
            late: stats.late_prefetches(),
            accuracy_ppm: stats.accuracy_ppm().map(u64::from),
            coverage_ppm: stats.coverage_ppm().map(u64::from),
            span_page: stats.span_page_prefetches(),
            useful_span_page: stats.useful_span_page_prefetches(),
            in_cache: stats.in_cache_drops(),
            queue_enqueued: stats.prefetch_queue().enqueued(),
            queue_issued: stats.prefetch_queue().issued(),
            queue_dropped: stats.prefetch_queue().dropped(),
            translation_queue_enqueued: stats.translation_queue().enqueued(),
            translation_queue_issued: stats.translation_queue().issued(),
            translation_queue_translated: stats.translation_queue().translated(),
            translation_queue_dropped: stats.translation_queue().dropped(),
        }
    }
}

fn cli_prefetch_runtime(
    layout: CacheLineLayout,
    prefetcher: Option<CliCachePrefetcher>,
) -> Result<Option<Arc<Mutex<CliDataCachePrefetchRuntime>>>, Rem6CliError> {
    prefetcher
        .map(|prefetcher| {
            CliDataCachePrefetchRuntime::new(prefetcher, layout)
                .map(|runtime| Arc::new(Mutex::new(runtime)))
        })
        .transpose()
}

fn is_cache_prefetch_source(request: &MemoryRequest) -> bool {
    matches!(
        request.operation(),
        MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::ReadUnique
            | MemoryOperation::LoadLocked
            | MemoryOperation::LockedRmwRead
    )
}

fn target_outcome_response_status(outcome: &TargetOutcome) -> Option<ResponseStatus> {
    match outcome {
        TargetOutcome::Respond(response) => Some(response.status()),
        TargetOutcome::RespondAfter { response, .. } => Some(response.status()),
        TargetOutcome::NoResponse => None,
    }
}

fn data_cache_request_updates_backing(
    operation: MemoryOperation,
    response_status: Option<ResponseStatus>,
) -> bool {
    match operation {
        MemoryOperation::StoreConditional => response_status == Some(ResponseStatus::Completed),
        MemoryOperation::Write
        | MemoryOperation::CacheBlockZero
        | MemoryOperation::LockedRmwWrite
        | MemoryOperation::Atomic
        | MemoryOperation::AtomicNoReturn
        | MemoryOperation::WriteClean
        | MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty => true,
        _ => false,
    }
}

fn prefetch_issue_request(
    issue: QueuedPrefetchIssue,
    sequence: u64,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, Rem6CliError> {
    let request = MemoryRequest::prefetch_read(
        MemoryRequestId::new(issue.context(), sequence),
        issue.address(),
        rem6_memory::AccessSize::new(layout.bytes()).map_err(execute_error)?,
        layout,
    )
    .map_err(execute_error)?;
    let snapshot = request.snapshot().with_response_required();
    MemoryRequest::from_snapshot(&snapshot).map_err(execute_error)
}

fn lower_fill_request(
    upper_request: &MemoryRequest,
    line: Address,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, Rem6CliError> {
    let sequence = LOWER_FILL_REQUEST_SEQUENCE_BASE
        | (upper_request.id().sequence() & (LOWER_FILL_REQUEST_SEQUENCE_BASE - 1));
    let id = MemoryRequestId::new(upper_request.id().agent(), sequence);
    let size = rem6_memory::AccessSize::new(layout.bytes()).map_err(execute_error)?;
    let request = if upper_request.operation() == MemoryOperation::PrefetchRead {
        MemoryRequest::prefetch_read(id, line, size, layout).map_err(execute_error)?
    } else {
        MemoryRequest::read_shared(id, line, size, layout).map_err(execute_error)?
    };
    let snapshot = request.snapshot().with_response_required();
    MemoryRequest::from_snapshot(&snapshot).map_err(execute_error)
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

fn data_cache_target_outcome<R>(
    response_mode: CliDataCacheResponseMode,
    request: &MemoryRequest,
    start_tick: u64,
    record: Option<&R>,
    missing_response_error: &'static str,
) -> Result<TargetOutcome, Rem6CliError>
where
    R: CliDataCacheResponseRecord,
{
    if response_mode == CliDataCacheResponseMode::Internal || !request.requires_response() {
        return Ok(TargetOutcome::NoResponse);
    }
    match record {
        Some(record) => response_record_to_target_outcome(request, start_tick, record),
        None if !request.returns_data() => Ok(TargetOutcome::Respond(
            MemoryResponse::completed(request, None).map_err(execute_error)?,
        )),
        None => Err(execute_error(missing_response_error)),
    }
}

fn response_count_for_request(
    response_mode: CliDataCacheResponseMode,
    request: &MemoryRequest,
    count: usize,
) -> usize {
    if response_mode == CliDataCacheResponseMode::External && request.requires_response() {
        count
    } else {
        0
    }
}

fn msi_bank_cycle_run_summary(
    run: &MsiBankCycleRun,
    directory_decision_count: usize,
    cpu_response_count: usize,
    dram_access_count: usize,
) -> ParallelCoherenceRunSummary {
    data_cache_run_summary(
        run.tick(),
        cpu_response_count,
        directory_decision_count,
        dram_access_count,
    )
    .with_bank_activity(
        run.accepted_count(),
        run.immediate_hit_count(),
        run.scheduled_miss_count(),
        run.coalesced_miss_count(),
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
#[path = "data_cache_runtime_tests.rs"]
mod tests;
