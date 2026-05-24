use std::collections::{BTreeMap, BTreeSet};

use rem6_cache::{
    CacheControllerError, CacheControllerResultKind, MshrQosClass, MshrQosProfile, MshrQueueConfig,
    MsiCacheBank, MsiCacheBankSnapshot,
};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryGrant, DirectoryLineState, MsiDirectory,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_transport::TargetOutcome;

use crate::{
    push_response_records_from_outcomes, CpuResponseRecord, HarnessError, SubmitKind, SubmitResult,
};

#[derive(Clone)]
pub struct MsiBankDirectoryHarness {
    layout: CacheLineLayout,
    directory: MsiDirectory,
    caches: BTreeMap<AgentId, MsiCacheBank>,
    backing: BTreeMap<Address, Vec<u8>>,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecision>,
    parallel_cycle_runs: Vec<MsiBankCycleRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankBackingLineSnapshot {
    line_address: Address,
    data: Vec<u8>,
}

impl MsiBankBackingLineSnapshot {
    pub fn new(line_address: Address, data: Vec<u8>) -> Self {
        Self { line_address, data }
    }

    pub const fn line_address(&self) -> Address {
        self.line_address
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankDirectoryHarnessSnapshot {
    layout: CacheLineLayout,
    cache_snapshots: BTreeMap<AgentId, MsiCacheBankSnapshot>,
    directory_states: Vec<DirectoryLineState>,
    backing_lines: Vec<MsiBankBackingLineSnapshot>,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecision>,
    parallel_cycle_runs: Vec<MsiBankCycleRun>,
}

#[derive(Clone, Debug)]
struct MsiBankCycleRequest {
    agent: AgentId,
    request: MemoryRequest,
    line_address: Address,
    qos: Option<MshrQosClass>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MsiBankCycleEntry {
    agent: AgentId,
    request: MemoryRequestId,
    line_address: Address,
}

impl MsiBankCycleEntry {
    pub const fn new(agent: AgentId, request: MemoryRequestId, line_address: Address) -> Self {
        Self {
            agent,
            request,
            line_address,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn line_address(&self) -> Address {
        self.line_address
    }
}

#[derive(Clone, Debug)]
pub struct MsiBankCyclePlan {
    tick: u64,
    entries: Vec<MsiBankCycleEntry>,
    requests: Vec<MsiBankCycleRequest>,
}

impl MsiBankCyclePlan {
    fn new(tick: u64, requests: Vec<MsiBankCycleRequest>) -> Self {
        let entries = requests
            .iter()
            .map(|request| {
                MsiBankCycleEntry::new(request.agent, request.request.id(), request.line_address)
            })
            .collect();
        Self {
            tick,
            entries,
            requests,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn entries(&self) -> &[MsiBankCycleEntry] {
        &self.entries
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.entries.len() > 1
    }

    pub fn lines(&self) -> Vec<Address> {
        self.entries
            .iter()
            .map(MsiBankCycleEntry::line_address)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankCycleAccepted {
    agent: AgentId,
    request: MemoryRequestId,
    line_address: Address,
    result: SubmitResult,
}

impl MsiBankCycleAccepted {
    pub const fn new(
        agent: AgentId,
        request: MemoryRequestId,
        line_address: Address,
        result: SubmitResult,
    ) -> Self {
        Self {
            agent,
            request,
            line_address,
            result,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn line_address(&self) -> Address {
        self.line_address
    }

    pub const fn result(&self) -> &SubmitResult {
        &self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankCycleRun {
    tick: u64,
    accepted: Vec<MsiBankCycleAccepted>,
    response_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankCycleHistory {
    cycle_count: usize,
    total_accepted: usize,
    total_responses: usize,
    total_immediate_hits: usize,
    total_scheduled_misses: usize,
    max_accepted_per_cycle: usize,
    has_parallel_work: bool,
    parallel_cycle_count: usize,
    single_request_cycle_count: usize,
    ticks: Vec<u64>,
    touched_lines: Vec<Address>,
    accepted_by_agent: BTreeMap<AgentId, usize>,
    accepted_by_line: BTreeMap<Address, usize>,
    accepted_by_tick: BTreeMap<u64, usize>,
    total_mshr_qos_accepted: usize,
    mshr_qos_parallel_cycle_count: usize,
    accepted_by_effective_mshr_qos_priority: BTreeMap<u8, usize>,
    accepted_by_effective_mshr_qos_requestor: BTreeMap<u32, usize>,
    best_mshr_qos_priority: Option<u8>,
}

impl MsiBankCycleHistory {
    pub fn from_runs(runs: &[MsiBankCycleRun]) -> Self {
        let mut touched_lines = BTreeSet::new();
        let mut accepted_by_agent = BTreeMap::new();
        let mut total_accepted = 0;
        let mut total_responses = 0;
        let mut total_immediate_hits = 0;
        let mut total_scheduled_misses = 0;
        let mut total_mshr_qos_accepted = 0;
        let mut max_accepted_per_cycle = 0;
        let mut has_parallel_work = false;
        let mut parallel_cycle_count = 0;
        let mut single_request_cycle_count = 0;
        let mut mshr_qos_parallel_cycle_count = 0;
        let mut ticks = Vec::with_capacity(runs.len());
        let mut accepted_by_line = BTreeMap::new();
        let mut accepted_by_tick = BTreeMap::new();
        let mut accepted_by_effective_mshr_qos_priority = BTreeMap::new();
        let mut accepted_by_effective_mshr_qos_requestor = BTreeMap::new();
        let mut best_mshr_qos_priority = None;

        for run in runs {
            let mut run_has_mshr_qos = false;
            ticks.push(run.tick());
            total_accepted += run.accepted_count();
            total_responses += run.response_count();
            total_immediate_hits += run.immediate_hit_count();
            total_scheduled_misses += run.scheduled_miss_count();
            max_accepted_per_cycle = max_accepted_per_cycle.max(run.accepted_count());
            has_parallel_work |= run.has_parallel_work();
            if run.has_parallel_work() {
                parallel_cycle_count += 1;
            } else if !run.is_empty() {
                single_request_cycle_count += 1;
            }
            accepted_by_tick.insert(run.tick(), run.accepted_count());
            for accepted in run.accepted() {
                touched_lines.insert(accepted.line_address());
                *accepted_by_agent.entry(accepted.agent()).or_insert(0) += 1;
                *accepted_by_line.entry(accepted.line_address()).or_insert(0) += 1;
                if let Some(qos) = accepted.result().cache_mshr_effective_qos() {
                    run_has_mshr_qos = true;
                    total_mshr_qos_accepted += 1;
                    *accepted_by_effective_mshr_qos_priority
                        .entry(qos.priority())
                        .or_insert(0) += 1;
                    *accepted_by_effective_mshr_qos_requestor
                        .entry(qos.requestor())
                        .or_insert(0) += 1;
                    best_mshr_qos_priority = Some(
                        best_mshr_qos_priority
                            .map_or(qos.priority(), |priority: u8| priority.min(qos.priority())),
                    );
                }
            }
            if run.has_parallel_work() && run_has_mshr_qos {
                mshr_qos_parallel_cycle_count += 1;
            }
        }

        Self {
            cycle_count: runs.len(),
            total_accepted,
            total_responses,
            total_immediate_hits,
            total_scheduled_misses,
            max_accepted_per_cycle,
            has_parallel_work,
            parallel_cycle_count,
            single_request_cycle_count,
            ticks,
            touched_lines: touched_lines.into_iter().collect(),
            accepted_by_agent,
            accepted_by_line,
            accepted_by_tick,
            total_mshr_qos_accepted,
            mshr_qos_parallel_cycle_count,
            accepted_by_effective_mshr_qos_priority,
            accepted_by_effective_mshr_qos_requestor,
            best_mshr_qos_priority,
        }
    }

    pub const fn cycle_count(&self) -> usize {
        self.cycle_count
    }

    pub const fn is_empty(&self) -> bool {
        self.cycle_count == 0
    }

    pub const fn total_accepted(&self) -> usize {
        self.total_accepted
    }

    pub const fn total_responses(&self) -> usize {
        self.total_responses
    }

    pub const fn total_immediate_hits(&self) -> usize {
        self.total_immediate_hits
    }

    pub const fn total_scheduled_misses(&self) -> usize {
        self.total_scheduled_misses
    }

    pub const fn max_accepted_per_cycle(&self) -> usize {
        self.max_accepted_per_cycle
    }

    pub const fn has_parallel_work(&self) -> bool {
        self.has_parallel_work
    }

    pub const fn parallel_cycle_count(&self) -> usize {
        self.parallel_cycle_count
    }

    pub const fn single_request_cycle_count(&self) -> usize {
        self.single_request_cycle_count
    }

    pub fn ticks(&self) -> Vec<u64> {
        self.ticks.clone()
    }

    pub fn touched_lines(&self) -> Vec<Address> {
        self.touched_lines.clone()
    }

    pub fn accepted_by_agent(&self, agent: AgentId) -> usize {
        self.accepted_by_agent.get(&agent).copied().unwrap_or(0)
    }

    pub fn accepted_by_agent_counts(&self) -> BTreeMap<AgentId, usize> {
        self.accepted_by_agent.clone()
    }

    pub fn accepted_by_line(&self, line: Address) -> usize {
        self.accepted_by_line.get(&line).copied().unwrap_or(0)
    }

    pub fn accepted_by_line_counts(&self) -> BTreeMap<Address, usize> {
        self.accepted_by_line.clone()
    }

    pub fn accepted_by_tick(&self, tick: u64) -> usize {
        self.accepted_by_tick.get(&tick).copied().unwrap_or(0)
    }

    pub fn accepted_by_tick_counts(&self) -> BTreeMap<u64, usize> {
        self.accepted_by_tick.clone()
    }

    pub const fn total_mshr_qos_accepted(&self) -> usize {
        self.total_mshr_qos_accepted
    }

    pub const fn has_mshr_qos(&self) -> bool {
        self.total_mshr_qos_accepted != 0
    }

    pub const fn mshr_qos_parallel_cycle_count(&self) -> usize {
        self.mshr_qos_parallel_cycle_count
    }

    pub fn accepted_by_effective_mshr_qos_priority(&self, priority: u8) -> usize {
        self.accepted_by_effective_mshr_qos_priority
            .get(&priority)
            .copied()
            .unwrap_or(0)
    }

    pub fn accepted_by_effective_mshr_qos_priority_counts(&self) -> BTreeMap<u8, usize> {
        self.accepted_by_effective_mshr_qos_priority.clone()
    }

    pub fn accepted_by_effective_mshr_qos_requestor(&self, requestor: u32) -> usize {
        self.accepted_by_effective_mshr_qos_requestor
            .get(&requestor)
            .copied()
            .unwrap_or(0)
    }

    pub fn accepted_by_effective_mshr_qos_requestor_counts(&self) -> BTreeMap<u32, usize> {
        self.accepted_by_effective_mshr_qos_requestor.clone()
    }

    pub const fn best_mshr_qos_priority(&self) -> Option<u8> {
        self.best_mshr_qos_priority
    }
}

impl MsiBankCycleRun {
    pub fn new(tick: u64, accepted: Vec<MsiBankCycleAccepted>, response_count: usize) -> Self {
        Self {
            tick,
            accepted,
            response_count,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn accepted(&self) -> &[MsiBankCycleAccepted] {
        &self.accepted
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.accepted.is_empty()
    }

    pub fn response_count(&self) -> usize {
        self.response_count
    }

    pub fn has_parallel_work(&self) -> bool {
        self.accepted.len() > 1
    }

    pub fn agents(&self) -> Vec<AgentId> {
        self.accepted
            .iter()
            .map(MsiBankCycleAccepted::agent)
            .collect()
    }

    pub fn requests(&self) -> Vec<MemoryRequestId> {
        self.accepted
            .iter()
            .map(MsiBankCycleAccepted::request)
            .collect()
    }

    pub fn lines(&self) -> Vec<Address> {
        self.accepted
            .iter()
            .map(MsiBankCycleAccepted::line_address)
            .collect()
    }

    pub fn has_agent(&self, agent: AgentId) -> bool {
        self.accepted
            .iter()
            .any(|accepted| accepted.agent() == agent)
    }

    pub fn has_line(&self, line: Address) -> bool {
        self.accepted
            .iter()
            .any(|accepted| accepted.line_address() == line)
    }

    pub fn immediate_hit_count(&self) -> usize {
        self.accepted
            .iter()
            .filter(|accepted| accepted.result.kind() == SubmitKind::ImmediateHit)
            .count()
    }

    pub fn scheduled_miss_count(&self) -> usize {
        self.accepted
            .iter()
            .filter(|accepted| accepted.result.kind() == SubmitKind::ScheduledMiss)
            .count()
    }

    pub fn accepted_lines(&self) -> Vec<Address> {
        self.accepted
            .iter()
            .map(MsiBankCycleAccepted::line_address)
            .collect()
    }
}

impl MsiBankDirectoryHarnessSnapshot {
    pub fn new(
        layout: CacheLineLayout,
        cache_snapshots: BTreeMap<AgentId, MsiCacheBankSnapshot>,
        directory_states: Vec<DirectoryLineState>,
        backing_lines: Vec<MsiBankBackingLineSnapshot>,
        cpu_responses: Vec<CpuResponseRecord>,
        directory_decisions: Vec<DirectoryDecision>,
        parallel_cycle_runs: Vec<MsiBankCycleRun>,
    ) -> Self {
        Self {
            layout,
            cache_snapshots,
            directory_states,
            backing_lines,
            cpu_responses,
            directory_decisions,
            parallel_cycle_runs,
        }
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub fn cache_snapshots(&self) -> &BTreeMap<AgentId, MsiCacheBankSnapshot> {
        &self.cache_snapshots
    }

    pub fn cache_count(&self) -> usize {
        self.cache_snapshots.len()
    }

    pub fn cache_agents(&self) -> Vec<AgentId> {
        self.cache_snapshots.keys().copied().collect()
    }

    pub fn cache_snapshot(&self, agent: AgentId) -> Option<&MsiCacheBankSnapshot> {
        self.cache_snapshots.get(&agent)
    }

    pub fn cache_mshr_qos_profile(&self, agent: AgentId) -> Option<MshrQosProfile> {
        self.cache_snapshot(agent)
            .and_then(MsiCacheBankSnapshot::mshr_qos_profile)
    }

    pub fn mshr_qos_profile(&self) -> MshrQosProfile {
        MshrQosProfile::from_profiles(
            self.cache_snapshots
                .values()
                .filter_map(MsiCacheBankSnapshot::mshr_qos_profile),
        )
    }

    pub fn directory_states(&self) -> &[DirectoryLineState] {
        &self.directory_states
    }

    pub fn directory_line_count(&self) -> usize {
        self.directory_states.len()
    }

    pub fn directory_line_addresses(&self) -> Vec<Address> {
        self.directory_states
            .iter()
            .map(|state| state.line().address())
            .collect()
    }

    pub fn backing_lines(&self) -> &[MsiBankBackingLineSnapshot] {
        &self.backing_lines
    }

    pub fn backing_line_count(&self) -> usize {
        self.backing_lines.len()
    }

    pub fn backing_line(&self, address: Address) -> Option<&[u8]> {
        let line_address = self.layout.line_address(address);
        self.backing_lines
            .iter()
            .find(|line| line.line_address() == line_address)
            .map(MsiBankBackingLineSnapshot::data)
    }

    pub fn cpu_responses(&self) -> &[CpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[DirectoryDecision] {
        &self.directory_decisions
    }

    pub fn parallel_cycle_runs(&self) -> &[MsiBankCycleRun] {
        &self.parallel_cycle_runs
    }

    pub fn parallel_cycle_history(&self) -> MsiBankCycleHistory {
        MsiBankCycleHistory::from_runs(&self.parallel_cycle_runs)
    }
}

impl MsiBankDirectoryHarness {
    pub fn new<I>(layout: CacheLineLayout, agents: I) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(agent, MsiCacheBank::new(agent, layout))
                .is_some()
            {
                return Err(HarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            layout,
            directory: MsiDirectory::new(),
            caches,
            backing: BTreeMap::new(),
            cpu_responses: Vec::new(),
            directory_decisions: Vec::new(),
            parallel_cycle_runs: Vec::new(),
        })
    }

    pub fn new_with_mshr<I>(
        layout: CacheLineLayout,
        agents: I,
        mshr_config: MshrQueueConfig,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(
                    agent,
                    MsiCacheBank::new_with_mshr(agent, layout, mshr_config.clone()),
                )
                .is_some()
            {
                return Err(HarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            layout,
            directory: MsiDirectory::new(),
            caches,
            backing: BTreeMap::new(),
            cpu_responses: Vec::new(),
            directory_decisions: Vec::new(),
            parallel_cycle_runs: Vec::new(),
        })
    }

    pub fn insert_backing_line(
        &mut self,
        line_address: Address,
        data: Vec<u8>,
    ) -> Result<(), HarnessError> {
        let line_address = self.layout.line_address(line_address);
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.backing.insert(line_address, data);
        Ok(())
    }

    pub fn cache_count(&self) -> usize {
        self.caches.len()
    }

    pub fn cache_agents(&self) -> Vec<AgentId> {
        self.caches.keys().copied().collect()
    }

    pub fn backing_line_addresses(&self) -> Vec<Address> {
        self.backing.keys().copied().collect()
    }

    pub fn submit_cpu_request(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        self.submit_cpu_request_at(0, agent, request)
    }

    pub fn submit_cpu_request_with_qos(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
        qos: MshrQosClass,
    ) -> Result<SubmitResult, HarnessError> {
        self.submit_cpu_request_at_inner(0, agent, request, Some(qos))
    }

    pub fn submit_coalesced_cpu_requests<I>(
        &mut self,
        tick: u64,
        agent: AgentId,
        requests: I,
    ) -> Result<Vec<SubmitResult>, HarnessError>
    where
        I: IntoIterator<Item = MemoryRequest>,
    {
        self.submit_coalesced_cpu_requests_inner(
            tick,
            agent,
            requests.into_iter().map(|request| (request, None)),
        )
    }

    pub fn submit_coalesced_cpu_requests_with_qos<I>(
        &mut self,
        tick: u64,
        agent: AgentId,
        requests: I,
    ) -> Result<Vec<SubmitResult>, HarnessError>
    where
        I: IntoIterator<Item = (MemoryRequest, MshrQosClass)>,
    {
        self.submit_coalesced_cpu_requests_inner(
            tick,
            agent,
            requests
                .into_iter()
                .map(|(request, qos)| (request, Some(qos))),
        )
    }

    fn submit_coalesced_cpu_requests_inner<I>(
        &mut self,
        tick: u64,
        agent: AgentId,
        requests: I,
    ) -> Result<Vec<SubmitResult>, HarnessError>
    where
        I: IntoIterator<Item = (MemoryRequest, Option<MshrQosClass>)>,
    {
        self.cache(agent)?;
        let requests = requests.into_iter().collect::<Vec<_>>();
        let Some(first) = requests.first() else {
            return Ok(Vec::new());
        };
        let expected_line = self.layout.line_address(first.0.line_address());
        for (request, _) in &requests {
            let actual = self.layout.line_address(request.line_address());
            if actual != expected_line {
                return Err(HarnessError::WrongLine {
                    expected: expected_line,
                    actual,
                });
            }
        }

        let mut results = Vec::with_capacity(requests.len());
        let mut pending_fill = None;
        for (request, qos) in requests {
            let result = match qos {
                Some(qos) => self
                    .cache_mut(agent)?
                    .accept_cpu_request_with_qos(request, qos),
                None => self.cache_mut(agent)?.accept_cpu_request(request),
            }
            .map_err(HarnessError::CacheBank)?;
            let cache_result = result.kind();
            let effective_qos = self.cache(agent)?.mshr_effective_qos(expected_line);
            if self.record_target_outcomes(tick, cache_result, result.target_outcomes()) > 0 {
                results.push(
                    SubmitResult::new(SubmitKind::ImmediateHit, cache_result)
                        .with_cache_mshr_effective_qos(effective_qos),
                );
                continue;
            }

            if let Some(downstream) = result.downstream_request().cloned() {
                if let Some((first, _, _)) = &pending_fill {
                    return Err(HarnessError::ParallelLineConflict {
                        line: expected_line,
                        first: *first,
                        second: downstream.id(),
                    });
                }
                let decision = self
                    .directory
                    .accept(downstream.clone())
                    .map_err(HarnessError::Directory)?;
                self.directory_decisions.push(decision.clone());
                pending_fill = Some((downstream.id(), downstream, decision.clone()));
                results.push(
                    SubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                        .with_directory_decision(decision)
                        .with_cache_mshr_effective_qos(effective_qos),
                );
            } else {
                results.push(
                    SubmitResult::new(SubmitKind::CoalescedMiss, cache_result)
                        .with_cache_mshr_effective_qos(effective_qos),
                );
            }
        }

        if let Some((_, downstream, decision)) = pending_fill {
            let response = self.directory_response(&downstream, &decision)?;
            let fill = self
                .cache_mut(agent)?
                .accept_fill(response)
                .map_err(HarnessError::CacheBank)?;
            self.record_target_outcomes(tick, fill.kind(), fill.target_outcomes());
        }

        Ok(results)
    }

    pub fn submit_parallel_cycle<I>(
        &mut self,
        tick: u64,
        requests: I,
    ) -> Result<MsiBankCycleRun, HarnessError>
    where
        I: IntoIterator<Item = (AgentId, MemoryRequest)>,
    {
        let plan = self.plan_parallel_cycle(tick, requests)?;
        self.submit_parallel_cycle_plan(plan)
    }

    pub fn submit_parallel_cycle_with_qos<I>(
        &mut self,
        tick: u64,
        requests: I,
    ) -> Result<MsiBankCycleRun, HarnessError>
    where
        I: IntoIterator<Item = (AgentId, MemoryRequest, MshrQosClass)>,
    {
        let plan = self.plan_parallel_cycle_with_qos(tick, requests)?;
        self.submit_parallel_cycle_plan(plan)
    }

    pub fn plan_parallel_cycle<I>(
        &self,
        tick: u64,
        requests: I,
    ) -> Result<MsiBankCyclePlan, HarnessError>
    where
        I: IntoIterator<Item = (AgentId, MemoryRequest)>,
    {
        let requests: Vec<_> = requests
            .into_iter()
            .map(|(agent, request)| MsiBankCycleRequest {
                agent,
                line_address: self.layout.line_address(request.line_address()),
                request,
                qos: None,
            })
            .collect();
        self.plan_parallel_cycle_inner(tick, requests)
    }

    pub fn plan_parallel_cycle_with_qos<I>(
        &self,
        tick: u64,
        requests: I,
    ) -> Result<MsiBankCyclePlan, HarnessError>
    where
        I: IntoIterator<Item = (AgentId, MemoryRequest, MshrQosClass)>,
    {
        let requests = requests
            .into_iter()
            .map(|(agent, request, qos)| MsiBankCycleRequest {
                agent,
                line_address: self.layout.line_address(request.line_address()),
                request,
                qos: Some(qos),
            })
            .collect();
        self.plan_parallel_cycle_inner(tick, requests)
    }

    fn plan_parallel_cycle_inner(
        &self,
        tick: u64,
        mut requests: Vec<MsiBankCycleRequest>,
    ) -> Result<MsiBankCyclePlan, HarnessError> {
        requests.sort_by_key(|entry| {
            (
                entry.line_address.get(),
                entry.agent.get(),
                entry.request.id().sequence(),
            )
        });

        self.validate_parallel_cycle_requests(&requests)?;
        Ok(MsiBankCyclePlan::new(tick, requests))
    }

    pub fn submit_parallel_cycle_plan(
        &mut self,
        plan: MsiBankCyclePlan,
    ) -> Result<MsiBankCycleRun, HarnessError> {
        self.validate_parallel_cycle_requests(&plan.requests)?;
        let mut next = self.clone();
        let response_count_before = next.cpu_responses.len();
        let mut accepted = Vec::with_capacity(plan.requests.len());
        for entry in plan.requests {
            let request_id = entry.request.id();
            let result =
                next.submit_cpu_request_at_inner(plan.tick, entry.agent, entry.request, entry.qos)?;
            accepted.push(MsiBankCycleAccepted::new(
                entry.agent,
                request_id,
                entry.line_address,
                result,
            ));
        }
        let response_count = next.cpu_responses.len() - response_count_before;
        let run = MsiBankCycleRun::new(plan.tick, accepted, response_count);
        if !run.is_empty() {
            next.parallel_cycle_runs.push(run.clone());
        }
        *self = next;
        Ok(run)
    }

    fn validate_parallel_cycle_requests(
        &self,
        requests: &[MsiBankCycleRequest],
    ) -> Result<(), HarnessError> {
        let mut claimed_lines = BTreeMap::new();
        for entry in requests {
            self.cache(entry.agent)?;
            if let Some(first) = claimed_lines.insert(entry.line_address, entry.request.id()) {
                return Err(HarnessError::ParallelLineConflict {
                    line: entry.line_address,
                    first,
                    second: entry.request.id(),
                });
            }
        }
        Ok(())
    }

    fn submit_cpu_request_at(
        &mut self,
        tick: u64,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        self.submit_cpu_request_at_inner(tick, agent, request, None)
    }

    fn submit_cpu_request_at_inner(
        &mut self,
        tick: u64,
        agent: AgentId,
        request: MemoryRequest,
        qos: Option<MshrQosClass>,
    ) -> Result<SubmitResult, HarnessError> {
        let line_address = self.layout.line_address(request.line_address());
        let result = match qos {
            Some(qos) => self
                .cache_mut(agent)?
                .accept_cpu_request_with_qos(request, qos),
            None => self.cache_mut(agent)?.accept_cpu_request(request),
        }
        .map_err(HarnessError::CacheBank)?;
        let cache_result = result.kind();
        let effective_qos = self.cache(agent)?.mshr_effective_qos(line_address);

        if self.record_target_outcomes(tick, cache_result, result.target_outcomes()) > 0 {
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result)
                .with_cache_mshr_effective_qos(effective_qos));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(HarnessError::Directory)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response)
            .map_err(HarnessError::CacheBank)?;
        self.record_target_outcomes(tick, fill.kind(), fill.target_outcomes());

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
            .with_directory_decision(decision)
            .with_cache_mshr_effective_qos(effective_qos))
    }

    pub fn cache_state(
        &self,
        agent: AgentId,
        address: Address,
    ) -> Result<Option<MsiState>, HarnessError> {
        Ok(self.cache(agent)?.state(address))
    }

    pub fn cache_line_addresses(&self, agent: AgentId) -> Result<Vec<Address>, HarnessError> {
        Ok(self.cache(agent)?.line_addresses())
    }

    pub fn cache_data(
        &self,
        agent: AgentId,
        address: Address,
    ) -> Result<Option<Vec<u8>>, HarnessError> {
        Ok(self.cache(agent)?.cached_data(address).map(<[u8]>::to_vec))
    }

    pub fn cache_mshr_effective_qos(
        &self,
        agent: AgentId,
        address: Address,
    ) -> Result<Option<MshrQosClass>, HarnessError> {
        Ok(self.cache(agent)?.mshr_effective_qos(address))
    }

    pub fn cache_mshr_qos_profile(
        &self,
        agent: AgentId,
    ) -> Result<Option<MshrQosProfile>, HarnessError> {
        Ok(self.cache(agent)?.mshr_qos_profile())
    }

    pub fn mshr_qos_profile(&self) -> MshrQosProfile {
        MshrQosProfile::from_profiles(
            self.caches
                .values()
                .filter_map(MsiCacheBank::mshr_qos_profile),
        )
    }

    pub fn directory_state(&self, address: Address) -> DirectoryLineState {
        self.directory
            .line_state(MsiLineId::new(self.layout.line_address(address)))
    }

    pub fn directory_line_addresses(&self) -> Vec<Address> {
        self.directory.line_addresses()
    }

    pub fn backing_line(&self, address: Address) -> Option<&[u8]> {
        self.backing
            .get(&self.layout.line_address(address))
            .map(Vec::as_slice)
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[DirectoryDecision] {
        &self.directory_decisions
    }

    pub fn parallel_cycle_runs(&self) -> &[MsiBankCycleRun] {
        &self.parallel_cycle_runs
    }

    pub fn parallel_cycle_history(&self) -> MsiBankCycleHistory {
        MsiBankCycleHistory::from_runs(&self.parallel_cycle_runs)
    }

    pub fn snapshot(&self) -> MsiBankDirectoryHarnessSnapshot {
        MsiBankDirectoryHarnessSnapshot::new(
            self.layout,
            self.caches
                .iter()
                .map(|(agent, cache)| (*agent, cache.snapshot()))
                .collect(),
            self.directory.line_states(),
            self.backing
                .iter()
                .map(|(line_address, data)| {
                    MsiBankBackingLineSnapshot::new(*line_address, data.clone())
                })
                .collect(),
            self.cpu_responses.clone(),
            self.directory_decisions.clone(),
            self.parallel_cycle_runs.clone(),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &MsiBankDirectoryHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        self.validate_snapshot_identity(snapshot)?;

        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.cache_snapshots() {
            caches
                .get_mut(agent)
                .ok_or(HarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(HarnessError::CacheBank)?;
        }

        let mut directory = MsiDirectory::new();
        for state in snapshot.directory_states() {
            let expected = self.layout.line_address(state.line().address());
            if expected != state.line().address() {
                return Err(HarnessError::WrongLine {
                    expected,
                    actual: state.line().address(),
                });
            }
        }
        directory.restore_line_states(snapshot.directory_states());

        let mut backing = BTreeMap::new();
        for line in snapshot.backing_lines() {
            let expected = self.layout.line_address(line.line_address());
            if expected != line.line_address() {
                return Err(HarnessError::WrongLine {
                    expected,
                    actual: line.line_address(),
                });
            }
            if line.data().len() as u64 != self.layout.bytes() {
                return Err(HarnessError::LineDataSizeMismatch {
                    expected: self.layout.bytes(),
                    actual: line.data().len() as u64,
                });
            }
            if backing
                .insert(line.line_address(), line.data().to_vec())
                .is_some()
            {
                return Err(HarnessError::SnapshotResourceMismatch {
                    resource: "msi bank backing line",
                });
            }
        }

        self.directory = directory;
        self.caches = caches;
        self.backing = backing;
        self.cpu_responses = snapshot.cpu_responses.clone();
        self.directory_decisions = snapshot.directory_decisions.clone();
        self.parallel_cycle_runs = snapshot.parallel_cycle_runs.clone();
        Ok(())
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &DirectoryDecision,
    ) -> Result<MemoryResponse, HarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(HarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(grant.line().address(), snoop.event())
                .map_err(HarnessError::CacheBank)?;
        }

        match grant.data_source() {
            DirectoryDataSource::BackingMemory => {
                let data = self.backing_data(grant.line())?;
                response_from_line(request, Some(data))
            }
            DirectoryDataSource::ModifiedOwner(_) => response_from_line(request, source_data),
            DirectoryDataSource::NoData => response_from_line(request, None),
        }
    }

    fn source_data(&self, grant: DirectoryGrant) -> Result<Option<Vec<u8>>, HarnessError> {
        match grant.data_source() {
            DirectoryDataSource::BackingMemory | DirectoryDataSource::NoData => Ok(None),
            DirectoryDataSource::ModifiedOwner(agent) => {
                let data = self
                    .cache(agent)?
                    .cached_data(grant.line().address())
                    .ok_or(HarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    })?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn backing_data(&self, line: MsiLineId) -> Result<Vec<u8>, HarnessError> {
        self.backing
            .get(&line.address())
            .cloned()
            .ok_or(HarnessError::MissingBackingMemory {
                line: line.address(),
            })
    }

    fn cache(&self, agent: AgentId) -> Result<&MsiCacheBank, HarnessError> {
        self.caches
            .get(&agent)
            .ok_or(HarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut MsiCacheBank, HarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(HarnessError::UnknownCache { agent })
    }

    fn record_target_outcomes(
        &mut self,
        tick: u64,
        cache_result: CacheControllerResultKind,
        outcomes: &[TargetOutcome],
    ) -> usize {
        push_response_records_from_outcomes(&mut self.cpu_responses, tick, cache_result, outcomes)
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &MsiBankDirectoryHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        if self.layout != snapshot.layout() {
            return Err(HarnessError::SnapshotResourceMismatch {
                resource: "msi bank directory harness layout",
            });
        }

        for agent in self.caches.keys() {
            if !snapshot.cache_snapshots().contains_key(agent) {
                return Err(HarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.cache_snapshots().keys() {
            if !self.caches.contains_key(agent) {
                return Err(HarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

fn response_from_line(
    request: &MemoryRequest,
    line_data: Option<Vec<u8>>,
) -> Result<MemoryResponse, HarnessError> {
    if !request.returns_data() {
        return MemoryResponse::completed(request, None).map_err(HarnessError::Memory);
    }

    let data = line_data.ok_or(HarnessError::MissingBackingMemory {
        line: request.line_address(),
    })?;
    MemoryResponse::completed(request, Some(data)).map_err(HarnessError::Memory)
}
