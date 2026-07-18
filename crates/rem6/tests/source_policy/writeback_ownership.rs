use super::*;

const WRITEBACK_ROOT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port.rs";
const RESULT_SUPPORT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_support.rs";
const RESULT_CLASSES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs";
const RESULT_CLASSES_OLD_SUPPORT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/support.rs";
const RESULT_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs";
const RESULT_BOUNDARIES_SUPPORT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries/support.rs";
const STORE_CONDITIONAL_RESULT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/store_conditional_result.rs";
const WRITEBACK_ROOT_MODULES: [ExpectedModuleDeclaration; 4] = [
    ExpectedModuleDeclaration {
        name: "result_support",
        path: "writeback_port/result_support.rs",
    },
    ExpectedModuleDeclaration {
        name: "result_classes",
        path: "writeback_port/result_classes.rs",
    },
    ExpectedModuleDeclaration {
        name: "result_boundaries",
        path: "writeback_port/result_boundaries.rs",
    },
    ExpectedModuleDeclaration {
        name: "store_conditional_result",
        path: "writeback_port/store_conditional_result.rs",
    },
];
const RESULT_BOUNDARY_SUPPORT_MODULES: [ExpectedModuleDeclaration; 1] =
    [ExpectedModuleDeclaration {
        name: "support",
        path: "result_boundaries/support.rs",
    }];
const RESULT_CLASS_TEST_PREFIX: &str = "rem6_run_o3_memory_result_writeback_";
const RESULT_CLASS_ANCHORS: [&str; 4] = [
    "rem6_run_o3_memory_result_writeback_matrix_direct",
    "rem6_run_o3_memory_result_writeback_matrix_cache_fabric_dram",
    "rem6_run_o3_memory_result_writeback_width_two_exact_fit",
    "rem6_run_o3_memory_result_writeback_readfile_mmio",
];
const RESULT_BOUNDARY_ANCHORS: [&str; 6] = [
    "rem6_run_o3_memory_result_writeback_rejects_resultless_and_unsupported_shapes",
    "rem6_run_o3_memory_result_writeback_all_inactive_vector_issues_no_request",
    "rem6_run_o3_memory_result_writeback_denied_amo_traps_before_transport",
    "rem6_run_o3_memory_result_writeback_live_checkpoint_rejects",
    "rem6_run_o3_memory_result_writeback_live_mode_switch_rejects",
    "rem6_run_timing_suppresses_o3_memory_result_writeback_surface",
];
const STORE_CONDITIONAL_RESULT_ANCHORS: [&str; 6] = [
    "rem6_run_o3_store_conditional_result_width_one_serializes_direct",
    "rem6_run_o3_store_conditional_result_width_two_exact_fit_direct",
    "rem6_run_o3_store_conditional_result_cache_fabric_dram",
    "rem6_run_o3_store_conditional_failure_is_local_and_deferred",
    "rem6_run_o3_store_conditional_result_live_actions_reject",
    "rem6_run_timing_suppresses_o3_store_conditional_result_surface",
];
const RESULT_SUPPORT_HELPERS: [&str; 11] = [
    "data_trace",
    "event_str",
    "json_u64",
    "assert_event_order",
    "assert_resource_counter",
    "memory_dump_hex",
    "assert_register",
    "assert_register_absent",
    "rob_entry_at_sequence",
    "assert_rob_sequence_absent",
    "memory_result_event_at_pc",
];
const RESULT_SUPPORT_FUNCTIONS: [&str; 12] = [
    "data_trace",
    "event_str",
    "json_u64",
    "assert_event_order",
    "assert_resource_counter",
    "memory_dump_hex",
    "assert_register",
    "assert_register_absent",
    "rob_entries",
    "rob_entry_at_sequence",
    "assert_rob_sequence_absent",
    "memory_result_event_at_pc",
];
const RESULT_BOUNDARY_SUPPORT_HELPERS: [&str; 2] = [
    "pmp_denied_amo_output",
    "assert_denied_amo_failure_diagnostics",
];
const WRITEBACK_ROOT_MAX_LINES: usize = 1250;
const RESULT_SUPPORT_MAX_LINES: usize = 160;
const RESULT_CLASSES_MAX_LINES: usize = 700;
const RESULT_CLASSES_AGGREGATE_MAX_LINES: usize = 800;
const RESULT_BOUNDARIES_MAX_LINES: usize = 700;
const RESULT_BOUNDARIES_SUPPORT_MAX_LINES: usize = 140;
const RESULT_BOUNDARIES_AGGREGATE_MAX_LINES: usize = 800;
const STORE_CONDITIONAL_RESULT_MAX_LINES: usize = 650;

#[derive(Clone, Copy)]
struct ExpectedModuleDeclaration {
    name: &'static str,
    path: &'static str,
}

#[derive(Debug)]
struct ModuleDeclaration {
    name: String,
    path_attributes: Vec<Option<String>>,
    inline: bool,
}

#[test]
fn writeback_result_class_cli_evidence_has_focused_ownership() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join(WRITEBACK_ROOT);
    let support_path = crate_dir.join(RESULT_SUPPORT);
    let child_path = crate_dir.join(RESULT_CLASSES);
    let old_support_path = crate_dir.join(RESULT_CLASSES_OLD_SUPPORT);
    let boundary_path = crate_dir.join(RESULT_BOUNDARIES);
    let boundary_support_path = crate_dir.join(RESULT_BOUNDARIES_SUPPORT);
    let store_conditional_path = crate_dir.join(STORE_CONDITIONAL_RESULT);
    let root = fs::read_to_string(&root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();
    let support = fs::read_to_string(&support_path);
    let boundary = fs::read_to_string(&boundary_path);
    let boundary_support = fs::read_to_string(&boundary_support_path);
    let store_conditional = fs::read_to_string(&store_conditional_path);

    let root_functions = top_level_function_names(WRITEBACK_ROOT, &root);
    let mut boundary_failures = Vec::new();
    if line_count(&root_path) >= WRITEBACK_ROOT_MAX_LINES {
        boundary_failures.push(format!(
            "{WRITEBACK_ROOT} must remain below {WRITEBACK_ROOT_MAX_LINES} lines"
        ));
    }
    for helper in RESULT_SUPPORT_HELPERS {
        if root_functions.iter().any(|name| name == helper) {
            boundary_failures.push(format!(
                "{WRITEBACK_ROOT} must not own result helper `{helper}`"
            ));
        }
    }
    boundary_failures.extend(module_declaration_failures(
        WRITEBACK_ROOT,
        &root,
        &WRITEBACK_ROOT_MODULES,
    ));
    for (relative, source) in [
        (WRITEBACK_ROOT, root.as_str()),
        (RESULT_CLASSES, child.as_str()),
    ] {
        let includes = top_level_include_paths(relative, source);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{relative} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if old_support_path.exists() {
        boundary_failures.push(format!("{RESULT_CLASSES_OLD_SUPPORT} must be removed"));
    }
    if support.is_err() {
        boundary_failures.push(format!("{RESULT_SUPPORT} must exist"));
    }
    if boundary.is_err() {
        boundary_failures.push(format!("{RESULT_BOUNDARIES} must exist"));
    }
    if store_conditional.is_err() {
        boundary_failures.push(format!("{STORE_CONDITIONAL_RESULT} must exist"));
    }
    match &boundary {
        Ok(boundary) => {
            boundary_failures.extend(boundary_support_module_failures(
                RESULT_BOUNDARIES,
                boundary,
            ));
            let includes = top_level_include_paths(RESULT_BOUNDARIES, boundary);
            if !includes.is_empty() {
                boundary_failures.push(format!(
                    "{RESULT_BOUNDARIES} must not contain top-level include! fragments: {includes:?}"
                ));
            }
        }
        Err(_) => boundary_failures.push(format!("{RESULT_BOUNDARIES} must exist")),
    }
    match &boundary_support {
        Ok(boundary_support) => {
            let includes = top_level_include_paths(RESULT_BOUNDARIES_SUPPORT, boundary_support);
            if !includes.is_empty() {
                boundary_failures.push(format!(
                    "{RESULT_BOUNDARIES_SUPPORT} must not contain top-level include! fragments: {includes:?}"
                ));
            }
        }
        Err(_) => boundary_failures.push(format!("{RESULT_BOUNDARIES_SUPPORT} must exist")),
    }
    if let Ok(store_conditional) = &store_conditional {
        let includes = top_level_include_paths(STORE_CONDITIONAL_RESULT, store_conditional);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{STORE_CONDITIONAL_RESULT} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    assert!(
        boundary_failures.is_empty(),
        "writeback result ownership boundary is incomplete:\n{}",
        boundary_failures.join("\n")
    );
    let support = support.unwrap();
    let boundary = boundary.unwrap();
    let boundary_support = boundary_support.unwrap();
    let store_conditional = store_conditional.unwrap();

    assert!(
        line_count(&support_path) <= RESULT_SUPPORT_MAX_LINES,
        "{RESULT_SUPPORT} must remain at or below {RESULT_SUPPORT_MAX_LINES} lines"
    );
    assert!(
        line_count(&child_path) <= RESULT_CLASSES_MAX_LINES,
        "{RESULT_CLASSES} must remain at or below {RESULT_CLASSES_MAX_LINES} lines"
    );
    assert!(
        line_count(&child_path) + line_count(&support_path) <= RESULT_CLASSES_AGGREGATE_MAX_LINES,
        "result-class implementation must remain at or below {RESULT_CLASSES_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&boundary_path) <= RESULT_BOUNDARIES_MAX_LINES,
        "{RESULT_BOUNDARIES} must remain at or below {RESULT_BOUNDARIES_MAX_LINES} lines"
    );
    assert!(
        line_count(&boundary_support_path) <= RESULT_BOUNDARIES_SUPPORT_MAX_LINES,
        "{RESULT_BOUNDARIES_SUPPORT} must remain at or below {RESULT_BOUNDARIES_SUPPORT_MAX_LINES} lines"
    );
    assert!(
        line_count(&boundary_path) + line_count(&boundary_support_path)
            <= RESULT_BOUNDARIES_AGGREGATE_MAX_LINES,
        "result-boundary implementation must remain at or below {RESULT_BOUNDARIES_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&store_conditional_path) <= STORE_CONDITIONAL_RESULT_MAX_LINES,
        "{STORE_CONDITIONAL_RESULT} must remain at or below {STORE_CONDITIONAL_RESULT_MAX_LINES} lines"
    );
    let support_leaf_failures = support_leaf_failures(RESULT_SUPPORT, &support);
    assert!(
        support_leaf_failures.is_empty(),
        "{RESULT_SUPPORT} must remain a leaf support module:\n{}",
        support_leaf_failures.join("\n")
    );
    let support_function_failures =
        result_support_function_inventory_failures(RESULT_SUPPORT, &support);
    assert!(
        support_function_failures.is_empty(),
        "{RESULT_SUPPORT} must own exactly the result support helper inventory:\n{}",
        support_function_failures.join("\n")
    );
    assert!(
        top_level_module_names(RESULT_BOUNDARIES_SUPPORT, &boundary_support).is_empty(),
        "{RESULT_BOUNDARIES_SUPPORT} must remain a leaf support module"
    );
    assert!(
        top_level_module_names(STORE_CONDITIONAL_RESULT, &store_conditional).is_empty(),
        "{STORE_CONDITIONAL_RESULT} must remain a leaf module"
    );

    let child_functions = top_level_function_names(RESULT_CLASSES, &child);
    let support_functions = top_level_function_names(RESULT_SUPPORT, &support);
    for helper in RESULT_SUPPORT_HELPERS {
        assert_eq!(
            root_functions.iter().filter(|name| *name == helper).count(),
            0,
            "{WRITEBACK_ROOT} must not own `{helper}`"
        );
        assert_eq!(
            child_functions
                .iter()
                .filter(|name| *name == helper)
                .count(),
            0,
            "{RESULT_CLASSES} must not own `{helper}`"
        );
        assert_eq!(
            support_functions
                .iter()
                .filter(|name| *name == helper)
                .count(),
            1,
            "{RESULT_SUPPORT} must own exactly one `{helper}`"
        );
    }
    assert_eq!(
        top_level_function_names(RESULT_BOUNDARIES_SUPPORT, &boundary_support),
        RESULT_BOUNDARY_SUPPORT_HELPERS,
        "{RESULT_BOUNDARIES_SUPPORT} must own exactly the focused PMP subprocess helper"
    );

    let child_tests = result_class_tests(RESULT_CLASSES, &child);
    assert_eq!(
        child_tests, RESULT_CLASS_ANCHORS,
        "{RESULT_CLASSES} must own exactly the required result-class test anchors in order"
    );
    for (relative, source) in [(WRITEBACK_ROOT, root.as_str()), (RESULT_SUPPORT, &support)] {
        assert!(
            result_class_tests(relative, source).is_empty(),
            "{relative} must not own result-class-prefixed tests"
        );
    }

    let boundary_tests = top_level_test_names(RESULT_BOUNDARIES, &boundary);
    assert_eq!(
        boundary_tests, RESULT_BOUNDARY_ANCHORS,
        "{RESULT_BOUNDARIES} must own exactly the required boundary test anchors in order"
    );
    for anchor in RESULT_BOUNDARY_ANCHORS {
        assert_eq!(
            boundary.matches(anchor).count(),
            1,
            "{RESULT_BOUNDARIES} must contain boundary anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain boundary anchor `{anchor}`"
            );
        }
    }

    let store_conditional_tests =
        top_level_test_names(STORE_CONDITIONAL_RESULT, &store_conditional);
    assert_eq!(
        store_conditional_tests, STORE_CONDITIONAL_RESULT_ANCHORS,
        "{STORE_CONDITIONAL_RESULT} must own exactly the required SC result test anchors in order"
    );
    for anchor in STORE_CONDITIONAL_RESULT_ANCHORS {
        assert_eq!(
            store_conditional.matches(anchor).count(),
            1,
            "{STORE_CONDITIONAL_RESULT} must contain SC result anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain SC result anchor `{anchor}`"
            );
        }
    }

    assert_rustfmt_clean(&child_path);
    assert_rustfmt_clean(&support_path);
    assert_rustfmt_clean(&boundary_path);
    assert_rustfmt_clean(&boundary_support_path);
    assert_rustfmt_clean(&store_conditional_path);
}

#[test]
fn writeback_result_module_declaration_policy_rejects_non_external_or_wrong_path_modules() {
    let valid = r#"
#[path = "writeback_port/result_support.rs"]
mod result_support;

#[path = "writeback_port/result_classes.rs"]
mod result_classes;

#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
"#;
    assert!(module_declaration_failures("synthetic.rs", valid, &WRITEBACK_ROOT_MODULES).is_empty());

    for (label, source) in [
        (
            "inline module",
            r#"
#[path = "writeback_port/result_support.rs"]
mod result_support {}
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
"#,
        ),
        (
            "missing path",
            r#"
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
"#,
        ),
        (
            "duplicate path",
            r#"
#[path = "wrong.rs"]
#[path = "writeback_port/result_support.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
"#,
        ),
        (
            "wrong path",
            r#"
#[path = "writeback_port/wrong.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
"#,
        ),
        (
            "wrong SC path",
            r#"
#[path = "writeback_port/result_support.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/wrong.rs"]
mod store_conditional_result;
"#,
        ),
    ] {
        assert!(
            !module_declaration_failures("synthetic.rs", source, &WRITEBACK_ROOT_MODULES)
                .is_empty(),
            "{label} must be rejected"
        );
    }
}

#[test]
fn writeback_result_boundary_support_module_policy_rejects_non_external_or_wrong_path_modules() {
    let valid = r#"
#[path = "result_boundaries/support.rs"]
mod support;
"#;
    assert!(boundary_support_module_failures("synthetic.rs", valid).is_empty());

    for (label, source) in [
        (
            "inline module",
            r#"
#[path = "result_boundaries/support.rs"]
mod support {}
"#,
        ),
        (
            "missing path",
            r#"
mod support;
"#,
        ),
        (
            "duplicate path",
            r#"
#[path = "wrong.rs"]
#[path = "result_boundaries/support.rs"]
mod support;
"#,
        ),
        (
            "wrong path",
            r#"
#[path = "result_boundaries/wrong.rs"]
mod support;
"#,
        ),
    ] {
        assert!(
            !boundary_support_module_failures("synthetic.rs", source).is_empty(),
            "{label} must be rejected"
        );
    }
}

#[test]
fn writeback_result_support_leaf_policy_rejects_includes_and_child_modules() {
    assert!(support_leaf_failures("synthetic.rs", "use super::*;\nfn helper() {}\n").is_empty());
    assert!(!support_leaf_failures("synthetic.rs", "mod nested;\n").is_empty());
    assert!(!support_leaf_failures("synthetic.rs", "include!(\"nested.rs\");\n").is_empty());
    assert!(
        !support_leaf_failures("synthetic.rs", "include!(concat!(\"nested\", \".rs\"));\n")
            .is_empty()
    );
}

#[test]
fn writeback_result_support_helper_policy_rejects_extra_exported_functions() {
    let mut valid = String::new();
    for helper in RESULT_SUPPORT_FUNCTIONS {
        valid.push_str(&format!("pub(super) fn {helper}() {{}}\n"));
    }
    assert!(result_support_function_inventory_failures("synthetic.rs", &valid).is_empty());

    let extra = format!("{valid}pub(super) fn pmp_denied_amo_output() {{}}\n");
    assert!(
        !result_support_function_inventory_failures("synthetic.rs", &extra).is_empty(),
        "extra exported support helper must be rejected"
    );
}

fn top_level_test_names(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            function
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("test"))
                .then(|| function.sig.ident.to_string())
        })
        .collect()
}

fn result_class_tests(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            let name = function.sig.ident.to_string();
            (name.starts_with(RESULT_CLASS_TEST_PREFIX)
                && function
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("test")))
            .then_some(name)
        })
        .collect()
}

fn top_level_function_names(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect()
}

fn top_level_include_paths(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Macro(item) = item else {
                return None;
            };
            item.mac
                .path
                .is_ident("include")
                .then(|| top_level_include_argument(item.mac.tokens))
        })
        .collect()
}

fn top_level_include_argument(tokens: proc_macro2::TokenStream) -> String {
    syn::parse2::<syn::LitStr>(tokens.clone())
        .map(|literal| literal.value())
        .unwrap_or_else(|_| tokens.to_string())
}

fn top_level_module_names(relative: &str, source: &str) -> Vec<String> {
    top_level_module_declarations(relative, source)
        .into_iter()
        .map(|module| module.name)
        .collect()
}

fn top_level_module_declarations(relative: &str, source: &str) -> Vec<ModuleDeclaration> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Mod(module) = item else {
                return None;
            };
            Some(ModuleDeclaration {
                name: module.ident.to_string(),
                path_attributes: module
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("path"))
                    .map(path_attribute_literal)
                    .collect(),
                inline: module.content.is_some(),
            })
        })
        .collect()
}

fn path_attribute_literal(attr: &syn::Attribute) -> Option<String> {
    let syn::Meta::NameValue(name_value) = &attr.meta else {
        return None;
    };
    let syn::Expr::Lit(expr) = &name_value.value else {
        return None;
    };
    let syn::Lit::Str(literal) = &expr.lit else {
        return None;
    };
    Some(literal.value())
}

fn module_declaration_failures(
    relative: &str,
    source: &str,
    expected: &[ExpectedModuleDeclaration],
) -> Vec<String> {
    let modules = top_level_module_declarations(relative, source);
    let actual_names = modules
        .iter()
        .map(|module| module.name.as_str())
        .collect::<Vec<_>>();
    let expected_names = expected
        .iter()
        .map(|module| module.name)
        .collect::<Vec<_>>();

    let mut failures = Vec::new();
    if actual_names != expected_names {
        failures.push(format!(
            "{relative} must declare exactly the normal modules {expected_names:?}"
        ));
    }

    for expected_module in expected {
        let Some(module) = modules
            .iter()
            .find(|module| module.name == expected_module.name)
        else {
            continue;
        };
        if module.inline {
            failures.push(format!(
                "{relative} module `{}` must be an external module declaration",
                expected_module.name
            ));
        }
        match module.path_attributes.as_slice() {
            [] => failures.push(format!(
                "{relative} module `{}` must declare #[path = \"{}\"]",
                expected_module.name, expected_module.path
            )),
            [Some(actual_path)] if actual_path == expected_module.path => {}
            [Some(actual_path)] => failures.push(format!(
                "{relative} module `{}` must use #[path = \"{}\"], got #[path = \"{}\"]",
                expected_module.name, expected_module.path, actual_path
            )),
            [None] => failures.push(format!(
                "{relative} module `{}` must use a string-literal #[path = \"{}\"]",
                expected_module.name, expected_module.path
            )),
            path_attributes => failures.push(format!(
                "{relative} module `{}` must declare exactly one #[path = \"{}\"], found {}",
                expected_module.name,
                expected_module.path,
                path_attributes.len()
            )),
        }
    }
    failures
}

fn boundary_support_module_failures(relative: &str, source: &str) -> Vec<String> {
    module_declaration_failures(relative, source, &RESULT_BOUNDARY_SUPPORT_MODULES)
}

fn result_support_function_inventory_failures(relative: &str, source: &str) -> Vec<String> {
    let support_functions = top_level_function_names(relative, source);
    let expected_functions = RESULT_SUPPORT_FUNCTIONS.to_vec();
    if support_functions
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        == expected_functions
    {
        Vec::new()
    } else {
        vec![format!(
            "{relative} must own exactly top-level functions {expected_functions:?}, got {support_functions:?}"
        )]
    }
}

fn support_leaf_failures(relative: &str, source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let includes = top_level_include_paths(relative, source);
    if !includes.is_empty() {
        failures.push(format!(
            "{relative} must not contain top-level include! fragments: {includes:?}"
        ));
    }
    let modules = top_level_module_names(relative, source);
    if !modules.is_empty() {
        failures.push(format!(
            "{relative} must not declare child modules: {modules:?}"
        ));
    }
    failures
}

fn assert_rustfmt_clean(path: &Path) {
    let rustfmt = std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    let output = std::process::Command::new(rustfmt)
        .args(["--check", "--edition", "2021"])
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("failed to run rustfmt for {}: {error}", path.display()));
    assert!(
        output.status.success(),
        "rustfmt check failed for {}:\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn parsed_source(relative: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| {
        panic!("failed to parse {relative} for writeback ownership: {error}")
    })
}
