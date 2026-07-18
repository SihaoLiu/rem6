use std::{fs, path::Path};

use super::{line_count, module_has_path_attribute, rust_function_definition_names};

const MAX_DEBUG_FLAGS_ROOT_LINES: usize = 11_300;
const MAX_DEBUG_FLAGS_HOST_ACTION_LINES: usize = 800;
const MAX_DEBUG_FLAGS_O3_CHECKPOINT_RESTORE_LINES: usize = 1_100;
const MAX_DEBUG_FLAGS_O3_FU_LATENCY_LINES: usize = 1_700;
const MAX_DEBUG_FLAGS_O3_LSQ_OPERATIONS_LINES: usize = 950;
const MAX_DEBUG_FLAGS_O3_STORE_FORWARDING_LINES: usize = 700;
const DEBUG_FLAGS_MODULES: [(&str, &str); 5] = [
    ("host_action", "debug_flags/host_action.rs"),
    (
        "o3_checkpoint_restore",
        "debug_flags/o3_checkpoint_restore.rs",
    ),
    ("o3_fu_latency", "debug_flags/o3_fu_latency.rs"),
    ("o3_lsq_operations", "debug_flags/o3_lsq_operations.rs"),
    ("o3_store_forwarding", "debug_flags/o3_store_forwarding.rs"),
];
const HOST_ACTION_TESTS: [&str; 4] = [
    "rem6_run_host_action_debug_flag_emits_real_m5_host_action_trace",
    "rem6_run_host_action_debug_flag_emits_m5_hypercall_checkpoint_and_switch_trace",
    "rem6_run_host_action_debug_flag_emits_checker_quiescence_switch_scope",
    "rem6_run_host_action_debug_flag_emits_scheduled_checkpoint_restore_trace",
];
const O3_CHECKPOINT_RESTORE_TESTS: [&str; 4] = [
    "rem6_run_o3_debug_flag_marks_checkpoint_restore_replay_scope",
    "rem6_run_o3_debug_flag_counts_multiple_checkpoint_restore_scopes",
    "rem6_run_o3_debug_flag_tracks_distinct_checkpoint_restore_labels",
    "rem6_run_o3_debug_flag_scopes_multicore_checkpoint_restore_traces",
];
const O3_CHECKPOINT_RESTORE_DEFINITIONS: [&str; 6] = [
    "o3_runtime_restore_component_and_chunk",
    "assert_o3_restore_scope_stats_match_latest",
    "rem6_run_o3_debug_flag_marks_checkpoint_restore_replay_scope",
    "rem6_run_o3_debug_flag_counts_multiple_checkpoint_restore_scopes",
    "rem6_run_o3_debug_flag_tracks_distinct_checkpoint_restore_labels",
    "rem6_run_o3_debug_flag_scopes_multicore_checkpoint_restore_traces",
];
const O3_FU_LATENCY_DEFINITIONS: [&str; 18] = [
    "rem6_run_o3_debug_flag_emits_fu_latency_event_classes",
    "rem6_run_o3_debug_flag_classifies_vector_integer_fu_latency_events",
    "rem6_run_o3_debug_flag_classifies_vector_mul_family_fu_latency_events",
    "rem6_run_o3_debug_flag_classifies_vector_saturating_mul_fu_latency_events",
    "rem6_run_o3_debug_flag_classifies_float_fu_latency_events",
    "rem6_run_o3_debug_flag_classifies_extended_float_fu_latency_events",
    "rem6_run_o3_debug_flag_classifies_float_compare_fu_latency_events",
    "rem6_run_o3_debug_flag_classifies_float_misc_fu_latency_events",
    "detailed_o3_fu_latency_debug_binary",
    "detailed_o3_vector_fu_latency_debug_binary",
    "detailed_o3_vector_mul_family_fu_latency_debug_binary",
    "detailed_o3_vector_saturating_mul_fu_latency_debug_binary",
    "detailed_o3_float_fu_latency_debug_binary",
    "detailed_o3_float_extended_fu_latency_debug_binary",
    "detailed_o3_float_compare_fu_latency_debug_binary",
    "detailed_o3_float_misc_fu_latency_debug_binary",
    "assert_o3_event_with_fu",
    "o3_event_fu_latency_class_count",
];
const O3_LSQ_OPERATIONS_TESTS: [&str; 5] = [
    "rem6_run_o3_debug_flag_emits_vector_lsq_byte_events",
    "rem6_run_o3_debug_flag_classifies_float_lsq_operation_shape",
    "rem6_run_o3_debug_flag_classifies_atomic_lsq_operation_shape",
    "rem6_run_o3_debug_flag_classifies_lsq_memory_ordering",
    "rem6_run_o3_debug_flag_marks_store_conditional_failures",
];
const O3_LSQ_OPERATIONS_DEFINITIONS: [&str; 10] = [
    "detailed_o3_vector_memory_debug_binary",
    "detailed_o3_float_memory_debug_binary",
    "detailed_o3_atomic_lsq_debug_binary",
    "detailed_o3_ordered_atomic_lsq_debug_binary",
    "detailed_o3_store_conditional_failure_debug_binary",
    "rem6_run_o3_debug_flag_emits_vector_lsq_byte_events",
    "rem6_run_o3_debug_flag_classifies_float_lsq_operation_shape",
    "rem6_run_o3_debug_flag_classifies_atomic_lsq_operation_shape",
    "rem6_run_o3_debug_flag_classifies_lsq_memory_ordering",
    "rem6_run_o3_debug_flag_marks_store_conditional_failures",
];
const O3_STORE_FORWARDING_TESTS: [&str; 2] = [
    "rem6_run_o3_debug_flag_emits_store_forwarding_events",
    "rem6_run_o3_debug_flag_classifies_store_forwarding_relations",
];
const O3_STORE_FORWARDING_DEFINITIONS: [&str; 8] = [
    "detailed_o3_store_forwarding_debug_binary",
    "detailed_o3_store_forwarding_suppression_debug_binary",
    "detailed_o3_store_forwarding_mismatch_debug_binary",
    "detailed_o3_store_forwarding_partial_overlap_debug_binary",
    "detailed_o3_store_forwarding_address_mismatch_byte_load_debug_binary",
    "assert_o3_event_with_store_forwarding",
    "rem6_run_o3_debug_flag_emits_store_forwarding_events",
    "rem6_run_o3_debug_flag_classifies_store_forwarding_relations",
];

#[test]
fn debug_flags_root_keeps_current_ratchet() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/debug_flags.rs");
    let source = fs::read_to_string(&path).unwrap();
    let syntax = parsed_source("tests/cli_run/debug_flags.rs", &source);
    let lines = line_count(&path);

    assert!(
        lines <= MAX_DEBUG_FLAGS_ROOT_LINES,
        "tests/cli_run/debug_flags.rs exceeds the current decomposition ratchet: {lines} lines"
    );
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "tests/cli_run/debug_flags.rs must not inline child sources with include!: {includes:?}"
    );
    let modules = syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Mod(module) = item else {
                return None;
            };
            Some(module.ident.to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        modules,
        DEBUG_FLAGS_MODULES
            .iter()
            .map(|(module, _)| module.to_string())
            .collect::<Vec<_>>(),
        "tests/cli_run/debug_flags.rs must declare exactly the focused child modules in order"
    );
    for (module, module_path) in DEBUG_FLAGS_MODULES {
        assert!(
            module_has_path_attribute(&source, module, module_path),
            "tests/cli_run/debug_flags.rs must declare `{module}` from `{module_path}`"
        );
    }
}

#[test]
fn debug_flags_host_action_tests_live_in_focused_module() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = manifest_dir.join("tests/cli_run/debug_flags.rs");
    let child_path = manifest_dir.join("tests/cli_run/debug_flags/host_action.rs");
    let root = fs::read_to_string(root_path).unwrap();
    let root_functions = rust_function_definition_names(&root);

    assert!(
        child_path.exists(),
        "HostAction debug evidence belongs in tests/cli_run/debug_flags/host_action.rs"
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let child_lines = line_count(&child_path);
    assert!(
        child_lines <= MAX_DEBUG_FLAGS_HOST_ACTION_LINES,
        "tests/cli_run/debug_flags/host_action.rs exceeds {MAX_DEBUG_FLAGS_HOST_ACTION_LINES} lines: {child_lines}"
    );
    let syntax = parsed_source("tests/cli_run/debug_flags/host_action.rs", &child);
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "tests/cli_run/debug_flags/host_action.rs must not inline sources with include!: {includes:?}"
    );
    assert!(
        focused_module_has_only_super_glob_use_and_functions(&syntax),
        "tests/cli_run/debug_flags/host_action.rs must contain only `use super::*;` and its exact test functions"
    );
    let expected = HOST_ACTION_TESTS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        top_level_function_names(&syntax),
        expected,
        "tests/cli_run/debug_flags/host_action.rs must own exactly the HostAction debug tests"
    );
    assert_eq!(
        top_level_test_function_names(&syntax),
        expected,
        "every function in tests/cli_run/debug_flags/host_action.rs must remain a test"
    );
    for function in HOST_ACTION_TESTS {
        assert!(
            !root_functions.contains(function),
            "tests/cli_run/debug_flags.rs still owns `fn {function}`"
        );
    }
}

#[test]
fn debug_flags_o3_checkpoint_restore_lives_in_focused_module() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = manifest_dir.join("tests/cli_run/debug_flags.rs");
    let child_path = manifest_dir.join("tests/cli_run/debug_flags/o3_checkpoint_restore.rs");
    let root = fs::read_to_string(root_path).unwrap();
    let root_functions = rust_function_definition_names(&root);

    assert!(
        child_path.exists(),
        "O3 checkpoint/restore debug evidence belongs in tests/cli_run/debug_flags/o3_checkpoint_restore.rs"
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let child_lines = line_count(&child_path);
    assert!(
        child_lines <= MAX_DEBUG_FLAGS_O3_CHECKPOINT_RESTORE_LINES,
        "tests/cli_run/debug_flags/o3_checkpoint_restore.rs exceeds {MAX_DEBUG_FLAGS_O3_CHECKPOINT_RESTORE_LINES} lines: {child_lines}"
    );
    let syntax = parsed_source("tests/cli_run/debug_flags/o3_checkpoint_restore.rs", &child);
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "tests/cli_run/debug_flags/o3_checkpoint_restore.rs must not inline sources with include!: {includes:?}"
    );
    assert!(
        focused_module_has_only_super_glob_use_and_functions(&syntax),
        "tests/cli_run/debug_flags/o3_checkpoint_restore.rs must contain only `use super::*;` and its exact helper/test functions"
    );
    let expected_definitions = O3_CHECKPOINT_RESTORE_DEFINITIONS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    let expected_tests = O3_CHECKPOINT_RESTORE_TESTS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        top_level_function_names(&syntax),
        expected_definitions,
        "tests/cli_run/debug_flags/o3_checkpoint_restore.rs must own exactly the O3 checkpoint/restore helpers and tests"
    );
    assert_eq!(
        top_level_test_function_names(&syntax),
        expected_tests,
        "tests/cli_run/debug_flags/o3_checkpoint_restore.rs must own exactly the O3 checkpoint/restore tests"
    );
    for function in O3_CHECKPOINT_RESTORE_DEFINITIONS {
        assert!(
            !root_functions.contains(function),
            "tests/cli_run/debug_flags.rs still owns `fn {function}`"
        );
    }
}

#[test]
fn debug_flags_o3_fu_latency_lives_in_focused_module() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = manifest_dir.join("tests/cli_run/debug_flags.rs");
    let child_path = manifest_dir.join("tests/cli_run/debug_flags/o3_fu_latency.rs");
    let root = fs::read_to_string(root_path).unwrap();
    let root_functions = rust_function_definition_names(&root);

    assert!(
        child_path.exists(),
        "O3 FU-latency debug evidence belongs in tests/cli_run/debug_flags/o3_fu_latency.rs"
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let child_lines = line_count(&child_path);
    assert!(
        child_lines <= MAX_DEBUG_FLAGS_O3_FU_LATENCY_LINES,
        "tests/cli_run/debug_flags/o3_fu_latency.rs exceeds {MAX_DEBUG_FLAGS_O3_FU_LATENCY_LINES} lines: {child_lines}"
    );
    let syntax = parsed_source("tests/cli_run/debug_flags/o3_fu_latency.rs", &child);
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "tests/cli_run/debug_flags/o3_fu_latency.rs must not inline sources with include!: {includes:?}"
    );
    let child_functions = rust_function_definition_names(&child);
    for function in O3_FU_LATENCY_DEFINITIONS {
        assert!(
            child_functions.contains(function),
            "tests/cli_run/debug_flags/o3_fu_latency.rs is missing `fn {function}`"
        );
        assert!(
            !root_functions.contains(function),
            "tests/cli_run/debug_flags.rs still owns `fn {function}`"
        );
    }
    let duplicates = root_functions
        .intersection(&child_functions)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        duplicates.is_empty(),
        "tests/cli_run/debug_flags.rs duplicates child functions: {duplicates:?}"
    );
}

#[test]
fn debug_flags_o3_lsq_operations_live_in_focused_module() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = manifest_dir.join("tests/cli_run/debug_flags.rs");
    let child_path = manifest_dir.join("tests/cli_run/debug_flags/o3_lsq_operations.rs");
    let root = fs::read_to_string(root_path).unwrap();
    let root_functions = rust_function_definition_names(&root);

    assert!(
        child_path.exists(),
        "O3 LSQ operation debug evidence belongs in tests/cli_run/debug_flags/o3_lsq_operations.rs"
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let child_lines = line_count(&child_path);
    assert!(
        child_lines <= MAX_DEBUG_FLAGS_O3_LSQ_OPERATIONS_LINES,
        "tests/cli_run/debug_flags/o3_lsq_operations.rs exceeds {MAX_DEBUG_FLAGS_O3_LSQ_OPERATIONS_LINES} lines: {child_lines}"
    );
    let syntax = parsed_source("tests/cli_run/debug_flags/o3_lsq_operations.rs", &child);
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "tests/cli_run/debug_flags/o3_lsq_operations.rs must not inline sources with include!: {includes:?}"
    );
    assert!(
        focused_module_has_only_super_glob_use_and_functions(&syntax),
        "tests/cli_run/debug_flags/o3_lsq_operations.rs must contain only `use super::*;` and its exact helper/test functions"
    );
    let expected_definitions = O3_LSQ_OPERATIONS_DEFINITIONS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    let expected_tests = O3_LSQ_OPERATIONS_TESTS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        top_level_function_names(&syntax),
        expected_definitions,
        "tests/cli_run/debug_flags/o3_lsq_operations.rs must own exactly the O3 LSQ operation helpers and tests"
    );
    assert_eq!(
        top_level_test_function_names(&syntax),
        expected_tests,
        "tests/cli_run/debug_flags/o3_lsq_operations.rs must own exactly the O3 LSQ operation tests"
    );
    for function in O3_LSQ_OPERATIONS_DEFINITIONS {
        assert!(
            !root_functions.contains(function),
            "tests/cli_run/debug_flags.rs still owns `fn {function}`"
        );
    }
}

#[test]
fn debug_flags_o3_store_forwarding_lives_in_focused_module() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = manifest_dir.join("tests/cli_run/debug_flags.rs");
    let child_path = manifest_dir.join("tests/cli_run/debug_flags/o3_store_forwarding.rs");
    let root = fs::read_to_string(root_path).unwrap();
    let root_functions = rust_function_definition_names(&root);

    assert!(
        child_path.exists(),
        "O3 store-forwarding debug evidence belongs in tests/cli_run/debug_flags/o3_store_forwarding.rs"
    );
    let child = fs::read_to_string(&child_path).unwrap();
    let child_lines = line_count(&child_path);
    assert!(
        child_lines <= MAX_DEBUG_FLAGS_O3_STORE_FORWARDING_LINES,
        "tests/cli_run/debug_flags/o3_store_forwarding.rs exceeds {MAX_DEBUG_FLAGS_O3_STORE_FORWARDING_LINES} lines: {child_lines}"
    );
    let syntax = parsed_source("tests/cli_run/debug_flags/o3_store_forwarding.rs", &child);
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "tests/cli_run/debug_flags/o3_store_forwarding.rs must not inline sources with include!: {includes:?}"
    );
    assert!(
        focused_module_has_only_super_glob_use_and_functions(&syntax),
        "tests/cli_run/debug_flags/o3_store_forwarding.rs must contain only `use super::*;` and its exact helper/test functions"
    );
    let expected_definitions = O3_STORE_FORWARDING_DEFINITIONS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    let expected_tests = O3_STORE_FORWARDING_TESTS
        .iter()
        .map(|function| function.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        top_level_function_names(&syntax),
        expected_definitions,
        "tests/cli_run/debug_flags/o3_store_forwarding.rs must own exactly the O3 store-forwarding helpers and tests"
    );
    assert_eq!(
        top_level_test_function_names(&syntax),
        expected_tests,
        "tests/cli_run/debug_flags/o3_store_forwarding.rs must own exactly the O3 store-forwarding tests"
    );
    for function in O3_STORE_FORWARDING_DEFINITIONS {
        assert!(
            !root_functions.contains(function),
            "tests/cli_run/debug_flags.rs still owns `fn {function}`"
        );
    }
}

#[test]
fn debug_flags_include_policy_rejects_non_literal_top_level_include() {
    let syntax = parsed_source("synthetic.rs", "include!(concat!(\"nested\", \".rs\"));\n");

    assert_eq!(top_level_include_tokens(&syntax).len(), 1);
}

#[test]
fn focused_debug_module_policy_rejects_extra_top_level_items() {
    let exact = parsed_source(
        "exact.rs",
        "use super::*;\nfn helper() {}\n#[test]\nfn row() {}\n",
    );
    let extra = parsed_source(
        "extra.rs",
        "use super::*;\nconst EXTRA: usize = 1;\nfn helper() {}\n#[test]\nfn row() {}\n",
    );

    assert!(focused_module_has_only_super_glob_use_and_functions(&exact));
    assert!(!focused_module_has_only_super_glob_use_and_functions(
        &extra
    ));
}

fn focused_module_has_only_super_glob_use_and_functions(syntax: &syn::File) -> bool {
    let Some((first, remaining)) = syntax.items.split_first() else {
        return false;
    };
    let syn::Item::Use(import) = first else {
        return false;
    };
    let syn::UseTree::Path(path) = &import.tree else {
        return false;
    };

    path.ident == "super"
        && matches!(path.tree.as_ref(), syn::UseTree::Glob(_))
        && remaining
            .iter()
            .all(|item| matches!(item, syn::Item::Fn(_)))
}

fn top_level_function_names(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect()
}

fn top_level_test_function_names(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("test"))
                .then(|| function.sig.ident.to_string())
        })
        .collect()
}

fn top_level_include_tokens(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Macro(item) = item else {
                return None;
            };
            if !item.mac.path.is_ident("include") {
                return None;
            }
            Some(item.mac.tokens.to_string())
        })
        .collect()
}

fn parsed_source(relative: &str, source: &str) -> syn::File {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative} for ownership: {error}"))
}
