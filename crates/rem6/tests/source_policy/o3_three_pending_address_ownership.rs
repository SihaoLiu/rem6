use std::fs;
use std::path::{Path, PathBuf};

use super::{line_count, module_has_path_attribute};

const PARENT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs";
const FIXTURE: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs";
const BOUNDARY: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs";
const PARENT_OWNER: &str =
    "crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs";
const BOUNDARY_OWNER: &str = "crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs";
const ANCHORS: [&str; 6] = [
    "rem6_run_o3_three_pending_sibling_width_one_direct",
    "rem6_run_o3_three_pending_sibling_width_two_direct",
    "rem6_run_o3_three_pending_sibling_width_four_hierarchy",
    "rem6_run_o3_three_pending_chain_width_four_direct",
    "rem6_run_o3_three_pending_chain_width_two_hierarchy",
    "rem6_run_o3_three_pending_mixed_fanout_width_two_hierarchy",
];
const BOUNDARY_ANCHORS: [&str; 6] = [
    "rem6_run_o3_three_pending_rejects_fourth_unresolved",
    "rem6_run_o3_three_pending_rejects_nonadjacent_graph",
    "rem6_run_o3_three_pending_replays_middle_failure",
    "rem6_run_o3_three_pending_checkpoint_boundary",
    "rem6_run_host_switch_preserves_o3_three_pending_transport_ticks",
    "rem6_run_timing_suppresses_o3_three_pending_surface",
];

#[test]
fn o3_three_pending_positive_cli_ownership() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = rem6.parent().unwrap().parent().unwrap();
    let dependent_root =
        rem6.join("tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs");
    let source_policy = rem6.join("tests/source_policy.rs");
    let parent = rem6.join(PARENT);
    let fixture = rem6.join(FIXTURE);
    let boundary = rem6.join(BOUNDARY);

    assert_module_path(
        &dependent_root,
        "three_pending",
        "dependent_result_address/three_pending.rs",
    );
    assert_module_path(
        &source_policy,
        "o3_three_pending_address_ownership",
        "source_policy/o3_three_pending_address_ownership.rs",
    );
    assert_module_path(&parent, "fixture", "three_pending/fixture.rs");
    assert_module_path(&parent, "boundaries", "three_pending/boundaries.rs");

    assert_line_cap(&parent, 550);
    assert_line_cap(&fixture, 450);
    assert_line_cap(&boundary, 550);

    let parent_source = read(&parent);
    let fixture_source = read(&fixture);
    let boundary_source = read(&boundary);
    assert!(!parent_source.contains("include!("));
    assert!(!fixture_source.contains("include!("));
    assert!(!boundary_source.contains("include!("));

    for anchor in ANCHORS {
        assert_eq!(
            function_definition_count(&parent_source, anchor),
            1,
            "{PARENT} must define exactly one positive test anchor {anchor}"
        );
        assert_eq!(
            duplicate_owners(repo, anchor),
            vec![PARENT_OWNER.to_string()],
            "{anchor} must have exactly one Rust-source owner"
        );
    }
    for anchor in BOUNDARY_ANCHORS {
        assert_eq!(
            function_definition_count(&boundary_source, anchor),
            1,
            "{BOUNDARY} must define exactly one boundary test anchor {anchor}"
        );
        assert_eq!(
            duplicate_owners(repo, anchor),
            vec![BOUNDARY_OWNER.to_string()],
            "{anchor} must have exactly one Rust-source owner"
        );
    }
}

fn assert_module_path(path: &Path, module: &str, expected: &str) {
    let source = read(path);
    assert!(
        module_has_path_attribute(&source, module, expected),
        "{} must attach {module} via #[path = \"{expected}\"]",
        relative(path).display()
    );
    assert_eq!(
        function_or_module_count(&source, &format!("mod {module};")),
        1,
        "{} must attach {module} exactly once",
        relative(path).display()
    );
}

fn assert_line_cap(path: &Path, maximum: usize) {
    let lines = line_count(path);
    assert!(
        lines <= maximum,
        "{} has {lines} lines, max {maximum}",
        relative(path).display()
    );
}

fn duplicate_owners(repo: &Path, anchor: &str) -> Vec<String> {
    let needle = format!("fn {anchor}(");
    rust_sources_under(&repo.join("crates/rem6/tests"))
        .into_iter()
        .filter_map(|path| {
            let source = read(&path);
            (function_or_module_count(&source, &needle) > 0).then(|| {
                path.strip_prefix(repo)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
        })
        .collect()
}

fn function_definition_count(source: &str, name: &str) -> usize {
    function_or_module_count(source, &format!("fn {name}("))
}

fn function_or_module_count(source: &str, needle: &str) -> usize {
    source.match_indices(needle).count()
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
