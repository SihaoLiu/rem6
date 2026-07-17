use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_O3_RUNTIME_ISSUE_LINES: usize = 800;
const MAX_O3_RUNTIME_MEMORY_LINES: usize = 1200;
const MAX_O3_RUNTIME_ROOT_LINES: usize = 1200;
const MAX_O3_RUNTIME_LIVE_WINDOW_LINES: usize = 800;
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
        "pub(crate) fn apply_completed_data_access(",
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
fn public_scalar_memory_lifecycle_methods_remain_deprecated_live_data_forwards() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw_memory = fs::read_to_string(crate_dir.join("src/o3_runtime_memory.rs")).unwrap();
    let memory = production_rust_source(&raw_memory);

    for (deprecated, forward) in [
        (
            "pub fn o3_scalar_memory_lifecycle_is_quiescent(&self) -> bool",
            "self.o3_live_data_access_lifecycle_is_quiescent()",
        ),
        (
            "pub fn has_pending_o3_scalar_memory_retirement(&self) -> bool",
            "self.has_pending_o3_live_data_access_retirement()",
        ),
        (
            "pub fn pending_o3_scalar_memory_retirement_count(&self) -> usize",
            "self.pending_o3_live_data_access_retirement_count()",
        ),
        (
            "pub fn owns_pending_o3_scalar_memory_retirement(",
            "self.owns_pending_o3_live_data_access_retirement(fetch_request)",
        ),
        (
            "pub fn ready_o3_scalar_memory_event_kind(&self) -> Option<RiscvDataAccessEventKind>",
            "self.ready_o3_live_data_access_event_kind()",
        ),
    ] {
        assert!(
            memory.contains(deprecated),
            "public scalar-memory compatibility method is missing `{deprecated}`"
        );
        assert!(
            memory.contains(forward),
            "public scalar-memory compatibility method must forward to `{forward}`"
        );
    }
    assert_eq!(
        raw_memory.matches("#[deprecated(note = \"use ").count(),
        5,
        "each scalar-memory public compatibility method must be explicitly deprecated"
    );
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
        "pub(crate) struct O3LiveControlOperands",
        "pub(crate) fn o3_live_control_operands(",
        "kind: BranchTargetKind",
        "sources: Vec<Register>",
        "destination: Option<Register>",
        "pub(crate) const fn destination(&self) -> Option<Register>",
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
    for anchor in [
        "fn scalar_memory_stops_live_retire_window_before_memory_and_younger_rows(",
        "fn completed_live_load_forwards_into_dependent_alu_candidate(",
        "fn invalidated_speculative_producer_revokes_dependent_issue_timing(",
        "fn live_rename_overlay_preserves_canonical_register_order(",
    ] {
        assert!(
            tests.contains(anchor),
            "src/o3_runtime_live_window_tests.rs is missing `{anchor}`"
        );
    }
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
            let item_indent = line.len() - trimmed.len();
            index += 1;
            while index < lines.len() && lines[index].trim().is_empty() {
                index += 1;
            }
            let mut opened = false;
            while index < lines.len() {
                let item = lines[index];
                let item_trimmed = item.trim();
                let single_line =
                    !opened && (item_trimmed.ends_with(';') || item_trimmed.ends_with(','));
                opened |= item_trimmed.contains('{');
                let indent = item.len() - item.trim_start().len();
                let closed = opened && indent == item_indent && item_trimmed == "}";
                index += 1;
                if single_line || closed {
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

fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let (_, section) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source section start `{start}`"));
    let (section, _) = section
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source section end `{end}`"));
    section
}
