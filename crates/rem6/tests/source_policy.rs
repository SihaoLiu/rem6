use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;
const MAX_ARCHITECTURE_OVERVIEW_LINES: usize = 600;
const MAX_MIGRATION_LEDGER_LINES: usize = 1200;
const MAX_ARCHITECTURE_README_LINES: usize = 80;
const MIGRATION_SCORE_BUCKETS: &[MigrationScoreBucket] = &[
    MigrationScoreBucket {
        name: "open",
        min: 0,
        max: 0,
    },
    MigrationScoreBucket {
        name: "scoped",
        min: 1,
        max: 19,
    },
    MigrationScoreBucket {
        name: "unit-slice",
        min: 20,
        max: 39,
    },
    MigrationScoreBucket {
        name: "single-axis",
        min: 40,
        max: 59,
    },
    MigrationScoreBucket {
        name: "representative",
        min: 60,
        max: 74,
    },
    MigrationScoreBucket {
        name: "matrix-gapped",
        min: 75,
        max: 89,
    },
    MigrationScoreBucket {
        name: "near-covered",
        min: 90,
        max: 99,
    },
    MigrationScoreBucket {
        name: "covered",
        min: 100,
        max: 100,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MigrationScoreBucket {
    name: &'static str,
    min: u8,
    max: u8,
}

impl MigrationScoreBucket {
    fn contains(self, percent: u8) -> bool {
        percent >= self.min && percent <= self.max
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MigrationScore<'a> {
    percent: u8,
    bucket: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScoreCalculation {
    completed: usize,
    total: usize,
    raw_percent: u8,
    bucket: String,
}

#[test]
fn public_run_config_exports_include_riscv_se_input_source() {
    let name = std::any::type_name::<rem6::RiscvSeInputSource>();

    assert!(name.ends_with("RiscvSeInputSource"));
}

#[test]
fn cli_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused CLI modules, but it has {lines} lines"
    );
}

#[test]
fn cli_source_files_stay_within_size_limit() {
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
fn cli_runtime_inputs_live_in_focused_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();

    assert!(
        crate_dir.join("src/config.rs").exists(),
        "CLI argument and runtime configuration parsing belongs in src/config.rs"
    );
    assert!(
        crate_dir.join("src/guest_memory.rs").exists(),
        "ELF and load-blob guest memory construction belongs in src/guest_memory.rs"
    );
    assert!(
        !lib_rs.contains("pub struct Rem6RunConfig {"),
        "src/lib.rs should re-export run configuration from a focused module"
    );
    assert!(
        !lib_rs.contains("fn read_load_blobs("),
        "src/lib.rs should delegate load-blob file reading to guest memory code"
    );
    assert!(
        !lib_rs.contains("fn build_cli_memory_store("),
        "src/lib.rs should delegate guest memory store construction to guest memory code"
    );
}

#[test]
fn cli_stats_output_root_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stats_output.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/stats_output.rs should remain a facade over focused stats-output modules, but it has {lines} lines"
    );
}

#[test]
fn architecture_docs_have_clear_boundaries() {
    let repo_root = repo_root();
    let readme = repo_root.join("docs/architecture/README.md");
    let architecture = repo_root.join("docs/architecture/rem6-architecture.md");
    let migration = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");

    assert!(
        readme.exists(),
        "architecture directory needs a content index"
    );
    assert!(
        architecture.exists(),
        "rem6 architecture needs one canonical doc"
    );
    assert!(
        migration.exists(),
        "gem5 to rem6 migration needs one canonical progress doc"
    );
    let architecture_docs = architecture_doc_paths(&repo_root);
    assert_eq!(
        architecture_docs,
        [
            "docs/architecture/README.md",
            "docs/architecture/gem5-to-rem6-migration.md",
            "docs/architecture/rem6-architecture.md",
        ],
        "docs/architecture should contain only the directory index, architecture overview, and migration ledger"
    );
    for retired in [
        "docs/architecture/gem5-test-migration.md",
        "docs/architecture/parallel-first-full-system-kernel.md",
    ] {
        assert!(
            !repo_root.join(retired).exists(),
            "{retired} should be folded into the canonical architecture or migration doc"
        );
    }

    let readme_contents = fs::read_to_string(&readme).unwrap();
    let readme_lines = readme_contents.lines().count();
    assert!(
        readme_lines <= MAX_ARCHITECTURE_README_LINES,
        "docs/architecture/README.md should stay a concise index, but it has {readme_lines} lines"
    );
    for required in [
        "# Architecture Documents",
        "## Content Index",
        "docs/architecture/rem6-architecture.md",
        "docs/architecture/gem5-to-rem6-migration.md",
        "stable architecture overview",
        "mutable migration ledger",
    ] {
        assert!(
            readme_contents.contains(required),
            "docs/architecture/README.md is missing required marker `{required}`"
        );
    }
    for forbidden in [
        "## Component Progress",
        "## Test Migration Ledger",
        "- [x]",
        "- [ ]",
    ] {
        assert!(
            !readme_contents.contains(forbidden),
            "docs/architecture/README.md should not duplicate migration ledger marker `{forbidden}`"
        );
    }

    let architecture_contents = fs::read_to_string(&architecture).unwrap();
    let architecture_lines = architecture_contents.lines().count();
    assert!(
        architecture_lines <= MAX_ARCHITECTURE_OVERVIEW_LINES,
        "rem6-architecture.md should stay a concise architecture overview, but it has {architecture_lines} lines"
    );
    for required in [
        "## gem5 Pain Points",
        "## rem6 Architecture",
        "## Runtime Invariants",
        "## Workspace Responsibilities",
        "## Evidence Policy",
    ] {
        assert!(
            architecture_contents.contains(required),
            "rem6-architecture.md is missing required section `{required}`"
        );
    }
    assert!(
        !architecture_contents.contains("## Migration Ledger")
            && !architecture_contents.contains("## Component Progress"),
        "architecture doc should not duplicate migration progress ledgers"
    );

    let migration_contents = fs::read_to_string(&migration).unwrap();
    let migration_lines = migration_contents.lines().count();
    assert!(
        migration_lines <= MAX_MIGRATION_LEDGER_LINES,
        "gem5-to-rem6-migration.md should stay a concise ledger, but it has {migration_lines} lines"
    );
    for required in [
        "## Document Boundary",
        "## Scoring Rubric",
        "## Component Progress",
        "## Test Migration Ledger",
        "## Update Rules",
        "- [x]",
        "- [ ]",
        "%",
        "Row score",
        "Checklist source",
        "single-axis",
    ] {
        assert!(
            migration_contents.contains(required),
            "gem5-to-rem6-migration.md is missing required marker `{required}`"
        );
    }
}

#[test]
fn gem5_migration_doc_tracks_core_test_anchors() {
    let repo_root = repo_root();
    let path = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");
    let contents = fs::read_to_string(&path).unwrap();

    for required in [
        "## Scoring Rubric",
        "## Test Migration Ledger",
        "tests/gem5/asmtest",
        "tests/gem5/cpu_tests",
        "tests/gem5/chi_tlm_tests",
        "tests/gem5/dram_lowp",
        "tests/gem5/fdp_tests",
        "tests/gem5/fs",
        "tests/gem5/gem5_resources",
        "tests/gem5/se_mode",
        "tests/gem5/riscv_boot_tests",
        "tests/gem5/kvm_fork_tests",
        "tests/gem5/kvm_switch_tests",
        "tests/gem5/memory",
        "tests/gem5/m5threads_test_atomic",
        "tests/gem5/parsec_benchmarks",
        "tests/gem5/processor_switch_tests",
        "tests/gem5/py_port",
        "tests/gem5/readfile_tests",
        "tests/gem5/replacement_policies",
        "tests/gem5/regression_tests",
        "tests/gem5/traffic_gen",
        "tests/gem5/stats",
        "tests/gem5/checkpoint_tests",
        "tests/gem5/gpu",
        "tests/gem5/stdlib",
        "tests/pyunit",
        "tests/test-progs",
        "rem6 owner",
        "Score",
        "Row score",
        "Next evidence",
        "demandMshrHits",
        "conditional_branch_predictions",
        "conditional_branch_predicted_taken",
        "conditional_branch_mispredictions",
        "system.cpu.branchPred.condPredicted",
        "system.cpu.branchPred.condPredictedTaken",
        "system.cpu.branchPred.condIncorrect",
        "system.cpu0.branchPred.condPredicted",
        "system.cpu0.branchPred.condPredictedTaken",
        "system.cpu0.branchPred.condIncorrect",
        "system.cpu1.branchPred.condPredicted",
        "system.cpu1.branchPred.condPredictedTaken",
        "system.cpu1.branchPred.condIncorrect",
        "system.cpu.branchPred.BTBLookups",
        "system.cpu.branchPred.BTBHits",
        "system.cpu.branchPred.BTBUpdates",
        "system.cpu.branchPred.BTBHitRatio",
        "system.cpu0.branchPred.BTBLookups",
        "system.cpu0.branchPred.BTBHits",
        "system.cpu0.branchPred.BTBUpdates",
        "system.cpu0.branchPred.BTBHitRatio",
        "system.cpu1.branchPred.BTBLookups",
        "system.cpu1.branchPred.BTBHits",
        "system.cpu1.branchPred.BTBUpdates",
        "system.cpu1.branchPred.BTBHitRatio",
        "system.l2.overallMshrHits",
        "system.l3.overallMshrMissRate",
        "demandMshrMisses",
        "demandMshrMissRate",
        "pfUnused",
        "pfHitInCache",
        "pfHitInMSHR",
        "pfHitInWB",
        "pfLate",
        "pfUsefulSpanPage",
        "pfUsefulButMiss",
        "accuracy_ppm",
        "coverage_ppm",
    ] {
        assert!(
            contents.contains(required),
            "gem5-to-rem6-migration.md is missing required anchor `{required}`"
        );
    }
}

#[test]
fn gem5_migration_sections_are_auditable() {
    let repo_root = repo_root();
    let path = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");
    let contents = fs::read_to_string(&path).unwrap();

    for section_heading in ["## Component Progress", "## External Adapter Migration"] {
        let section = markdown_section(&contents, section_heading)
            .unwrap_or_else(|| panic!("missing section `{section_heading}`"));
        let subsections = markdown_subsections(section);
        assert!(
            !subsections.is_empty(),
            "{section_heading} should contain auditable component subsections"
        );

        for (heading, body) in subsections {
            let score = migration_heading_score(heading).unwrap_or_else(|| {
                panic!("`{heading}` should end with a numeric percent and migration bucket")
            });
            let calculation = score_calculation(body)
                .unwrap_or_else(|| panic!("`{heading}` is missing a parseable score calculation"));
            let completed = count_checkbox_items(body, "- [x]");
            let total = completed + count_checkbox_items(body, "- [ ]");
            let raw_percent = rounded_percent(completed, total);
            let bucket = migration_score_bucket(&calculation.bucket).unwrap_or_else(|| {
                panic!(
                    "`{heading}` uses unknown bucket `{}` in score calculation",
                    calculation.bucket
                )
            });

            assert_eq!(
                score.bucket,
                calculation.bucket.as_str(),
                "`{heading}` header bucket should match its score calculation"
            );
            assert_eq!(
                calculation.completed, completed,
                "`{heading}` score calculation should match completed checklist items"
            );
            assert_eq!(
                calculation.total, total,
                "`{heading}` score calculation should match total checklist items"
            );
            assert_eq!(
                calculation.raw_percent, raw_percent,
                "`{heading}` score calculation should match rounded checklist percentage"
            );
            assert_eq!(
                score.percent,
                raw_percent.min(bucket.max),
                "`{heading}` header percent should be the raw percentage after the bucket cap"
            );
            assert!(
                bucket.contains(score.percent),
                "`{heading}` header percent should fit bucket `{}`",
                bucket.name
            );
            assert!(
                total > 0,
                "`{heading}` should have at least one checklist item"
            );
            assert!(
                migration_score_bucket(score.bucket).is_some(),
                "`{heading}` should end with a numeric percent and migration bucket"
            );
            assert!(
                body.contains("% raw") && body.contains("bucket cap"),
                "`{heading}` should state raw percentage and bucket cap"
            );
            for required in ["**Migrated:**", "**Not migrated:**", "**Next evidence:**"] {
                assert!(
                    body.contains(required),
                    "`{heading}` is missing required field `{required}`"
                );
            }
        }
    }

    let table = markdown_section(&contents, "## Test Migration Ledger")
        .expect("missing test migration ledger");
    let mut row_count = 0;
    for cells in markdown_table_rows(table) {
        if cells.first().is_none_or(|cell| !cell.starts_with('`')) {
            continue;
        }
        row_count += 1;
        assert_eq!(
            cells.len(),
            5,
            "test migration ledger rows should have five cells"
        );
        let score = migration_score(cells[2]).unwrap_or_else(|| {
            panic!(
                "test migration ledger row `{}` has invalid row score",
                cells[0]
            )
        });
        let bucket = migration_score_bucket(score.bucket).unwrap_or_else(|| {
            panic!(
                "test migration ledger row `{}` uses unknown bucket `{}`",
                cells[0], score.bucket
            )
        });
        assert!(
            bucket.contains(score.percent),
            "test migration ledger row `{}` percent should fit bucket `{}`",
            cells[0],
            bucket.name
        );
    }
    assert!(
        row_count > 0,
        "test migration ledger should contain scored gem5 test-anchor rows"
    );
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
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

fn architecture_doc_paths(repo_root: &Path) -> Vec<String> {
    let architecture_dir = repo_root.join("docs/architecture");
    let mut paths = Vec::new();
    for entry in fs::read_dir(&architecture_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(
                path.strip_prefix(repo_root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    paths.sort();
    paths
}

fn markdown_section<'a>(contents: &'a str, heading: &str) -> Option<&'a str> {
    let start = contents.find(heading)?;
    let after_heading = start + heading.len();
    let after = &contents[after_heading..];
    let end = after.find("\n## ").unwrap_or(after.len());
    Some(after[..end].trim())
}

fn markdown_subsections(section: &str) -> Vec<(&str, &str)> {
    let mut headings = Vec::new();
    let mut offset = 0;
    while offset < section.len() {
        let rest = &section[offset..];
        let line_end = rest
            .find('\n')
            .map(|relative| offset + relative)
            .unwrap_or(section.len());
        let line = &section[offset..line_end];
        if let Some(heading) = line.strip_prefix("### ") {
            headings.push((offset, heading.trim()));
        }
        offset = if line_end < section.len() {
            line_end + 1
        } else {
            section.len()
        };
    }

    let mut subsections = Vec::new();
    for (index, (start, heading)) in headings.iter().enumerate() {
        let body_start = section[*start..]
            .find('\n')
            .map(|relative| *start + relative + 1)
            .unwrap_or(section.len());
        let body_end = headings
            .get(index + 1)
            .map(|(next_start, _)| *next_start)
            .unwrap_or(section.len());
        subsections.push((*heading, &section[body_start..body_end]));
    }
    subsections
}

fn markdown_table_rows(section: &str) -> Vec<Vec<&str>> {
    section
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('|') || !line.ends_with('|') {
                return None;
            }
            Some(
                line.trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn migration_heading_score(heading: &str) -> Option<MigrationScore<'_>> {
    let (_, score) = heading.rsplit_once(" - ")?;
    migration_score(score)
}

fn migration_score(score: &str) -> Option<MigrationScore<'_>> {
    let (percent, bucket) = score.trim().split_once("% ")?;
    let percent = percent.trim().parse::<u8>().ok()?;
    let bucket = bucket.trim();
    (percent <= 100 && migration_score_bucket(bucket).is_some())
        .then_some(MigrationScore { percent, bucket })
}

fn score_calculation(body: &str) -> Option<ScoreCalculation> {
    let prefix = "**Score calculation:**";
    let start = body.find(prefix)?;
    let paragraph = body[start + prefix.len()..]
        .trim_start()
        .split("\n\n")
        .next()?;
    let paragraph = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let (fraction, after_fraction) =
        paragraph.split_once(" items have executable evidence, or ")?;
    let (completed, total) = fraction.split_once(" of ")?;
    let (raw_percent, after_percent) = after_fraction.split_once("% raw")?;
    let (_, after_bucket) = after_percent.split_once("The bucket cap is ")?;
    let bucket_end = after_bucket
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .unwrap_or(after_bucket.len());
    let bucket = &after_bucket[..bucket_end];

    Some(ScoreCalculation {
        completed: completed.trim().parse().ok()?,
        total: total.trim().parse().ok()?,
        raw_percent: raw_percent.trim().parse().ok()?,
        bucket: bucket.to_string(),
    })
}

fn count_checkbox_items(body: &str, marker: &str) -> usize {
    body.lines()
        .filter(|line| line.trim_start().starts_with(marker))
        .count()
}

fn rounded_percent(completed: usize, total: usize) -> u8 {
    assert!(
        total > 0,
        "migration score calculation needs a nonzero item count"
    );
    (((completed * 100) + (total / 2)) / total) as u8
}

fn migration_score_bucket(name: &str) -> Option<MigrationScoreBucket> {
    MIGRATION_SCORE_BUCKETS
        .iter()
        .copied()
        .find(|bucket| bucket.name == name)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
