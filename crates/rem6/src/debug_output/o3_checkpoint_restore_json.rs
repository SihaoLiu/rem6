use std::collections::{BTreeMap, BTreeSet};

use crate::{
    formatting::json_escape, Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary,
    Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary, Rem6HostO3RuntimeCheckpointStatValue,
};

use super::{Rem6O3ExecutionModeAuthorityStat, Rem6O3TraceRecord, Rem6O3TraceStat};
use crate::debug_output::checkpoint_components_json::checkpoint_components_to_json;

const EXECUTION_MODE_AUTHORITY_JSON_LANES: [&str; 3] = ["functional", "timing", "detailed"];

const O3_CHECKPOINT_RESTORE_AUTHORITY_STAT_LANES: [(&str, usize); 3] = [
    (
        "checkpoint_restore.execution_mode_authority.mode.functional",
        0,
    ),
    ("checkpoint_restore.execution_mode_authority.mode.timing", 1),
    (
        "checkpoint_restore.execution_mode_authority.mode.detailed",
        2,
    ),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6O3CheckpointRestoreScope {
    pub(super) count: u64,
    pub(super) labels: Vec<String>,
    pub(super) latest_label: String,
    pub(super) latest_tick: u64,
    pub(super) latest_manifest_tick: u64,
    pub(super) latest_payload_bytes: u64,
    pub(super) execution_mode_authority_present_manifests: u64,
    pub(super) execution_mode_authority_cleared_manifests: u64,
    pub(super) execution_mode_authority_decode_errors: u64,
    aggregate_execution_modes: Vec<Rem6HostExecutionModeSummary>,
    latest_execution_mode_targets: BTreeSet<String>,
    pub(super) latest_components: Vec<Rem6HostCheckpointComponentSummary>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3CheckpointRestoreAuthorityTotals {
    present_manifests: u64,
    cleared_manifests: u64,
    decode_errors: u64,
    targets: u64,
    modes: [u64; 3],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6O3CheckpointRestoreComponentTotals {
    components: u64,
    chunks: u64,
    payload_bytes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Rem6O3CheckpointRestoreChunkTotals {
    chunks: u64,
    payload_bytes: u64,
    payload_checksum_accumulator: u64,
    o3_runtime_numeric: BTreeMap<String, Rem6HostO3RuntimeCheckpointStatValue>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Rem6O3CheckpointRestoreComponentStatTotals {
    components: BTreeMap<String, Rem6O3CheckpointRestoreComponentTotals>,
    chunks: BTreeMap<(String, String), Rem6O3CheckpointRestoreChunkTotals>,
    targets: BTreeMap<String, Rem6O3CheckpointRestoreComponentTotals>,
    target_components: BTreeMap<(String, String), Rem6O3CheckpointRestoreComponentTotals>,
    target_chunks: BTreeMap<(String, String, String), Rem6O3CheckpointRestoreChunkTotals>,
}

impl Rem6O3CheckpointRestoreScope {
    pub(super) fn from_summaries(summaries: &[Rem6HostCheckpointSummary]) -> Option<Self> {
        let summary = summaries.last()?;
        let aggregate_execution_modes = summaries
            .iter()
            .flat_map(|summary| summary.execution_modes.iter().cloned())
            .collect::<Vec<_>>();
        Some(Self {
            count: summaries.len() as u64,
            labels: summaries
                .iter()
                .map(|summary| summary.label.clone())
                .collect(),
            latest_label: summary.label.clone(),
            latest_tick: summary.tick,
            latest_manifest_tick: summary.manifest_tick,
            latest_payload_bytes: summary.payload_bytes,
            execution_mode_authority_present_manifests: summaries
                .iter()
                .filter(|summary| summary.execution_mode_authority_present)
                .count() as u64,
            execution_mode_authority_cleared_manifests: summaries
                .iter()
                .filter(|summary| summary.execution_mode_authority_cleared)
                .count() as u64,
            execution_mode_authority_decode_errors: summaries
                .iter()
                .filter(|summary| summary.execution_mode_authority_decode_error)
                .count() as u64,
            aggregate_execution_modes,
            latest_execution_mode_targets: summary
                .execution_modes
                .iter()
                .map(|authority| authority.target.clone())
                .collect(),
            latest_components: summary.components.clone(),
        })
    }

    pub(super) fn execution_mode_authority_targets(&self) -> u64 {
        self.aggregate_execution_modes.len() as u64
    }

    pub(super) fn execution_mode_authority_mode_counts(&self) -> [u64; 3] {
        let mut counts = [0_u64; 3];
        for execution_mode in &self.aggregate_execution_modes {
            let Some(index) = execution_mode_authority_lane_index(execution_mode.mode) else {
                continue;
            };
            counts[index] = counts[index].saturating_add(1);
        }
        counts
    }
}

impl Rem6O3CheckpointRestoreComponentTotals {
    fn add_component(&mut self, component: &Rem6HostCheckpointComponentSummary) {
        self.components = self.components.saturating_add(1);
        self.chunks = self.chunks.saturating_add(component.chunk_count);
        self.payload_bytes = self.payload_bytes.saturating_add(component.payload_bytes);
    }

    fn merge_max(&mut self, other: Self) {
        self.components = self.components.max(other.components);
        self.chunks = self.chunks.max(other.chunks);
        self.payload_bytes = self.payload_bytes.max(other.payload_bytes);
    }
}

impl Rem6O3CheckpointRestoreChunkTotals {
    fn add_chunk(&mut self, chunk: &Rem6HostCheckpointChunkSummary) {
        self.chunks = self.chunks.saturating_add(1);
        self.payload_bytes = self.payload_bytes.saturating_add(chunk.payload_bytes);
        self.payload_checksum_accumulator = self
            .payload_checksum_accumulator
            .wrapping_add(chunk.payload_checksum);
        let Some(o3_runtime) = &chunk.o3_runtime else {
            return;
        };
        for (field, value) in o3_runtime.numeric_stat_fields() {
            self.o3_runtime_numeric
                .entry(field.to_string())
                .and_modify(|current| current.merge_restore_value(value))
                .or_insert(value);
        }
    }

    fn merge_max(&mut self, other: Self) {
        self.chunks = self.chunks.max(other.chunks);
        self.payload_bytes = self.payload_bytes.max(other.payload_bytes);
        self.payload_checksum_accumulator = self
            .payload_checksum_accumulator
            .max(other.payload_checksum_accumulator);
        for (field, value) in other.o3_runtime_numeric {
            self.o3_runtime_numeric
                .entry(field)
                .and_modify(|current| current.merge_trace_duplicate(value))
                .or_insert(value);
        }
    }
}

impl Rem6O3CheckpointRestoreComponentStatTotals {
    fn from_restore(
        restore: &Rem6O3CheckpointRestoreScope,
        stat_path_segment: &impl Fn(&str) -> String,
    ) -> Self {
        let restore_targets = restore
            .latest_execution_mode_targets
            .iter()
            .map(|target| stat_path_segment(target))
            .collect::<BTreeSet<_>>();
        let mut totals = Self::default();
        for component in &restore.latest_components {
            let component_path = stat_path_segment(&component.component);
            totals
                .components
                .entry(component_path.clone())
                .or_default()
                .add_component(component);
            let is_target_component = restore_targets.contains(&component_path);
            if is_target_component {
                totals
                    .targets
                    .entry(component_path.clone())
                    .or_default()
                    .add_component(component);
                totals
                    .target_components
                    .entry((component_path.clone(), component_path.clone()))
                    .or_default()
                    .add_component(component);
            }
            for chunk in &component.chunks {
                let chunk_path = stat_path_segment(&chunk.name);
                totals
                    .chunks
                    .entry((component_path.clone(), chunk_path.clone()))
                    .or_default()
                    .add_chunk(chunk);
                if is_target_component {
                    totals
                        .target_chunks
                        .entry((component_path.clone(), component_path.clone(), chunk_path))
                        .or_default()
                        .add_chunk(chunk);
                }
            }
        }
        totals
    }

    fn merge_max(&mut self, other: Self) {
        for (key, stats) in other.components {
            self.components.entry(key).or_default().merge_max(stats);
        }
        for (key, stats) in other.chunks {
            self.chunks.entry(key).or_default().merge_max(stats);
        }
        for (key, stats) in other.targets {
            self.targets.entry(key).or_default().merge_max(stats);
        }
        for (key, stats) in other.target_components {
            self.target_components
                .entry(key)
                .or_default()
                .merge_max(stats);
        }
        for (key, stats) in other.target_chunks {
            self.target_chunks.entry(key).or_default().merge_max(stats);
        }
    }

    fn push_stats(self, stats: &mut Vec<Rem6O3ExecutionModeAuthorityStat>, prefix: &str) {
        for (component, component_stats) in self.components {
            push_component_stats(
                stats,
                &format!("{prefix}.component.{component}"),
                component_stats,
            );
        }
        for ((component, chunk), chunk_stats) in self.chunks {
            push_chunk_stats(
                stats,
                &format!("{prefix}.component.{component}.chunk.{chunk}"),
                chunk_stats,
            );
        }
        for (target, target_stats) in self.targets {
            push_component_stats(stats, &format!("{prefix}.target.{target}"), target_stats);
        }
        for ((target, component), component_stats) in self.target_components {
            push_component_stats(
                stats,
                &format!("{prefix}.target.{target}.component.{component}"),
                component_stats,
            );
        }
        for ((target, component, chunk), chunk_stats) in self.target_chunks {
            push_chunk_stats(
                stats,
                &format!("{prefix}.target.{target}.component.{component}.chunk.{chunk}"),
                chunk_stats,
            );
        }
    }
}

impl Rem6O3CheckpointRestoreAuthorityTotals {
    pub(super) fn add(&mut self, restore: &Rem6O3CheckpointRestoreScope) {
        self.present_manifests = self
            .present_manifests
            .max(restore.execution_mode_authority_present_manifests);
        self.cleared_manifests = self
            .cleared_manifests
            .max(restore.execution_mode_authority_cleared_manifests);
        self.decode_errors = self
            .decode_errors
            .max(restore.execution_mode_authority_decode_errors);
        self.targets = self.targets.max(restore.execution_mode_authority_targets());
        for (index, count) in restore
            .execution_mode_authority_mode_counts()
            .into_iter()
            .enumerate()
        {
            self.modes[index] = self.modes[index].max(count);
        }
    }

    pub(super) fn push_stats(self, stats: &mut Vec<Rem6O3TraceStat>) {
        for (suffix, value) in [
            (
                "checkpoint_restore.execution_mode_authority.manifests",
                self.present_manifests,
            ),
            (
                "checkpoint_restore.execution_mode_authority.cleared_manifests",
                self.cleared_manifests,
            ),
            (
                "checkpoint_restore.execution_mode_authority.decode_errors",
                self.decode_errors,
            ),
            (
                "checkpoint_restore.execution_mode_authority.targets",
                self.targets,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
        for (suffix, index) in O3_CHECKPOINT_RESTORE_AUTHORITY_STAT_LANES {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value: self.modes[index],
            });
        }
    }
}

pub(super) fn o3_checkpoint_restore_to_json(
    restore: Option<&Rem6O3CheckpointRestoreScope>,
) -> String {
    let Some(restore) = restore else {
        return format!(
            "{{\"count\":0,\"labels\":[],\"latest_label\":null,\"latest_tick\":0,\"latest_manifest_tick\":0,\"latest_payload_bytes\":0,\"components\":[],\"execution_mode_authority\":{}}}",
            execution_mode_authority_to_json(0, 0, 0, &[])
        );
    };
    let labels = restore
        .labels
        .iter()
        .map(|label| format!("\"{}\"", json_escape(label)))
        .collect::<Vec<_>>()
        .join(",");
    let authority = execution_mode_authority_to_json(
        restore.execution_mode_authority_present_manifests,
        restore.execution_mode_authority_cleared_manifests,
        restore.execution_mode_authority_decode_errors,
        &restore.aggregate_execution_modes,
    );
    format!(
        "{{\"count\":{},\"labels\":[{}],\"latest_label\":\"{}\",\"latest_tick\":{},\"latest_manifest_tick\":{},\"latest_payload_bytes\":{},\"components\":{},\"execution_mode_authority\":{}}}",
        restore.count,
        labels,
        json_escape(&restore.latest_label),
        restore.latest_tick,
        restore.latest_manifest_tick,
        restore.latest_payload_bytes,
        checkpoint_components_to_json(&restore.latest_components),
        authority
    )
}

pub(super) fn o3_trace_checkpoint_restore_authority_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
    let mut target_modes =
        BTreeMap::<String, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]>::new();
    for record in records {
        let Some(restore) = record.checkpoint_restore() else {
            continue;
        };
        for (target, counts) in
            execution_mode_target_counts(&restore.aggregate_execution_modes, &stat_path_segment)
        {
            let totals = target_modes.entry(target).or_default();
            for (index, count) in counts.into_iter().enumerate() {
                totals[index] = totals[index].max(count);
            }
        }
    }

    let mut stats =
        Vec::with_capacity(target_modes.len() * EXECUTION_MODE_AUTHORITY_JSON_LANES.len());
    for (target, counts) in target_modes {
        for (index, mode) in EXECUTION_MODE_AUTHORITY_JSON_LANES.iter().enumerate() {
            stats.push(Rem6O3ExecutionModeAuthorityStat::new(
                format!("checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"),
                counts[index],
            ));
        }
    }
    stats
}

pub(super) fn o3_trace_checkpoint_restore_component_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
    let mut totals = Rem6O3CheckpointRestoreComponentStatTotals::default();
    for record in records {
        let Some(restore) = record.checkpoint_restore() else {
            continue;
        };
        totals.merge_max(Rem6O3CheckpointRestoreComponentStatTotals::from_restore(
            restore,
            &stat_path_segment,
        ));
    }

    let mut stats = Vec::new();
    totals.push_stats(&mut stats, "checkpoint_restore");
    stats
}

pub(in crate::debug_output) fn o3_trace_cpu_checkpoint_restore_authority_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<(u32, Rem6O3ExecutionModeAuthorityStat)> {
    let mut cpu_target_modes =
        BTreeMap::<u32, BTreeMap<String, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]>>::new();
    for record in records {
        let Some(restore) = record.checkpoint_restore() else {
            continue;
        };
        for (target, counts) in
            execution_mode_target_counts(&restore.aggregate_execution_modes, &stat_path_segment)
        {
            let target_modes = cpu_target_modes.entry(record.cpu()).or_default();
            let totals = target_modes.entry(target).or_default();
            for (index, count) in counts.into_iter().enumerate() {
                totals[index] = totals[index].max(count);
            }
        }
    }

    let mut stats = Vec::new();
    for (cpu, target_modes) in cpu_target_modes {
        for (target, counts) in target_modes {
            for (index, mode) in EXECUTION_MODE_AUTHORITY_JSON_LANES.iter().enumerate() {
                stats.push((
                    cpu,
                    Rem6O3ExecutionModeAuthorityStat::new(
                        format!(
                            "checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
                        ),
                        counts[index],
                    ),
                ));
            }
        }
    }
    stats
}

pub(in crate::debug_output) fn o3_trace_cpu_checkpoint_restore_component_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<(u32, Rem6O3ExecutionModeAuthorityStat)> {
    let mut cpu_totals = BTreeMap::<u32, Rem6O3CheckpointRestoreComponentStatTotals>::new();
    for record in records {
        let Some(restore) = record.checkpoint_restore() else {
            continue;
        };
        cpu_totals.entry(record.cpu()).or_default().merge_max(
            Rem6O3CheckpointRestoreComponentStatTotals::from_restore(restore, &stat_path_segment),
        );
    }

    let mut stats = Vec::new();
    for (cpu, totals) in cpu_totals {
        let mut cpu_stats = Vec::new();
        totals.push_stats(&mut cpu_stats, "checkpoint_restore");
        stats.extend(cpu_stats.into_iter().map(|stat| (cpu, stat)));
    }
    stats
}

fn push_component_stats(
    stats: &mut Vec<Rem6O3ExecutionModeAuthorityStat>,
    prefix: &str,
    component_stats: Rem6O3CheckpointRestoreComponentTotals,
) {
    stats.push(Rem6O3ExecutionModeAuthorityStat::new(
        format!("{prefix}.components"),
        component_stats.components,
    ));
    stats.push(Rem6O3ExecutionModeAuthorityStat::new(
        format!("{prefix}.chunks"),
        component_stats.chunks,
    ));
    stats.push(Rem6O3ExecutionModeAuthorityStat::with_unit(
        format!("{prefix}.payload_bytes"),
        "Byte",
        component_stats.payload_bytes,
    ));
}

fn push_chunk_stats(
    stats: &mut Vec<Rem6O3ExecutionModeAuthorityStat>,
    prefix: &str,
    chunk_stats: Rem6O3CheckpointRestoreChunkTotals,
) {
    stats.push(Rem6O3ExecutionModeAuthorityStat::new(
        format!("{prefix}.chunks"),
        chunk_stats.chunks,
    ));
    stats.push(Rem6O3ExecutionModeAuthorityStat::with_unit(
        format!("{prefix}.payload_bytes"),
        "Byte",
        chunk_stats.payload_bytes,
    ));
    stats.push(Rem6O3ExecutionModeAuthorityStat::with_unit(
        format!("{prefix}.payload_checksum_accumulator"),
        "Unspecified",
        chunk_stats.payload_checksum_accumulator,
    ));
    for (field, value) in chunk_stats.o3_runtime_numeric {
        stats.push(Rem6O3ExecutionModeAuthorityStat::with_unit(
            format!("{prefix}.o3_runtime.{field}"),
            value.unit(),
            value.value(),
        ));
    }
}

fn execution_mode_authority_to_json(
    present_manifests: u64,
    cleared_manifests: u64,
    decode_errors: u64,
    execution_modes: &[Rem6HostExecutionModeSummary],
) -> String {
    let mode = execution_mode_counts_to_json(execution_modes.iter().map(|mode| mode.mode));
    let target = execution_mode_targets_to_json(execution_modes);
    format!(
        "{{\"present_manifests\":{},\"cleared_manifests\":{},\"decode_errors\":{},\"targets\":{},\"mode\":{},\"target\":{}}}",
        present_manifests,
        cleared_manifests,
        decode_errors,
        execution_modes.len(),
        mode,
        target
    )
}

fn execution_mode_counts_to_json<'a>(modes: impl Iterator<Item = &'a str>) -> String {
    let mut counts = [0_u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()];
    for mode in modes {
        if let Some(index) = execution_mode_authority_lane_index(mode) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    execution_mode_count_array_to_json(counts)
}

fn execution_mode_count_array_to_json(
    counts: [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()],
) -> String {
    let fields = EXECUTION_MODE_AUTHORITY_JSON_LANES
        .iter()
        .zip(counts)
        .map(|(mode, count)| format!("\"{mode}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn execution_mode_targets_to_json(execution_modes: &[Rem6HostExecutionModeSummary]) -> String {
    let mut targets = BTreeMap::<&str, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]>::new();
    for execution_mode in execution_modes {
        let Some(index) = execution_mode_authority_lane_index(execution_mode.mode) else {
            continue;
        };
        let counts = targets.entry(&execution_mode.target).or_default();
        counts[index] = counts[index].saturating_add(1);
    }
    let fields = targets
        .into_iter()
        .map(|(target, counts)| {
            format!(
                "\"{}\":{{\"mode\":{}}}",
                json_escape(target),
                execution_mode_count_array_to_json(counts)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn execution_mode_target_counts(
    execution_modes: &[Rem6HostExecutionModeSummary],
    stat_path_segment: &impl Fn(&str) -> String,
) -> BTreeMap<String, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]> {
    let mut targets = BTreeMap::<String, [u64; EXECUTION_MODE_AUTHORITY_JSON_LANES.len()]>::new();
    for execution_mode in execution_modes {
        let Some(index) = execution_mode_authority_lane_index(execution_mode.mode) else {
            continue;
        };
        let target = stat_path_segment(&execution_mode.target);
        let counts = targets.entry(target).or_default();
        counts[index] = counts[index].saturating_add(1);
    }
    targets
}

fn execution_mode_authority_lane_index(mode: &str) -> Option<usize> {
    EXECUTION_MODE_AUTHORITY_JSON_LANES
        .iter()
        .position(|lane| *lane == mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_component_target_stats_ignore_historical_execution_mode_authority() {
        let older = restore_summary(
            "older",
            true,
            false,
            vec![Rem6HostExecutionModeSummary {
                target: "cpu0".to_string(),
                mode: "detailed",
            }],
        );
        let latest = restore_summary("latest", false, true, Vec::new());
        let restore = Rem6O3CheckpointRestoreScope::from_summaries(&[older, latest]).unwrap();

        assert_eq!(restore.execution_mode_authority_targets(), 1);
        let stats =
            Rem6O3CheckpointRestoreComponentStatTotals::from_restore(&restore, &|segment| {
                segment.replace('-', "_")
            });
        assert!(stats.components.contains_key("cpu0"));
        assert!(stats.targets.is_empty());
        assert!(stats.target_components.is_empty());
        assert!(stats.target_chunks.is_empty());
    }

    fn restore_summary(
        label: &str,
        execution_mode_authority_present: bool,
        execution_mode_authority_cleared: bool,
        execution_modes: Vec<Rem6HostExecutionModeSummary>,
    ) -> Rem6HostCheckpointSummary {
        Rem6HostCheckpointSummary {
            tick: 17,
            event: 19,
            source: 0,
            label: label.to_string(),
            manifest_tick: 13,
            component_count: 1,
            chunk_count: 1,
            payload_bytes: 8,
            execution_mode_authority_present,
            execution_mode_authority_cleared,
            execution_mode_authority_decode_error: false,
            execution_modes,
            components: vec![Rem6HostCheckpointComponentSummary {
                component: "cpu0".to_string(),
                chunk_count: 1,
                payload_bytes: 8,
                chunks: vec![Rem6HostCheckpointChunkSummary {
                    name: "o3-runtime-state".to_string(),
                    payload_bytes: 8,
                    payload_checksum: 11,
                    o3_runtime: None,
                    o3_live_data_handoff: None,
                }],
            }],
        }
    }
}
