use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::{
    WorkloadError, WorkloadId, WorkloadManifest, WorkloadManifestIdentity, WorkloadReplayPlan,
    WorkloadResult,
};

mod dispatch;

pub use dispatch::{
    WorkloadSuiteDispatchLoadExpectation, WorkloadSuiteDispatchLoadSummary,
    WorkloadSuiteDispatchOccupancyWindow, WorkloadSuiteDispatchPlan, WorkloadSuiteDispatchRecord,
    WorkloadSuiteDispatchTimeline, WorkloadSuiteDispatchTimelineEntry, WorkloadSuiteDispatchWeight,
    WorkloadSuiteWorkerDispatchLoad,
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
pub struct WorkloadSuiteExecutionExpectation {
    suite_identity: WorkloadSuiteIdentity,
    worker_count: usize,
    minimum_simultaneous_workers: usize,
    occupancy_tick_requirements: Vec<(usize, Tick)>,
    minimum_full_occupancy_ticks: Option<Tick>,
    maximum_underoccupied_ticks: Option<Tick>,
    minimum_parallel_speedup: Option<WorkloadSuiteExecutionRatio>,
    minimum_worker_utilization: Option<WorkloadSuiteExecutionRatio>,
}

impl WorkloadSuiteExecutionExpectation {
    pub fn new(
        suite_identity: WorkloadSuiteIdentity,
        minimum_simultaneous_workers: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_simultaneous_workers == 0 {
            return Err(WorkloadError::ZeroSuiteParallelismRequirement);
        }
        Ok(Self {
            suite_identity,
            worker_count: minimum_simultaneous_workers,
            minimum_simultaneous_workers,
            occupancy_tick_requirements: Vec::new(),
            minimum_full_occupancy_ticks: None,
            maximum_underoccupied_ticks: None,
            minimum_parallel_speedup: None,
            minimum_worker_utilization: None,
        })
    }

    fn for_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = worker_count;
        self
    }

    pub fn with_minimum_occupancy_ticks_for_worker_count(
        mut self,
        worker_count: usize,
        minimum_ticks: Tick,
    ) -> Self {
        match self
            .occupancy_tick_requirements
            .binary_search_by_key(&worker_count, |(worker_count, _)| *worker_count)
        {
            Ok(index) => {
                self.occupancy_tick_requirements[index].1 =
                    self.occupancy_tick_requirements[index].1.max(minimum_ticks);
            }
            Err(index) => self
                .occupancy_tick_requirements
                .insert(index, (worker_count, minimum_ticks)),
        }
        self
    }

    pub fn with_minimum_full_occupancy_ticks(mut self, minimum_ticks: Tick) -> Self {
        self.minimum_full_occupancy_ticks = Some(
            self.minimum_full_occupancy_ticks
                .map_or(minimum_ticks, |current| current.max(minimum_ticks)),
        );
        self
    }

    pub fn with_maximum_underoccupied_ticks(mut self, maximum_ticks: Tick) -> Self {
        self.maximum_underoccupied_ticks = Some(
            self.maximum_underoccupied_ticks
                .map_or(maximum_ticks, |current| current.min(maximum_ticks)),
        );
        self
    }

    pub fn with_minimum_parallel_speedup(
        mut self,
        minimum_parallel_speedup: WorkloadSuiteExecutionRatio,
    ) -> Self {
        self.minimum_parallel_speedup = Some(minimum_parallel_speedup);
        self
    }

    pub fn with_minimum_worker_utilization(
        mut self,
        minimum_worker_utilization: WorkloadSuiteExecutionRatio,
    ) -> Self {
        self.minimum_worker_utilization = Some(minimum_worker_utilization);
        self
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub const fn minimum_simultaneous_workers(&self) -> usize {
        self.minimum_simultaneous_workers
    }

    pub const fn minimum_parallel_speedup(&self) -> Option<WorkloadSuiteExecutionRatio> {
        self.minimum_parallel_speedup
    }

    pub const fn minimum_worker_utilization(&self) -> Option<WorkloadSuiteExecutionRatio> {
        self.minimum_worker_utilization
    }

    pub const fn minimum_full_occupancy_ticks(&self) -> Option<Tick> {
        self.minimum_full_occupancy_ticks
    }

    pub const fn maximum_underoccupied_ticks(&self) -> Option<Tick> {
        self.maximum_underoccupied_ticks
    }

    pub fn occupancy_tick_requirements(&self) -> &[(usize, Tick)] {
        &self.occupancy_tick_requirements
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
                result.start_tick(),
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

    pub fn occupancy_windows(&self) -> Vec<WorkloadSuiteExecutionOccupancyWindow> {
        let mut ticks = self
            .records
            .iter()
            .flat_map(|record| [record.start_tick(), record.final_tick()])
            .collect::<Vec<_>>();
        ticks.sort_unstable();
        ticks.dedup();

        ticks
            .windows(2)
            .filter_map(|window| {
                let start_tick = window[0];
                let final_tick = window[1];
                if start_tick == final_tick {
                    return None;
                }
                let active_worker_count = self
                    .records
                    .iter()
                    .filter(|record| {
                        record.start_tick() < final_tick && start_tick < record.final_tick()
                    })
                    .map(WorkloadSuiteExecutionRecord::worker_index)
                    .collect::<BTreeSet<_>>()
                    .len();
                Some(WorkloadSuiteExecutionOccupancyWindow::new(
                    start_tick,
                    final_tick,
                    active_worker_count,
                ))
            })
            .collect()
    }

    pub fn occupancy_worker_count_tick_histogram(&self) -> BTreeMap<usize, Tick> {
        let mut histogram: BTreeMap<usize, Tick> = BTreeMap::new();
        for window in self.occupancy_windows() {
            let ticks = histogram.entry(window.active_worker_count()).or_insert(0);
            *ticks = (*ticks).saturating_add(window.duration_ticks());
        }
        histogram
    }

    pub fn occupancy_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        self.occupancy_worker_count_tick_histogram()
            .get(&worker_count)
            .copied()
            .unwrap_or(0)
    }

    pub fn verify_minimum_occupancy_ticks_for_worker_count(
        &self,
        worker_count: usize,
        minimum_ticks: Tick,
    ) -> Result<(), WorkloadError> {
        let actual_ticks = self.occupancy_ticks_for_worker_count(worker_count);
        if actual_ticks < minimum_ticks {
            return Err(
                WorkloadError::SuiteExecutionOccupancyWorkerCountTicksBelowMinimum {
                    worker_count,
                    minimum_ticks,
                    actual_ticks,
                },
            );
        }
        Ok(())
    }

    pub fn full_occupancy_ticks(&self, worker_count: usize) -> Tick {
        self.occupancy_windows()
            .iter()
            .filter(|window| window.active_worker_count() == worker_count)
            .map(WorkloadSuiteExecutionOccupancyWindow::duration_ticks)
            .sum()
    }

    pub fn underoccupied_ticks(&self, worker_count: usize) -> Tick {
        self.occupancy_windows()
            .iter()
            .filter(|window| window.active_worker_count() < worker_count)
            .map(WorkloadSuiteExecutionOccupancyWindow::duration_ticks)
            .sum()
    }

    pub fn verify_minimum_full_occupancy_ticks(
        &self,
        worker_count: usize,
        minimum_ticks: Tick,
    ) -> Result<(), WorkloadError> {
        let actual_ticks = self.full_occupancy_ticks(worker_count);
        if actual_ticks < minimum_ticks {
            return Err(
                WorkloadError::SuiteExecutionFullOccupancyTicksBelowMinimum {
                    minimum_ticks,
                    actual_ticks,
                },
            );
        }
        Ok(())
    }

    pub fn verify_maximum_underoccupied_ticks(
        &self,
        worker_count: usize,
        maximum_ticks: Tick,
    ) -> Result<(), WorkloadError> {
        let actual_ticks = self.underoccupied_ticks(worker_count);
        if actual_ticks > maximum_ticks {
            return Err(
                WorkloadError::SuiteExecutionUnderoccupiedTicksAboveMaximum {
                    maximum_ticks,
                    actual_ticks,
                },
            );
        }
        Ok(())
    }

    pub fn verify_minimum_simultaneous_workers(
        &self,
        minimum_workers: usize,
    ) -> Result<(), WorkloadError> {
        if minimum_workers == 0 {
            return Err(WorkloadError::ZeroSuiteParallelismRequirement);
        }
        let actual_workers = self.maximum_simultaneous_workers();
        if actual_workers < minimum_workers {
            return Err(WorkloadError::SuiteParallelismBelowMinimum {
                minimum_workers,
                actual_workers,
            });
        }
        Ok(())
    }

    pub fn verify_against_expectation(
        &self,
        expectation: &WorkloadSuiteExecutionExpectation,
    ) -> Result<(), WorkloadError> {
        let expected_suite_identity = expectation.suite_identity();
        if self.suite_identity != expected_suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: expected_suite_identity,
                actual: self.suite_identity.clone(),
            });
        }
        self.verify_minimum_simultaneous_workers(expectation.minimum_simultaneous_workers())?;
        for &(worker_count, minimum_ticks) in expectation.occupancy_tick_requirements() {
            self.verify_minimum_occupancy_ticks_for_worker_count(worker_count, minimum_ticks)?;
        }
        if let Some(minimum_ticks) = expectation.minimum_full_occupancy_ticks() {
            self.verify_minimum_full_occupancy_ticks(expectation.worker_count(), minimum_ticks)?;
        }
        if let Some(maximum_ticks) = expectation.maximum_underoccupied_ticks() {
            self.verify_maximum_underoccupied_ticks(expectation.worker_count(), maximum_ticks)?;
        }

        if expectation.minimum_parallel_speedup().is_some()
            || expectation.minimum_worker_utilization().is_some()
        {
            let efficiency = self.execution_efficiency(expectation.worker_count())?;
            if let Some(minimum_speedup) = expectation.minimum_parallel_speedup() {
                let actual_speedup =
                    efficiency
                        .parallel_speedup_ratio()
                        .unwrap_or(WorkloadSuiteExecutionRatio {
                            numerator: 0,
                            denominator: 1,
                        });
                if !actual_speedup.meets_or_exceeds(minimum_speedup) {
                    return Err(WorkloadError::SuiteParallelSpeedupBelowMinimum {
                        minimum_numerator: minimum_speedup.numerator(),
                        minimum_denominator: minimum_speedup.denominator(),
                        actual_numerator: actual_speedup.numerator(),
                        actual_denominator: actual_speedup.denominator(),
                    });
                }
            }
            if let Some(minimum_utilization) = expectation.minimum_worker_utilization() {
                let actual_utilization =
                    efficiency
                        .worker_utilization_ratio()
                        .unwrap_or(WorkloadSuiteExecutionRatio {
                            numerator: 0,
                            denominator: 1,
                        });
                if !actual_utilization.meets_or_exceeds(minimum_utilization) {
                    return Err(WorkloadError::SuiteWorkerUtilizationBelowMinimum {
                        minimum_numerator: minimum_utilization.numerator(),
                        minimum_denominator: minimum_utilization.denominator(),
                        actual_numerator: actual_utilization.numerator(),
                        actual_denominator: actual_utilization.denominator(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn execution_efficiency(
        &self,
        worker_count: usize,
    ) -> Result<WorkloadSuiteExecutionEfficiency, WorkloadError> {
        if worker_count == 0 {
            return Err(WorkloadError::ZeroWorkloadSuiteWorkers);
        }
        let active_workers = self
            .records
            .iter()
            .map(WorkloadSuiteExecutionRecord::worker_index)
            .max()
            .map_or(0, |worker| worker + 1);
        if worker_count < active_workers {
            return Err(WorkloadError::SuiteExecutionWorkerCountBelowActiveWorkers {
                worker_count,
                active_workers,
            });
        }
        let minimum_start_tick = self.minimum_start_tick();
        let maximum_final_tick = self.maximum_final_tick();
        let wall_clock_ticks = match (minimum_start_tick, maximum_final_tick) {
            (Some(start), Some(final_tick)) => final_tick - start,
            _ => 0,
        };
        let serial_completion_ticks = self.total_completion_ticks();
        let worker_capacity_ticks = wall_clock_ticks * worker_count as Tick;
        if worker_capacity_ticks < serial_completion_ticks {
            return Err(WorkloadError::SuiteExecutionCapacityBelowCompletionTicks {
                worker_capacity_ticks,
                serial_completion_ticks,
            });
        }
        Ok(WorkloadSuiteExecutionEfficiency::new(
            self.suite_identity(),
            worker_count,
            minimum_start_tick,
            maximum_final_tick,
            wall_clock_ticks,
            serial_completion_ticks,
            worker_capacity_ticks,
        ))
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
pub struct WorkloadSuiteExecutionOccupancyWindow {
    start_tick: Tick,
    final_tick: Tick,
    duration_ticks: Tick,
    active_worker_count: usize,
}

impl WorkloadSuiteExecutionOccupancyWindow {
    const fn new(start_tick: Tick, final_tick: Tick, active_worker_count: usize) -> Self {
        Self {
            start_tick,
            final_tick,
            duration_ticks: final_tick.saturating_sub(start_tick),
            active_worker_count,
        }
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }

    pub const fn duration_ticks(&self) -> Tick {
        self.duration_ticks
    }

    pub const fn active_worker_count(&self) -> usize {
        self.active_worker_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteExecutionEfficiency {
    suite_identity: WorkloadSuiteIdentity,
    worker_count: usize,
    minimum_start_tick: Option<Tick>,
    maximum_final_tick: Option<Tick>,
    wall_clock_ticks: Tick,
    serial_completion_ticks: Tick,
    worker_capacity_ticks: Tick,
    idle_worker_ticks: Tick,
}

impl WorkloadSuiteExecutionEfficiency {
    fn new(
        suite_identity: WorkloadSuiteIdentity,
        worker_count: usize,
        minimum_start_tick: Option<Tick>,
        maximum_final_tick: Option<Tick>,
        wall_clock_ticks: Tick,
        serial_completion_ticks: Tick,
        worker_capacity_ticks: Tick,
    ) -> Self {
        Self {
            suite_identity,
            worker_count,
            minimum_start_tick,
            maximum_final_tick,
            wall_clock_ticks,
            serial_completion_ticks,
            worker_capacity_ticks,
            idle_worker_ticks: worker_capacity_ticks.saturating_sub(serial_completion_ticks),
        }
    }

    pub fn ratio(
        numerator: Tick,
        denominator: Tick,
    ) -> Result<WorkloadSuiteExecutionRatio, WorkloadError> {
        WorkloadSuiteExecutionRatio::new(numerator, denominator)
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub const fn minimum_start_tick(&self) -> Option<Tick> {
        self.minimum_start_tick
    }

    pub const fn maximum_final_tick(&self) -> Option<Tick> {
        self.maximum_final_tick
    }

    pub const fn wall_clock_ticks(&self) -> Tick {
        self.wall_clock_ticks
    }

    pub const fn serial_completion_ticks(&self) -> Tick {
        self.serial_completion_ticks
    }

    pub const fn worker_capacity_ticks(&self) -> Tick {
        self.worker_capacity_ticks
    }

    pub const fn idle_worker_ticks(&self) -> Tick {
        self.idle_worker_ticks
    }

    pub fn parallel_speedup_ratio(&self) -> Option<WorkloadSuiteExecutionRatio> {
        WorkloadSuiteExecutionRatio::new(self.serial_completion_ticks, self.wall_clock_ticks).ok()
    }

    pub fn worker_utilization_ratio(&self) -> Option<WorkloadSuiteExecutionRatio> {
        WorkloadSuiteExecutionRatio::new(self.serial_completion_ticks, self.worker_capacity_ticks)
            .ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteExecutionRatio {
    numerator: Tick,
    denominator: Tick,
}

impl WorkloadSuiteExecutionRatio {
    pub const fn numerator(&self) -> Tick {
        self.numerator
    }

    pub const fn denominator(&self) -> Tick {
        self.denominator
    }

    pub fn meets_or_exceeds(&self, minimum: Self) -> bool {
        u128::from(self.numerator) * u128::from(minimum.denominator)
            >= u128::from(minimum.numerator) * u128::from(self.denominator)
    }

    fn new(numerator: Tick, denominator: Tick) -> Result<Self, WorkloadError> {
        if denominator == 0 {
            return Err(WorkloadError::ZeroSuiteExecutionRatioDenominator);
        }
        Ok(Self {
            numerator,
            denominator,
        })
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
