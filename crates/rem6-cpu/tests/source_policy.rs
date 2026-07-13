use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_O3_RUNTIME_MEMORY_LINES: usize = 1200;
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
        "src/o3_runtime.rs must delegate scalar memory lifecycle state to src/o3_runtime_memory.rs"
    );

    let module_path = crate_dir.join("src/o3_runtime_memory.rs");
    assert!(
        module_path.exists(),
        "scalar O3 memory lifecycle code belongs in src/o3_runtime_memory.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let lines = module.lines().count();
    assert!(
        lines <= MAX_O3_RUNTIME_MEMORY_LINES,
        "src/o3_runtime_memory.rs exceeds {MAX_O3_RUNTIME_MEMORY_LINES} lines: {lines}"
    );

    for anchor in [
        "struct O3LiveScalarMemory",
        "fn stage_live_scalar_memory_issue",
        "fn complete_live_scalar_memory_response",
        "fn take_ready_live_scalar_memory_event",
        "fn consume_live_scalar_memory_retirement",
    ] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_memory.rs is missing lifecycle owner `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "src/o3_runtime.rs still owns scalar memory lifecycle `{anchor}`"
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
