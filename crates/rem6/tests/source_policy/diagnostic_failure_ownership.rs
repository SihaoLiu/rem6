use std::fs;
use std::path::Path;

use rem6::{Rem6CliError, Rem6CliFailure, Rem6RunArtifact, Rem6RunConfig};

const MAX_CLI_FAILURE_LINES: usize = 220;

#[test]
fn diagnostic_failures_use_additive_focused_ownership() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = crate_dir.parent().unwrap();
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let main = fs::read_to_string(crate_dir.join("src/main.rs")).unwrap();
    let cli_error = fs::read_to_string(crate_dir.join("src/cli_error.rs")).unwrap();
    let cli_failure_path = crate_dir.join("src/cli_failure.rs");
    let diagnostics = fs::read_to_string(crate_dir.join("src/run_failure_diagnostics.rs")).unwrap();
    let runtime_memory = fs::read_to_string(crate_dir.join("src/runtime_memory.rs")).unwrap();
    let cpu_diagnostics =
        fs::read_to_string(crates_dir.join("rem6-cpu/src/riscv_failure_diagnostic.rs")).unwrap();
    let transport_trace =
        fs::read_to_string(crates_dir.join("rem6-transport/src/trace.rs")).unwrap();

    assert!(lib.lines().any(|line| line.trim() == "mod cli_failure;"));
    assert!(cli_failure_path.is_file());
    assert!(
        fs::read_to_string(cli_failure_path)
            .unwrap()
            .lines()
            .count()
            <= MAX_CLI_FAILURE_LINES
    );
    assert!(main.contains("run_cli_with_diagnostics"));
    assert!(!cli_error.contains("ExecuteWithDiagnostics"));
    assert!(!diagnostics.contains("catch_unwind"));
    assert!(!diagnostics.contains("AssertUnwindSafe"));
    for (source, anchor) in [
        (
            diagnostics.as_str(),
            "fn provider_failures_are_aggregated_in_capture_errors()",
        ),
        (
            runtime_memory.as_str(),
            "fn memory_dump_reports_a_poisoned_store_lock()",
        ),
        (
            cpu_diagnostics.as_str(),
            "fn failure_diagnostic_snapshot_reports_a_poisoned_core_lock()",
        ),
        (
            transport_trace.as_str(),
            "fn try_snapshot_reports_a_poisoned_trace_lock()",
        ),
    ] {
        assert!(
            source.contains(anchor),
            "missing behavioral anchor `{anchor}`"
        );
    }
}

#[test]
fn public_cli_entrypoints_keep_compatible_error_signatures() {
    let _: fn(Vec<String>) -> Result<String, Rem6CliError> = rem6::run_cli::<Vec<String>, String>;
    let _: fn(Rem6RunConfig) -> Result<Rem6RunArtifact, Rem6CliError> = rem6::run_config;
    let _: fn(Vec<String>) -> Result<String, Rem6CliFailure> =
        rem6::run_cli_with_diagnostics::<Vec<String>, String>;
}
