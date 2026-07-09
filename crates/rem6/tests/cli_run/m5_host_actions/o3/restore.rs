use super::*;

#[path = "restore/lsq_data_response.rs"]
mod lsq_data_response;

#[test]
fn rem6_run_m5_dump_stats_restores_multicore_o3_branch_ftq_snapshot_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_indirect_call_ftq_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-indirect-call-ftq-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
            "--dump-memory",
            "0x80000400:16",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("30000080000000001800008000000000"),
        "restored indirect-call O3 run should replay the checkpointed target and link witnesses"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let restored_modes = host_actions
        .pointer("/checkpoint_restores/0/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {host_actions}"));
    assert!(
        restored_modes.iter().any(|mode| {
            mode.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && mode.pointer("/mode").and_then(Value::as_str) == Some("detailed")
        }),
        "restored checkpoint should preserve cpu1 detailed-mode authority: {restored_modes:?}"
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );

    for dump in [first_dump, restored_dump] {
        assert_eq!(
            dump.pointer("/epoch").and_then(Value::as_u64),
            Some(0),
            "restored dump should preserve checkpoint-era stats epoch: {dump}"
        );
        for (path, value) in [
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.branches",
                2,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_writes",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.without_link_writes",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.mispredictions",
                2,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.predicted_taken",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.predicted_target_mismatches",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashes",
                2,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_targets",
                2,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.kind.call_indirect",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.kind.direct_unconditional",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_write_kind.call_indirect",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.without_link_write_kind.direct_unconditional",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.misprediction_kind.call_indirect",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.misprediction_kind.direct_unconditional",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.squash_kind.call_indirect",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.squash_kind.direct_unconditional",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_link_write_kind.call_indirect",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
                1,
            ),
            ("system.cpu1.ftq.squashes_0::CallIndirect", 1),
            ("system.cpu1.ftq.squashes_0::DirectUncond", 1),
            ("system.cpu1.ftq.squashes_0::total", 2),
            ("system.cpu1.ftq.squashedTargets_0::CallIndirect", 1),
            ("system.cpu1.ftq.squashedTargets_0::DirectUncond", 1),
            ("system.cpu1.ftq.squashedTargets_0::total", 2),
            (
                "system.cpu1.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
                1,
            ),
            (
                "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
                1,
            ),
            ("system.cpu1.fetch.predictedBranches", 1),
            ("system.cpu1.bac.branchMisspredict", 2),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_indirect",
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_taken",
            "system.cpu.ftq.squashes_0::CallIndirect",
            "system.cpu0.ftq.squashes_0::CallIndirect",
            "system.cpu.ftq.squashedTargets_0::CallIndirect",
            "system.cpu0.ftq.squashedTargets_0::CallIndirect",
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
            "system.cpu0.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
            "system.cpu0.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
            "system.cpu.fetch.predictedBranches",
            "system.cpu0.fetch.predictedBranches",
            "system.cpu.bac.branchMisspredict",
            "system.cpu0.bac.branchMisspredict",
        ] {
            assert_stats_dump_sample_absent(dump, path);
            assert_json_stat_absent(&json, path);
        }
    }

    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.branches",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.call_indirect",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.direct_unconditional",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.call_direct",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.ftq.squashes_0::CallDirect",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.branch_event.kind.call_indirect");
}

#[test]
fn rem6_run_host_restore_scopes_sparse_three_core_o3_trace_authority() {
    let path = sparse_three_core_detailed_o3_restore_trace_binary(
        "m5-switch-cpu-sparse-three-core-o3-restore-trace",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "700",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "3",
            "--parallel-workers",
            "3",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-checkpoint",
            "18:sparse-o3",
            "--host-restore-checkpoint",
            "70:sparse-o3",
            "--dump-memory",
            "0x80000400:16",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a000000000000005c00000000000000"),
        "sparse restore should replay detailed CPU0/CPU2 store/load work into distinct per-hart slots without CPU1 O3 activity"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoints/0/label")
            .and_then(Value::as_str),
        Some("sparse-o3")
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let restore = host_actions
        .pointer("/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing checkpoint restore detail: {host_actions}"));
    assert_eq!(
        restore.pointer("/label").and_then(Value::as_str),
        Some("sparse-o3")
    );
    let restore_tick = restore
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restore tick: {restore}"));
    let restore_manifest_tick = restore
        .pointer("/manifest_tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restore manifest tick: {restore}"));
    let restored_modes = restore
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {restore}"));
    let mut restored_targets = restored_modes
        .iter()
        .map(|mode| {
            (
                mode.pointer("/target")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                mode.pointer("/mode")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    restored_targets.sort_unstable();
    assert_eq!(
        restored_targets,
        [("cpu0", "detailed"), ("cpu2", "detailed")],
        "restore authority should preserve the sparse detailed CPU set: {restored_modes:?}"
    );
    for cpu in [0, 2] {
        let core_o3 = json
            .pointer(&format!("/cores/{cpu}/o3_runtime"))
            .unwrap_or_else(|| panic!("missing CPU{cpu} top-level O3 runtime summary: {json}"));
        assert_eq!(
            core_o3.pointer("/execution_mode").and_then(Value::as_str),
            Some("detailed"),
            "CPU{cpu} top-level O3 runtime should preserve restored detailed authority: {core_o3}"
        );
        let core_restore = core_o3
            .pointer("/checkpoint_restore")
            .unwrap_or_else(|| panic!("missing CPU{cpu} top-level O3 restore scope: {core_o3}"));
        assert_eq!(
            core_restore, restore,
            "CPU{cpu} top-level O3 restore scope should mirror the restored host manifest: core {core_restore}; host {restore}"
        );
        assert_trace_restore_component_chunk(
            core_restore,
            &format!("cpu{cpu}"),
            "o3-runtime-state",
        );
    }
    assert!(
        json.pointer("/cores/1/o3_runtime").is_none(),
        "CPU1 should remain suppressed from sparse top-level O3 runtime: {json}"
    );

    let o3_trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 trace records: {json}"));
    let mut traced_cpus = o3_trace
        .iter()
        .map(|record| record.pointer("/cpu").and_then(Value::as_u64).unwrap_or(99))
        .collect::<Vec<_>>();
    traced_cpus.sort_unstable();
    assert_eq!(
        traced_cpus,
        [0, 2],
        "only sparse detailed CPUs should emit O3 restore traces: {o3_trace:?}"
    );
    for cpu in [0, 2] {
        let expected_address = match cpu {
            0 => "0x80000400",
            2 => "0x80000408",
            _ => unreachable!(),
        };
        let record = o3_trace
            .iter()
            .find(|record| record.pointer("/cpu").and_then(Value::as_u64) == Some(cpu))
            .unwrap_or_else(|| panic!("missing CPU{cpu} O3 trace record: {o3_trace:?}"));
        assert_eq!(
            record.pointer("/target").and_then(Value::as_str),
            Some(format!("cpu{cpu}").as_str())
        );
        assert_eq!(
            record
                .pointer("/checkpoint_restore_count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            record
                .pointer("/checkpoint_restore_label")
                .and_then(Value::as_str),
            Some("sparse-o3")
        );
        assert_eq!(
            record
                .pointer("/checkpoint_restore/latest_tick")
                .and_then(Value::as_u64),
            Some(restore_tick)
        );
        let restore_scope = record
            .pointer("/checkpoint_restore")
            .unwrap_or_else(|| panic!("missing CPU{cpu} checkpoint restore scope: {record}"));
        assert_eq!(
            restore_scope.pointer("/components"),
            restore.pointer("/components"),
            "CPU{cpu} O3 restore scope should preserve component payload metadata without per-record mutation: {restore_scope}"
        );
        let component = format!("cpu{cpu}");
        assert_trace_restore_component_chunk(restore_scope, &component, "o3-runtime-state");
        for target in ["cpu0", "cpu2"] {
            assert_eq!(
                record
                    .pointer(&format!(
                        "/checkpoint_restore/execution_mode_authority/target/{target}/mode/detailed"
                    ))
                    .and_then(Value::as_u64),
                Some(1),
                "CPU{cpu} restore trace should carry sparse authority for {target}: {record}"
            );
        }
        assert_eq!(
            record
                .pointer("/checkpoint_restore/execution_mode_authority/target/cpu1/mode/detailed")
                .and_then(Value::as_u64),
            None,
            "CPU1 should not appear in sparse detailed authority: {record}"
        );
        let events = record
            .pointer("/events")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("CPU{cpu} O3 trace should include replay events: {record}"));
        assert!(
            !events.is_empty()
                && events.iter().all(
                    |event| event.pointer("/tick").and_then(Value::as_u64) > Some(restore_tick)
                ),
            "CPU{cpu} O3 trace should contain only post-restore replay events: {events:?}"
        );
        for event in events {
            if event.pointer("/lsq_operation").and_then(Value::as_str) == Some("store") {
                assert_eq!(
                    event.pointer("/lsq_store_address").and_then(Value::as_str),
                    Some(expected_address),
                    "CPU{cpu} store replay should use only its per-hart slot: {events:?}"
                );
            }
            if event.pointer("/lsq_operation").and_then(Value::as_str) == Some("load") {
                assert_eq!(
                    event.pointer("/lsq_load_address").and_then(Value::as_str),
                    Some(expected_address),
                    "CPU{cpu} load replay should use only its per-hart slot: {events:?}"
                );
            }
        }
        assert!(
            events.iter().any(|event| {
                event.pointer("/lsq_operation").and_then(Value::as_str) == Some("store")
                    && event.pointer("/lsq_store_address").and_then(Value::as_str)
                        == Some(expected_address)
            }),
            "CPU{cpu} restored replay should include the store event: {events:?}"
        );
        assert!(
            events.iter().any(|event| {
                event.pointer("/lsq_operation").and_then(Value::as_str) == Some("load")
                    && event.pointer("/lsq_load_address").and_then(Value::as_str)
                        == Some(expected_address)
            }),
            "CPU{cpu} restored replay should include the load event: {events:?}"
        );
    }
    for target in ["cpu0", "cpu2"] {
        assert_restore_component_chunk_stat(
            &json,
            restore,
            "sim.debug.o3_trace.checkpoint_restore",
            target,
            "o3-runtime-state",
            target,
            "o3_runtime_state",
            target,
        );
        assert_restore_o3_runtime_chunk_stats(
            &json,
            restore,
            "sim.debug.o3_trace.checkpoint_restore",
            target,
            "o3-runtime-state",
        );
        assert_restore_component_chunk_stat(
            &json,
            restore,
            &format!("sim.debug.o3_trace.cpu.{target}.checkpoint_restore"),
            target,
            "o3-runtime-state",
            target,
            "o3_runtime_state",
            target,
        );
    }

    assert_json_stat(&json, "sim.debug.o3_trace.records", "Count", 2, "monotonic");
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.checkpoint_restore_records",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.checkpoint_restore_manifest_tick",
        "Tick",
        restore_manifest_tick,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.targets",
        "Count",
        2,
        "monotonic",
    );
    for target in ["cpu0", "cpu2"] {
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.{target}.mode.detailed"
            ),
            "Count",
            1,
            "monotonic",
        );
        for cpu in ["cpu0", "cpu2"] {
            assert_json_stat(
                &json,
                &format!(
                    "sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore.execution_mode_authority.target.{target}.mode.detailed"
                ),
                "Count",
                1,
                "monotonic",
            );
        }
    }
    for cpu in ["cpu0", "cpu2"] {
        assert_json_stat(
            &json,
            &format!("sim.debug.o3_trace.cpu.{cpu}.checkpoint_restore_manifest_tick"),
            "Tick",
            restore_manifest_tick,
            "monotonic",
        );
    }
    assert_json_stat_absent(
        &json,
        "sim.debug.o3_trace.cpu.cpu1.checkpoint_restore_manifest_tick",
    );
    assert_json_stat_absent(&json, "sim.debug.o3_trace.cpu.cpu1.records");
}

#[test]
fn rem6_run_m5_dump_stats_restores_multicore_o3_lsq_forwarding_snapshot_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_lsq_forwarding_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-lsq-forwarding-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
            "--dump-memory",
            "0x80000400:16",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a0000006b0000000000000000000000"),
        "restored LSQ forwarding run should replay the checkpointed store/load and then the post-restore mutation"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let restored_modes = host_actions
        .pointer("/checkpoint_restores/0/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {host_actions}"));
    assert!(
        restored_modes.iter().any(|mode| {
            mode.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && mode.pointer("/mode").and_then(Value::as_str) == Some("detailed")
        }),
        "restored checkpoint should preserve cpu1 detailed-mode authority: {restored_modes:?}"
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );

    for dump in [first_dump, restored_dump] {
        for (path, value) in [
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_candidates",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_matches",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_suppressed",
                0,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load_forwarding_candidates",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load_forwarding_matches",
                1,
            ),
            ("system.cpu1.lsq0.storeLoadForwardingCandidates", 1),
            ("system.cpu1.lsq0.storeLoadForwardingMatches", 1),
            ("system.cpu1.lsq0.storeLoadForwardingSuppressed", 0),
            (
                "system.cpu1.lsq0.operation.load.storeLoadForwardingCandidates",
                1,
            ),
            (
                "system.cpu1.lsq0.operation.load.storeLoadForwardingMatches",
                1,
            ),
            (
                "system.cpu1.lsq0.operation.load.storeLoadForwardingSuppressed",
                0,
            ),
            (
                "system.cpu1.lsq0.operation.store.storeLoadForwardingCandidates",
                0,
            ),
            (
                "system.cpu1.lsq0.operation.store.storeLoadForwardingMatches",
                0,
            ),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_candidates",
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
            "system.cpu.lsq0.storeLoadForwardingCandidates",
            "system.cpu.lsq0.storeLoadForwardingMatches",
            "system.cpu0.lsq0.storeLoadForwardingMatches",
            "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
            "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
            "system.cpu0.lsq0.operation.load.storeLoadForwardingCandidates",
            "system.cpu0.lsq0.operation.load.storeLoadForwardingMatches",
        ] {
            assert_stats_dump_sample_absent(dump, path);
            assert_json_stat_absent(&json, path);
        }
    }

    let o3_trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 trace records: {json}"));
    let cpu1_record = o3_trace
        .iter()
        .find(|record| record.pointer("/cpu").and_then(Value::as_u64) == Some(1))
        .unwrap_or_else(|| panic!("missing CPU1 O3 trace record: {o3_trace:?}"));
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("gem5-m5-checkpoint")
    );
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore/latest_tick")
            .and_then(Value::as_u64),
        Some(restore_tick)
    );
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore/execution_mode_authority/target/cpu1/mode/detailed")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        cpu1_record
            .pointer("/events")
            .and_then(Value::as_array)
            .is_some_and(|events| {
                !events.is_empty()
                    && events.iter().all(|event| {
                        event.pointer("/tick").and_then(Value::as_u64) > Some(restore_tick)
                    })
            }),
        "CPU1 O3 trace should contain post-restore replay events: {cpu1_record}"
    );

    assert_json_stat_at_least(
        &json,
        "sim.debug.o3_trace.checkpoint_restore_records",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.lsq0.operation.load.storeLoadForwardingMatches",
        "Count",
        2,
        "monotonic",
    );
    for path in [
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.storeLoadForwardingMatches",
        "system.cpu0.lsq0.storeLoadForwardingMatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
        "system.cpu0.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu0.lsq0.operation.load.storeLoadForwardingMatches",
    ] {
        assert_json_stat_absent(&json, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_restores_multicore_o3_fu_snapshot_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_fu_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-fu-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let restored_modes = host_actions
        .pointer("/checkpoint_restores/0/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {host_actions}"));
    assert!(
        restored_modes.iter().any(|mode| {
            mode.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && mode.pointer("/mode").and_then(Value::as_str) == Some("detailed")
        }),
        "restored checkpoint should preserve cpu1 detailed-mode authority: {restored_modes:?}"
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );

    for dump in [first_dump, restored_dump] {
        for (path, unit, value) in [
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_latency_instructions",
                "Count",
                2,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_latency_cycles",
                "Cycle",
                21,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_integer_mul_instructions",
                "Count",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_integer_div_latency_cycles",
                "Cycle",
                19,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_float_misc_instructions",
                "Count",
                0,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_vector_float_misc_instructions",
                "Count",
                0,
            ),
        ] {
            assert_stats_dump_sample(dump, path, "counter", unit, value, "resettable");
        }
        for (path, value) in [
            ("system.cpu1.iq.issuedInstType_0::IntMult", 1),
            ("system.cpu1.iq.issuedInstType_0::IntDiv", 1),
            ("system.cpu1.iq.issuedInstType_0::FloatMisc", 0),
            ("system.cpu1.iq.issuedInstType_0::SimdFloatMisc", 0),
            ("system.cpu1.commit.committedInstType_0::IntMult", 1),
            ("system.cpu1.commit.committedInstType_0::IntDiv", 1),
            ("system.cpu1.commit.committedInstType_0::FloatMisc", 0),
            ("system.cpu1.commit.committedInstType_0::SimdFloatMisc", 0),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu1.o3.snapshot.rob.count",
            "sim.host_actions.stats_dump.cpu1.o3.snapshot.rob.entries",
            "sim.host_actions.stats_dump.cpu1.o3.snapshot.lsq.count",
            "sim.host_actions.stats_dump.cpu1.o3.snapshot.lsq.entries",
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", 0, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu1.o3.snapshot.rename_map.count",
            "sim.host_actions.stats_dump.cpu1.o3.snapshot.rename_map.entries",
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", 3, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
            "sim.host_actions.stats_dump.cpu0.o3.snapshot.rename_map.count",
            "system.cpu0.iq.issuedInstType_0::IntMult",
            "system.cpu0.commit.committedInstType_0::IntMult",
            "system.cpu.iq.issuedInstType_0::IntMult",
            "system.cpu.commit.committedInstType_0::IntMult",
        ] {
            assert_stats_dump_sample_absent(dump, path);
        }
    }
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_latency_instructions",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_instructions");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType_0::IntMult");
}

#[test]
fn rem6_run_host_action_trace_restores_multicore_o3_checkpoint_components_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_fu_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-host-action-components",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--debug-flags",
            "HostAction",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing host checkpoint restore: {json}"));
    assert_eq!(
        host_restore.pointer("/label").and_then(Value::as_str),
        Some("gem5-m5-checkpoint")
    );
    let trace_restore = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing HostAction trace: {json}"))
        .iter()
        .find(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("checkpoint_restore")
        })
        .unwrap_or_else(|| panic!("missing checkpoint restore trace: {json}"));
    assert_eq!(
        trace_restore.pointer("/components"),
        host_restore.pointer("/components"),
        "HostAction restore trace should preserve restored component/chunk payload metadata: trace {trace_restore}; host {host_restore}"
    );
    assert_trace_restore_component_chunk(trace_restore, "cpu1", "xregs");
    assert_trace_restore_component_chunk(trace_restore, "cpu1", "in-order-pipeline");
    assert_trace_restore_component_chunk(trace_restore, "cpu1", "o3-runtime-state");
    assert_eq!(
        trace_restore
            .pointer("/execution_mode_authority/target/cpu0/mode/detailed")
            .and_then(Value::as_u64),
        None,
        "CPU0 should not regain detailed authority from a CPU1 restore trace: {trace_restore}"
    );
    assert_json_stat(
        &json,
        "sim.debug.host_action_trace.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_restore_multicore_o3_checkpoint_component_stats_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_fu_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-host-action-component-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing host checkpoint restore: {json}"));
    assert_eq!(
        host_restore.pointer("/label").and_then(Value::as_str),
        Some("gem5-m5-checkpoint")
    );
    assert_restore_execution_modes_exact(host_restore, &[("cpu1", "detailed")]);
    assert_restore_component_present(host_restore, "cpu0");
    assert_restore_component_chunk_stat(
        &json,
        host_restore,
        "sim.host_actions.checkpoint_restore",
        "cpu1",
        "xregs",
        "cpu1",
        "xregs",
        "cpu1",
    );
    assert_restore_component_chunk_stat(
        &json,
        host_restore,
        "sim.host_actions.checkpoint_restore",
        "cpu1",
        "in-order-pipeline",
        "cpu1",
        "in_order_pipeline",
        "cpu1",
    );
    assert_restore_component_chunk_stat(
        &json,
        host_restore,
        "sim.host_actions.checkpoint_restore",
        "cpu1",
        "o3-runtime-state",
        "cpu1",
        "o3_runtime_state",
        "cpu1",
    );
    assert_restore_o3_runtime_chunk_stats(
        &json,
        host_restore,
        "sim.host_actions.checkpoint_restore",
        "cpu1",
        "o3-runtime-state",
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
    );
    assert_json_stat_prefix_absent(&json, "sim.host_actions.checkpoint_restore.target.cpu0.");

    let runtime_o3 = json
        .pointer("/cores/1/o3_runtime")
        .unwrap_or_else(|| panic!("missing CPU1 O3 runtime metadata: {json}"));
    assert_eq!(
        runtime_o3
            .pointer("/execution_mode")
            .and_then(Value::as_str),
        Some("detailed"),
        "CPU1 O3 runtime should preserve restored detailed execution mode: {runtime_o3}"
    );
    for (pointer, stat_path, unit) in [
        ("/stats_epoch", "sim.cpu1.o3.stats_epoch", "Count"),
        ("/stats_reset_tick", "sim.cpu1.o3.stats_reset_tick", "Tick"),
    ] {
        let expected = runtime_o3
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing O3 runtime metadata lane {pointer}: {runtime_o3}"));
        assert_json_stat(&json, stat_path, unit, expected, "monotonic");
    }
    for (stat_path, expected) in [
        ("sim.cpu1.o3.execution_mode.functional", 0),
        ("sim.cpu1.o3.execution_mode.timing", 0),
        ("sim.cpu1.o3.execution_mode.detailed", 1),
    ] {
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }
    assert_json_stat_absent(&json, "sim.cpu0.o3.stats_reset_tick");
    assert_json_stat_absent(&json, "sim.cpu0.o3.stats_epoch");
    assert_json_stat_prefix_absent(&json, "sim.cpu0.o3.execution_mode.");

    let runtime_restore = runtime_o3
        .pointer("/checkpoint_restore")
        .unwrap_or_else(|| panic!("missing CPU1 O3 runtime restore metadata: {json}"));
    let runtime_restore_count = runtime_o3
        .pointer("/checkpoint_restore_count")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing CPU1 O3 runtime restore count: {runtime_o3}"));
    assert_eq!(
        runtime_restore_count, 1,
        "CPU1 O3 runtime should report one restored checkpoint scope: {runtime_o3}"
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.checkpoint_restore.count",
        "Count",
        runtime_restore_count,
        "monotonic",
    );
    for (pointer, stat_path, unit) in [
        ("/tick", "sim.cpu1.o3.checkpoint_restore.tick", "Tick"),
        (
            "/manifest_tick",
            "sim.cpu1.o3.checkpoint_restore.manifest_tick",
            "Tick",
        ),
        (
            "/component_count",
            "sim.cpu1.o3.checkpoint_restore.component_count",
            "Count",
        ),
        (
            "/chunk_count",
            "sim.cpu1.o3.checkpoint_restore.chunk_count",
            "Count",
        ),
        (
            "/payload_bytes",
            "sim.cpu1.o3.checkpoint_restore.payload_bytes",
            "Byte",
        ),
    ] {
        let expected = runtime_restore
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing runtime restore lane {pointer}: {runtime_restore}"));
        assert_json_stat(&json, stat_path, unit, expected, "monotonic");
    }
    assert_restore_execution_modes_exact(runtime_restore, &[("cpu1", "detailed")]);
    for (stat_path, expected) in [
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.manifests",
            1,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.cleared_manifests",
            0,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.decode_errors",
            0,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.targets",
            1,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.mode.functional",
            0,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.mode.timing",
            0,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.mode.detailed",
            1,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.target.cpu1.mode.functional",
            0,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.target.cpu1.mode.timing",
            0,
        ),
        (
            "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.target.cpu1.mode.detailed",
            1,
        ),
    ] {
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }
    assert_json_stat_absent(&json, "sim.cpu0.o3.checkpoint_restore.count");
    assert_json_stat_absent(&json, "sim.cpu0.o3.checkpoint_restore.tick");
    assert_restore_component_chunk_stat(
        &json,
        runtime_restore,
        "sim.cpu1.o3.checkpoint_restore",
        "cpu1",
        "o3-runtime-state",
        "cpu1",
        "o3_runtime_state",
        "cpu1",
    );
    assert_restore_o3_runtime_chunk_stats(
        &json,
        runtime_restore,
        "sim.cpu1.o3.checkpoint_restore",
        "cpu1",
        "o3-runtime-state",
    );
    assert_json_stat_prefix_absent(&json, "sim.cpu0.o3.checkpoint_restore.component.");
    assert_json_stat_prefix_absent(
        &json,
        "sim.cpu1.o3.checkpoint_restore.execution_mode_authority.target.cpu0.",
    );
    assert_json_stat_prefix_absent(
        &json,
        "sim.cpu0.o3.checkpoint_restore.execution_mode_authority.",
    );

    let o3_restore = o3_trace_checkpoint_restore_scope(&json, 1);
    assert_eq!(
        o3_restore.pointer("/components"),
        host_restore.pointer("/components"),
        "O3 restore scope should preserve restored component/chunk payload metadata: O3 {o3_restore}; host {host_restore}"
    );
    assert_trace_restore_component_chunk(o3_restore, "cpu1", "xregs");
    assert_trace_restore_component_chunk(o3_restore, "cpu1", "in-order-pipeline");
    assert_trace_restore_component_chunk(o3_restore, "cpu1", "o3-runtime-state");
    assert_restore_component_chunk_stat(
        &json,
        o3_restore,
        "sim.debug.o3_trace.checkpoint_restore",
        "cpu1",
        "xregs",
        "cpu1",
        "xregs",
        "cpu1",
    );
    assert_restore_component_chunk_stat(
        &json,
        o3_restore,
        "sim.debug.o3_trace.cpu.cpu1.checkpoint_restore",
        "cpu1",
        "o3-runtime-state",
        "cpu1",
        "o3_runtime_state",
        "cpu1",
    );
    assert_restore_o3_runtime_chunk_stats(
        &json,
        o3_restore,
        "sim.debug.o3_trace.cpu.cpu1.checkpoint_restore",
        "cpu1",
        "o3-runtime-state",
    );
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.checkpoint_restore.component.cpu0.components",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(
        &json,
        "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
    );
    assert_json_stat_prefix_absent(&json, "sim.debug.o3_trace.checkpoint_restore.target.cpu0.");
    assert_json_stat_prefix_absent(&json, "sim.debug.o3_trace.cpu.cpu0.checkpoint_restore.");
}

fn o3_trace_checkpoint_restore_scope(json: &Value, cpu: u64) -> &Value {
    let o3_trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 trace records: {json}"));
    let record = o3_trace
        .iter()
        .find(|record| record.pointer("/cpu").and_then(Value::as_u64) == Some(cpu))
        .unwrap_or_else(|| panic!("missing CPU{cpu} O3 trace record: {o3_trace:?}"));
    assert_eq!(
        record
            .pointer("/checkpoint_restore_count")
            .and_then(Value::as_u64),
        Some(1),
        "CPU{cpu} O3 trace should be scoped to one checkpoint restore: {record}"
    );
    record
        .pointer("/checkpoint_restore")
        .unwrap_or_else(|| panic!("missing CPU{cpu} O3 checkpoint restore scope: {record}"))
}

fn assert_trace_restore_component_chunk(restore: &Value, component: &str, chunk: &str) {
    let components = restore
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restore trace components: {restore}"));
    let component = components
        .iter()
        .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing restore trace component {component}: {restore}"));
    let chunks = component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restore trace chunks for {component}: {restore}"));
    let chunk = chunks
        .iter()
        .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
        .unwrap_or_else(|| panic!("missing restore trace chunk {chunk}: {component}"));
    assert!(
        chunk
            .pointer("/payload_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > 0),
        "restore trace chunk should expose payload bytes: {chunk}"
    );
    assert!(
        chunk
            .pointer("/payload_checksum")
            .and_then(Value::as_str)
            .is_some_and(|checksum| checksum.starts_with("0x") && checksum.len() == 18),
        "restore trace chunk should expose payload checksum: {chunk}"
    );
}

fn assert_restore_execution_modes_exact(restore: &Value, expected: &[(&str, &str)]) {
    let mut actual = restore
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restore execution modes: {restore}"))
        .iter()
        .map(|mode| {
            (
                mode.pointer("/target")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                mode.pointer("/mode")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "unexpected restored execution modes: {restore}"
    );
}

fn assert_restore_component_present(restore: &Value, component: &str) {
    let components = restore
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restore components: {restore}"));
    assert!(
        components
            .iter()
            .any(|entry| entry.pointer("/component").and_then(Value::as_str) == Some(component)),
        "expected restore manifest to include component {component}: {restore}"
    );
}

fn assert_restore_component_chunk_stat(
    json: &Value,
    restore: &Value,
    stat_prefix: &str,
    component: &str,
    chunk: &str,
    component_path: &str,
    chunk_path: &str,
    target_path: &str,
) {
    let components = restore
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restore components: {restore}"));
    let component = components
        .iter()
        .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some(component))
        .unwrap_or_else(|| panic!("missing restore component {component}: {restore}"));
    let chunks = component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restore chunks for {component}: {restore}"));
    let chunk = chunks
        .iter()
        .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
        .unwrap_or_else(|| panic!("missing restore chunk {chunk}: {component}"));
    let component_chunks = component
        .pointer("/chunk_count")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing component chunk count: {component}"));
    let component_payload_bytes = component
        .pointer("/payload_bytes")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing component payload bytes: {component}"));
    let chunk_payload_bytes = chunk
        .pointer("/payload_bytes")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing chunk payload bytes: {chunk}"));
    let chunk_payload_checksum = chunk
        .pointer("/payload_checksum")
        .and_then(Value::as_str)
        .and_then(parse_hex_u64)
        .unwrap_or_else(|| panic!("missing chunk payload checksum: {chunk}"));

    assert_json_stat(
        json,
        &format!("{stat_prefix}.component.{component_path}.components"),
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.component.{component_path}.chunks"),
        "Count",
        component_chunks,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.component.{component_path}.payload_bytes"),
        "Byte",
        component_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.component.{component_path}.chunk.{chunk_path}.chunks"),
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.component.{component_path}.chunk.{chunk_path}.payload_bytes"),
        "Byte",
        chunk_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!(
            "{stat_prefix}.component.{component_path}.chunk.{chunk_path}.payload_checksum_accumulator"
        ),
        "Unspecified",
        chunk_payload_checksum,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.target.{target_path}.components"),
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.target.{target_path}.chunks"),
        "Count",
        component_chunks,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.target.{target_path}.payload_bytes"),
        "Byte",
        component_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.target.{target_path}.component.{component_path}.components"),
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.target.{target_path}.component.{component_path}.chunks"),
        "Count",
        component_chunks,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{stat_prefix}.target.{target_path}.component.{component_path}.payload_bytes"),
        "Byte",
        component_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!(
            "{stat_prefix}.target.{target_path}.component.{component_path}.chunk.{chunk_path}.chunks"
        ),
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!(
            "{stat_prefix}.target.{target_path}.component.{component_path}.chunk.{chunk_path}.payload_bytes"
        ),
        "Byte",
        chunk_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!(
            "{stat_prefix}.target.{target_path}.component.{component_path}.chunk.{chunk_path}.payload_checksum_accumulator"
        ),
        "Unspecified",
        chunk_payload_checksum,
        "monotonic",
    );
}

fn assert_restore_o3_runtime_chunk_stats(
    json: &Value,
    restore: &Value,
    stat_prefix: &str,
    component: &str,
    chunk_name: &str,
) {
    let chunk = restore
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks
                .iter()
                .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk_name))
        })
        .unwrap_or_else(|| panic!("missing restore chunk {component}/{chunk_name}: {restore}"));
    let o3_runtime = chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing decoded O3 restore chunk summary: {chunk}"));
    let component_path = component.replace('-', "_");
    let chunk_path = chunk_name.replace('-', "_");
    for (field, unit) in [
        ("stats_fu_latency_instructions", "Count"),
        ("stats_lsq_data_latency_ticks", "Tick"),
        ("stats_lsq_data_latency_max_ticks", "Tick"),
        ("stats_lsq_data_latency_min_ticks", "Tick"),
        ("stats_fu_latency_class_integer_div_cycles", "Cycle"),
    ] {
        let expected = o3_runtime
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing decoded O3 restore field {field}: {o3_runtime}"));
        for path in [
            format!("{stat_prefix}.component.{component_path}.chunk.{chunk_path}.o3_runtime.{field}"),
            format!("{stat_prefix}.target.{component_path}.component.{component_path}.chunk.{chunk_path}.o3_runtime.{field}"),
        ] {
            assert_json_stat(json, &path, unit, expected, "monotonic");
        }
    }
    for field in ["stats_lsq_data_latency_avg_ticks"] {
        for path in [
            format!("{stat_prefix}.component.{component_path}.chunk.{chunk_path}.o3_runtime.{field}"),
            format!("{stat_prefix}.target.{component_path}.component.{component_path}.chunk.{chunk_path}.o3_runtime.{field}"),
        ] {
            assert_json_stat_absent(json, &path);
        }
    }
}

fn assert_json_stat_prefix_absent(json: &Value, path_prefix: &str) {
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array in run JSON: {json}"));
    let matches = stats
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| path.starts_with(path_prefix))
        .collect::<Vec<_>>();
    assert!(
        matches.is_empty(),
        "unexpected stat paths with prefix {path_prefix}: {matches:?}"
    );
}

fn parse_hex_u64(value: &str) -> Option<u64> {
    u64::from_str_radix(value.strip_prefix("0x")?, 16).ok()
}
