use super::*;

#[test]
fn task6_scheduler_wrapper_is_sole_production_issue_service_owner() {
    let sources = SchedulerContractSources::read();
    assert!(scheduler_contract_holds(&sources));
}

#[test]
fn task6_scheduler_contract_policy_rejects_final_owner_mutations() {
    let canonical = SchedulerContractSources::read();
    assert!(scheduler_contract_holds(&canonical));

    let mut mutation = canonical.clone();
    mutation.service = mutation.service.replacen(
        "self.enter_live_issue_scheduler_at(now);\n        self.service_live_issue_queue_at(hart, now)",
        "self.service_live_issue_queue_at(hart, now)",
        1,
    );
    assert_rejected("scheduler entry removed", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.service = mutation.service.replacen(
        "self.enter_live_issue_scheduler_at(now);\n        self.service_live_issue_queue_at(hart, now)",
        "let outcome = self.service_live_issue_queue_at(hart, now);\n        self.enter_live_issue_scheduler_at(now);\n        outcome",
        1,
    );
    assert_rejected("scheduler entry moved after service", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.service = mutation.service.replacen(
        "let finalized = self.live_issue.enter_scheduler_at(earliest_tick);",
        "self.live_issue.request_service_at(earliest_tick);\n        let finalized = self.live_issue.enter_scheduler_at(earliest_tick);",
        1,
    );
    assert_rejected(
        "frontier entry creates a service request",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.service = mutation.service.replacen(
        "self.enter_live_issue_scheduler_at(now);",
        "self.live_issue.request_service_at(now);\n        self.enter_live_issue_scheduler_at(now);",
        1,
    );
    assert_rejected(
        "scheduler wrapper creates a service request",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.service.push_str(
        "\nimpl O3RuntimeState {\n    pub(crate) fn schedule_live_speculative_issues(&mut self) {}\n}\n",
    );
    assert_rejected(
        "production compatibility helper restored",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.legacy_driver = mutation
        .legacy_driver
        .replace("#[cfg(test)]", "#[allow(dead_code)]");
    assert_rejected("legacy driver cfg gate removed", &canonical, mutation);
}

#[derive(Clone)]
struct SchedulerContractSources {
    service: String,
    service_tests: String,
    legacy_driver: String,
}

impl SchedulerContractSources {
    fn read() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        Self {
            service: production_rust_source(
                &fs::read_to_string(root.join("src/o3_runtime_issue/service.rs")).unwrap(),
            ),
            service_tests: fs::read_to_string(root.join("src/o3_runtime_issue/service_tests.rs"))
                .unwrap(),
            legacy_driver: fs::read_to_string(
                root.join("src/o3_runtime_issue/service_tests/legacy_driver.rs"),
            )
            .unwrap_or_default(),
        }
    }
}

fn assert_rejected(
    description: &str,
    canonical: &SchedulerContractSources,
    mutation: SchedulerContractSources,
) {
    assert!(
        mutation.service != canonical.service
            || mutation.service_tests != canonical.service_tests
            || mutation.legacy_driver != canonical.legacy_driver,
        "scheduler-contract mutation did not apply: {description}",
    );
    assert!(
        !scheduler_contract_holds(&mutation),
        "accepted weakened scheduler-contract mutation: {description}",
    );
}

fn scheduler_contract_holds(sources: &SchedulerContractSources) -> bool {
    let Some(frontier) =
        rust_function_definition(&sources.service, "enter_live_issue_scheduler_at")
    else {
        return false;
    };
    let Some(entry) = rust_function_definition(&sources.service, "service_live_issue_scheduler_at")
    else {
        return false;
    };
    let frontier = compact_rust_code(&frontier);
    let entry = compact_rust_code(&entry);
    let enter = "self.enter_live_issue_scheduler_at(now);";
    let service = "self.service_live_issue_queue_at(hart,now)";
    let legacy_driver = &sources.legacy_driver;

    let checks = [
        ("one scheduler entry", entry.matches(enter).count() == 1),
        ("one queue service", entry.matches(service).count() == 1),
        (
            "entry before service",
            entry.find(enter) < entry.find(service),
        ),
        (
            "entry brace depth",
            rust_anchor_occurs_at_brace_depth(&entry, enter, 1),
        ),
        (
            "service brace depth",
            rust_anchor_occurs_at_brace_depth(&entry, service, 1),
        ),
        (
            "frontier does not request",
            !frontier.contains("request_service_at"),
        ),
        (
            "wrapper does not request",
            !entry.contains("request_service_at"),
        ),
        (
            "wrapper owns only production direct service call",
            rust_method_call_positions(&sources.service, "service_live_issue_queue_at").len() == 1,
        ),
        (
            "no production compatibility helper",
            !sources.service.contains("schedule_live_speculative_issues")
                && !sources
                    .service
                    .contains("service_live_issue_queue_until_boundary_for_test"),
        ),
        (
            "legacy helper module is cfg-gated",
            sources.service_tests.contains("#[cfg(test)]")
                && sources
                    .service_tests
                    .contains("#[path = \"service_tests/legacy_driver.rs\"]")
                && sources.service_tests.contains("mod legacy_driver;"),
        ),
        (
            "legacy helper is cfg-test only",
            legacy_driver.contains("#[cfg(test)]")
                && legacy_driver.contains("service_live_issue_queue_until_boundary_for_test")
                && legacy_driver.contains("service_live_issue_scheduler_at(hart, tick)?")
                && legacy_driver.contains("service_live_issue_queue_at(hart, tick)?"),
        ),
        (
            "legacy helper not in production source",
            !sources
                .service
                .contains("service_live_issue_queue_until_boundary_for_test"),
        ),
    ];
    for (description, passed) in checks {
        if !passed {
            eprintln!("failed scheduler-contract check: {description}");
            return false;
        }
    }
    true
}
