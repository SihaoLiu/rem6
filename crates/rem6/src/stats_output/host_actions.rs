use std::collections::{BTreeMap, BTreeSet};

use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{emit_histogram_stat, increment_stat, stat_path_segment, Rem6CliError};
use crate::{host_actions::Rem6ExecutionModeSwitchCheckerSummary, Rem6HostActionSummary};

const EXECUTION_MODE_STAT_LANES: [&str; 3] = ["functional", "timing", "detailed"];

pub(super) fn emit_run_host_action_stats(
    stats: &mut StatsRegistry,
    summary: &Rem6HostActionSummary,
) -> Result<(), Rem6CliError> {
    #[derive(Default)]
    struct HostCheckpointComponentStats {
        components: u64,
        chunks: u64,
        payload_bytes: u64,
    }

    #[derive(Default)]
    struct HostCheckpointChunkStats {
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
    let mut execution_mode_authority_targets_seen = BTreeSet::<String>::new();
    let mut execution_mode_authority_target_modes = BTreeMap::<(String, &'static str), u64>::new();
    for authority in &summary.execution_modes {
        let target = stat_path_segment(&authority.target);
        execution_mode_authority_targets_seen.insert(target.clone());
        *execution_mode_authority_modes
            .entry(authority.mode)
            .or_default() += 1;
        *execution_mode_authority_target_modes
            .entry((target, authority.mode))
            .or_default() += 1;
    }

    let mut checkpoint_restore_execution_mode_authority_manifests = 0;
    let mut checkpoint_restore_execution_mode_authority_cleared_manifests = 0;
    let mut checkpoint_restore_execution_mode_authority_targets = 0;
    let mut checkpoint_restore_execution_mode_authority_modes =
        BTreeMap::<&'static str, u64>::new();
    let mut checkpoint_restore_execution_mode_authority_targets_seen = BTreeSet::<String>::new();
    let mut checkpoint_restore_execution_mode_authority_target_modes =
        BTreeMap::<(String, &'static str), u64>::new();
    let mut checkpoint_restore_component_stats =
        BTreeMap::<String, HostCheckpointComponentStats>::new();
    let mut checkpoint_restore_chunk_stats =
        BTreeMap::<(String, String), HostCheckpointChunkStats>::new();
    let mut checkpoint_restore_target_stats =
        BTreeMap::<String, HostCheckpointComponentStats>::new();
    let mut checkpoint_restore_target_component_stats =
        BTreeMap::<(String, String), HostCheckpointComponentStats>::new();
    let mut checkpoint_restore_target_chunk_stats =
        BTreeMap::<(String, String, String), HostCheckpointChunkStats>::new();
    for restore in &summary.checkpoint_restores {
        if restore.execution_mode_authority_present {
            checkpoint_restore_execution_mode_authority_manifests += 1;
        }
        if restore.execution_mode_authority_cleared {
            checkpoint_restore_execution_mode_authority_cleared_manifests += 1;
        }
        checkpoint_restore_execution_mode_authority_targets += restore.execution_modes.len() as u64;
        for authority in &restore.execution_modes {
            let target = stat_path_segment(&authority.target);
            checkpoint_restore_execution_mode_authority_targets_seen.insert(target.clone());
            *checkpoint_restore_execution_mode_authority_modes
                .entry(authority.mode)
                .or_default() += 1;
            *checkpoint_restore_execution_mode_authority_target_modes
                .entry((target, authority.mode))
                .or_default() += 1;
        }
        let restore_target_paths = restore
            .execution_modes
            .iter()
            .map(|authority| stat_path_segment(&authority.target))
            .collect::<BTreeSet<_>>();
        for component in &restore.components {
            let component_path = stat_path_segment(&component.component);
            let component_stats = checkpoint_restore_component_stats
                .entry(component_path.clone())
                .or_default();
            component_stats.components += 1;
            component_stats.chunks += component.chunk_count;
            component_stats.payload_bytes += component.payload_bytes;
            if restore_target_paths.contains(&component_path) {
                let target_stats = checkpoint_restore_target_stats
                    .entry(component_path.clone())
                    .or_default();
                target_stats.components += 1;
                target_stats.chunks += component.chunk_count;
                target_stats.payload_bytes += component.payload_bytes;
                let target_component_stats = checkpoint_restore_target_component_stats
                    .entry((component_path.clone(), component_path.clone()))
                    .or_default();
                target_component_stats.components += 1;
                target_component_stats.chunks += component.chunk_count;
                target_component_stats.payload_bytes += component.payload_bytes;
            }
            for chunk in &component.chunks {
                let chunk_path = stat_path_segment(&chunk.name);
                let chunk_stats = checkpoint_restore_chunk_stats
                    .entry((component_path.clone(), chunk_path.clone()))
                    .or_default();
                chunk_stats.chunks += 1;
                chunk_stats.payload_bytes += chunk.payload_bytes;
                chunk_stats.payload_checksum_accumulator = chunk_stats
                    .payload_checksum_accumulator
                    .wrapping_add(chunk.payload_checksum);
                if restore_target_paths.contains(&component_path) {
                    let target_chunk_stats = checkpoint_restore_target_chunk_stats
                        .entry((component_path.clone(), component_path.clone(), chunk_path))
                        .or_default();
                    target_chunk_stats.chunks += 1;
                    target_chunk_stats.payload_bytes += chunk.payload_bytes;
                    target_chunk_stats.payload_checksum_accumulator = target_chunk_stats
                        .payload_checksum_accumulator
                        .wrapping_add(chunk.payload_checksum);
                }
            }
        }
    }

    let mut switch_modes = BTreeMap::<&'static str, u64>::new();
    let mut switch_targets_seen = BTreeSet::<String>::new();
    let mut switch_target_modes = BTreeMap::<(String, &'static str), u64>::new();
    let mut switch_previous_modes = BTreeMap::<&'static str, u64>::new();
    let mut switch_previous_target_modes = BTreeMap::<(String, &'static str), u64>::new();
    let mut switch_previous_mode_none = 0;
    let mut switch_previous_target_mode_none = BTreeMap::<String, u64>::new();
    for switch in &summary.execution_mode_switches {
        let target = stat_path_segment(&switch.target);
        switch_targets_seen.insert(target.clone());
        *switch_modes.entry(switch.mode).or_default() += 1;
        *switch_target_modes
            .entry((target.clone(), switch.mode))
            .or_default() += 1;
        if let Some(previous_mode) = switch.previous_mode {
            *switch_previous_modes.entry(previous_mode).or_default() += 1;
            *switch_previous_target_modes
                .entry((target, previous_mode))
                .or_default() += 1;
        } else {
            switch_previous_mode_none += 1;
            *switch_previous_target_mode_none.entry(target).or_default() += 1;
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
    let mut switch_quiescence_target_captured_stats =
        BTreeMap::<String, HostCheckpointComponentStats>::new();
    let mut switch_quiescence_target_checkers =
        BTreeMap::<String, Rem6ExecutionModeSwitchCheckerSummary>::new();
    let mut switch_state_transfer_component_stats =
        BTreeMap::<String, HostCheckpointComponentStats>::new();
    let mut switch_state_transfer_chunk_stats =
        BTreeMap::<(String, String), HostCheckpointChunkStats>::new();
    let mut switch_state_transfer_target_stats =
        BTreeMap::<String, HostCheckpointComponentStats>::new();
    let mut switch_state_transfer_target_component_stats =
        BTreeMap::<(String, String), HostCheckpointComponentStats>::new();
    let mut switch_state_transfer_target_chunk_stats =
        BTreeMap::<(String, String, String), HostCheckpointChunkStats>::new();
    for switch in &summary.execution_mode_switches {
        let Some(transfer) = switch.state_transfer.as_ref() else {
            continue;
        };
        let target_path = stat_path_segment(&switch.target);
        switch_state_transfer_count += 1;
        switch_state_transfer_components += transfer.component_count;
        switch_state_transfer_chunks += transfer.chunk_count;
        switch_state_transfer_payload_bytes += transfer.payload_bytes;
        let target_stats = switch_state_transfer_target_stats
            .entry(target_path.clone())
            .or_default();
        target_stats.components += transfer.component_count;
        target_stats.chunks += transfer.chunk_count;
        target_stats.payload_bytes += transfer.payload_bytes;
        let quiescence_target_path = stat_path_segment(&transfer.quiescence_gate.target);
        if transfer.quiescence_gate.validated {
            switch_quiescence_validated += 1;
            *switch_quiescence_target_validated
                .entry(quiescence_target_path.clone())
                .or_default() += 1;
        }
        switch_quiescence_captured_components += transfer.quiescence_gate.captured_component_count;
        switch_quiescence_captured_chunks += transfer.quiescence_gate.captured_chunk_count;
        switch_quiescence_captured_payload_bytes += transfer.quiescence_gate.captured_payload_bytes;
        if transfer.quiescence_gate.captured_component_count > 0
            || transfer.quiescence_gate.captured_chunk_count > 0
            || transfer.quiescence_gate.captured_payload_bytes > 0
        {
            let quiescence_target_stats = switch_quiescence_target_captured_stats
                .entry(quiescence_target_path.clone())
                .or_default();
            quiescence_target_stats.components += transfer.quiescence_gate.captured_component_count;
            quiescence_target_stats.chunks += transfer.quiescence_gate.captured_chunk_count;
            quiescence_target_stats.payload_bytes +=
                transfer.quiescence_gate.captured_payload_bytes;
        }
        if let Some(checker) = transfer.quiescence_gate.checker {
            switch_quiescence_checker = Some(checker);
            switch_quiescence_target_checkers.insert(quiescence_target_path, checker);
        }
        for component in &transfer.components {
            let component_path = stat_path_segment(&component.component);
            let component_stats = switch_state_transfer_component_stats
                .entry(component_path.clone())
                .or_default();
            component_stats.components += 1;
            component_stats.chunks += component.chunk_count;
            component_stats.payload_bytes += component.payload_bytes;

            let target_component_stats = switch_state_transfer_target_component_stats
                .entry((target_path.clone(), component_path.clone()))
                .or_default();
            target_component_stats.components += 1;
            target_component_stats.chunks += component.chunk_count;
            target_component_stats.payload_bytes += component.payload_bytes;

            for chunk in &component.chunks {
                let chunk_path = stat_path_segment(&chunk.name);
                let chunk_stats = switch_state_transfer_chunk_stats
                    .entry((component_path.clone(), chunk_path.clone()))
                    .or_default();
                chunk_stats.chunks += 1;
                chunk_stats.payload_bytes += chunk.payload_bytes;
                chunk_stats.payload_checksum_accumulator = chunk_stats
                    .payload_checksum_accumulator
                    .wrapping_add(chunk.payload_checksum);
                let target_chunk_stats = switch_state_transfer_target_chunk_stats
                    .entry((target_path.clone(), component_path.clone(), chunk_path))
                    .or_default();
                target_chunk_stats.chunks += 1;
                target_chunk_stats.payload_bytes += chunk.payload_bytes;
                target_chunk_stats.payload_checksum_accumulator = target_chunk_stats
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
    if let Some(reset) = summary.stats_resets.last() {
        for (name, unit, value) in [
            ("latest_id", "Count", reset.id),
            ("latest_tick", "Tick", reset.tick),
            ("latest_epoch", "Count", reset.epoch),
        ] {
            increment_stat(
                stats,
                &format!("sim.host_actions.stats_reset.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    if let Some(dump) = summary.stats_dumps.last() {
        for (name, unit, value) in [
            ("latest_id", "Count", dump.id),
            ("latest_tick", "Tick", dump.tick),
            ("latest_epoch", "Count", dump.epoch),
            ("latest_reset_tick", "Tick", dump.reset_tick),
            ("latest_sample_count", "Count", dump.samples.len() as u64),
        ] {
            increment_stat(
                stats,
                &format!("sim.host_actions.stats_dump.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    if let Some(checkpoint) = summary.checkpoints.last() {
        for (name, unit, value) in [
            ("latest_tick", "Tick", checkpoint.tick),
            ("latest_manifest_tick", "Tick", checkpoint.manifest_tick),
            (
                "latest_component_count",
                "Count",
                checkpoint.component_count,
            ),
            ("latest_chunk_count", "Count", checkpoint.chunk_count),
            ("latest_payload_bytes", "Byte", checkpoint.payload_bytes),
        ] {
            increment_stat(
                stats,
                &format!("sim.host_actions.checkpoint.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    if let Some(restore) = summary.checkpoint_restores.last() {
        for (name, unit, value) in [
            ("latest_tick", "Tick", restore.tick),
            ("latest_manifest_tick", "Tick", restore.manifest_tick),
            ("latest_component_count", "Count", restore.component_count),
            ("latest_chunk_count", "Count", restore.chunk_count),
            ("latest_payload_bytes", "Byte", restore.payload_bytes),
        ] {
            increment_stat(
                stats,
                &format!("sim.host_actions.checkpoint_restore.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    if let Some(switch) = summary.execution_mode_switches.last() {
        for (name, unit, value) in [
            ("latest_tick", "Tick", switch.tick),
            ("latest_event", "Count", switch.event),
            ("latest_source", "Count", u64::from(switch.source)),
            ("latest_stats_epoch", "Count", switch.stats_epoch),
            ("latest_stats_reset_tick", "Tick", switch.stats_reset_tick),
        ] {
            increment_stat(
                stats,
                &format!("sim.host_actions.execution_mode_switch.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_stat(
                stats,
                &format!("sim.host_actions.execution_mode_switch.latest_mode.{mode}"),
                "Count",
                StatResetPolicy::Monotonic,
                if switch.mode == mode { 1 } else { 0 },
            )?;
            increment_stat(
                stats,
                &format!("sim.host_actions.execution_mode_switch.latest_previous_mode.{mode}"),
                "Count",
                StatResetPolicy::Monotonic,
                if switch.previous_mode == Some(mode) {
                    1
                } else {
                    0
                },
            )?;
        }
        if !EXECUTION_MODE_STAT_LANES.contains(&switch.mode) {
            increment_stat(
                stats,
                &format!(
                    "sim.host_actions.execution_mode_switch.latest_mode.{}",
                    switch.mode
                ),
                "Count",
                StatResetPolicy::Monotonic,
                1,
            )?;
        }
        match switch.previous_mode {
            Some(previous_mode) => {
                if !EXECUTION_MODE_STAT_LANES.contains(&previous_mode) {
                    increment_stat(
                        stats,
                        &format!(
                            "sim.host_actions.execution_mode_switch.latest_previous_mode.{previous_mode}"
                        ),
                        "Count",
                        StatResetPolicy::Monotonic,
                        1,
                    )?;
                }
                increment_stat(
                    stats,
                    "sim.host_actions.execution_mode_switch.latest_previous_mode.none",
                    "Count",
                    StatResetPolicy::Monotonic,
                    0,
                )?;
            }
            None => {
                increment_stat(
                    stats,
                    "sim.host_actions.execution_mode_switch.latest_previous_mode.none",
                    "Count",
                    StatResetPolicy::Monotonic,
                    1,
                )?;
            }
        }
        let target = stat_path_segment(&switch.target);
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch.latest_target.{target}.mode.{}",
                switch.mode
            ),
            "Count",
            StatResetPolicy::Monotonic,
            1,
        )?;
    }
    if let Some((_, transfer)) = summary
        .execution_mode_switches
        .iter()
        .rev()
        .find_map(|switch| {
            switch
                .state_transfer
                .as_ref()
                .map(|transfer| (switch, transfer))
        })
    {
        for (name, unit, value) in [
            ("latest_manifest_tick", "Tick", transfer.manifest_tick),
            ("latest_component_count", "Count", transfer.component_count),
            ("latest_chunk_count", "Count", transfer.chunk_count),
            ("latest_payload_bytes", "Byte", transfer.payload_bytes),
            (
                "latest_quiescence_captured_components",
                "Count",
                transfer.quiescence_gate.captured_component_count,
            ),
            (
                "latest_quiescence_captured_chunks",
                "Count",
                transfer.quiescence_gate.captured_chunk_count,
            ),
            (
                "latest_quiescence_captured_payload_bytes",
                "Byte",
                transfer.quiescence_gate.captured_payload_bytes,
            ),
        ] {
            increment_stat(
                stats,
                &format!("sim.host_actions.execution_mode_switch_state_transfer.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        increment_stat(
            stats,
            "sim.host_actions.execution_mode_switch_state_transfer.latest_quiescence_validated",
            "Count",
            StatResetPolicy::Monotonic,
            if transfer.quiescence_gate.validated {
                1
            } else {
                0
            },
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
    for target in &execution_mode_authority_targets_seen {
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_stat(
                stats,
                &format!("sim.host_actions.execution_mode_authority.target.{target}.mode.{mode}"),
                "Count",
                StatResetPolicy::Monotonic,
                execution_mode_authority_target_modes
                    .get(&(target.clone(), mode))
                    .copied()
                    .unwrap_or_default(),
            )?;
        }
    }
    for ((target, mode), count) in execution_mode_authority_target_modes {
        if EXECUTION_MODE_STAT_LANES.contains(&mode) {
            continue;
        }
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_authority.target.{target}.mode.{mode}"),
            "Count",
            StatResetPolicy::Monotonic,
            count,
        )?;
    }
    for target in checkpoint_restore_execution_mode_authority_targets_seen {
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_stat(
                stats,
                &format!(
                    "sim.host_actions.checkpoint_restore.execution_mode_authority.target.{target}.mode.{mode}"
                ),
                "Count",
                StatResetPolicy::Monotonic,
                checkpoint_restore_execution_mode_authority_target_modes
                    .get(&(target.clone(), mode))
                    .copied()
                    .unwrap_or_default(),
            )?;
        }
    }
    for target in &switch_targets_seen {
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_stat(
                stats,
                &format!("sim.host_actions.execution_mode_switch.target.{target}.mode.{mode}"),
                "Count",
                StatResetPolicy::Monotonic,
                switch_target_modes
                    .get(&(target.clone(), mode))
                    .copied()
                    .unwrap_or_default(),
            )?;
        }
    }
    for target in &switch_targets_seen {
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_switch.previous_mode.target.{target}.none"),
            "Count",
            StatResetPolicy::Monotonic,
            switch_previous_target_mode_none
                .get(target)
                .copied()
                .unwrap_or_default(),
        )?;
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_stat(
                stats,
                &format!(
                    "sim.host_actions.execution_mode_switch.previous_mode.target.{target}.{mode}"
                ),
                "Count",
                StatResetPolicy::Monotonic,
                switch_previous_target_modes
                    .get(&(target.clone(), mode))
                    .copied()
                    .unwrap_or_default(),
            )?;
        }
    }
    for ((target, mode), count) in switch_previous_target_modes {
        if EXECUTION_MODE_STAT_LANES.contains(&mode) {
            continue;
        }
        increment_stat(
            stats,
            &format!("sim.host_actions.execution_mode_switch.previous_mode.target.{target}.{mode}"),
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
    for (target, captured_stats) in switch_quiescence_target_captured_stats {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch.quiescence.target.{target}.captured_components"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            captured_stats.components,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch.quiescence.target.{target}.captured_chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            captured_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch.quiescence.target.{target}.captured_payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            captured_stats.payload_bytes,
        )?;
    }
    for (target, checker) in switch_quiescence_target_checkers {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch.quiescence.target.{target}.checker.checked_instructions"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            checker.checked_instructions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch.quiescence.target.{target}.checker.mismatches"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            checker.mismatches,
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
    for (component_path, component_stats) in checkpoint_restore_component_stats {
        increment_stat(
            stats,
            &format!("sim.host_actions.checkpoint_restore.component.{component_path}.components"),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.components,
        )?;
        increment_stat(
            stats,
            &format!("sim.host_actions.checkpoint_restore.component.{component_path}.chunks"),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.component.{component_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            component_stats.payload_bytes,
        )?;
    }
    for ((component_path, chunk_path), chunk_stats) in checkpoint_restore_chunk_stats {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.component.{component_path}.chunk.{chunk_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            chunk_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.component.{component_path}.chunk.{chunk_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_bytes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.component.{component_path}.chunk.{chunk_path}.payload_checksum_accumulator"
            ),
            "Unspecified",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_checksum_accumulator,
        )?;
    }
    for (target_path, target_stats) in checkpoint_restore_target_stats {
        increment_stat(
            stats,
            &format!("sim.host_actions.checkpoint_restore.target.{target_path}.components"),
            "Count",
            StatResetPolicy::Monotonic,
            target_stats.components,
        )?;
        increment_stat(
            stats,
            &format!("sim.host_actions.checkpoint_restore.target.{target_path}.chunks"),
            "Count",
            StatResetPolicy::Monotonic,
            target_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!("sim.host_actions.checkpoint_restore.target.{target_path}.payload_bytes"),
            "Byte",
            StatResetPolicy::Monotonic,
            target_stats.payload_bytes,
        )?;
    }
    for ((target_path, component_path), component_stats) in
        checkpoint_restore_target_component_stats
    {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.target.{target_path}.component.{component_path}.components"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.components,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.target.{target_path}.component.{component_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.target.{target_path}.component.{component_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            component_stats.payload_bytes,
        )?;
    }
    for ((target_path, component_path, chunk_path), chunk_stats) in
        checkpoint_restore_target_chunk_stats
    {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.target.{target_path}.component.{component_path}.chunk.{chunk_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            chunk_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.target.{target_path}.component.{component_path}.chunk.{chunk_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_bytes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.checkpoint_restore.target.{target_path}.component.{component_path}.chunk.{chunk_path}.payload_checksum_accumulator"
            ),
            "Unspecified",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_checksum_accumulator,
        )?;
    }
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
    for (target_path, target_stats) in switch_state_transfer_target_stats {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.components"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            target_stats.components,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            target_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            target_stats.payload_bytes,
        )?;
    }
    for ((target_path, component_path), component_stats) in
        switch_state_transfer_target_component_stats
    {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.component.{component_path}.components"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.components,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.component.{component_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            component_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.component.{component_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            component_stats.payload_bytes,
        )?;
    }
    for ((target_path, component_path, chunk_path), chunk_stats) in
        switch_state_transfer_target_chunk_stats
    {
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.component.{component_path}.chunk.{chunk_path}.chunks"
            ),
            "Count",
            StatResetPolicy::Monotonic,
            chunk_stats.chunks,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.component.{component_path}.chunk.{chunk_path}.payload_bytes"
            ),
            "Byte",
            StatResetPolicy::Monotonic,
            chunk_stats.payload_bytes,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.host_actions.execution_mode_switch_state_transfer.target.{target_path}.component.{component_path}.chunk.{chunk_path}.payload_checksum_accumulator"
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
        Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary,
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
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu_0.components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu_0.chunk.pipe_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu_0.chunk.pipe_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.component.cpu_0.chunk.pipe_0.payload_checksum_accumulator",
            "Unspecified",
            StatResetPolicy::Monotonic,
            36,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch.target.cpu0.mode.timing",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.atomic",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.captured_components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.captured_chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.captured_payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
    }

    #[test]
    fn host_action_checkpoint_restore_stats_merge_normalized_path_collisions() {
        let mut stats = StatsRegistry::new();
        let summary = Rem6HostActionSummary {
            total_action_count: 2,
            checkpoint_restored_count: 2,
            checkpoint_restored_component_count: 2,
            checkpoint_restored_chunk_count: 2,
            checkpoint_restored_payload_bytes: 24,
            checkpoint_restores: vec![
                restore_with_component_chunk("cpu-0", "pipe-0", 11, u64::MAX),
                restore_with_component_chunk("cpu_0", "pipe_0", 13, 2),
            ],
            ..Rem6HostActionSummary::default()
        };

        emit_run_host_action_stats(&mut stats, &summary).unwrap();
        let snapshot = stats.snapshot(0);

        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.component.cpu_0.components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.component.cpu_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.component.cpu_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.component.cpu_0.chunk.pipe_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.component.cpu_0.chunk.pipe_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.component.cpu_0.chunk.pipe_0.payload_checksum_accumulator",
            "Unspecified",
            StatResetPolicy::Monotonic,
            1,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.component.cpu_0.components",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.component.cpu_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.component.cpu_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.component.cpu_0.chunk.pipe_0.chunks",
            "Count",
            StatResetPolicy::Monotonic,
            2,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.component.cpu_0.chunk.pipe_0.payload_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            24,
        );
        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.checkpoint_restore.target.cpu_0.component.cpu_0.chunk.pipe_0.payload_checksum_accumulator",
            "Unspecified",
            StatResetPolicy::Monotonic,
            1,
        );
    }

    #[test]
    fn host_action_quiescence_target_capture_stats_skip_uncaptured_transfers() {
        let mut stats = StatsRegistry::new();
        let summary = Rem6HostActionSummary {
            total_action_count: 1,
            execution_mode_switch_count: 1,
            execution_mode_switches: vec![switch_with_uncaptured_quiescence("cpu-0")],
            ..Rem6HostActionSummary::default()
        };

        emit_run_host_action_stats(&mut stats, &summary).unwrap();
        let snapshot = stats.snapshot(0);

        assert_snapshot_stat(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu_0.validated",
            "Count",
            StatResetPolicy::Monotonic,
            1,
        );
        assert_snapshot_stat_absent(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu_0.captured_components",
        );
        assert_snapshot_stat_absent(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu_0.captured_chunks",
        );
        assert_snapshot_stat_absent(
            &snapshot,
            "sim.host_actions.execution_mode_switch.quiescence.target.cpu_0.captured_payload_bytes",
        );
    }

    fn restore_with_component_chunk(
        component: &str,
        chunk: &str,
        payload_bytes: u64,
        payload_checksum: u64,
    ) -> Rem6HostCheckpointSummary {
        Rem6HostCheckpointSummary {
            tick: 0,
            event: 0,
            source: 0,
            label: format!("restore-{component}-{chunk}"),
            manifest_tick: 0,
            component_count: 1,
            chunk_count: 1,
            payload_bytes,
            execution_mode_authority_present: false,
            execution_mode_authority_cleared: false,
            execution_mode_authority_decode_error: false,
            execution_modes: vec![Rem6HostExecutionModeSummary {
                target: component.to_string(),
                mode: "detailed",
            }],
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
        }
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

    fn switch_with_uncaptured_quiescence(target: &str) -> Rem6HostExecutionModeSwitchSummary {
        Rem6HostExecutionModeSwitchSummary {
            tick: 0,
            event: 0,
            source: 0,
            target: target.to_string(),
            previous_mode: Some("atomic"),
            mode: "timing",
            stats_epoch: 0,
            stats_reset_tick: 0,
            state_transfer: Some(Rem6ExecutionModeStateTransferSummary {
                manifest_label: format!("switch-{target}-uncaptured"),
                manifest_tick: 0,
                component_count: 0,
                chunk_count: 0,
                payload_bytes: 0,
                quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary {
                    validated: true,
                    target: target.to_string(),
                    captured_component_count: 0,
                    captured_chunk_count: 0,
                    captured_payload_bytes: 0,
                    checker: None,
                },
                components: Vec::new(),
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

    fn assert_snapshot_stat_absent(snapshot: &StatSnapshot, path: &str) {
        let matches = snapshot
            .samples()
            .iter()
            .filter(|sample| sample.path() == path)
            .collect::<Vec<_>>();
        assert!(
            matches.is_empty(),
            "expected stat path {path} to be absent, found {}",
            matches.len()
        );
    }
}
