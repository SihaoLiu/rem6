use std::fs;
use std::path::{Path, PathBuf};

use super::line_count;

const RESULT_CLASSES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs";
const PARENT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs";
const FIXTURE: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs";
const BOUNDARIES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries.rs";
const HANDOFF: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries/handoff.rs";
const POLICY: &str = "tests/source_policy/o3_translated_mmio_pair_ownership.rs";
const PARENT_OWNER: &str = "crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs";
const TESTS: [&str; 5] = [
    "rem6_run_o3_translated_memory_result_pair_width_one_direct",
    "rem6_run_o3_translated_memory_result_pair_width_two_exact_fit_direct",
    "rem6_run_o3_translated_memory_result_pair_width_one_cache_fabric_dram",
    "rem6_run_o3_translated_memory_mmio_result_pair_width_one_direct",
    "rem6_run_o3_translated_memory_mmio_result_pair_width_one_cache_fabric_dram",
];

#[test]
fn o3_translated_mmio_pair_ownership() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = rem6.parent().unwrap().parent().unwrap();
    let source_policy = rem6.join("tests/source_policy.rs");
    let result_classes = rem6.join(RESULT_CLASSES);
    let parent = rem6.join(PARENT);
    let fixture = rem6.join(FIXTURE);
    let boundaries = rem6.join(BOUNDARIES);
    let handoff = rem6.join(HANDOFF);
    let policy = rem6.join(POLICY);

    assert_external_module_path(
        &result_classes,
        "translated_mmio_pairs",
        "result_classes/translated_mmio_pairs.rs",
    );
    assert_external_module_path(
        &source_policy,
        "o3_translated_mmio_pair_ownership",
        "source_policy/o3_translated_mmio_pair_ownership.rs",
    );
    assert_external_module_path(&parent, "boundaries", "translated_mmio_pairs/boundaries.rs");
    assert_external_module_path(&parent, "fixture", "translated_mmio_pairs/fixture.rs");
    assert_external_module_path(&boundaries, "handoff", "boundaries/handoff.rs");

    assert_line_cap(&parent, 500);
    assert_line_cap(&fixture, 600);
    assert_line_cap(&boundaries, 550);
    assert_line_cap(&handoff, 180);
    assert_line_cap(&policy, 350);
    let aggregate =
        line_count(&parent) + line_count(&fixture) + line_count(&boundaries) + line_count(&handoff);
    assert!(
        aggregate <= 1_550,
        "translated result-pair family has {aggregate} lines, max 1550"
    );

    let parent_source = read(&parent);
    let fixture_source = read(&fixture);
    let boundaries_source = read(&boundaries);
    let handoff_source = read(&handoff);
    for (relative, source) in [
        (PARENT, parent_source.as_str()),
        (FIXTURE, fixture_source.as_str()),
        (BOUNDARIES, boundaries_source.as_str()),
        (HANDOFF, handoff_source.as_str()),
    ] {
        assert!(
            !source.contains("include!("),
            "{relative} must not contain top-level include! fragments"
        );
    }

    assert_eq!(
        top_level_test_names(PARENT, &parent_source),
        TESTS,
        "{PARENT} must own exactly the translated result-pair rows in order"
    );
    for test in TESTS {
        assert_eq!(
            duplicate_owners(repo, test),
            vec![PARENT_OWNER.to_string()],
            "{test} must have exactly one Rust-source owner"
        );
    }

    let core_anchors = read(&rem6.join("tests/source_policy/core_test_anchors.txt"));
    for test in TESTS {
        assert_eq!(
            core_anchors
                .lines()
                .filter(|anchor| *anchor == test)
                .count(),
            0,
            "Task 4 must not register core anchor {test} yet"
        );
    }

    for path in [&parent, &fixture, &boundaries, &handoff, &policy] {
        assert_rustfmt_clean(path);
    }
}

fn assert_external_module_path(path: &Path, module_name: &str, expected_path: &str) {
    let source = read(path);
    let syntax = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", relative(path).display()));
    let modules = syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Mod(module) = item else {
                return None;
            };
            (module.ident == module_name).then_some(module)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        modules.len(),
        1,
        "{} must attach {module_name} exactly once",
        relative(path).display()
    );
    let module = modules[0];
    assert!(
        module.content.is_none(),
        "{} module {module_name} must be external",
        relative(path).display()
    );
    let paths = module
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("path"))
        .map(path_attribute_literal)
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![Some(expected_path.to_string())],
        "{} module {module_name} must use #[path = \"{expected_path}\"]",
        relative(path).display()
    );
}

fn path_attribute_literal(attribute: &syn::Attribute) -> Option<String> {
    let syn::Meta::NameValue(value) = &attribute.meta else {
        return None;
    };
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(path),
        ..
    }) = &value.value
    else {
        return None;
    };
    Some(path.value())
}

fn top_level_test_names(relative: &str, source: &str) -> Vec<String> {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"))
        .items
        .into_iter()
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

fn assert_line_cap(path: &Path, maximum: usize) {
    let lines = line_count(path);
    assert!(
        lines <= maximum,
        "{} has {lines} lines, max {maximum}",
        relative(path).display()
    );
}

fn duplicate_owners(repo: &Path, test: &str) -> Vec<String> {
    let needle = format!("fn {test}(");
    rust_sources_under(&repo.join("crates/rem6/tests"))
        .into_iter()
        .filter_map(|path| {
            read(&path).contains(&needle).then(|| {
                path.strip_prefix(repo)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
        })
        .collect()
}

fn rust_sources_under(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_sources(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_sources(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_sources(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
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

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", relative(path).display()))
}

fn relative(path: &Path) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    path.strip_prefix(repo).unwrap_or(path).to_path_buf()
}
