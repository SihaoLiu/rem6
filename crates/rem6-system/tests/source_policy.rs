use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;
const RISCV_SYSCALL_SBI_TEST_COUNT: usize = 261;
const RISCV_SYSCALL_SBI_TEST_MODULES: [&str; 34] = [
    "riscv_sbi_base",
    "riscv_sbi_debug_console",
    "riscv_sbi_firmware",
    "riscv_sbi_hsm_suspend",
    "riscv_syscall_admin",
    "riscv_syscall_brk_emulation",
    "riscv_syscall_clock_gettime",
    "riscv_syscall_close_range",
    "riscv_syscall_epoll",
    "riscv_syscall_eventfd",
    "riscv_syscall_getcwd",
    "riscv_syscall_getrlimit",
    "riscv_syscall_getrusage",
    "riscv_syscall_inotify",
    "riscv_syscall_ioctl",
    "riscv_syscall_lseek",
    "riscv_syscall_mknod",
    "riscv_syscall_openat2",
    "riscv_syscall_pipe",
    "riscv_syscall_prlimit64",
    "riscv_syscall_pselect",
    "riscv_syscall_readlinkat",
    "riscv_syscall_readv",
    "riscv_syscall_rename",
    "riscv_syscall_riscv_flush_icache",
    "riscv_syscall_robust_list",
    "riscv_syscall_rseq",
    "riscv_syscall_signalfd",
    "riscv_syscall_socket",
    "riscv_syscall_startup_stack",
    "riscv_syscall_times",
    "riscv_syscall_uname",
    "riscv_syscall_wait4",
    "riscv_syscall_writev",
];

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
fn host_assisted_kvm_facade_stays_retired_until_real_adapter_exists() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    assert!(
        !crate_dir.join("src/host_assist.rs").exists(),
        "admission-only host_assist.rs must stay retired until a real host adapter exists"
    );
    assert!(
        !crate_dir.join("tests/host_assist.rs").exists(),
        "synthetic host_assist integration tests should be deleted with the facade"
    );

    for root in [crate_dir.join("src"), crate_dir.join("tests")] {
        for path in rust_source_files(&root) {
            if path.ends_with("tests/source_policy.rs") {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            for forbidden in ["HostAssisted", "host_assist"] {
                assert!(
                    !source.contains(forbidden),
                    "{} must not restore retired host-assist marker `{forbidden}`",
                    path.strip_prefix(crate_dir).unwrap().display()
                );
            }
        }
    }
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
fn execution_mode_switch_transfers_derive_hierarchy_totals_from_components() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = fs::read_to_string(crate_dir.join("src/host.rs")).unwrap();
    let transfer = struct_body(&host, "ExecutionModeSwitchStateTransfer");
    let component = struct_body(&host, "ExecutionModeSwitchStateTransferComponent");
    let host_without_whitespace = without_whitespace(&host);

    for forbidden in ["component_count:", "chunk_count:", "payload_bytes:"] {
        assert!(!transfer.contains(forbidden), "transfer caches {forbidden}");
    }
    for forbidden in ["chunk_count:", "payload_bytes:"] {
        assert!(
            !component.contains(forbidden),
            "component caches {forbidden}"
        );
    }
    assert!(host.contains("self.components.len() as u64"));
    assert!(host.contains(".map(ExecutionModeSwitchStateTransferComponent::chunk_count)"));
    assert!(host.contains(".map(ExecutionModeSwitchStateTransferComponent::payload_bytes)"));
    assert!(host.contains("self.chunks.len() as u64"));
    assert!(host.contains(".map(ExecutionModeSwitchStateTransferChunk::payload_bytes)"));
    for required in [
        "pub const fn component_count(&self) -> u64 {
            self.components.len() as u64
        }",
        "pub const fn chunk_count(&self) -> u64 {
            self.chunks.len() as u64
        }",
    ] {
        assert!(
            host_without_whitespace.contains(&without_whitespace(required)),
            "len-only accessor must stay const-compatible: `{required}`"
        );
    }

    let constructor =
        fs::read_to_string(crate_dir.join("src/host/execution_mode_transfer.rs")).unwrap();
    let constructor_without_whitespace = without_whitespace(&constructor);
    for required in [
        "components.len() as u64",
        ".map(ExecutionModeSwitchStateTransferComponent::chunk_count).sum()",
        ".map(ExecutionModeSwitchStateTransferComponent::payload_bytes).sum()",
    ] {
        assert!(
            constructor_without_whitespace.contains(&without_whitespace(required)),
            "transfer construction must derive projection fragment `{required}` from built components"
        );
    }
    assert!(
        !constructor_without_whitespace.contains("manifest.summary()"),
        "transfer construction must not use manifest summary totals as authority"
    );
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
fn riscv_checkpoint_emits_one_o3_authority_and_isolates_legacy_decode() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checkpoint_rs = fs::read_to_string(crate_dir.join("src/riscv_checkpoint.rs")).unwrap();
    let o3_payload_path = crate_dir.join("src/riscv_checkpoint/o3_payload.rs");
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
    let write_record = source_section(&checkpoint_rs, "fn write_record(", "pub fn restore_from(");

    for definition in [record, record_parts] {
        assert!(
            !definition.contains("O3PendingStateCheckpointPayload"),
            "RISC-V checkpoint records must retain only the complete O3 runtime payload"
        );
    }
    assert!(
        checkpoint_rs.contains("mod o3_payload;"),
        "RISC-V checkpoint manifest compatibility must live in src/riscv_checkpoint/o3_payload.rs"
    );
    assert!(
        !checkpoint_rs.contains("O3PendingStateCheckpointPayload")
            && !checkpoint_rs.contains("\"o3-pending-state\""),
        "legacy O3 pending payload types and chunk literals must stay out of the root checkpoint module"
    );
    assert!(
        o3_payload_path.exists(),
        "RISC-V O3 checkpoint payload compatibility belongs in src/riscv_checkpoint/o3_payload.rs"
    );
    let o3_payload = fs::read_to_string(o3_payload_path).unwrap();
    assert!(
        write_record.contains("registry.remove_chunk(&self.component, O3_PENDING_STATE_CHUNK)"),
        "current capture must prune stale legacy O3 pending chunks"
    );
    assert_eq!(
        write_record.matches("O3_PENDING_STATE_CHUNK").count(),
        1,
        "current capture may reference the legacy O3 pending chunk only for pruning"
    );
    assert!(
        write_record.contains("O3_RUNTIME_STATE_CHUNK"),
        "current capture must emit the complete O3 runtime authority"
    );
    assert!(
        !write_record.contains("encode_o3_pending_state_payload")
            && !write_record.contains("o3_pending_state_payload_from_runtime"),
        "current capture must not derive or emit a second O3 pending-state authority"
    );
    for anchor in [
        "O3PendingStateCheckpointPayload",
        "decode_o3_runtime_authority",
        "MismatchedO3PendingStateSnapshot",
    ] {
        assert!(
            o3_payload.contains(anchor),
            "legacy O3 checkpoint decode module is missing `{anchor}`"
        );
    }
    assert!(
        o3_payload.contains("O3RuntimeCheckpointPayload::from_legacy_pending_state("),
        "legacy O3 pending-only decode must delegate snapshot construction to rem6-cpu"
    );
    assert!(
        !o3_payload.contains("O3RuntimeSnapshot::new"),
        "the rem6-system legacy bridge must not reconstruct O3 runtime snapshots locally"
    );
    assert!(
        checkpoint_rs.contains("decode_o3_runtime_authority("),
        "RISC-V checkpoint restore must delegate O3 authority selection"
    );
}

#[test]
fn riscv_checkpoint_owns_one_versioned_vector_state_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checkpoint_path = crate_dir.join("src/riscv_checkpoint.rs");
    let vector_state_path = crate_dir.join("src/riscv_checkpoint/vector_state.rs");
    assert!(
        vector_state_path.exists(),
        "RISC-V vector checkpoint compatibility belongs in src/riscv_checkpoint/vector_state.rs"
    );

    let checkpoint_source = fs::read_to_string(&checkpoint_path).unwrap();
    let vector_state_source = fs::read_to_string(&vector_state_path).unwrap();
    let checkpoint = rust_code_without_comments_and_literals(&checkpoint_source);
    let vector_state = rust_code_without_comments_and_literals(&vector_state_source);
    let checkpoint_literals = active_rust_string_literals(&checkpoint_source);
    let vector_state_literals = active_rust_string_literals(&vector_state_source);
    let record = source_section(
        &checkpoint,
        "pub struct RiscvCoreCheckpointRecord {",
        "struct RiscvCoreCheckpointRecordParts {",
    );
    let record_parts = source_section(
        &checkpoint,
        "struct RiscvCoreCheckpointRecordParts {",
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]",
    );
    let write_record = source_section(&checkpoint, "fn write_record(", "pub fn restore_from(");
    let decode_from = source_section(&checkpoint, "fn decode_from(", "fn restore_record(");
    let restore_record = source_section(&checkpoint, "fn restore_record(", "fn capture_record(");
    let compact_write = without_whitespace(write_record);
    let (encode_signature, encode_body) = rust_function_signature_and_body(
        &vector_state,
        "pub(super) fn encode_vector_architectural_state",
    );
    let (decode_signature, decode_body) = rust_function_signature_and_body(
        &vector_state,
        "pub(super) fn decode_vector_architectural_state",
    );

    assert!(
        checkpoint.contains("mod vector_state;"),
        "the root checkpoint module must declare its vector-state codec child"
    );
    for (owner, definition) in [
        ("RiscvCoreCheckpointRecord", record),
        ("RiscvCoreCheckpointRecordParts", record_parts),
    ] {
        assert_eq!(
            definition.matches("RiscvVectorArchitecturalState").count(),
            1,
            "{owner} must carry exactly one complete vector architectural-state field"
        );
    }

    for chunk in ["RISCV_STATE_VERSION_CHUNK", "VECTOR_STATE_CHUNK"] {
        assert_eq!(
            write_record.matches(chunk).count(),
            1,
            "write_record must reference {chunk} exactly once"
        );
        assert!(
            compact_write.contains(&format!("registry.write_chunk(&self.component,{chunk},")),
            "write_record must write {chunk} directly"
        );
    }
    for literal in ["riscv-state-version", "vector-state"] {
        assert!(
            !checkpoint_literals.contains(&literal),
            "the root checkpoint module must not own vector chunk literal {literal:?}"
        );
        assert_eq!(
            vector_state_literals
                .iter()
                .filter(|candidate| **candidate == literal)
                .count(),
            1,
            "the vector-state child must own active chunk literal {literal:?} exactly once"
        );
    }

    let riscv_version = vector_state.lines().any(|line| {
        line.contains("const ")
            && line.contains("RISCV")
            && line.contains("VERSION")
            && line.contains("u8")
            && line.contains("= 1;")
    });
    let payload_version = vector_state.lines().any(|line| {
        line.contains("const ")
            && line.contains("VECTOR")
            && line.contains("VERSION")
            && line.contains("u8")
            && line.contains("= 1;")
    });
    let exact_payload_bytes = vector_state.lines().any(|line| {
        line.contains("const ")
            && line.contains("VECTOR")
            && line.contains("BYTES")
            && line.contains("usize")
            && line.contains("= 526;")
    });
    assert!(
        riscv_version,
        "the child must own current RISC-V state version 1"
    );
    assert!(
        payload_version,
        "the child must own vector payload version 1"
    );
    assert!(
        exact_payload_bytes,
        "the child must name the exact 526-byte vector payload contract"
    );

    assert_eq!(
        without_whitespace(encode_signature),
        "pub(super)fnencode_vector_architectural_state(state:&RiscvVectorArchitecturalState)->Vec<u8>",
        "the child must expose the exact complete-state encoder signature"
    );
    let compact_encode_body = without_whitespace(encode_body);
    for anchor in ["state.config()", "state.fixed_point()", "state.registers()"] {
        assert!(
            compact_encode_body.contains(anchor),
            "complete-state encoder body is missing `{anchor}`"
        );
    }
    assert_eq!(
        write_record
            .matches("encode_vector_architectural_state(")
            .count(),
        1,
        "write_record must call the child encoder exactly once"
    );
    let encode_arguments = without_whitespace(rust_call_arguments(
        write_record,
        "encode_vector_architectural_state",
    ));
    assert_eq!(
        encode_arguments.trim_end_matches(','),
        "record.vector_architectural_state()",
        "write_record must encode the record's complete vector state"
    );

    assert_eq!(
        normalized_rust_function_signature(decode_signature),
        "pub(super)fndecode_vector_architectural_state(component:&CheckpointComponentId,state_version:Option<&[u8]>,vector_state:Option<&[u8]>)->Result<RiscvVectorArchitecturalState,RiscvCoreCheckpointError>",
        "the child must expose the exact paired vector decoder signature"
    );
    let compact_decode_body = without_whitespace(decode_body);
    assert!(
        compact_decode_body.contains("match(state_version,vector_state)"),
        "vector decode must pair the named generation and vector payload options in one match"
    );
    assert!(
        compact_decode_body.contains("Ok(RiscvVectorArchitecturalState::default())"),
        "paired decode must map legacy chunk absence to architectural defaults"
    );
    for error in [
        "InvalidChunkSize",
        "UnsupportedRiscvStateVersion",
        "UnexpectedVectorStateWithoutVersion",
        "MissingChunk",
        "UnsupportedVectorStateVersion",
        "InvalidVectorStateVcsr",
    ] {
        let branch = format!("RiscvCoreCheckpointError::{error}{{");
        assert!(
            compact_decode_body.contains(&branch),
            "paired vector decode body is missing `{error}` error branch"
        );
    }
    assert_eq!(
        decode_from
            .matches("decode_vector_architectural_state(")
            .count(),
        1,
        "decode_from must call the child decoder exactly once"
    );
    let decode_arguments = without_whitespace(rust_call_arguments(
        decode_from,
        "decode_vector_architectural_state",
    ));
    assert_eq!(
        decode_arguments.trim_end_matches(','),
        "&self.component,registry.chunk(&self.component,RISCV_STATE_VERSION_CHUNK),registry.chunk(&self.component,VECTOR_STATE_CHUNK)",
        "decode_from must pass the component and both chunk lookups directly to the child decoder"
    );

    let split_chunks = ["vl", "vtype", "vxrm", "vxsat"]
        .into_iter()
        .map(String::from)
        .chain((0..32).map(|index| format!("v{index}")));
    for split_chunk in split_chunks {
        assert!(
            !checkpoint_literals.contains(&split_chunk.as_str())
                && !vector_state_literals.contains(&split_chunk.as_str()),
            "vector checkpointing must not introduce active split chunk literal {split_chunk:?}"
        );
    }

    for required in [
        "prepare_checkpoint_restore",
        "install_prepared_checkpoint_restore",
        "hart.restore_vector_architectural_state",
        "unwrap_or_else(|| self.core.checkpoint_hart_state())",
    ] {
        assert!(
            restore_record.contains(required),
            "missing prepared restore `{required}`"
        );
    }
    for forbidden in [
        "restore_o3_runtime_checkpoint_payload",
        "restore_branch_predictor_checkpoint_payload",
        "restore_in_order_pipeline_snapshot",
    ] {
        assert!(
            !restore_record.contains(forbidden),
            "commit path retains `{forbidden}`"
        );
    }
    assert!(
        line_count(&checkpoint_path) <= 1800,
        "src/riscv_checkpoint.rs must remain at or below 1,800 lines"
    );
}

#[test]
fn rust_source_structure_helpers_ignore_comments_and_literals() {
    let source = r####"
// fn real_target(fake: usize) { real_call(comment); } fn after_target(
/* "comment-only" r#"block-only"# { real_call(block); } */
const NORMAL: &str = "fn real_target(fake: usize) { real_call(string); }";
const RAW: &str = r##"fn real_target(fake: usize) { real_call(raw); }"##;

fn real_target(value: usize) -> usize {
    // real_call(comment); }
    let _normal = "real_call(string)); } fn after_target(";
    let _raw = r#"real_call(raw)); } fn after_target("#;
    let _character = '}';
    real_call(value, (value + 1))
}

fn after_target() {}
"####;
    let code = rust_code_without_comments_and_literals(source);
    let section = source_section(&code, "fn real_target(", "fn after_target(");
    let (signature, body) = rust_function_signature_and_body(&code, "fn real_target");
    let arguments = rust_call_arguments(body, "real_call");

    assert_eq!(
        without_whitespace(signature),
        "fnreal_target(value:usize)->usize"
    );
    for formatted_signature in [
        "fn real_target(value: usize) -> usize",
        "fn real_target(\n    value: usize,\n) -> usize",
    ] {
        assert_eq!(
            normalized_rust_function_signature(formatted_signature),
            "fnreal_target(value:usize)->usize"
        );
    }
    assert_eq!(without_whitespace(arguments), "value,(value+1)");
    assert_eq!(section.matches("real_call(").count(), 1);
    assert!(without_whitespace(body).ends_with("real_call(value,(value+1))"));

    let literals = active_rust_string_literals(source);
    assert!(literals.contains(&"fn real_target(fake: usize) { real_call(string); }"));
    assert!(literals.contains(&"fn real_target(fake: usize) { real_call(raw); }"));
    assert!(!literals.contains(&"comment-only"));
    assert!(!literals.contains(&"block-only"));
    assert!(!literals.contains(&"}"));
}

#[test]
fn riscv_syscall_and_sbi_tests_share_one_integration_target() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = crate_dir.join("tests");
    let suite_dir = tests_dir.join("riscv_syscall_emulation");
    let suite_root = fs::read_to_string(tests_dir.join("riscv_syscall_emulation.rs")).unwrap();
    let manifest = fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
    let support = fs::read_to_string(suite_dir.join("support.rs")).unwrap();
    let shared_helpers = support
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("pub(crate) fn ")?
                .split_once('(')
                .map(|(name, _)| name)
        })
        .collect::<Vec<_>>();
    let mut split_roots = Vec::new();

    assert!(
        !manifest_declares_explicit_test(&manifest),
        "rem6-system integration tests must remain auto-discovered so split explicit targets cannot bypass the suite"
    );
    assert_no_test_conditional_compilation(&suite_root, "suite root");
    assert_no_test_conditional_compilation(&support, "shared support");
    assert_no_test_warning_suppression(&suite_root, "suite root");
    assert_no_shared_helper_shadowing(&suite_root, "suite root", &shared_helpers);
    assert!(
        !suite_root.contains("include!("),
        "suite root must not include another support or test source"
    );
    let mut test_count = source_test_count(&suite_root);

    for entry in fs::read_dir(&tests_dir).unwrap() {
        let path = entry.unwrap().path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_file()
            && ((name.starts_with("riscv_syscall_") && name != "riscv_syscall_emulation.rs")
                || name.starts_with("riscv_sbi_"))
        {
            split_roots.push(name.to_string());
        }
        if path.extension().is_some_and(|extension| extension == "rs")
            && name != "riscv_syscall_emulation.rs"
            && name != "source_policy.rs"
        {
            let source = fs::read_to_string(&path).unwrap();
            assert!(
                !source.contains("riscv_syscall_emulation"),
                "top-level integration target {name} references the consolidated syscall/SBI suite"
            );
        }
    }
    split_roots.sort();
    assert!(
        split_roots.is_empty(),
        "split syscall/SBI test roots remain: {split_roots:?}"
    );

    let mut modules = Vec::new();
    for entry in fs::read_dir(&suite_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "rs")
            && path.file_name().is_some_and(|name| name != "support.rs")
        {
            let module = path.file_stem().unwrap().to_str().unwrap();
            let source = fs::read_to_string(&path).unwrap();
            let declaration =
                format!("#[path = \"riscv_syscall_emulation/{module}.rs\"]\nmod {module};");
            assert_eq!(
                suite_root.matches(&declaration).count(),
                1,
                "suite root must declare module {module} exactly once with its canonical path"
            );
            assert_no_test_conditional_compilation(&source, module);
            assert_no_test_warning_suppression(&source, module);
            assert!(
                !source.contains("#[path") && !source.contains("include!("),
                "module {module} must not import another local support or test source"
            );
            assert_eq!(
                source
                    .matches("use super::riscv_syscall_emulation_support::*;")
                    .count(),
                1,
                "module {module} must import the shared support module exactly once"
            );
            for local_support in ["mod support;", "mod riscv_syscall_emulation_support;"] {
                assert!(
                    !source.contains(local_support),
                    "module {module} retains local support declaration `{local_support}`"
                );
            }
            assert_no_shared_helper_shadowing(&source, module, &shared_helpers);
            test_count += source_test_count(&source);
            modules.push(module.to_string());
        }
    }
    modules.sort();
    assert_eq!(
        modules,
        RISCV_SYSCALL_SBI_TEST_MODULES
            .iter()
            .map(|module| (*module).to_string())
            .collect::<Vec<_>>(),
        "consolidated syscall/SBI module inventory changed"
    );
    assert_eq!(
        suite_root
            .matches(
                "#[path = \"riscv_syscall_emulation/support.rs\"]\nmod riscv_syscall_emulation_support;"
            )
            .count(),
        1,
        "suite root must declare the shared support module exactly once"
    );
    assert_eq!(
        suite_root
            .matches("#[path = \"riscv_syscall_emulation/")
            .count(),
        RISCV_SYSCALL_SBI_TEST_MODULES.len() + 1,
        "suite root must contain one canonical path declaration per module and shared support"
    );
    assert_eq!(
        suite_root.matches("#[path").count(),
        RISCV_SYSCALL_SBI_TEST_MODULES.len() + 1,
        "suite root must not declare additional path-based modules"
    );
    assert_eq!(
        test_count, RISCV_SYSCALL_SBI_TEST_COUNT,
        "consolidated syscall/SBI source test inventory changed"
    );
    assert_no_test_warning_suppression(&support, "shared support");
}

fn manifest_declares_explicit_test(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        let header = line
            .split('#')
            .next()
            .unwrap()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        matches!(header.as_str(), "[[test]]" | "[[\"test\"]]" | "[['test']]")
    })
}

fn assert_no_test_conditional_compilation(source: &str, owner: &str) {
    let compact = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(
        !compact.contains("#[cfg") && !compact.contains("#![cfg"),
        "{owner} must not conditionally compile consolidated syscall/SBI coverage"
    );
}

fn assert_no_test_warning_suppression(source: &str, owner: &str) {
    let compact = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    for remainder in compact.split("allow(").skip(1) {
        let (allowance, _) = remainder
            .split_once(')')
            .unwrap_or_else(|| panic!("{owner} contains a malformed allow attribute"));
        for forbidden in ["dead_code", "unused", "unused_imports", "warnings"] {
            assert!(
                !allowance.contains(forbidden),
                "{owner} suppresses `{forbidden}` warnings"
            );
        }
    }
}

fn assert_no_shared_helper_shadowing(source: &str, owner: &str, shared_helpers: &[&str]) {
    for helper in shared_helpers {
        assert!(
            !source.contains(&format!("fn {helper}(")),
            "{owner} shadows shared support helper `{helper}`"
        );
    }
}

fn source_test_count(source: &str) -> usize {
    source
        .lines()
        .filter(|line| line.trim() == "#[test]")
        .count()
}

struct RustSourceProjection<'a> {
    code: String,
    string_literals: Vec<&'a str>,
}

fn rust_code_without_comments_and_literals(source: &str) -> String {
    rust_source_projection(source).code
}

fn active_rust_string_literals(source: &str) -> Vec<&str> {
    rust_source_projection(source).string_literals
}

fn rust_source_projection(source: &str) -> RustSourceProjection<'_> {
    let bytes = source.as_bytes();
    let mut code = bytes.to_vec();
    let mut string_literals = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            let start = index;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            mask_rust_source_range(&mut code, start, index);
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            let start = index;
            let mut depth = 1_usize;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            mask_rust_source_range(&mut code, start, index);
            continue;
        }
        if let Some((content_start, content_end, end)) = rust_raw_string_bounds(bytes, index) {
            string_literals.push(&source[content_start..content_end]);
            mask_rust_source_range(&mut code, index, end);
            index = end;
            continue;
        }
        if bytes[index] == b'\'' {
            if let Some(end) = rust_char_literal_end(source, index) {
                mask_rust_source_range(&mut code, index, end);
                index = end;
                continue;
            }
        }
        if bytes[index] == b'"' {
            let content_start = index + 1;
            let mut end = content_start;
            let mut escaped = false;
            while end < bytes.len() {
                let current = bytes[end];
                if escaped {
                    escaped = false;
                } else if current == b'\\' {
                    escaped = true;
                } else if current == b'"' {
                    break;
                }
                end += 1;
            }
            string_literals.push(&source[content_start..end]);
            if end < bytes.len() {
                end += 1;
            }
            mask_rust_source_range(&mut code, index, end);
            index = end;
            continue;
        }
        index += 1;
    }

    RustSourceProjection {
        code: String::from_utf8(code).expect("masked Rust source remains UTF-8"),
        string_literals,
    }
}

fn rust_raw_string_bounds(bytes: &[u8], start: usize) -> Option<(usize, usize, usize)> {
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let mut quote = start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }

    let hashes = quote - start - 1;
    let content_start = quote + 1;
    let mut closing_quote = content_start;
    while closing_quote < bytes.len() {
        if bytes[closing_quote] == b'"'
            && bytes
                .get(closing_quote + 1..closing_quote + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some((content_start, closing_quote, closing_quote + 1 + hashes));
        }
        closing_quote += 1;
    }
    Some((content_start, bytes.len(), bytes.len()))
}

fn rust_char_literal_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'\'') {
        return None;
    }
    let mut index = start + 1;
    if bytes.get(index) == Some(&b'\\') {
        index += 1;
        match *bytes.get(index)? {
            b'x' => {
                if !bytes.get(index + 1)?.is_ascii_hexdigit()
                    || !bytes.get(index + 2)?.is_ascii_hexdigit()
                {
                    return None;
                }
                index += 3;
            }
            b'u' => {
                index += 1;
                if bytes.get(index) != Some(&b'{') {
                    return None;
                }
                index += 1;
                let digits_start = index;
                while bytes
                    .get(index)
                    .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'_')
                {
                    index += 1;
                }
                if index == digits_start || bytes.get(index) != Some(&b'}') {
                    return None;
                }
                index += 1;
            }
            b'\n' | b'\r' => return None,
            _ => index += 1,
        }
    } else {
        let character = source.get(index..)?.chars().next()?;
        if matches!(character, '\n' | '\r' | '\'') {
            return None;
        }
        index += character.len_utf8();
    }
    (bytes.get(index) == Some(&b'\'')).then_some(index + 1)
}

fn mask_rust_source_range(code: &mut [u8], start: usize, end: usize) {
    for byte in &mut code[start..end] {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
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

fn rust_function_signature_and_body<'a>(source: &'a str, declaration: &str) -> (&'a str, &'a str) {
    let start = source
        .find(declaration)
        .unwrap_or_else(|| panic!("missing function declaration `{declaration}`"));
    let open_offset = source[start..]
        .find('{')
        .unwrap_or_else(|| panic!("function declaration `{declaration}` is missing its body"));
    let open = start + open_offset;
    (
        source[start..open].trim(),
        balanced_delimited_content(source, open, '{', '}', declaration),
    )
}

fn rust_call_arguments<'a>(source: &'a str, function: &str) -> &'a str {
    let call = format!("{function}(");
    let start = source
        .find(&call)
        .unwrap_or_else(|| panic!("missing call `{function}`"));
    let open = start + call.len() - 1;
    balanced_delimited_content(source, open, '(', ')', function)
}

fn balanced_delimited_content<'a>(
    source: &'a str,
    open: usize,
    opening: char,
    closing: char,
    owner: &str,
) -> &'a str {
    assert_eq!(source[open..].chars().next(), Some(opening));
    let body_start = open + opening.len_utf8();
    let mut depth = 1_usize;
    for (offset, character) in source[body_start..].char_indices() {
        if character == opening {
            depth += 1;
        } else if character == closing {
            depth -= 1;
            if depth == 0 {
                return &source[body_start..body_start + offset];
            }
        }
    }
    panic!("`{owner}` is missing balanced delimiter `{closing}`");
}

fn struct_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("pub struct {name}"))
        .unwrap_or_else(|| panic!("missing struct {name}"));
    let open_offset = source[start..]
        .find('{')
        .unwrap_or_else(|| panic!("missing opening brace for struct {name}"));
    let body_start = start + open_offset + 1;
    let mut depth = 1usize;

    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[body_start..body_start + offset];
                }
            }
            _ => {}
        }
    }

    panic!("missing closing brace for struct {name}");
}

fn without_whitespace(source: &str) -> String {
    source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn normalized_rust_function_signature(signature: &str) -> String {
    without_whitespace(signature).replace(",)->", ")->")
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
