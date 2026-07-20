use super::*;

const LOAD_ROOT: &str = "tests/cli_run/load.rs";
const POWER_MATRIX: &str = "tests/cli_run/load/power_activity_matrix.rs";

#[test]
fn run_power_activity_matrix_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join(LOAD_ROOT)).unwrap();
    let matrix = fs::read_to_string(crate_dir.join(POWER_MATRIX)).unwrap();

    assert!(root.contains("#[path = \"load/power_activity_matrix.rs\"]"));
    assert!(root.contains("mod power_activity_matrix;"));
    for test in [
        "rem6_run_power_analysis_includes_dram_activity",
        "rem6_run_power_analysis_includes_cache_activity",
        "rem6_run_power_analysis_includes_shared_cache_activity",
        "rem6_run_power_analysis_includes_fabric_activity",
        "rem6_run_power_analysis_includes_transport_activity",
    ] {
        assert!(matrix.contains(&format!("fn {test}")));
        assert!(!root.contains(&format!("fn {test}")));
    }
}
