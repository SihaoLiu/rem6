use std::collections::BTreeMap;

use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{emit_histogram_stat, increment_stat, stat_path_segment, Rem6CliError};
use crate::Rem6HostActionSummary;

const EXECUTION_MODE_STAT_LANES: [&str; 3] = ["functional", "timing", "detailed"];

pub(super) fn emit_run_host_action_stats(
    stats: &mut StatsRegistry,
    summary: &Rem6HostActionSummary,
) -> Result<(), Rem6CliError> {
    #[derive(Default)]
    struct SwitchTransferComponentStats {
        components: u64,
        chunks: u64,
        payload_bytes: u64,
    }

    #[derive(Default)]
    struct SwitchTransferChunkStats {
        chunks: u64,
        payload_bytes: u64,
        payload_checksum_accumulator: u64,
    }

    let mut guest_host_call_arguments = 0;
    let mut guest_host_call_payload_bytes = 0;
    let mut guest_host_call_response_return_values = 0;
    let mut guest_host_call_response_payload_bytes = 0;
    for call in &summary.guest_host_calls {
        guest_host_call_arguments += call.argument_count;
        guest_host_call_payload_bytes += call.payload_bytes;
        guest_host_call_response_return_values += call.response_return_count;
        guest_host_call_response_payload_bytes += call.response_payload_bytes;
    }

    let mut execution_mode_authority_modes = BTreeMap::<&'static str, u64>::new();
    let mut execution_mode_authority_target_modes = BTreeMap::<(String, &'static str), u64>::new();
    for authority in &summary.execution_modes {
        *execution_mode_authority_modes
            .entry(authority.mode)
            .or_default() += 1;
        *execution_mode_authority_target_modes
            .entry((stat_path_segment(&authority.target), authority.mode))
            .or_default() += 1;
    }

    let mut checkpoint_restore_execution_mode_authority_manifests = 0;
    let mut checkpoint_restore_execution_mode_authority_cleared_manifests = 0;
    let mut checkpoint_restore_execution_mode_authority_targets = 0;
    let mut checkpoint_restore_execution_mode_authority_modes =
        BTreeMap::<&'static str, u64>::new();
    let mut checkpoint_restore_execution_mode_authority_target_modes =
        BTreeMap::<(String, &'static str), u64>::new();
    for restore in &summary.checkpoint_restores {
        if restore.execution_mode_authority_present {
            checkpoint_restore_execution_mode_authority_manifests += 1;
        }
        if restore.execution_mode_authority_cleared {
            checkpoint_restore_execution_mode_authority_cleared_manifests += 1;
        }
        checkpoint_restore_execution_mode_authority_targets += restore.execution_modes.len() as u64;
        for authority in &restore.execution_modes {
            *checkpoint_restore_execution_mode_authority_modes
                .entry(authority.mode)
                .or_default() += 1;
            *checkpoint_restore_execution_mode_authority_target_modes
                .entry((stat_path_segment(&authority.target), authority.mode))
                .or_default() += 1;
        }
    }

    let mut switch_modes = BTreeMap::<&'static str, u64>::new();
    let mut switch_previous_modes = BTreeMap::<&'static str, u64>::new();
    let mut switch_previous_mode_none = 0;
    for switch in &summary.execution_mode_switches {
        *switch_modes.entry(switch.mode).or_default() += 1;
        if let Some(previous_mode) = switch.previous_mode {
            *switch_previous_modes.entry(previous_mode).or_default() += 1;
        } else {
            switch_previous_mode_none += 1;
        }
    }

    let mut switch_state_transfer_count = 0;
    let mut switch_state_transfer_components = 0;
    let mut switch_state_transfer_chunks = 0;
    let mut switch_state_transfer_payload_bytes = 0;
    let mut switch_quiescence_validated = 0;
    let mut switch_quiescence_captured_components = 0;
    let mut switch_quiescence_captured_chunks = 0;
    let mut switch_quiescence_captured_payload_bytes = 0;
    let mut switch_quiescence_checker = None;
    let mut switch_quiescence_target_validated = BTreeMap::<String, u64>::new();
    let mut switch_state_transfer_component_stats =
        BTreeMap::<String, SwitchTransferComponentStats>::new();
    let mut switch_state_transfer_chunk_stats =
        BTreeMap::<(String, String), SwitchTransferChunkStats>::new();
    for transfer in summary
        .execution_mode_switches
        .iter()
        .filter_map(|switch| switch.state_transfer.as_ref())
    {
        switch_state_transfer_count += 1;
        switch_state_transfer_components += transfer.component_count;
        switch_state_transfer_chunks += transfer.chunk_count;
        switch_state_transfer_payload_bytes += transfer.payload_bytes;
        if transfer.quiescence_gate.validated {
            switch_quiescence_validated += 1;
            *switch_quiescence_target_validated
                .entry(stat_path_segment(&transfer.quiescence_gate.target))
                .or_default() += 1;
        }
        switch_quiescence_captured_components += transfer.quiescence_gate.captured_component_count;
        switch_quiescence_captured_chunks += transfer.quiescence_gate.captured_chunk_count;
        switch_quiescence_captured_payload_bytes += transfer.quiescence_gate.captured_payload_bytes;
        if let Some(checker) = transfer.quiescence_gate.checker {
            switch_quiescence_checker = Some(checker);
        }
        for component in &transfer.components {
            let component_path = stat_path_segment(&component.component);
            let component_stats = switch_state_transfer_component_stats
                .entry(component_path.clone())
                .or_default();
            component_stats.components += 1;
            component_stats.chunks += component.chunk_count;
            component_stats.payload_bytes += component.payload_bytes;

            for chunk in &component.chunks {
                let chunk_path = stat_path_segment(&chunk.name);
                let chunk_stats = switch_state_transfer_chunk_stats
                    .entry((component_path.clone(), chunk_path))
                    .or_default();
                chunk_stats.chunks += 1;
                chunk_stats.payload_bytes += chunk.payload_bytes;
                chunk_stats.payload_checksum_accumulator = chunk_stats
                    .payload_checksum_accumulator
                    .wrapping_add(chunk.payload_checksum);
            }
        }
    }

    let samples = [
        ("total", summary.total_action_count),
        ("injected_commands", summary.injected_command_count),
        ("guest_host_calls", summary.guest_host_calls.len() as u64),
        ("guest_host_call_arguments", guest_host_call_arguments),
        (
            "guest_host_call_response_return_values",
            guest_host_call_response_return_values,
        ),
        ("roi_begin", summary.roi_begin.len() as u64),
        ("roi_end", summary.roi_end.len() as u64),
        ("stats_resets", summary.stats_resets.len() as u64),
        ("stats_dumps", summary.stats_dumps.len() as u64),
        ("checkpoints", summary.checkpoints.len() as u64),
        ("checkpoint_restores", summary.checkpoint_restored_count),
        (
            "checkpoint_restored_components",
            summary.checkpoint_restored_component_count,
        ),
        (
            "checkpoint_restored_chunks",
            summary.checkpoint_restored_chunk_count,
        ),
        (
            "checkpoint_restore.execution_mode_authority.manifests",
            checkpoint_restore_execution_mode_authority_manifests,
        ),
        (
            "checkpoint_restore.execution_mode_authority.cleared_manifests",
            checkpoint_restore_execution_mode_authority_cleared_manifests,
        ),
        (
            "checkpoint_restore.execution_mode_authority.targets",
            checkpoint_restore_execution_mode_authority_targets,
        ),
        (
            "execution_mode_switches",
            summary.execution_mode_switch_count,
        ),
        (
            "execution_mode_switch_state_transfers",
            switch_state_transfer_count,
        ),
        (
            "execution_mode_switch_state_transfer_components",
            switch_state_transfer_components,
        ),
        (
            "execution_mode_switch_state_transfer_chunks",
            switch_state_transfer_chunks,
        ),
        (
            "execution_mode_switch.quiescence.validated",
            switch_quiescence_validated,
        ),
        (
            "execution_mode_switch.quiescence.captured_components",
            switch_quiescence_captured_components,
        ),
        (
            "execution_mode_switch.quiescence.captured_chunks",
            switch_quiescence_captured_chunks,
        ),
        (
            "execution_mode_authority.targets",
            summary.execution_modes.len() as u64,
        ),
        (
            "execution_mode_switch.previous_mode.none",
            switch_previous_mode_none,
        ),
        ("stops", summary.stops.len() as u64),
    ];
    for (name, value) in samples {
        increment_stat(
            stats,
            &format!("sim.host_actions.{name}"),
            "Count",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for mode in EXECUTION_MODE_STAT_LANES {
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_authority.mode.{mode}"),
            "Count",
            StatResetPolicy::Monotonic,
            execution_mode_authority_modes
                .get(mode)
                .copied()
                .unwrap_or_default(),
        )?;
        increment_stat(
            stats,
            &format!("sim.host_actions.checkpoint_restore.execution_mode_authority.mode.{mode}"),
            "Count",
            StatResetPolicy::Monotonic,
            checkpoint_restore_execution_mode_authority_modes
                .get(mode)
                .copied()
                .unwrap_or_default(),
        )?;
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_switch.mode.{mode}"),
            "Count",
            StatResetPolicy::Monotonic,
            switch_modes.get(mode).copied().unwrap_or_default(),
        )?;
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_switch.previous_mode.{mode}"),
            "Count",
            StatResetPolicy::Monotonic,
            switch_previous_modes.get(mode).copied().unwrap_or_default(),
        )?;
    }
    for ((target, mode), count) in execution_mode_authority_target_modes {
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_authority.target.{target}.mode.{mode}"),
            "Count",
            StatResetPolicy::Monotonic,
            count,
        )?;
    }
    for ((target, mode), count) in checkpoint_restore_execution_mode_authority_target_modes {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            count,
        )?;
    }
    for (target, validated) in switch_quiescence_target_validated {
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_switch.quiescence.target.{target}.validated"),
            "Count",
            StatResetPolicy::Monotonic,
            validated,
        )?;
    }
    if let Some(checker) = switch_quiescence_checker {
        increment_stat(
            stats,
            "sim.host_actions.execution_mode_switch_quiescence.checker.checked_instructions",
            "Count",
            StatResetPolicy::Monotonic,
            checker.checked_instructions,
        )?;
        increment_stat(
            stats,
            "sim.host_actions.execution_mode_switch_quiescence.checker.mismatches",
            "Count",
            StatResetPolicy::Monotonic,
            checker.mismatches,
        )?;
    }
    increment_stat(
        stats,
        "sim.host_actions.checkpoint_restored_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.checkpoint_restored_payload_bytes,
    )?;
    increment_stat(
        stats,
        "sim.host_actions.guest_host_call_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        guest_host_call_payload_bytes,
    )?;
    increment_stat(
        stats,
        "sim.host_actions.guest_host_call_response_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        guest_host_call_response_payload_bytes,
    )?;
    increment_stat(
        stats,
        "sim.host_actions.execution_mode_switch_state_transfer_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        switch_state_transfer_payload_bytes,
    )?;
    increment_stat(
        stats,
        "sim.host_actions.execution_mode_switch.quiescence.captured_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        switch_quiescence_captured_payload_bytes,
    )?;
    for (component_path, component_stats) in switch_state_transfer_component_stats {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.{component_path}.components"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.components,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.{component_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.{component_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            component_stats.payload_bytes,
        )?;
    }
    for ((component_path, chunk_path), chunk_stats) in switch_state_transfer_chunk_stats {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.{component_path}.chunk.{chunk_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            chunk_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.{component_path}.chunk.{chunk_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_bytes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.component.{component_path}.chunk.{chunk_path}.payload_checksum_accumulator"
            ),
            "Unspecified",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_checksum_accumulator,
        )?;
    }
    for (work_id, buckets) in roi_duration_histograms(summary) {
        let buckets = buckets.into_iter().collect::<Vec<_>>();
        emit_histogram_stat(
            stats,
            &format!("sim.host_actions.roi_work_item_type{work_id}.duration_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            &buckets,
        )?;
    }
    Ok(())
}

fn roi_duration_histograms(summary: &Rem6HostActionSummary) -> BTreeMap<u64, BTreeMap<u64, u64>> {
    let mut events = Vec::with_capacity(summary.roi_begin.len() + summary.roi_end.len());
    for marker in &summary.roi_begin {
        events.push((marker.tick, marker.event, true, marker));
    }
    for marker in &summary.roi_end {
        events.push((marker.tick, marker.event, false, marker));
    }
    events.sort_by_key(|(tick, event, is_begin, _)| (*tick, *event, !*is_begin));

    let mut active = BTreeMap::new();
    let mut durations: BTreeMap<u64, BTreeMap<u64, u64>> = BTreeMap::new();
    for (_, _, is_begin, marker) in events {
        let key = (marker.thread_id, marker.work_id);
        if is_begin {
            active.insert(key, marker.tick);
            continue;
        }
        let Some(start_tick) = active.remove(&key) else {
            continue;
        };
        let Some(duration) = marker.tick.checked_sub(start_tick) else {
            continue;
        };
        *durations
            .entry(marker.work_id)
            .or_default()
            .entry(duration)
            .or_default() += 1;
    }
    durations
}

#[cfg(test)]
mod tests {
    use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};

    use super::emit_run_host_action_stats;
    use crate::{
        Rem6ExecutionModeQuiescenceGateSummary, Rem6ExecutionModeStateTransferSummary,
        Rem6HostActionSummary, Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary,
        Rem6HostExecutionModeSwitchSummary,
    };

    #[test]
    fn host_action_transfer_stats_merge_normalized_path_collisions() {
        let mut stats = StatsRegistry::new();
        let summary = Rem6HostActionSummary {
            total_action_count: 2,
            execution_mode_switch_count: 2,
            execution_mode_switches: vec![
                switch_with_transfer_component_chunk("cpu-0", "pipe-0", 11, 17),
                switch_with_transfer_component_chunk("cpu_0", "pipe_0", 13, 19),
            ],
            ..Rem6HostActionSummary::default()
        };

        emit_run_host_action_stats(&mut stats, &summary).unwrap();
        let snapshot = stats.snapshot(0);

        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.component.cpu_0.components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.component.cpu_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.component.cpu_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.component.cpu_0.chunk.pipe_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.component.cpu_0.chunk.pipe_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.component.cpu_0.chunk.pipe_0.payload_checksum_accumulator",
            "Unspecified",
            StatResetPolicy::Monotonic,
            36,
        );
    }

    fn switch_with_transfer_component_chunk(
        component: &str,
        chunk: &str,
        payload_bytes: u64,
        payload_checksum: u64,
    ) -> Rem6HostExecutionModeSwitchSummary {
        Rem6HostExecutionModeSwitchSummary {
            tick: 0,
            event: 0,
            source: 0,
            target: "cpu0".to_string(),
            previous_mode: Some("atomic"),
            mode: "timing",
            stats_epoch: 0,
            stats_reset_tick: 0,
            state_transfer: Some(Rem6ExecutionModeStateTransferSummary {
                manifest_label: format!("switch-{component}-{chunk}"),
                manifest_tick: 0,
                component_count: 1,
                chunk_count: 1,
                payload_bytes,
                quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary {
                    validated: true,
                    target: "cpu0".to_string(),
                    captured_component_count: 1,
                    captured_chunk_count: 1,
                    captured_payload_bytes: payload_bytes,
                    checker: None,
                },
                components: vec![Rem6HostCheckpointComponentSummary {
                    component: component.to_string(),
                    chunk_count: 1,
                    payload_bytes,
                    chunks: vec![Rem6HostCheckpointChunkSummary {
                        name: chunk.to_string(),
                        payload_bytes,
                        payload_checksum,
                    }],
                }],
            }),
        }
    }

    fn assert_snapshot_stat(
        snapshot: &StatSnapshot,
        path: &str,
        unit: &str,
        reset_policy: StatResetPolicy,
        value: u64,
    ) {
        let matches = snapshot
            .samples()
            .iter()
            .filter(|sample| sample.path() == path)
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "expected exactly one stat path {path}, found {}",
            matches.len()
        );
        let sample = matches[0];
        assert_eq!(sample.unit(), unit, "unexpected unit for {path}");
        assert_eq!(
            sample.reset_policy(),
            reset_policy,
            "unexpected reset policy for {path}"
        );
        assert_eq!(sample.value(), value, "unexpected value for {path}");
    }
}
