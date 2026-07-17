use super::*;

const WRITEBACK_ROOT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port.rs";
const RESULT_CLASSES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs";
const RESULT_CLASSES_SUPPORT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/support.rs";
const RESULT_CLASSES_INCLUDE: &str = "writeback_port/result_classes.rs";
const RESULT_CLASSES_SUPPORT_INCLUDE: &str = "result_classes/support.rs";
const RESULT_CLASS_TEST_PREFIX: &str = "rem6_run_o3_memory_result_writeback_";
const RESULT_CLASS_ANCHORS: [&str; 4] = [
    "rem6_run_o3_memory_result_writeback_matrix_direct",
    "rem6_run_o3_memory_result_writeback_matrix_cache_fabric_dram",
    "rem6_run_o3_memory_result_writeback_width_two_exact_fit",
    "rem6_run_o3_memory_result_writeback_readfile_mmio",
];
const RESULT_SUPPORT_HELPERS: [&str; 6] = [
    "data_trace",
    "event_str",
    "json_u64",
    "assert_event_order",
    "assert_resource_counter",
    "memory_dump_hex",
];
const WRITEBACK_ROOT_MAX_LINES: usize = 1250;
const RESULT_CLASSES_MAX_LINES: usize = 700;
const RESULT_CLASSES_SUPPORT_MAX_LINES: usize = 160;
const RESULT_CLASSES_AGGREGATE_MAX_LINES: usize = 800;

#[test]
fn writeback_result_class_cli_evidence_has_focused_ownership() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join(WRITEBACK_ROOT);
    let child_path = crate_dir.join(RESULT_CLASSES);
    let support_path = crate_dir.join(RESULT_CLASSES_SUPPORT);
    let root = fs::read_to_string(&root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();
    let support = fs::read_to_string(&support_path);

    let root_functions = top_level_function_names(WRITEBACK_ROOT, &root);
    let child_includes = top_level_include_paths(RESULT_CLASSES, &child);
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
    if child_includes != [RESULT_CLASSES_SUPPORT_INCLUDE] {
        boundary_failures.push(format!(
            "{RESULT_CLASSES} must contain exactly include!(\"{RESULT_CLASSES_SUPPORT_INCLUDE}\")"
        ));
    }
    if support.is_err() {
        boundary_failures.push(format!("{RESULT_CLASSES_SUPPORT} must exist"));
    }
    assert!(
        boundary_failures.is_empty(),
        "writeback result ownership boundary is incomplete:\n{}",
        boundary_failures.join("\n")
    );
    let support = support.unwrap();

    assert_eq!(
        top_level_include_paths(WRITEBACK_ROOT, &root),
        [RESULT_CLASSES_INCLUDE],
        "{WRITEBACK_ROOT} must contain exactly the result-class include"
    );
    assert!(
        line_count(&child_path) <= RESULT_CLASSES_MAX_LINES,
        "{RESULT_CLASSES} must remain at or below {RESULT_CLASSES_MAX_LINES} lines"
    );
    assert!(
        line_count(&support_path) <= RESULT_CLASSES_SUPPORT_MAX_LINES,
        "{RESULT_CLASSES_SUPPORT} must remain at or below {RESULT_CLASSES_SUPPORT_MAX_LINES} lines"
    );
    assert!(
        line_count(&child_path) + line_count(&support_path) <= RESULT_CLASSES_AGGREGATE_MAX_LINES,
        "result-class implementation must remain at or below {RESULT_CLASSES_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        top_level_include_paths(RESULT_CLASSES_SUPPORT, &support).is_empty(),
        "{RESULT_CLASSES_SUPPORT} must remain a leaf support fragment"
    );

    let child_functions = top_level_function_names(RESULT_CLASSES, &child);
    let support_functions = top_level_function_names(RESULT_CLASSES_SUPPORT, &support);
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
            "{RESULT_CLASSES_SUPPORT} must own exactly one `{helper}`"
        );
    }

    let child_tests = result_class_tests(RESULT_CLASSES, &child);
    assert_eq!(
        child_tests, RESULT_CLASS_ANCHORS,
        "{RESULT_CLASSES} must own exactly the required result-class test anchors in order"
    );
    for (relative, source) in [
        (WRITEBACK_ROOT, root.as_str()),
        (RESULT_CLASSES_SUPPORT, &support),
    ] {
        assert!(
            result_class_tests(relative, source).is_empty(),
            "{relative} must not own result-class-prefixed tests"
        );
    }

    assert_rustfmt_clean(&child_path);
    assert_rustfmt_clean(&support_path);
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
                .then(|| syn::parse2::<syn::LitStr>(item.mac.tokens).ok())
                .flatten()
                .map(|literal| literal.value())
        })
        .collect()
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
