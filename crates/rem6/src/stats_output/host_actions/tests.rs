use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};
use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

use super::emit_run_host_action_stats;
use crate::{
    Rem6ExecutionModeQuiescenceGateSummary, Rem6ExecutionModeStateTransferSummary,
    Rem6HostActionSummary, Rem6HostCheckpointChunkSummary, Rem6HostCheckpointComponentSummary,
    Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary, Rem6HostExecutionModeSwitchSummary,
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
fn host_action_checkpoint_restore_stats_count_authority_decode_errors() {
    let mut stats = StatsRegistry::new();
    let mut decoded_restore = restore_with_component_chunk("cpu0", "o3-runtime-state", 32, 7);
    decoded_restore.execution_mode_authority_present = true;
    let mut failed_restore = restore_with_component_chunk("cpu0", "o3-runtime-state", 32, 11);
    failed_restore.execution_mode_authority_decode_error = true;
    failed_restore.execution_modes.clear();
    let summary = Rem6HostActionSummary {
        total_action_count: 2,
        checkpoint_restored_count: 2,
        checkpoint_restored_component_count: 2,
        checkpoint_restored_chunk_count: 2,
        checkpoint_restored_payload_bytes: 64,
        checkpoint_restores: vec![decoded_restore, failed_restore],
        ..Rem6HostActionSummary::default()
    };

    emit_run_host_action_stats(&mut stats, &summary).unwrap();
    let snapshot = stats.snapshot(0);

    assert_snapshot_stat(
        &snapshot,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.decode_errors",
        "Count",
        StatResetPolicy::Monotonic,
        1,
    );
}

#[test]
fn host_action_latest_transfer_stats_merge_normalized_path_collisions() {
    let mut stats = StatsRegistry::new();
    let summary = Rem6HostActionSummary {
        total_action_count: 1,
        execution_mode_switch_count: 1,
        execution_mode_switches: vec![switch_with_colliding_latest_transfer()],
        ..Rem6HostActionSummary::default()
    };

    emit_run_host_action_stats(&mut stats, &summary).unwrap();
    let snapshot = stats.snapshot(0);

    let target_prefix = "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu0";
    let component_prefix = format!("{target_prefix}.component.cpu_0");
    let chunk_prefix = format!("{component_prefix}.chunk.pipe_0");
    for (path, unit, value) in [
        (format!("{target_prefix}.components"), "Count", 2),
        (format!("{target_prefix}.chunks"), "Count", 2),
        (format!("{target_prefix}.payload_bytes"), "Byte", 24),
        (format!("{component_prefix}.components"), "Count", 2),
        (format!("{component_prefix}.chunks"), "Count", 2),
        (format!("{component_prefix}.payload_bytes"), "Byte", 24),
        (format!("{chunk_prefix}.chunks"), "Count", 2),
        (format!("{chunk_prefix}.payload_bytes"), "Byte", 24),
        (
            format!("{chunk_prefix}.payload_checksum_accumulator"),
            "Unspecified",
            36,
        ),
    ] {
        assert_snapshot_stat(&snapshot, &path, unit, StatResetPolicy::Monotonic, value);
    }
}

#[test]
fn host_action_live_data_handoff_stats_mark_non_restorable_transfer() {
    let mut stats = StatsRegistry::new();
    let mut switch =
        switch_with_transfer_component_chunk("cpu0", RISCV_O3_LIVE_DATA_HANDOFF_CHUNK, 73, 17);
    let transfer = switch.state_transfer.as_mut().unwrap();
    transfer.restorable = false;
    transfer.live_data_handoff = true;
    let summary = Rem6HostActionSummary {
        total_action_count: 1,
        execution_mode_switch_count: 1,
        execution_mode_switches: vec![switch],
        ..Rem6HostActionSummary::default()
    };

    emit_run_host_action_stats(&mut stats, &summary).unwrap();
    let snapshot = stats.snapshot(0);
    for (path, value) in [
        (
            "sim.host_actions.execution_mode_switch_state_transfer.restorable",
            0,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.non_restorable",
            1,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.live_data_handoffs",
            1,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_restorable",
            0,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_live_data_handoff",
            1,
        ),
    ] {
        assert_snapshot_stat(&snapshot, path, "Count", StatResetPolicy::Monotonic, value);
    }
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
                o3_runtime: None,
                o3_live_data_handoff: None,
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
            restorable: true,
            live_data_handoff: false,
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
                    o3_runtime: None,
                    o3_live_data_handoff: None,
                }],
            }],
        }),
    }
}

fn switch_with_colliding_latest_transfer() -> Rem6HostExecutionModeSwitchSummary {
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
            manifest_label: "switch-colliding-latest".to_string(),
            manifest_tick: 0,
            component_count: 2,
            chunk_count: 2,
            payload_bytes: 24,
            restorable: true,
            live_data_handoff: false,
            quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary {
                validated: true,
                target: "cpu0".to_string(),
                captured_component_count: 2,
                captured_chunk_count: 2,
                captured_payload_bytes: 24,
                checker: None,
            },
            components: [("cpu-0", "pipe-0", 11, 17), ("cpu_0", "pipe_0", 13, 19)]
                .into_iter()
                .map(|(component, chunk, payload_bytes, payload_checksum)| {
                    Rem6HostCheckpointComponentSummary {
                        component: component.to_string(),
                        chunk_count: 1,
                        payload_bytes,
                        chunks: vec![Rem6HostCheckpointChunkSummary {
                            name: chunk.to_string(),
                            payload_bytes,
                            payload_checksum,
                            o3_runtime: None,
                            o3_live_data_handoff: None,
                        }],
                    }
                })
                .collect(),
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
            restorable: true,
            live_data_handoff: false,
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
