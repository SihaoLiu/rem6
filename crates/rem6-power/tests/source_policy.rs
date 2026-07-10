use std::fs;
use std::path::{Path, PathBuf};

const MAX_EXPORT_LINES: usize = 950;
const MAX_PARSER_LINES: usize = 450;

#[test]
fn external_power_parsers_live_in_focused_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let export_rs = fs::read_to_string(crate_dir.join("src/export.rs")).unwrap();

    for relative in ["src/export/mcpat_xml.rs", "src/export/dsent_csv.rs"] {
        assert!(
            crate_dir.join(relative).exists(),
            "external power parser belongs in {relative}"
        );
    }

    for handwritten_parser_anchor in [
        "fn xml_first_tag(",
        "fn xml_attributes(",
        "fn parse_csv_records(",
        "Document::parse",
        "ReaderBuilder",
        "use roxmltree::",
        "use csv::",
        "enum QuoteState",
        "in_quotes",
        "field_was_quoted",
    ] {
        assert!(
            !export_rs.contains(handwritten_parser_anchor),
            "src/export.rs should delegate parsing instead of defining {handwritten_parser_anchor}"
        );
    }

    for delegate in [
        concat!(
            "    pub fn from_mcpat_compatible_xml(input: &str) -> Result<Self, PowerError> {\n",
            "        mcpat_xml::parse(input)\n",
            "    }\n",
        ),
        concat!(
            "    pub fn from_dsent_compatible_csv(input: &str) -> Result<Self, PowerError> {\n",
            "        dsent_csv::parse(input)\n",
            "    }\n",
        ),
    ] {
        assert!(
            export_rs.contains(delegate),
            "src/export.rs import methods must remain delegation-only facades"
        );
    }
}

#[test]
fn external_power_parser_sources_stay_within_size_limits() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let export_path = crate_dir.join("src/export.rs");
    let export_lines = line_count(&export_path);
    assert!(
        export_lines <= MAX_EXPORT_LINES,
        "src/export.rs should remain a facade over focused parsers, but it has {export_lines} lines"
    );

    let parser_dir = crate_dir.join("src/export");
    if parser_dir.exists() {
        let mut oversized = Vec::new();
        for path in rust_source_files(&parser_dir) {
            let lines = line_count(&path);
            if lines > MAX_PARSER_LINES {
                oversized.push(format!(
                    "{} has {lines} lines",
                    path.strip_prefix(crate_dir).unwrap().display()
                ));
            }
        }
        assert!(
            oversized.is_empty(),
            "parser modules exceed {MAX_PARSER_LINES} lines: {}",
            oversized.join(", ")
        );
    }
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
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
