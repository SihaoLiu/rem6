use rem6_kernel::Tick;
use rem6_stats::{StatHistoryRecord, StatSnapshot};

use crate::{
    WorkloadError, WorkloadExecutionMode, WorkloadExecutionModeSwitch, WorkloadHostActionSummary,
    WorkloadManifest, WorkloadManifestIdentity, WorkloadParallelExecutionSummary,
    WorkloadStatsHistorySummary,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadCheckpointChunkSummary {
    name: String,
    payload_bytes: usize,
}

impl WorkloadCheckpointChunkSummary {
    pub fn new(name: impl Into<String>, payload_bytes: usize) -> Self {
        Self {
            name: name.into(),
            payload_bytes,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadCheckpointComponentSummary {
    component: String,
    chunk_count: usize,
    payload_bytes: usize,
    chunk_summaries: Vec<WorkloadCheckpointChunkSummary>,
}

impl WorkloadCheckpointComponentSummary {
    pub fn new(component: impl Into<String>, chunk_count: usize, payload_bytes: usize) -> Self {
        Self {
            component: component.into(),
            chunk_count,
            payload_bytes,
            chunk_summaries: Vec::new(),
        }
    }

    pub fn with_chunk_summaries(
        component: impl Into<String>,
        chunk_summaries: impl IntoIterator<Item = WorkloadCheckpointChunkSummary>,
    ) -> Self {
        let mut chunk_summaries = chunk_summaries.into_iter().collect::<Vec<_>>();
        chunk_summaries.sort_by(|left, right| left.name().cmp(right.name()));
        let chunk_count = chunk_summaries.len();
        let payload_bytes = chunk_summaries
            .iter()
            .map(WorkloadCheckpointChunkSummary::payload_bytes)
            .sum();
        Self {
            component: component.into(),
            chunk_count,
            payload_bytes,
            chunk_summaries,
        }
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub const fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    pub fn chunk_summaries(&self) -> &[WorkloadCheckpointChunkSummary] {
        &self.chunk_summaries
    }

    pub fn chunk_summary(&self, name: &str) -> Option<&WorkloadCheckpointChunkSummary> {
        self.chunk_summaries()
            .iter()
            .find(|summary| summary.name() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadCheckpointManifestSummary {
    label: String,
    tick: Tick,
    component_count: usize,
    chunk_count: usize,
    payload_bytes: usize,
    component_summaries: Vec<WorkloadCheckpointComponentSummary>,
}

impl WorkloadCheckpointManifestSummary {
    pub fn new(
        label: impl Into<String>,
        tick: Tick,
        component_count: usize,
        chunk_count: usize,
        payload_bytes: usize,
    ) -> Self {
        Self {
            label: label.into(),
            tick,
            component_count,
            chunk_count,
            payload_bytes,
            component_summaries: Vec::new(),
        }
    }

    pub fn with_component_summaries(
        label: impl Into<String>,
        tick: Tick,
        component_summaries: impl IntoIterator<Item = WorkloadCheckpointComponentSummary>,
    ) -> Self {
        let component_summaries = component_summaries.into_iter().collect::<Vec<_>>();
        let component_count = component_summaries.len();
        let chunk_count = component_summaries
            .iter()
            .map(WorkloadCheckpointComponentSummary::chunk_count)
            .sum();
        let payload_bytes = component_summaries
            .iter()
            .map(WorkloadCheckpointComponentSummary::payload_bytes)
            .sum();
        Self {
            label: label.into(),
            tick,
            component_count,
            chunk_count,
            payload_bytes,
            component_summaries,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn component_count(&self) -> usize {
        self.component_count
    }

    pub const fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    pub const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    pub fn component_summaries(&self) -> &[WorkloadCheckpointComponentSummary] {
        &self.component_summaries
    }

    pub fn component_summary(
        &self,
        component: &str,
    ) -> Option<&WorkloadCheckpointComponentSummary> {
        self.component_summaries
            .iter()
            .find(|summary| summary.component() == component)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedCheckpointManifestSummary {
    label: String,
    minimum_component_count: usize,
    minimum_chunk_count: usize,
    minimum_payload_bytes: usize,
}

impl WorkloadExpectedCheckpointManifestSummary {
    pub fn new(
        label: impl Into<String>,
        minimum_component_count: usize,
        minimum_chunk_count: usize,
        minimum_payload_bytes: usize,
    ) -> Self {
        Self {
            label: label.into(),
            minimum_component_count,
            minimum_chunk_count,
            minimum_payload_bytes,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn minimum_component_count(&self) -> usize {
        self.minimum_component_count
    }

    pub const fn minimum_chunk_count(&self) -> usize {
        self.minimum_chunk_count
    }

    pub const fn minimum_payload_bytes(&self) -> usize {
        self.minimum_payload_bytes
    }

    pub(crate) fn sort_key(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedCheckpointChunkSummary {
    name: String,
    minimum_payload_bytes: usize,
}

impl WorkloadExpectedCheckpointChunkSummary {
    pub fn new(name: impl Into<String>, minimum_payload_bytes: usize) -> Self {
        Self {
            name: name.into(),
            minimum_payload_bytes,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn minimum_payload_bytes(&self) -> usize {
        self.minimum_payload_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedCheckpointComponentSummary {
    label: String,
    component: String,
    minimum_chunk_count: usize,
    minimum_payload_bytes: usize,
    required_chunk_names: Vec<String>,
    required_chunk_payloads: Vec<WorkloadExpectedCheckpointChunkSummary>,
}

impl WorkloadExpectedCheckpointComponentSummary {
    pub fn new(
        label: impl Into<String>,
        component: impl Into<String>,
        minimum_chunk_count: usize,
        minimum_payload_bytes: usize,
    ) -> Self {
        Self {
            label: label.into(),
            component: component.into(),
            minimum_chunk_count,
            minimum_payload_bytes,
            required_chunk_names: Vec::new(),
            required_chunk_payloads: Vec::new(),
        }
    }

    pub fn with_required_chunks(
        mut self,
        chunk_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.required_chunk_names = chunk_names.into_iter().map(Into::into).collect::<Vec<_>>();
        self.required_chunk_names.sort();
        self.required_chunk_names.dedup();
        self
    }

    pub fn with_required_chunk_payloads(
        mut self,
        chunk_payloads: impl IntoIterator<Item = WorkloadExpectedCheckpointChunkSummary>,
    ) -> Self {
        let mut chunk_payloads = chunk_payloads.into_iter().collect::<Vec<_>>();
        chunk_payloads.sort_by(|left, right| left.name().cmp(right.name()));
        let mut required_chunk_payloads: Vec<WorkloadExpectedCheckpointChunkSummary> = Vec::new();
        for chunk_payload in chunk_payloads {
            match required_chunk_payloads.last_mut() {
                Some(existing)
                    if existing.name() == chunk_payload.name()
                        && existing.minimum_payload_bytes()
                            < chunk_payload.minimum_payload_bytes() =>
                {
                    *existing = chunk_payload;
                }
                Some(existing) if existing.name() == chunk_payload.name() => {}
                _ => required_chunk_payloads.push(chunk_payload),
            }
        }
        self.required_chunk_payloads = required_chunk_payloads;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub const fn minimum_chunk_count(&self) -> usize {
        self.minimum_chunk_count
    }

    pub const fn minimum_payload_bytes(&self) -> usize {
        self.minimum_payload_bytes
    }

    pub fn required_chunk_names(&self) -> &[String] {
        &self.required_chunk_names
    }

    pub fn required_chunk_payloads(&self) -> &[WorkloadExpectedCheckpointChunkSummary] {
        &self.required_chunk_payloads
    }

    pub(crate) fn sort_key(&self) -> (&str, &str) {
        (&self.label, &self.component)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResult {
    manifest_identity: WorkloadManifestIdentity,
    start_tick: Tick,
    final_tick: Tick,
    stop_reason: Option<String>,
    stats_snapshot: Option<StatSnapshot>,
    stats_history_records: Vec<StatHistoryRecord>,
    parallel_execution_summary: Option<WorkloadParallelExecutionSummary>,
    host_action_summary: Option<WorkloadHostActionSummary>,
    checkpoint_labels: Vec<String>,
    restored_checkpoint_labels: Vec<String>,
    checkpoint_manifest_summaries: Vec<WorkloadCheckpointManifestSummary>,
    restored_checkpoint_manifest_summaries: Vec<WorkloadCheckpointManifestSummary>,
    execution_mode_switches: Vec<WorkloadExecutionModeSwitch>,
}

impl WorkloadResult {
    pub const fn new(manifest_identity: WorkloadManifestIdentity, final_tick: Tick) -> Self {
        Self {
            manifest_identity,
            start_tick: 0,
            final_tick,
            stop_reason: None,
            stats_snapshot: None,
            stats_history_records: Vec::new(),
            parallel_execution_summary: None,
            host_action_summary: None,
            checkpoint_labels: Vec::new(),
            restored_checkpoint_labels: Vec::new(),
            checkpoint_manifest_summaries: Vec::new(),
            restored_checkpoint_manifest_summaries: Vec::new(),
            execution_mode_switches: Vec::new(),
        }
    }

    pub fn with_start_tick(mut self, start_tick: Tick) -> Result<Self, WorkloadError> {
        if start_tick > self.final_tick {
            return Err(WorkloadError::ResultStartAfterFinalTick {
                start_tick,
                final_tick: self.final_tick,
            });
        }
        self.start_tick = start_tick;
        Ok(self)
    }

    pub fn with_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.stop_reason = Some(reason.into());
        self
    }

    pub fn with_stats_snapshot(mut self, snapshot: StatSnapshot) -> Self {
        self.stats_snapshot = Some(snapshot);
        self
    }

    pub fn with_stats_history_records(
        mut self,
        records: impl IntoIterator<Item = StatHistoryRecord>,
    ) -> Self {
        self.stats_history_records = records.into_iter().collect();
        self
    }

    pub fn with_parallel_execution_summary(
        mut self,
        summary: WorkloadParallelExecutionSummary,
    ) -> Self {
        self.parallel_execution_summary = Some(summary);
        self
    }

    pub fn with_host_action_summary(mut self, summary: WorkloadHostActionSummary) -> Self {
        self.host_action_summary = Some(summary);
        self
    }

    pub fn with_checkpoint_label(mut self, label: impl Into<String>) -> Self {
        self.checkpoint_labels.push(label.into());
        self
    }

    pub fn with_restored_checkpoint_label(mut self, label: impl Into<String>) -> Self {
        self.restored_checkpoint_labels.push(label.into());
        self
    }

    pub fn with_checkpoint_manifest_summary(
        mut self,
        summary: WorkloadCheckpointManifestSummary,
    ) -> Self {
        self.checkpoint_labels.push(summary.label().to_string());
        self.checkpoint_manifest_summaries.push(summary);
        self
    }

    pub fn with_restored_checkpoint_manifest_summary(
        mut self,
        summary: WorkloadCheckpointManifestSummary,
    ) -> Self {
        self.restored_checkpoint_labels
            .push(summary.label().to_string());
        self.restored_checkpoint_manifest_summaries.push(summary);
        self
    }

    pub fn with_execution_mode_switch(
        mut self,
        tick: Tick,
        target: impl Into<String>,
        mode: WorkloadExecutionMode,
    ) -> Self {
        self.execution_mode_switches
            .push(WorkloadExecutionModeSwitch::new(tick, target, mode));
        self
    }

    pub fn with_execution_mode_switch_stats_scope(
        mut self,
        tick: Tick,
        target: impl Into<String>,
        mode: WorkloadExecutionMode,
        stats_epoch: u64,
        stats_reset_tick: Tick,
    ) -> Self {
        self.execution_mode_switches.push(
            WorkloadExecutionModeSwitch::new(tick, target, mode)
                .with_stats_scope(stats_epoch, stats_reset_tick),
        );
        self
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }

    pub fn stop_reason(&self) -> Option<&str> {
        self.stop_reason.as_deref()
    }

    pub const fn stats_snapshot(&self) -> Option<&StatSnapshot> {
        self.stats_snapshot.as_ref()
    }

    pub fn stats_history_records(&self) -> &[StatHistoryRecord] {
        &self.stats_history_records
    }

    pub fn stats_history_summary(&self) -> WorkloadStatsHistorySummary {
        WorkloadStatsHistorySummary::from_records(&self.stats_history_records)
    }

    pub const fn parallel_execution_summary(&self) -> Option<&WorkloadParallelExecutionSummary> {
        self.parallel_execution_summary.as_ref()
    }

    pub const fn host_action_summary(&self) -> Option<&WorkloadHostActionSummary> {
        self.host_action_summary.as_ref()
    }

    pub fn checkpoint_labels(&self) -> &[String] {
        &self.checkpoint_labels
    }

    pub fn restored_checkpoint_labels(&self) -> &[String] {
        &self.restored_checkpoint_labels
    }

    pub fn checkpoint_manifest_summaries(&self) -> &[WorkloadCheckpointManifestSummary] {
        &self.checkpoint_manifest_summaries
    }

    pub fn restored_checkpoint_manifest_summaries(&self) -> &[WorkloadCheckpointManifestSummary] {
        &self.restored_checkpoint_manifest_summaries
    }

    pub fn execution_mode_switches(&self) -> &[WorkloadExecutionModeSwitch] {
        &self.execution_mode_switches
    }

    pub fn verify_manifest(&self, manifest: &WorkloadManifest) -> Result<(), WorkloadError> {
        let expected = manifest.identity();
        if self.manifest_identity != expected {
            return Err(WorkloadError::ManifestIdentityMismatch {
                expected,
                actual: self.manifest_identity.clone(),
            });
        }

        self.verify_stats_timing()
    }

    pub(crate) fn verify_stats_timing(&self) -> Result<(), WorkloadError> {
        let Some(snapshot) = &self.stats_snapshot else {
            return Ok(());
        };

        if snapshot.tick() <= self.final_tick {
            return Ok(());
        }

        Err(WorkloadError::StatsAfterFinalTick {
            stats_tick: snapshot.tick(),
            final_tick: self.final_tick,
        })
    }
}
