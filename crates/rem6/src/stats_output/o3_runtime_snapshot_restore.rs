use std::collections::{BTreeMap, BTreeSet};

use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{increment_count_stat, increment_stat, stat_path_segment, EXECUTION_MODE_STAT_LANES};
use crate::{
    Rem6CliError, Rem6CoreSummary, Rem6HostCheckpointChunkSummary,
    Rem6HostO3RuntimeCheckpointStatValue,
};

pub(super) fn emit_o3_runtime_snapshot_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
) -> Result<(), Rem6CliError> {
    let snapshot = &core.o3_runtime_snapshot;
    for (name, value) in [
        ("snapshot.rob.count", snapshot.reorder_buffer().len() as u64),
        (
            "snapshot.lsq.count",
            snapshot.load_store_queue().len() as u64,
        ),
        (
            "snapshot.rename_map.count",
            snapshot.rename_map().len() as u64,
        ),
        (
            "snapshot.rob.entries",
            snapshot.reorder_buffer().len() as u64,
        ),
        (
            "snapshot.lsq.entries",
            snapshot.load_store_queue().len() as u64,
        ),
        (
            "snapshot.rename_map.entries",
            snapshot.rename_map().len() as u64,
        ),
    ] {
        increment_count_stat(stats, format!("sim.cpu{}.o3.{name}", core.cpu), value)?;
    }
    Ok(())
}

#[derive(Default)]
struct O3RestoreComponentStats {
    components: u64,
    chunks: u64,
    payload_bytes: u64,
}

#[derive(Default)]
struct O3RestoreChunkStats {
    chunks: u64,
    payload_bytes: u64,
    payload_checksum_accumulator: u64,
    o3_runtime_numeric: BTreeMap<String, Rem6HostO3RuntimeCheckpointStatValue>,
}

fn add_o3_restore_chunk_stats(
    stats: &mut O3RestoreChunkStats,
    chunk: &Rem6HostCheckpointChunkSummary,
) {
    stats.chunks = stats.chunks.saturating_add(1);
    stats.payload_bytes = stats.payload_bytes.saturating_add(chunk.payload_bytes);
    stats.payload_checksum_accumulator = stats
        .payload_checksum_accumulator
        .wrapping_add(chunk.payload_checksum);
    let Some(o3_runtime) = &chunk.o3_runtime else {
        return;
    };
    for (field, value) in o3_runtime.numeric_stat_fields() {
        stats
            .o3_runtime_numeric
            .entry(field.to_string())
            .and_modify(|current| current.merge_restore_value(value))
            .or_insert(value);
    }
}

fn emit_o3_restore_component_stat_set(
    stats: &mut StatsRegistry,
    prefix: &str,
    component_stats: &O3RestoreComponentStats,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("components", "Count", component_stats.components),
        ("chunks", "Count", component_stats.chunks),
        ("payload_bytes", "Byte", component_stats.payload_bytes),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_restore_chunk_stat_set(
    stats: &mut StatsRegistry,
    prefix: &str,
    chunk_stats: &O3RestoreChunkStats,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("chunks", "Count", chunk_stats.chunks),
        ("payload_bytes", "Byte", chunk_stats.payload_bytes),
        (
            "payload_checksum_accumulator",
            "Unspecified",
            chunk_stats.payload_checksum_accumulator,
        ),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for (field, value) in &chunk_stats.o3_runtime_numeric {
        increment_stat(
            stats,
            &format!("{prefix}.o3_runtime.{field}"),
            value.unit(),
            StatResetPolicy::Monotonic,
            value.value(),
        )?;
    }
    Ok(())
}

fn emit_o3_restore_target_stat_sets(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    lane: &str,
    target_stats: &BTreeMap<String, O3RestoreComponentStats>,
    target_component_stats: &BTreeMap<(String, String), O3RestoreComponentStats>,
    target_chunk_stats: &BTreeMap<(String, String, String), O3RestoreChunkStats>,
) -> Result<(), Rem6CliError> {
    for (target_path, target_stats) in target_stats {
        emit_o3_restore_component_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.{lane}.{target_path}",
                core.cpu
            ),
            target_stats,
        )?;
    }
    for ((target_path, component_path), component_stats) in target_component_stats {
        emit_o3_restore_component_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.{lane}.{target_path}.component.{component_path}",
                core.cpu
            ),
            component_stats,
        )?;
    }
    for ((target_path, component_path, chunk_path), chunk_stats) in target_chunk_stats {
        emit_o3_restore_chunk_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.{lane}.{target_path}.component.{component_path}.chunk.{chunk_path}",
                core.cpu
            ),
            chunk_stats,
        )?;
    }
    Ok(())
}

pub(super) fn emit_o3_runtime_checkpoint_restore_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
) -> Result<(), Rem6CliError> {
    let Some(restore) = &core.o3_runtime_checkpoint_restore else {
        return Ok(());
    };
    for (name, unit, value) in [
        ("count", "Count", 1),
        ("tick", "Tick", restore.tick),
        ("manifest_tick", "Tick", restore.manifest_tick),
        ("component_count", "Count", restore.component_count),
        ("chunk_count", "Count", restore.chunk_count),
        ("payload_bytes", "Byte", restore.payload_bytes),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.checkpoint_restore.{name}", core.cpu),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }

    let authority_prefix = format!(
        "sim.cpu{}.o3.checkpoint_restore.execution_mode_authority",
        core.cpu
    );
    for (name, value) in [
        (
            "manifests",
            if restore.execution_mode_authority_present {
                1
            } else {
                0
            },
        ),
        (
            "cleared_manifests",
            if restore.execution_mode_authority_cleared {
                1
            } else {
                0
            },
        ),
        (
            "decode_errors",
            if restore.execution_mode_authority_decode_error {
                1
            } else {
                0
            },
        ),
        ("targets", restore.execution_modes.len() as u64),
    ] {
        increment_count_stat(stats, format!("{authority_prefix}.{name}"), value)?;
    }

    let mut authority_modes = BTreeMap::<&'static str, u64>::new();
    let mut authority_targets_seen = BTreeSet::<String>::new();
    let mut authority_target_modes = BTreeMap::<(String, &'static str), u64>::new();
    for authority in &restore.execution_modes {
        let target = stat_path_segment(&authority.target);
        authority_targets_seen.insert(target.clone());
        *authority_modes.entry(authority.mode).or_default() += 1;
        *authority_target_modes
            .entry((target, authority.mode))
            .or_default() += 1;
    }
    for mode in EXECUTION_MODE_STAT_LANES {
        increment_count_stat(
            stats,
            format!("{authority_prefix}.mode.{mode}"),
            authority_modes.get(mode).copied().unwrap_or_default(),
        )?;
    }
    for target in authority_targets_seen {
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_count_stat(
                stats,
                format!("{authority_prefix}.target.{target}.mode.{mode}"),
                authority_target_modes
                    .get(&(target.clone(), mode))
                    .copied()
                    .unwrap_or_default(),
            )?;
        }
    }
    for ((target, mode), count) in authority_target_modes {
        if EXECUTION_MODE_STAT_LANES.contains(&mode) {
            continue;
        }
        increment_count_stat(
            stats,
            format!("{authority_prefix}.target.{target}.mode.{mode}"),
            count,
        )?;
    }

    let restore_targets = restore
        .execution_modes
        .iter()
        .map(|mode| stat_path_segment(&mode.target))
        .collect::<Vec<_>>();
    let mut component_stats = BTreeMap::<String, O3RestoreComponentStats>::new();
    let mut chunk_stats = BTreeMap::<(String, String), O3RestoreChunkStats>::new();
    let mut target_stats = BTreeMap::<String, O3RestoreComponentStats>::new();
    let mut target_component_stats = BTreeMap::<(String, String), O3RestoreComponentStats>::new();
    let mut target_chunk_stats = BTreeMap::<(String, String, String), O3RestoreChunkStats>::new();
    for component in &restore.components {
        let component_path = stat_path_segment(&component.component);
        let component_entry = component_stats.entry(component_path.clone()).or_default();
        component_entry.components = component_entry.components.saturating_add(1);
        component_entry.chunks = component_entry.chunks.saturating_add(component.chunk_count);
        component_entry.payload_bytes = component_entry
            .payload_bytes
            .saturating_add(component.payload_bytes);
        let is_target_component = restore_targets
            .iter()
            .any(|target| target.as_str() == component_path.as_str());
        if is_target_component {
            let target_entry = target_stats.entry(component_path.clone()).or_default();
            target_entry.components = target_entry.components.saturating_add(1);
            target_entry.chunks = target_entry.chunks.saturating_add(component.chunk_count);
            target_entry.payload_bytes = target_entry
                .payload_bytes
                .saturating_add(component.payload_bytes);
            let target_component_entry = target_component_stats
                .entry((component_path.clone(), component_path.clone()))
                .or_default();
            target_component_entry.components = target_component_entry.components.saturating_add(1);
            target_component_entry.chunks = target_component_entry
                .chunks
                .saturating_add(component.chunk_count);
            target_component_entry.payload_bytes = target_component_entry
                .payload_bytes
                .saturating_add(component.payload_bytes);
        }
        for chunk in &component.chunks {
            let chunk_path = stat_path_segment(&chunk.name);
            let chunk_entry = chunk_stats
                .entry((component_path.clone(), chunk_path.clone()))
                .or_default();
            add_o3_restore_chunk_stats(chunk_entry, chunk);
            if is_target_component {
                let target_chunk_entry = target_chunk_stats
                    .entry((component_path.clone(), component_path.clone(), chunk_path))
                    .or_default();
                add_o3_restore_chunk_stats(target_chunk_entry, chunk);
            }
        }
    }
    for (component_path, component_stats) in component_stats {
        emit_o3_restore_component_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.component.{component_path}",
                core.cpu
            ),
            &component_stats,
        )?;
    }
    for ((component_path, chunk_path), chunk_stats) in chunk_stats {
        emit_o3_restore_chunk_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.component.{component_path}.chunk.{chunk_path}",
                core.cpu
            ),
            &chunk_stats,
        )?;
    }
    emit_o3_restore_target_stat_sets(
        stats,
        core,
        "target",
        &target_stats,
        &target_component_stats,
        &target_chunk_stats,
    )?;
    emit_o3_restore_target_stat_sets(
        stats,
        core,
        "latest_target",
        &target_stats,
        &target_component_stats,
        &target_chunk_stats,
    )?;
    Ok(())
}
