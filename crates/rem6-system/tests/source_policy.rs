use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn system_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused system modules, but it has {lines} lines"
    );
}

#[test]
fn system_source_files_stay_within_size_limit() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut oversized = Vec::new();

    for path in rust_source_files(&src_dir) {
        let lines = line_count(&path);
        if lines > MAX_SOURCE_LINES {
            oversized.push(format!(
                "{} has {lines} lines",
                path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap()
                    .display()
            ));
        }
    }

    assert!(
        oversized.is_empty(),
        "source files exceed {MAX_SOURCE_LINES} lines: {}",
        oversized.join(", ")
    );
}

#[test]
fn workload_replay_data_cache_backend_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let replay_rs = fs::read_to_string(crate_dir.join("src/workload_replay.rs")).unwrap();
    let backend_rs = crate_dir.join("src/workload_replay/data_cache_backend.rs");

    assert!(
        backend_rs.exists(),
        "workload replay data-cache backend belongs in src/workload_replay/data_cache_backend.rs"
    );
    assert!(
        !replay_rs.contains("struct WorkloadDataCacheBackend"),
        "src/workload_replay.rs should delegate data-cache replay backend state to a focused module"
    );
    assert!(
        !replay_rs.contains("struct WorkloadDataCacheLineBackend"),
        "src/workload_replay.rs should delegate data-cache line backend state to a focused module"
    );
}

#[test]
fn workload_replay_planned_data_cache_sync_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let replay_rs = fs::read_to_string(crate_dir.join("src/workload_replay.rs")).unwrap();
    let sync_rs = crate_dir.join("src/workload_replay/planned_data_cache_sync.rs");

    assert!(
        sync_rs.exists(),
        "planned host data-cache sync belongs in src/workload_replay/planned_data_cache_sync.rs"
    );
    for anchor in [
        "struct PlannedDataCacheTraceOverlap",
        "fn planned_data_cache_trace_overlap(",
        "fn planned_host_data_cache_sync_handler(",
        "fn sync_data_cache_lines_to_memory(",
        "fn sync_data_cache_lines_from_memory(",
    ] {
        assert!(
            !replay_rs.contains(anchor),
            "src/workload_replay.rs should delegate {anchor} to a focused module"
        );
    }
}

#[test]
fn workload_replay_summary_tests_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let summary_rs = fs::read_to_string(crate_dir.join("src/workload_replay/summary.rs")).unwrap();
    let tests_rs =
        crate_dir.join("src/workload_replay/summary/parallel_execution_summary_tests.rs");

    assert!(
        tests_rs.exists(),
        "workload replay summary tests belong in src/workload_replay/summary/parallel_execution_summary_tests.rs"
    );
    for anchor in [
        "fn livelock_transition_threshold_uses_lowest_declared_clean_threshold",
        "fn parallel_execution_summary_copies_dram_qos_activity",
        "fn parallel_execution_summary_copies_dma_scheduler_empty_epochs",
        "fn parallel_execution_summary_copies_dma_scheduler_frontiers",
        "fn parallel_execution_summary_copies_dma_scheduler_remote_traffic",
        "fn parallel_execution_summary_copies_scheduler_remote_flows",
        "fn parallel_execution_summary_copies_full_system_batch_partition_streaks",
        "fn parallel_execution_summary_copies_scoped_batch_timeline",
        "fn parallel_execution_summary_copies_scheduler_progress_transitions",
        "fn parallel_execution_summary_uses_livelock_transition_threshold",
        "fn parallel_execution_summary_preserves_livelock_diagnostic_records",
        "fn parallel_execution_summary_preserves_cross_subsystem_deadlocks",
        "fn parallel_execution_summary_preserves_compute_and_dma_wait_for_edge_kinds",
        "fn parallel_execution_summary_copies_data_cache_scheduler_frontiers",
    ] {
        assert!(
            !summary_rs.contains(anchor),
            "src/workload_replay/summary.rs should delegate {anchor} to a focused test module"
        );
    }
}

#[test]
fn topology_boot_handoff_types_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let topology_rs = fs::read_to_string(crate_dir.join("src/topology.rs")).unwrap();
    let boot_handoff_rs = crate_dir.join("src/topology/boot_handoff.rs");

    assert!(
        boot_handoff_rs.exists(),
        "RISC-V boot handoff types belong in src/topology/boot_handoff.rs"
    );
    for anchor in [
        "struct RiscvDtbHandoffReport",
        "struct RiscvLinuxInitrdImage",
        "struct RiscvLinuxBootHandoffConfig",
        "struct RiscvLinuxBootHandoffReport",
    ] {
        assert!(
            !topology_rs.contains(anchor),
            "src/topology.rs should delegate {anchor} to a focused module"
        );
    }
}

#[test]
fn host_execution_mode_checkpoint_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host_rs = fs::read_to_string(crate_dir.join("src/host.rs")).unwrap();
    let checkpoint_rs = crate_dir.join("src/host/execution_mode_checkpoint.rs");

    assert!(
        checkpoint_rs.exists(),
        "execution-mode checkpoint helpers belong in src/host/execution_mode_checkpoint.rs"
    );
    assert!(
        !host_rs.contains("enum ExecutionModeCheckpointError"),
        "src/host.rs should delegate execution-mode checkpoint errors to a focused module"
    );
    for anchor in [
        "fn execution_mode_checkpoint_component",
        "fn manifest_has_execution_mode_checkpoint",
        "fn encode_execution_modes",
        "fn decode_execution_modes",
        "fn read_u64",
        "fn execution_mode_from_code",
    ] {
        assert!(
            !host_rs.contains(anchor),
            "src/host.rs should delegate {anchor} to a focused module"
        );
    }
}

#[test]
fn riscv_syscall_table_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_rs = fs::read_to_string(crate_dir.join("src/riscv_syscall.rs")).unwrap();
    let table_rs = crate_dir.join("src/riscv_syscall/table.rs");

    assert!(
        table_rs.exists(),
        "RISC-V syscall dispatch table belongs in src/riscv_syscall/table.rs"
    );
    for anchor in [
        "pub struct RiscvSyscallTable",
        "impl RiscvSyscallTable",
        "fn unsupported_syscall_outcome(",
    ] {
        assert!(
            !root_rs.contains(anchor),
            "src/riscv_syscall.rs should delegate {anchor} to a focused dispatch-table module"
        );
    }
}

#[test]
fn riscv_checkpoint_projects_legacy_o3_pending_state_from_runtime() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checkpoint_rs = fs::read_to_string(crate_dir.join("src/riscv_checkpoint.rs")).unwrap();
    let record = source_section(
        &checkpoint_rs,
        "pub struct RiscvCoreCheckpointRecord {",
        "struct RiscvCoreCheckpointRecordParts {",
    );
    let record_parts = source_section(
        &checkpoint_rs,
        "struct RiscvCoreCheckpointRecordParts {",
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]",
    );
    let capture = source_section(
        &checkpoint_rs,
        "pub fn capture_into(",
        "pub fn restore_from(",
    );

    for definition in [record, record_parts] {
        assert!(
            !definition.contains("O3PendingStateCheckpointPayload"),
            "RISC-V checkpoint records must retain only the complete O3 runtime payload"
        );
    }
    assert!(
        capture.contains(
            "o3_pending_state_payload_from_runtime(record.o3_runtime_payload())"
        ),
        "checkpoint capture must project the legacy pending chunk from the captured runtime payload"
    );
    assert!(
        capture.contains("O3_PENDING_STATE_CHUNK"),
        "checkpoint capture must continue emitting the legacy pending chunk"
    );
}

fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source section start: {start}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source section end: {end}"))
        .0
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}
