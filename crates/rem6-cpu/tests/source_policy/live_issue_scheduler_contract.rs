use super::*;

const PLAN_PATH: &str =
    "../../docs/superpowers/plans/2026-07-23-riscv-o3-persistent-cross-class-issue-queue.md";

#[test]
fn task6_scheduler_callback_uses_frontier_entry_before_service() {
    let sources = SchedulerContractSources::read();
    assert!(scheduler_contract_holds(&sources));
}

#[test]
fn task6_scheduler_contract_policy_rejects_bypass_mutations() {
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
        "outcome = self.service_live_issue_queue_at(hart, tick)?;",
        "outcome = self.service_live_issue_scheduler_at(hart, tick)?;",
        1,
    );
    assert_rejected(
        "lookahead advances scheduler frontier",
        &canonical,
        mutation,
    );

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
    mutation.service = mutation.service.replacen(
        "self.live_issue.request_service_at(earliest_tick);\n        let mut tick = earliest_tick;",
        "let mut tick = earliest_tick;",
        1,
    );
    assert_rejected("compatibility seed removed", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.plan = mutation.plan.replace(
        "service_live_issue_scheduler_at(&hart, now)",
        "service_live_issue_queue_at(&hart, now)",
    );
    assert_rejected(
        "Task 6 callback bypasses scheduler entry",
        &canonical,
        mutation,
    );
}

#[derive(Clone)]
struct SchedulerContractSources {
    service: String,
    plan: String,
}

impl SchedulerContractSources {
    fn read() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        Self {
            service: production_rust_source(
                &fs::read_to_string(root.join("src/o3_runtime_issue/service.rs")).unwrap(),
            ),
            plan: fs::read_to_string(root.join(PLAN_PATH)).unwrap(),
        }
    }
}

fn assert_rejected(
    description: &str,
    canonical: &SchedulerContractSources,
    mutation: SchedulerContractSources,
) {
    assert!(
        mutation.service != canonical.service || mutation.plan != canonical.plan,
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
    let Some(compatibility) =
        rust_function_definition(&sources.service, "schedule_live_speculative_issues")
    else {
        return false;
    };
    let frontier = compact_rust_code(&frontier);
    let entry = compact_rust_code(&entry);
    let compatibility = compact_rust_code(&compatibility);
    let enter = "self.enter_live_issue_scheduler_at(now);";
    let service = "self.service_live_issue_queue_at(hart,now)";
    let seed = "self.live_issue.request_service_at(earliest_tick);";
    let compatibility_entry = "letmutoutcome=self.service_live_issue_scheduler_at(hart,tick)?;";
    let task6 = sources
        .plan
        .split_once("### Task 6: Route All Queue Progress Through the O3 Wake")
        .and_then(|(_, tail)| tail.split_once("### Task 7:").map(|(task, _)| task));
    let Some(task6) = task6 else {
        return false;
    };
    let compact_task6 = compact_rust_code(task6);

    let checks = [
        ("one scheduler entry", entry.matches(enter).count() == 1),
        ("one queue service", entry.matches(service).count() == 1),
        ("entry before service", entry.find(enter) < entry.find(service)),
        (
            "entry brace depth",
            rust_anchor_occurs_at_brace_depth(&entry, enter, 1),
        ),
        (
            "service brace depth",
            rust_anchor_occurs_at_brace_depth(&entry, service, 1),
        ),
        ("frontier does not request", !frontier.contains("request_service_at")),
        ("wrapper does not request", !entry.contains("request_service_at")),
        ("one compatibility seed", compatibility.matches(seed).count() == 1),
        (
            "seed before wrapper",
            compatibility.find(seed) < compatibility.find(compatibility_entry),
        ),
        ("compatibility wrapper", compatibility.contains(compatibility_entry)),
        (
            "lookahead direct service",
            compatibility.contains("outcome=self.service_live_issue_queue_at(hart,tick)?;"),
        ),
        (
            "one compatibility wrapper",
            compatibility
                .matches("service_live_issue_scheduler_at(hart,tick)")
                .count()
                == 1,
        ),
        (
            "Task 6 wrapper call",
            task6.contains("service_live_issue_scheduler_at(&hart, now)"),
        ),
        (
            "Task 6 no direct call",
            !task6.contains("state.o3_runtime.service_live_issue_queue_at(&hart, now)"),
        ),
        (
            "Task 6 sole callback",
            task6.contains(
                "Only `mark_o3_writeback_wake_fired` calls `service_live_issue_scheduler_at` in production",
            ),
        ),
        (
            "Task 6 sole direct owner",
            task6.contains(
                "only `service_live_issue_scheduler_at` calls `service_live_issue_queue_at` directly",
            ),
        ),
        (
            "Task 6 preserves requests",
            compact_task6.contains("doesnotcreateoradvancealive-issueservicerequest"),
        ),
        (
            "Task 6 policy forbids request mutation",
            compact_task6.contains("assertthatneitherfunctioncontains`request_service_at`"),
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
