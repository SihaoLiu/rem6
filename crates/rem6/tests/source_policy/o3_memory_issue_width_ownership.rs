use std::fs;
use std::path::{Path, PathBuf};

use super::{line_count, module_has_path_attribute};

const WIDTHS_OWNER: &str = "crates/rem6-cpu/src/o3_runtime_widths.rs";
const CALENDAR_OWNER: &str = "crates/rem6-cpu/src/o3_runtime_issue/calendar.rs";
const CONFIG_OWNER: &str = "crates/rem6/src/config.rs";
const SUMMARY_JSON_OWNER: &str = "crates/rem6/src/core_summary_json.rs";
const VALIDATION_OWNER: &str = "crates/rem6/tests/cli_run/validation/o3_memory_issue_width.rs";
const THREE_PENDING_PARENT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs";
const THREE_PENDING_PARENT_OWNER: &str = "crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs";
const THREE_PENDING_FIXTURE: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs";
const THREE_PENDING_BOUNDARY: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs";

#[test]
fn o3_memory_issue_width_config_and_runtime_ownership() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = rem6.parent().unwrap().parent().unwrap();
    let cpu = repo.join("crates/rem6-cpu");

    assert_line_cap(&cpu.join("src/o3_runtime_widths.rs"), 180);
    assert_line_cap(
        &rem6.join("tests/cli_run/validation/o3_memory_issue_width.rs"),
        500,
    );
    assert_line_cap(&cpu.join("src/o3_runtime.rs"), 1200);
    assert_line_cap(&rem6.join("src/config.rs"), 1699);
    assert_line_cap(&rem6.join(THREE_PENDING_PARENT), 550);
    assert_line_cap(&rem6.join(THREE_PENDING_FIXTURE), 450);
    assert_line_cap(&rem6.join(THREE_PENDING_BOUNDARY), 550);

    assert_module_path(
        &rem6.join("tests/cli_run/validation.rs"),
        "o3_memory_issue_width",
        "validation/o3_memory_issue_width.rs",
    );
    assert_module_path(
        &rem6.join("tests/source_policy.rs"),
        "o3_memory_issue_width_ownership",
        "source_policy/o3_memory_issue_width_ownership.rs",
    );
    assert_module_path(
        &cpu.join("src/o3_runtime.rs"),
        "o3_runtime_widths",
        "o3_runtime_widths.rs",
    );
    assert_module_path(
        &rem6.join("tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs"),
        "three_pending",
        "dependent_result_address/three_pending.rs",
    );
    assert_module_path(
        &rem6.join(THREE_PENDING_PARENT),
        "fixture",
        "three_pending/fixture.rs",
    );
    assert_module_path(
        &rem6.join(THREE_PENDING_PARENT),
        "boundaries",
        "three_pending/boundaries.rs",
    );

    let config = read(&rem6.join("src/config.rs"));
    let timing = read(&rem6.join("src/config/riscv_timing.rs"));
    let runtime = read(&cpu.join("src/o3_runtime.rs"));
    let widths = read(&cpu.join("src/o3_runtime_widths.rs"));
    let core_runtime = read(&rem6.join("src/riscv_core_runtime.rs"));
    let summary_json = read(&rem6.join("src/core_summary_json.rs"));
    let validation = read(&rem6.join("tests/cli_run/validation/o3_memory_issue_width.rs"));
    let three_pending = read(&rem6.join(THREE_PENDING_PARENT));
    let three_pending_fixture = read(&rem6.join(THREE_PENDING_FIXTURE));
    let three_pending_boundary = read(&rem6.join(THREE_PENDING_BOUNDARY));
    let all_sources = repository_rust_sources(repo);
    let production_sources = production_rust_sources(repo);

    assert_eq!(occurrences(&config, &memory_issue_width_config_field()), 2);
    assert_only_allowed_occurrences(
        &all_sources,
        &[memory_issue_width_config_field()],
        &[(CONFIG_OWNER, 2)],
        "RISC-V O3 memory issue width config field",
        occurrences,
    );
    assert!(timing.contains("struct RiscvO3WidthOptions"));
    assert!(timing.contains("memory_issue: Option<usize>"));
    assert!(timing.contains("fn validate_riscv_o3_memory_issue_width("));
    assert!(timing.contains("RiscvO3MemoryIssueWidthExceedsIssueWidth"));
    assert!(!config.contains("RiscvO3MemoryIssueWidthExceedsIssueWidth"));
    assert!(!config.contains("fn validate_riscv_o3_memory_issue_width("));
    assert!(!config.contains("parse_riscv_o3_memory_issue_width"));

    assert_eq!(occurrences(&runtime, "memory_issue_width: usize"), 1);
    assert!(!runtime.contains("fn set_issue_width("));
    assert!(!runtime.contains("fn set_memory_issue_width("));
    assert!(!runtime.contains("fn set_writeback_width("));
    for anchor in [
        "pub(crate) const fn issue_width(&self) -> usize",
        "pub(crate) const fn memory_issue_width(&self) -> usize",
        "pub(crate) fn set_memory_issue_width(&mut self, memory_issue_width: usize) -> bool",
        "pub(crate) fn set_writeback_width(&mut self, writeback_width: usize) -> bool",
    ] {
        assert!(
            widths.contains(anchor),
            "missing width owner anchor {anchor}"
        );
    }

    assert_eq!(
        occurrences(
            &core_runtime,
            "core.set_o3_memory_issue_width(config.riscv_o3_memory_issue_width());"
        ),
        1
    );
    assert!(
        core_runtime.find("core.set_o3_issue_width").unwrap()
            < core_runtime.find("core.set_o3_memory_issue_width").unwrap()
    );

    for key in [
        ["configured", "width"].join("_"),
        ["configured", "memory", "width"].join("_"),
    ] {
        assert_eq!(occurrences(&summary_json, &key), 1);
        assert!(
            validation.contains(&key),
            "CLI assertions must cover JSON key {key}"
        );
        assert_only_allowed_occurrences(
            &all_sources,
            &[key],
            &[
                (SUMMARY_JSON_OWNER, 1),
                (VALIDATION_OWNER, 1),
                (THREE_PENDING_PARENT_OWNER, 1),
            ],
            "O3 runtime issue JSON configured-width key",
            json_key_occurrences,
        );
    }
    assert_only_allowed_occurrences(
        &production_sources,
        &[
            "fn set_issue_width(".to_string(),
            "fn set_memory_issue_width(".to_string(),
            "fn set_writeback_width(".to_string(),
        ],
        &[(WIDTHS_OWNER, 3)],
        "O3 runtime width setter definitions",
        occurrences,
    );
    assert_only_allowed_occurrences(
        &production_sources,
        &[
            "self.issue_width =".to_string(),
            "self.memory_issue_width =".to_string(),
        ],
        &[(WIDTHS_OWNER, 2)],
        "O3 runtime width field mutations",
        occurrences,
    );
    assert_synthetic_policy_scans_are_meaningful();

    for anchor in [
        "rem6_run_o3_three_pending_sibling_width_one_direct",
        "rem6_run_o3_three_pending_sibling_width_two_direct",
        "rem6_run_o3_three_pending_sibling_width_four_hierarchy",
        "rem6_run_o3_three_pending_chain_width_four_direct",
        "rem6_run_o3_three_pending_chain_width_two_hierarchy",
        "rem6_run_o3_three_pending_mixed_fanout_width_two_hierarchy",
    ] {
        assert!(three_pending.contains(&format!("fn {anchor}(")));
    }
    assert_eq!(
        occurrences(&three_pending_fixture, "--riscv-o3-memory-issue-width"),
        1
    );
    for anchor in [
        "rem6_run_host_switch_preserves_o3_three_pending_transport_ticks",
        "rem6_run_timing_suppresses_o3_three_pending_surface",
    ] {
        assert!(three_pending_boundary.contains(&format!("fn {anchor}(")));
    }
}

#[test]
fn o3_memory_issue_width_calendar_ownership() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = rem6.parent().unwrap().parent().unwrap();
    let calendar_path = repo.join(CALENDAR_OWNER);
    let calendar = read(&calendar_path);
    let production_sources = production_rust_sources(repo);
    let anchors = memory_issue_width_calendar_anchors();

    assert_line_cap(&calendar_path, 450);
    for anchor in &anchors {
        assert!(
            calendar.contains(anchor),
            "calendar owner is missing memory issue width anchor `{anchor}`"
        );
    }
    assert!(
        !calendar.contains("collect::<BTreeSet") && !calendar.contains("pending_ticks"),
        "calendar capture must reserve memory once per materialized pending row without deduplicating same-tick rows"
    );
    assert_only_allowed_occurrences(
        &production_sources,
        &anchors,
        &[(CALENDAR_OWNER, anchors.len())],
        "O3 memory issue width calendar arbitration",
        occurrences,
    );
    assert_calendar_policy_scan_is_meaningful();
}

fn assert_line_cap(path: &Path, maximum: usize) {
    let lines = line_count(path);
    assert!(
        lines <= maximum,
        "{} has {lines} lines, max {maximum}",
        relative(path).display()
    );
}

fn assert_module_path(path: &Path, module: &str, expected: &str) {
    let source = read(path);
    assert!(
        module_has_path_attribute(&source, module, expected),
        "{} must attach {module} via #[path = \"{expected}\"]",
        relative(path).display()
    );
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", relative(path).display()))
}

fn occurrences(source: &str, needle: &str) -> usize {
    source.match_indices(needle).count()
}

fn json_key_occurrences(source: &str, needle: &str) -> usize {
    source
        .match_indices(needle)
        .filter(|(index, _)| {
            let before = source[..*index].chars().next_back();
            let after = source[*index + needle.len()..].chars().next();
            matches!(before, Some('"') | Some('/')) && !is_rust_identifier_char(after)
        })
        .count()
}

fn is_rust_identifier_char(character: Option<char>) -> bool {
    character.is_some_and(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn assert_synthetic_policy_scans_are_meaningful() {
    let json_key = configured_width_key();
    let json_literal = format!("\"{json_key}\"");
    let sources = vec![
        SourceFile {
            relative: WIDTHS_OWNER.to_string(),
            source: [
                "fn set_issue_width(",
                "fn set_memory_issue_width(",
                "fn set_writeback_width(",
                "self.issue_width =",
                "self.memory_issue_width =",
            ]
            .join("\n"),
        },
        SourceFile {
            relative: "crates/rem6-cpu/src/o3_runtime.rs".to_string(),
            source: [
                "fn set_memory_issue_width(",
                "self.issue_width =",
                &json_literal,
                &memory_issue_width_config_field(),
            ]
            .join("\n"),
        },
    ];

    let failures = disallowed_occurrence_failures(
        &sources,
        &[
            "fn set_issue_width(".to_string(),
            "fn set_memory_issue_width(".to_string(),
            "fn set_writeback_width(".to_string(),
            "self.issue_width =".to_string(),
            configured_width_key(),
            memory_issue_width_config_field(),
        ],
        &[WIDTHS_OWNER],
        occurrences,
    );
    let json_failures = disallowed_occurrence_failures(
        &sources,
        &[json_key.clone()],
        &[WIDTHS_OWNER],
        json_key_occurrences,
    );

    assert!(
        failures
            .iter()
            .any(|failure| failure.contains("o3_runtime.rs")),
        "synthetic negative scan must report an injected non-owner violation: {failures:?}"
    );
    assert!(
        json_failures
            .iter()
            .any(|failure| failure.contains(&json_key)),
        "synthetic JSON-key scan must report an injected non-owner violation: {json_failures:?}"
    );
}

fn assert_calendar_policy_scan_is_meaningful() {
    let anchors = memory_issue_width_calendar_anchors();
    let sources = vec![
        SourceFile {
            relative: CALENDAR_OWNER.to_string(),
            source: anchors.join("\n"),
        },
        SourceFile {
            relative: "crates/rem6-cpu/src/o3_runtime.rs".to_string(),
            source: anchors.join("\n"),
        },
    ];

    let failures =
        disallowed_occurrence_failures(&sources, &anchors, &[CALENDAR_OWNER], occurrences);

    assert!(
        failures
            .iter()
            .any(|failure| failure.contains("o3_runtime.rs")),
        "synthetic calendar policy scan must report an injected non-owner violation: {failures:?}"
    );
}

fn assert_only_allowed_occurrences(
    sources: &[SourceFile],
    needles: &[String],
    allowed: &[(&str, usize)],
    label: &str,
    count_occurrences: fn(&str, &str) -> usize,
) {
    let failures = disallowed_occurrence_failures(
        sources,
        needles,
        &allowed
            .iter()
            .map(|(relative, _)| *relative)
            .collect::<Vec<_>>(),
        count_occurrences,
    );
    assert!(
        failures.is_empty(),
        "{label} must stay in the allowlist only: {}",
        failures.join(", ")
    );

    for (relative, expected) in allowed {
        let count = sources
            .iter()
            .filter(|source| source.relative == *relative)
            .map(|source| {
                needles
                    .iter()
                    .map(|needle| count_occurrences(&source.source, needle))
                    .sum::<usize>()
            })
            .sum::<usize>();
        assert_eq!(
            count, *expected,
            "{label} expected {expected} occurrence(s) in {relative}, found {count}"
        );
    }
}

fn disallowed_occurrence_failures(
    sources: &[SourceFile],
    needles: &[String],
    allowed_relatives: &[&str],
    count_occurrences: fn(&str, &str) -> usize,
) -> Vec<String> {
    let mut failures = Vec::new();
    for source in sources {
        if allowed_relatives
            .iter()
            .any(|allowed| source.relative == *allowed)
        {
            continue;
        }
        for needle in needles {
            let count = count_occurrences(&source.source, needle);
            if count > 0 {
                failures.push(format!(
                    "{} contains {count} occurrence(s) of `{needle}`",
                    source.relative
                ));
            }
        }
    }
    failures
}

#[derive(Clone, Debug)]
struct SourceFile {
    relative: String,
    source: String,
}

fn repository_rust_sources(repo: &Path) -> Vec<SourceFile> {
    rust_sources_under(repo, &repo.join("crates"), |_| true)
}

fn production_rust_sources(repo: &Path) -> Vec<SourceFile> {
    rust_sources_under(repo, &repo.join("crates"), is_production_relative)
}

fn rust_sources_under(
    repo: &Path,
    root: &Path,
    include_relative: fn(&str) -> bool,
) -> Vec<SourceFile> {
    let mut paths = Vec::new();
    collect_rust_paths(root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let relative = path
                .strip_prefix(repo)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            include_relative(&relative).then(|| SourceFile {
                source: read(&path),
                relative,
            })
        })
        .collect()
}

fn collect_rust_paths(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rust_paths(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

fn is_production_relative(relative: &str) -> bool {
    relative.contains("/src/")
        && !relative.contains("/tests/")
        && !relative.contains("_tests/")
        && !relative.ends_with("_tests.rs")
        && !relative.ends_with("/tests.rs")
}

fn configured_width_key() -> String {
    ["configured", "width"].join("_")
}

fn memory_issue_width_config_field() -> String {
    format!(
        "{}: Option<usize>",
        ["riscv", "o3", "memory", "issue", "width"].join("_")
    )
}

fn memory_issue_width_calendar_anchors() -> Vec<String> {
    vec![
        [
            "struct O3LiveIssueCalendar {",
            "    issue_width: usize,",
            "    memory_issue_width: usize,",
        ]
        .join("\n"),
        "memory_issue_width: runtime.memory_issue_width(),".to_string(),
        "memory_issue_width.saturating_sub(reservations.memory)".to_string(),
        [
            "for tick in runtime",
            "            .pending_data_addresses",
            "            .iter()",
            "            .filter_map(|pending| pending.selected_issue_tick)",
            "        {",
            "            calendar.reserve(tick, O3IssueOpClass::Memory);",
            "        }",
        ]
        .join("\n"),
    ]
}

fn relative(path: &Path) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    path.strip_prefix(repo).unwrap_or(path).to_path_buf()
}
