use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;
const MAX_IDE_CORE_LINES: usize = 1700;

#[test]
fn storage_lib_rs_remains_within_facade_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should stay within the storage facade budget, but it has {lines} lines"
    );
}

#[test]
fn storage_source_files_stay_within_size_limit() {
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
fn ide_core_stays_within_focused_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ide.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_IDE_CORE_LINES,
        "src/ide.rs should keep only core controller behavior, but it has {lines} lines"
    );
}

#[test]
fn ide_disk_lives_in_focused_module() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let disk_path = src_dir.join("ide_disk.rs");
    let controller = fs::read_to_string(src_dir.join("ide.rs")).unwrap();

    assert!(
        disk_path.exists(),
        "src/ide_disk.rs should own IDE disk behavior"
    );
    let disk = fs::read_to_string(disk_path).unwrap();

    assert!(
        !controller.contains("pub struct IdeDisk"),
        "src/ide.rs should not define IdeDisk"
    );
    assert!(
        !controller.contains("impl IdeDisk"),
        "src/ide.rs should not implement IdeDisk"
    );
    assert!(
        disk.contains("pub struct IdeDisk"),
        "src/ide_disk.rs should define IdeDisk"
    );
    assert!(
        disk.contains("impl IdeDisk"),
        "src/ide_disk.rs should implement IdeDisk"
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
