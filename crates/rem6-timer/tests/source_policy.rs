use std::fs;
use std::path::{Path, PathBuf};

const MAX_CPU_LOCAL_TIMER_LINES: usize = 1550;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn cpu_local_timer_core_delegates_control_state() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let core_path = src_dir.join("cpu_local_timer.rs");
    let control_path = src_dir.join("cpu_local_timer/control.rs");
    let core = fs::read_to_string(&core_path).unwrap();

    assert!(
        control_path.exists(),
        "src/cpu_local_timer/control.rs should own CPU-local timer control state"
    );
    let control = fs::read_to_string(control_path).unwrap();

    for item in [
        "pub struct CpuLocalTimerControl",
        "pub struct CpuLocalWatchdogControl",
        "pub struct CpuLocalTimerZeroOutcome",
        "pub struct CpuLocalTimerWriteEffect",
    ] {
        assert!(
            !core.contains(item),
            "src/cpu_local_timer.rs should not define {item}"
        );
        assert!(
            control.contains(item),
            "src/cpu_local_timer/control.rs should define {item}"
        );
    }
}

#[test]
fn cpu_local_timer_core_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cpu_local_timer.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_CPU_LOCAL_TIMER_LINES,
        "src/cpu_local_timer.rs should delegate focused CPU-local timer facets, but it has {lines} lines"
    );
}

#[test]
fn timer_source_files_stay_within_size_limit() {
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
