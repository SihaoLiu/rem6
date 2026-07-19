use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_O3_RUNTIME_ISSUE_LINES: usize = 800;
const MAX_O3_RUNTIME_MEMORY_LINES: usize = 1200;
const MAX_O3_RUNTIME_ROOT_LINES: usize = 1200;
const MAX_O3_RUNTIME_CONTROL_WINDOW_TEST_ROOT_LINES: usize = 1350;
const MAX_O3_RUNTIME_CONTROL_WINDOW_LIFECYCLE_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_CONTROL_WINDOW_LINEAGE_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_TARGET_TEST_LINES: usize = 225;
const MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_RETURN_TEST_LINES: usize = 200;
const MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_SCALAR_RETURN_TEST_LINES: usize = 240;
const MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_CHAIN_VALIDATION_TEST_LINES: usize = 180;
const MAX_O3_RUNTIME_PRODUCER_FORWARDED_CHAIN_LINES: usize = 650;
const MAX_O3_RUNTIME_PRODUCER_FORWARDED_VALUE_LINES: usize = 400;
const MAX_O3_RUNTIME_PRODUCER_FORWARDED_OWNER_LINES: usize = 1000;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CHAIN_VALIDATION_TEST_LINES: usize = 120;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CONTROL_VALIDATION_TEST_LINES: usize = 100;
const MAX_RISCV_FETCH_AHEAD_RAS_REQUIRED_VALIDATION_TEST_LINES: usize = 100;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_RETURN_TEST_LINES: usize = 200;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_RETURN_LINK_SHAPES_TEST_LINES: usize = 100;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_SCALAR_RETURN_TEST_LINES: usize = 600;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_SCALAR_RETURN_LINK_SHAPES_TEST_LINES: usize = 100;
const MAX_RISCV_FETCH_AHEAD_PREPARED_LINES: usize = 175;
const MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CONTINUATION_LINES: usize = 240;
const MAX_RISCV_FETCH_AHEAD_PREPARED_OWNER_LINES: usize = 390;
const MAX_O3_RUNTIME_LIVE_WINDOW_LINES: usize = 800;
const MAX_O3_RUNTIME_LIVE_WINDOW_TEST_LINES: usize = 1100;
const MAX_O3_RUNTIME_LIVE_WINDOW_IDENTITY_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_WRITEBACK_LINES: usize = 800;
const MAX_O3_RUNTIME_WRITEBACK_REPLAN_LINES: usize = 600;
const MAX_O3_RUNTIME_WRITEBACK_OWNERSHIP_LINES: usize = 300;
const MAX_RISCV_O3_WRITEBACK_WAKE_LINES: usize = 800;
const MAX_RISCV_DATA_ISSUE_TEST_ROOT_LINES: usize = 1500;
const MAX_RISCV_DATA_ISSUE_LIFECYCLE_TEST_LINES: usize = 450;
const MAX_RISCV_FAILURE_DIAGNOSTIC_LINES: usize = 300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn cpu_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused CPU modules, but it has {lines} lines"
    );
}

#[test]
fn riscv_failure_diagnostics_use_a_focused_snapshot_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let owner = crate_dir.join("src/riscv_failure_diagnostic.rs");

    assert!(lib
        .lines()
        .any(|line| line.trim() == "mod riscv_failure_diagnostic;"));
    assert!(owner.is_file());
    assert!(
        fs::read_to_string(owner).unwrap().lines().count() <= MAX_RISCV_FAILURE_DIAGNOSTIC_LINES
    );
}

#[test]
fn riscv_data_issue_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let data_issue_rs = crate_dir.join("src/riscv_data_issue.rs");

    assert!(
        data_issue_rs.exists(),
        "RISC-V data issue code belongs in src/riscv_data_issue.rs"
    );
    assert!(
        !lib_rs.contains("fn prepare_data_access"),
        "src/lib.rs should delegate RISC-V data issue preparation to a focused module"
    );
    assert!(
        !lib_rs.contains("struct OutstandingDataAccess"),
        "src/lib.rs should delegate RISC-V data access records to a focused module"
    );
}

#[test]
fn riscv_data_issue_lifecycle_tests_live_in_focused_child() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/riscv_data_issue_tests.rs");
    let child_path = crate_dir.join("src/riscv_data_issue_tests/lifecycle.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert!(
        root.lines().count() < MAX_RISCV_DATA_ISSUE_TEST_ROOT_LINES,
        "src/riscv_data_issue_tests.rs must stay below {MAX_RISCV_DATA_ISSUE_TEST_ROOT_LINES} lines"
    );
    assert!(
        root.contains("#[path = \"riscv_data_issue_tests/lifecycle.rs\"]\nmod lifecycle;"),
        "RISC-V data-issue lifecycle tests must be delegated to the focused child"
    );
    assert!(
        child_path.exists(),
        "RISC-V data-issue lifecycle tests belong in src/riscv_data_issue_tests/lifecycle.rs"
    );
    let child = fs::read_to_string(child_path).unwrap();
    assert!(
        child.lines().count() <= MAX_RISCV_DATA_ISSUE_LIFECYCLE_TEST_LINES,
        "src/riscv_data_issue_tests/lifecycle.rs exceeds {MAX_RISCV_DATA_ISSUE_LIFECYCLE_TEST_LINES} lines"
    );
    for anchor in [
        "fn retry_response_discards_pending_o3_trace_data_access_outcome()",
        "fn control_boundary_after_stats_reset_discards_pending_o3_data_access_outcome()",
        "fn detailed_scalar_load_submission_stages_live_o3_rob_and_lsq_rows()",
        "fn assert_mode_disable_preserves_dependent_scalar_load_younger_wakeup_timing(",
    ] {
        assert!(
            child.contains(anchor),
            "focused lifecycle child is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "data-issue test root still owns `{anchor}`"
        );
    }
}

#[test]
fn riscv_data_completion_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = production_rust_source(&fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap());
    let data_issue = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_data_issue.rs")).unwrap(),
    );
    let request_helpers = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_data_issue/request_helpers.rs")).unwrap(),
    );
    let completion_path = crate_dir.join("src/riscv_data_completion.rs");

    assert!(
        lib_rs.contains("mod riscv_data_completion;"),
        "src/lib.rs must declare the focused RISC-V data completion module"
    );
    assert!(
        completion_path.exists(),
        "RISC-V data response application belongs in src/riscv_data_completion.rs"
    );
    let completion = production_rust_source(&fs::read_to_string(completion_path).unwrap());
    for anchor in [
        "pub(crate) struct RiscvDataCompletion",
        "pub(crate) fn apply_data_completion(",
        "fn scatter_segment_load(",
        "fn scatter_strided_load(",
        "fn scatter_indexed_load(",
        "fn read_vector_register_group(",
        "fn write_vector_register_group(",
    ] {
        assert!(
            completion.contains(anchor),
            "src/riscv_data_completion.rs is missing completion owner `{anchor}`"
        );
        assert!(
            !data_issue.contains(anchor),
            "src/riscv_data_issue.rs still owns completion detail `{anchor}`"
        );
    }
    assert!(
        !data_issue.contains("fn record_load_completion("),
        "src/riscv_data_issue.rs should delegate response application to src/riscv_data_completion.rs"
    );
    for anchor in [
        "normalized_masked_load_data",
        "normalized_masked_indexed_load_data",
        "normalized_masked_strided_load_data",
    ] {
        assert!(
            production_defines_exact_function(&completion, anchor),
            "src/riscv_data_completion.rs is missing vector completion normalization owner `{anchor}`"
        );
    }
    for anchor in [
        "normalized_masked_load_data",
        "normalized_masked_indexed_load_data",
        "normalized_masked_strided_load_data",
    ] {
        assert!(
            !data_issue.contains(anchor),
            "src/riscv_data_issue.rs must not import or re-export completion normalization `{anchor}`"
        );
        assert!(
            !request_helpers.contains(anchor),
            "src/riscv_data_issue/request_helpers.rs must not own completion normalization `{anchor}`"
        );
    }
}

#[test]
fn in_order_pipeline_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let in_order_rs = crate_dir.join("src/in_order_pipeline.rs");
    let in_order_src = fs::read_to_string(&in_order_rs).unwrap();

    assert!(
        in_order_rs.exists(),
        "in-order pipeline policy code belongs in src/in_order_pipeline.rs"
    );
    assert!(
        in_order_src.contains("pub enum InOrderPipelineStage"),
        "src/in_order_pipeline.rs should own the in-order stage model"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineScheduler"),
        "src/in_order_pipeline.rs should own the in-order scheduler"
    );
    assert!(
        in_order_src.contains("pub struct InOrderBranchRedirect"),
        "src/in_order_pipeline.rs should own in-order branch redirect evidence"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineSnapshot"),
        "src/in_order_pipeline.rs should own in-order snapshot state"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineState"),
        "src/in_order_pipeline.rs should own in-order pipeline state"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineCycleRecord"),
        "src/in_order_pipeline.rs should own in-order cycle records"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineCycleSummary"),
        "src/in_order_pipeline.rs should own in-order cycle summaries"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineRunSummary"),
        "src/in_order_pipeline.rs should own in-order run summaries"
    );
    assert!(
        in_order_src.contains("pub struct InOrderPipelineCheckpointPayload"),
        "src/in_order_pipeline.rs should own in-order checkpoint payloads"
    );
    assert!(
        !lib_rs.contains("pub struct InOrderPipelineScheduler"),
        "src/lib.rs should re-export the in-order scheduler from a focused module"
    );
}

#[test]
fn normal_riscv_drivers_delegate_pipeline_time_to_focused_scheduler_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let drive = fs::read_to_string(crate_dir.join("src/riscv_drive.rs")).unwrap();
    let translated = fs::read_to_string(crate_dir.join("src/riscv_translation.rs")).unwrap();
    let cluster_drive = fs::read_to_string(crate_dir.join("src/riscv_cluster_drive.rs")).unwrap();
    let timing = fs::read_to_string(crate_dir.join("src/riscv_in_order_drive.rs")).unwrap();
    let in_order = fs::read_to_string(crate_dir.join("src/in_order_pipeline.rs")).unwrap();
    let enqueue = source_section(&in_order, "pub fn enqueue_fetch(", "pub fn plan_cycle(");

    assert!(timing.contains("fn schedule_reserved_pipeline_cycle("));
    assert!(timing.contains("pub(crate) enum RiscvInOrderFetchAdmission"));
    assert!(timing.contains("pub(crate) fn in_order_fetch_admission("));
    assert!(timing.contains("try_advance_cycle_recorded_without_retirement"));
    assert!(drive.contains("schedule_next_completed_fetch_pipeline_cycle_serial"));
    assert!(translated.contains("schedule_next_completed_fetch_pipeline_cycle_serial"));
    assert!(cluster_drive.contains("schedule_next_completed_fetch_pipeline_cycle_parallel"));
    assert!(!in_order.contains("enqueue_fetch_recorded"));
    assert!(!enqueue.contains("advance_cycle"));
    assert!(!drive.contains("execute_next_completed_fetch_serial("));
    assert!(!translated.contains("execute_next_completed_fetch_serial("));
    assert!(!cluster_drive.contains("execute_next_completed_fetch_parallel("));
}

#[test]
fn riscv_fu_latency_has_one_typed_owner_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let latency_path = crate_dir.join("src/riscv_fu_latency.rs");
    let latency = fs::read_to_string(&latency_path).unwrap();
    let execute = fs::read_to_string(crate_dir.join("src/riscv_execute.rs")).unwrap();
    let drive = fs::read_to_string(crate_dir.join("src/riscv_in_order_drive.rs")).unwrap();
    let data_issue = fs::read_to_string(crate_dir.join("src/riscv_data_issue.rs")).unwrap();
    let o3_runtime = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let o3_stats = fs::read_to_string(crate_dir.join("src/o3_runtime_stats.rs")).unwrap();

    assert!(latency_path.exists());
    assert!(latency.contains("pub(crate) enum RiscvFuLatencyOwner"));
    assert!(latency.contains("pub(crate) const fn riscv_fu_latency("));
    assert!(latency.contains("riscv_pipeline_execute_wait_cycles"));
    assert!(latency.contains("riscv_data_completion_execute_wait_cycles"));
    assert!(latency.contains("riscv_o3_fu_latency_class"));
    assert!(drive.contains("riscv_pipeline_execute_wait_cycles"));
    assert!(data_issue.contains("riscv_data_completion_execute_wait_cycles"));
    assert!(o3_runtime.contains("riscv_o3_fu_latency_class as o3_fu_latency_class"));
    assert!(o3_stats.contains("riscv_o3_fu_latency_class as o3_fu_latency_class"));
    assert!(!execute.contains("scheduled_in_order_execute_wait_cycles"));
    assert!(!execute.contains("SCALAR_INTEGER_MUL_CYCLES"));
    assert!(!crate_dir.join("src/o3_fu_latency.rs").exists());
}

#[test]
fn in_order_redirect_flush_authority_stays_in_pipeline_plan() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/in_order_pipeline.rs")).unwrap();
    let branch_record = source_section(
        &source,
        "pub struct InOrderBranchPredictionRecord {",
        "pub struct InOrderPipelineAdvance {",
    );
    let plan = source_section(
        &source,
        "pub struct InOrderPipelinePlan {",
        "pub struct InOrderPipelineSnapshot {",
    );
    let cycle_summary = source_section(
        &source,
        "pub struct InOrderPipelineCycleSummary {",
        "pub struct InOrderPipelineRunSummary {",
    );
    let prediction_cycle = source_section(
        &source,
        "pub fn try_advance_cycle_recorded_with_prediction(",
        "pub(crate) fn try_advance_cycle_recorded_retiring_sequence(",
    );
    let retiring_cycle = source_section(
        &source,
        "pub(crate) fn try_advance_cycle_recorded_retiring_sequence(",
        "pub fn try_advance_cycle(",
    );

    assert!(
        !branch_record.contains("flushed:") && !branch_record.contains("fn flushed("),
        "branch-prediction records must not duplicate scheduler-owned flushed rows"
    );
    assert!(
        branch_record.contains("from_prediction")
            && branch_record.contains("prediction.repair_target_pc()?"),
        "branch-prediction repair evidence must derive from the prediction itself"
    );
    assert!(
        !source.contains("InOrderBranchPredictionRecord::from_plan"),
        "recorded cycles must not rebuild branch evidence from the scheduler plan"
    );
    for constructor in [prediction_cycle, retiring_cycle] {
        assert!(
            constructor.contains(".map(InOrderBranchPredictionRecord::from_prediction)")
                && constructor.contains(".transpose()?"),
            "recorded branch-prediction cycles must construct evidence from the prediction"
        );
    }
    for anchor in [
        "pub fn redirect_cause(",
        "pub fn flush_cause(",
        "pub fn flushed_for_cause(",
    ] {
        assert!(
            plan.contains(anchor),
            "in-order pipeline plan is missing redirect/flush authority `{anchor}`"
        );
    }
    for anchor in [
        "redirect_cause: Option<InOrderPipelineRedirectCause>",
        "flush_cause: Option<InOrderPipelineRedirectCause>",
    ] {
        assert!(
            cycle_summary.contains(anchor),
            "in-order cycle summary is missing typed cause `{anchor}`"
        );
    }
}

#[test]
fn cpu_source_files_stay_within_size_limit() {
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
fn o3_pending_checkpoint_payload_stays_in_codec_boundary() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let allowed = [
        "src/o3_pipeline.rs",
        "src/o3_runtime_checkpoint.rs",
        "src/o3_runtime_helpers.rs",
        "src/public_api.rs",
    ];

    for path in rust_source_files(&crate_dir.join("src")) {
        let source = fs::read_to_string(&path).unwrap();
        let relative = path.strip_prefix(crate_dir).unwrap().to_string_lossy();
        if !source.contains("O3PendingStateCheckpointPayload") {
            continue;
        }
        assert!(
            allowed.contains(&relative.as_ref()),
            "O3 pending checkpoint payload must stay in codec and runtime-composition modules, but {relative} depends on it"
        );
    }
}

#[test]
fn o3_runtime_memory_lifecycle_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/o3_runtime.rs");
    let root = fs::read_to_string(root_path).unwrap();

    assert!(
        root.contains("mod o3_runtime_memory;"),
        "src/o3_runtime.rs must delegate live data-access lifecycle state to src/o3_runtime_memory.rs"
    );

    let module_path = crate_dir.join("src/o3_runtime_memory.rs");
    assert!(
        module_path.exists(),
        "live data-access lifecycle code belongs in src/o3_runtime_memory.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let lines = module.lines().count();
    assert!(
        lines <= MAX_O3_RUNTIME_MEMORY_LINES,
        "src/o3_runtime_memory.rs exceeds {MAX_O3_RUNTIME_MEMORY_LINES} lines: {lines}"
    );

    for anchor in [
        "struct O3LiveDataAccess",
        "enum O3DataAccessWindowPolicy",
        "younger_window_policy: O3DataAccessWindowPolicy",
        "memory_result: Option<RiscvDataCompletion>",
        "fn o3_memory_result_destination",
        "fn stage_live_data_access_issue",
        "fn complete_live_data_access_completion",
        "fn complete_live_data_access_response",
        "fn ready_live_memory_result_completion",
        "fn take_ready_live_data_access_event",
        "fn consume_live_data_access_retirement",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_memory.rs is missing lifecycle owner `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "src/o3_runtime.rs still owns live data-access lifecycle `{anchor}`"
        );
    }
}

#[test]
fn task3_sequence_span_lsq_span_and_retire_ownership_stay_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let memory = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_memory.rs")).unwrap(),
    );
    let retire = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_retire.rs")).unwrap(),
    );

    for anchor in [
        "pub(super) fn o3_instruction_sequence_span(",
        "self.allocate_sequence_span(lsq_sequence_span)",
    ] {
        assert!(
            memory.contains(anchor),
            "src/o3_runtime_memory.rs is missing Task 3 memory-sequence owner `{anchor}`"
        );
    }
    for anchor in [
        "pub(super) fn allocate_sequence_span(&mut self, span: u64) -> u64",
        "self.allocate_sequence_span(o3_instruction_sequence_span(record.memory_access()))",
        "self.remove_live_data_access_rows(live.sequence, live.lsq_sequence_span)",
    ] {
        assert!(
            retire.contains(anchor),
            "src/o3_runtime_retire.rs is missing Task 3 retire-sequence owner `{anchor}`"
        );
    }
    assert!(
        !retire.contains("Some(MemoryAccessKind::AtomicMemory"),
        "src/o3_runtime_retire.rs must not keep a legacy separate AMO sequence increment"
    );
}

#[test]
fn generic_o3_live_data_owner_uses_data_access_names() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stale = [
        "O3LiveScalarMemory",
        "O3LiveScalarMemoryOutcome",
        "live_scalar_memories",
        "deferred_scalar_memory_execution",
        "is_deferred_o3_scalar_memory_access",
        "is_deferred_o3_scalar_memory_instruction",
        "is_terminal_o3_scalar_memory_event",
        "stage_live_scalar_memory_issue",
        "complete_live_scalar_memory_response",
        "take_ready_live_scalar_memory_event",
        "consume_live_scalar_memory_retirement",
        "record_ready_o3_scalar_memory_event_with_trace",
        "has_pending_scalar_memory_retirement",
        "pending_scalar_memory_retirement_count",
        "owns_pending_scalar_memory_retirement",
        "defer_scalar_memory_execution",
        "defer_scalar_memory_if_detailed",
        "abort_deferred_scalar_memory_execution",
        "clear_deferred_scalar_memory_execution",
        "has_live_scalar_memory",
        "has_live_scalar_memory_window",
        "has_ready_live_scalar_memory_event",
        "earliest_unpublished_scalar_load_writeback_tick",
        "ready_live_scalar_memory_event_kind",
        "ready_live_scalar_memory_completion_timing",
        "replace_ready_live_scalar_memory_execution",
        "live_scalar_memory_publication_is_admitted",
        "ready_live_scalar_load_writeback",
        "discard_live_scalar_memory_lifecycle",
        "discard_live_scalar_memory_window_rows",
        "discard_live_scalar_memory_window_rows_at",
        "remove_live_scalar_memory_rows",
        "o3_scalar_memory_lifecycle_is_quiescent",
        "has_pending_o3_scalar_memory_retirement",
        "pending_o3_scalar_memory_retirement_count",
        "owns_pending_o3_scalar_memory_retirement",
        "ready_o3_scalar_memory_event_kind",
        "clear_deferred_o3_scalar_memory_execution",
        "deferred_o3_scalar_memory_retirement",
        "completed_live_scalar_memory",
        "live_scalar_memory: Option<&O3LiveDataAccess>",
        "let live_scalar_memory",
        "completed O3 scalar memory",
        "completed live scalar memory",
        "taken live scalar memory",
        "pending_retirement_tracks_deferred_and_live_scalar_memory",
        "stages_two_live_scalar_memory_rows",
        "rejects_live_scalar_memory",
        "apply_deferred_scalar_load_writeback",
        "live_scalar_memory_younger_sequences",
        "stage_live_scalar_memory_younger_window",
        "live_scalar_memory_younger_wakeup_seed",
        "retain_live_scalar_memory_younger_sequences_in_rob",
        "stage_o3_scalar_memory_younger_window",
        "wake_o3_scalar_memory_younger_window",
        "wake_ready_o3_scalar_memory_younger_window",
        "live_scalar_memory_head_reservation",
        "scalar_memory_integer_window",
        "completed_live_scalar_load_source",
        "allows_detailed_memory_head_fetch_ahead",
        "scalar_memory_has_younger_fetch",
        "scalar_memory_waits_for_younger_fetch",
        "translated_scalar_load_result_fetch_ahead_allowed",
        "cached_translated_scalar_load_head_physical_range",
        "next_mmio_aware_cached_translated_memory_fetch_ahead_before_retire",
        "can_retire_completed_fetch_while_mmio_aware_cached_translated_memory_fetch_pending",
        "scalar_load_result_head_targets_mmio",
    ];
    let rem6_system_dir = crate_dir
        .parent()
        .expect("rem6-cpu has a workspace crates parent")
        .join("rem6-system");
    let roots = [
        crate_dir.join("src"),
        crate_dir.join("tests"),
        rem6_system_dir.join("src"),
        rem6_system_dir.join("tests"),
    ];
    let policy_path = crate_dir.join("tests/source_policy.rs");
    let offenders = roots
        .into_iter()
        .flat_map(|root| rust_source_files(&root))
        .filter(|path| path != &policy_path)
        .filter_map(|path| {
            let source = fs::read_to_string(&path).unwrap();
            let names = stale
                .iter()
                .filter(|name| source.contains(**name))
                .copied()
                .collect::<Vec<_>>();
            (!names.is_empty()).then_some((path, names))
        })
        .collect::<Vec<_>>();
    assert!(
        offenders.is_empty(),
        "stale generic live-data names: {offenders:?}"
    );
}

#[test]
fn o3_data_access_younger_window_has_focused_owners() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let memory_window =
        fs::read_to_string(crate_dir.join("src/o3_runtime_memory_window.rs")).unwrap();
    let live_window = fs::read_to_string(crate_dir.join("src/o3_runtime_live_window.rs")).unwrap();
    let issue = fs::read_to_string(crate_dir.join("src/o3_runtime_issue.rs")).unwrap();
    let data_issue = fs::read_to_string(crate_dir.join("src/riscv_data_issue.rs")).unwrap();
    let fetch_ahead =
        fs::read_to_string(crate_dir.join("src/riscv_fetch_ahead/detailed_o3.rs")).unwrap();
    let fetch_driver =
        fs::read_to_string(crate_dir.join("src/riscv_fetch_ahead/driver.rs")).unwrap();
    let cluster_translation =
        fs::read_to_string(crate_dir.join("src/riscv_cluster_translation.rs")).unwrap();

    for (owner, anchor) in [
        (&memory_window, "pub(crate) fn data_access_integer_window("),
        (
            &live_window,
            "pub(crate) fn stage_live_data_access_younger_window(",
        ),
        (&issue, "pub(crate) fn live_data_access_head_reservation("),
        (&data_issue, "O3DataAccessWindowPolicy::ScalarMemoryPrefix"),
        (&fetch_ahead, "fn data_access_result_fetch_ahead_shape("),
        (&fetch_ahead, "fn data_access_result_head_probe("),
        (&fetch_ahead, "masked_vector_memory_request_span("),
        (&fetch_ahead, "fault_only_first: false"),
        (
            &fetch_ahead,
            "pub(super) fn data_access_result_head_physical_probe(",
        ),
        (
            &fetch_ahead,
            "pub(super) fn data_access_waits_for_younger_fetch(",
        ),
        (
            &fetch_driver,
            "pub(crate) fn next_mmio_aware_fetch_ahead_before_retire(",
        ),
        (&fetch_driver, "enum DataAccessResultHeadRoute"),
        (
            &cluster_translation,
            "pub(crate) fn can_retire_mmio_fetch_pending(",
        ),
    ] {
        assert!(
            owner.contains(anchor),
            "missing data-access window owner `{anchor}`"
        );
    }
}

#[test]
fn producer_forwarded_pending_data_escape_is_fetch_only() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = production_rust_source(&fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap());
    let pending_data_gate = source_section(
        &root,
        "    pub(crate) fn pending_data_access_blocks_new_work(&self) -> bool {",
        "    pub fn data_access_lifecycle_is_quiescent(&self) -> bool {",
    );
    assert!(
        !pending_data_gate.contains("producer_forwarded"),
        "the global pending-data gate must not bypass unrelated work for producer-forwarded control"
    );

    let driver = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_fetch_ahead/driver.rs")).unwrap(),
    );
    let fetch = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_fetch_ahead.rs")).unwrap(),
    );
    let drive =
        production_rust_source(&fs::read_to_string(crate_dir.join("src/riscv_drive.rs")).unwrap());
    let cluster = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_cluster.rs")).unwrap(),
    );
    let direct_entry = "next_pending_data_fetch_ahead";
    let mmio_entry = "next_pending_data_mmio_fetch_ahead";
    for helper in [direct_entry, mmio_entry] {
        assert!(
            driver.contains(&format!("pub(crate) fn {helper}(")),
            "src/riscv_fetch_ahead/driver.rs must own `{helper}`"
        );
    }
    for legacy in [
        "next_producer_forwarded_fetch_ahead_before_retire",
        "next_mmio_aware_producer_forwarded_fetch_ahead_before_retire",
    ] {
        assert!(
            !driver.contains(&format!("fn {legacy}(")),
            "obsolete producer-forwarded fetch facade remains: `{legacy}`"
        );
    }
    let speculation = source_section(
        &fetch,
        "pub(crate) struct RiscvFetchAheadSpeculation {",
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\nenum ReturnAddressStackAction",
    );
    assert!(
        speculation.contains("target_authority: PredictedControlTargetAuthority"),
        "fetch speculation must carry one typed target authority"
    );
    for legacy in [
        "producer_forwarded_control_target:",
        "producer_forwarded_return_descendant:",
    ] {
        assert!(
            !speculation.contains(legacy),
            "parallel producer-forwarded authority field remains: `{legacy}`"
        );
    }
    let decision_filter = rust_function_definition(&driver, "producer_forwarded_control_decision")
        .expect("missing producer_forwarded_control_decision definition");
    for marker in [
        "producer_forwarded_scalar_continuation.is_some()",
        "PredictedControlTargetAuthority::ProducerForwarded(",
        "PredictedControlTargetAuthority::ProducerForwardedReturn(",
    ] {
        assert!(
            decision_filter.contains(marker),
            "the fetch-only decision filter must require carried authority marker `{marker}`"
        );
    }
    assert_eq!(
        driver.matches("(!self.has_pending_fetch())").count(),
        2,
        "both pending-data entry points must reject an already pending fetch before filtering"
    );
    assert!(
        driver
            .matches("producer_forwarded_control_decision(")
            .count()
            >= 3,
        "pending-data entry points must apply the carried-authority filter directly"
    );
    assert_eq!(
        drive.matches(direct_entry).count(),
        1,
        "the serial drive path must consume the focused producer-forwarded decision filter"
    );
    assert_eq!(
        cluster.matches(direct_entry).count(),
        2,
        "the two direct cluster drive variants must consume the focused decision filter"
    );
    assert_eq!(
        cluster.matches(mmio_entry).count(),
        2,
        "the two MMIO cluster drive variants must consume the MMIO-aware decision filter"
    );
}

#[test]
fn disjoint_store_prefix_uses_ordered_store_ownership_not_overlap_connectivity() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let memory_window = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_memory_window.rs")).unwrap(),
    );
    let fetch_ahead = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_fetch_ahead/detailed_o3.rs")).unwrap(),
    );
    let scalar_window = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_scalar_memory_window.rs")).unwrap(),
    );

    for (owner, source) in [
        ("runtime memory window", &memory_window),
        ("detailed fetch-ahead", &fetch_ahead),
        ("scalar-memory helper", &scalar_window),
    ] {
        assert!(
            !source.contains("store_range_extends_overlap_prefix"),
            "{owner} must not restore the obsolete overlap-connectivity gate"
        );
    }
    for (owner, source) in [
        ("runtime memory window", &memory_window),
        ("detailed fetch-ahead", &fetch_ahead),
    ] {
        assert!(
            source.contains("RiscvInstruction::Store { .. } if destinations.is_empty()")
                || source
                    .contains("MemoryAccessKind::Store { .. } if load_destinations.is_empty()"),
            "{owner} must admit cacheable stores through the store-only prefix boundary"
        );
    }
    assert!(
        memory_window.contains("o3_store_load_composition("),
        "runtime load admission must keep byte composition in the focused forwarding owner"
    );
    let predecessor = rust_function_definition(&memory_window, "scalar_store_predecessor")
        .expect("missing scalar_store_predecessor definition");
    for anchor in [
        "self.live_data_accesses",
        ".last()",
        ".map(|store| store.data_request)",
    ] {
        assert!(
            predecessor.contains(anchor),
            "buffered store ordering must remain predecessor-based: missing `{anchor}`"
        );
    }
}

#[test]
fn o3_runtime_issue_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let module_path = crate_dir.join("src/o3_runtime_issue.rs");
    let live_retire =
        fs::read_to_string(crate_dir.join("src/riscv_live_retire_window.rs")).unwrap();

    assert!(
        root.contains("mod o3_runtime_issue;"),
        "src/o3_runtime.rs must declare the focused O3 issue module"
    );
    assert!(
        module_path.exists(),
        "live O3 issue scheduling belongs in src/o3_runtime_issue.rs"
    );
    let module = fs::read_to_string(&module_path).unwrap();
    let lines = module.lines().count();
    assert!(
        lines <= MAX_O3_RUNTIME_ISSUE_LINES,
        "src/o3_runtime_issue.rs exceeds {MAX_O3_RUNTIME_ISSUE_LINES} lines: {lines}"
    );

    let issue_authority_patterns = [
        "pub(crate) fn schedule_live_speculative_issues(",
        "O3ScopedIssueScheduler::new(",
        "self.stats.record_issue_cycle(",
        "pub(crate) fn live_data_access_head_reservation(",
        "O3LiveIssueHeadReservation::memory(",
    ];
    for anchor in issue_authority_patterns {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_issue.rs is missing issue-scheduler owner `{anchor}`"
        );
    }
    for anchor in issue_authority_patterns {
        for path in rust_source_files(&crate_dir.join("src")) {
            let relative = path.strip_prefix(crate_dir).unwrap();
            if relative == Path::new("src/o3_runtime_issue.rs") {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            assert!(
                !source.contains(anchor),
                "{} duplicates scoped issue authority `{anchor}`; keep it in src/o3_runtime_issue.rs",
                relative.display()
            );
        }
    }
    assert!(
        live_retire.contains(".schedule_live_speculative_issues("),
        "src/riscv_live_retire_window.rs must delegate live younger issue scheduling"
    );
    assert!(
        !live_retire.contains("O3ScopedIssueScheduler"),
        "src/riscv_live_retire_window.rs must not construct the scoped issue scheduler"
    );
}

#[test]
fn o3_runtime_writeback_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/o3_runtime.rs");
    let root = production_rust_source(&fs::read_to_string(&root_path).unwrap());
    let module_path = crate_dir.join("src/o3_runtime_writeback.rs");
    let module = production_rust_source(&fs::read_to_string(&module_path).unwrap());
    let replan_path = crate_dir.join("src/o3_runtime_writeback/replan.rs");
    let replan = production_rust_source(&fs::read_to_string(&replan_path).unwrap());
    let ownership_path = crate_dir.join("src/o3_runtime_writeback/ownership.rs");
    let ownership = production_rust_source(&fs::read_to_string(&ownership_path).unwrap());
    let issue = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_issue.rs")).unwrap(),
    );
    let live_retire = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_live_retire_window.rs")).unwrap(),
    );
    let memory = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_memory.rs")).unwrap(),
    );
    let live_window = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_live_window.rs")).unwrap(),
    );

    assert!(
        root.contains("mod o3_runtime_writeback;"),
        "src/o3_runtime.rs must declare the focused O3 writeback module"
    );
    assert!(
        module_path.exists(),
        "live O3 writeback reservation belongs in src/o3_runtime_writeback.rs"
    );
    assert!(
        module.contains("mod replan;"),
        "src/o3_runtime_writeback.rs must delegate transactional replanning to its focused child module"
    );
    assert!(
        module.contains("mod ownership;"),
        "src/o3_runtime_writeback.rs must delegate finalized/live statistics ownership to its focused child module"
    );
    assert!(
        replan_path.exists(),
        "transactional O3 writeback replanning belongs in src/o3_runtime_writeback/replan.rs"
    );
    assert!(
        ownership_path.exists(),
        "finalized O3 writeback statistics ownership belongs in src/o3_runtime_writeback/ownership.rs"
    );
    let root_lines = line_count(&root_path);
    assert!(
        root_lines < MAX_O3_RUNTIME_ROOT_LINES,
        "src/o3_runtime.rs must keep strict headroom below {MAX_O3_RUNTIME_ROOT_LINES} lines, but it has {root_lines} lines"
    );
    let module_lines = line_count(&module_path);
    assert!(
        module_lines < MAX_O3_RUNTIME_WRITEBACK_LINES,
        "src/o3_runtime_writeback.rs must stay below {MAX_O3_RUNTIME_WRITEBACK_LINES} lines, but it has {module_lines} lines"
    );
    let replan_lines = line_count(&replan_path);
    assert!(
        replan_lines < MAX_O3_RUNTIME_WRITEBACK_REPLAN_LINES,
        "src/o3_runtime_writeback/replan.rs must stay below {MAX_O3_RUNTIME_WRITEBACK_REPLAN_LINES} lines, but it has {replan_lines} lines"
    );
    let ownership_lines = line_count(&ownership_path);
    assert!(
        ownership_lines < MAX_O3_RUNTIME_WRITEBACK_OWNERSHIP_LINES,
        "src/o3_runtime_writeback/ownership.rs must stay below {MAX_O3_RUNTIME_WRITEBACK_OWNERSHIP_LINES} lines, but it has {ownership_lines} lines"
    );
    assert!(
        ownership.contains("struct O3FinalizedWritebackPortStats"),
        "src/o3_runtime_writeback/ownership.rs must own finalized writeback statistics"
    );
    assert!(
        ownership.contains("fn finalize_live_writeback_ownership("),
        "src/o3_runtime_writeback/ownership.rs must own exact finalized/live ownership transfer"
    );

    for anchor in [
        "pub(super) fn reserve_writeback_completions_in_place(",
        "fn speculative_descendants(",
        "fn invalidate_speculative_descendants(",
        "fn sync_writeback_reservation_owners(",
    ] {
        assert!(
            replan.contains(anchor),
            "src/o3_runtime_writeback/replan.rs is missing transactional replanning owner `{anchor}`"
        );
    }

    let writeback_authority_patterns = [
        "struct O3WritebackReservationCalendar",
        "fn reserve_writeback_completions<I>(",
        "fn discard_future_writeback_from_sequence(",
    ];
    for anchor in writeback_authority_patterns {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_writeback.rs is missing writeback owner `{anchor}`"
        );
    }
    for anchor in writeback_authority_patterns {
        for path in rust_source_files(&crate_dir.join("src")) {
            let relative = path.strip_prefix(crate_dir).unwrap();
            if relative == Path::new("src/o3_runtime_writeback.rs")
                || is_test_only_rust_source(relative)
            {
                continue;
            }
            let source = production_rust_source(&fs::read_to_string(&path).unwrap());
            assert!(
                !source.contains(anchor),
                "{} duplicates O3 writeback authority `{anchor}`; keep it in src/o3_runtime_writeback.rs",
                relative.display()
            );
        }
    }
    for anchor in [
        "fn set_writeback_port_schedule(",
        "observe_finalized_schedule(",
        "reconcile_live_schedule(",
        "close_all_reopenable_ticks(",
    ] {
        assert!(
            module.contains(anchor) || ownership.contains(anchor),
            "focused writeback statistics ownership is missing `{anchor}`"
        );
    }

    assert!(
        live_retire.contains(".record_live_issue_head_execution("),
        "src/riscv_live_retire_window.rs must delegate fixed-FU issue and writeback reservation"
    );
    assert!(
        issue.contains(".reserve_fixed_fu_writeback("),
        "src/o3_runtime_issue.rs must forward fixed-FU completions to focused writeback reservation"
    );
    assert!(
        memory.contains(".reserve_writeback_completions("),
        "src/o3_runtime_memory.rs must delegate scalar-load completion to focused writeback reservation"
    );
    for anchor in [
        "fn discard_live_writeback_reservations(",
        "fn discard_live_writeback_from_sequence(",
        "fn finalize_all_writeback_reservations(",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_writeback.rs is missing cleanup owner `{anchor}`"
        );
    }
    assert!(
        !module.contains("discard_all_writeback_reservations"),
        "live lifecycle cleanup must not expose a blind writeback-calendar discard alias"
    );
    assert!(
        live_window.contains(".discard_live_writeback_reservations()")
            && memory.contains(".discard_live_writeback_reservations()"),
        "live window and data lifecycle cleanup must preserve published writeback occupancy"
    );
    assert!(
        replan.contains("WritebackReservationTickClosed")
            && replan.contains("published_writeback_sequences"),
        "transactional replanning must enforce the close watermark and retain published slots"
    );
    for (relative, source) in [
        ("src/riscv_live_retire_window.rs", live_retire),
        ("src/o3_runtime_memory.rs", memory),
    ] {
        for forbidden in [
            "O3WritebackReservationCalendar",
            "O3WritebackTransferBuffer",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must not construct focused or generic writeback authority `{forbidden}`"
            );
        }
    }
}

#[test]
fn task3_writeback_reservation_uses_bounded_transaction_state() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let writeback = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_writeback.rs")).unwrap(),
    );
    let replan = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_writeback/replan.rs")).unwrap(),
    );
    let ownership = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_writeback/ownership.rs")).unwrap(),
    );
    let reservation = source_section(
        &writeback,
        "pub(crate) fn reserve_writeback_completions<I>(",
        "pub(crate) fn reserve_fixed_fu_writeback(",
    );

    assert!(
        !reservation.contains("self.clone()"),
        "writeback reservation must not clone the full O3 runtime"
    );
    assert!(
        reservation.contains("O3WritebackReplanTransaction::capture(self)"),
        "writeback reservation must capture a focused bounded transaction"
    );
    for anchor in [
        "struct O3WritebackReplanTransaction",
        "fn capture(runtime: &O3RuntimeState)",
        "fn commit(self, runtime: &mut O3RuntimeState)",
        "fn reserve_writeback_completions_in_place(",
    ] {
        assert!(
            replan.contains(anchor),
            "bounded writeback transaction is missing `{anchor}`"
        );
    }
    assert!(
        ownership.contains("struct O3FinalizedWritebackPortStats"),
        "focused writeback owner must separate finalized maxima from the live schedule"
    );
    let transaction = source_section(
        &replan,
        "struct O3WritebackReplanTransaction",
        "impl O3WritebackReplanTransaction",
    );
    assert!(
        !transaction.contains("trace_records"),
        "bounded writeback transaction must not own trace history"
    );
    for anchor in [
        "fn finalize_writeback_reservations_before(",
        "fn discard_writeback_reservations(",
        "fn rebuild_live_writeback_schedule_ownership(",
    ] {
        assert!(
            writeback.contains(anchor),
            "focused writeback ownership is missing bounded cleanup `{anchor}`"
        );
    }
    assert!(
        ownership.contains("fn observe_finalized_schedule(")
            && ownership.contains("fn reconcile_live_schedule("),
        "focused writeback owner must partition finalized and bounded live schedule ownership"
    );
}

#[test]
fn riscv_o3_writeback_wake_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = production_rust_source(&fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap());
    let module_path = crate_dir.join("src/riscv_o3_writeback_wake.rs");
    let module = production_rust_source(&fs::read_to_string(&module_path).unwrap());

    assert!(
        lib.contains("mod riscv_o3_writeback_wake;"),
        "src/lib.rs must declare the focused RISC-V O3 writeback wake module"
    );
    let module_lines = line_count(&module_path);
    assert!(
        module_lines < MAX_RISCV_O3_WRITEBACK_WAKE_LINES,
        "src/riscv_o3_writeback_wake.rs must stay below {MAX_RISCV_O3_WRITEBACK_WAKE_LINES} lines, but it has {module_lines} lines"
    );

    let wake_authority_patterns = [
        "struct RiscvO3WritebackWakeState",
        "desired_tick: Option<Tick>",
        "scheduled: Option<RiscvO3WritebackWake>",
        "detached: Vec<RiscvO3WritebackWake>",
        "fn set_desired_tick(",
        "if let Some(wake) = self.scheduled.take()",
        "self.detached.push(wake);",
    ];
    for anchor in wake_authority_patterns {
        assert!(
            module.contains(anchor),
            "src/riscv_o3_writeback_wake.rs is missing wake owner `{anchor}`"
        );
        for path in rust_source_files(&crate_dir.join("src")) {
            let relative = path.strip_prefix(crate_dir).unwrap();
            if relative == Path::new("src/riscv_o3_writeback_wake.rs")
                || is_test_only_rust_source(relative)
            {
                continue;
            }
            let source = production_rust_source(&fs::read_to_string(&path).unwrap());
            assert!(
                !source.contains(anchor),
                "{} duplicates RISC-V O3 writeback wake authority `{anchor}`",
                relative.display()
            );
        }
    }
}

#[test]
fn o3_writeback_transfer_planning_stays_in_generic_pipeline_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pipeline_path = crate_dir.join("src/o3_pipeline.rs");
    let pipeline = production_rust_source(&fs::read_to_string(&pipeline_path).unwrap());
    let runtime_writeback = [
        production_rust_source(
            &fs::read_to_string(crate_dir.join("src/o3_runtime_writeback.rs")).unwrap(),
        ),
        production_rust_source(
            &fs::read_to_string(crate_dir.join("src/o3_runtime_writeback/replan.rs")).unwrap(),
        ),
    ]
    .join("\n");
    let planner = "pub fn plan_cycle_with_occupied_slots";

    assert_eq!(
        pipeline.matches(planner).count(),
        1,
        "src/o3_pipeline.rs must be the sole generic occupied-slot planner owner"
    );
    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if relative == Path::new("src/o3_pipeline.rs") || is_test_only_rust_source(relative) {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        assert!(
            !source.contains(planner),
            "{} duplicates generic writeback transfer planning",
            relative.display()
        );
    }
    assert!(
        runtime_writeback.contains(".plan_cycle_with_occupied_slots("),
        "src/o3_runtime_writeback.rs must delegate occupied-slot planning to src/o3_pipeline.rs"
    );
    for anchor in [
        "O3PipelineError::DuplicateWritebackOccupiedSlot",
        "O3PipelineError::WritebackOccupiedSlotOutOfRange",
        "occupied_slots.windows(2)",
        "while let Some(completion) = self.deferred.pop_front()",
        "ordered.extend(new_ready)",
    ] {
        assert!(
            pipeline.contains(anchor),
            "src/o3_pipeline.rs is missing generic planner invariant `{anchor}`"
        );
        assert!(
            !runtime_writeback.contains(anchor),
            "src/o3_runtime_writeback.rs must not reimplement generic planner invariant `{anchor}`"
        );
    }
    for runtime_authority in ["O3RuntimeState", "O3WritebackReservationCalendar"] {
        assert!(
            !pipeline.contains(runtime_authority),
            "src/o3_pipeline.rs must remain generic rather than owning RISC-V runtime authority `{runtime_authority}`"
        );
    }
}

#[test]
fn o3_runtime_control_window_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let live = fs::read_to_string(crate_dir.join("src/o3_runtime_live_window.rs")).unwrap();
    let module_path = crate_dir.join("src/o3_runtime_control_window.rs");

    assert!(
        root.contains("mod o3_runtime_control_window;"),
        "src/o3_runtime.rs must delegate transient issue state to src/o3_runtime_control_window.rs"
    );
    assert!(
        module_path.exists(),
        "transient O3 issue state belongs in src/o3_runtime_control_window.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    for anchor in [
        "struct O3LiveSpeculativeExecution",
        "struct O3LiveSpeculativeIssueCandidate",
        "fn live_speculative_issue_candidate",
        "fn record_live_speculative_execution",
        "fn live_speculative_source_forwarding",
        "fn invalidate_live_speculative_execution_chain",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_control_window.rs is missing control-window owner `{anchor}`"
        );
        assert!(
            !live.contains(anchor),
            "src/o3_runtime_live_window.rs still owns transient control-window state `{anchor}`"
        );
    }
}

#[test]
fn o3_live_control_operands_have_one_typed_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let owner_path = crate_dir.join("src/o3_source_operands.rs");
    let owner = production_rust_source(&fs::read_to_string(&owner_path).unwrap());

    for anchor in [
        "pub(crate) struct O3LiveIndirectControlTarget",
        "pub(crate) struct O3LiveControlOperands",
        "pub(crate) fn o3_live_control_operands(",
        "kind: BranchTargetKind",
        "sources: Vec<Register>",
        "destination: Option<Register>",
        "indirect_target: Option<O3LiveIndirectControlTarget>",
        "pub(crate) const fn destination(&self) -> Option<Register>",
        "pub(crate) const fn indirect_target(&self) -> Option<O3LiveIndirectControlTarget>",
    ] {
        assert!(
            owner.contains(anchor),
            "src/o3_source_operands.rs is missing live-control authority `{anchor}`"
        );
    }
    assert_eq!(
        owner
            .matches("pub(crate) fn o3_live_control_operands(")
            .count(),
        1,
        "src/o3_source_operands.rs must define one live-control classifier"
    );

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        assert!(
            !source.contains("o3_direct_conditional_sources"),
            "{} retains the obsolete conditional-only control helper",
            relative.display()
        );
        if relative != Path::new("src/o3_source_operands.rs") {
            assert!(
                !source.contains("struct O3LiveControlOperands"),
                "{} duplicates typed live-control operand ownership",
                relative.display()
            );
            assert!(
                !source.contains("struct O3LiveIndirectControlTarget")
                    && !source.contains("fn indirect_target(&self)"),
                "{} duplicates typed indirect control-target ownership",
                relative.display()
            );
        }
    }

    let consumers = [
        "src/riscv_o3_window_policy.rs",
        "src/o3_runtime_control_window.rs",
        "src/o3_runtime_issue.rs",
        "src/o3_runtime_live_window.rs",
    ];
    let opcode_inventory = [
        "RiscvInstruction::Beq",
        "RiscvInstruction::Bne",
        "RiscvInstruction::Blt",
        "RiscvInstruction::Bge",
        "RiscvInstruction::Bltu",
        "RiscvInstruction::Bgeu",
        "RiscvInstruction::Jal {",
        "RiscvInstruction::Jalr {",
    ];
    for relative in consumers {
        let source = production_rust_source(&fs::read_to_string(crate_dir.join(relative)).unwrap());
        assert!(
            source.contains("o3_live_control_operands"),
            "{relative} must consume the typed live-control authority"
        );
        for opcode in opcode_inventory {
            assert!(
                !source.contains(opcode),
                "{relative} duplicates live-control opcode inventory `{opcode}`"
            );
        }
    }

    let execute = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_execute.rs")).unwrap(),
    );
    let retire = source_section(
        &execute,
        "    fn retire_completed_fetch(",
        "fn next_completed_fetch_suffix<'a>(",
    );
    assert!(
        retire.contains("o3_live_control_operands(instruction)"),
        "RISC-V retirement must derive live-control cleanup from the typed authority"
    );
    for opcode in opcode_inventory {
        assert!(
            !retire.contains(opcode),
            "RISC-V retirement duplicates live-control opcode inventory `{opcode}`"
        );
    }
}

#[test]
fn o3_runtime_error_lives_in_focused_module_with_stable_public_exports() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root =
        production_rust_source(&fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap());
    let public_api =
        production_rust_source(&fs::read_to_string(crate_dir.join("src/public_api.rs")).unwrap());
    let module_path = crate_dir.join("src/o3_runtime_error.rs");

    assert!(
        root.contains("mod o3_runtime_error;"),
        "src/o3_runtime.rs must privately own the focused O3 runtime error module"
    );
    assert!(
        !root.contains("pub mod o3_runtime_error;"),
        "src/o3_runtime_error.rs must stay private behind the existing o3_runtime public path"
    );
    assert!(
        root.contains("pub use o3_runtime_error::O3RuntimeError;"),
        "src/o3_runtime.rs must preserve the public o3_runtime::O3RuntimeError path"
    );
    assert!(
        public_api.contains("O3RuntimeError"),
        "crate public API must continue re-exporting O3RuntimeError"
    );
    assert!(
        module_path.exists(),
        "O3 runtime error ownership belongs in src/o3_runtime_error.rs"
    );

    let module = production_rust_source(&fs::read_to_string(&module_path).unwrap());
    for anchor in [
        "pub enum O3RuntimeError",
        "impl fmt::Display for O3RuntimeError",
        "impl Error for O3RuntimeError",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_error.rs is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "src/o3_runtime.rs still owns `{anchor}`"
        );
    }
}

#[test]
fn o3_runtime_tests_live_in_sibling_test_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let module_path = crate_dir.join("src/o3_runtime_tests.rs");

    assert!(
        root.contains("#[cfg(test)]\n#[path = \"o3_runtime_tests.rs\"]\nmod o3_runtime_tests;"),
        "src/o3_runtime.rs must declare its sibling test-only module"
    );
    assert!(
        !root.contains("mod tests {"),
        "src/o3_runtime.rs must not keep the former inline cfg(test) tests body"
    );
    assert!(
        module_path.exists(),
        "former inline O3 runtime tests belong in src/o3_runtime_tests.rs"
    );

    let module = fs::read_to_string(&module_path).unwrap();
    for anchor in [
        "mod pending_data;",
        "fn o3_issue_width_defaults_to_shared_cpu_default(",
        "fn failed_store_conditional_stats_count_failed_operation(",
        "fn branch_repair_stats_checkpoint_round_trips_current_payload(",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_tests.rs is missing former inline test body anchor `{anchor}`"
        );
    }
}

#[test]
fn o3_runtime_live_window_tests_live_in_sibling_test_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let owner_path = crate_dir.join("src/o3_runtime_live_window.rs");
    let owner = fs::read_to_string(&owner_path).unwrap();
    let tests_path = crate_dir.join("src/o3_runtime_live_window_tests.rs");
    let identity_path = crate_dir.join("src/o3_runtime_live_window_identity_tests.rs");

    assert!(
        owner.contains("#[cfg(test)]\n#[path = \"o3_runtime_live_window_tests.rs\"]\nmod tests;"),
        "src/o3_runtime_live_window.rs must declare its sibling test-only module"
    );
    assert!(
        !owner.contains("mod tests {"),
        "src/o3_runtime_live_window.rs must not retain its inline test body"
    );
    assert!(
        line_count(&owner_path) < MAX_O3_RUNTIME_LIVE_WINDOW_LINES,
        "src/o3_runtime_live_window.rs must stay below {MAX_O3_RUNTIME_LIVE_WINDOW_LINES} lines"
    );
    assert!(
        tests_path.exists(),
        "live-window tests belong in src/o3_runtime_live_window_tests.rs"
    );

    let tests = fs::read_to_string(tests_path).unwrap();
    let tests_code = rust_code_without_comments_and_literals(&tests);
    let include_lines = include_macro_lines(&tests);
    assert!(
        include_lines.is_empty(),
        "src/o3_runtime_live_window_tests.rs must use path-owned child modules instead of include! fragments; found lines {include_lines:?}"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &tests,
            "o3_runtime_live_window_identity_tests.rs",
            "identity",
        ),
        1,
        "live-window identity tests must have exactly one attached path-owned child declaration"
    );
    assert!(
        tests.lines().count() <= MAX_O3_RUNTIME_LIVE_WINDOW_TEST_LINES,
        "src/o3_runtime_live_window_tests.rs exceeds {MAX_O3_RUNTIME_LIVE_WINDOW_TEST_LINES} lines"
    );
    for anchor in [
        "fn scalar_memory_stops_live_retire_window_before_memory_and_younger_rows(",
        "fn completed_live_load_forwards_into_dependent_alu_candidate(",
        "fn invalidated_speculative_producer_revokes_dependent_issue_timing(",
        "fn live_rename_overlay_preserves_canonical_register_order(",
    ] {
        assert!(
            tests_code.contains(anchor),
            "src/o3_runtime_live_window_tests.rs is missing `{anchor}`"
        );
    }

    assert!(
        identity_path.exists(),
        "live-window identity tests belong in src/o3_runtime_live_window_identity_tests.rs"
    );
    let identity = fs::read_to_string(identity_path).unwrap();
    let identity_code = rust_code_without_comments_and_literals(&identity);
    let identity_include_lines = include_macro_lines(&identity);
    assert!(
        identity_include_lines.is_empty(),
        "src/o3_runtime_live_window_identity_tests.rs must not inline hidden test fragments; found lines {identity_include_lines:?}"
    );
    assert!(
        identity.lines().count() <= MAX_O3_RUNTIME_LIVE_WINDOW_IDENTITY_TEST_LINES,
        "src/o3_runtime_live_window_identity_tests.rs exceeds {MAX_O3_RUNTIME_LIVE_WINDOW_IDENTITY_TEST_LINES} lines"
    );
    for anchor in [
        "fn mismatched_live_speculative_record_does_not_claim_early_issue(",
        "fn malformed_live_speculative_fetch_identity_does_not_occupy_candidate(",
        "fn restored_live_staged_row_without_transient_identity_fails_closed(",
        "fn invalidated_descendant_fetch_identity_keeps_retirement_authority(",
    ] {
        assert!(
            identity_code.contains(anchor),
            "src/o3_runtime_live_window_identity_tests.rs is missing `{anchor}`"
        );
        assert!(
            !tests_code.contains(anchor),
            "src/o3_runtime_live_window_tests.rs still owns identity test `{anchor}`"
        );
    }
}

#[test]
fn o3_runtime_control_window_lifecycle_tests_live_in_focused_child() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/o3_runtime_control_window_tests.rs");
    let child_path = crate_dir.join("src/o3_runtime_control_window_tests/lifecycle.rs");
    let lineage_path = crate_dir.join("src/o3_runtime_control_window_tests/lineage.rs");
    let producer_forwarded_target_path =
        crate_dir.join("src/o3_runtime_control_window_tests/producer_forwarded_target.rs");
    let producer_forwarded_return_path =
        crate_dir.join("src/o3_runtime_control_window_tests/producer_forwarded_return.rs");
    let producer_forwarded_scalar_return_path =
        crate_dir.join("src/o3_runtime_control_window_tests/producer_forwarded_scalar_return.rs");
    let producer_forwarded_chain_validation_path = crate_dir
        .join("src/o3_runtime_control_window_tests/producer_forwarded_chain_validation.rs");
    let root = fs::read_to_string(&root_path).unwrap();
    let root_code = rust_code_without_comments_and_literals(&root);
    let root_include_lines = include_macro_lines(&root);

    assert!(
        root_include_lines.is_empty(),
        "src/o3_runtime_control_window_tests.rs must use path-owned child modules instead of include! fragments; found lines {root_include_lines:?}"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "o3_runtime_control_window_tests/lifecycle.rs",
            "lifecycle",
        ),
        1,
        "control-window lifecycle tests must have exactly one attached path-owned child declaration"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "o3_runtime_control_window_tests/lineage.rs",
            "lineage",
        ),
        1,
        "control-window lineage tests must have exactly one attached path-owned child declaration"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "o3_runtime_control_window_tests/producer_forwarded_target.rs",
            "producer_forwarded_target",
        ),
        1,
        "control-window producer-forwarded target tests must have exactly one attached path-owned child declaration"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "o3_runtime_control_window_tests/producer_forwarded_return.rs",
            "producer_forwarded_return",
        ),
        1,
        "control-window producer-forwarded return tests must have exactly one attached path-owned child declaration"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "o3_runtime_control_window_tests/producer_forwarded_scalar_return.rs",
            "producer_forwarded_scalar_return",
        ),
        1,
        "control-window producer-forwarded scalar-return tests must have exactly one attached path-owned child declaration"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "o3_runtime_control_window_tests/producer_forwarded_chain_validation.rs",
            "producer_forwarded_chain_validation",
        ),
        1,
        "control-window producer-forwarded chain validation tests must have exactly one attached path-owned child declaration"
    );
    for legacy in [
        "same_link_return.rs",
        "same_link_scalar_return.rs",
        "same_link_validation.rs",
    ] {
        assert!(
            !crate_dir
                .join("src/o3_runtime_control_window_tests")
                .join(legacy)
                .exists(),
            "obsolete same-link test owner still exists: {legacy}"
        );
    }
    assert!(
        line_count(&root_path) <= MAX_O3_RUNTIME_CONTROL_WINDOW_TEST_ROOT_LINES,
        "src/o3_runtime_control_window_tests.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_TEST_ROOT_LINES} lines"
    );
    for anchor in [
        "fn predicted_control_branch_candidate_has_no_destination_and_keeps_issue_tick(",
        "fn predicted_mul_wakes_dependent_add_candidate(",
    ] {
        assert!(
            root_code.contains(anchor),
            "src/o3_runtime_control_window_tests.rs is missing retained test `{anchor}`"
        );
    }

    assert!(
        child_path.exists(),
        "control-window lifecycle tests belong in src/o3_runtime_control_window_tests/lifecycle.rs"
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let child_code = rust_code_without_comments_and_literals(&child);
    let child_include_lines = include_macro_lines(&child);
    assert!(
        child_include_lines.is_empty(),
        "src/o3_runtime_control_window_tests/lifecycle.rs must not inline hidden test fragments; found lines {child_include_lines:?}"
    );
    assert!(
        line_count(&child_path) <= MAX_O3_RUNTIME_CONTROL_WINDOW_LIFECYCLE_TEST_LINES,
        "src/o3_runtime_control_window_tests/lifecycle.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_LIFECYCLE_TEST_LINES} lines"
    );
    for anchor in [
        "fn inner_control_uses_staged_outer_ownership_before_execution_record(",
        "fn outer_control_validation_preserves_inner_control_chain(",
        "fn validated_outer_control_keeps_terminal_inner_timing_window_live(",
        "fn outer_control_discard_removes_inner_branch_and_descendant(",
        "fn branch_descendant_cleanup_discards_only_future_writeback_reservations(",
        "fn branch_descendant_cleanup_discards_unpublished_past_writeback_reservation(",
        "fn inner_control_discard_preserves_outer_branch(",
        "fn middle_control_discard_removes_only_inner_control(",
        "fn mixed_middle_control_discard_removes_only_indirect_jump(",
        "fn split_inner_branch_suffix_replacement_prunes_nested_chain(",
        "fn outer_control_redirect_discards_invalidated_descendant_fetch_identity(",
        "fn mismatched_control_redirect_preserves_current_fallback_until_runtime_recording(",
        "fn predicted_descendants_use_staged_branch_ownership_and_invalidate_with_it(",
    ] {
        assert!(
            child_code.contains(anchor),
            "src/o3_runtime_control_window_tests/lifecycle.rs is missing lifecycle test `{anchor}`"
        );
        assert!(
            !root_code.contains(anchor),
            "src/o3_runtime_control_window_tests.rs still owns lifecycle test `{anchor}`"
        );
    }

    assert!(lineage_path.exists());
    let lineage = fs::read_to_string(&lineage_path).unwrap();
    let lineage_code = rust_code_without_comments_and_literals(&lineage);
    assert!(include_macro_lines(&lineage).is_empty());
    assert!(
        line_count(&lineage_path) <= MAX_O3_RUNTIME_CONTROL_WINDOW_LINEAGE_TEST_LINES,
        "src/o3_runtime_control_window_tests/lineage.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_LINEAGE_TEST_LINES} lines"
    );
    for anchor in [
        "staged_window_truncation_prunes_control_lineage",
        "validated_committed_control_keeps_scalar_descendant_window_until_drain",
        "invalidated_resident_control_and_descendant_do_not_keep_window_live",
    ] {
        assert!(
            production_defines_exact_function(&lineage_code, anchor),
            "lineage.rs is missing `{anchor}`"
        );
        assert!(
            !production_defines_exact_function(&child_code, anchor),
            "lifecycle.rs still owns lineage test `{anchor}`"
        );
    }

    assert!(producer_forwarded_target_path.exists());
    let producer_forwarded_target = fs::read_to_string(&producer_forwarded_target_path).unwrap();
    let producer_forwarded_target_code =
        rust_code_without_comments_and_literals(&producer_forwarded_target);
    let producer_forwarded_target_include_lines = include_macro_lines(&producer_forwarded_target);
    assert!(
        producer_forwarded_target_include_lines.is_empty(),
        "producer_forwarded_target.rs must not inline hidden test fragments"
    );
    assert!(
        line_count(&producer_forwarded_target_path)
            <= MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_TARGET_TEST_LINES,
        "producer_forwarded_target.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_TARGET_TEST_LINES} lines"
    );
    for anchor in [
        "live_no_link_and_split_link_controls_expose_exact_producer_forwarded_targets",
        "live_same_link_control_exposes_exact_producer_forwarded_target",
    ] {
        assert!(
            production_defines_exact_function(&producer_forwarded_target_code, anchor),
            "producer_forwarded_target.rs is missing `{anchor}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, anchor),
            "control-window test root still owns `{anchor}`"
        );
    }

    assert!(
        producer_forwarded_return_path.exists(),
        "producer-forwarded return tests belong in their focused owner"
    );
    let producer_forwarded_return = fs::read_to_string(&producer_forwarded_return_path).unwrap();
    let producer_forwarded_return_code =
        rust_code_without_comments_and_literals(&producer_forwarded_return);
    assert!(
        include_macro_lines(&producer_forwarded_return).is_empty(),
        "producer_forwarded_return.rs must not inline hidden test fragments"
    );
    assert!(
        line_count(&producer_forwarded_return_path)
            <= MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_RETURN_TEST_LINES,
        "producer_forwarded_return.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_RETURN_TEST_LINES} lines"
    );
    for anchor in [
        "producer_forwarded_linked_calls_append_target_returns",
        "producer_forwarded_linked_call_rejects_nonordinary_target_controls",
        "producer_forwarded_split_link_call_appends_return_after_data_head_retires",
    ] {
        assert!(
            production_defines_exact_function(&producer_forwarded_return_code, anchor),
            "missing exact test definition `{anchor}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, anchor),
            "root still owns `{anchor}`"
        );
    }

    assert!(
        producer_forwarded_scalar_return_path.exists(),
        "producer-forwarded scalar-return tests belong in their focused owner"
    );
    let producer_forwarded_scalar_return =
        fs::read_to_string(&producer_forwarded_scalar_return_path).unwrap();
    let producer_forwarded_scalar_return_code =
        rust_code_without_comments_and_literals(&producer_forwarded_scalar_return);
    assert!(
        include_macro_lines(&producer_forwarded_scalar_return).is_empty(),
        "producer_forwarded_scalar_return.rs must not inline hidden test fragments"
    );
    assert!(
        line_count(&producer_forwarded_scalar_return_path)
            <= MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_SCALAR_RETURN_TEST_LINES,
        "producer_forwarded_scalar_return.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_SCALAR_RETURN_TEST_LINES} lines"
    );
    for anchor in [
        "producer_forwarded_scalar_lineage_survives_successful_data_head_retirement",
        "producer_forwarded_scalar_return_waits_for_data_head_retirement",
        "producer_forwarded_split_link_scalar_return_uses_link_destination",
        "producer_forwarded_split_link_scalar_requires_link_dependency",
        "producer_forwarded_scalar_return_rejects_nonordinary_shapes",
        "producer_forwarded_scalar_lineage_fails_closed_after_identity_change",
    ] {
        assert!(
            production_defines_exact_function(&producer_forwarded_scalar_return_code, anchor),
            "missing exact scalar-return runtime test definition `{anchor}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, anchor),
            "root still owns scalar-return runtime test `{anchor}`"
        );
    }

    assert!(
        producer_forwarded_chain_validation_path.exists(),
        "producer-forwarded chain validation tests belong in their focused owner"
    );
    let producer_forwarded_chain_validation =
        fs::read_to_string(&producer_forwarded_chain_validation_path).unwrap();
    let producer_forwarded_chain_validation_code =
        rust_code_without_comments_and_literals(&producer_forwarded_chain_validation);
    assert!(
        include_macro_lines(&producer_forwarded_chain_validation).is_empty(),
        "producer_forwarded_chain_validation.rs must not inline hidden test fragments"
    );
    assert!(
        line_count(&producer_forwarded_chain_validation_path)
            <= MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_CHAIN_VALIDATION_TEST_LINES,
        "producer_forwarded_chain_validation.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_CHAIN_VALIDATION_TEST_LINES} lines"
    );
    for anchor in [
        "direct_return_requires_dependency_and_live_staged_residency",
        "direct_return_requires_bound_fetch_identity",
        "head_retired_direct_return_reconstructs_exact_recorded_parent",
        "direct_return_carries_empty_scalar_chain",
        "scalar_descendant_requires_dependency_and_live_staged_residency",
        "scalar_return_carries_one_step_scalar_chain",
        "retained_scalar_chain_rejects_longer_candidate",
        "scalar_return_requires_dependency_residency_and_fetch_identity",
    ] {
        assert!(
            production_defines_exact_function(&producer_forwarded_chain_validation_code, anchor),
            "missing exact producer-forwarded validation test definition `{anchor}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, anchor),
            "root still owns same-link validation test `{anchor}`"
        );
    }
}

#[test]
fn producer_forwarded_scalar_authority_uses_one_typed_chain() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime_path = crate_dir.join("src/o3_runtime_producer_forwarded_chain.rs");
    let value_path = crate_dir.join("src/o3_runtime_producer_forwarded_chain/value.rs");
    let fetch_root_path = crate_dir.join("src/riscv_fetch_ahead.rs");
    let continuation_path =
        crate_dir.join("src/riscv_fetch_ahead/producer_forwarded_continuation.rs");
    let detailed_path = crate_dir.join("src/riscv_fetch_ahead/detailed_o3.rs");

    assert!(
        value_path.exists(),
        "producer-forwarded scalar-chain values need one focused owner"
    );
    assert!(
        continuation_path.exists(),
        "retained producer-forwarded fetch state needs one focused owner"
    );

    let runtime = fs::read_to_string(runtime_path).unwrap();
    assert_eq!(
        path_owned_module_declaration_count(
            &runtime,
            "o3_runtime_producer_forwarded_chain/value.rs",
            "value",
        ),
        1,
        "the runtime chain owner must attach its value module exactly once"
    );
    let values = production_rust_source(&fs::read_to_string(value_path).unwrap());
    assert!(
        production_defines_exact_named_item(&values, "struct", "O3ProducerForwardedScalarChain",),
        "producer-forwarded scalar lineage must have one typed chain"
    );
    assert!(
        production_defines_exact_named_item(
            &values,
            "enum",
            "O3ProducerForwardedScalarDescendants",
        ),
        "the scalar chain must keep its zero/one-step cases inline"
    );
    let descendants_definition =
        production_enum_definition(&values, "O3ProducerForwardedScalarDescendants")
            .expect("missing inline scalar-descendant representation");
    assert!(
        enum_has_unit_variant(&descendants_definition, "Empty"),
        "the zero-step scalar chain must remain inline"
    );
    assert!(
        enum_tuple_variant_payload(&descendants_definition, "One").is_some_and(|payload| {
            contains_rust_identifier(
                &payload.chars().collect::<Vec<_>>(),
                "O3ProducerForwardedScalarDescendant",
            )
        }),
        "the one-step scalar chain must remain inline"
    );
    assert!(
        enum_tuple_variant_payload(&descendants_definition, "Many").is_some_and(|payload| {
            generic_type_contains_named_type(&payload, "Vec", "O3ProducerForwardedScalarDescendant")
        }),
        "only the multi-step fallback may own a descendant vector"
    );
    let chain_definition = production_struct_definitions(&values)
        .into_iter()
        .find(|definition| {
            production_defines_exact_named_item(
                definition,
                "struct",
                "O3ProducerForwardedScalarChain",
            )
        })
        .expect("missing producer-forwarded scalar-chain definition");
    assert!(
        !generic_type_contains_named_type(
            &chain_definition,
            "Vec",
            "O3ProducerForwardedScalarDescendant",
        ),
        "the scalar chain must not heap-allocate its admitted zero/one-step cases"
    );
    let mut fixed_shape_owners = Vec::new();
    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let mut definitions = production_struct_definitions(&source);
        definitions.extend(production_type_alias_definitions(&source));
        if definitions.iter().any(|definition| {
            generic_type_contains_named_type(
                definition,
                "Option",
                "O3ProducerForwardedScalarDescendant",
            )
        }) {
            fixed_shape_owners.push(relative.display().to_string());
        }
    }
    assert!(
        fixed_shape_owners.is_empty(),
        "producer-forwarded authority restored the fixed optional-scalar shape: {}",
        fixed_shape_owners.join(", ")
    );

    let fetch_root = fs::read_to_string(fetch_root_path).unwrap();
    assert_eq!(
        path_owned_module_declaration_count(
            &fetch_root,
            "riscv_fetch_ahead/producer_forwarded_continuation.rs",
            "producer_forwarded_continuation",
        ),
        1,
        "fetch ahead must attach the retained-chain continuation exactly once"
    );
    let continuation = production_rust_source(&fs::read_to_string(continuation_path).unwrap());
    assert!(
        production_defines_exact_named_item(
            &continuation,
            "struct",
            "ProducerForwardedScalarContinuation",
        ),
        "retained fetch state is missing its typed-chain owner"
    );
    assert!(
        production_defines_exact_function(&values, "matches_retained_candidate"),
        "the typed chain must own the bounded retained-candidate predicate"
    );
    assert!(
        !contains_rust_identifier(&continuation.chars().collect::<Vec<_>>(), "is_prefix_of",),
        "retained fetch authority must not restore unbounded prefix matching"
    );
    let detailed = production_rust_source(&fs::read_to_string(detailed_path).unwrap());
    let authority = detailed
        .find("pub(crate) enum PredictedControlTargetAuthority")
        .expect("missing predicted-control target authority");
    let derive = detailed[..authority]
        .rfind("#[derive(")
        .expect("predicted-control target authority needs derives");
    assert!(
        !detailed[derive..authority].contains("Copy"),
        "producer-forwarded return authority owns a scalar chain and must not be Copy"
    );
}

#[test]
fn producer_forwarded_chain_authority_stays_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime_root_path = crate_dir.join("src/o3_runtime.rs");
    let runtime_path = crate_dir.join("src/o3_runtime_producer_forwarded_chain.rs");
    let value_path = crate_dir.join("src/o3_runtime_producer_forwarded_chain/value.rs");
    let fetch_root_path = crate_dir.join("src/riscv_fetch_ahead.rs");
    let prepared_path = crate_dir.join("src/riscv_fetch_ahead/prepared.rs");
    let continuation_path =
        crate_dir.join("src/riscv_fetch_ahead/producer_forwarded_continuation.rs");
    let detailed_path = crate_dir.join("src/riscv_fetch_ahead/detailed_o3.rs");
    let fetch_tests_root_path = crate_dir.join("src/riscv_fetch_ahead/tests.rs");
    let checkpoint_test_path = crate_dir.join("src/riscv_fetch_ahead/tests/checkpoint.rs");
    let fetch_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/producer_forwarded_return.rs");
    let fetch_link_shapes_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/producer_forwarded_return_link_shapes.rs");
    let scalar_fetch_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/producer_forwarded_scalar_return.rs");
    let scalar_fetch_link_shapes_test_path = crate_dir
        .join("src/riscv_fetch_ahead/tests/producer_forwarded_scalar_return_link_shapes.rs");
    let chain_validation_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/producer_forwarded_chain_validation.rs");
    let ras_required_validation_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/ras_required_validation.rs");
    let control_validation_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/producer_forwarded_control_validation.rs");
    let runtime_root = fs::read_to_string(&runtime_root_path).unwrap();
    let fetch_root = fs::read_to_string(&fetch_root_path).unwrap();
    let fetch_tests_root = fs::read_to_string(&fetch_tests_root_path).unwrap();

    assert_eq!(
        path_owned_module_declaration_count(
            &runtime_root,
            "o3_runtime_producer_forwarded_chain.rs",
            "o3_runtime_producer_forwarded_chain",
        ),
        1,
        "the O3 root must attach the focused producer-forwarded chain owner exactly once"
    );
    let legacy_modules = [
        "o3_runtime_producer_forwarded_return",
        "o3_runtime_scalar_return",
        "o3_runtime_same_link_chain",
    ];
    for (path, module) in [
        (
            "o3_runtime_producer_forwarded_return.rs",
            "o3_runtime_producer_forwarded_return",
        ),
        ("o3_runtime_scalar_return.rs", "o3_runtime_scalar_return"),
        (
            "o3_runtime_same_link_chain.rs",
            "o3_runtime_same_link_chain",
        ),
    ] {
        assert_eq!(
            path_owned_module_declaration_count(&runtime_root, path, module),
            0,
            "the O3 root must not retain split same-link owner `{module}`"
        );
        assert!(
            !crate_dir.join("src").join(path).exists(),
            "obsolete split same-link owner still exists at src/{path}"
        );
        assert!(
            !production_defines_exact_named_item(
                &production_rust_source(&runtime_root),
                "mod",
                module,
            ),
            "the O3 root must not declare legacy same-link module `{module}`"
        );
        assert!(
            !crate_dir.join("src").join(module).exists(),
            "obsolete split same-link module directory still exists at src/{module}"
        );
    }
    assert!(runtime_path.exists());
    assert!(value_path.exists());
    assert!(
        line_count(&runtime_path) <= MAX_O3_RUNTIME_PRODUCER_FORWARDED_CHAIN_LINES,
        "o3_runtime_producer_forwarded_chain.rs exceeds {MAX_O3_RUNTIME_PRODUCER_FORWARDED_CHAIN_LINES} lines"
    );
    let runtime = production_rust_source(&fs::read_to_string(&runtime_path).unwrap());
    assert!(include_macro_lines(&fs::read_to_string(&value_path).unwrap()).is_empty());
    assert!(
        line_count(&value_path) <= MAX_O3_RUNTIME_PRODUCER_FORWARDED_VALUE_LINES,
        "producer-forwarded value owner exceeds {MAX_O3_RUNTIME_PRODUCER_FORWARDED_VALUE_LINES} lines"
    );
    assert!(
        module_family_line_count(&runtime_path) <= MAX_O3_RUNTIME_PRODUCER_FORWARDED_OWNER_LINES,
        "producer-forwarded runtime owner exceeds {MAX_O3_RUNTIME_PRODUCER_FORWARDED_OWNER_LINES} aggregate lines"
    );
    let values = production_rust_source(&fs::read_to_string(&value_path).unwrap());
    let runtime_authority = format!("{runtime}\n{values}");
    let runtime_items = [
        "O3ProducerForwardedControlTarget",
        "O3ProducerForwardedScalarDescendant",
        "O3ProducerForwardedScalarChain",
        "O3ProducerForwardedReturnDescendant",
    ];
    for item in runtime_items {
        assert!(
            production_defines_exact_named_item(&runtime_authority, "struct", item),
            "producer-forwarded chain owner is missing `{item}`"
        );
        assert!(
            production_defines_exact_inherent_impl(&runtime_authority, item),
            "producer-forwarded chain owner is missing inherent implementation for `{item}`"
        );
    }
    let runtime_trait_impls = [
        ("PartialEq", "O3ProducerForwardedScalarDescendant"),
        ("Eq", "O3ProducerForwardedScalarDescendant"),
        ("PartialEq", "O3ProducerForwardedScalarChain"),
        ("Eq", "O3ProducerForwardedScalarChain"),
        ("PartialEq", "O3ProducerForwardedReturnDescendant"),
        ("Eq", "O3ProducerForwardedReturnDescendant"),
    ];
    for (trait_name, item) in runtime_trait_impls {
        assert!(
            production_defines_exact_trait_impl(&runtime_authority, trait_name, item),
            "producer-forwarded chain owner is missing `{trait_name}` for `{item}`"
        );
    }
    let runtime_functions = [
        "target_source",
        "link_destination",
        "fetched_scalar_chain",
        "scalar_chain",
        "matches_retained_candidate",
        "retained_return_descendant",
        "producer_forwarded_control_target",
        "retained_producer_forwarded_control_target",
        "producer_forwarded_control_target_with_completed",
        "producer_forwarded_control_target_for_sequences",
        "producer_forwarded_control_target_from_rows",
        "record_producer_forwarded_control_target",
        "has_recorded_producer_forwarded_control_target",
        "recorded_producer_forwarded_control_target_after_head_retire_for_sequences",
        "producer_forwarded_control_target_after_head_retire",
        "producer_forwarded_parent_for_descendant_sequences",
        "producer_forwarded_descendant_rows",
        "producer_forwarded_control_descendant_sequence",
        "producer_forwarded_return_descendant_for_sequence",
        "producer_forwarded_descendant_issue_context",
        "record_producer_forwarded_return_descendant",
        "has_recorded_producer_forwarded_return_descendant",
        "producer_forwarded_return_descendant",
        "direct_producer_forwarded_return_descendant",
        "producer_forwarded_scalar_chain_for_sequences",
        "producer_forwarded_scalar_chain",
        "producer_forwarded_scalar_return_issue_context",
        "append_producer_forwarded_scalar_return_descendant",
        "producer_forwarded_scalar_return_descendant",
    ];
    for anchor in runtime_functions {
        assert!(
            production_defines_exact_function(&runtime_authority, anchor),
            "producer-forwarded chain owner is missing `{anchor}`"
        );
    }
    for anchor in [
        "producer_forwarded_control_target_with_completed",
        "producer_forwarded_control_target_for_sequences",
        "producer_forwarded_control_target_from_rows",
        "recorded_producer_forwarded_control_target_after_head_retire_for_sequences",
        "producer_forwarded_parent_for_descendant_sequences",
        "producer_forwarded_descendant_rows",
        "producer_forwarded_control_descendant_sequence",
        "producer_forwarded_return_descendant_for_sequence",
        "direct_producer_forwarded_return_descendant",
        "producer_forwarded_scalar_chain_for_sequences",
        "producer_forwarded_scalar_return_descendant",
    ] {
        assert!(
            !production_function_is_visible(&runtime, anchor),
            "producer-forwarded chain internal helper `{anchor}` must remain private"
        );
    }
    for field in [
        "data_access_fetch_request",
        "fetch_request",
        "last_fetch_request",
        "pc",
        "sequential_pc",
        "instruction",
        "consumer_sequence",
        "producer_sequence",
        "ready_tick",
        "target_source",
        "target",
        "parent",
        "descendants",
        "scalar_chain",
        "sequence",
    ] {
        assert!(
            !production_defines_visible_field(&runtime_authority, field),
            "producer-forwarded chain field `{field}` must remain private"
        );
    }
    for test_only in [
        "retire_producer_forwarded_data_head_for_test",
        "producer_forwarded_scalar_return_issue_tick_for_test",
        "replace_producer_forwarded_chain_fetch_identity_for_test",
    ] {
        assert!(
            !production_defines_exact_function(&runtime, test_only),
            "test-only producer-forwarded helper `{test_only}` escaped into production"
        );
    }
    assert_eq!(
        runtime.matches("pub(super)").count(),
        1,
        "producer-forwarded chain internals must remain private except for the runtime recording hook"
    );
    assert!(
        runtime.contains("pub(super) fn record_producer_forwarded_return_descendant(&mut self)"),
        "the one sibling-visible chain item must be the runtime recording hook"
    );
    for legacy in [
        "supports_same_link_descendants",
        "record_producer_forwarded_same_link_return_descendant",
        "has_recorded_producer_forwarded_same_link_return_descendant",
        "producer_forwarded_same_link_return_descendant",
        "direct_producer_forwarded_same_link_return_descendant",
        "producer_forwarded_same_link_scalar_descendant",
        "producer_forwarded_same_link_scalar_descendant_for_sequences",
        "replace_same_link_chain_fetch_identity_for_test",
    ] {
        assert!(
            !production_defines_exact_function(&runtime, legacy),
            "obsolete same-link chain API remains: `{legacy}`"
        );
    }
    let mut legacy_module_owners = Vec::new();
    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        if legacy_modules
            .iter()
            .any(|module| production_defines_exact_named_item(&source, "mod", module))
        {
            legacy_module_owners.push(relative.display().to_string());
        }
    }
    assert!(
        legacy_module_owners.is_empty(),
        "legacy same-link modules remain declared in production source: {}",
        legacy_module_owners.join(", ")
    );
    let mut escaped_owners = Vec::new();
    for path in rust_source_files(&crate_dir.join("src")) {
        if path == runtime_path
            || path == value_path
            || is_test_only_rust_source(path.strip_prefix(crate_dir).unwrap())
        {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        if runtime_items.iter().any(|item| {
            production_defines_exact_named_item(&source, "struct", item)
                || production_defines_exact_inherent_impl(&source, item)
        }) || runtime_trait_impls.iter().any(|(trait_name, item)| {
            production_defines_exact_trait_impl(&source, trait_name, item)
        }) || runtime_functions
            .iter()
            .any(|function| production_defines_exact_function(&source, function))
        {
            escaped_owners.push(path.strip_prefix(crate_dir).unwrap().display().to_string());
        }
    }
    assert!(
        escaped_owners.is_empty(),
        "producer-forwarded chain authority escaped its focused owner: {}",
        escaped_owners.join(", ")
    );

    let fetch_tests_root_code = rust_code_without_comments_and_literals(&fetch_tests_root);
    assert_eq!(
        fetch_tests_root_code
            .matches("mod producer_forwarded_return;")
            .count(),
        1,
        "fetch-ahead tests must attach producer_forwarded_return exactly once"
    );
    assert!(fetch_test_path.exists());
    assert!(
        line_count(&fetch_test_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_RETURN_TEST_LINES,
        "producer_forwarded_return.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_RETURN_TEST_LINES} lines"
    );
    let fetch_test =
        rust_code_without_comments_and_literals(&fs::read_to_string(&fetch_test_path).unwrap());
    for anchor in [
        "pending_data_gate_admits_producer_forwarded_call_target_return",
        "producer_forwarded_return_apply_fails_closed_after_descendant_invalidation",
        "producer_forwarded_return_apply_fails_closed_after_ras_lineage_changes",
        "branch_lookahead_one_does_not_stage_producer_forwarded_return",
    ] {
        assert!(
            production_defines_exact_function(&fetch_test, anchor),
            "missing exact fetch test definition `{anchor}`"
        );
    }
    assert_eq!(
        fetch_tests_root_code
            .matches("mod producer_forwarded_return_link_shapes;")
            .count(),
        1,
        "fetch-ahead tests must attach producer_forwarded_return_link_shapes exactly once"
    );
    assert!(fetch_link_shapes_test_path.exists());
    let fetch_link_shapes_source = fs::read_to_string(&fetch_link_shapes_test_path).unwrap();
    assert!(include_macro_lines(&fetch_link_shapes_source).is_empty());
    assert!(
        line_count(&fetch_link_shapes_test_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_RETURN_LINK_SHAPES_TEST_LINES,
        "producer_forwarded_return_link_shapes.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_RETURN_LINK_SHAPES_TEST_LINES} lines"
    );
    let fetch_link_shapes_test = rust_code_without_comments_and_literals(&fetch_link_shapes_source);
    assert!(
        production_defines_exact_function(
            &fetch_link_shapes_test,
            "pending_data_gate_admits_same_and_split_link_direct_returns",
        ),
        "missing exact direct-return link-shape fetch test definition"
    );

    assert_eq!(
        fetch_tests_root_code
            .matches("mod producer_forwarded_chain_validation;")
            .count(),
        1,
        "fetch-ahead tests must attach producer_forwarded_chain_validation exactly once"
    );
    assert!(chain_validation_test_path.exists());
    let chain_validation_test = rust_code_without_comments_and_literals(
        &fs::read_to_string(&chain_validation_test_path).unwrap(),
    );
    assert!(
        include_macro_lines(&fs::read_to_string(&chain_validation_test_path).unwrap()).is_empty()
    );
    assert!(
        line_count(&chain_validation_test_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CHAIN_VALIDATION_TEST_LINES,
        "producer_forwarded_chain_validation.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CHAIN_VALIDATION_TEST_LINES} lines"
    );
    for anchor in [
        "direct_return_apply_fails_closed_after_fetch_identity_changes",
        "scalar_return_apply_fails_closed_after_fetch_identity_changes",
    ] {
        assert!(
            production_defines_exact_function(&chain_validation_test, anchor),
            "missing exact chain-validation fetch test definition `{anchor}`"
        );
    }

    assert_eq!(
        fetch_tests_root_code
            .matches("mod ras_required_validation;")
            .count(),
        1,
        "fetch-ahead tests must attach ras_required_validation exactly once"
    );
    assert!(ras_required_validation_test_path.exists());
    let ras_required_validation_source =
        fs::read_to_string(&ras_required_validation_test_path).unwrap();
    assert!(include_macro_lines(&ras_required_validation_source).is_empty());
    assert!(
        line_count(&ras_required_validation_test_path)
            <= MAX_RISCV_FETCH_AHEAD_RAS_REQUIRED_VALIDATION_TEST_LINES,
        "ras_required_validation.rs exceeds {MAX_RISCV_FETCH_AHEAD_RAS_REQUIRED_VALIDATION_TEST_LINES} lines"
    );
    let ras_required_validation_test =
        rust_code_without_comments_and_literals(&ras_required_validation_source);
    assert!(
        production_defines_exact_function(
            &ras_required_validation_test,
            "ras_required_apply_fails_closed_after_lineage_changes",
        ),
        "missing exact RAS-required apply validation test definition"
    );

    assert_eq!(
        fetch_tests_root_code
            .matches("mod producer_forwarded_control_validation;")
            .count(),
        1,
        "fetch-ahead tests must attach producer_forwarded_control_validation exactly once"
    );
    assert!(control_validation_test_path.exists());
    let control_validation_test = rust_code_without_comments_and_literals(
        &fs::read_to_string(&control_validation_test_path).unwrap(),
    );
    assert!(
        include_macro_lines(&fs::read_to_string(&control_validation_test_path).unwrap()).is_empty()
    );
    assert!(
        line_count(&control_validation_test_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CONTROL_VALIDATION_TEST_LINES,
        "producer_forwarded_control_validation.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CONTROL_VALIDATION_TEST_LINES} lines"
    );
    for anchor in [
        "producer_forwarded_control_requires_exact_speculation_kind_and_ras",
        "discarded_producer_forwarded_control_clears_only_speculation_binding",
    ] {
        assert!(
            production_defines_exact_function(&control_validation_test, anchor),
            "missing exact control-validation fetch test definition `{anchor}`"
        );
    }

    assert_eq!(
        rust_code_without_comments_and_literals(&fetch_root)
            .matches("mod prepared;")
            .count(),
        1,
        "riscv_fetch_ahead.rs must attach prepared.rs exactly once"
    );
    let prepared = fs::read_to_string(&prepared_path).unwrap();
    assert!(include_macro_lines(&prepared).is_empty());
    assert!(
        line_count(&prepared_path) <= MAX_RISCV_FETCH_AHEAD_PREPARED_LINES,
        "riscv_fetch_ahead/prepared.rs exceeds {MAX_RISCV_FETCH_AHEAD_PREPARED_LINES} lines"
    );
    let prepared_code = production_rust_source(&prepared);
    assert!(production_defines_exact_named_item(
        &prepared_code,
        "struct",
        "PreparedRiscvFetchAheadSpeculation",
    ));
    assert_eq!(
        path_owned_module_declaration_count(
            &fetch_root,
            "riscv_fetch_ahead/producer_forwarded_continuation.rs",
            "producer_forwarded_continuation",
        ),
        1,
        "riscv_fetch_ahead.rs must attach the retained scalar-chain continuation exactly once"
    );
    assert!(continuation_path.exists());
    let continuation = fs::read_to_string(&continuation_path).unwrap();
    assert!(include_macro_lines(&continuation).is_empty());
    assert!(
        line_count(&continuation_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CONTINUATION_LINES,
        "producer_forwarded_continuation.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_CONTINUATION_LINES} lines"
    );
    assert!(
        module_family_line_count(&prepared_path) + module_family_line_count(&continuation_path)
            <= MAX_RISCV_FETCH_AHEAD_PREPARED_OWNER_LINES,
        "fetch preparation owner exceeds {MAX_RISCV_FETCH_AHEAD_PREPARED_OWNER_LINES} aggregate lines"
    );
    assert!(production_defines_exact_named_item(
        &production_rust_source(&continuation),
        "struct",
        "ProducerForwardedScalarContinuation",
    ));
    assert!(production_defines_exact_function(
        &production_rust_source(&fs::read_to_string(&detailed_path).unwrap()),
        "retained_parent_resolution_preserves_fetch_path",
    ));
    let fetch_tests_root_code = rust_code_without_comments_and_literals(&fetch_tests_root);
    assert_eq!(
        fetch_tests_root_code
            .matches("mod producer_forwarded_scalar_return;")
            .count(),
        1,
        "fetch-ahead tests must attach producer_forwarded_scalar_return exactly once"
    );
    assert_eq!(
        fetch_tests_root_code
            .matches("mod producer_forwarded_scalar_return_link_shapes;")
            .count(),
        1,
        "fetch-ahead tests must attach producer_forwarded_scalar_return_link_shapes exactly once"
    );
    assert!(include_macro_lines(&fs::read_to_string(&scalar_fetch_test_path).unwrap()).is_empty());
    assert!(
        line_count(&scalar_fetch_test_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_SCALAR_RETURN_TEST_LINES,
        "producer_forwarded_scalar_return.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_SCALAR_RETURN_TEST_LINES} lines"
    );
    let scalar_fetch_test = rust_code_without_comments_and_literals(
        &fs::read_to_string(&scalar_fetch_test_path).unwrap(),
    );
    for anchor in [
        "live_data_head_allows_scalar_sequential_fetch_but_not_return_row",
        "pending_data_gate_allows_typed_scalar_sequential_fetch",
        "scalar_continuation_preparation_holds_lineage_while_fetch_is_pending",
        "committed_scalar_continuation_retains_exact_return_authority",
        "committed_call_seed_reconstructs_unstaged_scalar_return_authority",
        "committed_call_seed_reconstructs_already_executed_scalar_return_authority",
        "full_lookahead_at_call_recording_retains_later_scalar_return_authority",
        "prepared_scalar_continuation_survives_parent_commit_before_apply",
        "scalar_stage_retains_authority_before_continuation_decision_is_prepared",
        "retired_data_head_opens_scalar_sequential_fetch",
        "branch_lookahead_one_does_not_stage_scalar_sequential_return",
        "stale_ras_does_not_stage_scalar_sequential_return",
        "incorrect_parent_resolution_discards_retained_scalar_authority",
        "branch_checkpoint_restore_discards_retained_scalar_authority",
        "scalar_return_issue_waits_for_data_head_retirement_tick",
        "scalar_return_apply_fails_closed_after_scalar_lineage_changes",
    ] {
        assert!(
            production_defines_exact_function(&scalar_fetch_test, anchor),
            "missing exact scalar-return fetch test definition `{anchor}`"
        );
    }
    assert!(scalar_fetch_link_shapes_test_path.exists());
    let scalar_fetch_link_shapes_source =
        fs::read_to_string(&scalar_fetch_link_shapes_test_path).unwrap();
    assert!(include_macro_lines(&scalar_fetch_link_shapes_source).is_empty());
    assert!(
        line_count(&scalar_fetch_link_shapes_test_path)
            <= MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_SCALAR_RETURN_LINK_SHAPES_TEST_LINES,
        "producer_forwarded_scalar_return_link_shapes.rs exceeds {MAX_RISCV_FETCH_AHEAD_PRODUCER_FORWARDED_SCALAR_RETURN_LINK_SHAPES_TEST_LINES} lines"
    );
    let scalar_fetch_link_shapes_test =
        rust_code_without_comments_and_literals(&scalar_fetch_link_shapes_source);
    assert!(
        production_defines_exact_function(
            &scalar_fetch_link_shapes_test,
            "retired_data_head_admits_same_and_split_link_scalar_return_predictions",
        ),
        "missing exact scalar-return link-shape fetch test definition"
    );
    let checkpoint_test = rust_code_without_comments_and_literals(
        &fs::read_to_string(&checkpoint_test_path).unwrap(),
    );
    assert!(production_defines_exact_function(
        &checkpoint_test,
        "o3_checkpoint_restore_discards_retained_scalar_authority",
    ));
}

#[test]
fn live_window_module_policy_ignores_comments_and_detects_nonliteral_includes() {
    let live = "#[path = \"identity.rs\"]\nmod identity;\n";
    let commented = "/* #[path = \"identity.rs\"]\nmod identity; */\n";
    let detached = "#[path = \"identity.rs\"]\nconst MARKER: () = ();\nmod identity;\n";

    assert_eq!(
        path_owned_module_declaration_count(live, "identity.rs", "identity"),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(commented, "identity.rs", "identity"),
        0
    );
    assert_eq!(
        path_owned_module_declaration_count(detached, "identity.rs", "identity"),
        0
    );
    assert_eq!(
        include_macro_lines("include ! (concat!(\"identity\", \".rs\"));\n"),
        [1]
    );
    assert_eq!(
        include_macro_lines("mod hidden { include!(\"identity.rs\"); }\n"),
        [1]
    );
    assert_eq!(include_macro_lines("include\n!(\"identity.rs\");\n"), [1]);
    assert!(include_macro_lines("// include!(\"identity.rs\");\n").is_empty());
    assert!(include_macro_lines("include_str!(\"identity.rs\");\n").is_empty());
}

#[test]
fn production_rust_source_has_no_dead_code_allowances() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        if production_allows_dead_code(&source) {
            offenders.push(relative.display().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "production Rust source must not use #[allow(dead_code)]: {}",
        offenders.join(", ")
    );
}

#[test]
fn o3_control_window_has_no_obsolete_zero_tick_wrappers() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        for name in [
            "take_live_speculative_issue_timing",
            "discard_live_control_descendants_from",
            "discard_live_control_descendant_rows_from",
            "retain_live_speculative_executions",
            "remove_live_writeback_sequence",
        ] {
            if production_defines_exact_function(&source, name) {
                offenders.push(format!("{} defines {name}(", relative.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "obsolete non-time-aware O3 control-window wrappers remain: {}",
        offenders.join(", ")
    );
}

#[test]
fn o3_live_control_window_uses_one_typed_lineage_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = production_rust_source(&fs::read_to_string(&path).unwrap());
        for forbidden in ["live_control_window_sequences", "live_control_dependencies"] {
            if source.contains(forbidden) {
                offenders.push(format!("{} contains {forbidden}", relative.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "live O3 control-window membership must use one typed lineage authority: {}",
        offenders.join(", ")
    );

    let control_window =
        fs::read_to_string(crate_dir.join("src/o3_runtime_control_window.rs")).unwrap();
    assert!(production_defines_exact_named_item(
        &control_window,
        "struct",
        "O3LiveControlLineage",
    ));
    for method in [
        "pending",
        "control_sequence",
        "pending_control_sequence",
        "resolve",
    ] {
        assert!(
            production_defines_exact_function(&control_window, method),
            "typed O3 live-control lineage is missing `{method}`"
        );
    }
    let membership = rust_function_definition(&control_window, "is_live_control_window_sequence")
        .expect("O3 control-window authority must define derived sequence membership");
    for anchor in ["reorder_buffer", "live_control_window_entry"] {
        assert!(
            membership.contains(anchor),
            "derived O3 control-window sequence membership must consume `{anchor}`"
        );
    }

    let entry_membership = rust_function_definition(&control_window, "live_control_window_entry")
        .expect("O3 control-window authority must classify resident ROB entries");
    for anchor in ["is_live_staged", "live_control_lineages"] {
        assert!(
            entry_membership.contains(anchor),
            "derived O3 control-window entry membership must consume `{anchor}`"
        );
    }

    let has_window = rust_function_definition(&control_window, "has_live_control_window")
        .expect("O3 runtime must expose live control-window presence");
    for anchor in ["reorder_buffer", "live_control_window_entry"] {
        assert!(
            has_window.contains(anchor),
            "live control-window presence must consume `{anchor}`"
        );
    }

    let validation =
        rust_function_definition(&control_window, "validate_live_speculative_producer")
            .expect("O3 control validation must update lineage state");
    for anchor in ["live_control_lineages", "resolve"] {
        assert!(
            validation.contains(anchor),
            "O3 control validation must consume `{anchor}`"
        );
    }

    let issue = rust_function_definition(&control_window, "live_speculative_issue_candidate")
        .expect("O3 control-window authority must select issue candidates");
    assert!(
        issue.contains("pending_control_sequence"),
        "O3 issue dependencies must consume only pending control lineage"
    );
}

fn production_defines_exact_function(source: &str, name: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "fn" {
            let name_start = skip_rust_whitespace(&chars, end);
            if let Some((function_name, name_end)) = rust_identifier_at(&chars, name_start) {
                if function_name == name {
                    let after_name = skip_rust_whitespace(&chars, name_end);
                    if matches!(chars.get(after_name), Some('(' | '<')) {
                        return true;
                    }
                }
            }
        }
        index = end;
    }
    false
}

fn generic_type_contains_named_type(source: &str, outer: &str, inner: &str) -> bool {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        let open = skip_rust_whitespace(&chars, end);
        if identifier == outer && chars.get(open) == Some(&'<') {
            let Some(close) = matching_delimiter(&chars, open, '<', '>') else {
                return false;
            };
            let mut nested = open + 1;
            while nested < close {
                let Some((nested_identifier, nested_end)) = rust_identifier_at(&chars, nested)
                else {
                    nested += 1;
                    continue;
                };
                if nested_identifier == inner {
                    return true;
                }
                nested = nested_end;
            }
            index = close + 1;
            continue;
        }
        index = end;
    }
    false
}

fn production_struct_definitions(source: &str) -> Vec<String> {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut definitions = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier != "struct" {
            index = end;
            continue;
        }
        let name_start = skip_rust_whitespace(&chars, end);
        let Some((_, name_end)) = rust_identifier_at(&chars, name_start) else {
            index = end;
            continue;
        };
        let mut body = skip_rust_whitespace(&chars, name_end);
        if chars.get(body) == Some(&'<') {
            let Some(generics_end) = matching_delimiter(&chars, body, '<', '>') else {
                index = name_end;
                continue;
            };
            body = skip_rust_whitespace(&chars, generics_end + 1);
        }
        if chars.get(body) == Some(&'(') {
            let Some(close) = matching_delimiter(&chars, body, '(', ')') else {
                index = body + 1;
                continue;
            };
            definitions.push(chars[index..=close].iter().collect());
            index = close + 1;
            continue;
        }
        while body < chars.len() && !matches!(chars[body], '{' | ';') {
            body += 1;
        }
        if chars.get(body) == Some(&'{') {
            let Some(close) = matching_delimiter(&chars, body, '{', '}') else {
                index = body + 1;
                continue;
            };
            definitions.push(chars[index..=close].iter().collect());
            index = close + 1;
            continue;
        }
        index = body.saturating_add(1);
    }
    definitions
}

fn production_enum_definition(source: &str, name: &str) -> Option<String> {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier != "enum" {
            index = end;
            continue;
        }
        let name_start = skip_rust_whitespace(&chars, end);
        let Some((item_name, name_end)) = rust_identifier_at(&chars, name_start) else {
            index = end;
            continue;
        };
        if item_name != name {
            index = name_end;
            continue;
        }
        let mut body = skip_rust_whitespace(&chars, name_end);
        if chars.get(body) == Some(&'<') {
            body = matching_delimiter(&chars, body, '<', '>')? + 1;
        }
        while body < chars.len() && chars[body] != '{' {
            if chars[body] == ';' {
                return None;
            }
            body += 1;
        }
        let close = matching_delimiter(&chars, body, '{', '}')?;
        return Some(chars[index..=close].iter().collect());
    }
    None
}

fn production_type_alias_definitions(source: &str) -> Vec<String> {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut definitions = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier != "type" {
            index = end;
            continue;
        }
        let name_start = skip_rust_whitespace(&chars, end);
        let Some((_, name_end)) = rust_identifier_at(&chars, name_start) else {
            index = end;
            continue;
        };
        let Some(close) = chars
            .iter()
            .enumerate()
            .skip(name_end)
            .find_map(|(candidate, character)| (*character == ';').then_some(candidate))
        else {
            break;
        };
        if chars[name_end..close].contains(&'=') {
            definitions.push(chars[index..=close].iter().collect());
        }
        index = close + 1;
    }
    definitions
}

fn enum_has_unit_variant(definition: &str, name: &str) -> bool {
    let chars = definition.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == name
            && matches!(
                chars.get(skip_rust_whitespace(&chars, end)),
                Some(',' | '}')
            )
        {
            return true;
        }
        index = end;
    }
    false
}

fn enum_tuple_variant_payload(definition: &str, name: &str) -> Option<String> {
    let chars = definition.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        let open = skip_rust_whitespace(&chars, end);
        if identifier == name && chars.get(open) == Some(&'(') {
            let close = matching_delimiter(&chars, open, '(', ')')?;
            return Some(chars[open + 1..close].iter().collect());
        }
        index = end;
    }
    None
}

fn production_function_is_visible(source: &str, name: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "fn" {
            let name_start = skip_rust_whitespace(&chars, end);
            if let Some((function_name, name_end)) = rust_identifier_at(&chars, name_start) {
                let after_name = skip_rust_whitespace(&chars, name_end);
                if function_name == name && matches!(chars.get(after_name), Some('(' | '<')) {
                    let item_start = (0..index)
                        .rev()
                        .find(|candidate| matches!(chars[*candidate], '{' | '}' | ';'))
                        .map_or(0, |boundary| boundary + 1);
                    let mut visibility_index = item_start;
                    while visibility_index < index {
                        let Some((visibility, visibility_end)) =
                            rust_identifier_at(&chars, visibility_index)
                        else {
                            visibility_index += 1;
                            continue;
                        };
                        if visibility == "pub" {
                            return true;
                        }
                        visibility_index = visibility_end;
                    }
                    return false;
                }
            }
        }
        index = end;
    }
    false
}

fn production_defines_visible_field(source: &str, name: &str) -> bool {
    source.lines().any(|line| {
        let chars = line.trim().chars().collect::<Vec<_>>();
        let Some((visibility, visibility_end)) = rust_identifier_at(&chars, 0) else {
            return false;
        };
        if visibility != "pub" {
            return false;
        }
        let mut field_start = skip_rust_whitespace(&chars, visibility_end);
        if chars.get(field_start) == Some(&'(') {
            let Some(visibility_close) = matching_delimiter(&chars, field_start, '(', ')') else {
                return false;
            };
            field_start = skip_rust_whitespace(&chars, visibility_close + 1);
        }
        let Some((field_name, field_end)) = rust_identifier_at(&chars, field_start) else {
            return false;
        };
        field_name == name && chars.get(skip_rust_whitespace(&chars, field_end)) == Some(&':')
    })
}

fn production_defines_exact_named_item(source: &str, keyword: &str, name: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == keyword {
            let name_start = skip_rust_whitespace(&chars, end);
            if let Some((item_name, name_end)) = rust_identifier_at(&chars, name_start) {
                if item_name == name {
                    let after_name = skip_rust_whitespace(&chars, name_end);
                    if matches!(chars.get(after_name), Some('<' | '{' | '(' | ';')) {
                        return true;
                    }
                }
            }
        }
        index = end;
    }
    false
}

fn production_defines_exact_inherent_impl(source: &str, name: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "impl" {
            let name_start = skip_rust_whitespace(&chars, end);
            if let Some((impl_name, name_end)) = rust_identifier_at(&chars, name_start) {
                if impl_name == name
                    && matches!(chars.get(skip_rust_whitespace(&chars, name_end)), Some('{'))
                {
                    return true;
                }
            }
        }
        index = end;
    }
    false
}

fn production_defines_exact_trait_impl(source: &str, trait_name: &str, name: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "impl" {
            let trait_start = skip_rust_whitespace(&chars, end);
            let Some((found_trait, trait_end)) = rust_identifier_at(&chars, trait_start) else {
                index = end;
                continue;
            };
            let for_start = skip_rust_whitespace(&chars, trait_end);
            let Some((for_keyword, for_end)) = rust_identifier_at(&chars, for_start) else {
                index = trait_end;
                continue;
            };
            let name_start = skip_rust_whitespace(&chars, for_end);
            let Some((impl_name, name_end)) = rust_identifier_at(&chars, name_start) else {
                index = for_end;
                continue;
            };
            if found_trait == trait_name
                && for_keyword == "for"
                && impl_name == name
                && matches!(chars.get(skip_rust_whitespace(&chars, name_end)), Some('{'))
            {
                return true;
            }
        }
        index = end;
    }
    false
}

fn rust_function_definition(source: &str, name: &str) -> Option<String> {
    let code = rust_code_without_comments_and_literals(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "fn" {
            let name_start = skip_rust_whitespace(&chars, end);
            let Some((function_name, name_end)) = rust_identifier_at(&chars, name_start) else {
                index = end;
                continue;
            };
            if function_name == name {
                let Some(open) =
                    chars
                        .iter()
                        .enumerate()
                        .skip(name_end)
                        .find_map(|(index, character)| match character {
                            '{' => Some(index),
                            ';' => None,
                            _ => None,
                        })
                else {
                    return None;
                };
                let close = matching_delimiter(&chars, open, '{', '}')?;
                return Some(chars[index..=close].iter().collect());
            }
        }
        index = end;
    }
    None
}

#[test]
fn o3_store_forwarding_policy_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let memory_window =
        fs::read_to_string(crate_dir.join("src/o3_runtime_memory_window.rs")).unwrap();
    let stats = fs::read_to_string(crate_dir.join("src/o3_runtime_stats.rs")).unwrap();
    let module_path = crate_dir.join("src/o3_store_forwarding.rs");

    assert!(
        root.contains("mod o3_store_forwarding;"),
        "src/o3_runtime.rs must delegate store-forwarding classification"
    );
    assert!(
        module_path.exists(),
        "O3 store-forwarding policy belongs in src/o3_store_forwarding.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    for anchor in [
        "struct O3StoreForwardingEntry",
        "struct O3StoreLoadForwardingPlan",
        "enum O3StoreLoadRelation",
        "enum O3StoreLoadSuppressionReason",
        "fn o3_store_load_composition",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_store_forwarding.rs is missing policy owner `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "src/o3_runtime.rs still owns store-forwarding policy `{anchor}`"
        );
    }
    assert!(
        memory_window.contains("o3_load_forwarding_access"),
        "the live memory window must use the shared scalar-load range conversion"
    );
    assert!(
        !memory_window.contains("fn scalar_memory_range"),
        "the live memory window must not duplicate scalar range construction"
    );
    assert!(
        stats.contains("O3StoreLoadSuppressionReason"),
        "forwarding stats must consume the classifier-owned suppression reason"
    );
}

#[test]
fn riscv_live_data_handoff_codec_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join("src/riscv_execution_mode_handoff.rs")).unwrap();
    let codec_path = crate_dir.join("src/riscv_execution_mode_handoff/codec.rs");

    assert!(
        root.contains("mod codec;"),
        "the RISC-V live-data handoff root must delegate binary encoding"
    );
    assert!(
        codec_path.exists(),
        "live-data handoff encoding belongs in src/riscv_execution_mode_handoff/codec.rs"
    );
    let codec = fs::read_to_string(codec_path).unwrap();
    for anchor in [
        "const MAGIC:",
        "pub fn encode(&self)",
        "pub fn decode(payload:",
        "fn read_target(",
    ] {
        assert!(
            codec.contains(anchor),
            "live-data handoff codec is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "the semantic handoff root still owns codec detail `{anchor}`"
        );
    }
}

#[test]
fn production_rust_source_ignores_comments_and_cfg_test_items() {
    let source = r##"
// fn line_comment_owner() {}
let production_value = 1; // fn inline_line_comment_owner() {}
const STRING_LITERAL_OWNER: &str = "fn string_literal_owner() {} // not a comment";
const RAW_STRING_LITERAL_OWNER: &str = r#"fn raw_string_literal_owner() {} /* not a comment */"#;
/*
fn block_comment_owner() {}
*/
let another_production_value = 2; /* fn inline_block_comment_owner() {} */
fn production_before_inline_block() {} /* removed */ fn production_after_inline_block() {}
fn lifetime_owner<'a>(value: &'a str) -> &'a str { value }
#[cfg(test)]
const TEST_ONLY_BRACE: &str = "{";
const PRODUCTION_AFTER_CONST: &str = "production";
struct State {
    #[cfg(test)]
    test_only: u64,
    production: u64,
}
#[cfg(test)]
fn test_only_owner() {
    let text = "{ ignored brace }";
}
fn production_owner() {}
#[cfg(test)]
fn inline_test_owner() {}
fn production_after_inline_test() {}
#[cfg(test)]
fn multiline_test_owner(
    value: u64,
) {
    let _ = value;
    fn multiline_test_body_owner() {}
}
fn production_after_multiline_test() {}
#[cfg(test)]
fn where_test_owner<T>()
where
    T: Copy,
{
    fn where_test_body_owner() {}
}
fn production_after_where_test() {}
#[cfg(test)]
const TEST_ONLY_VALUES: &[u64] = &[
    TEST_ONLY_FIRST_ELEMENT,
    TEST_ONLY_SECOND_ELEMENT,
];
fn production_after_multiline_const() {}
#[cfg(test)]
fn char_literal_test_owner() {
    let _ = '{';
}
fn production_after_char_literal_test() {}
#[cfg(test)]
mod tests {
    fn nested_test_owner() {}
}
fn production_after_tests() {}
"##;

    let production = production_rust_source(source);

    for absent in [
        "line_comment_owner",
        "inline_line_comment_owner",
        "string_literal_owner",
        "raw_string_literal_owner",
        "block_comment_owner",
        "inline_block_comment_owner",
        "TEST_ONLY_BRACE",
        "test_only",
        "test_only_owner",
        "inline_test_owner",
        "multiline_test_owner",
        "multiline_test_body_owner",
        "where_test_owner",
        "where_test_body_owner",
        "TEST_ONLY_VALUES",
        "TEST_ONLY_SECOND_ELEMENT",
        "char_literal_test_owner",
        "nested_test_owner",
    ] {
        assert!(
            !production.contains(absent),
            "production source retained `{absent}`:\n{production}"
        );
    }
    for present in [
        "production: u64",
        "PRODUCTION_AFTER_CONST",
        "production_owner",
        "lifetime_owner<'a>",
        "production_after_inline_test",
        "production_after_multiline_test",
        "production_after_where_test",
        "production_after_multiline_const",
        "production_after_char_literal_test",
        "production_after_inline_block",
        "production_after_tests",
    ] {
        assert!(production.contains(present));
    }
}

#[test]
fn source_policy_helper_detects_dead_code_allowance_variants() {
    for source in [
        "#[allow(dead_code)]\nfn production_owner() {}\n",
        "#![allow(dead_code)]\nfn production_owner() {}\n",
        "#[allow( dead_code , unused )]\nfn production_owner() {}\n",
        "#[allow(unused, dead_code, reason = \"legacy\")]\nfn production_owner() {}\n",
        "#[cfg_attr(feature = \"x\", allow(dead_code))]\nfn production_owner() {}\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            production_allows_dead_code(&production),
            "failed to detect dead_code allowance in:\n{source}"
        );
    }

    for source in [
        "// #[allow(dead_code)]\nfn production_owner() {}\n",
        "/* #[allow(dead_code)] */\nfn production_owner() {}\n",
        "const TEXT: &str = \"#[allow(dead_code)]\";\nfn production_owner() {}\n",
        "const RAW: &str = r#\"#[allow(dead_code)]\"#;\nfn production_owner() {}\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            !production_allows_dead_code(&production),
            "comment or literal should not count as dead_code allowance:\n{source}"
        );
    }
}

#[test]
fn source_policy_helper_detects_exact_function_definitions() {
    for source in [
        "fn obsolete_name() {}\n",
        "fn\nobsolete_name() {}\n",
        "fn obsolete_name<T>() {}\n",
        "fn\nobsolete_name\n<T>() {}\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            production_defines_exact_function(&production, "obsolete_name"),
            "failed to detect obsolete function definition in:\n{source}"
        );
    }

    let production = production_rust_source(
        r#"
obsolete_name();
let obsolete_name = 1;
let text = "fn obsolete_name() {}";
// fn obsolete_name() {}
/* fn obsolete_name() {} */
fn obsolete_name_extra() {}
fn not_obsolete_name() {}
"#,
    );
    assert!(
        !production_defines_exact_function(&production, "obsolete_name"),
        "unrelated identifiers, calls, comments, literals, or longer names must not count"
    );
}

#[test]
fn source_policy_helper_detects_named_types_inside_generics() {
    assert!(generic_type_contains_named_type(
        "type Legacy = Option<\ncrate::o3_runtime::O3ProducerForwardedScalarDescendant\n>;",
        "Option",
        "O3ProducerForwardedScalarDescendant",
    ));
    assert!(generic_type_contains_named_type(
        "type Nested = Option<Vec<O3ProducerForwardedScalarDescendant>>;",
        "Option",
        "O3ProducerForwardedScalarDescendant",
    ));
    assert!(!generic_type_contains_named_type(
        "// Option<O3ProducerForwardedScalarDescendant>\ntype Current = O3ProducerForwardedScalarChain;",
        "Option",
        "O3ProducerForwardedScalarDescendant",
    ));

    let source = "\
struct Authority<T>\n\
where\n\
    T: Copy,\n\
{\n\
    scalar: Option<crate::o3_runtime::O3ProducerForwardedScalarDescendant>,\n\
}\n\
struct Tuple(Option<O3ProducerForwardedScalarDescendant>);\n\
fn query() -> Option<O3ProducerForwardedScalarDescendant> { None }\n";
    let definitions = production_struct_definitions(source);
    assert_eq!(definitions.len(), 2);
    for definition in definitions {
        assert!(generic_type_contains_named_type(
            &definition,
            "Option",
            "O3ProducerForwardedScalarDescendant",
        ));
        assert!(!definition.contains("fn query"));
    }
    assert!(production_struct_definitions(
        "fn query() -> Option<O3ProducerForwardedScalarDescendant> { None }"
    )
    .is_empty());
}

#[test]
fn source_policy_helper_extracts_enum_variants_and_type_aliases() {
    let source = "\
enum Descendants<T>\n\
where\n\
    T: Copy,\n\
{\n\
    Empty,\n\
    One(O3ProducerForwardedScalarDescendant),\n\
    Many(Vec<O3ProducerForwardedScalarDescendant>),\n\
}\n\
type Legacy = Option<O3ProducerForwardedScalarDescendant>;\n\
fn query() -> Option<O3ProducerForwardedScalarDescendant> { None }\n";
    let definition = production_enum_definition(source, "Descendants").unwrap();
    assert!(!definition.contains("type Legacy"));
    assert!(enum_has_unit_variant(&definition, "Empty"));
    assert!(
        enum_tuple_variant_payload(&definition, "One").is_some_and(|payload| {
            contains_rust_identifier(
                &payload.chars().collect::<Vec<_>>(),
                "O3ProducerForwardedScalarDescendant",
            )
        })
    );
    assert!(
        enum_tuple_variant_payload(&definition, "Many").is_some_and(|payload| {
            generic_type_contains_named_type(&payload, "Vec", "O3ProducerForwardedScalarDescendant")
        })
    );

    let aliases = production_type_alias_definitions(source);
    assert_eq!(aliases.len(), 1);
    assert!(generic_type_contains_named_type(
        &aliases[0],
        "Option",
        "O3ProducerForwardedScalarDescendant",
    ));
    assert!(!aliases[0].contains("fn query"));
}

#[test]
fn source_policy_helper_detects_function_and_field_visibility() {
    for source in [
        "pub fn authority() {}\n",
        "pub(crate)\nfn authority() {}\n",
        "pub(in crate::owner) fn authority() {}\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            production_function_is_visible(&production, "authority"),
            "failed to detect visible function in:\n{source}"
        );
    }
    assert!(!production_function_is_visible(
        &production_rust_source("fn authority() {}\n"),
        "authority",
    ));

    for source in [
        "struct Authority {\n    pub value: u64,\n}\n",
        "struct Authority {\n    pub(crate) value: u64,\n}\n",
        "struct Authority {\n    pub(in crate::owner) value: u64,\n}\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            production_defines_visible_field(&production, "value"),
            "failed to detect visible field in:\n{source}"
        );
    }
    assert!(!production_defines_visible_field(
        &production_rust_source("struct Authority { value: u64 }\n"),
        "value",
    ));
}

#[test]
fn source_policy_helper_detects_exact_inherent_implementations() {
    for source in [
        "impl Authority {}\n",
        "impl\nAuthority\n{}\n",
        "impl Authority { fn check(&self) {} }\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            production_defines_exact_inherent_impl(&production, "Authority"),
            "failed to detect inherent implementation in:\n{source}"
        );
    }

    let production = production_rust_source(
        r#"
impl PartialEq for Authority {}
impl AuthorityExtra {}
// impl Authority {}
const TEXT: &str = "impl Authority {}";
"#,
    );
    assert!(
        !production_defines_exact_inherent_impl(&production, "Authority"),
        "trait impls, longer identifiers, comments, and literals must not count"
    );
}

#[test]
fn source_policy_helper_detects_exact_trait_implementations() {
    for source in [
        "impl PartialEq for Authority {}\n",
        "impl\nPartialEq\nfor\nAuthority\n{}\n",
    ] {
        let production = production_rust_source(source);
        assert!(
            production_defines_exact_trait_impl(&production, "PartialEq", "Authority"),
            "failed to detect trait implementation in:\n{source}"
        );
    }

    let production = production_rust_source(
        r#"
impl Authority {}
impl Eq for Authority {}
impl PartialEq for AuthorityExtra {}
// impl PartialEq for Authority {}
const TEXT: &str = "impl PartialEq for Authority {}";
"#,
    );
    assert!(
        !production_defines_exact_trait_impl(&production, "PartialEq", "Authority"),
        "inherent impls, other traits or types, comments, and literals must not count"
    );
}

#[test]
fn source_policy_helper_classifies_test_only_paths() {
    for path in [
        Path::new("src/o3_runtime/tests/pending_data.rs"),
        Path::new("src/o3_runtime_tests.rs"),
        Path::new("src/foo_tests.rs"),
    ] {
        assert!(
            is_test_only_rust_source(path),
            "{} should be test-only",
            path.display()
        );
    }

    for path in [
        Path::new("src/o3_runtime.rs"),
        Path::new("src/o3_runtime/test_support.rs"),
        Path::new("src/foo_test_helpers.rs"),
    ] {
        assert!(
            !is_test_only_rust_source(path),
            "{} should be production source",
            path.display()
        );
    }
}

fn production_allows_dead_code(source: &str) -> bool {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '#' {
            index += 1;
            continue;
        }
        let mut attribute_start = skip_rust_whitespace(&chars, index + 1);
        if chars.get(attribute_start) == Some(&'!') {
            attribute_start = skip_rust_whitespace(&chars, attribute_start + 1);
        }
        if chars.get(attribute_start) != Some(&'[') {
            index += 1;
            continue;
        }
        let Some(attribute_end) = matching_delimiter(&chars, attribute_start, '[', ']') else {
            index += 1;
            continue;
        };
        if attribute_allows_dead_code(&chars[attribute_start + 1..attribute_end]) {
            return true;
        }
        index = attribute_end + 1;
    }
    false
}

fn attribute_allows_dead_code(chars: &[char]) -> bool {
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "allow" {
            let open = skip_rust_whitespace(chars, end);
            if chars.get(open) == Some(&'(') {
                if let Some(close) = matching_delimiter(chars, open, '(', ')') {
                    if contains_rust_identifier(&chars[open + 1..close], "dead_code") {
                        return true;
                    }
                    index = close + 1;
                    continue;
                }
            }
        }
        index = end;
    }
    false
}

fn contains_rust_identifier(chars: &[char], needle: &str) -> bool {
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(chars, index) else {
            index += 1;
            continue;
        };
        if identifier == needle {
            return true;
        }
        index = end;
    }
    false
}

fn rust_identifier_at(chars: &[char], index: usize) -> Option<(String, usize)> {
    let first = *chars.get(index)?;
    if !is_rust_identifier_start(first) {
        return None;
    }
    let mut end = index + 1;
    while chars
        .get(end)
        .is_some_and(|character| is_rust_identifier_continue(*character))
    {
        end += 1;
    }
    Some((chars[index..end].iter().collect(), end))
}

fn is_rust_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_rust_identifier_continue(character: char) -> bool {
    is_rust_identifier_start(character) || character.is_ascii_digit()
}

fn skip_rust_whitespace(chars: &[char], mut index: usize) -> usize {
    while chars
        .get(index)
        .is_some_and(|character| character.is_whitespace())
    {
        index += 1;
    }
    index
}

fn matching_delimiter(chars: &[char], open: usize, left: char, right: char) -> Option<usize> {
    debug_assert_eq!(chars.get(open), Some(&left));
    let mut depth = 0;
    for (index, character) in chars.iter().copied().enumerate().skip(open) {
        if character == left {
            depth += 1;
        } else if character == right {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn include_macro_lines(source: &str) -> Vec<usize> {
    let code = rust_code_without_comments_and_literals(source);
    let bytes = code.as_bytes();
    let mut lines = Vec::new();
    let mut index = 0;
    while index + "include".len() <= bytes.len() {
        let Some(relative) = code[index..].find("include") else {
            break;
        };
        let start = index + relative;
        let end = start + "include".len();
        let identifier_byte = |byte: u8| byte == b'_' || byte.is_ascii_alphanumeric();
        let bounded_left = start == 0 || !identifier_byte(bytes[start - 1]);
        let bounded_right = end == bytes.len() || !identifier_byte(bytes[end]);
        let mut next = end;
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if bounded_left && bounded_right && bytes.get(next) == Some(&b'!') {
            let line = bytes[..start].iter().filter(|byte| **byte == b'\n').count() + 1;
            if lines.last() != Some(&line) {
                lines.push(line);
            }
        }
        index = end;
    }
    lines
}

fn path_owned_module_declaration_count(source: &str, path: &str, module: &str) -> usize {
    let code = rust_code_without_comments_and_literals(source);
    let source_lines = source.lines().collect::<Vec<_>>();
    let code_lines = code.lines().collect::<Vec<_>>();
    let attribute = format!("#[path = \"{path}\"]");
    let declaration = format!("mod {module};");

    source_lines
        .iter()
        .zip(&code_lines)
        .enumerate()
        .filter(|(index, (source_line, code_line))| {
            source_line.trim() == attribute
                && code_line.trim_start().starts_with("#[path")
                && code_lines
                    .iter()
                    .skip(index + 1)
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| line.trim() == declaration)
        })
        .count()
}

fn production_rust_source(source: &str) -> String {
    let code = rust_code_without_comments_and_literals(source);
    let lines = code.lines().collect::<Vec<_>>();
    let mut production = String::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            index += 1;
            while index < lines.len() && lines[index].trim().is_empty() {
                index += 1;
            }
            let termination = cfg_test_item_termination(&lines, index);
            let mut parenthesis_depth = 0isize;
            let mut bracket_depth = 0isize;
            let mut brace_depth = 0isize;
            let mut angle_depth = 0isize;
            let mut opened_brace = false;
            while index < lines.len() {
                let item = lines[index];
                let item_trimmed = item.trim();
                for character in item.chars() {
                    match character {
                        '(' => parenthesis_depth += 1,
                        ')' => parenthesis_depth -= 1,
                        '[' => bracket_depth += 1,
                        ']' => bracket_depth -= 1,
                        '{' => {
                            brace_depth += 1;
                            opened_brace = true;
                        }
                        '}' => brace_depth -= 1,
                        '<' => angle_depth += 1,
                        '>' if angle_depth > 0 => angle_depth -= 1,
                        _ => {}
                    }
                }
                let delimiters_closed =
                    parenthesis_depth == 0 && bracket_depth == 0 && brace_depth == 0;
                let terminated = match termination {
                    CfgTestItemTermination::Brace => {
                        (opened_brace && brace_depth == 0)
                            || (delimiters_closed && item_trimmed.ends_with(';'))
                    }
                    CfgTestItemTermination::Semicolon => {
                        delimiters_closed && item_trimmed.ends_with(';')
                    }
                    CfgTestItemTermination::Comma => {
                        delimiters_closed
                            && (item_trimmed.ends_with(';')
                                || (angle_depth == 0 && item_trimmed.ends_with(',')))
                    }
                };
                index += 1;
                if terminated {
                    break;
                }
            }
            continue;
        }
        production.push_str(line);
        production.push('\n');
        index += 1;
    }
    production
}

#[derive(Clone, Copy)]
enum CfgTestItemTermination {
    Brace,
    Semicolon,
    Comma,
}

fn cfg_test_item_termination(lines: &[&str], start: usize) -> CfgTestItemTermination {
    let source = lines[start..].join("\n");
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut saw_const = false;
    while index < chars.len() {
        index = skip_rust_whitespace(&chars, index);
        if chars.get(index) == Some(&'#') {
            let attribute = skip_rust_whitespace(&chars, index + 1);
            if chars.get(attribute) == Some(&'[') {
                index = matching_delimiter(&chars, attribute, '[', ']')
                    .map(|close| close + 1)
                    .unwrap_or(chars.len());
                continue;
            }
        }
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            return match chars.get(index) {
                Some('{') => CfgTestItemTermination::Brace,
                _ if saw_const => CfgTestItemTermination::Semicolon,
                _ => CfgTestItemTermination::Comma,
            };
        };
        match identifier.as_str() {
            "pub" => {
                index = skip_rust_whitespace(&chars, end);
                if chars.get(index) == Some(&'(') {
                    index = matching_delimiter(&chars, index, '(', ')')
                        .map(|close| close + 1)
                        .unwrap_or(chars.len());
                }
            }
            "async" | "unsafe" | "default" | "auto" => index = end,
            "const" => {
                saw_const = true;
                index = end;
            }
            "fn" | "mod" | "struct" | "enum" | "union" | "trait" | "impl" | "extern"
            | "macro_rules" | "macro" | "if" | "match" | "loop" | "while" | "for" => {
                return CfgTestItemTermination::Brace
            }
            "static" | "type" | "use" | "let" => {
                return CfgTestItemTermination::Semicolon;
            }
            _ if saw_const => return CfgTestItemTermination::Semicolon,
            _ => return CfgTestItemTermination::Comma,
        }
    }
    CfgTestItemTermination::Semicolon
}

fn rust_code_without_comments_and_literals(source: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let mut code = String::with_capacity(source.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
            while index < chars.len() && chars[index] != '\n' {
                push_rust_blank(&mut code, chars[index]);
                index += 1;
            }
            continue;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
            let mut depth = 1;
            push_rust_blank(&mut code, chars[index]);
            push_rust_blank(&mut code, chars[index + 1]);
            index += 2;
            while index < chars.len() && depth > 0 {
                if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                    depth += 1;
                    push_rust_blank(&mut code, chars[index]);
                    push_rust_blank(&mut code, chars[index + 1]);
                    index += 2;
                } else if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    push_rust_blank(&mut code, chars[index]);
                    push_rust_blank(&mut code, chars[index + 1]);
                    index += 2;
                } else {
                    push_rust_blank(&mut code, chars[index]);
                    index += 1;
                }
            }
            continue;
        }
        if chars[index] == 'r' {
            let mut quote = index + 1;
            while chars.get(quote) == Some(&'#') {
                quote += 1;
            }
            if chars.get(quote) == Some(&'"') {
                let hashes = quote - index - 1;
                while index <= quote {
                    push_rust_blank(&mut code, chars[index]);
                    index += 1;
                }
                while index < chars.len() {
                    let closes = chars[index] == '"'
                        && (0..hashes).all(|offset| chars.get(index + 1 + offset) == Some(&'#'));
                    push_rust_blank(&mut code, chars[index]);
                    index += 1;
                    if closes {
                        for _ in 0..hashes {
                            push_rust_blank(&mut code, chars[index]);
                            index += 1;
                        }
                        break;
                    }
                }
                continue;
            }
        }
        if chars[index] == '\'' {
            if let Some(end) = rust_char_literal_end(&chars, index) {
                while index < end {
                    push_rust_blank(&mut code, chars[index]);
                    index += 1;
                }
                continue;
            }
        }
        if chars[index] == '"' {
            push_rust_blank(&mut code, chars[index]);
            index += 1;
            let mut escaped = false;
            while index < chars.len() {
                let current = chars[index];
                push_rust_blank(&mut code, current);
                index += 1;
                if escaped {
                    escaped = false;
                } else if current == '\\' {
                    escaped = true;
                } else if current == '"' {
                    break;
                }
            }
            continue;
        }
        code.push(chars[index]);
        index += 1;
    }
    code
}

fn rust_char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start) != Some(&'\'') {
        return None;
    }
    let mut index = start + 1;
    let character = *chars.get(index)?;
    if character == '\\' {
        index += 1;
        match *chars.get(index)? {
            'x' => {
                if !chars.get(index + 1)?.is_ascii_hexdigit()
                    || !chars.get(index + 2)?.is_ascii_hexdigit()
                {
                    return None;
                }
                index += 3;
            }
            'u' => {
                index += 1;
                if chars.get(index) != Some(&'{') {
                    return None;
                }
                index += 1;
                let digits_start = index;
                while chars
                    .get(index)
                    .is_some_and(|character| character.is_ascii_hexdigit() || *character == '_')
                {
                    index += 1;
                }
                if index == digits_start || chars.get(index) != Some(&'}') {
                    return None;
                }
                index += 1;
            }
            '\n' | '\r' => return None,
            _ => index += 1,
        }
    } else {
        if matches!(character, '\n' | '\r' | '\'') {
            return None;
        }
        index += 1;
    }
    (chars.get(index) == Some(&'\'')).then_some(index + 1)
}

fn push_rust_blank(output: &mut String, character: char) {
    output.push(if character == '\n' { '\n' } else { ' ' });
}

fn is_test_only_rust_source(path: &Path) -> bool {
    if path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|component| component == "tests" || component.ends_with("_tests"))
    }) {
        return true;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == "tests" || stem.ends_with("_tests"))
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn module_family_rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![root.to_path_buf()];
    let family = root.with_extension("");
    if family.is_dir() {
        paths.extend(rust_source_files(&family));
    }
    paths.sort();
    paths.dedup();
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

fn module_family_line_count(root: &Path) -> usize {
    module_family_rust_source_files(root)
        .iter()
        .map(|path| line_count(path))
        .sum()
}

#[test]
fn source_policy_helper_collects_module_family_files() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_dir.join("src/o3_runtime_producer_forwarded_chain.rs");
    let value = crate_dir.join("src/o3_runtime_producer_forwarded_chain/value.rs");
    let files = module_family_rust_source_files(&root);

    assert!(files.contains(&root));
    assert!(files.contains(&value));
}

fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, section) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source section start `{start}`"));
    let (section, _) = section
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source section end `{end}`"));
    section
}
