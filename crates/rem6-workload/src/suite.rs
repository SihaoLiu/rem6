use std::collections::BTreeMap;

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
        mut self,
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        final_tick: u64,
    ) -> Result<Self, WorkloadError> {
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
            final_tick,
        ));
        self.records
            .sort_by_key(WorkloadSuiteExecutionRecord::dispatch_order);
        Ok(self)
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub fn records(&self) -> &[WorkloadSuiteExecutionRecord] {
        &self.records
    }

    pub fn maximum_final_tick(&self) -> Option<u64> {
        self.records
            .iter()
            .map(WorkloadSuiteExecutionRecord::final_tick)
            .max()
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
pub struct WorkloadSuiteExecutionRecord {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
    dispatch_order: usize,
    worker_index: usize,
    final_tick: u64,
}

impl WorkloadSuiteExecutionRecord {
    fn new(
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        final_tick: u64,
    ) -> Self {
        Self {
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
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

    pub const fn final_tick(&self) -> u64 {
        self.final_tick
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
