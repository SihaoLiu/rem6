use std::collections::BTreeMap;

use crate::{formatting::json_escape, Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary};

use super::Rem6O3TraceStat;

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
    pub(super) label: String,
    pub(super) tick: u64,
    pub(super) manifest_tick: u64,
    pub(super) payload_bytes: u64,
    pub(super) execution_mode_authority_present_manifests: u64,
    pub(super) execution_mode_authority_cleared_manifests: u64,
    pub(super) execution_mode_authority_decode_errors: u64,
    pub(super) execution_modes: Vec<Rem6HostExecutionModeSummary>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3CheckpointRestoreAuthorityTotals {
    present_manifests: u64,
    cleared_manifests: u64,
    decode_errors: u64,
    targets: u64,
    modes: [u64; 3],
}

impl Rem6O3CheckpointRestoreScope {
    pub(super) fn from_summaries(summaries: &[Rem6HostCheckpointSummary]) -> Option<Self> {
        let summary = summaries.last()?;
        let execution_modes = summaries
            .iter()
            .flat_map(|summary| summary.execution_modes.iter().cloned())
            .collect::<Vec<_>>();
        Some(Self {
            count: summaries.len() as u64,
            labels: summaries
                .iter()
                .map(|summary| summary.label.clone())
                .collect(),
            label: summary.label.clone(),
            tick: summary.tick,
            manifest_tick: summary.manifest_tick,
            payload_bytes: summary.payload_bytes,
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
            execution_modes,
        })
    }

    pub(super) fn execution_mode_authority_targets(&self) -> u64 {
        self.execution_modes.len() as u64
    }

    pub(super) fn execution_mode_authority_mode_counts(&self) -> [u64; 3] {
        let mut counts = [0_u64; 3];
        for execution_mode in &self.execution_modes {
            let Some(index) = EXECUTION_MODE_AUTHORITY_JSON_LANES
                .iter()
                .position(|mode| *mode == execution_mode.mode)
            else {
                continue;
            };
            counts[index] = counts[index].saturating_add(1);
        }
        counts
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
            "{{\"count\":0,\"labels\":[],\"latest_label\":null,\"latest_tick\":0,\"latest_manifest_tick\":0,\"latest_payload_bytes\":0,\"execution_mode_authority\":{}}}",
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
        &restore.execution_modes,
    );
    format!(
        "{{\"count\":{},\"labels\":[{}],\"latest_label\":\"{}\",\"latest_tick\":{},\"latest_manifest_tick\":{},\"latest_payload_bytes\":{},\"execution_mode_authority\":{}}}",
        restore.count,
        labels,
        json_escape(&restore.label),
        restore.tick,
        restore.manifest_tick,
        restore.payload_bytes,
        authority
    )
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
        if let Some(index) = EXECUTION_MODE_AUTHORITY_JSON_LANES
            .iter()
            .position(|lane| *lane == mode)
        {
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
        let Some(index) = EXECUTION_MODE_AUTHORITY_JSON_LANES
            .iter()
            .position(|mode| *mode == execution_mode.mode)
        else {
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
