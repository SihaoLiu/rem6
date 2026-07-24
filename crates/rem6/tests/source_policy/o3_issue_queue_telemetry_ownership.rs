use std::fs;
use std::path::Path;

use super::rust_source_files;

const QUEUE_FIELDS: [(&str, &str, &str); 9] = [
    ("enqueued_rows", "enqueued_rows", "enqueued_rows"),
    ("service_turns", "service_turns", "service_turns"),
    ("wake_requests", "wake_requests", "wake_requests"),
    (
        "current_occupancy",
        "current_occupancy",
        "current_occupancy",
    ),
    ("peak_occupancy", "peak_occupancy", "peak_occupancy"),
    (
        "scalar_integer",
        "issued_by_class.scalar_integer",
        "scalar_integer_issued_rows",
    ),
    (
        "integer_mul_div",
        "issued_by_class.integer_mul_div",
        "integer_mul_div_issued_rows",
    ),
    (
        "memory_agu",
        "issued_by_class.memory_agu",
        "memory_agu_issued_rows",
    ),
    ("control", "issued_by_class.control", "control_issued_rows"),
];

#[test]
fn o3_issue_queue_telemetry_paths_have_focused_output_owners() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let final_stats =
        fs::read_to_string(crate_dir.join("src/stats_output/o3_runtime_issue.rs")).unwrap();
    let summary_json = fs::read_to_string(crate_dir.join("src/core_summary_json.rs")).unwrap();
    let live_stats =
        fs::read_to_string(crate_dir.join("../rem6-system/src/riscv_o3_runtime_stats/cpu.rs"))
            .unwrap();

    assert!(final_stats.contains("StatResetPolicy::Resettable"));
    for (json_key, stat_suffix, getter) in QUEUE_FIELDS {
        assert!(
            summary_json.contains(&format!("\\\"{json_key}\\\""))
                && summary_json.contains(&format!("queue.{getter}()")),
            "core summary JSON is missing queue field {json_key} via {getter}"
        );
        assert!(
            final_stats.contains(&format!("\"{stat_suffix}\""))
                && final_stats.contains(&format!("queue.{getter}()")),
            "final stats are missing issue_queue.{stat_suffix} via {getter}"
        );
        assert!(
            live_stats.contains(&format!("\"issue_queue.{stat_suffix}\""))
                && live_stats.contains(&format!("current_live_issue.{getter}()")),
            "live stats are missing issue_queue.{stat_suffix} via {getter}"
        );
    }

    let final_stats_owner = crate_dir.join("src/stats_output/o3_runtime_issue.rs");
    for path in rust_source_files(&crate_dir.join("src/stats_output")) {
        if path == final_stats_owner {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("issue_queue."),
            "{} duplicates final issue_queue stat ownership",
            path.display()
        );
    }
}
