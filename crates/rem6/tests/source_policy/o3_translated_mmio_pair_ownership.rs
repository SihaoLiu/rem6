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
const MIGRATION_LEDGER: &str = "docs/architecture/gem5-to-rem6-migration.md";
const PARENT_OWNER: &str = "crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs";
const BOUNDARY_OWNER: &str = "crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries.rs";
pub(super) const POSITIVE_ANCHORS: [&str; 5] = [
    "rem6_run_o3_translated_memory_result_pair_width_one_direct",
    "rem6_run_o3_translated_memory_result_pair_width_two_exact_fit_direct",
    "rem6_run_o3_translated_memory_result_pair_width_one_cache_fabric_dram",
    "rem6_run_o3_translated_memory_mmio_result_pair_width_one_direct",
    "rem6_run_o3_translated_memory_mmio_result_pair_width_one_cache_fabric_dram",
];
pub(super) const BOUNDARY_ANCHORS: [&str; 6] = [
    "rem6_run_o3_translated_result_pair_dependency_and_fault_boundaries",
    "rem6_run_o3_translated_result_pair_target_ordering_and_capacity_boundaries",
    "rem6_run_o3_translated_result_pair_live_checkpoint_and_prebind_switch_reject",
    "rem6_run_host_switch_transfers_o3_translated_memory_mmio_result_pair",
    "rem6_run_o3_translated_result_pair_drained_restore",
    "rem6_run_timing_suppresses_o3_translated_result_pairs",
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

    assert_external_modules(
        &result_classes,
        &[(
            "translated_mmio_pairs",
            "result_classes/translated_mmio_pairs.rs",
        )],
        false,
    );
    assert_external_modules(
        &source_policy,
        &[(
            "o3_translated_mmio_pair_ownership",
            "source_policy/o3_translated_mmio_pair_ownership.rs",
        )],
        false,
    );
    assert_external_modules(
        &parent,
        &[
            ("boundaries", "translated_mmio_pairs/boundaries.rs"),
            ("fixture", "translated_mmio_pairs/fixture.rs"),
        ],
        true,
    );
    assert_external_modules(&boundaries, &[("handoff", "boundaries/handoff.rs")], true);
    assert_external_modules(&fixture, &[], true);
    assert_external_modules(&handoff, &[], true);

    assert_line_cap(&parent, 500);
    assert_line_cap(&fixture, 600);
    assert_line_cap(&boundaries, 550);
    assert_line_cap(&handoff, 180);
    assert_line_cap(&policy, 350);
    let mut family = rust_sources_under(&parent.with_extension(""));
    family.push(parent.clone());
    family.sort();
    let mut expected_family = [
        parent.clone(),
        fixture.clone(),
        boundaries.clone(),
        handoff.clone(),
    ]
    .to_vec();
    expected_family.sort();
    assert_eq!(
        family, expected_family,
        "translated pair recursive file inventory"
    );
    let aggregate = family.iter().map(|path| line_count(path)).sum::<usize>();
    assert!(
        aggregate <= 1_625,
        "translated result-pair family has {aggregate} lines, max 1625"
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
        POSITIVE_ANCHORS,
        "{PARENT} must own exactly the translated result-pair rows in order"
    );
    assert_eq!(
        top_level_test_names(BOUNDARIES, &boundaries_source),
        BOUNDARY_ANCHORS,
        "{BOUNDARIES} must own exactly the translated result-pair boundaries in order"
    );
    for (anchors, owner, source) in [
        (
            POSITIVE_ANCHORS.as_slice(),
            PARENT_OWNER,
            parent_source.as_str(),
        ),
        (
            BOUNDARY_ANCHORS.as_slice(),
            BOUNDARY_OWNER,
            boundaries_source.as_str(),
        ),
    ] {
        for test in anchors {
            assert_eq!(source.matches(*test).count(), 1);
            assert_eq!(
                duplicate_owners(repo, test),
                vec![owner.to_string()],
                "{test} must have exactly one Rust-source owner"
            );
        }
    }

    let registered = read(&rem6.join("tests/source_policy/core_test_anchors.txt"))
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let translated_anchors = POSITIVE_ANCHORS
        .into_iter()
        .chain(BOUNDARY_ANCHORS)
        .map(str::to_string)
        .collect::<Vec<_>>();
    for test in &translated_anchors {
        assert_eq!(
            registered.iter().filter(|anchor| *anchor == test).count(),
            1,
            "core_test_anchors.txt must register {test} exactly once"
        );
    }
    let untranslated_tail = registered
        .iter()
        .position(|anchor| anchor == "rem6_run_timing_suppresses_o3_memory_result_pairs")
        .unwrap();
    assert_eq!(
        registered.get(untranslated_tail + 1..untranslated_tail + 1 + translated_anchors.len()),
        Some(translated_anchors.as_slice()),
        "translated result-pair anchors must immediately follow untranslated pair anchors"
    );

    let ledger = read(&repo.join(MIGRATION_LEDGER));
    assert!(ledger.contains("### CPU Execution Models - 74% representative\n**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap."));
    let cpu_section = between(
        &ledger,
        "### CPU Execution Models - 74% representative",
        "\n### ",
    );
    let overview = between(cpu_section, "**Score calculation:**", "\n**Migrated:**");
    let migrated = between(cpu_section, "**Migrated:**", "\n**Not migrated:**");
    let not_migrated = between(cpu_section, "**Not migrated:**", "\n**Evidence:**");
    let evidence = between(cpu_section, "**Evidence:**", "\n**Next evidence:**");
    let next_evidence = cpu_section.split_once("**Next evidence:**").unwrap().1;
    let crosswalk = ledger
        .lines()
        .find(|line| line.starts_with("| `tests/gem5/cpu_tests` |"))
        .expect("CPU crosswalk row");
    for anchor in &translated_anchors {
        for (surface, text) in [
            ("overview", overview),
            ("Migrated", migrated),
            ("Evidence", evidence),
            ("crosswalk", crosswalk),
        ] {
            assert!(text.contains(anchor), "{surface} missing `{anchor}`");
        }
    }
    for (surface, text) in [
        ("Not migrated", not_migrated),
        ("Next evidence", next_evidence),
        ("crosswalk", crosswalk),
    ] {
        assert!(
            text.contains("page-table-walk transport"),
            "{surface} must keep page-table-walk transport open"
        );
    }

    for path in [&parent, &fixture, &boundaries, &handoff, &policy] {
        assert_rustfmt_clean(path);
    }
}

fn assert_external_modules(path: &Path, expected: &[(&str, &str)], exact: bool) {
    let source = read(path);
    let syntax = syn::parse_file(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let modules = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) => Some(module),
            _ => None,
        })
        .filter(|module| exact || expected.iter().any(|(name, _)| module.ident == *name))
        .collect::<Vec<_>>();
    assert_eq!(
        modules.len(),
        expected.len(),
        "{} external module inventory",
        path.display()
    );
    for (module, (name, expected_path)) in modules.into_iter().zip(expected) {
        assert_eq!(module.ident, *name, "{} module inventory", path.display());
        assert!(
            module.content.is_none(),
            "{} module {name} must be external",
            path.display()
        );
        let paths = module
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("path"))
            .map(path_attribute_literal)
            .collect::<Vec<_>>();
        assert_eq!(paths, [expected_path.to_string()]);
    }
}

fn path_attribute_literal(attribute: &syn::Attribute) -> String {
    let value = attribute.meta.require_name_value().unwrap();
    let syn::Expr::Lit(value) = &value.value else {
        panic!("path attribute must be a literal");
    };
    let syn::Lit::Str(path) = &value.lit else {
        panic!("path attribute must be a string");
    };
    path.value()
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

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    text.split_once(start)
        .unwrap_or_else(|| panic!("missing ledger start `{start}`"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing ledger end `{end}`"))
        .0
}

fn assert_line_cap(path: &Path, maximum: usize) {
    let lines = line_count(path);
    assert!(
        lines <= maximum,
        "{} has {lines} lines, max {maximum}",
        path.display()
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
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
