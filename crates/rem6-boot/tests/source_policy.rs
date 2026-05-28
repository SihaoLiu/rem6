use std::fs;
use std::path::{Path, PathBuf};

fn crate_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn source_line_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        .lines()
        .count()
}

#[test]
fn boot_lib_rs_remains_a_facade() {
    assert!(
        source_line_count(&crate_path("src/lib.rs")) <= 240,
        "rem6-boot root lib.rs should remain a facade over focused modules",
    );
}

#[test]
fn boot_source_files_stay_within_module_budget() {
    let source_dir = crate_path("src");
    for entry in fs::read_dir(source_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        assert!(
            source_line_count(&path) <= 900,
            "rem6-boot source file {} is too large",
            path.display(),
        );
    }
}
