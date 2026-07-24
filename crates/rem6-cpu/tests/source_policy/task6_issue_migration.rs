use super::*;

const PLAN_PATH: &str =
    "../../docs/superpowers/plans/2026-07-23-riscv-o3-persistent-cross-class-issue-queue.md";

const MIGRATION_PATHS: &[&str] = &[
    "crates/rem6-cpu/src/o3_runtime_issue_tests.rs",
    "crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs",
    "crates/rem6-cpu/src/o3_runtime_issue/service_tests/scheduler_request.rs",
    "crates/rem6-cpu/src/o3_runtime_issue/service_tests/legacy_driver.rs",
    "crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs",
    "crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs",
    "crates/rem6-cpu/src/o3_runtime_pending_address_tests/scheduling.rs",
    "crates/rem6-cpu/src/o3_runtime_pending_address_tests/three_pending.rs",
    "crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs",
    "crates/rem6-cpu/src/o3_runtime_memory_result_tests/replan.rs",
    "crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs",
    "crates/rem6-cpu/tests/source_policy.rs",
    "crates/rem6-cpu/tests/source_policy/live_issue_scheduler_contract.rs",
];

const MIGRATION_CONTRACT: &[&str] = &[
    "Remove the production `schedule_live_speculative_issues` function and its future-tick loop.",
    "`service_live_issue_queue_until_boundary_for_test`",
    "same `(hart, head, earliest_tick)` arguments",
    "`#[cfg(test)]`",
    "must not appear in `production_rust_source`",
    "`scheduler_request.rs` must use explicit real scheduler-facing turns",
    "`request_service_at(tick)` followed by `service_live_issue_scheduler_at`",
    "Mechanically replace every other legacy test call",
    "Remove `o3_live_issue_compatibility_driver_fails_closed`",
    "Remove `o3_live_issue_service_compatibility_policy_rejects_silent_result_mutations`",
    "remove compatibility anchors and required compatibility test names",
    "scheduler wrapper is the sole production scheduler entry and direct-service owner",
    "legacy helper is cfg-gated and test-only",
];

const MIGRATED_SELECTORS: &[&str] = &[
    "o3_runtime_issue_tests",
    "scheduler_facing_",
    "service_live_issue_queue_at_",
    "scoped_issue_",
    "replay",
    "o3_runtime_pending_address_tests",
    "o3_runtime_memory_result_tests",
    "deep_scalar_cleanup",
    "task6_issue_migration",
    "o3_live_issue_",
];

#[test]
fn task6_plan_inventories_live_issue_test_and_policy_migration() {
    let plan = read_plan();
    assert!(task6_migration_contract_holds(&plan));
}

#[test]
fn task6_plan_migration_contract_rejects_omissions() {
    let canonical = read_plan();
    assert!(task6_migration_contract_holds(&canonical));

    for path in MIGRATION_PATHS {
        let files_mutation =
            replace_in_task6(&canonical, &format!("`{path}`"), "`omitted.rs`", false);
        assert_rejected(
            &format!("Files list omits {path}"),
            &canonical,
            files_mutation,
        );

        let staging_mutation = replace_in_task6(&canonical, path, "omitted.rs", true);
        assert_rejected(
            &format!("staging command omits {path}"),
            &canonical,
            staging_mutation,
        );
    }

    for instruction in MIGRATION_CONTRACT {
        let mutation =
            replace_all_in_task6(&canonical, instruction, "omitted migration instruction");
        assert_rejected(instruction, &canonical, mutation);
    }

    for selector in MIGRATED_SELECTORS {
        let mutation = replace_in_verification(&canonical, selector, "omitted_selector");
        assert_rejected(
            &format!("verification omits {selector}"),
            &canonical,
            mutation,
        );
    }
}

fn read_plan() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(PLAN_PATH)).unwrap()
}

fn assert_rejected(description: &str, canonical: &str, mutation: String) {
    assert_ne!(mutation, canonical, "mutation did not apply: {description}");
    assert!(
        !task6_migration_contract_holds(&mutation),
        "accepted incomplete Task 6 migration: {description}",
    );
}

fn replace_in_task6(source: &str, needle: &str, replacement: &str, from_end: bool) -> String {
    let (start, end) = task6_bounds(source).expect("missing Task 6 section");
    let task6 = &source[start..end];
    let relative = if from_end {
        task6.rfind(needle)
    } else {
        task6.find(needle)
    }
    .unwrap_or_else(|| panic!("missing mutation anchor {needle}"));
    let position = start + relative;
    let mut mutated = source.to_owned();
    mutated.replace_range(position..position + needle.len(), replacement);
    mutated
}

fn replace_all_in_task6(source: &str, needle: &str, replacement: &str) -> String {
    let (start, end) = task6_bounds(source).expect("missing Task 6 section");
    let replaced = source[start..end].replace(needle, replacement);
    assert_ne!(
        replaced,
        source[start..end],
        "missing mutation anchor {needle}"
    );
    let mut mutated = source.to_owned();
    mutated.replace_range(start..end, &replaced);
    mutated
}

fn replace_in_verification(source: &str, needle: &str, replacement: &str) -> String {
    let (start, end) = verification_bounds(source).expect("missing Task 6 verification block");
    let relative = source[start..end]
        .find(needle)
        .unwrap_or_else(|| panic!("missing verification mutation anchor {needle}"));
    let position = start + relative;
    let mut mutated = source.to_owned();
    mutated.replace_range(position..position + needle.len(), replacement);
    mutated
}

fn task6_migration_contract_holds(plan: &str) -> bool {
    let Some(task6) = task6_section(plan) else {
        return false;
    };
    let Some((files, steps)) = task6.split_once("- [ ] **Step 1:") else {
        return false;
    };
    let Some((_, staging_tail)) = steps.split_once("git add ") else {
        return false;
    };
    let staging = staging_tail.lines().next().unwrap_or_default();
    let Some((verification_start, verification_end)) = verification_bounds(plan) else {
        return false;
    };
    let verification = &plan[verification_start..verification_end];

    let mut valid = true;
    for path in MIGRATION_PATHS {
        if !files.contains(&format!("`{path}`")) {
            eprintln!("Task 6 Files list misses {path}");
            valid = false;
        }
        if !staging.split_whitespace().any(|item| item == *path) {
            eprintln!("Task 6 staging misses {path}");
            valid = false;
        }
    }
    for instruction in MIGRATION_CONTRACT {
        if !task6.contains(instruction) {
            eprintln!("Task 6 migration text misses {instruction}");
            valid = false;
        }
    }
    for selector in MIGRATED_SELECTORS {
        if !verification
            .lines()
            .any(|line| line.contains("cargo test -p rem6-cpu") && line.contains(selector))
        {
            eprintln!("Task 6 verification misses {selector}");
            valid = false;
        }
    }
    valid
}

fn task6_section(plan: &str) -> Option<&str> {
    task6_bounds(plan).map(|(start, end)| &plan[start..end])
}

fn task6_bounds(plan: &str) -> Option<(usize, usize)> {
    let marker = "### Task 6: Route All Queue Progress Through the O3 Wake";
    let start = plan.find(marker)?.checked_add(marker.len())?;
    let end = start.checked_add(plan[start..].find("### Task 7:")?)?;
    Some((start, end))
}

fn verification_bounds(plan: &str) -> Option<(usize, usize)> {
    let (task6_start, task6_end) = task6_bounds(plan)?;
    let task6 = &plan[task6_start..task6_end];
    let step6 = task6.find("- [ ] **Step 6:")?;
    let block_marker = step6.checked_add(task6[step6..].find("```bash")?)?;
    let start = task6_start
        .checked_add(block_marker)?
        .checked_add("```bash".len())?;
    let end = start.checked_add(plan[start..task6_end].find("```")?)?;
    Some((start, end))
}
