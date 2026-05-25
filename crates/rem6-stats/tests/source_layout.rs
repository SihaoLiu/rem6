use std::fs;
use std::path::Path;

#[test]
fn stats_crate_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let line_count = line_count(&path);

    assert!(
        line_count <= 120,
        "src/lib.rs should remain a facade, but it has {line_count} lines"
    );
}

#[test]
fn stats_crate_source_files_stay_within_module_budget() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    for entry in fs::read_dir(source_dir).expect("source directory should be readable") {
        let path = entry.expect("source entry should be readable").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }

        let line_count = line_count(&path);
        assert!(
            line_count <= 1300,
            "{} has {line_count} lines",
            path.display()
        );
    }
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()))
        .lines()
        .count()
}
