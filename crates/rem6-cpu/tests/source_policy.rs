use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_O3_RUNTIME_DEEP_CLEANUP_TEST_LINES: usize = 350;
const MAX_O3_RUNTIME_ISSUE_DEPENDENCY_LINES: usize = 500;
const MAX_O3_RUNTIME_ISSUE_DEPENDENCY_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_ISSUE_CALENDAR_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_CALENDAR_TEST_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_QUEUE_LINES: usize = 600;
const MAX_O3_RUNTIME_ISSUE_QUEUE_TEST_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_LINES: usize = 800;
const MAX_O3_RUNTIME_MEMORY_LINES: usize = 1200;
const MAX_O3_RUNTIME_MEMORY_RESULT_ADMISSION_LINES: usize = 220;
const MAX_O3_RUNTIME_TRANSLATED_RESULT_PAIR_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_ROOT_LINES: usize = 1200;
const MAX_O3_RUNTIME_CONTROL_WINDOW_TEST_ROOT_LINES: usize = 1350;
const MAX_O3_RUNTIME_CONTROL_WINDOW_LIFECYCLE_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_CONTROL_WINDOW_LINEAGE_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_CONTROL_WINDOW_PRODUCER_FORWARDED_TARGET_TEST_LINES: usize = 225;
const MAX_O3_RUNTIME_CONTROL_WINDOW_NONADJACENT_TARGET_TEST_LINES: usize = 180;
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
const MAX_RISCV_DATA_ACCESS_RESULT_LINES: usize = 450;
const MAX_RISCV_DATA_ACCESS_RESULT_TRANSLATION_LINES: usize = 250;
const MAX_RISCV_DATA_ACCESS_RESULT_PAIR_POLICY_LINES: usize = 100;
const MAX_RISCV_DATA_ACCESS_RESULT_EFFECT_POLICY_LINES: usize = 120;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_LINES: usize = 200;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_FETCH_TEST_LINES: usize = 450;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_TWO_PENDING_FETCH_TEST_LINES: usize = 350;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_THREE_PENDING_FETCH_TEST_LINES: usize = 450;
const MAX_RISCV_UNISSUED_DATA_LINES: usize = 110;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_ISSUE_LINES: usize = 350;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_ISSUE_TEST_LINES: usize = 550;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_MULTIPLE_ISSUE_TEST_LINES: usize = 500;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_THREE_PENDING_ISSUE_TEST_LINES: usize = 450;
const MAX_RISCV_RETAINED_DATA_ACCESS_RESULT_LINES: usize = 100;
const MAX_RISCV_MEMORY_RESULT_AUTHORIZATION_LINES: usize = 150;
const MAX_RISCV_MEMORY_RESULT_TRANSLATED_AUTHORIZATION_LINES: usize = 220;
const MAX_RISCV_BUFFERED_EFFECT_LINES: usize = 220;
const MAX_O3_RUNTIME_PENDING_ADDRESS_LINES: usize = 650;
const MAX_O3_RUNTIME_PENDING_ADDRESS_SET_LINES: usize = 350;
const MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_LINES: usize = 350;
const MAX_O3_RUNTIME_ISSUE_PENDING_ADDRESS_LINES: usize = 300;
const MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FACADE_LINES: usize = 300;
const MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_TEST_LINES: usize = 450;
const MAX_O3_RUNTIME_PENDING_ADDRESS_SCHEDULING_TEST_LINES: usize = 550;
const MAX_O3_RUNTIME_PENDING_ADDRESS_LIFECYCLE_TEST_LINES: usize = 650;
const MAX_O3_RUNTIME_PENDING_ADDRESS_MULTIPLE_TEST_LINES: usize = 550;
const MAX_O3_RUNTIME_PENDING_ADDRESS_THREE_PENDING_TEST_LINES: usize = 450;
const MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FAMILY_LINES: usize = 2550;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_STAGE_LINES: usize = 250;
const MAX_RISCV_DETAILED_O3_CONTROL_TEST_ROOT_LINES: usize = 450;
const MAX_RISCV_DETAILED_O3_LINKED_CONTROL_TEST_LINES: usize = 1500;
const MAX_RISCV_DETAILED_O3_LINKED_CONTROL_FETCH_RESPONSE_TEST_LINES: usize = 200;
const MAX_O3_RUNTIME_LIVE_WINDOW_LINES: usize = 800;
const MAX_O3_RUNTIME_LIVE_WINDOW_TEST_LINES: usize = 1100;
const MAX_O3_RUNTIME_LIVE_WINDOW_IDENTITY_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_WRITEBACK_LINES: usize = 800;
const MAX_O3_RUNTIME_WRITEBACK_REPLAN_LINES: usize = 600;
const MAX_O3_RUNTIME_WRITEBACK_OWNERSHIP_LINES: usize = 300;
const MAX_RISCV_O3_WRITEBACK_WAKE_LINES: usize = 800;
const MAX_RISCV_DATA_ISSUE_TEST_ROOT_LINES: usize = 1500;
const MAX_RISCV_DATA_ISSUE_LIFECYCLE_TEST_LINES: usize = 450;
const MAX_RISCV_O3_RESULT_PAIR_ADMISSION_LINES: usize = 300;
const MAX_RISCV_TRANSLATED_RESULT_PAIR_TRANSLATION_LINES: usize = 500;
const MAX_RISCV_TRANSLATED_RESULT_PAIR_ISSUE_TEST_LINES: usize = 450;
const MAX_RISCV_TRANSLATED_RESULT_PAIR_ISSUE_FIXTURE_LINES: usize = 400;
const MAX_RISCV_TRANSLATED_RESULT_PAIR_ISSUE_REVIEW_LINES: usize = 160;
const MAX_RISCV_FAILURE_DIAGNOSTIC_LINES: usize = 300;
const MAX_RISCV_PRODUCER_FORWARDED_DESCENDANT_LINES: usize = 120;
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
fn riscv_producer_forwarded_descendant_staging_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/riscv_live_retire_window.rs");
    let child_path =
        crate_dir.join("src/riscv_live_retire_window/producer_forwarded_descendant.rs");
    let root = fs::read_to_string(&root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();

    assert!(root
        .lines()
        .any(|line| line.trim() == "mod producer_forwarded_descendant;"));
    assert!(child_path.is_file());
    assert!(
        child.lines().count() <= MAX_RISCV_PRODUCER_FORWARDED_DESCENDANT_LINES,
        "producer_forwarded_descendant.rs exceeds {MAX_RISCV_PRODUCER_FORWARDED_DESCENDANT_LINES} lines"
    );
    for function in [
        "stage_o3_producer_forwarded_control_descendant",
        "stage_o3_producer_forwarded_control_descendant_for_response",
    ] {
        assert!(production_defines_exact_function(&child, function));
        assert!(!production_defines_exact_function(&root, function));
    }
}

#[test]
fn riscv_data_issue_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let data_issue_rs = crate_dir.join("src/riscv_data_issue.rs");
    let data_issue = fs::read_to_string(&data_issue_rs).unwrap();
    let buffered_effect_path = crate_dir.join("src/riscv_data_issue/buffered_effect.rs");

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
    assert!(
        data_issue.contains("mod buffered_effect;"),
        "riscv_data_issue.rs must delegate buffered side effects to buffered_effect.rs"
    );
    assert!(buffered_effect_path.is_file());
    assert!(!crate_dir
        .join("src/riscv_data_issue/buffered_store.rs")
        .exists());
    let buffered_effect = fs::read_to_string(buffered_effect_path).unwrap();
    assert!(
        buffered_effect.lines().count() <= MAX_RISCV_BUFFERED_EFFECT_LINES,
        "buffered_effect.rs exceeds {MAX_RISCV_BUFFERED_EFFECT_LINES} lines"
    );
    for anchor in [
        "struct BufferedO3Effect",
        "fn o3_buffered_effect_predecessor(",
        "fn schedule_buffered_o3_effect(",
        "fn record_buffered_o3_effect_submission(",
    ] {
        assert!(
            buffered_effect.contains(anchor),
            "buffered_effect.rs is missing `{anchor}`"
        );
    }
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
fn o3_translated_result_pair_issue_admission_has_focused_ownership() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = crate_dir.join("src");
    let o3_root_path = src_dir.join("o3_runtime.rs");
    let o3_owner_path = src_dir.join("o3_runtime_memory_result_admission.rs");
    let calendar_path = src_dir.join("o3_runtime_issue/calendar.rs");
    let runtime_test_root_path = src_dir.join("o3_runtime_memory_result_tests.rs");
    let runtime_test_path =
        src_dir.join("o3_runtime_memory_result_tests/translated_mmio_result_pair.rs");
    let issue_root_path = src_dir.join("riscv_data_issue.rs");
    let progress_owner_path = src_dir.join("riscv_data_issue/o3_result_pair_admission.rs");
    let translation_root_path = src_dir.join("riscv_translation.rs");
    let translation_owner_path = src_dir.join("riscv_translation/o3_result_pair.rs");
    let issue_test_root_path = src_dir.join("riscv_data_issue_tests.rs");
    let issue_test_path = src_dir.join("riscv_data_issue_tests/translated_mmio_result_pair.rs");
    let issue_test_fixture_path =
        src_dir.join("riscv_data_issue_tests/translated_mmio_result_pair/fixture.rs");
    let issue_test_review_path =
        src_dir.join("riscv_data_issue_tests/translated_mmio_result_pair/review_boundaries.rs");
    let window_path = src_dir.join("riscv_memory_result_window.rs");

    let o3_root = fs::read_to_string(&o3_root_path).unwrap();
    let runtime_test_root = fs::read_to_string(&runtime_test_root_path).unwrap();
    let issue_root = fs::read_to_string(&issue_root_path).unwrap();
    let translation_root = fs::read_to_string(&translation_root_path).unwrap();
    let issue_test_root = fs::read_to_string(&issue_test_root_path).unwrap();
    assert_eq!(
        path_owned_module_declaration_count(
            &o3_root,
            "o3_runtime_memory_result_admission.rs",
            "o3_runtime_memory_result_admission",
        ),
        1
    );
    assert!(runtime_test_root.contains(
        "#[path = \"o3_runtime_memory_result_tests/translated_mmio_result_pair.rs\"]\nmod translated_mmio_result_pair;"
    ));
    assert_eq!(
        top_level_external_module_declaration_count(&issue_root, "o3_result_pair_admission"),
        1
    );
    assert_eq!(
        top_level_external_module_declaration_count(&translation_root, "o3_result_pair"),
        1
    );
    assert!(issue_root.contains(
        "#[cfg(test)]\n#[path = \"riscv_data_issue_tests.rs\"]\nmod riscv_data_issue_tests;"
    ));
    assert!(issue_test_root.contains(
        "#[path = \"riscv_data_issue_tests/translated_mmio_result_pair.rs\"]\nmod translated_mmio_result_pair;"
    ));

    for path in [
        &o3_owner_path,
        &calendar_path,
        &runtime_test_path,
        &progress_owner_path,
        &translation_owner_path,
        &issue_test_path,
        &issue_test_fixture_path,
        &issue_test_review_path,
        &window_path,
    ] {
        assert!(path.is_file(), "missing focused owner {}", path.display());
    }
    assert!(line_count(&o3_owner_path) <= MAX_O3_RUNTIME_MEMORY_RESULT_ADMISSION_LINES);
    assert!(line_count(&runtime_test_path) <= MAX_O3_RUNTIME_TRANSLATED_RESULT_PAIR_TEST_LINES);
    assert!(line_count(&progress_owner_path) <= MAX_RISCV_O3_RESULT_PAIR_ADMISSION_LINES);
    assert!(
        line_count(&translation_owner_path) <= MAX_RISCV_TRANSLATED_RESULT_PAIR_TRANSLATION_LINES
    );
    for owner_path in [
        &o3_owner_path,
        &progress_owner_path,
        &translation_owner_path,
    ] {
        let owner = fs::read_to_string(owner_path).unwrap();
        assert!(include_macro_lines(&owner).is_empty());
        assert!(external_module_declaration_lines(&owner).is_empty());
    }
    assert!(line_count(&issue_test_path) <= MAX_RISCV_TRANSLATED_RESULT_PAIR_ISSUE_TEST_LINES);
    assert!(
        line_count(&issue_test_fixture_path)
            <= MAX_RISCV_TRANSLATED_RESULT_PAIR_ISSUE_FIXTURE_LINES
    );
    assert!(
        line_count(&issue_test_review_path) <= MAX_RISCV_TRANSLATED_RESULT_PAIR_ISSUE_REVIEW_LINES
    );

    let runtime_tests = fs::read_to_string(&runtime_test_path).unwrap();
    assert!(include_macro_lines(&runtime_tests).is_empty());
    assert!(external_module_declaration_lines(&runtime_tests).is_empty());

    let issue_tests = fs::read_to_string(&issue_test_path).unwrap();
    assert!(include_macro_lines(&issue_tests).is_empty());
    assert_eq!(path_attribute_lines(&issue_tests).len(), 2);
    assert_eq!(external_module_declaration_lines(&issue_tests).len(), 2);
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_tests,
            "translated_mmio_result_pair/fixture.rs",
            "fixture",
        ),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_tests,
            "translated_mmio_result_pair/review_boundaries.rs",
            "review_boundaries",
        ),
        1
    );

    let issue_test_fixture = fs::read_to_string(&issue_test_fixture_path).unwrap();
    assert!(include_macro_lines(&issue_test_fixture).is_empty());
    assert!(path_attribute_lines(&issue_test_fixture).is_empty());
    assert!(external_module_declaration_lines(&issue_test_fixture).is_empty());
    let issue_test_review = fs::read_to_string(&issue_test_review_path).unwrap();
    assert!(include_macro_lines(&issue_test_review).is_empty());
    assert!(path_attribute_lines(&issue_test_review).is_empty());
    assert!(external_module_declaration_lines(&issue_test_review).is_empty());

    for test in [
        "translated_result_pair_memory_width_one_selects_the_next_tick",
        "translated_result_pair_memory_width_two_reuses_the_head_tick",
        "translated_result_pair_total_width_one_still_selects_the_next_tick",
        "translated_result_pair_max_tick_free_selects_max_tick",
        "translated_result_pair_max_tick_occupied_returns_none",
        "scalar_memory_prefix_is_not_an_exact_memory_result_head",
        "memory_result_window_head_matches_its_exact_identity",
    ] {
        assert_eq!(rust_test_function_definition_count(&runtime_tests, test), 1);
    }
    for test in [
        "translated_result_pair_without_outstanding_data_is_ordinary",
        "translated_result_pair_exact_resident_pair_is_ready",
        "translated_result_pair_memory_width_waits_for_selected_tick",
        "translated_result_pair_rejects_unrelated_outstanding_request",
        "translated_result_pair_blocks_multiple_or_unrelated_auxiliary_state",
        "translated_split_gapped_result_pair_is_ready_with_two_memory_slots",
        "translated_split_gapped_result_pair_waits_with_one_memory_slot",
        "translated_result_pair_exact_pending_and_ready_keys_preserve_progress",
        "translated_result_pair_rejects_mismatched_pending_and_ready_map_keys",
        "translated_result_pair_requires_exact_outstanding_access_identity",
        "translated_result_pair_requires_coherent_pending_and_ready_spans",
    ] {
        assert_eq!(rust_test_function_definition_count(&issue_tests, test), 1);
    }
    for test in [
        "translated_result_pair_rejects_aliased_physical_ranges",
        "translated_alias_waits_for_older_o3_retirement_after_response",
        "serial_driver_rechecks_physical_alias_after_translation_completion",
        "serial_translated_result_pair_wait_requests_selected_issue_tick",
        "parallel_translated_result_pair_wait_requests_selected_issue_tick",
    ] {
        assert_eq!(
            rust_test_function_definition_count(&issue_test_review, test),
            1
        );
    }

    for (function, owner_path) in [
        ("next_memory_slot_at_or_after", &calendar_path),
        ("memory_result_head_identity", &o3_owner_path),
        ("next_memory_result_issue_tick", &o3_owner_path),
        ("matches_exact_memory_result_head", &o3_owner_path),
        ("sole_memory_result_head", &o3_owner_path),
        (
            "discard_live_staged_suffix_for_fetch_identity",
            &o3_owner_path,
        ),
        (
            "translated_result_pair_has_translation_work",
            &progress_owner_path,
        ),
        ("translated_result_pair_progress", &progress_owner_path),
        (
            "has_unbound_translated_result_state",
            &translation_owner_path,
        ),
        (
            "translated_result_pair_retry_wake_tick",
            &translation_owner_path,
        ),
        (
            "discard_translated_result_pair_from",
            &translation_owner_path,
        ),
        (
            "translated_result_authorization_is_pending",
            &translation_owner_path,
        ),
        ("bind_translated_result_range", &translation_owner_path),
        ("bind_translated_result_target", &translation_owner_path),
        (
            "translated_result_pair_prepare_result",
            &translation_owner_path,
        ),
        (
            "reject_uncacheable_translated_result_memory_target",
            &translation_owner_path,
        ),
        (
            "prepare_ready_translated_mmio_data_access",
            &translation_owner_path,
        ),
        ("prepare_translated_data_access", &translation_owner_path),
        ("has_exact_translated_result_pair_window", &window_path),
    ] {
        let owner = production_rust_source(&fs::read_to_string(owner_path).unwrap());
        assert!(
            production_defines_exact_function(&owner, function),
            "{} must own `{function}`",
            owner_path.display()
        );
        for path in rust_source_files(&src_dir) {
            if path == *owner_path
                || is_test_only_rust_source(path.strip_prefix(crate_dir).unwrap())
            {
                continue;
            }
            let source = production_rust_source(&fs::read_to_string(&path).unwrap());
            assert!(
                !production_defines_exact_function(&source, function),
                "{} duplicates focused owner `{function}`",
                path.display()
            );
        }
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
fn riscv_detailed_o3_linked_control_tests_have_focused_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/riscv_fetch_ahead/tests/detailed_o3_control.rs");
    let linked_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs");
    let response_path = crate_dir
        .join("src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control/fetch_response.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "detailed_o3_control/linked_control.rs",
            "linked_control"
        ),
        1,
        "the detailed O3 control test root must declare its linked-control child"
    );
    assert!(
        linked_path.exists(),
        "linked-control tests belong in {}",
        linked_path.display()
    );

    let linked = fs::read_to_string(&linked_path).unwrap();
    assert_eq!(
        path_owned_module_declaration_count(
            &linked,
            "linked_control/fetch_response.rs",
            "fetch_response",
        ),
        1,
        "the linked-control owner must declare its fetch-response child"
    );
    assert!(
        response_path.exists(),
        "fetch-response tests belong in {}",
        response_path.display()
    );
    let response = fs::read_to_string(&response_path).unwrap();
    for (path, source) in [
        (&root_path, &root),
        (&linked_path, &linked),
        (&response_path, &response),
    ] {
        let include_lines = include_macro_lines(source);
        assert!(
            include_lines.is_empty(),
            "{} must use path-owned modules instead of include! fragments; found lines {include_lines:?}",
            path.display(),
        );
    }

    let root_code = rust_code_without_comments_and_literals(&root);
    let linked_code = rust_code_without_comments_and_literals(&linked);
    let response_code = rust_code_without_comments_and_literals(&response);
    for function in [
        "producer_forwarded_target_response_stages_descendant_without_later_drive_turn",
        "producer_forwarded_target_response_respects_frontend_gates_and_exact_completion",
    ] {
        assert!(
            production_defines_exact_function(&response_code, function),
            "fetch-response owner is missing `{function}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, function)
                && !production_defines_exact_function(&linked_code, function),
            "fetch-response anchor `{function}` escaped its focused child"
        );
    }
    for function in [
        "recorded_same_window_coroutine_core",
        "detailed_live_same_link_control_uses_runtime_forwarded_target",
        "detailed_scalar_window_direct_call_follows_target_and_pushes_ras",
        "detailed_recorded_coroutine_accepts_exact_pop_then_push",
        "detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction",
    ] {
        assert!(
            production_defines_exact_function(&linked_code, function),
            "linked-control owner is missing `{function}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, function),
            "detailed O3 control root still owns `{function}`"
        );
    }

    for function in [
        "live_same_link_core",
        "detailed_scalar_window_returns_existing_branch_prediction_decision",
        "detailed_control_target_authority_rejects_non_predicted_decision",
        "detailed_split_control_keys_prediction_to_prefix_request",
    ] {
        assert!(
            production_defines_exact_function(&root_code, function),
            "detailed O3 control root is missing `{function}`"
        );
        assert!(
            !production_defines_exact_function(&linked_code, function),
            "linked-control child must not own `{function}`"
        );
    }
    assert!(
        production_function_is_visible(&root_code, "live_same_link_core"),
        "live_same_link_core must remain visible to its sibling test caller"
    );

    let root_lines = line_count(&root_path);
    assert!(
        root_lines <= MAX_RISCV_DETAILED_O3_CONTROL_TEST_ROOT_LINES,
        "detailed_o3_control.rs exceeds its focused root budget: {root_lines}"
    );
    let linked_lines = line_count(&linked_path);
    assert!(
        linked_lines <= MAX_RISCV_DETAILED_O3_LINKED_CONTROL_TEST_LINES,
        "linked_control.rs exceeds its focused child budget: {linked_lines}"
    );
    let response_lines = line_count(&response_path);
    assert!(
        response_lines <= MAX_RISCV_DETAILED_O3_LINKED_CONTROL_FETCH_RESPONSE_TEST_LINES,
        "fetch_response.rs exceeds its focused child budget: {response_lines}"
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
fn task3_pending_data_address_staging_stays_in_focused_owners() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime_path = crate_dir.join("src/o3_runtime.rs");
    let memory_path = crate_dir.join("src/o3_runtime_memory.rs");
    let retire_path = crate_dir.join("src/riscv_live_retire_window.rs");
    let pending_path = crate_dir.join("src/o3_runtime_pending_address.rs");
    let pending_set_path = crate_dir.join("src/o3_runtime_pending_address_set.rs");
    let pending_staging_path = crate_dir.join("src/o3_runtime_pending_address_staging.rs");
    let issue_root_path = crate_dir.join("src/o3_runtime_issue.rs");
    let issue_calendar_path = crate_dir.join("src/o3_runtime_issue/calendar.rs");
    let issue_pending_path = crate_dir.join("src/o3_runtime_issue/pending_address.rs");
    let writeback_wake_path = crate_dir.join("src/riscv_o3_writeback_wake.rs");
    let test_root_path = crate_dir.join("src/o3_runtime_pending_address_tests.rs");
    let staging_test_path = crate_dir.join("src/o3_runtime_pending_address_tests/staging.rs");
    let scheduling_test_path = crate_dir.join("src/o3_runtime_pending_address_tests/scheduling.rs");
    let lifecycle_test_path = crate_dir.join("src/o3_runtime_pending_address_tests/lifecycle.rs");
    let multiple_test_path = crate_dir.join("src/o3_runtime_pending_address_tests/multiple.rs");
    let three_pending_test_path =
        crate_dir.join("src/o3_runtime_pending_address_tests/three_pending.rs");
    let stage_child_path =
        crate_dir.join("src/riscv_live_retire_window/dependent_result_address.rs");

    for path in [
        &pending_path,
        &pending_set_path,
        &pending_staging_path,
        &issue_root_path,
        &issue_calendar_path,
        &issue_pending_path,
        &writeback_wake_path,
        &test_root_path,
        &staging_test_path,
        &scheduling_test_path,
        &lifecycle_test_path,
        &multiple_test_path,
        &three_pending_test_path,
        &stage_child_path,
    ] {
        assert!(
            path.is_file(),
            "missing Task 3 focused file {}",
            path.display()
        );
    }

    let runtime = fs::read_to_string(&runtime_path).unwrap();
    let memory = fs::read_to_string(&memory_path).unwrap();
    let retire = fs::read_to_string(&retire_path).unwrap();
    let pending = fs::read_to_string(&pending_path).unwrap();
    let pending_set = fs::read_to_string(&pending_set_path).unwrap();
    let pending_staging = fs::read_to_string(&pending_staging_path).unwrap();
    let issue_root = fs::read_to_string(&issue_root_path).unwrap();
    let issue_calendar = fs::read_to_string(&issue_calendar_path).unwrap();
    let issue_pending = fs::read_to_string(&issue_pending_path).unwrap();
    let writeback_wake = fs::read_to_string(&writeback_wake_path).unwrap();
    let test_root = fs::read_to_string(&test_root_path).unwrap();
    let staging_test = fs::read_to_string(&staging_test_path).unwrap();
    let scheduling_test = fs::read_to_string(&scheduling_test_path).unwrap();
    let lifecycle_test = fs::read_to_string(&lifecycle_test_path).unwrap();
    let multiple_test = fs::read_to_string(&multiple_test_path).unwrap();
    let three_pending_test = fs::read_to_string(&three_pending_test_path).unwrap();
    let stage_child = fs::read_to_string(&stage_child_path).unwrap();

    assert_eq!(
        path_owned_module_declaration_count(
            &runtime,
            "o3_runtime_pending_address.rs",
            "o3_runtime_pending_address"
        ),
        1,
        "src/o3_runtime.rs must attach the pending-address owner exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &runtime,
            "o3_runtime_pending_address_set.rs",
            "o3_runtime_pending_address_set"
        ),
        1,
        "src/o3_runtime.rs must attach the pending-address set owner exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &runtime,
            "o3_runtime_pending_address_staging.rs",
            "o3_runtime_pending_address_staging"
        ),
        1,
        "src/o3_runtime.rs must attach the pending-address staging owner exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_root,
            "o3_runtime_issue/pending_address.rs",
            "pending_address"
        ),
        1,
        "src/o3_runtime_issue.rs must attach the pending-address scheduler owner exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &runtime,
            "o3_runtime_pending_address_tests.rs",
            "o3_runtime_pending_address_tests"
        ),
        1,
        "src/o3_runtime.rs must attach the pending-address test facade exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &test_root,
            "o3_runtime_pending_address_tests/lifecycle.rs",
            "lifecycle"
        ),
        1,
        "pending-address tests must use the focused lifecycle child exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &test_root,
            "o3_runtime_pending_address_tests/staging.rs",
            "staging"
        ),
        1,
        "pending-address tests must use the focused staging child exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &test_root,
            "o3_runtime_pending_address_tests/scheduling.rs",
            "scheduling"
        ),
        1,
        "pending-address tests must use the focused scheduling child exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &test_root,
            "o3_runtime_pending_address_tests/multiple.rs",
            "multiple"
        ),
        1,
        "pending-address tests must use the focused multiple-row child exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &test_root,
            "o3_runtime_pending_address_tests/three_pending.rs",
            "three_pending"
        ),
        1,
        "pending-address tests must use the focused three-pending child exactly once"
    );
    assert_eq!(
        rust_code_without_comments_and_literals(&retire)
            .matches("mod dependent_result_address;")
            .count(),
        1,
        "src/riscv_live_retire_window.rs must attach the staging child exactly once"
    );

    assert!(include_macro_lines(&pending).is_empty());
    assert!(include_macro_lines(&pending_set).is_empty());
    assert!(include_macro_lines(&pending_staging).is_empty());
    assert!(include_macro_lines(&issue_pending).is_empty());
    assert!(include_macro_lines(&test_root).is_empty());
    assert!(include_macro_lines(&staging_test).is_empty());
    assert!(include_macro_lines(&scheduling_test).is_empty());
    assert!(include_macro_lines(&lifecycle_test).is_empty());
    assert!(include_macro_lines(&multiple_test).is_empty());
    assert!(include_macro_lines(&three_pending_test).is_empty());
    assert!(include_macro_lines(&stage_child).is_empty());
    assert!(
        external_module_declaration_lines(&pending).is_empty()
            && path_attribute_lines(&pending).is_empty()
            && external_module_declaration_lines(&pending_set).is_empty()
            && path_attribute_lines(&pending_set).is_empty()
            && external_module_declaration_lines(&pending_staging).is_empty()
            && path_attribute_lines(&pending_staging).is_empty()
            && external_module_declaration_lines(&issue_pending).is_empty()
            && path_attribute_lines(&issue_pending).is_empty()
            && external_module_declaration_lines(&staging_test).is_empty()
            && path_attribute_lines(&staging_test).is_empty()
            && external_module_declaration_lines(&scheduling_test).is_empty()
            && path_attribute_lines(&scheduling_test).is_empty()
            && external_module_declaration_lines(&lifecycle_test).is_empty()
            && path_attribute_lines(&lifecycle_test).is_empty()
            && external_module_declaration_lines(&multiple_test).is_empty()
            && path_attribute_lines(&multiple_test).is_empty()
            && external_module_declaration_lines(&three_pending_test).is_empty()
            && path_attribute_lines(&three_pending_test).is_empty()
            && external_module_declaration_lines(&stage_child).is_empty()
            && path_attribute_lines(&stage_child).is_empty(),
        "Task 3 leaf owners must not declare child modules"
    );

    assert!(
        line_count(&pending_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_LINES,
        "o3_runtime_pending_address.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_LINES} lines"
    );
    assert!(
        line_count(&pending_set_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_SET_LINES,
        "o3_runtime_pending_address_set.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_SET_LINES} lines"
    );
    assert!(
        line_count(&pending_staging_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_LINES,
        "o3_runtime_pending_address_staging.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_LINES} lines"
    );
    assert!(
        line_count(&issue_pending_path) <= MAX_O3_RUNTIME_ISSUE_PENDING_ADDRESS_LINES,
        "o3_runtime_issue/pending_address.rs exceeds {MAX_O3_RUNTIME_ISSUE_PENDING_ADDRESS_LINES} lines"
    );
    assert!(
        line_count(&test_root_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FACADE_LINES,
        "o3_runtime_pending_address_tests.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FACADE_LINES} lines"
    );
    assert!(
        line_count(&staging_test_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_TEST_LINES,
        "o3_runtime_pending_address_tests/staging.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_TEST_LINES} lines"
    );
    assert!(
        line_count(&scheduling_test_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_SCHEDULING_TEST_LINES,
        "o3_runtime_pending_address_tests/scheduling.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_SCHEDULING_TEST_LINES} lines"
    );
    assert!(
        line_count(&lifecycle_test_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_LIFECYCLE_TEST_LINES,
        "o3_runtime_pending_address_tests/lifecycle.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_LIFECYCLE_TEST_LINES} lines"
    );
    assert!(
        line_count(&multiple_test_path) <= MAX_O3_RUNTIME_PENDING_ADDRESS_MULTIPLE_TEST_LINES,
        "o3_runtime_pending_address_tests/multiple.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_MULTIPLE_TEST_LINES} lines"
    );
    assert!(
        line_count(&three_pending_test_path)
            <= MAX_O3_RUNTIME_PENDING_ADDRESS_THREE_PENDING_TEST_LINES,
        "o3_runtime_pending_address_tests/three_pending.rs exceeds {MAX_O3_RUNTIME_PENDING_ADDRESS_THREE_PENDING_TEST_LINES} lines"
    );
    assert!(
        line_count(&test_root_path)
            + line_count(&staging_test_path)
            + line_count(&scheduling_test_path)
            + line_count(&lifecycle_test_path)
            + line_count(&multiple_test_path)
            + line_count(&three_pending_test_path)
            < MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FAMILY_LINES,
        "pending-address test family must remain below {MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FAMILY_LINES} lines"
    );
    assert!(
        line_count(&stage_child_path) <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_STAGE_LINES,
        "riscv_live_retire_window/dependent_result_address.rs exceeds {MAX_RISCV_DEPENDENT_RESULT_ADDRESS_STAGE_LINES} lines"
    );

    let pending_code = production_rust_source(&pending);
    let pending_set_code = production_rust_source(&pending_set);
    let pending_staging_code = production_rust_source(&pending_staging);
    let issue_root_code = production_rust_source(&issue_root);
    let issue_calendar_code = production_rust_source(&issue_calendar);
    let issue_pending_code = production_rust_source(&issue_pending);
    let writeback_wake_code = production_rust_source(&writeback_wake);
    let runtime_code = production_rust_source(&runtime);
    let memory_code = production_rust_source(&memory);
    let retire_code = production_rust_source(&retire);
    let lib_code =
        production_rust_source(&fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap());
    let stage_child_code = production_rust_source(&stage_child);
    let production_code = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .filter(|path| !is_test_only_rust_source(path.strip_prefix(crate_dir).unwrap()))
        .map(|path| production_rust_source(&fs::read_to_string(path).unwrap()))
        .collect::<String>();

    for stale in [
        "pending_data_address_request_sequence",
        "pending_data_address_materialization_matches",
    ] {
        assert!(
            !production_defines_exact_function(&production_code, stale),
            "stale pending request adapter `{stale}` must have no production definition"
        );
    }

    assert_eq!(
        pending_code
            .matches("pub(super) struct O3PendingDataAddress {")
            .count(),
        1,
        "pending-address owner must define exactly one O3PendingDataAddress struct"
    );
    assert_eq!(
        pending_code
            .matches("pub(crate) struct O3PendingDataAddressRequest {")
            .count(),
        1,
        "pending-address request must retain its exact crate-visible type"
    );
    assert_eq!(
        pending_code
            .matches("pub(super) struct O3PendingDataAddressRootHead {")
            .count(),
        1,
        "pending-address owner must define one root-head metadata type"
    );
    for field in [
        "pub(crate) fetch_predecessor_request: MemoryRequestId,",
        "pub(crate) fetch: CpuFetchEvent,",
        "pub(crate) consumed_requests: Vec<MemoryRequestId>,",
        "pub(crate) decoded: RiscvDecodedInstruction,",
        "pub(crate) producer_register: Register,",
    ] {
        assert_eq!(
            pending_code.matches(field).count(),
            1,
            "pending-address request is missing exact field `{field}`"
        );
    }
    for field in [
        "pub(super) fetch_predecessor_request: MemoryRequestId,",
        "pub(super) producer_register: Register,",
        "pub(super) producer_sequence: u64,",
        "pub(super) root_head: O3PendingDataAddressRootHead,",
        "pub(super) fetch_request: MemoryRequestId,",
        "pub(super) range: AddressRange,",
        "pub(super) atomic_head: bool,",
    ] {
        assert_eq!(
            pending_code.matches(field).count(),
            1,
            "pending-address metadata is missing exact field `{field}`"
        );
    }
    assert_eq!(pending_code.matches("pub(super) sequence: u64,").count(), 2);
    for stale in [
        "pub(super) producer_fetch: MemoryRequestId,",
        "pub(super) head_range: AddressRange,",
    ] {
        assert!(
            !pending_code.contains(stale),
            "pending-address row retains coupled metadata `{stale}`"
        );
    }
    assert_eq!(
        pending_set_code
            .matches("pub(super) struct O3PendingDataAddresses {")
            .count(),
        1,
        "pending-address set owner must define exactly one O3PendingDataAddresses struct"
    );
    assert_eq!(
        pending_set_code
            .matches("pub(crate) const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 3;")
            .count(),
        1,
        "pending-address set owner must define the exact capacity-three constant"
    );
    assert_eq!(
        runtime_code
            .matches(
                "pub(crate) use o3_runtime_pending_address_set::O3_PENDING_DATA_ADDRESS_CAPACITY;"
            )
            .count(),
        1,
        "o3_runtime.rs must re-export the pending-address capacity exactly once"
    );
    assert_eq!(
        pending_set_code
            .matches("rows: Vec<O3PendingDataAddress>,")
            .count(),
        1,
        "pending-address set owner must hold one ordered row vector"
    );
    assert_eq!(
        runtime_code
            .matches("pending_data_addresses: O3PendingDataAddresses,")
            .count(),
        1,
        "runtime state must hold exactly one pending-address collection"
    );
    assert_eq!(
        production_code
            .matches("pending_data_addresses: O3PendingDataAddresses,")
            .count(),
        1,
        "production source must contain exactly one pending-address collection field"
    );
    assert!(
        !production_code.contains("Option<O3PendingDataAddress>")
            && !production_code.contains("pending_data_address_2"),
        "production source must not retain singleton or parallel pending-address fields"
    );
    for (source_name, source) in [
        ("o3_runtime.rs", &runtime_code),
        ("riscv_live_retire_window.rs", &retire_code),
        ("lib.rs", &lib_code),
    ] {
        assert!(
            !source.contains("struct O3PendingDataAddress"),
            "{source_name} must not define the pending-address struct"
        );
        assert!(
            !source.contains("BTreeMap<MemoryRequestId, O3PendingDataAddress")
                && !source.contains("BTreeMap<u64, O3PendingDataAddress"),
            "{source_name} must not grow a pending-address map"
        );
    }

    assert!(production_defines_exact_function(
        &pending_staging_code,
        "stage_pending_data_address_window"
    ));
    let staging_definition =
        rust_function_definition(&pending_staging_code, "stage_pending_data_address_window")
            .expect("pending-address staging owner must define stage_pending_data_address_window");
    for anchor in [
        "pending: impl IntoIterator<Item = O3PendingDataAddressRequest>",
        "let pending = pending.into_iter().collect::<Vec<_>>();",
        "let mut staged = self.clone();",
        "staged.try_stage_pending_data_address_window(",
        "*self = staged;",
    ] {
        assert!(
            staging_definition.contains(anchor),
            "pending-address staging is missing transactional anchor `{anchor}`"
        );
    }
    assert!(production_defines_exact_function(
        &pending_staging_code,
        "try_stage_pending_data_address_window"
    ));
    assert!(!pending_staging_code.contains("discard_live_staged_window_from"));
    for source in [&pending_code, &pending_set_code, &runtime_code] {
        assert!(!production_defines_exact_function(
            source,
            "stage_pending_data_address_window"
        ));
    }

    for helper in [
        "pending_data_address_count",
        "has_pending_data_address",
        "pending_data_address_owns_fetch",
        "oldest_pending_data_address_execution",
        "pending_data_address_execution_for_fetch",
        "pending_data_address_execution_for_fetch_mut",
        "pending_data_address_decoded",
        "pending_data_address_issue_matches",
        "discard_pending_data_address_at_internal",
        "discard_pending_data_address_from",
        "discard_pending_data_address_for_fetch",
        "discard_pending_data_address",
        "discard_pending_data_address_at",
        "bind_pending_data_address_issue",
        "pending_data_address_owner_is_consistent",
    ] {
        assert!(
            production_defines_exact_function(&pending_set_code, helper),
            "pending-address set owner is missing `{helper}`"
        );
        for source in [&pending_code, &pending_staging_code, &runtime_code] {
            assert!(
                !production_defines_exact_function(source, helper),
                "pending-address helper `{helper}` escaped its set owner"
            );
        }
    }
    for helper in [
        "len",
        "is_empty",
        "first",
        "last",
        "iter",
        "find_sequence",
        "find_sequence_mut",
        "find_fetch",
        "find_primary_fetch",
        "find_primary_fetch_mut",
        "try_push",
        "take_fetch",
        "take_from",
    ] {
        assert!(
            production_defines_exact_function(&pending_set_code, helper),
            "pending-address set owner is missing bounded collection API `{helper}`"
        );
    }
    let count_definition =
        rust_function_definition(&pending_set_code, "pending_data_address_count").unwrap();
    assert!(count_definition.contains("self.pending_data_addresses.len()"));
    let retirement_count =
        rust_function_definition(&memory_code, "pending_live_data_access_retirement_count")
            .unwrap();
    assert!(retirement_count.contains("+ self.pending_data_address_count()"));
    assert!(!retirement_count.contains("usize::from(self.has_pending_data_address())"));
    let discard_definition = rust_function_definition(
        &pending_set_code,
        "discard_pending_data_address_at_internal",
    )
    .unwrap();
    let remove_position = discard_definition
        .find("self.pending_data_addresses.take_from(sequence)")
        .expect("boundary discard must take all matching collection rows");
    let cleanup_position = discard_definition
        .find("discard_live_staged_window_from")
        .expect("boundary discard must clean the staged suffix");
    assert!(remove_position < cleanup_position);
    assert!(issue_pending_code.contains("pending.fetch_predecessor_request"));
    assert!(!production_code.contains("producer_fetch"));
    for helper in [
        "issue_matches",
        "record_pending_data_address_materialization",
    ] {
        assert!(
            production_defines_exact_function(&pending_code, helper),
            "pending-address row owner is missing `{helper}`"
        );
        for source in [&pending_set_code, &pending_staging_code, &runtime_code] {
            assert!(!production_defines_exact_function(source, helper));
        }
    }
    let materialization =
        rust_function_definition(&pending_code, "record_pending_data_address_materialization")
            .unwrap();
    for anchor in [
        "let sequence = candidate.sequence();",
        "find_sequence(sequence)",
        "pending.consumed_requests != consumed_requests",
        "find_sequence_mut(sequence)",
    ] {
        assert!(
            materialization.contains(anchor),
            "pending-address materialization is missing exact-row anchor `{anchor}`"
        );
    }
    assert!(
        !materialization.contains("pending_data_addresses.first()")
            && !materialization.contains("pending_data_addresses.first_mut()")
    );
    let pending_raw_code = rust_code_without_comments_and_literals(&pending);
    let pending_set_raw_code = rust_code_without_comments_and_literals(&pending_set);
    for helper in [
        "pending_data_address_sequence_for_test",
        "pending_data_address_owner_count_for_test",
        "pending_data_address_selected_issue_tick_for_test",
        "pending_data_address_materialized_execution_for_test",
    ] {
        assert_eq!(rust_function_definition_count(&pending_raw_code, helper), 1);
        assert_eq!(
            rust_function_definition_count(&pending_set_raw_code, helper),
            0
        );
    }
    for helper in [
        "corrupt_pending_data_address_lsq_bytes_for_test",
        "corrupt_pending_data_address_producer_sequence_for_test",
    ] {
        assert_eq!(rust_function_definition_count(&pending_raw_code, helper), 1);
        assert_eq!(
            rust_function_definition_count(&pending_set_raw_code, helper),
            0
        );
    }

    for helper in [
        "pending_data_address_candidate_metadata",
        "pending_data_address_producer_ready_tick",
        "pending_data_address_committed_producer_ready_tick",
        "pending_data_address_sequence_for_replay",
        "pending_data_address_has_producer_sequence",
        "pending_data_address_wake_seed",
        "pending_data_address_wake_tick",
        "record_pending_data_address_resource_blocked",
    ] {
        assert!(
            production_defines_exact_function(&issue_pending_code, helper),
            "pending-address scheduler owner is missing `{helper}`"
        );
        assert!(
            !production_defines_exact_function(&issue_root_code, helper),
            "issue root must not retain pending-address scheduler helper `{helper}`"
        );
    }
    assert!(
        !production_code.contains("pending_data_address_selected_issue_tick_for_reservation"),
        "the live issue calendar must capture selected pending ticks without the obsolete adapter"
    );
    let calendar_capture = rust_function_definition(&issue_calendar_code, "capture").unwrap();
    for anchor in [
        ".pending_data_addresses",
        ".filter_map(|pending| pending.selected_issue_tick)",
        "calendar.reserve(tick, O3IssueOpClass::Memory);",
    ] {
        assert!(
            calendar_capture.contains(anchor),
            "live issue calendar capture is missing pending reservation anchor `{anchor}`"
        );
    }
    assert!(!production_code.contains("pending_data_address_younger_materialization_matches"));
    for anchor in [
        "pub(crate) struct O3PendingDataAddressWakeSeed {",
        "fetch_predecessor_request: MemoryRequestId,",
        "head_reservation: O3LiveIssueHeadReservation,",
        "younger_pcs: Vec<Address>,",
        "pub(crate) const fn fetch_predecessor_request(&self) -> MemoryRequestId",
        "pub(crate) const fn head_reservation(&self) -> O3LiveIssueHeadReservation",
        "pub(crate) fn younger_pcs(&self) -> &[Address]",
        "head_reservation: O3LiveIssueHeadReservation::memory(pending.producer_sequence, 0)",
    ] {
        assert!(
            issue_pending_code.contains(anchor),
            "pending-address typed wake owner is missing `{anchor}`"
        );
    }
    let candidate_metadata = rust_function_definition(
        &issue_pending_code,
        "pending_data_address_candidate_metadata",
    )
    .unwrap();
    assert!(candidate_metadata.contains("find_sequence(sequence)"));
    let wake_tick =
        rust_function_definition(&issue_pending_code, "pending_data_address_wake_tick").unwrap();
    assert!(wake_tick.contains("pending.materialized.is_none()") && wake_tick.contains(".min()"));
    let blocked_wake = rust_function_definition(
        &issue_pending_code,
        "record_pending_data_address_resource_blocked",
    )
    .unwrap();
    for anchor in [
        "find_sequence(sequence)",
        "for mut pending in self.pending_data_addresses.iter().cloned()",
        "pending.sequence == sequence",
        "self.pending_data_addresses = updated;",
    ] {
        assert!(blocked_wake.contains(anchor));
    }
    let prepare_batch =
        rust_function_definition(&issue_root_code, "prepare_live_issue_batch").unwrap();
    for anchor in [
        "queue.entry(issued.sequence())",
        "!entry.scheduling().is_pending_data_address()",
        "entry.sequence()",
        "entry.packet()",
    ] {
        assert!(prepare_batch.contains(anchor));
    }
    for anchor in [
        "pending_data_address_sequence_for_replay(sequence)",
        "O3LiveIssueQueueCapture::ReplayPending(sequence)",
        "discard_pending_data_address_from(sequence)",
    ] {
        assert!(
            issue_root_code.contains(anchor),
            "pending replay owner is missing `{anchor}`"
        );
    }
    assert!(!production_defines_exact_function(
        &issue_root_code,
        "live_issue_capacities_after_reservations"
    ));
    let capacities = rust_function_definition(
        &issue_calendar_code,
        "live_issue_capacities_after_reservations",
    )
    .unwrap();
    assert!(capacities.contains("memory_issue_width.saturating_sub(reservations.memory)"));
    let wake_window =
        rust_function_definition(&retire_code, "wake_o3_data_access_younger_window").unwrap();
    for anchor in [
        "pending_data_address_wake_seed()",
        "seed.fetch_predecessor_request()",
        "seed.younger_pcs().to_vec()",
        "seed.head_reservation()",
    ] {
        assert!(wake_window.contains(anchor));
    }
    for helper in [
        "requested_o3_writeback_wake_tick",
        "refresh_o3_writeback_wake",
    ] {
        let definition = rust_function_definition(&writeback_wake_code, helper).unwrap();
        assert!(
            definition.contains("pending_data_address_wake_tick()"),
            "writeback wake helper `{helper}` must include the minimum pending-row wake"
        );
    }
    assert!(
        production_defines_exact_function(
            &stage_child_code,
            "stage_dependent_result_address_window"
        ),
        "retire-window staging child is missing stage_dependent_result_address_window"
    );
    assert!(
        !production_defines_exact_function(&retire_code, "stage_dependent_result_address_window"),
        "src/riscv_live_retire_window.rs must not own dependent-address staging"
    );
    let stage_child_definition =
        rust_function_definition(&stage_child_code, "stage_dependent_result_address_window")
            .unwrap();
    assert!(
        stage_child.contains(
            "o3_runtime::{O3PendingDataAddressRequest, O3_PENDING_DATA_ADDRESS_CAPACITY}"
        ),
        "live-retire staging must import pending-address capacity through crate::o3_runtime"
    );
    assert!(
        !stage_child.contains(
            "o3_runtime_pending_address_set::O3_PENDING_DATA_ADDRESS_CAPACITY"
        ),
        "live-retire staging must not import pending-address capacity from the private child module"
    );
    for anchor in [
        "let mut predecessor = head_completed.last_consumed_request();",
        "let pending_capacity = O3_PENDING_DATA_ADDRESS_CAPACITY;",
        "let scheduled_capacity = state",
        ".scalar_memory_window_limit()",
        ".saturating_sub(1);",
        "let mut requests = Vec::with_capacity(pending_capacity);",
        "for _ in 0..pending_capacity {",
        "let mut scheduled = Vec::with_capacity(scheduled_capacity);",
        "let mut result_destinations = Vec::with_capacity(pending_capacity + 1);",
        "dependent_authorization(state, dependent.first_consumed_request())",
        "O3PendingDataAddressRequest::new(",
        "predecessor,",
        "dependent_requests.push(dependent.first_consumed_request());",
        "predecessor = dependent.last_consumed_request();",
        "requests,",
        "state.memory_result_window_authorizations.remove(&request);",
    ] {
        assert!(
            stage_child_definition.contains(anchor),
            "live-retire staging is missing split-fetch batch anchor `{anchor}`"
        );
    }
    assert!(!stage_child_definition.contains(".values()"));
    assert!(!stage_child_definition.contains("authorized_rows"));
    let schedule_position = stage_child_definition
        .find("let schedule_result")
        .expect("live-retire staging must schedule the complete batch");
    let removal_position = stage_child_definition
        .find("for request in dependent_requests")
        .expect("live-retire staging must remove every consumed authorization");
    assert!(schedule_position < removal_position);

    let expected_tests = [
        "pending_address_stages_addressless_lsq_and_live_rename_once",
        "pending_address_window_stages_two_scalar_suffix_rows",
        "pending_address_rejects_a_second_owner",
        "pending_address_window_stays_four_rows_at_scalar_live_depth_eight",
        "pending_address_discard_restores_prior_rename_and_removes_lsq",
    ];
    let staging_test_code = rust_code_without_comments_and_literals(&staging_test);
    assert_eq!(
        staging_test_code.matches("#[test]").count(),
        expected_tests.len(),
        "pending-address staging tests must expose exactly the focused inventory"
    );
    for anchor in expected_tests {
        assert_eq!(
            rust_function_definition_count(&staging_test_code, anchor),
            1,
            "missing or duplicated pending-address staging test `{anchor}`"
        );
    }

    let expected_scheduling_tests = [
        "pending_address_scheduler_waits_for_head_writeback",
        "pending_address_scheduler_width_one_orders_memory_before_scalar",
        "pending_address_scheduler_width_two_coissues_memory_and_scalar",
        "pending_address_materialization_uses_admitted_producer_value",
        "pending_address_materialization_stale_identity_replays_and_discards_suffix",
        "pending_address_materialization_failure_replays_without_callback_error",
        "pending_address_materialization_does_not_allocate_a_request",
    ];
    let scheduling_test_code = rust_code_without_comments_and_literals(&scheduling_test);
    assert_eq!(
        scheduling_test_code.matches("#[test]").count(),
        expected_scheduling_tests.len(),
        "pending-address scheduling tests must expose exactly the focused inventory"
    );
    for anchor in expected_scheduling_tests {
        assert_eq!(
            rust_function_definition_count(&scheduling_test_code, anchor),
            1,
            "missing or duplicated pending-address scheduling test `{anchor}`"
        );
    }

    let expected_lifecycle_tests = [
        "head_retry_discards_pending_address_and_suffix",
        "head_failure_discards_pending_address_and_suffix",
        "redirect_discards_pending_address_and_future_wake",
        "interrupt_discards_pending_address_and_suffix",
        "restart_discards_pending_address_and_suffix",
        "reset_and_restore_clear_pending_address_state",
        "detailed_mode_disable_discards_pending_address_state",
        "pending_address_keeps_live_data_handoff_nonquiescent",
        "pending_address_rejects_live_checkpoint_capture",
        "drained_pending_address_restores_checkpoint_compatibility",
    ];
    let lifecycle_test_code = rust_code_without_comments_and_literals(&lifecycle_test);
    assert_eq!(
        lifecycle_test_code.matches("#[test]").count(),
        expected_lifecycle_tests.len(),
        "pending-address lifecycle tests must expose exactly the focused inventory"
    );
    for anchor in expected_lifecycle_tests {
        assert_eq!(
            rust_function_definition_count(&lifecycle_test_code, anchor),
            1,
            "missing or duplicated pending-address lifecycle test `{anchor}`"
        );
    }

    let expected_multiple_tests = [
        "pending_address_collection_orders_by_sequence_and_rejects_fourth",
        "two_pending_sibling_stages_two_addressless_lsq_rows_and_one_suffix",
        "two_pending_chain_stages_second_with_first_as_immediate_producer",
        "two_pending_staging_failure_rolls_back_both_rows_and_rename",
        "two_pending_discard_from_second_preserves_first_row",
        "two_pending_retirement_accounting_counts_zero_one_two_rows",
        "two_pending_staging_removes_both_authorizations_only_after_schedule",
        "two_pending_siblings_width_one_issue_oldest_across_ticks",
        "two_pending_siblings_width_two_keep_one_memory_slot_and_coissue_scalar",
        "two_pending_chain_initial_schedule_waits_on_first_sequence",
        "two_pending_typed_wake_seed_separates_second_fetch_predecessor",
        "two_pending_resource_wake_updates_only_blocked_row",
        "two_pending_first_materialization_replay_discards_complete_chain",
        "two_pending_second_materialization_replay_preserves_older_row",
        "two_pending_chain_wakes_second_after_first_admitted_writeback",
        "two_pending_interrupt_reset_restore_and_mode_cleanup_remove_all_rows",
        "two_pending_live_checkpoint_and_handoff_reject_two_rows",
    ];
    let multiple_test_code = rust_code_without_comments_and_literals(&multiple_test);
    assert!(multiple_test_code.contains("use super::*;"));
    assert_eq!(
        multiple_test_code.matches("#[test]").count(),
        expected_multiple_tests.len(),
        "pending-address multiple-row tests must expose exactly the Task 3 inventory"
    );
    for anchor in expected_multiple_tests {
        assert_eq!(
            rust_function_definition_count(&multiple_test_code, anchor),
            1,
            "missing or duplicated pending-address multiple-row test `{anchor}`"
        );
    }

    let expected_three_pending_tests = [
        "three_pending_staging_allocates_three_addressless_lsq_rows",
        "three_pending_sibling_width_one_issues_in_sequence",
        "three_pending_sibling_width_two_issues_two_then_one",
        "three_pending_sibling_width_four_issues_all_three_together",
        "three_pending_chain_waits_for_each_admitted_writeback",
        "three_pending_mixed_fanout_coissues_two_and_blocks_third",
        "three_pending_resource_wake_updates_only_the_blocked_suffix",
        "three_pending_replay_from_middle_preserves_older_and_discards_younger",
        "three_pending_interrupt_reset_htm_and_mode_cleanup_remove_all_rows",
        "three_pending_live_checkpoint_and_addressless_handoff_reject",
    ];
    let three_pending_test_code = rust_code_without_comments_and_literals(&three_pending_test);
    assert!(three_pending_test_code.contains("use super::*;"));
    assert_eq!(
        three_pending_test_code.matches("#[test]").count(),
        expected_three_pending_tests.len(),
        "pending-address three-pending tests must expose exactly the Task 3 inventory"
    );
    for anchor in expected_three_pending_tests {
        assert_eq!(
            rust_function_definition_count(&three_pending_test_code, anchor),
            1,
            "missing or duplicated pending-address three-pending test `{anchor}`"
        );
    }
}

#[test]
fn task5_dependent_result_address_data_issue_stays_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = crate_dir.join("src");
    let translation_root_path = crate_dir.join("src/riscv_translation.rs");
    let unissued_path = crate_dir.join("src/riscv_translation/unissued_data.rs");
    let issue_root_path = crate_dir.join("src/riscv_data_issue.rs");
    let issue_child_path = crate_dir.join("src/riscv_data_issue/dependent_result_address.rs");
    let prepared_path = crate_dir.join("src/riscv_data_issue/prepared.rs");
    let issue_test_root_path = crate_dir.join("src/riscv_data_issue_tests.rs");
    let issue_test_path = crate_dir.join("src/riscv_data_issue_tests/dependent_result_address.rs");
    let issue_multiple_test_path =
        crate_dir.join("src/riscv_data_issue_tests/dependent_result_address_multiple.rs");
    let issue_three_pending_test_path =
        crate_dir.join("src/riscv_data_issue_tests/dependent_result_address_three_pending.rs");
    let memory_path = crate_dir.join("src/o3_runtime_memory.rs");
    let pending_path = crate_dir.join("src/o3_runtime_pending_address.rs");
    let pending_set_path = crate_dir.join("src/o3_runtime_pending_address_set.rs");
    let lib_path = crate_dir.join("src/lib.rs");
    for path in [
        &translation_root_path,
        &unissued_path,
        &issue_root_path,
        &issue_child_path,
        &prepared_path,
        &issue_test_root_path,
        &issue_test_path,
        &issue_multiple_test_path,
        &issue_three_pending_test_path,
        &memory_path,
        &pending_path,
        &pending_set_path,
        &lib_path,
    ] {
        assert!(path.is_file(), "missing Task 5 file {}", path.display());
    }

    let translation_root = fs::read_to_string(&translation_root_path).unwrap();
    let unissued = fs::read_to_string(&unissued_path).unwrap();
    let issue_root = fs::read_to_string(&issue_root_path).unwrap();
    let issue_child = fs::read_to_string(&issue_child_path).unwrap();
    let prepared = fs::read_to_string(&prepared_path).unwrap();
    let issue_test_root = fs::read_to_string(&issue_test_root_path).unwrap();
    let issue_test = fs::read_to_string(&issue_test_path).unwrap();
    let issue_multiple_test = fs::read_to_string(&issue_multiple_test_path).unwrap();
    let issue_three_pending_test = fs::read_to_string(&issue_three_pending_test_path).unwrap();
    let memory = fs::read_to_string(&memory_path).unwrap();
    let pending = fs::read_to_string(&pending_path).unwrap();
    let pending_set = fs::read_to_string(&pending_set_path).unwrap();
    let lib = fs::read_to_string(&lib_path).unwrap();

    assert_eq!(
        rust_code_without_comments_and_literals(&translation_root)
            .matches("mod unissued_data;")
            .count(),
        1
    );
    assert_eq!(
        rust_code_without_comments_and_literals(&issue_root)
            .matches("mod dependent_result_address;")
            .count(),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_test_root,
            "riscv_data_issue_tests/dependent_result_address.rs",
            "dependent_result_address",
        ),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_test_root,
            "riscv_data_issue_tests/dependent_result_address_multiple.rs",
            "dependent_result_address_multiple",
        ),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_test_root,
            "riscv_data_issue_tests/dependent_result_address_three_pending.rs",
            "dependent_result_address_three_pending",
        ),
        1
    );
    assert!(include_macro_lines(&unissued).is_empty());
    assert!(include_macro_lines(&issue_child).is_empty());
    assert!(include_macro_lines(&issue_test).is_empty());
    assert!(include_macro_lines(&issue_multiple_test).is_empty());
    assert!(include_macro_lines(&issue_three_pending_test).is_empty());
    assert!(
        external_module_declaration_lines(&unissued).is_empty()
            && path_attribute_lines(&unissued).is_empty()
            && external_module_declaration_lines(&issue_child).is_empty()
            && path_attribute_lines(&issue_child).is_empty()
            && external_module_declaration_lines(&issue_test).is_empty()
            && path_attribute_lines(&issue_test).is_empty()
            && external_module_declaration_lines(&issue_multiple_test).is_empty()
            && path_attribute_lines(&issue_multiple_test).is_empty()
            && external_module_declaration_lines(&issue_three_pending_test).is_empty()
            && path_attribute_lines(&issue_three_pending_test).is_empty()
    );
    assert!(line_count(&unissued_path) <= MAX_RISCV_UNISSUED_DATA_LINES);
    assert!(line_count(&issue_child_path) <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_ISSUE_LINES);
    assert!(line_count(&issue_test_path) <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_ISSUE_TEST_LINES);
    assert!(
        line_count(&issue_multiple_test_path)
            <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_MULTIPLE_ISSUE_TEST_LINES
    );
    assert!(
        line_count(&issue_three_pending_test_path)
            <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_THREE_PENDING_ISSUE_TEST_LINES
    );

    let translation_code = production_rust_source(&translation_root);
    let unissued_code = production_rust_source(&unissued);
    let issue_root_code = production_rust_source(&issue_root);
    let issue_child_code = production_rust_source(&issue_child);
    let prepared_code = production_rust_source(&prepared);
    let memory_code = production_rust_source(&memory);
    let pending_code = production_rust_source(&pending);
    let pending_set_code = production_rust_source(&pending_set);
    let lib_code = production_rust_source(&lib);
    assert!(production_defines_exact_function(
        &unissued_code,
        "next_unissued_data_access"
    ));
    assert!(!production_defines_exact_function(
        &translation_code,
        "next_unissued_data_access"
    ));
    for helper in [
        "bind_pending_address_issue",
        "validate_pending_address_pre_submit",
        "replay_pending_address_before_submit",
    ] {
        assert!(production_defines_exact_function(&issue_child_code, helper));
        assert!(!production_defines_exact_function(&issue_root_code, helper));
        assert!(!production_defines_exact_function(&lib_code, helper));
    }
    assert!(production_defines_exact_function(
        &pending_set_code,
        "bind_pending_data_address_issue"
    ));
    for helper in [
        "oldest_pending_data_address_execution",
        "pending_data_address_execution_for_fetch",
        "pending_data_address_execution_for_fetch_mut",
        "discard_pending_data_address_for_fetch",
    ] {
        assert!(production_defines_exact_function(&pending_set_code, helper));
    }
    assert!(!production_defines_exact_function(
        &pending_code,
        "bind_pending_data_address_issue"
    ));
    assert!(!production_defines_exact_function(
        &issue_root_code,
        "bind_pending_data_address_issue"
    ));
    assert!(!production_defines_exact_function(
        &lib_code,
        "bind_pending_data_address_issue"
    ));
    let bind_token = "bind_pending_data_address_issue";
    let bind_occurrences = rust_source_files(&source_root)
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            if is_test_only_rust_source(&relative) {
                return None;
            }
            let source = fs::read_to_string(path).unwrap();
            let production = production_rust_source(&source);
            let chars = production.chars().collect::<Vec<_>>();
            let count = rust_identifier_count(&chars, bind_token);
            (count != 0).then_some((relative, count))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bind_occurrences,
        vec![
            (PathBuf::from("src/o3_runtime_pending_address_set.rs"), 1),
            (
                PathBuf::from("src/riscv_data_issue/dependent_result_address.rs"),
                1,
            ),
        ]
    );
    let bind_call = ".bind_pending_data_address_issue(";
    let bind_callers = rust_source_files(&source_root)
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            if is_test_only_rust_source(&relative) {
                return None;
            }
            let source = fs::read_to_string(path).unwrap();
            let count = production_rust_source(&source).matches(bind_call).count();
            (count != 0).then_some((relative, count))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bind_callers,
        vec![(
            PathBuf::from("src/riscv_data_issue/dependent_result_address.rs"),
            1,
        )]
    );
    let issue_recording = rust_function_definition(&issue_root_code, "try_record_data_issue_state")
        .expect("data-issue recording root");
    assert_eq!(
        issue_recording
            .matches("Self::bind_pending_address_issue(")
            .count(),
        1
    );
    for source in [&translation_code, &issue_root_code, &lib_code] {
        assert!(!source.contains("struct O3PendingDataAddress"));
    }
    let unissued_definition = rust_function_definition(&unissued_code, "next_unissued_data_access")
        .expect("unissued selector owner");
    assert!(unissued_definition.contains("oldest_pending_data_address_execution()"));
    let replay_definition =
        rust_function_definition(&issue_child_code, "replay_pending_address_before_submit")
            .expect("pending replay owner");
    assert!(replay_definition.contains("abort_prepared_data_issue(fetch_request, now)"));
    assert!(!replay_definition.contains("discard_pending_data_address_for_fetch(fetch_request)"));
    assert!(!replay_definition.contains("discard_pending_data_address();"));
    let abort_definition = rust_function_definition(&prepared_code, "abort_prepared_data_issue")
        .expect("prepared abort owner");
    assert!(abort_definition.contains("discard_pending_data_address_for_fetch(fetch_request)"));
    assert!(abort_definition.contains("o3_writeback_wake.clear()"));
    assert!(abort_definition.contains("refresh_o3_writeback_wake(now)"));
    let can_issue_definition =
        rust_function_definition(&memory_code, "pending_data_address_can_issue")
            .expect("pending issue lookup owner");
    assert!(
        can_issue_definition.contains("pending_data_address_execution_for_fetch(fetch_request)")
    );
    assert!(lib_code.contains("pending_data_address_execution_for_fetch(fetch_request)"));
    assert!(lib_code.contains("pending_data_address_execution_for_fetch_mut(fetch_request)"));

    let expected_tests = [
        "dependent_result_address_pre_submit_validation_binds_serial_request",
        "dependent_result_address_parallel_transaction_binds_after_submit",
        "dependent_result_address_forwarded_load_binds_without_transport",
        "dependent_result_address_pmp_denial_replays_without_request",
        "dependent_result_address_pma_uncacheable_replays_without_request",
        "dependent_result_address_cross_line_replays_without_request",
        "dependent_result_address_mmio_route_replays_without_mmio_request",
        "dependent_result_address_unknown_memory_route_replays_without_request",
        "dependent_result_address_atomic_overlap_replays_without_request",
        "dependent_result_address_dropped_parallel_prepare_discards_pending_suffix",
        "dependent_result_address_submit_failure_discards_pending_suffix",
        "pending_address_bind_reuses_sequence_and_resolves_lsq_address",
        "pending_address_bind_publishes_one_execution_event",
    ];
    let issue_test_code = rust_code_without_comments_and_literals(&issue_test);
    assert_eq!(
        issue_test_code.matches("#[test]").count(),
        expected_tests.len()
    );
    for test in expected_tests {
        assert_eq!(rust_function_definition_count(&issue_test_code, test), 1);
    }
    let expected_multiple_tests = [
        "two_pending_unissued_selector_returns_oldest_materialized_first",
        "two_pending_data_access_execution_looks_up_exact_pending_fetch",
        "two_pending_bind_first_removes_exact_row_and_keeps_second_pending",
        "two_pending_bind_second_preserves_first_live_access",
        "two_pending_first_pre_submit_replay_discards_second_and_suffix",
        "two_pending_dropped_parallel_prepare_clears_scheduled_pending_wake",
        "two_pending_second_pre_submit_replay_preserves_first_live_access",
        "two_pending_atomic_chain_second_overlap_uses_root_head_range",
        "two_pending_second_pma_and_cross_line_replay_preserve_first_live_access",
    ];
    let issue_multiple_test_code = rust_code_without_comments_and_literals(&issue_multiple_test);
    assert_eq!(
        issue_multiple_test_code.matches("#[test]").count(),
        expected_multiple_tests.len()
    );
    for test in expected_multiple_tests {
        assert_eq!(
            rust_function_definition_count(&issue_multiple_test_code, test),
            1
        );
    }
    let expected_three_pending_tests = [
        "three_pending_unissued_selector_returns_oldest_materialized_first",
        "three_pending_bind_removes_exact_rows_in_sequence",
        "selected_three_pending_addresses_remain_driveable_between_submissions",
        "three_pending_first_replay_discards_complete_suffix",
        "three_pending_middle_replay_preserves_first_live_access",
        "three_pending_last_replay_preserves_two_older_live_accesses",
        "three_pending_atomic_root_range_applies_to_every_descendant",
    ];
    let issue_three_pending_test_code =
        rust_code_without_comments_and_literals(&issue_three_pending_test);
    assert_eq!(
        issue_three_pending_test_code.matches("#[test]").count(),
        expected_three_pending_tests.len()
    );
    for test in expected_three_pending_tests {
        assert_eq!(
            rust_function_definition_count(&issue_three_pending_test_code, test),
            1
        );
    }
}

#[test]
fn task8_dependent_result_address_production_ownership_is_final() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = crate_dir.join("src");
    let runtime_owner_path = Path::new("src/o3_runtime.rs");
    let issue_root_path = Path::new("src/o3_runtime_issue.rs");
    let calendar_owner_path = Path::new("src/o3_runtime_issue/calendar.rs");
    let pending_owner_path = Path::new("src/o3_runtime_pending_address.rs");
    let pending_set_owner_path = Path::new("src/o3_runtime_pending_address_set.rs");
    let pending_staging_owner_path = Path::new("src/o3_runtime_pending_address_staging.rs");
    let pending_issue_owner_path = Path::new("src/o3_runtime_issue/pending_address.rs");
    let authorization_owner_path =
        Path::new("src/riscv_fetch_ahead/memory_result_authorization.rs");
    let issue_owner_path = Path::new("src/riscv_data_issue/dependent_result_address.rs");
    let production = rust_source_files(&source_root)
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            (!is_test_only_rust_source(&relative)).then(|| {
                let source = fs::read_to_string(path).unwrap();
                (relative, production_rust_source(&source))
            })
        })
        .collect::<Vec<_>>();
    let source = |path: &Path| {
        production
            .iter()
            .find_map(|(candidate, source)| (candidate == path).then_some(source.as_str()))
            .unwrap_or_else(|| panic!("missing production source {}", path.display()))
    };

    let runtime_module_source =
        fs::read_to_string(crate_dir.join(runtime_owner_path)).expect("runtime root source");
    let issue_root_module_source =
        fs::read_to_string(crate_dir.join(issue_root_path)).expect("issue root source");
    let runtime_owner = source(runtime_owner_path);
    let calendar_owner = source(calendar_owner_path);
    let pending_owner = source(pending_owner_path);
    let pending_set_owner = source(pending_set_owner_path);
    let pending_staging_owner = source(pending_staging_owner_path);
    let pending_issue_owner = source(pending_issue_owner_path);
    let authorization_owner = source(authorization_owner_path);
    let issue_owner = source(issue_owner_path);
    let production_code = production
        .iter()
        .map(|(_, source)| source.as_str())
        .collect::<String>();

    for (path, module) in [
        (
            "o3_runtime_pending_address.rs",
            "o3_runtime_pending_address",
        ),
        (
            "o3_runtime_pending_address_set.rs",
            "o3_runtime_pending_address_set",
        ),
        (
            "o3_runtime_pending_address_staging.rs",
            "o3_runtime_pending_address_staging",
        ),
    ] {
        assert_eq!(
            path_owned_module_declaration_count(&runtime_module_source, path, module),
            1,
            "runtime root must attach focused owner `{module}` exactly once"
        );
    }
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_root_module_source,
            "o3_runtime_issue/pending_address.rs",
            "pending_address"
        ),
        1,
        "issue root must attach the pending-address scheduler owner exactly once"
    );

    let pending_struct_owners = production
        .iter()
        .flat_map(|(path, source)| {
            production_struct_definitions(source)
                .into_iter()
                .filter(|definition| {
                    production_defines_exact_named_item(
                        definition,
                        "struct",
                        "O3PendingDataAddress",
                    )
                })
                .map(|_| path.display().to_string())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pending_struct_owners,
        [pending_owner_path.display().to_string()],
        "O3PendingDataAddress must have exactly one focused production owner"
    );
    let pending_collection_struct_owners = production
        .iter()
        .flat_map(|(path, source)| {
            production_struct_definitions(source)
                .into_iter()
                .filter(|definition| {
                    production_defines_exact_named_item(
                        definition,
                        "struct",
                        "O3PendingDataAddresses",
                    )
                })
                .map(|_| path.display().to_string())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pending_collection_struct_owners,
        [pending_set_owner_path.display().to_string()],
        "O3PendingDataAddresses must have exactly one focused production owner"
    );
    let pending_wake_seed_owners = production
        .iter()
        .flat_map(|(path, source)| {
            production_struct_definitions(source)
                .into_iter()
                .filter(|definition| {
                    production_defines_exact_named_item(
                        definition,
                        "struct",
                        "O3PendingDataAddressWakeSeed",
                    )
                })
                .map(|_| path.display().to_string())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pending_wake_seed_owners,
        [pending_issue_owner_path.display().to_string()],
        "O3PendingDataAddressWakeSeed must stay in the focused scheduler owner"
    );
    let mut alternate_pending_storage = Vec::new();
    for (path, source) in &production {
        let mut definitions = production_struct_definitions(source);
        definitions.extend(production_enum_definitions(source));
        definitions.extend(production_type_alias_definitions(source));
        for definition in definitions {
            let storage_count =
                production_definition_named_type_storage_count(&definition, "O3PendingDataAddress");
            if storage_count == 0 {
                continue;
            }
            let exact_collection_owner = path == pending_set_owner_path
                && production_defines_exact_named_item(
                    &definition,
                    "struct",
                    "O3PendingDataAddresses",
                )
                && storage_count == 1;
            if !exact_collection_owner {
                alternate_pending_storage.push(format!(
                    "{} ({storage_count} exact row references)",
                    path.display()
                ));
            }
        }
    }
    assert!(
        alternate_pending_storage.is_empty(),
        "alternate pending-row aggregate storage is forbidden: {}",
        alternate_pending_storage.join(", ")
    );
    assert_eq!(
        production_struct_named_type_storage(&production, "O3PendingDataAddresses"),
        [(runtime_owner_path.to_path_buf(), 1)],
        "O3PendingDataAddresses struct-field storage must be exactly the canonical runtime field"
    );
    assert_eq!(
        runtime_owner
            .matches("pending_data_addresses: O3PendingDataAddresses,")
            .count(),
        1,
        "runtime owner must contain exactly one pending-address collection field"
    );
    assert_eq!(
        production_code
            .matches("pending_data_addresses: O3PendingDataAddresses,")
            .count(),
        1,
        "production must contain exactly one pending-address collection field"
    );
    let singleton_owners = production
        .iter()
        .filter_map(|(path, source)| {
            source
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
                .contains("Option<O3PendingDataAddress>")
                .then_some(path.display().to_string())
        })
        .collect::<Vec<_>>();
    assert!(
        singleton_owners.is_empty(),
        "optional pending-address singleton ownership is forbidden: {}",
        singleton_owners.join(", ")
    );
    assert!(
        !production_code.contains("pending_data_address_2"),
        "parallel pending-address fields are forbidden"
    );
    assert_eq!(
        pending_set_owner
            .matches("pub(crate) const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 3;")
            .count(),
        1,
        "the pending-address collection must have one exact capacity-three constant"
    );
    assert_eq!(
        production
            .iter()
            .map(|(_, source)| {
                production_named_const_declaration_count(source, "O3_PENDING_DATA_ADDRESS_CAPACITY")
            })
            .sum::<usize>(),
        1,
        "production must declare exactly one O3_PENDING_DATA_ADDRESS_CAPACITY constant"
    );
    assert_eq!(
        runtime_owner
            .matches(
                "pub(crate) use o3_runtime_pending_address_set::O3_PENDING_DATA_ADDRESS_CAPACITY;"
            )
            .count(),
        1,
        "o3_runtime.rs must expose exactly one crate-visible capacity re-export"
    );
    assert_eq!(
        pending_set_owner
            .matches("rows: Vec<O3PendingDataAddress>,")
            .count(),
        1,
        "the pending-address collection must have one ordered row vector"
    );
    let try_push = rust_function_definition(pending_set_owner, "try_push")
        .expect("pending-address set owner must define try_push");
    for anchor in [
        "self.rows.len() >= O3_PENDING_DATA_ADDRESS_CAPACITY",
        "existing.sequence == row.sequence",
        "existing.sequence >= row.sequence",
        "duplicate_fetch",
        "self.rows.push(row)",
    ] {
        assert!(
            try_push.contains(anchor),
            "pending-address insertion is missing bounded invariant `{anchor}`"
        );
    }
    let pending_set_compact = pending_set_owner
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let try_push_compact = try_push
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert_eq!(
        pending_set_compact.matches("self.rows.push(").count(),
        1,
        "pending-address set owner must contain exactly one row push"
    );
    assert_eq!(
        pending_set_compact.matches("self.rows.push(row)").count(),
        1,
        "the sole pending-address insertion must push the guarded row"
    );
    assert_eq!(
        try_push_compact.matches("self.rows.push(row)").count(),
        1,
        "the sole pending-address insertion must remain inside try_push"
    );
    for forbidden in [
        "self.rows.extend(",
        "self.rows.extend_from_slice(",
        "self.rows.append(",
        "self.rows.insert(",
        "self.rows.splice(",
        "self.rows.push_front(",
        "self.rows.push_back(",
        "self.rows=",
        "&mutself.rows",
    ] {
        assert!(
            !pending_set_compact.contains(forbidden),
            "pending-address rows must not have an alternate insertion path `{forbidden}`"
        );
    }
    for forbidden in [
        "rows_mut",
        "as_mut_slice",
        "DerefMut",
        "pub(super) fn rows(",
        "pub(crate) fn rows(",
    ] {
        assert!(
            !pending_set_owner.contains(forbidden),
            "pending-address set owner must not expose unbounded mutable rows via `{forbidden}`"
        );
    }

    let role = production_enum_definition(authorization_owner, "O3MemoryResultWindowRole")
        .expect("missing O3MemoryResultWindowRole authorization enum");
    let address_authority =
        production_enum_definition(authorization_owner, "O3MemoryResultWindowAddressAuthority")
            .expect("missing O3MemoryResultWindowAddressAuthority authorization enum");
    assert!(enum_has_unit_variant(&role, "YoungerDependentRead"));
    assert_eq!(
        role.matches("Dependent").count(),
        1,
        "YoungerDependentRead must be the only dependent result role"
    );
    assert!(!role.contains("Pending"));
    assert!(address_authority.contains("DependentSource {"));
    assert_eq!(
        address_authority.matches("Dependent").count(),
        1,
        "DependentSource must be the only dependent address authority"
    );
    assert!(!address_authority.contains("Pending"));

    let owners_of = |helper: &str| {
        production
            .iter()
            .filter_map(|(path, source)| {
                production_defines_exact_function(source, helper)
                    .then_some(path.display().to_string())
            })
            .collect::<Vec<_>>()
    };

    for helper in [
        "stage_pending_data_address_window",
        "try_stage_pending_data_address_window",
    ] {
        assert_eq!(
            owners_of(helper),
            [pending_staging_owner_path.display().to_string()],
            "pending-address staging helper `{helper}` must stay in its focused owner"
        );
        assert_eq!(
            rust_function_definition_count(pending_staging_owner, helper),
            1
        );
    }

    for helper in [
        "issue_matches",
        "record_pending_data_address_materialization",
    ] {
        assert_eq!(
            owners_of(helper),
            [pending_owner_path.display().to_string()],
            "pending-address row helper `{helper}` must stay in the row owner"
        );
        assert_eq!(rust_function_definition_count(pending_owner, helper), 1);
    }

    for helper in [
        "pending_data_address_count",
        "has_pending_data_address",
        "pending_data_address_owns_fetch",
        "oldest_pending_data_address_execution",
        "pending_data_address_execution_for_fetch",
        "pending_data_address_execution_for_fetch_mut",
        "pending_data_address_decoded",
        "pending_data_address_issue_matches",
        "bind_pending_data_address_issue",
        "discard_pending_data_address_at_internal",
        "discard_pending_data_address_from",
        "discard_pending_data_address_for_fetch",
        "discard_pending_data_address",
        "discard_pending_data_address_at",
        "pending_data_address_owner_is_consistent",
    ] {
        assert_eq!(
            owners_of(helper),
            [pending_set_owner_path.display().to_string()],
            "pending-address collection helper `{helper}` must stay in the set owner"
        );
        assert_eq!(
            rust_function_definition_count(pending_set_owner, helper),
            1,
            "pending-address helper `{helper}` must have exactly one definition"
        );
    }

    for helper in [
        "pending_data_address_candidate_metadata",
        "pending_data_address_producer_ready_tick",
        "pending_data_address_committed_producer_ready_tick",
        "pending_data_address_sequence_for_replay",
        "pending_data_address_has_producer_sequence",
        "pending_data_address_wake_seed",
        "pending_data_address_wake_tick",
        "record_pending_data_address_resource_blocked",
    ] {
        assert_eq!(
            owners_of(helper),
            [pending_issue_owner_path.display().to_string()],
            "pending-address scheduler helper `{helper}` must stay in the issue child"
        );
        assert_eq!(
            rust_function_definition_count(pending_issue_owner, helper),
            1
        );
    }
    assert_eq!(
        owners_of("pending_data_address_selected_issue_tick_for_reservation"),
        Vec::<String>::new(),
        "the obsolete pending reservation adapter must have no production owner"
    );
    let calendar_capture = rust_function_definition(calendar_owner, "capture").unwrap();
    for anchor in [
        ".pending_data_addresses",
        ".filter_map(|pending| pending.selected_issue_tick)",
        "calendar.reserve(tick, O3IssueOpClass::Memory);",
    ] {
        assert!(
            calendar_capture.contains(anchor),
            "final pending reservation ownership is missing calendar anchor `{anchor}`"
        );
    }

    for helper in [
        "validate_pending_address_pre_submit",
        "replay_pending_address_before_submit",
    ] {
        assert_eq!(
            owners_of(helper),
            [issue_owner_path.display().to_string()],
            "pre-submit helper `{helper}` must stay in the focused data-issue owner"
        );
        assert_eq!(
            rust_function_definition_count(issue_owner, helper),
            1,
            "pre-submit helper `{helper}` must have exactly one definition"
        );
    }

    let mut collection_owners = Vec::new();
    let mut compatibility_aliases = Vec::new();
    let mut parallel_authorities = Vec::new();
    let protected_pending_items = [
        "O3PendingDataAddress",
        "O3PendingDataAddresses",
        "O3_PENDING_DATA_ADDRESS_CAPACITY",
    ];
    for (path, source) in &production {
        if ["BTreeMap", "HashMap", "BTreeSet", "HashSet"]
            .into_iter()
            .any(|collection| {
                generic_type_contains_named_type(source, collection, "O3PendingDataAddress")
            })
        {
            collection_owners.push(path.display().to_string());
        }
        for alias in production_renamed_use_aliases(source, &protected_pending_items) {
            compatibility_aliases.push(format!("{} ({alias})", path.display()));
        }
        let mut definitions = production_struct_definitions(source);
        definitions.extend(production_enum_definitions(source));
        definitions.extend(production_type_alias_definitions(source));
        for definition in definitions {
            if definition.starts_with("type ")
                && (definition.contains("PendingDataAddress")
                    || definition.contains("DependentResultAddress"))
            {
                compatibility_aliases.push(path.display().to_string());
            }
            if (definition.contains("PendingDataAddress")
                || definition.contains("DependentResultAddress"))
                && definition.contains("Authorization")
            {
                parallel_authorities.push(path.display().to_string());
            }
        }
    }
    assert!(
        collection_owners.is_empty(),
        "pending-address map/set ownership is forbidden: {}",
        collection_owners.join(", ")
    );
    assert!(
        compatibility_aliases.is_empty(),
        "pending-address compatibility aliases are forbidden: {}",
        compatibility_aliases.join(", ")
    );
    assert!(
        parallel_authorities.is_empty(),
        "parallel pending-address authorization is forbidden: {}",
        parallel_authorities.join(", ")
    );

    assert!(line_count(&crate_dir.join("src/o3_runtime.rs")) < 1200);
    assert!(
        line_count(&crate_dir.join("src/o3_runtime_pending_address.rs"))
            <= MAX_O3_RUNTIME_PENDING_ADDRESS_LINES
    );
    assert!(
        line_count(&crate_dir.join("src/o3_runtime_pending_address_set.rs"))
            <= MAX_O3_RUNTIME_PENDING_ADDRESS_SET_LINES
    );
    assert!(
        line_count(&crate_dir.join("src/o3_runtime_pending_address_staging.rs"))
            <= MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_LINES
    );
    assert!(
        line_count(&crate_dir.join("src/o3_runtime_issue/pending_address.rs"))
            <= MAX_O3_RUNTIME_ISSUE_PENDING_ADDRESS_LINES
    );
    assert!(
        line_count(&crate_dir.join("src/riscv_fetch_ahead/detailed_o3/data_access_result.rs"))
            <= 450
    );
    assert!(line_count(&crate_dir.join("src/riscv_translation.rs")) < 1800);
    assert!(line_count(&crate_dir.join("src/riscv_data_issue.rs")) < 1800);
    assert_eq!(
        line_count(&crate_dir.join("src/riscv_execution_mode_handoff.rs")),
        1800
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
    let data_access_result = fs::read_to_string(
        crate_dir.join("src/riscv_fetch_ahead/detailed_o3/data_access_result.rs"),
    )
    .unwrap();
    let data_access_result_translation = fs::read_to_string(
        crate_dir.join("src/riscv_fetch_ahead/detailed_o3/data_access_result_translation.rs"),
    )
    .unwrap();
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
        (
            &data_access_result,
            "fn data_access_result_fetch_ahead_shape(",
        ),
        (
            &data_access_result_translation,
            "fn data_access_result_head_probe(",
        ),
        (
            &data_access_result_translation,
            "masked_vector_memory_request_span(",
        ),
        (&data_access_result_translation, "fault_only_first: false"),
        (
            &data_access_result_translation,
            "pub(in crate::riscv_fetch_ahead) fn data_access_result_head_physical_probe(",
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
fn riscv_data_access_result_fetch_authority_is_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = crate_dir.join("src");
    let root_owner_path = Path::new("src/riscv_fetch_ahead/detailed_o3.rs");
    let child_owner_path = Path::new("src/riscv_fetch_ahead/detailed_o3/data_access_result.rs");
    let translation_owner_path =
        Path::new("src/riscv_fetch_ahead/detailed_o3/data_access_result_translation.rs");
    let root_path = crate_dir.join(root_owner_path);
    let child_path = crate_dir.join(child_owner_path);
    let translation_path = crate_dir.join(translation_owner_path);
    let pair_policy_path =
        crate_dir.join("src/riscv_fetch_ahead/detailed_o3/data_access_result_pair_policy.rs");
    let effect_policy_path =
        crate_dir.join("src/riscv_fetch_ahead/detailed_o3/data_access_result_effect_policy.rs");
    let dependent_address_path =
        crate_dir.join("src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs");
    let retained_path =
        crate_dir.join("src/riscv_fetch_ahead/detailed_o3/retained_data_access_result.rs");
    let fetch_tests_root_path = crate_dir.join("src/riscv_fetch_ahead/tests.rs");
    let dependent_address_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/dependent_result_address.rs");
    let dependent_address_two_pending_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs");
    let dependent_address_three_pending_test_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/dependent_result_address_three_pending.rs");
    let root = fs::read_to_string(&root_path).unwrap();
    let fetch_tests_root = fs::read_to_string(&fetch_tests_root_path).unwrap();
    let production = rust_source_files(&source_root)
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            (!is_test_only_rust_source(&relative)).then(|| {
                let source = fs::read_to_string(path).unwrap();
                (relative, production_rust_source(&source))
            })
        })
        .collect::<Vec<_>>();
    let production_source = |path: &Path| {
        production
            .iter()
            .find_map(|(candidate, source)| (candidate == path).then_some(source.as_str()))
            .unwrap_or_else(|| panic!("missing production source {}", path.display()))
    };
    let root_production = production_source(root_owner_path);
    let child_production = production_source(child_owner_path);
    let translation_production = production_source(translation_owner_path);

    assert!(
        root.contains("#[path = \"detailed_o3/data_access_result.rs\"]\nmod data_access_result;"),
        "detailed_o3.rs must delegate result admission to the focused child"
    );
    assert!(
        root.contains(
            "#[path = \"detailed_o3/data_access_result_translation.rs\"]\nmod data_access_result_translation;"
        ),
        "detailed_o3.rs must delegate result translation probing to the focused child"
    );
    assert_eq!(
        path_owned_module_path_declaration_count(
            &root,
            "detailed_o3/data_access_result_translation.rs"
        ),
        1,
        "detailed_o3.rs must attach result translation probing exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "detailed_o3/data_access_result_translation.rs",
            "data_access_result_translation"
        ),
        1,
        "detailed_o3.rs must declare result translation probing exactly once"
    );
    assert!(
        root.contains(
            "#[path = \"detailed_o3/data_access_result_pair_policy.rs\"]\nmod data_access_result_pair_policy;"
        ),
        "detailed_o3.rs must delegate pair policy to the focused child"
    );
    assert!(
        root.contains(
            "#[path = \"detailed_o3/data_access_result_effect_policy.rs\"]\nmod data_access_result_effect_policy;"
        ),
        "detailed_o3.rs must delegate effect policy to the focused child"
    );
    assert!(
        root.contains(
            "#[path = \"detailed_o3/dependent_result_address.rs\"]\nmod dependent_result_address;"
        ),
        "detailed_o3.rs must delegate dependent-result address policy to the focused child"
    );
    assert!(
        root.contains(
            "#[path = \"detailed_o3/retained_data_access_result.rs\"]\nmod retained_data_access_result;"
        ),
        "detailed_o3.rs must delegate retained result authority to the focused child"
    );
    assert!(
        child_path.is_file(),
        "data-access result fetch authority belongs in {}",
        child_path.display()
    );
    assert!(
        translation_path.is_file(),
        "data-access result translation probing belongs in {}",
        translation_path.display()
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let translation = fs::read_to_string(&translation_path).unwrap();
    let pair_policy = fs::read_to_string(&pair_policy_path).unwrap();
    let effect_policy = fs::read_to_string(&effect_policy_path).unwrap();
    let dependent_address = fs::read_to_string(&dependent_address_path).unwrap();
    let retained = fs::read_to_string(&retained_path).unwrap();
    assert!(
        child.lines().count() <= MAX_RISCV_DATA_ACCESS_RESULT_LINES,
        "data_access_result.rs exceeds {MAX_RISCV_DATA_ACCESS_RESULT_LINES} lines"
    );
    assert!(
        translation.lines().count() <= MAX_RISCV_DATA_ACCESS_RESULT_TRANSLATION_LINES,
        "data_access_result_translation.rs exceeds {MAX_RISCV_DATA_ACCESS_RESULT_TRANSLATION_LINES} lines"
    );
    assert!(
        external_module_declaration_lines(&translation).is_empty()
            && path_attribute_lines(&translation).is_empty(),
        "data-access result translation probing must remain a leaf child"
    );
    assert!(
        pair_policy.lines().count() <= MAX_RISCV_DATA_ACCESS_RESULT_PAIR_POLICY_LINES,
        "data_access_result_pair_policy.rs exceeds {MAX_RISCV_DATA_ACCESS_RESULT_PAIR_POLICY_LINES} lines"
    );
    assert!(
        pair_policy.contains("fn result_head_allows_younger_read("),
        "pair policy is missing its focused owner"
    );
    assert!(
        !child.contains("fn result_head_allows_younger_read("),
        "data_access_result.rs must not own pair dependency policy"
    );
    assert!(
        effect_policy.lines().count() <= MAX_RISCV_DATA_ACCESS_RESULT_EFFECT_POLICY_LINES,
        "data_access_result_effect_policy.rs exceeds {MAX_RISCV_DATA_ACCESS_RESULT_EFFECT_POLICY_LINES} lines"
    );
    for anchor in [
        "fn data_access_result_younger_authorization(",
        "fn result_head_allows_younger_effect(",
    ] {
        assert!(
            effect_policy.contains(anchor),
            "effect policy is missing `{anchor}`"
        );
        assert!(
            !child.contains(anchor),
            "data_access_result.rs must not own effect policy `{anchor}`"
        );
    }
    assert!(dependent_address_path.is_file());
    assert!(include_macro_lines(&dependent_address).is_empty());
    assert!(
        dependent_address.lines().count() <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_LINES,
        "dependent_result_address.rs exceeds {MAX_RISCV_DEPENDENT_RESULT_ADDRESS_LINES} lines"
    );
    assert!(
        dependent_address.contains(
            "pub(in crate::riscv_fetch_ahead) fn dependent_result_address_authorization("
        ),
        "dependent-result address policy is missing its focused owner"
    );
    for anchor in [
        "pub(in crate::riscv_fetch_ahead) struct DependentResultAddressAuthorizer",
        "pub(in crate::riscv_fetch_ahead) fn from_head(",
        "pub(in crate::riscv_fetch_ahead) fn try_authorize_next(",
        "pub(in crate::riscv_fetch_ahead) fn result_destinations(",
        "pub(in crate::riscv_fetch_ahead) const fn dependent_rows(",
    ] {
        assert!(
            dependent_address.contains(anchor),
            "dependent-result address policy is missing `{anchor}`"
        );
    }
    for anchor in [
        "DependentResultAddressAuthorizer::from_head(",
        "authorizer.try_authorize_next(&younger)",
        "authorizer.result_destinations().iter().copied()",
        "dependent_rows == 0 && result_rows == 1",
        "dependent_rows = authorizer.dependent_rows();",
        "result_rows = 1 + dependent_rows;",
    ] {
        assert!(
            child.contains(anchor),
            "data-access result loop is missing bounded authorizer integration `{anchor}`"
        );
    }
    let dependent_address_code = rust_code_without_comments_and_literals(&dependent_address);
    assert_eq!(
        rust_function_definition_count(
            &dependent_address_code,
            "dependent_result_address_authorization"
        ),
        1,
        "dependent-result address policy must own exactly one helper definition"
    );
    assert!(
        root.contains(
            "pub(super) use dependent_result_address::dependent_result_address_authorization;\n#[cfg(test)]\npub(super) use dependent_result_address::DependentResultAddressAuthorizer;"
        ),
        "detailed_o3.rs must keep the authorizer re-export test-only beside the normal compatibility helper"
    );
    for (label, source) in [
        ("detailed_o3.rs", root.as_str()),
        ("dependent_result_address.rs", dependent_address.as_str()),
    ] {
        for suppression in ["#[allow(dead_code)]", "#[allow(unused_imports)]"] {
            assert!(
                !source.contains(suppression),
                "{label} must not use production warning suppression `{suppression}`"
            );
        }
    }
    assert!(
        external_module_declaration_lines(&dependent_address).is_empty()
            && path_attribute_lines(&dependent_address).is_empty(),
        "dependent-result address policy must remain a leaf child"
    );
    assert!(
        !root.contains("fn dependent_result_address_authorization(")
            && !child.contains("fn dependent_result_address_authorization("),
        "dependent-result address helper escaped into a root owner"
    );
    assert!(
        retained.lines().count() <= MAX_RISCV_RETAINED_DATA_ACCESS_RESULT_LINES,
        "retained_data_access_result.rs exceeds {MAX_RISCV_RETAINED_DATA_ACCESS_RESULT_LINES} lines"
    );
    assert!(
        retained.contains("fn retained_data_access_result_window_candidate("),
        "retained result authority is missing its focused owner"
    );
    assert!(
        !child.contains("fn retained_data_access_result_window_candidate("),
        "data_access_result.rs must not own retained result lifecycle"
    );
    for anchor in [
        "fn data_access_result_fetch_ahead_shape(",
        "pub(in crate::riscv_fetch_ahead) fn data_access_result_fetch_ahead_authorization(",
    ] {
        assert!(
            child.contains(anchor),
            "focused data-access result owner is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "detailed_o3.rs still owns `{anchor}`"
        );
    }
    let expected_translation_owner = [translation_owner_path.display().to_string()];
    for function in [
        "data_access_result_head_physical_probe",
        "data_access_result_head_probe",
        "data_access_result_translation_probe",
        "translated_younger_result_authorization",
    ] {
        let owners = production
            .iter()
            .filter_map(|(path, source)| {
                (rust_function_definition_count(source, function) > 0)
                    .then(|| path.display().to_string())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            owners, expected_translation_owner,
            "translated result probe function `{function}` must have exactly one focused production owner"
        );
        assert_eq!(
            rust_function_definition_count(translation_production, function),
            1,
            "translated result probe function `{function}` must have exactly one definition"
        );
    }
    for (keyword, item) in [
        ("enum", "DataAccessResultHeadPhysicalProbe"),
        ("struct", "DataAccessResultHeadProbe"),
        ("enum", "DataAccessResultTranslationProbe"),
    ] {
        let owners = production
            .iter()
            .filter_map(|(path, source)| {
                production_defines_exact_named_item(source, keyword, item)
                    .then(|| path.display().to_string())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            owners, expected_translation_owner,
            "translated result probe {keyword} `{item}` must have exactly one focused production owner"
        );
        let definitions = match keyword {
            "struct" => production_struct_definitions(translation_production),
            "enum" => production_enum_definitions(translation_production),
            _ => unreachable!("translated probe items are structs or enums"),
        };
        assert_eq!(
            definitions
                .iter()
                .filter(|definition| {
                    production_defines_exact_named_item(definition, keyword, item)
                })
                .count(),
            1,
            "translated result probe {keyword} `{item}` must have exactly one definition"
        );
    }
    let translation_reexport_statements = root_production
        .split(';')
        .filter(|statement| {
            let chars = statement.chars().collect::<Vec<_>>();
            rust_identifier_count(&chars, "use") == 1
                && rust_identifier_count(&chars, "data_access_result_translation") == 1
        })
        .collect::<Vec<_>>();
    assert!(
        !translation_reexport_statements.is_empty(),
        "detailed_o3.rs must re-export the public physical probe from the translation child"
    );
    let mut translation_reexports = Vec::new();
    for statement in translation_reexport_statements {
        let compact = statement
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(
            compact.starts_with("pub(super)use"),
            "translation-child compatibility names must remain parent-visible re-exports"
        );
        let chars = statement.chars().collect::<Vec<_>>();
        let mut index = 0;
        while index < chars.len() {
            let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
                index += 1;
                continue;
            };
            if identifier != "data_access_result_translation" {
                index = end;
                continue;
            }
            let separator = skip_rust_whitespace(&chars, end);
            assert_eq!(chars.get(separator), Some(&':'));
            assert_eq!(chars.get(separator + 1), Some(&':'));
            let import_start = skip_rust_whitespace(&chars, separator + 2);
            if chars.get(import_start) == Some(&'{') {
                let import_end = matching_delimiter(&chars, import_start, '{', '}')
                    .expect("translation-child use group must close");
                let mut import_index = import_start + 1;
                while import_index < import_end {
                    let Some((imported, imported_end)) = rust_identifier_at(&chars, import_index)
                    else {
                        import_index += 1;
                        continue;
                    };
                    translation_reexports.push(imported);
                    import_index = imported_end;
                }
            } else {
                let (imported, imported_end) = rust_identifier_at(&chars, import_start)
                    .expect("translation-child use must name an imported item");
                translation_reexports.push(imported);
                let alias_start = skip_rust_whitespace(&chars, imported_end);
                if let Some((alias_keyword, _)) = rust_identifier_at(&chars, alias_start) {
                    translation_reexports.push(alias_keyword);
                }
            }
            break;
        }
    }
    translation_reexports.sort();
    assert_eq!(
        translation_reexports,
        [
            "DataAccessResultHeadPhysicalProbe".to_string(),
            "data_access_result_head_physical_probe".to_string(),
        ],
        "detailed_o3.rs must re-export exactly the two existing public physical probe names"
    );
    let root_production_chars = root_production.chars().collect::<Vec<_>>();
    for private_helper in [
        "DataAccessResultHeadProbe",
        "data_access_result_head_probe",
        "data_access_result_translation_probe",
        "DataAccessResultTranslationProbe",
    ] {
        assert_eq!(
            rust_identifier_count(&root_production_chars, private_helper),
            0,
            "detailed_o3.rs must not re-export private translation helper `{private_helper}`"
        );
    }
    let child_production_chars = child_production.chars().collect::<Vec<_>>();
    for private_helper in [
        "data_access_result_head_probe",
        "data_access_result_translation_probe",
        "DataAccessResultTranslationProbe",
    ] {
        assert!(
            rust_identifier_count(&child_production_chars, private_helper) > 0,
            "data_access_result.rs must import private translation helper `{private_helper}`"
        );
    }
    assert_eq!(
        rust_code_without_comments_and_literals(&fetch_tests_root)
            .matches("mod dependent_result_address;")
            .count(),
        1,
        "fetch-ahead tests must attach dependent_result_address exactly once"
    );
    assert_eq!(
        rust_code_without_comments_and_literals(&fetch_tests_root)
            .matches("mod dependent_result_address_two_pending;")
            .count(),
        1,
        "fetch-ahead tests must attach dependent_result_address_two_pending exactly once"
    );
    assert_eq!(
        rust_code_without_comments_and_literals(&fetch_tests_root)
            .matches("mod dependent_result_address_three_pending;")
            .count(),
        1,
        "fetch-ahead tests must attach dependent_result_address_three_pending exactly once"
    );
    assert!(dependent_address_test_path.is_file());
    let dependent_address_test_source = fs::read_to_string(&dependent_address_test_path).unwrap();
    assert!(include_macro_lines(&dependent_address_test_source).is_empty());
    assert!(
        line_count(&dependent_address_test_path)
            <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_FETCH_TEST_LINES,
        "dependent_result_address.rs exceeds {MAX_RISCV_DEPENDENT_RESULT_ADDRESS_FETCH_TEST_LINES} lines"
    );
    let dependent_address_test =
        rust_code_without_comments_and_literals(&dependent_address_test_source);
    assert!(
        external_module_declaration_lines(&dependent_address_test_source).is_empty()
            && path_attribute_lines(&dependent_address_test_source).is_empty(),
        "dependent-result address fetch tests must remain a leaf child"
    );
    let expected_tests = [
        "dependent_scalar_ld_authorizes_addressless_younger_read",
        "dependent_address_fetches_second_scalar_suffix_after_first_dependency",
        "dependent_address_fetch_rejects_non_exact_load_shapes",
        "dependent_address_authorization_requires_integer_result_head",
        "dependent_address_atomic_head_rejects_ordering_and_allows_unordered",
        "dependent_address_authorization_rejects_translation_and_mmio_heads",
        "dependent_address_counts_as_second_result_and_blocks_third_result",
        "dependent_address_window_remains_four_rows_at_scalar_live_depth_eight",
        "retained_unissued_head_preserves_dependent_address_authority",
    ];
    assert_eq!(
        dependent_address_test.matches("#[test]").count(),
        expected_tests.len(),
        "dependent-result address fetch tests must expose exactly the focused inventory"
    );
    for anchor in expected_tests {
        assert_eq!(
            rust_function_definition_count(&dependent_address_test, anchor),
            1,
            "missing or duplicated dependent-result address fetch test `{anchor}`"
        );
    }
    assert!(dependent_address_two_pending_test_path.is_file());
    let two_pending_test_source =
        fs::read_to_string(&dependent_address_two_pending_test_path).unwrap();
    assert!(include_macro_lines(&two_pending_test_source).is_empty());
    assert!(
        line_count(&dependent_address_two_pending_test_path)
            <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_TWO_PENDING_FETCH_TEST_LINES,
        "dependent_result_address_two_pending.rs exceeds {MAX_RISCV_DEPENDENT_RESULT_ADDRESS_TWO_PENDING_FETCH_TEST_LINES} lines"
    );
    let two_pending_test = rust_code_without_comments_and_literals(&two_pending_test_source);
    assert!(
        external_module_declaration_lines(&two_pending_test_source).is_empty()
            && path_attribute_lines(&two_pending_test_source).is_empty(),
        "two-pending dependent-result address fetch tests must remain a leaf child"
    );
    let expected_two_pending_tests = [
        "dependent_address_two_pending_authorizes_sibling_loads_before_suffix",
        "dependent_address_two_pending_authorizes_one_deep_chain_before_suffix",
        "dependent_address_depth_three_rejects_a_second_pending_row",
        "dependent_address_two_pending_rejects_duplicate_self_cycle_and_unrelated_graphs",
        "dependent_address_two_pending_window_records_both_authorizations",
        "dependent_address_two_pending_split_fetch_uses_previous_last_request",
        "dependent_address_two_pending_rejects_late_pending_after_scalar",
        "dependent_address_two_pending_rejects_dependent_plus_unrelated_memory_result",
    ];
    assert_eq!(
        two_pending_test.matches("#[test]").count(),
        expected_two_pending_tests.len(),
        "two-pending dependent-result address fetch tests must expose exactly the focused inventory"
    );
    for anchor in expected_two_pending_tests {
        assert_eq!(
            rust_function_definition_count(&two_pending_test, anchor),
            1,
            "missing or duplicated two-pending dependent-result address fetch test `{anchor}`"
        );
    }

    assert!(dependent_address_three_pending_test_path.is_file());
    let three_pending_test_source =
        fs::read_to_string(&dependent_address_three_pending_test_path).unwrap();
    assert!(include_macro_lines(&three_pending_test_source).is_empty());
    assert!(
        line_count(&dependent_address_three_pending_test_path)
            <= MAX_RISCV_DEPENDENT_RESULT_ADDRESS_THREE_PENDING_FETCH_TEST_LINES,
        "dependent_result_address_three_pending.rs exceeds {MAX_RISCV_DEPENDENT_RESULT_ADDRESS_THREE_PENDING_FETCH_TEST_LINES} lines"
    );
    let three_pending_test = rust_code_without_comments_and_literals(&three_pending_test_source);
    assert!(
        external_module_declaration_lines(&three_pending_test_source).is_empty()
            && path_attribute_lines(&three_pending_test_source).is_empty(),
        "three-pending dependent-result address fetch tests must remain a leaf child"
    );
    let expected_three_pending_tests = [
        "dependent_address_three_pending_authorizes_siblings_at_depth_four",
        "dependent_address_three_pending_authorizes_full_chain_at_depth_four",
        "dependent_address_three_pending_authorizes_mixed_fanout_at_depth_four",
        "dependent_address_three_pending_rejects_fourth_and_nonadjacent_graphs",
        "dependent_address_three_pending_window_records_three_split_fetch_authorizations",
        "dependent_address_three_pending_rejects_late_memory_after_scalar_start",
    ];
    assert_eq!(
        three_pending_test.matches("#[test]").count(),
        expected_three_pending_tests.len(),
        "three-pending dependent-result address fetch tests must expose exactly the focused inventory"
    );
    for anchor in expected_three_pending_tests {
        assert_eq!(
            rust_function_definition_count(&three_pending_test, anchor),
            1,
            "missing or duplicated three-pending dependent-result address fetch test `{anchor}`"
        );
    }

    let production = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .map(|source| production_rust_source(&source))
        .collect::<String>();
    for stale in [
        "O3MemoryResultScalarSuffixAuthorization",
        "O3MemoryResultScalarSuffixRoute",
        "memory_result_scalar_suffix_authorizations",
        "MemoryResultScalarSuffix",
    ] {
        assert!(
            !production.contains(stale),
            "stale result-window production name `{stale}`"
        );
    }
}

#[test]
fn riscv_memory_result_authorization_has_focused_ownership() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/riscv_fetch_ahead.rs");
    let owner_path = crate_dir.join("src/riscv_fetch_ahead/memory_result_authorization.rs");
    let translated_path =
        crate_dir.join("src/riscv_fetch_ahead/memory_result_authorization/translated.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert!(
        root.contains(
            "#[path = \"riscv_fetch_ahead/memory_result_authorization.rs\"]\nmod memory_result_authorization;"
        ),
        "riscv_fetch_ahead.rs must delegate result authorization to a focused owner"
    );
    assert!(
        owner_path.is_file(),
        "memory-result authorization belongs in {}",
        owner_path.display()
    );
    assert!(
        translated_path.is_file(),
        "translated memory-result authorization belongs in {}",
        translated_path.display()
    );
    let owner = fs::read_to_string(&owner_path).unwrap();
    let translated = fs::read_to_string(&translated_path).unwrap();
    assert!(
        owner.lines().count() <= MAX_RISCV_MEMORY_RESULT_AUTHORIZATION_LINES,
        "memory_result_authorization.rs exceeds {MAX_RISCV_MEMORY_RESULT_AUTHORIZATION_LINES} lines"
    );
    assert!(
        translated.lines().count() <= MAX_RISCV_MEMORY_RESULT_TRANSLATED_AUTHORIZATION_LINES,
        "memory_result_authorization/translated.rs exceeds {MAX_RISCV_MEMORY_RESULT_TRANSLATED_AUTHORIZATION_LINES} lines"
    );
    assert_eq!(
        path_owned_module_path_declaration_count(
            &owner,
            "memory_result_authorization/translated.rs"
        ),
        1,
        "memory-result authorization must attach its translated child exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &owner,
            "memory_result_authorization/translated.rs",
            "translated"
        ),
        1,
        "memory-result authorization must declare its translated child exactly once"
    );
    for anchor in [
        "#[path = \"memory_result_authorization/translated.rs\"]\nmod translated;",
        "enum O3MemoryResultWindowRoute",
        "Translated,",
        "enum O3MemoryResultWindowRole",
        "YoungerDependentRead",
        "enum O3MemoryResultWindowAddressAuthority",
        "ResolvedRange(AddressRange)",
        "TranslatedRange {",
        "physical_range: Option<AddressRange>",
        "target: Option<O3MemoryResultWindowRoute>",
        "DependentSource",
        "struct O3MemoryResultWindowAuthorization",
        "address_authority: O3MemoryResultWindowAddressAuthority",
        "const fn resolved(",
        "const fn dependent(\n        integer_destination: Register,",
        "integer_destination: Some(integer_destination)",
        "const fn is_younger(self)",
        "const fn is_buffered_effect(self)",
        "const fn resolved_range(self)",
        "const fn dependent_source(self)",
        "const fn route(self)",
        "fn matches_resolved_range(",
    ] {
        assert!(
            owner.contains(anchor),
            "focused memory-result authorization owner is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "riscv_fetch_ahead.rs still owns `{anchor}`"
        );
    }
    for method in [
        "translated_unbound",
        "bind_translated",
        "bind_target",
        "is_translated",
        "virtual_range",
        "matches_virtual_range",
        "matches_bound_target",
    ] {
        assert_eq!(
            rust_function_definition_count(&production_rust_source(&translated), method),
            1,
            "translated memory-result authorization must define `{method}` exactly once"
        );
        assert_eq!(
            rust_function_definition_count(&production_rust_source(&owner), method),
            0,
            "memory-result authorization parent must not define translated method `{method}`"
        );
        assert_eq!(
            rust_function_definition_count(&production_rust_source(&root), method),
            0,
            "fetch-ahead root must not define translated method `{method}`"
        );
    }
    assert!(
        external_module_declaration_lines(&translated).is_empty()
            && path_attribute_lines(&translated).is_empty(),
        "translated memory-result authorization must remain a leaf child"
    );
    assert!(
        !owner.contains("fn physical_range(") && !owner.contains("fn matches(\n"),
        "memory-result authorization must expose resolved-only matching"
    );
    let production = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .map(|source| production_rust_source(&source))
        .collect::<String>();
    for stale in [
        "BufferedO3Store",
        "buffered_o3_stores",
        "ready_buffered_o3_store",
        "schedule_buffered_o3_store",
        "record_buffered_o3_store_submission",
    ] {
        assert!(
            !production.contains(stale),
            "stale buffered-store production name `{stale}`"
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
fn o3_live_issue_queue_owns_candidate_inventory() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let issue_path = crate_dir.join("src/o3_runtime_issue.rs");
    let queue_path = crate_dir.join("src/o3_runtime_issue/queue.rs");
    let queue_tests_path = crate_dir.join("src/o3_runtime_issue/queue_tests.rs");
    let issue_tests_path = crate_dir.join("src/o3_runtime_issue_tests.rs");
    let control_path = crate_dir.join("src/o3_runtime_control_window.rs");
    let live_window_path = crate_dir.join("src/o3_runtime_live_window.rs");
    let retire_path = crate_dir.join("src/riscv_live_retire_window.rs");
    let issue_source = fs::read_to_string(&issue_path).unwrap();
    let issue = production_rust_source(&issue_source);
    let queue_source = fs::read_to_string(&queue_path).unwrap();
    let queue = production_rust_source(&queue_source);
    let queue_tests_source = fs::read_to_string(&queue_tests_path).unwrap();
    let issue_tests_source = fs::read_to_string(&issue_tests_path).unwrap();
    let control = production_rust_source(&fs::read_to_string(&control_path).unwrap());
    let live_window = production_rust_source(&fs::read_to_string(&live_window_path).unwrap());
    let retire = production_rust_source(&fs::read_to_string(&retire_path).unwrap());

    assert!(line_count(&queue_path) <= MAX_O3_RUNTIME_ISSUE_QUEUE_LINES);
    assert!(line_count(&queue_tests_path) <= MAX_O3_RUNTIME_ISSUE_QUEUE_TEST_LINES);
    assert_eq!(
        path_owned_module_path_declaration_count(&issue_source, "o3_runtime_issue/queue.rs",),
        1,
    );
    assert_eq!(
        path_owned_module_declaration_count(&issue_source, "o3_runtime_issue/queue.rs", "queue",),
        1,
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_tests_source,
            "o3_runtime_issue/queue_tests.rs",
            "queue",
        ),
        1,
    );

    let queue_authority_patterns = [
        "struct O3LiveIssuePacket",
        "struct O3LiveIssueQueueEntry",
        "struct O3LiveIssueQueue",
        "enum O3LiveIssueQueueCapture",
        "struct O3LiveIssueSchedulingCandidate",
        "struct O3LiveSpeculativeIssueCandidate",
        "fn live_issue_scheduling_candidate_from_metadata(",
        "fn materialize_live_speculative_issue_candidate(",
        "fn live_issue_source_producers(",
        "fn live_issue_op_class(",
        "fn live_issue_instruction_is_supported(",
        "fn valid_recorded_execution(",
        "fn consumes_writeback_slot(",
    ];
    for anchor in queue_authority_patterns {
        assert!(queue.contains(anchor), "queue owner missing `{anchor}`");
    }

    for forbidden in [
        "candidate.scheduling.",
        "candidate.producer_sequences,",
        "O3LiveSpeculativeIssueKind::",
    ] {
        assert!(
            !control.contains(forbidden),
            "control owner reaches into `{forbidden}`"
        );
    }

    let production_sources = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            (!is_test_only_rust_source(&relative)).then(|| {
                let source = fs::read_to_string(path).unwrap();
                (relative, production_rust_source(&source))
            })
        })
        .collect::<Vec<_>>();
    let mut offenders = Vec::new();
    for (relative, source) in &production_sources {
        if relative == Path::new("src/o3_runtime_issue/queue.rs") {
            continue;
        }
        for anchor in queue_authority_patterns {
            if source.contains(anchor) {
                offenders.push(format!("{} retains `{anchor}`", relative.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "live issue queue authority must live only in queue.rs: {}",
        offenders.join(", ")
    );
    let stale_request_struct_owners = production_sources
        .iter()
        .filter_map(|(relative, source)| {
            production_struct_definitions(source)
                .into_iter()
                .any(|definition| {
                    production_defines_exact_named_item(&definition, "struct", "O3LiveIssueRequest")
                })
                .then(|| relative.display().to_string())
        })
        .collect::<Vec<_>>();
    assert!(
        stale_request_struct_owners.is_empty(),
        "legacy O3LiveIssueRequest definitions remain: {}",
        stale_request_struct_owners.join(", ")
    );
    for stale in ["live_issue_candidates", "live_issue_request_is_recorded"] {
        let owners = production_sources
            .iter()
            .filter_map(|(relative, source)| {
                production_defines_exact_function(source, stale)
                    .then(|| relative.display().to_string())
            })
            .collect::<Vec<_>>();
        assert!(
            owners.is_empty(),
            "legacy live issue request helper `{stale}` remains in {}",
            owners.join(", ")
        );
    }
    for (owner, source) in [("issue root", &issue), ("queue owner", &queue)] {
        let chars = source.chars().collect::<Vec<_>>();
        assert!(
            !contains_rust_identifier(&chars, "O3LiveIssueRequest"),
            "{owner} retains the legacy O3LiveIssueRequest type"
        );
    }
    let queue_struct_definitions = production_struct_definitions(&queue);
    for candidate in [
        "O3LiveIssueSchedulingCandidate",
        "O3LiveSpeculativeIssueCandidate",
    ] {
        let definitions = queue_struct_definitions
            .iter()
            .filter(|definition| {
                production_defines_exact_named_item(definition, "struct", candidate)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            definitions.len(),
            1,
            "queue owner must define exactly one `{candidate}`"
        );
        let chars = definitions[0].chars().collect::<Vec<_>>();
        assert!(
            !contains_rust_identifier(&chars, "request_index"),
            "queue candidate `{candidate}` retains request_index"
        );
    }
    assert!(
        !production_defines_exact_function(&queue, "request_index"),
        "queue owner retains a request_index accessor"
    );
    let persistent_queue_storage = production_sources
        .iter()
        .filter_map(|(relative, source)| {
            let mut definitions = production_struct_definitions(source);
            definitions.extend(production_enum_definitions(source));
            definitions.extend(production_type_alias_definitions(source));
            let storage_count = definitions
                .iter()
                .map(|definition| {
                    let storage_count = production_definition_named_type_storage_count(
                        definition,
                        "O3LiveIssueQueue",
                    );
                    let canonical_capture = relative == Path::new("src/o3_runtime_issue/queue.rs")
                        && production_defines_exact_named_item(
                            definition,
                            "enum",
                            "O3LiveIssueQueueCapture",
                        )
                        && storage_count == 1
                        && enum_tuple_variant_payload(definition, "Ready")
                            .is_some_and(|payload| payload.trim() == "O3LiveIssueQueue");
                    if canonical_capture {
                        0
                    } else {
                        storage_count
                    }
                })
                .sum::<usize>();
            (storage_count > 0).then(|| (relative, storage_count))
        })
        .map(|(relative, storage_count)| {
            format!(
                "{} ({storage_count} exact O3LiveIssueQueue references)",
                relative.display()
            )
        })
        .collect::<Vec<_>>();
    assert!(
        persistent_queue_storage.is_empty(),
        "production types must not persist a derived live issue queue: {}",
        persistent_queue_storage.join(", "),
    );

    let schedule = rust_function_definition(&issue, "schedule_live_speculative_issues")
        .expect("missing live issue scheduler");
    assert_eq!(schedule.matches("O3LiveIssueQueue::capture(").count(), 1);
    let loop_position = schedule.find("loop {").unwrap();
    let queue_position = schedule.find("O3LiveIssueQueue::capture(").unwrap();
    let dependency_position = schedule.find("O3LiveIssueDependencyTable::new(").unwrap();
    let calendar_position = schedule.find("O3LiveIssueCalendar::capture(").unwrap();
    let plan_position = schedule.find(".plan_at(").unwrap();
    assert!(
        loop_position < queue_position
            && queue_position < dependency_position
            && dependency_position < calendar_position
            && calendar_position < plan_position,
    );
    let prepare = rust_function_definition(&issue, "prepare_live_issue_batch")
        .expect("missing live issue preparation");
    assert!(prepare.contains("queue.entry(issued.sequence())"));
    let scheduling_from_metadata =
        rust_function_definition(&queue, "live_issue_scheduling_candidate_from_metadata")
            .expect("missing queue metadata scheduling path");
    let scheduling_from_instruction =
        rust_function_definition(&queue, "live_issue_scheduling_candidate_from_instruction")
            .expect("missing queue instruction scheduling path");
    let materialize =
        rust_function_definition(&queue, "materialize_live_speculative_issue_candidate")
            .expect("missing queue candidate materialization path");
    for (owner, definition) in [
        ("live issue scheduler", &schedule),
        ("live issue preparation", &prepare),
    ] {
        let chars = definition.chars().collect::<Vec<_>>();
        for legacy in ["requests", "O3LiveIssueRequest"] {
            assert!(
                !contains_rust_identifier(&chars, legacy),
                "{owner} retains legacy request identifier `{legacy}`"
            );
        }
    }
    for (owner, definition) in [
        ("live issue scheduler", &schedule),
        ("live issue preparation", &prepare),
        ("queue metadata scheduling", &scheduling_from_metadata),
        ("queue instruction scheduling", &scheduling_from_instruction),
        ("queue candidate materialization", &materialize),
    ] {
        let chars = definition.chars().collect::<Vec<_>>();
        assert!(
            !contains_rust_identifier(&chars, "request_index"),
            "{owner} retains request_index"
        );
    }
    let retire_schedule =
        rust_function_definition(&retire, "schedule_o3_live_speculative_younger_executions")
            .expect("missing retire-window live issue delegation");
    let retire_schedule_chars = retire_schedule.chars().collect::<Vec<_>>();
    for legacy in ["requests", "O3LiveIssueRequest"] {
        assert!(
            !contains_rust_identifier(&retire_schedule_chars, legacy),
            "retire-window live issue delegation retains `{legacy}`"
        );
    }
    assert_eq!(
        retire_schedule
            .matches(".schedule_live_speculative_issues(&hart, head, issue_tick)")
            .count(),
        1,
        "retire-window live issue path must delegate directly without a request adapter"
    );
    assert!(live_window.contains("issue_packet: Option<O3LiveIssuePacket>"));

    for anchor in [
        "live_issue_queue_packet_binding_is_idempotent_and_exact",
        "live_issue_queue_packet_rebinding_rejects_any_identity_change",
        "live_issue_queue_capture_is_sequence_ordered_and_requires_bound_packets",
        "live_issue_queue_lookup_is_sequence_owned",
        "live_issue_queue_excludes_unsupported_bound_packets",
        "live_issue_queue_rejects_duplicate_sequence_inventory",
        "live_issue_queue_excludes_invalidated_descendant_identities",
        "live_issue_queue_stale_pending_row_returns_exact_replay_boundary",
        "live_issue_queue_excludes_materialized_pending_rows",
    ] {
        assert_eq!(
            rust_test_function_definition_count(&queue_tests_source, anchor),
            1,
            "missing exact compiled queue test `{anchor}`",
        );
    }
    assert_eq!(
        rust_test_function_definition_count(
            &issue_tests_source,
            "scoped_issue_partial_reentry_keeps_previously_bound_rows_visible",
        ),
        1,
    );
}

#[test]
fn o3_live_issue_calendar_owns_reservations_and_arbiter() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let issue_path = crate_dir.join("src/o3_runtime_issue.rs");
    let calendar_path = crate_dir.join("src/o3_runtime_issue/calendar.rs");
    let calendar_tests_path = crate_dir.join("src/o3_runtime_issue/calendar_tests.rs");
    let issue_tests_path = crate_dir.join("src/o3_runtime_issue_tests.rs");
    let issue_source = fs::read_to_string(&issue_path).unwrap();
    let issue = production_rust_source(&issue_source);
    let calendar = production_rust_source(&fs::read_to_string(&calendar_path).unwrap());
    let calendar_tests_source = fs::read_to_string(&calendar_tests_path).unwrap();
    let issue_tests_source = fs::read_to_string(&issue_tests_path).unwrap();

    assert!(
        line_count(&issue_path) <= MAX_O3_RUNTIME_ISSUE_LINES,
        "src/o3_runtime_issue.rs exceeds {MAX_O3_RUNTIME_ISSUE_LINES} lines"
    );
    assert!(
        line_count(&calendar_path) <= MAX_O3_RUNTIME_ISSUE_CALENDAR_LINES,
        "src/o3_runtime_issue/calendar.rs exceeds {MAX_O3_RUNTIME_ISSUE_CALENDAR_LINES} lines"
    );
    assert!(
        line_count(&calendar_tests_path) <= MAX_O3_RUNTIME_ISSUE_CALENDAR_TEST_LINES,
        "src/o3_runtime_issue/calendar_tests.rs exceeds {MAX_O3_RUNTIME_ISSUE_CALENDAR_TEST_LINES} lines"
    );
    assert_eq!(
        path_owned_module_path_declaration_count(&issue_source, "o3_runtime_issue/calendar.rs"),
        1,
        "src/o3_runtime_issue.rs must attach o3_runtime_issue/calendar.rs exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_source,
            "o3_runtime_issue/calendar.rs",
            "calendar"
        ),
        1,
        "src/o3_runtime_issue.rs must declare o3_runtime_issue/calendar.rs exactly once"
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &issue_tests_source,
            "o3_runtime_issue/calendar_tests.rs",
            "calendar"
        ),
        1,
        "src/o3_runtime_issue_tests.rs must compile the focused calendar tests exactly once"
    );

    let calendar_authority_patterns = [
        "struct O3LiveIssueCalendar",
        "struct O3LiveIssueCalendar {\n    issue_width: usize,\n    memory_issue_width: usize,",
        "struct O3LiveIssueReservations",
        "struct O3LiveIssueCyclePlan",
        "struct O3LiveIssueTickDecision",
        "O3ScopedIssueScheduler::new(",
        "fn live_issue_capacities_after_reservations(",
        "memory_issue_width: runtime.memory_issue_width(),",
        "memory_issue_width.saturating_sub(reservations.memory)",
        "const LIVE_ISSUE_QUEUE",
    ];
    for anchor in calendar_authority_patterns {
        assert!(
            calendar.contains(anchor),
            "live issue calendar authority is missing `{anchor}`"
        );
    }

    let production_sources = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().to_path_buf();
            (!is_test_only_rust_source(&relative)).then(|| {
                let source = fs::read_to_string(path).unwrap();
                (relative, production_rust_source(&source))
            })
        })
        .collect::<Vec<_>>();
    let mut offenders = Vec::new();
    for (relative, source) in &production_sources {
        if relative == Path::new("src/o3_runtime_issue/calendar.rs") {
            continue;
        }
        for anchor in calendar_authority_patterns {
            if source.contains(anchor) {
                offenders.push(format!("{} retains `{anchor}`", relative.display()));
            }
        }
        for stale in [
            "live_issue_reservations_at",
            "live_issue_capacities_after_reservations",
            "pending_data_address_selected_issue_tick_for_reservation",
        ] {
            if source.contains(stale) {
                offenders.push(format!("{} retains `{stale}`", relative.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "live issue reservation and arbitration authority must live only in calendar.rs: {}",
        offenders.join(", ")
    );
    let persistent_calendars =
        production_struct_named_type_storage(&production_sources, "O3LiveIssueCalendar");
    assert!(
        persistent_calendars.is_empty(),
        "production structs must not persist O3LiveIssueCalendar state: {persistent_calendars:?}"
    );

    let calendar_capture = rust_function_definition(&calendar, "capture").unwrap();
    let pending_row_reservation_loop = [
        "for tick in runtime",
        "            .pending_data_addresses",
        "            .iter()",
        "            .filter_map(|pending| pending.selected_issue_tick)",
        "        {",
        "            calendar.reserve(tick, O3IssueOpClass::Memory);",
        "        }",
    ]
    .join("\n");
    assert!(
        calendar_capture.contains(&pending_row_reservation_loop),
        "calendar capture must reserve once per materialized pending row without same-tick deduplication"
    );
    assert!(
        !calendar_capture.contains("collect::<BTreeSet")
            && !calendar_capture.contains("pending_ticks"),
        "calendar capture must not deduplicate selected pending issue ticks"
    );

    for forbidden in [
        "fn live_issue_reservations_at(",
        "struct O3LiveIssueReservations",
        "struct O3LiveIssueTickDecision",
        "fn live_issue_capacities_after_reservations(",
        "O3ScopedIssueScheduler::new(",
        "const LIVE_ISSUE_QUEUE",
    ] {
        assert!(
            !issue.contains(forbidden),
            "src/o3_runtime_issue.rs must not retain `{forbidden}`"
        );
    }
    let schedule = rust_function_definition(&issue, "schedule_live_speculative_issues")
        .expect("missing schedule_live_speculative_issues definition");
    assert_eq!(
        schedule.matches("O3LiveIssueCalendar::capture(").count(),
        1,
        "schedule_live_speculative_issues must capture exactly one fresh calendar per loop pass"
    );
    let loop_position = schedule.find("loop {").expect("missing scheduling loop");
    let dependency_position = schedule
        .find("let dependency_table = O3LiveIssueDependencyTable::new(")
        .expect("missing per-pass dependency table");
    let capture_position = schedule
        .find("O3LiveIssueCalendar::capture(")
        .expect("missing per-pass calendar capture");
    let plan_position = schedule.find(".plan_at(").expect("missing calendar plan");
    let issued_position = schedule
        .find("let issued_rows = plan.issued().len();")
        .expect("missing issued-row observation");
    assert!(
        loop_position < dependency_position
            && dependency_position < capture_position
            && capture_position < plan_position
            && plan_position < issued_position,
        "calendar capture must stay inside the scheduling loop after dependency discovery and before planning"
    );

    let expected_calendar_tests = [
        "live_issue_calendar_head_and_recorded_head_consume_one_slot",
        "live_issue_calendar_rebuild_releases_removed_prior_row",
        "live_issue_calendar_selected_pending_tick_reserves_memory",
        "live_issue_calendar_memory_width_one_blocks_younger_ready_memory_rows",
        "live_issue_calendar_memory_width_two_selects_two_oldest_ready_memory_rows",
        "live_issue_calendar_total_width_still_bounds_memory_width_four",
        "live_issue_calendar_rebuild_counts_each_same_tick_memory_reservation",
        "live_issue_calendar_separates_resource_and_dependency_blocks",
        "live_issue_calendar_tick_decision_aggregates_same_tick_attempts",
    ];
    for anchor in expected_calendar_tests {
        assert_eq!(
            rust_test_function_definition_count(&calendar_tests_source, anchor),
            1,
            "calendar_tests.rs must compile exact test anchor `{anchor}`"
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
        "self.stats.record_issue_cycle(",
        "pub(crate) fn live_data_access_head_reservation(",
        "pub(super) const fn memory(",
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
    for anchor in ["O3LiveIssueCalendar::capture(", ".plan_at("] {
        assert!(
            module.contains(anchor),
            "src/o3_runtime_issue.rs must delegate live issue arbitration through `{anchor}`"
        );
    }
    assert!(
        !module.contains("O3ScopedIssueScheduler::new("),
        "src/o3_runtime_issue.rs must not construct the scoped issue scheduler directly"
    );
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
fn deep_scalar_issue_dependency_ownership_is_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let issue = fs::read_to_string(crate_dir.join("src/o3_runtime_issue.rs")).unwrap();
    let dependency =
        fs::read_to_string(crate_dir.join("src/o3_runtime_issue/dependency.rs")).unwrap();
    let dependency_tests =
        fs::read_to_string(crate_dir.join("src/o3_runtime_issue_tests/dependency_scopes.rs"))
            .unwrap();
    let cleanup =
        fs::read_to_string(crate_dir.join("src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs"))
            .unwrap();
    assert!(issue.lines().count() <= MAX_O3_RUNTIME_ISSUE_LINES);
    assert!(dependency.lines().count() <= MAX_O3_RUNTIME_ISSUE_DEPENDENCY_LINES);
    assert!(dependency_tests.lines().count() <= MAX_O3_RUNTIME_ISSUE_DEPENDENCY_TEST_LINES);
    assert!(cleanup.lines().count() <= MAX_O3_RUNTIME_DEEP_CLEANUP_TEST_LINES);
    assert!(issue.contains("mod dependency;"));
    for removed in [
        "enum O3LiveIssueDependencyReadiness",
        "fn earliest_dependency_tick",
        "fn live_issue_dependencies_ready_at",
        "fn live_issue_dependency_readiness",
    ] {
        let offenders = rust_source_files(&crate_dir.join("src"))
            .into_iter()
            .filter(|path| fs::read_to_string(path).unwrap().contains(removed))
            .collect::<Vec<_>>();
        assert!(
            offenders.is_empty(),
            "manual issue authority remains: {removed}"
        );
    }

    let defaults_path = crate_dir.join("src/riscv_defaults.rs");
    let defaults = fs::read_to_string(&defaults_path).unwrap();
    let rem6_src = crate_dir.parent().unwrap().join("rem6/src");
    let mut sources = rust_source_files(&crate_dir.join("src"));
    sources.extend(rust_source_files(&rem6_src));
    for constant in [
        "MAX_RISCV_O3_SCALAR_MEMORY_DEPTH",
        "MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH",
    ] {
        assert!(defaults.contains(&format!("pub const {constant}")));
        let duplicate_definitions = sources
            .iter()
            .filter(|path| path.as_path() != defaults_path.as_path())
            .filter(|path| {
                fs::read_to_string(path)
                    .unwrap()
                    .contains(&format!("const {constant}"))
            })
            .collect::<Vec<_>>();
        assert!(
            duplicate_definitions.is_empty(),
            "duplicate scalar-depth definition for {constant}: {duplicate_definitions:?}"
        );
    }
}

#[test]
fn o3_writeback_test_helpers_live_only_in_test_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = [
        "force_test_writeback_reservation_to_memory_result",
        "force_test_writeback_reservation_to_fixed_fu",
        "force_test_writeback_reservation_raw_ready_tick",
        "reserve_test_fixed_fu_writeback",
    ];
    let mut offenders = Vec::new();

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let code = rust_code_without_comments_and_literals(&source);
        for helper in forbidden {
            if production_defines_exact_function(&code, helper) {
                offenders.push(format!("{} defines {helper}(", relative.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "O3 writeback test helpers must live in test-only modules, not production source files: {}",
        offenders.join(", ")
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
    for redundant in [
        "live_writeback_cycle_ticks",
        "live_writeback_ready_rows_by_tick",
    ] {
        for (relative, source) in [
            ("src/o3_runtime.rs", root.as_str()),
            ("src/o3_runtime_writeback/replan.rs", replan.as_str()),
        ] {
            assert!(
                !source.contains(redundant),
                "{relative} must derive live writeback schedule state instead of storing `{redundant}`"
            );
        }
    }
    let runtime_state = source_section(
        &root,
        "pub struct O3RuntimeState {",
        "struct O3PendingDataAccess",
    );
    assert_eq!(
        runtime_state.matches("BTreeSet<u64>").count(),
        5,
        "O3RuntimeState gained an unreviewed sequence-set authority"
    );
    assert!(
        !runtime_state.contains("BTreeMap<u64, BTreeSet<u64>>"),
        "O3RuntimeState must not store a writeback-ready schedule cache"
    );
    let replan_transaction = source_section(
        &replan,
        "pub(super) struct O3WritebackReplanTransaction {",
        "struct O3WritebackReservationPlan",
    );
    let live_data_access = source_section(
        &memory,
        "pub(super) struct O3LiveDataAccess {",
        "pub(crate) enum O3DataAccessWindowPolicy",
    );
    for redundant in [
        "raw_ready_tick: Option<u64>",
        "admitted_writeback_tick: Option<u64>",
        "writeback_slot: Option<usize>",
    ] {
        assert!(
            !live_data_access.contains(redundant),
            "live data access must derive writeback reservation state instead of storing `{redundant}`"
        );
    }
    assert_eq!(
        replan_transaction.matches("BTreeSet<u64>").count(),
        1,
        "bounded writeback replanning gained an unreviewed sequence-set authority"
    );
    for unrelated in [
        "live_data_accesses: Vec<O3LiveDataAccess>",
        "live_control_lineages: BTreeMap<u64, O3LiveControlLineage>",
        "live_serializing_control_sequences: BTreeSet<u64>",
        "live_staged_fetch_identities: BTreeMap<u64, O3LiveStagedFetchIdentity>",
        "published_writeback_sequences: BTreeSet<u64>",
    ] {
        assert!(
            !replan_transaction.contains(unrelated),
            "bounded writeback replanning must borrow read-only authority instead of cloning `{unrelated}`"
        );
    }
    assert!(
        !replan_transaction.contains("BTreeMap<u64, BTreeSet<u64>>"),
        "bounded writeback replanning must not capture a writeback-ready schedule cache"
    );
    assert_eq!(
        module.matches("BTreeMap<u64, BTreeSet<u64>>").count(),
        1,
        "only the derived O3WritebackPortStatsSchedule may own ready rows by tick"
    );
    for (relative, source) in [
        ("src/o3_runtime_writeback/replan.rs", replan.as_str()),
        ("src/o3_runtime_writeback/ownership.rs", ownership.as_str()),
    ] {
        assert!(
            !source.contains("BTreeMap<u64, BTreeSet<u64>>"),
            "{relative} must not store a parallel ready-row schedule"
        );
    }
    let live_schedule = rust_function_definition(&module, "live_writeback_schedule")
        .expect("focused writeback authority must derive the live statistics schedule");
    for anchor in [
        "O3WritebackPortStatsSchedule::from_calendar",
        "writeback_calendar",
        "live_writeback_counted_sequences",
    ] {
        assert!(
            live_schedule.contains(anchor),
            "live writeback statistics schedule must derive from `{anchor}`"
        );
    }
    assert!(
        !module.contains("fn rebuild_live_writeback_schedule_ownership("),
        "focused writeback authority must not rebuild parallel schedule caches"
    );
    assert!(
        !replan.contains("fn sync_live_memory_result_writeback_owner("),
        "transactional replanning must validate calendar-backed memory results without synchronizing shadow owner fields"
    );
    let memory_result_reservation =
        rust_function_definition(&module, "memory_result_writeback_reservation")
            .expect("focused writeback authority must classify memory-result reservations");
    for anchor in [
        "writeback_calendar",
        "O3LiveWritebackReadySource::MemoryResult",
    ] {
        assert!(
            memory_result_reservation.contains(anchor),
            "memory-result writeback lookup must consume `{anchor}`"
        );
    }
    let publication_tick = rust_function_definition(&memory, "live_data_access_publication_tick")
        .expect("live data access publication must derive timing from writeback authority");
    for anchor in ["memory_result_writeback_reservation", "response_tick"] {
        assert!(
            publication_tick.contains(anchor),
            "live data access publication timing must consume `{anchor}`"
        );
    }
    for function in [
        "ready_live_data_access_completion_timing",
        "take_ready_live_data_access_event",
        "live_data_access_publication_is_admitted",
    ] {
        let definition = rust_function_definition(&memory, function)
            .unwrap_or_else(|| panic!("missing live data access timing consumer `{function}`"));
        assert!(
            definition.contains("live_data_access_publication_tick"),
            "`{function}` must consume the derived live data access publication tick"
        );
    }

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
        "fn live_writeback_schedule(",
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
        "fn record_live_speculative_execution",
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
        "src/o3_runtime_issue/queue.rs",
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
    let issue = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/o3_runtime_issue.rs")).unwrap(),
    );
    for opcode in opcode_inventory {
        assert!(
            !issue.contains(opcode),
            "src/o3_runtime_issue.rs duplicates live-control opcode inventory `{opcode}`"
        );
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
    let nonadjacent_target_path = crate_dir
        .join("src/o3_runtime_control_window_tests/producer_forwarded_target/nonadjacent.rs");
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
    assert_eq!(
        path_owned_module_declaration_count(
            &producer_forwarded_target,
            "producer_forwarded_target/nonadjacent.rs",
            "nonadjacent",
        ),
        1,
        "non-adjacent producer-forwarded target tests need one focused child"
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

    assert!(nonadjacent_target_path.exists());
    let nonadjacent_target = fs::read_to_string(&nonadjacent_target_path).unwrap();
    let nonadjacent_target_code = rust_code_without_comments_and_literals(&nonadjacent_target);
    assert!(include_macro_lines(&nonadjacent_target).is_empty());
    assert!(
        line_count(&nonadjacent_target_path)
            <= MAX_O3_RUNTIME_CONTROL_WINDOW_NONADJACENT_TARGET_TEST_LINES,
        "producer_forwarded_target/nonadjacent.rs exceeds {MAX_O3_RUNTIME_CONTROL_WINDOW_NONADJACENT_TARGET_TEST_LINES} lines"
    );
    for anchor in [
        "nonadjacent_no_link_and_split_link_controls_use_exact_dependency_producer",
        "nonadjacent_control_rejects_missing_or_ambiguous_dependency",
        "recorded_nonadjacent_target_revalidates_after_data_head_retire",
    ] {
        assert!(
            production_defines_exact_function(&nonadjacent_target_code, anchor),
            "nonadjacent.rs is missing `{anchor}`"
        );
        assert!(
            !production_defines_exact_function(&producer_forwarded_target_code, anchor),
            "producer_forwarded_target.rs still owns `{anchor}`"
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
fn producer_forwarded_test_helpers_live_only_in_test_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = [
        "retire_producer_forwarded_data_head_for_test",
        "producer_forwarded_scalar_return_issue_tick_for_test",
        "replace_producer_forwarded_chain_fetch_identity_for_test",
        "repeated_last_for_test",
    ];
    let mut offenders = Vec::new();

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let code = rust_code_without_comments_and_literals(&source);
        for helper in forbidden {
            if production_defines_exact_function(&code, helper) {
                offenders.push(format!("{} defines {helper}(", relative.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "producer-forwarded test helpers must live in test-only modules, not production source files: {}",
        offenders.join(", ")
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
    let restricted = "#[path = \"identity.rs\"]\npub(in crate::runtime) mod identity;\n";
    let shadow = "#[path = \"identity.rs\"]\nmod identity_shadow;\n";
    let duplicate = format!("{live}{shadow}");
    let commented = "/* #[path = \"identity.rs\"]\nmod identity; */\n";
    let detached = "#[path = \"identity.rs\"]\nconst MARKER: () = ();\nmod identity;\n";

    assert_eq!(
        path_owned_module_declaration_count(live, "identity.rs", "identity"),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(restricted, "identity.rs", "identity"),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(&duplicate, "identity.rs", "identity"),
        1
    );
    assert_eq!(
        path_owned_module_path_declaration_count(&duplicate, "identity.rs"),
        2
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
    assert_eq!(path_attribute_lines(live), [1]);
    assert!(path_attribute_lines(commented).is_empty());
    assert_eq!(external_module_declaration_lines(live), [2]);
    assert_eq!(
        external_module_declaration_lines("pub(crate) mod child;\n"),
        [1]
    );
    assert!(external_module_declaration_lines("mod inline {}\n").is_empty());
    assert!(external_module_declaration_lines("// mod child;\n").is_empty());
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

    let queue = fs::read_to_string(crate_dir.join("src/o3_runtime_issue/queue.rs")).unwrap();
    let issue =
        rust_function_definition(&queue, "live_issue_scheduling_candidate_from_instruction")
            .expect("O3 live issue queue authority must classify scheduling candidates");
    assert!(
        issue.contains("pending_control_sequence"),
        "O3 issue dependencies must consume only pending control lineage"
    );
}

#[test]
fn branch_predictor_legacy_checkpoints_use_frozen_payloads() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/branch_predictor.rs");
    let fixture_path = crate_dir.join("tests/branch_predictor/legacy_checkpoint_fixtures.rs");
    assert!(
        fixture_path.exists(),
        "legacy branch-predictor checkpoint bytes belong in a focused fixture module"
    );

    let root = fs::read_to_string(&root_path).unwrap();
    let code = rust_code_without_comments_and_literals(&root);
    for forbidden in [
        "fn current_payload_prefix_without_btb_kind_counters",
        "const VERSION_OFFSET",
        "const ACTIVE_SPECULATION_V2_BYTES",
        "const ACTIVE_SPECULATION_V3_BYTES",
        "const ACTIVE_SPECULATION_V4_BYTES",
        "v1_encoded",
        "v2_encoded",
        "v3_encoded",
        "v4_encoded",
        "v5_encoded",
    ] {
        assert!(
            !code.contains(forbidden),
            "valid legacy branch-predictor compatibility must not regenerate retired schema code `{forbidden}`"
        );
    }

    assert!(
        root.contains("mod legacy_checkpoint_fixtures;"),
        "branch_predictor.rs must import the focused legacy fixture module"
    );
    let fixtures = fs::read_to_string(&fixture_path).unwrap();
    let fixtures_code = rust_code_without_comments_and_literals(&fixtures);
    for forbidden in [
        "fn ",
        "include!",
        "include_bytes!",
        "concat!",
        "Vec::",
        ".encode(",
        ".decode(",
        ".to_vec(",
    ] {
        assert!(
            !fixtures_code.contains(forbidden),
            "legacy branch-predictor fixture module must contain literal constants, not `{forbidden}`"
        );
    }
    for required in [
        "LEGACY_V1_DEFAULT_PAYLOAD",
        "LEGACY_V2_ACTIVE_MAPPING_PAYLOAD",
        "LEGACY_V3_TARGET_PREDICTION_PAYLOAD",
        "LEGACY_V4_RAS_PAYLOAD",
        "LEGACY_V5_BRANCH_KIND_PAYLOAD",
    ] {
        assert!(
            fixtures_code.contains(&format!("pub(super) const {required}: &[u8] = &[")),
            "legacy branch-predictor fixture module is missing `{required}`"
        );
        assert!(
            root.contains(required),
            "branch predictor compatibility tests must consume `{required}`"
        );
    }

    for (test, next_test, fixture) in [
        (
            "checkpoint_payload_decodes_v4_active_mapping_with_ras_without_branch_kinds",
            "checkpoint_payload_decodes_v2_active_mapping_without_branch_target_predictions",
            "LEGACY_V4_RAS_PAYLOAD",
        ),
        (
            "checkpoint_payload_decodes_v2_active_mapping_without_branch_target_predictions",
            "checkpoint_payload_decodes_v3_active_mapping_with_branch_target_predictions_without_ras",
            "LEGACY_V2_ACTIVE_MAPPING_PAYLOAD",
        ),
        (
            "checkpoint_payload_decodes_v3_active_mapping_with_branch_target_predictions_without_ras",
            "checkpoint_payload_decodes_legacy_v1_with_default_btb_snapshot",
            "LEGACY_V3_TARGET_PREDICTION_PAYLOAD",
        ),
        (
            "checkpoint_payload_decodes_legacy_v1_with_default_btb_snapshot",
            "checkpoint_payload_decodes_v5_active_mapping_with_branch_kinds_without_btb_kind_counters",
            "LEGACY_V1_DEFAULT_PAYLOAD",
        ),
        (
            "checkpoint_payload_decodes_v5_active_mapping_with_branch_kinds_without_btb_kind_counters",
            "checkpoint_payload_rejects_btb_config_outside_decode_limits",
            "LEGACY_V5_BRANCH_KIND_PAYLOAD",
        ),
    ] {
        let body = source_section(
            &root,
            &format!("fn {test}()"),
            &format!("fn {next_test}()"),
        );
        let body_code = rust_code_without_comments_and_literals(body);
        let decode = format!("BranchPredictorCheckpointPayload::decode({fixture})");
        assert!(
            body_code.contains(&decode),
            "legacy branch-predictor test `{test}` must decode `{fixture}` directly"
        );
        assert_eq!(
            body_code
                .matches("BranchPredictorCheckpointPayload::decode(")
                .count(),
            1,
            "legacy branch-predictor test `{test}` must have exactly one payload decode"
        );
        assert_eq!(
            body_code.matches(fixture).count(),
            2,
            "legacy branch-predictor test `{test}` may use `{fixture}` only for direct decode and migration verification"
        );
        for forbidden in [
            ".encode()",
            ".truncate(",
            ".extend_from_slice(",
            ".to_vec()",
            "Vec::from(",
            "Vec::new(",
            "Vec::with_capacity(",
            "vec![",
            ".push(",
            ".resize(",
            ".resize_with(",
            ".copy_from_slice(",
            ".clone_from_slice(",
            ".copy_within(",
            ".fill(",
            ".splice(",
            ".insert(",
            ".append(",
            ".as_mut_slice(",
            ".get_mut(",
            ".iter_mut(",
            "] =",
            "]=",
        ] {
            assert!(
                !body_code.contains(forbidden),
                "legacy branch-predictor test `{test}` must not synthesize valid retired bytes with `{forbidden}`"
            );
        }
    }
}

fn production_defines_exact_function(source: &str, name: &str) -> bool {
    rust_function_definition_count(source, name) > 0
}

fn rust_function_definition_count(source: &str, name: &str) -> usize {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut count = 0;
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
                        count += 1;
                    }
                }
            }
        }
        index = end;
    }
    count
}

fn rust_test_function_definition_count(source: &str, name: &str) -> usize {
    let code = rust_code_without_comments_and_literals(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut count = 0;
    while index < chars.len() {
        if chars.get(index) != Some(&'#') {
            index += 1;
            continue;
        }
        let attribute_start = skip_rust_whitespace(&chars, index + 1);
        if chars.get(attribute_start) != Some(&'[') {
            index += 1;
            continue;
        }
        let Some(attribute_end) = matching_delimiter(&chars, attribute_start, '[', ']') else {
            break;
        };
        let attribute = chars[attribute_start + 1..attribute_end]
            .iter()
            .collect::<String>();
        if attribute.trim() == "test" {
            let mut item_start = attribute_end + 1;
            loop {
                item_start = skip_rust_whitespace(&chars, item_start);
                if chars.get(item_start) != Some(&'#') {
                    break;
                }
                let next_attribute = skip_rust_whitespace(&chars, item_start + 1);
                if chars.get(next_attribute) != Some(&'[') {
                    break;
                }
                let Some(next_attribute_end) = matching_delimiter(&chars, next_attribute, '[', ']')
                else {
                    break;
                };
                item_start = next_attribute_end + 1;
            }

            item_start = skip_rust_whitespace(&chars, item_start);
            if let Some((visibility, visibility_end)) = rust_identifier_at(&chars, item_start) {
                if visibility == "pub" {
                    item_start = skip_rust_whitespace(&chars, visibility_end);
                    if chars.get(item_start) == Some(&'(') {
                        let Some(visibility_close) =
                            matching_delimiter(&chars, item_start, '(', ')')
                        else {
                            break;
                        };
                        item_start = skip_rust_whitespace(&chars, visibility_close + 1);
                    }
                }
            }
            loop {
                let Some((qualifier, qualifier_end)) = rust_identifier_at(&chars, item_start)
                else {
                    break;
                };
                if matches!(qualifier.as_str(), "async" | "unsafe") {
                    item_start = skip_rust_whitespace(&chars, qualifier_end);
                    continue;
                }
                if qualifier == "fn" {
                    let name_start = skip_rust_whitespace(&chars, qualifier_end);
                    if let Some((function_name, name_end)) = rust_identifier_at(&chars, name_start)
                    {
                        let after_name = skip_rust_whitespace(&chars, name_end);
                        if function_name == name && matches!(chars.get(after_name), Some('(' | '<'))
                        {
                            count += 1;
                        }
                    }
                }
                break;
            }
        }
        index = attribute_end + 1;
    }
    count
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

fn production_definition_named_type_storage_count(definition: &str, name: &str) -> usize {
    let code = production_rust_source(definition);
    let chars = code.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut definition_kind = None;
    let mut definition_name_end = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if matches!(identifier.as_str(), "struct" | "enum" | "type") {
            let name_start = skip_rust_whitespace(&chars, end);
            let Some((_, name_end)) = rust_identifier_at(&chars, name_start) else {
                return 0;
            };
            definition_kind = Some(identifier);
            definition_name_end = name_end;
            break;
        }
        index = end;
    }
    let Some(definition_kind) = definition_kind else {
        return 0;
    };
    let storage_start = chars[definition_name_end..]
        .iter()
        .position(|character| match definition_kind.as_str() {
            "struct" => matches!(character, '{' | '('),
            "enum" => *character == '{',
            "type" => *character == '=',
            _ => false,
        })
        .map(|offset| definition_name_end + offset + 1);
    let Some(mut index) = storage_start else {
        return 0;
    };
    let mut count = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        count += usize::from(identifier == name);
        index = end;
    }
    count
}

fn production_struct_named_type_storage(
    production: &[(PathBuf, String)],
    name: &str,
) -> Vec<(PathBuf, usize)> {
    production
        .iter()
        .filter_map(|(path, source)| {
            let storage_count = production_struct_definitions(source)
                .iter()
                .map(|definition| production_definition_named_type_storage_count(definition, name))
                .sum();
            (storage_count > 0).then(|| (path.clone(), storage_count))
        })
        .collect()
}

fn production_renamed_use_aliases(source: &str, protected_names: &[&str]) -> Vec<String> {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut aliases = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier != "use" {
            index = end;
            continue;
        }
        let Some(statement_end) = chars
            .iter()
            .enumerate()
            .skip(end)
            .find_map(|(candidate, character)| (*character == ';').then_some(candidate))
        else {
            break;
        };
        let mut import_index = end;
        while import_index < statement_end {
            let Some((imported, imported_end)) = rust_identifier_at(&chars, import_index) else {
                import_index += 1;
                continue;
            };
            if protected_names.contains(&imported.as_str()) {
                let as_start = skip_rust_whitespace(&chars, imported_end);
                if let Some((as_keyword, as_end)) = rust_identifier_at(&chars, as_start) {
                    if as_keyword == "as" {
                        let alias_start = skip_rust_whitespace(&chars, as_end);
                        if let Some((alias, alias_end)) = rust_identifier_at(&chars, alias_start) {
                            aliases.push(format!("{imported} as {alias}"));
                            import_index = alias_end;
                            continue;
                        }
                    }
                }
            }
            import_index = imported_end;
        }
        index = statement_end + 1;
    }
    aliases
}

fn production_named_const_declaration_count(source: &str, name: &str) -> usize {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut count = 0;
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        if identifier == "const" {
            let mut previous = index;
            while previous > 0 && chars[previous - 1].is_whitespace() {
                previous -= 1;
            }
            let const_generic = previous > 0 && matches!(chars.get(previous - 1), Some('<' | ','));
            if !const_generic {
                let name_start = skip_rust_whitespace(&chars, end);
                if let Some((const_name, name_end)) = rust_identifier_at(&chars, name_start) {
                    let declaration = skip_rust_whitespace(&chars, name_end);
                    if const_name == name && matches!(chars.get(declaration), Some(':' | '=' | ';'))
                    {
                        count += 1;
                    }
                }
            }
        }
        index = end;
    }
    count
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

fn production_enum_definitions(source: &str) -> Vec<String> {
    let code = production_rust_source(source);
    let chars = code.chars().collect::<Vec<_>>();
    let mut definitions = Vec::new();
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
        let Some((_, name_end)) = rust_identifier_at(&chars, name_start) else {
            index = end;
            continue;
        };
        let mut body = skip_rust_whitespace(&chars, name_end);
        if chars.get(body) == Some(&'<') {
            let Some(close) = matching_delimiter(&chars, body, '<', '>') else {
                index = name_end;
                continue;
            };
            body = close + 1;
        }
        while body < chars.len() && chars[body] != '{' {
            if chars[body] == ';' {
                break;
            }
            body += 1;
        }
        if chars.get(body) != Some(&'{') {
            index = body.saturating_add(1);
            continue;
        }
        let Some(close) = matching_delimiter(&chars, body, '{', '}') else {
            index = body + 1;
            continue;
        };
        definitions.push(chars[index..=close].iter().collect());
        index = close + 1;
    }
    definitions
}

fn production_enum_definition(source: &str, name: &str) -> Option<String> {
    production_enum_definitions(source)
        .into_iter()
        .find(|definition| production_defines_exact_named_item(definition, "enum", name))
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
    let root_path = crate_dir.join("src/riscv_execution_mode_handoff.rs");
    let root = fs::read_to_string(&root_path).unwrap();
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

    for path in module_family_rust_source_files(&root_path) {
        let source = fs::read_to_string(&path).unwrap();
        let code = rust_code_without_comments_and_literals(&source);
        for forbidden in ["fn encode_legacy", "fn write_legacy", "_legacy_for_test"] {
            assert!(
                !code.contains(forbidden),
                "RISC-V live-data handoff compatibility must use frozen decode fixtures instead of retired-schema writer `{forbidden}` in {}",
                path.display()
            );
        }
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

    let tests = r#"
#[test]
fn focused_test() {}

fn detached_test() {}

#[test]
#[cfg(target_pointer_width = "64")]
fn attributed_test() {}
"#;
    assert_eq!(
        rust_test_function_definition_count(tests, "focused_test"),
        1
    );
    assert_eq!(
        rust_test_function_definition_count(tests, "detached_test"),
        0
    );
    assert_eq!(
        rust_test_function_definition_count(tests, "attributed_test"),
        1
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
fn task8_collection_storage_scan_rejects_nested_parallel_field() {
    let runtime_path = PathBuf::from("src/o3_runtime.rs");
    let mutation = vec![(
        runtime_path.clone(),
        production_rust_source(
            r#"
struct O3RuntimeState {
    pending_data_addresses: O3PendingDataAddresses,
}

mod mutation {
    struct ShadowRuntime {
        shadow_pending: O3PendingDataAddresses,
    }
}
"#,
        ),
    )];

    let storage = production_struct_named_type_storage(&mutation, "O3PendingDataAddresses");
    assert_eq!(storage, [(runtime_path.clone(), 2)]);
    assert_ne!(
        storage,
        [(runtime_path, 1)],
        "nested parallel collection storage must violate canonical runtime ownership"
    );
}

#[test]
fn task8_renamed_pending_address_import_mutation_is_rejected() {
    let mutation = r#"
mod mutation {
    use super::O3PendingDataAddresses as PendingSet;
    use super::{
        O3PendingDataAddress as PendingRow,
        O3_PENDING_DATA_ADDRESS_CAPACITY as PendingCapacity,
    };

    struct Shadow {
        rows: PendingSet,
    }
}

// use super::O3PendingDataAddresses as CommentedPendingSet;
const IMPORT_TEXT: &str = "use super::O3PendingDataAddresses as StringPendingSet;";

#[cfg(test)]
mod tests {
    use super::O3PendingDataAddresses as TestPendingSet;
}
"#;

    let aliases = production_renamed_use_aliases(
        mutation,
        &[
            "O3PendingDataAddress",
            "O3PendingDataAddresses",
            "O3_PENDING_DATA_ADDRESS_CAPACITY",
        ],
    );
    assert_eq!(
        aliases,
        [
            "O3PendingDataAddresses as PendingSet",
            "O3PendingDataAddress as PendingRow",
            "O3_PENDING_DATA_ADDRESS_CAPACITY as PendingCapacity",
        ]
    );
    assert!(
        !aliases.is_empty(),
        "renamed protected imports must violate compatibility-alias ownership"
    );
}

#[test]
fn task8_duplicate_capacity_const_mutation_is_rejected() {
    let mutation = r#"
pub(super) const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 2;

mod mutation {
    const O3_PENDING_DATA_ADDRESS_CAPACITY
        : usize = 3;
}

struct ConstGeneric<const O3_PENDING_DATA_ADDRESS_CAPACITY: usize>;
// const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 4;
const CAPACITY_TEXT: &str = "const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 5;";

#[cfg(test)]
const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 6;
"#;

    let declaration_count =
        production_named_const_declaration_count(mutation, "O3_PENDING_DATA_ADDRESS_CAPACITY");
    assert_eq!(declaration_count, 2);
    assert_ne!(
        declaration_count, 1,
        "nested duplicate capacity declarations must violate canonical ownership"
    );
}

#[test]
fn source_policy_helper_detects_exact_named_type_storage_without_request_false_positives() {
    for definition in [
        "struct Alternate { row: O3PendingDataAddress }",
        "struct Alternate(O3PendingDataAddress);",
        "struct Alternate { rows: Vec<O3PendingDataAddress> }",
        "struct Alternate { rows: VecDeque<O3PendingDataAddress> }",
        "struct Alternate { rows: [O3PendingDataAddress; 2] }",
        "struct Alternate { row: Option<O3PendingDataAddress> }",
        "enum Alternate { Row(O3PendingDataAddress) }",
        "type Alternate = Vec<O3PendingDataAddress>;",
    ] {
        assert_eq!(
            production_definition_named_type_storage_count(definition, "O3PendingDataAddress"),
            1,
            "failed to detect exact pending-row storage in `{definition}`"
        );
    }
    assert_eq!(
        production_definition_named_type_storage_count(
            "enum Alternate { First(O3PendingDataAddress), Second(O3PendingDataAddress) }",
            "O3PendingDataAddress"
        ),
        2
    );
    for definition in [
        "struct O3PendingDataAddress { sequence: u64 }",
        "struct O3PendingDataAddressRequest { sequence: u64 }",
        "type Requests = Vec<O3PendingDataAddressRequest>;",
    ] {
        assert_eq!(
            production_definition_named_type_storage_count(definition, "O3PendingDataAddress"),
            0,
            "request or row declarations must not count as alternate storage: `{definition}`"
        );
    }
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
    rust_identifier_count(chars, needle) != 0
}

fn rust_identifier_count(chars: &[char], needle: &str) -> usize {
    let mut index = 0;
    let mut count = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(chars, index) else {
            index += 1;
            continue;
        };
        if identifier == needle {
            count += 1;
        }
        index = end;
    }
    count
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

fn path_attribute_lines(source: &str) -> Vec<usize> {
    rust_code_without_comments_and_literals(source)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| line.trim_start().starts_with("#[path").then_some(index + 1))
        .collect()
}

fn external_module_declaration_lines(source: &str) -> Vec<usize> {
    rust_code_without_comments_and_literals(source)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.trim();
            let is_external_mod = line.ends_with(';')
                && (line.starts_with("mod ")
                    || line.starts_with("pub mod ")
                    || line.starts_with("pub(crate) mod ")
                    || line.starts_with("pub(super) mod ")
                    || (line.starts_with("pub(in ") && line.contains(" mod ")));
            is_external_mod.then_some(index + 1)
        })
        .collect()
}

fn path_owned_module_path_declaration_count(source: &str, path: &str) -> usize {
    path_owned_module_declarations(source, path).len()
}

fn path_owned_module_declaration_count(source: &str, path: &str, module: &str) -> usize {
    path_owned_module_declarations(source, path)
        .into_iter()
        .filter(|declared| declared == module)
        .count()
}

fn path_owned_module_declarations(source: &str, path: &str) -> Vec<String> {
    let code = rust_code_without_comments_and_literals(source);
    let source_lines = source.lines().collect::<Vec<_>>();
    let code_lines = code.lines().collect::<Vec<_>>();
    let attribute = format!("#[path = \"{path}\"]");

    source_lines
        .iter()
        .zip(&code_lines)
        .enumerate()
        .filter_map(|(index, (source_line, code_line))| {
            (source_line.trim() == attribute && code_line.trim_start().starts_with("#[path"))
                .then(|| {
                    code_lines
                        .iter()
                        .skip(index + 1)
                        .find(|line| !line.trim().is_empty())
                        .and_then(|line| external_module_declaration_name(line))
                        .map(str::to_owned)
                })
                .flatten()
        })
        .collect()
}

fn external_module_declaration_name(line: &str) -> Option<&str> {
    let line = line.trim();
    let declaration = if let Some(declaration) = line.strip_prefix("mod ") {
        declaration
    } else if let Some(declaration) = line.strip_prefix("pub mod ") {
        declaration
    } else if let Some(declaration) = line.strip_prefix("pub(crate) mod ") {
        declaration
    } else if let Some(declaration) = line.strip_prefix("pub(super) mod ") {
        declaration
    } else {
        let restricted = line.strip_prefix("pub(in ")?;
        let visibility_end = restricted.find(')')?;
        restricted[visibility_end + 1..]
            .trim_start()
            .strip_prefix("mod ")?
    };
    let name = declaration.strip_suffix(';')?.trim();
    let chars = name.chars().collect::<Vec<_>>();
    let (_, end) = rust_identifier_at(&chars, 0)?;
    (end == chars.len()).then_some(name)
}

fn top_level_external_module_declaration_count(source: &str, module: &str) -> usize {
    let code = rust_code_without_comments_and_literals(source);
    let mut depth = 0_i64;
    code.lines()
        .filter(|line| {
            let top_level = depth == 0;
            let matches = top_level && external_module_declaration_name(line) == Some(module);
            for byte in line.bytes() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
            }
            matches
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
