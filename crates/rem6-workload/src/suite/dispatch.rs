use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::{WorkloadError, WorkloadId, WorkloadManifestIdentity};

use super::{
    WorkloadSuiteExecutionExpectation, WorkloadSuiteExecutionRatio, WorkloadSuiteExecutionSummary,
    WorkloadSuiteIdentity, WorkloadSuiteReplayPlan, WorkloadSuiteWorkerExecutionSummary,
};

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
                    None,
                )
            })
            .collect();

        Ok(Self {
            suite_identity: plan.suite_identity(),
            worker_count,
            records,
        })
    }

    pub fn from_replay_plan_weighted(
        plan: &WorkloadSuiteReplayPlan,
        worker_count: usize,
        weights: &[WorkloadSuiteDispatchWeight],
    ) -> Result<Self, WorkloadError> {
        if worker_count == 0 {
            return Err(WorkloadError::ZeroWorkloadSuiteWorkers);
        }

        let mut expected_weights = BTreeMap::new();
        for entry in plan.entries() {
            expected_weights.insert(entry.workload_id().clone(), None);
        }
        for weight in weights {
            let Some(slot) = expected_weights.get_mut(weight.workload_id()) else {
                return Err(WorkloadError::UnexpectedSuiteDispatchWeight {
                    workload: weight.workload_id().clone(),
                });
            };
            if slot.is_some() {
                return Err(WorkloadError::DuplicateSuiteDispatchWeight {
                    workload: weight.workload_id().clone(),
                });
            }
            *slot = Some(weight.weight_ticks());
        }

        let mut worker_loads = vec![0_u64; worker_count];
        let mut records = Vec::with_capacity(plan.entries().len());
        for (order, entry) in plan.entries().iter().enumerate() {
            let Some(weight_ticks) = expected_weights
                .get(entry.workload_id())
                .and_then(|weight| *weight)
            else {
                return Err(WorkloadError::MissingSuiteDispatchWeight {
                    workload: entry.workload_id().clone(),
                });
            };
            let worker_index = worker_loads
                .iter()
                .enumerate()
                .min_by_key(|(worker, load)| (**load, *worker))
                .map(|(worker, _)| worker)
                .unwrap_or(0);
            worker_loads[worker_index] = worker_loads[worker_index].saturating_add(weight_ticks);
            records.push(WorkloadSuiteDispatchRecord::new(
                entry.workload_id().clone(),
                entry.manifest_identity(),
                order,
                worker_index,
                Some(weight_ticks),
            ));
        }

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

    pub fn estimated_load_summary(
        &self,
    ) -> Result<WorkloadSuiteDispatchLoadSummary, WorkloadError> {
        let mut worker_loads = (0..self.worker_count())
            .map(WorkloadSuiteWorkerDispatchLoad::new)
            .collect::<Vec<_>>();

        for record in &self.records {
            let Some(estimated_ticks) = record.estimated_ticks() else {
                return Err(WorkloadError::MissingSuiteDispatchEstimate {
                    workload: record.workload_id().clone(),
                });
            };
            worker_loads[record.worker_index()].include(estimated_ticks);
        }

        Ok(WorkloadSuiteDispatchLoadSummary::new(
            self.suite_identity(),
            self.worker_count(),
            worker_loads,
        ))
    }

    pub fn planned_execution_timeline(
        &self,
    ) -> Result<WorkloadSuiteDispatchTimeline, WorkloadError> {
        let mut worker_next_ticks = vec![0 as Tick; self.worker_count()];
        let mut entries = Vec::with_capacity(self.records().len());

        for record in &self.records {
            let Some(estimated_ticks) = record.estimated_ticks() else {
                return Err(WorkloadError::MissingSuiteDispatchEstimate {
                    workload: record.workload_id().clone(),
                });
            };
            let planned_start_tick = worker_next_ticks[record.worker_index()];
            let planned_final_tick = planned_start_tick.saturating_add(estimated_ticks);
            worker_next_ticks[record.worker_index()] = planned_final_tick;
            entries.push(WorkloadSuiteDispatchTimelineEntry::new(
                record.workload_id().clone(),
                record.manifest_identity(),
                record.dispatch_order(),
                record.worker_index(),
                estimated_ticks,
                planned_start_tick,
                planned_final_tick,
            ));
        }

        Ok(WorkloadSuiteDispatchTimeline::new(
            self.suite_identity(),
            self.worker_count(),
            entries,
        ))
    }

    pub fn execution_expectation(
        &self,
        minimum_simultaneous_workers: usize,
    ) -> Result<WorkloadSuiteExecutionExpectation, WorkloadError> {
        if minimum_simultaneous_workers == 0 {
            return Err(WorkloadError::ZeroSuiteParallelismRequirement);
        }
        let active_workers = self.active_worker_count();
        if minimum_simultaneous_workers > active_workers {
            return Err(
                WorkloadError::SuiteParallelismRequirementExceedsActiveWorkers {
                    minimum_workers: minimum_simultaneous_workers,
                    active_workers,
                },
            );
        }
        Ok(WorkloadSuiteExecutionExpectation::new(
            self.suite_identity(),
            minimum_simultaneous_workers,
        )?
        .for_worker_count(self.worker_count()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchWeight {
    workload_id: WorkloadId,
    weight_ticks: Tick,
}

impl WorkloadSuiteDispatchWeight {
    pub fn new(workload_id: WorkloadId, weight_ticks: Tick) -> Result<Self, WorkloadError> {
        if weight_ticks == 0 {
            return Err(WorkloadError::ZeroSuiteDispatchWeight {
                workload: workload_id,
            });
        }
        Ok(Self {
            workload_id,
            weight_ticks,
        })
    }

    pub const fn workload_id(&self) -> &WorkloadId {
        &self.workload_id
    }

    pub const fn weight_ticks(&self) -> Tick {
        self.weight_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchLoadSummary {
    suite_identity: WorkloadSuiteIdentity,
    worker_count: usize,
    worker_loads: Vec<WorkloadSuiteWorkerDispatchLoad>,
    serial_estimated_ticks: Tick,
    maximum_worker_estimated_ticks: Tick,
    worker_capacity_ticks: Tick,
    idle_worker_ticks: Tick,
}

impl WorkloadSuiteDispatchLoadSummary {
    fn new(
        suite_identity: WorkloadSuiteIdentity,
        worker_count: usize,
        worker_loads: Vec<WorkloadSuiteWorkerDispatchLoad>,
    ) -> Self {
        let serial_estimated_ticks = worker_loads
            .iter()
            .map(WorkloadSuiteWorkerDispatchLoad::estimated_ticks)
            .sum();
        let maximum_worker_estimated_ticks = worker_loads
            .iter()
            .map(WorkloadSuiteWorkerDispatchLoad::estimated_ticks)
            .max()
            .unwrap_or(0);
        let worker_capacity_ticks =
            maximum_worker_estimated_ticks.saturating_mul(worker_count as Tick);
        let idle_worker_ticks = worker_capacity_ticks.saturating_sub(serial_estimated_ticks);
        Self {
            suite_identity,
            worker_count,
            worker_loads,
            serial_estimated_ticks,
            maximum_worker_estimated_ticks,
            worker_capacity_ticks,
            idle_worker_ticks,
        }
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn worker_loads(&self) -> &[WorkloadSuiteWorkerDispatchLoad] {
        &self.worker_loads
    }

    pub const fn serial_estimated_ticks(&self) -> Tick {
        self.serial_estimated_ticks
    }

    pub const fn maximum_worker_estimated_ticks(&self) -> Tick {
        self.maximum_worker_estimated_ticks
    }

    pub const fn worker_capacity_ticks(&self) -> Tick {
        self.worker_capacity_ticks
    }

    pub const fn idle_worker_ticks(&self) -> Tick {
        self.idle_worker_ticks
    }

    pub fn parallel_speedup_ratio(&self) -> Option<WorkloadSuiteExecutionRatio> {
        WorkloadSuiteExecutionRatio::new(
            self.serial_estimated_ticks,
            self.maximum_worker_estimated_ticks,
        )
        .ok()
    }

    pub fn worker_utilization_ratio(&self) -> Option<WorkloadSuiteExecutionRatio> {
        WorkloadSuiteExecutionRatio::new(self.serial_estimated_ticks, self.worker_capacity_ticks)
            .ok()
    }

    pub fn verify_against_expectation(
        &self,
        expectation: &WorkloadSuiteDispatchLoadExpectation,
    ) -> Result<(), WorkloadError> {
        let expected_suite_identity = expectation.suite_identity();
        if self.suite_identity != expected_suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: expected_suite_identity,
                actual: self.suite_identity.clone(),
            });
        }
        if self.worker_count() != expectation.worker_count() {
            return Err(WorkloadError::SuiteDispatchWorkerCountMismatch {
                expected: expectation.worker_count(),
                actual: self.worker_count(),
            });
        }

        if let Some(minimum_speedup) = expectation.minimum_parallel_speedup() {
            let actual_speedup =
                self.parallel_speedup_ratio()
                    .unwrap_or(WorkloadSuiteExecutionRatio {
                        numerator: 0,
                        denominator: 1,
                    });
            if !actual_speedup.meets_or_exceeds(minimum_speedup) {
                return Err(WorkloadError::SuitePlannedParallelSpeedupBelowMinimum {
                    minimum_numerator: minimum_speedup.numerator(),
                    minimum_denominator: minimum_speedup.denominator(),
                    actual_numerator: actual_speedup.numerator(),
                    actual_denominator: actual_speedup.denominator(),
                });
            }
        }

        if let Some(minimum_utilization) = expectation.minimum_worker_utilization() {
            let actual_utilization =
                self.worker_utilization_ratio()
                    .unwrap_or(WorkloadSuiteExecutionRatio {
                        numerator: 0,
                        denominator: 1,
                    });
            if !actual_utilization.meets_or_exceeds(minimum_utilization) {
                return Err(WorkloadError::SuitePlannedWorkerUtilizationBelowMinimum {
                    minimum_numerator: minimum_utilization.numerator(),
                    minimum_denominator: minimum_utilization.denominator(),
                    actual_numerator: actual_utilization.numerator(),
                    actual_denominator: actual_utilization.denominator(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchLoadExpectation {
    suite_identity: WorkloadSuiteIdentity,
    worker_count: usize,
    minimum_parallel_speedup: Option<WorkloadSuiteExecutionRatio>,
    minimum_worker_utilization: Option<WorkloadSuiteExecutionRatio>,
}

impl WorkloadSuiteDispatchLoadExpectation {
    pub fn new(
        suite_identity: WorkloadSuiteIdentity,
        worker_count: usize,
    ) -> Result<Self, WorkloadError> {
        if worker_count == 0 {
            return Err(WorkloadError::ZeroWorkloadSuiteWorkers);
        }
        Ok(Self {
            suite_identity,
            worker_count,
            minimum_parallel_speedup: None,
            minimum_worker_utilization: None,
        })
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

    pub const fn minimum_parallel_speedup(&self) -> Option<WorkloadSuiteExecutionRatio> {
        self.minimum_parallel_speedup
    }

    pub const fn minimum_worker_utilization(&self) -> Option<WorkloadSuiteExecutionRatio> {
        self.minimum_worker_utilization
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchTimeline {
    suite_identity: WorkloadSuiteIdentity,
    worker_count: usize,
    entries: Vec<WorkloadSuiteDispatchTimelineEntry>,
}

impl WorkloadSuiteDispatchTimeline {
    fn new(
        suite_identity: WorkloadSuiteIdentity,
        worker_count: usize,
        entries: Vec<WorkloadSuiteDispatchTimelineEntry>,
    ) -> Self {
        Self {
            suite_identity,
            worker_count,
            entries,
        }
    }

    pub fn suite_identity(&self) -> WorkloadSuiteIdentity {
        self.suite_identity.clone()
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub fn entries(&self) -> &[WorkloadSuiteDispatchTimelineEntry] {
        &self.entries
    }

    pub fn minimum_start_tick(&self) -> Option<Tick> {
        self.entries
            .iter()
            .map(WorkloadSuiteDispatchTimelineEntry::planned_start_tick)
            .min()
    }

    pub fn maximum_final_tick(&self) -> Option<Tick> {
        self.entries
            .iter()
            .map(WorkloadSuiteDispatchTimelineEntry::planned_final_tick)
            .max()
    }

    pub fn wall_clock_ticks(&self) -> Tick {
        match (self.minimum_start_tick(), self.maximum_final_tick()) {
            (Some(start), Some(final_tick)) => final_tick - start,
            _ => 0,
        }
    }

    pub fn total_estimated_ticks(&self) -> Tick {
        self.entries
            .iter()
            .map(WorkloadSuiteDispatchTimelineEntry::estimated_ticks)
            .sum()
    }

    pub fn maximum_simultaneous_workers(&self) -> usize {
        let mut ticks = self
            .entries
            .iter()
            .flat_map(|entry| [entry.planned_start_tick(), entry.planned_final_tick()])
            .collect::<Vec<_>>();
        ticks.sort_unstable();
        ticks.dedup();

        ticks
            .into_iter()
            .map(|tick| {
                self.entries
                    .iter()
                    .filter(|entry| {
                        entry.planned_start_tick() <= tick && tick < entry.planned_final_tick()
                    })
                    .map(WorkloadSuiteDispatchTimelineEntry::worker_index)
                    .collect::<BTreeSet<_>>()
                    .len()
            })
            .max()
            .unwrap_or(0)
    }

    pub fn to_execution_summary(&self) -> Result<WorkloadSuiteExecutionSummary, WorkloadError> {
        let mut summary = WorkloadSuiteExecutionSummary::new(self.suite_identity());
        for entry in &self.entries {
            summary = summary.add_timed_completion(
                entry.workload_id().clone(),
                entry.manifest_identity(),
                entry.dispatch_order(),
                entry.worker_index(),
                entry.planned_start_tick(),
                entry.planned_final_tick(),
            )?;
        }
        Ok(summary)
    }

    pub fn worker_summaries(
        &self,
    ) -> Result<Vec<WorkloadSuiteWorkerExecutionSummary>, WorkloadError> {
        Ok(self.to_execution_summary()?.worker_summaries())
    }

    pub fn worker_summary(
        &self,
        worker_index: usize,
    ) -> Result<Option<WorkloadSuiteWorkerExecutionSummary>, WorkloadError> {
        Ok(self.to_execution_summary()?.worker_summary(worker_index))
    }

    pub fn worker_idle_ticks(&self, worker_index: usize) -> Result<Option<Tick>, WorkloadError> {
        if worker_index >= self.worker_count() {
            return Ok(None);
        }

        let Some(summary) = self.worker_summary(worker_index)? else {
            return Ok(Some(self.wall_clock_ticks()));
        };

        Ok(Some(
            self.wall_clock_ticks()
                .saturating_sub(summary.total_completion_ticks()),
        ))
    }

    pub fn total_worker_idle_ticks(&self) -> Result<Tick, WorkloadError> {
        Ok(self
            .to_execution_summary()?
            .execution_efficiency(self.worker_count())?
            .idle_worker_ticks())
    }

    pub fn verify_against_expectation(
        &self,
        expectation: &WorkloadSuiteExecutionExpectation,
    ) -> Result<(), WorkloadError> {
        let expected_suite_identity = expectation.suite_identity();
        if self.suite_identity != expected_suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: expected_suite_identity,
                actual: self.suite_identity(),
            });
        }
        if self.worker_count() != expectation.worker_count() {
            return Err(WorkloadError::SuiteDispatchWorkerCountMismatch {
                expected: expectation.worker_count(),
                actual: self.worker_count(),
            });
        }

        let actual_workers = self.maximum_simultaneous_workers();
        if actual_workers < expectation.minimum_simultaneous_workers() {
            return Err(WorkloadError::SuiteParallelismBelowMinimum {
                minimum_workers: expectation.minimum_simultaneous_workers(),
                actual_workers,
            });
        }

        if expectation.minimum_parallel_speedup().is_some()
            || expectation.minimum_worker_utilization().is_some()
        {
            let efficiency = self
                .to_execution_summary()?
                .execution_efficiency(self.worker_count())?;
            if let Some(minimum_speedup) = expectation.minimum_parallel_speedup() {
                let actual_speedup =
                    efficiency
                        .parallel_speedup_ratio()
                        .unwrap_or(WorkloadSuiteExecutionRatio {
                            numerator: 0,
                            denominator: 1,
                        });
                if !actual_speedup.meets_or_exceeds(minimum_speedup) {
                    return Err(WorkloadError::SuitePlannedParallelSpeedupBelowMinimum {
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
                    return Err(WorkloadError::SuitePlannedWorkerUtilizationBelowMinimum {
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

    pub fn verify_execution_summary(
        &self,
        summary: &WorkloadSuiteExecutionSummary,
    ) -> Result<(), WorkloadError> {
        let mut expected_records = BTreeMap::new();
        for entry in &self.entries {
            expected_records.insert(entry.workload_id().clone(), entry);
        }

        if summary.suite_identity() != self.suite_identity {
            return Err(WorkloadError::WorkloadSuiteIdentityMismatch {
                expected: self.suite_identity.clone(),
                actual: summary.suite_identity(),
            });
        }

        for record in summary.records() {
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
            if record.start_tick() != expected.planned_start_tick()
                || record.final_tick() != expected.planned_final_tick()
            {
                return Err(WorkloadError::SuiteDispatchTimelineWindowMismatch {
                    workload: record.workload_id().clone(),
                    expected_start_tick: expected.planned_start_tick(),
                    expected_final_tick: expected.planned_final_tick(),
                    actual_start_tick: record.start_tick(),
                    actual_final_tick: record.final_tick(),
                });
            }
        }

        for workload in expected_records.keys() {
            if !summary
                .records()
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
pub struct WorkloadSuiteDispatchTimelineEntry {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
    dispatch_order: usize,
    worker_index: usize,
    estimated_ticks: Tick,
    planned_start_tick: Tick,
    planned_final_tick: Tick,
}

impl WorkloadSuiteDispatchTimelineEntry {
    fn new(
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        estimated_ticks: Tick,
        planned_start_tick: Tick,
        planned_final_tick: Tick,
    ) -> Self {
        Self {
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
            estimated_ticks,
            planned_start_tick,
            planned_final_tick,
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

    pub const fn estimated_ticks(&self) -> Tick {
        self.estimated_ticks
    }

    pub const fn planned_start_tick(&self) -> Tick {
        self.planned_start_tick
    }

    pub const fn planned_final_tick(&self) -> Tick {
        self.planned_final_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteWorkerDispatchLoad {
    worker_index: usize,
    workload_count: usize,
    estimated_ticks: Tick,
}

impl WorkloadSuiteWorkerDispatchLoad {
    const fn new(worker_index: usize) -> Self {
        Self {
            worker_index,
            workload_count: 0,
            estimated_ticks: 0,
        }
    }

    fn include(&mut self, estimated_ticks: Tick) {
        self.workload_count += 1;
        self.estimated_ticks = self.estimated_ticks.saturating_add(estimated_ticks);
    }

    pub const fn worker_index(&self) -> usize {
        self.worker_index
    }

    pub const fn workload_count(&self) -> usize {
        self.workload_count
    }

    pub const fn estimated_ticks(&self) -> Tick {
        self.estimated_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadSuiteDispatchRecord {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
    dispatch_order: usize,
    worker_index: usize,
    estimated_ticks: Option<Tick>,
}

impl WorkloadSuiteDispatchRecord {
    fn new(
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        dispatch_order: usize,
        worker_index: usize,
        estimated_ticks: Option<Tick>,
    ) -> Self {
        Self {
            workload_id,
            manifest_identity,
            dispatch_order,
            worker_index,
            estimated_ticks,
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

    pub const fn estimated_ticks(&self) -> Option<Tick> {
        self.estimated_ticks
    }
}
