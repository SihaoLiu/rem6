use std::fs;
use std::path::{Path, PathBuf};

use super::{line_count, module_has_path_attribute};

const WIDTHS_OWNER: &str = "crates/rem6-cpu/src/o3_runtime_widths.rs";
const CONFIG_OWNER: &str = "crates/rem6/src/config.rs";
const SUMMARY_JSON_OWNER: &str = "crates/rem6/src/core_summary_json.rs";
const VALIDATION_OWNER: &str = "crates/rem6/tests/cli_run/validation/o3_memory_issue_width.rs";

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

    let config = read(&rem6.join("src/config.rs"));
    let timing = read(&rem6.join("src/config/riscv_timing.rs"));
    let runtime = read(&cpu.join("src/o3_runtime.rs"));
    let widths = read(&cpu.join("src/o3_runtime_widths.rs"));
    let core_runtime = read(&rem6.join("src/riscv_core_runtime.rs"));
    let summary_json = read(&rem6.join("src/core_summary_json.rs"));
    let validation = read(&rem6.join("tests/cli_run/validation/o3_memory_issue_width.rs"));
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
            &[(SUMMARY_JSON_OWNER, 1), (VALIDATION_OWNER, 1)],
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

fn relative(path: &Path) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    path.strip_prefix(repo).unwrap_or(path).to_path_buf()
}
