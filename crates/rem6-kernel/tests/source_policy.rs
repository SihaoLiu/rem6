use std::fs;
use std::path::{Path, PathBuf};

const MAX_SCHEDULER_LINES: usize = 1550;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn kernel_scheduler_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/scheduler.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_SCHEDULER_LINES,
        "src/scheduler.rs should delegate focused scheduler facets, but it has {lines} lines"
    );
}

#[test]
fn kernel_source_files_stay_within_size_limit() {
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
fn scheduler_snapshot_state_lives_in_focused_module() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/scheduler");
    let state = fs::read_to_string(src_dir.join("state.rs")).unwrap();
    let snapshot_path = src_dir.join("state/snapshot.rs");

    assert!(
        snapshot_path.exists(),
        "src/scheduler/state/snapshot.rs should own scheduler snapshot state"
    );
    let snapshot = fs::read_to_string(snapshot_path).unwrap();

    for item in [
        "pub struct SchedulerSnapshot",
        "pub struct PartitionSnapshot",
        "pub struct PendingEventSnapshot",
        "pub struct SchedulerDispatchRecord",
        "pub enum ScheduledEventKind",
    ] {
        assert!(
            !state.contains(item),
            "src/scheduler/state.rs should not define {item}"
        );
        assert!(
            snapshot.contains(item),
            "src/scheduler/state/snapshot.rs should define {item}"
        );
    }
}

#[test]
fn runtime_scheduler_event_token_stays_internal() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let scheduler = fs::read_to_string(src_dir.join("scheduler.rs")).unwrap();
    let snapshot = fs::read_to_string(src_dir.join("scheduler/state/snapshot.rs")).unwrap();
    let lib = fs::read_to_string(src_dir.join("lib.rs")).unwrap();

    assert!(scheduler.contains("pub(crate) struct SchedulerEventToken"));
    assert!(!scheduler.contains("pub struct SchedulerEventToken"));
    assert!(!snapshot.contains("pub fn token("));
    assert!(!lib.contains("SchedulerEventToken"));
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
