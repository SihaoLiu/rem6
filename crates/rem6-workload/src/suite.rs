use std::collections::BTreeMap;

use rem6_kernel::Tick;

use crate::{
    WorkloadError, WorkloadId, WorkloadManifest, WorkloadManifestIdentity, WorkloadReplayPlan,
    WorkloadResult,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadSuiteId(String);

impl WorkloadSuiteId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyWorkloadSuiteId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadSuiteIdentity(String);

impl WorkloadSuiteIdentity {
    fn new(hash: u64) -> Self {
        Self(format!("ws-{hash:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuite {
    id: WorkloadSuiteId,
    entries: Vec<WorkloadSuiteEntry>,
    identity: WorkloadSuiteIdentity,
}

impl WorkloadSuite {
    pub fn builder(id: WorkloadSuiteId) -> WorkloadSuiteBuilder {
        WorkloadSuiteBuilder::new(id)
    }

    pub const fn id(&self) -> &WorkloadSuiteId {
        &self.id
    }

    pub fn identity(&self) -> WorkloadSuiteIdentity {
        self.identity.clone()
    }

    pub fn entries(&self) -> &[WorkloadSuiteEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteEntry {
    manifest: WorkloadManifest,
}

impl WorkloadSuiteEntry {
    fn new(manifest: WorkloadManifest) -> Self {
        Self { manifest }
    }

    pub fn workload_id(&self) -> &WorkloadId {
        self.manifest.id()
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest.identity()
    }

    pub const fn manifest(&self) -> &WorkloadManifest {
        &self.manifest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteBuilder {
    id: WorkloadSuiteId,
    manifests: Vec<WorkloadManifest>,
}

impl WorkloadSuiteBuilder {
    fn new(id: WorkloadSuiteId) -> Self {
        Self {
            id,
            manifests: Vec::new(),
        }
    }

    pub fn add_manifest(mut self, manifest: WorkloadManifest) -> Result<Self, WorkloadError> {
        if self
            .manifests
            .iter()
            .any(|existing| existing.id() == manifest.id())
        {
            return Err(WorkloadError::DuplicateSuiteWorkload {
                workload: manifest.id().clone(),
            });
        }
        let identity = manifest.identity();
        if self
            .manifests
            .iter()
            .any(|existing| existing.identity() == identity)
        {
            return Err(WorkloadError::DuplicateSuiteManifest { manifest: identity });
        }
        self.manifests.push(manifest);
        Ok(self)
    }

    pub fn build(mut self) -> Result<WorkloadSuite, WorkloadError> {
        self.manifests.sort_by_key(|manifest| manifest.id().clone());
        let entries = self
            .manifests
            .into_iter()
            .map(WorkloadSuiteEntry::new)
            .collect::<Vec<_>>();
        let identity = suite_identity(&self.id, &entries);
        Ok(WorkloadSuite {
            id: self.id,
            entries,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteReplayPlan {
    suite_identity: WorkloadSuiteIdentity,
    plans: Vec<WorkloadReplayPlan>,
    entries: Vec<WorkloadSuiteReplayEntry>,
}

impl WorkloadSuiteReplayPlan {
    pub fn from_suite(suite: &WorkloadSuite) -> Result<Self, WorkloadError> {
        let mut plans = Vec::new();
        let entries = suite
            .entries()
            .iter()
            .map(|entry| {
                let plan = WorkloadReplayPlan::from_manifest(entry.manifest())?;
                let manifest_identity = plan.manifest_identity();
                plans.push(plan);
                Ok(WorkloadSuiteReplayEntry::new(
                    entry.workload_id().clone(),
                    manifest_identity,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            suite_identity: suite.identity(),
            plans,
            entries,
        })
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub fn entries(&self) -> &[WorkloadSuiteReplayEntry] {
        &self.entries
    }

    pub fn plans(&self) -> &[WorkloadReplayPlan] {
        &self.plans
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteReplayEntry {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
}

impl WorkloadSuiteReplayEntry {
    fn new(workload_id: WorkloadId, manifest_identity: WorkloadManifestIdentity) -> Self {
        Self {
            workload_id,
            manifest_identity,
        }
    }

    pub const fn workload_id(&self) -> &WorkloadId {
        &self.workload_id
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchPlan {
    suite_identity: WorkloadSuiteIdentity,
    worker_count: usize,
    records: Vec<WorkloadSuiteDispatchRecord>,
}

impl WorkloadSuiteDispatchPlan {
    pub fn from_replay_plan(
        plan: &WorkloadSuiteReplayPlan,
        worker_count: usize,
    ) -> Result<Self, WorkloadError> {
        if worker_count == 0 {
            return Err(WorkloadError::ZeroWorkloadSuiteWorkers);
        }

        let records = plan
            .entries()
            .iter()
            .enumerate()
            .map(|(order, entry)| {
                WorkloadSuiteDispatchRecord::new(
                    entry.workload_id().clone(),
                    entry.manifest_identity(),
                    order,
                    order % worker_count,
                )
            })
            .collect();

        Ok(Self {
            suite_identity: plan.suite_identity(),
            worker_count,
            records,
        })
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn active_worker_count(&self) -> usize {
        self.records
            .iter()
            .map(WorkloadSuiteDispatchRecord::worker_index)
            .max()
            .map_or(0, |worker| worker + 1)
    }

    pub fn records(&self) -> &[WorkloadSuiteDispatchRecord] {
        &self.records
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchRecord {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
    dispatch_order: usize,
    worker_index: usize,
}

impl WorkloadSuiteDispatchRecord {
    fn new(
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
    ) -> Self {
        Self {
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
        }
    }

    pub const fn workload_id(&self) -> &WorkloadId {
        &self.workload_id
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn dispatch_order(&self) -> usize {
        self.dispatch_order
    }

    pub const fn worker_index(&self) -> usize {
        self.worker_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteExecutionSummary {
    suite_identity: WorkloadSuiteIdentity,
    records: Vec<WorkloadSuiteExecutionRecord>,
}

impl WorkloadSuiteExecutionSummary {
    pub const fn new(suite_identity: WorkloadSuiteIdentity) -> Self {
        Self {
            suite_identity,
            records: Vec::new(),
        }
    }

    pub fn add_completion(
        self,
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        final_tick: u64,
    ) -> Result<Self, WorkloadError> {
        self.add_timed_completion(
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
            0,
            final_tick,
        )
    }

    pub fn add_timed_completion(
        mut self,
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        start_tick: Tick,
        final_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if start_tick > final_tick {
            return Err(WorkloadError::SuiteDispatchCompletionWindowInvalid {
                workload: workload_id,
                start_tick,
                final_tick,
            });
        }
        if self
            .records
            .iter()
            .any(|record| record.workload_id() == &workload_id)
        {
            return Err(WorkloadError::DuplicateSuiteDispatchCompletion {
                workload: workload_id,
            });
        }

        self.records.push(WorkloadSuiteExecutionRecord::new(
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
            start_tick,
            final_tick,
        ));
        self.records
            .sort_by_key(WorkloadSuiteExecutionRecord::dispatch_order);
        Ok(self)
    }

    pub fn from_dispatch_results(
        dispatch: &WorkloadSuiteDispatchPlan,
        results: &WorkloadSuiteResult,
    ) -> Result<Self, WorkloadError> {
        let expected_suite_identity = dispatch.suite_identity();
        if results.suite_identity() != expected_suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: expected_suite_identity,
                actual: results.suite_identity(),
            });
        }

        let mut expected_records = BTreeMap::new();
        for record in dispatch.records() {
            expected_records.insert(record.workload_id().clone(), record);
        }

        let mut actual_results = BTreeMap::new();
        for entry in results.results() {
            if !expected_records.contains_key(entry.workload_id()) {
                return Err(WorkloadError::UnexpectedSuiteWorkloadResult {
                    workload: entry.workload_id().clone(),
                });
            }
            actual_results.insert(entry.workload_id().clone(), entry.result());
        }

        let mut summary = Self::new(dispatch.suite_identity());
        for record in dispatch.records() {
            let Some(result) = actual_results.get(record.workload_id()) else {
                return Err(WorkloadError::MissingSuiteWorkloadResult {
                    workload: record.workload_id().clone(),
                });
            };
            let actual_identity = result.manifest_identity();
            let expected_identity = record.manifest_identity();
            if actual_identity != expected_identity {
                return Err(WorkloadError::SuiteWorkloadResultManifestMismatch {
                    workload: record.workload_id().clone(),
                    expected: expected_identity,
                    actual: actual_identity,
                });
            }
            summary = summary.add_timed_completion(
                record.workload_id().clone(),
                expected_identity,
                record.dispatch_order(),
                record.worker_index(),
                0,
                result.final_tick(),
            )?;
        }

        Ok(summary)
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub fn records(&self) -> &[WorkloadSuiteExecutionRecord] {
        &self.records
    }

    pub fn minimum_start_tick(&self) -> Option<Tick> {
        self.records
            .iter()
            .map(WorkloadSuiteExecutionRecord::start_tick)
            .min()
    }

    pub fn maximum_final_tick(&self) -> Option<Tick> {
        self.records
            .iter()
            .map(WorkloadSuiteExecutionRecord::final_tick)
            .max()
    }

    pub fn total_completion_ticks(&self) -> Tick {
        self.records
            .iter()
            .map(WorkloadSuiteExecutionRecord::duration_ticks)
            .sum()
    }

    pub fn maximum_simultaneous_workers(&self) -> usize {
        let mut ticks = self
            .records
            .iter()
            .flat_map(|record| [record.start_tick(), record.final_tick()])
            .collect::<Vec<_>>();
        ticks.sort_unstable();
        ticks.dedup();

        ticks
            .into_iter()
            .map(|tick| {
                self.records
                    .iter()
                    .filter(|record| record.start_tick() <= tick && tick < record.final_tick())
                    .map(WorkloadSuiteExecutionRecord::worker_index)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
            })
            .max()
            .unwrap_or(0)
    }

    pub fn has_parallel_worker_overlap(&self) -> bool {
        self.maximum_simultaneous_workers() > 1
    }

    pub fn worker_summaries(&self) -> Vec<WorkloadSuiteWorkerExecutionSummary> {
        let mut summaries = BTreeMap::new();
        for record in &self.records {
            summaries
                .entry(record.worker_index())
                .and_modify(|summary: &mut WorkloadSuiteWorkerExecutionSummary| {
                    summary.include(record);
                })
                .or_insert_with(|| WorkloadSuiteWorkerExecutionSummary::from_record(record));
        }
        summaries.into_values().collect()
    }

    pub fn worker_summary(
        &self,
        worker_index: usize,
    ) -> Option<WorkloadSuiteWorkerExecutionSummary> {
        let mut summary: Option<WorkloadSuiteWorkerExecutionSummary> = None;
        for record in self
            .records
            .iter()
            .filter(|record| record.worker_index() == worker_index)
        {
            match &mut summary {
                Some(summary) => summary.include(record),
                None => summary = Some(WorkloadSuiteWorkerExecutionSummary::from_record(record)),
            }
        }
        summary
    }

    pub fn verify_against_dispatch(
        &self,
        dispatch: &WorkloadSuiteDispatchPlan,
    ) -> Result<(), WorkloadError> {
        let expected_suite_identity = dispatch.suite_identity();
        if self.suite_identity != expected_suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: expected_suite_identity,
                actual: self.suite_identity.clone(),
            });
        }

        let mut expected_records = BTreeMap::new();
        for record in dispatch.records() {
            expected_records.insert(record.workload_id().clone(), record);
        }

        for record in &self.records {
            let Some(expected) = expected_records.get(record.workload_id()) else {
                return Err(WorkloadError::UnexpectedSuiteDispatchCompletion {
                    workload: record.workload_id().clone(),
                });
            };
            if record.manifest_identity() != expected.manifest_identity() {
                return Err(WorkloadError::SuiteWorkloadResultManifestMismatch {
                    workload: record.workload_id().clone(),
                    expected: expected.manifest_identity(),
                    actual: record.manifest_identity(),
                });
            }
            if record.dispatch_order() != expected.dispatch_order() {
                return Err(WorkloadError::SuiteDispatchOrderMismatch {
                    workload: record.workload_id().clone(),
                    expected: expected.dispatch_order(),
                    actual: record.dispatch_order(),
                });
            }
            if record.worker_index() != expected.worker_index() {
                return Err(WorkloadError::SuiteDispatchWorkerMismatch {
                    workload: record.workload_id().clone(),
                    expected: expected.worker_index(),
                    actual: record.worker_index(),
                });
            }
        }

        for workload in expected_records.keys() {
            if !self
                .records
                .iter()
                .any(|record| record.workload_id() == workload)
            {
                return Err(WorkloadError::MissingSuiteDispatchCompletion {
                    workload: workload.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteWorkerExecutionSummary {
    worker_index: usize,
    completion_count: usize,
    first_dispatch_order: Option<usize>,
    last_dispatch_order: Option<usize>,
    first_start_tick: Option<Tick>,
    last_final_tick: Option<Tick>,
    total_completion_ticks: Tick,
    maximum_final_tick: Option<Tick>,
}

impl WorkloadSuiteWorkerExecutionSummary {
    fn from_record(record: &WorkloadSuiteExecutionRecord) -> Self {
        Self {
            worker_index: record.worker_index(),
            completion_count: 1,
            first_dispatch_order: Some(record.dispatch_order()),
            last_dispatch_order: Some(record.dispatch_order()),
            first_start_tick: Some(record.start_tick()),
            last_final_tick: Some(record.final_tick()),
            total_completion_ticks: record.duration_ticks(),
            maximum_final_tick: Some(record.final_tick()),
        }
    }

    fn include(&mut self, record: &WorkloadSuiteExecutionRecord) {
        self.completion_count += 1;
        self.first_dispatch_order = Some(
            self.first_dispatch_order
                .map_or(record.dispatch_order(), |first| {
                    first.min(record.dispatch_order())
                }),
        );
        self.last_dispatch_order = Some(
            self.last_dispatch_order
                .map_or(record.dispatch_order(), |last| {
                    last.max(record.dispatch_order())
                }),
        );
        self.first_start_tick = Some(
            self.first_start_tick
                .map_or(record.start_tick(), |first| first.min(record.start_tick())),
        );
        self.last_final_tick = Some(
            self.last_final_tick
                .map_or(record.final_tick(), |last| last.max(record.final_tick())),
        );
        self.total_completion_ticks += record.duration_ticks();
        self.maximum_final_tick = Some(
            self.maximum_final_tick
                .map_or(record.final_tick(), |maximum| {
                    maximum.max(record.final_tick())
                }),
        );
    }

    pub const fn worker_index(&self) -> usize {
        self.worker_index
    }

    pub const fn completion_count(&self) -> usize {
        self.completion_count
    }

    pub const fn first_dispatch_order(&self) -> Option<usize> {
        self.first_dispatch_order
    }

    pub const fn last_dispatch_order(&self) -> Option<usize> {
        self.last_dispatch_order
    }

    pub const fn first_start_tick(&self) -> Option<Tick> {
        self.first_start_tick
    }

    pub const fn last_final_tick(&self) -> Option<Tick> {
        self.last_final_tick
    }

    pub const fn total_completion_ticks(&self) -> Tick {
        self.total_completion_ticks
    }

    pub const fn busy_tick_span(&self) -> Option<Tick> {
        match (self.first_start_tick, self.last_final_tick) {
            (Some(first), Some(last)) => Some(last - first),
            _ => None,
        }
    }

    pub const fn maximum_final_tick(&self) -> Option<Tick> {
        self.maximum_final_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteExecutionRecord {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
    dispatch_order: usize,
    worker_index: usize,
    start_tick: Tick,
    final_tick: Tick,
}

impl WorkloadSuiteExecutionRecord {
    fn new(
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        start_tick: Tick,
        final_tick: Tick,
    ) -> Self {
        Self {
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
            start_tick,
            final_tick,
        }
    }

    pub const fn workload_id(&self) -> &WorkloadId {
        &self.workload_id
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn dispatch_order(&self) -> usize {
        self.dispatch_order
    }

    pub const fn worker_index(&self) -> usize {
        self.worker_index
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }

    pub const fn duration_ticks(&self) -> Tick {
        self.final_tick - self.start_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteResult {
    suite_identity: WorkloadSuiteIdentity,
    results: Vec<WorkloadSuiteResultEntry>,
}

impl WorkloadSuiteResult {
    pub const fn new(suite_identity: WorkloadSuiteIdentity) -> Self {
        Self {
            suite_identity,
            results: Vec::new(),
        }
    }

    pub fn add_result(
        mut self,
        workload_id: WorkloadId,
        result: WorkloadResult,
    ) -> Result<Self, WorkloadError> {
        if self
            .results
            .iter()
            .any(|entry| entry.workload_id() == &workload_id)
        {
            return Err(WorkloadError::DuplicateSuiteWorkloadResult {
                workload: workload_id,
            });
        }
        self.results
            .push(WorkloadSuiteResultEntry::new(workload_id, result));
        self.results
            .sort_by_key(|entry| entry.workload_id().clone());
        Ok(self)
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub fn results(&self) -> &[WorkloadSuiteResultEntry] {
        &self.results
    }

    pub fn verify_against(&self, suite: &WorkloadSuite) -> Result<(), WorkloadError> {
        let expected_suite_identity = suite.identity();
        if self.suite_identity != expected_suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: expected_suite_identity,
                actual: self.suite_identity.clone(),
            });
        }

        let mut expected_manifests = BTreeMap::new();
        for entry in suite.entries() {
            expected_manifests.insert(entry.workload_id().clone(), entry.manifest_identity());
        }

        for result in &self.results {
            let Some(expected) = expected_manifests.get(result.workload_id()) else {
                return Err(WorkloadError::UnexpectedSuiteWorkloadResult {
                    workload: result.workload_id().clone(),
                });
            };
            let actual = result.result().manifest_identity();
            if &actual != expected {
                return Err(WorkloadError::SuiteWorkloadResultManifestMismatch {
                    workload: result.workload_id().clone(),
                    expected: expected.clone(),
                    actual,
                });
            }
        }

        for workload in expected_manifests.keys() {
            if !self
                .results
                .iter()
                .any(|result| result.workload_id() == workload)
            {
                return Err(WorkloadError::MissingSuiteWorkloadResult {
                    workload: workload.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteResultEntry {
    workload_id: WorkloadId,
    result: WorkloadResult,
}

impl WorkloadSuiteResultEntry {
    fn new(workload_id: WorkloadId, result: WorkloadResult) -> Self {
        Self {
            workload_id,
            result,
        }
    }

    pub const fn workload_id(&self) -> &WorkloadId {
        &self.workload_id
    }

    pub const fn result(&self) -> &WorkloadResult {
        &self.result
    }
}

fn suite_identity(id: &WorkloadSuiteId, entries: &[WorkloadSuiteEntry]) -> WorkloadSuiteIdentity {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    hash_str(&mut hash, "workload_suite.v1");
    hash_str(&mut hash, id.as_str());
    hash_u64(&mut hash, entries.len() as u64);
    for entry in entries {
        hash_str(&mut hash, entry.workload_id().as_str());
        hash_str(&mut hash, entry.manifest_identity().as_str());
    }
    WorkloadSuiteIdentity::new(hash)
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_u64(hash, value.len() as u64);
    hash_bytes(hash, value.as_bytes());
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
