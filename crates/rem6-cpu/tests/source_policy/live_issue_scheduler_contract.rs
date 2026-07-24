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
    let Some(entry) = rust_function_definition(&sources.service, "service_live_issue_scheduler_at")
    else {
        return false;
    };
    let Some(compatibility) =
        rust_function_definition(&sources.service, "schedule_live_speculative_issues")
    else {
        return false;
    };
    let entry = compact_rust_code(&entry);
    let compatibility = compact_rust_code(&compatibility);
    let enter = "self.enter_live_issue_scheduler_at(now);";
    let service = "self.service_live_issue_queue_at(hart,now)";
    let task6 = sources
        .plan
        .split_once("### Task 6: Route All Queue Progress Through the O3 Wake")
        .and_then(|(_, tail)| tail.split_once("### Task 7:").map(|(task, _)| task));
    let Some(task6) = task6 else {
        return false;
    };

    entry.matches(enter).count() == 1
        && entry.matches(service).count() == 1
        && entry.find(enter) < entry.find(service)
        && rust_anchor_occurs_at_brace_depth(&entry, enter, 1)
        && rust_anchor_occurs_at_brace_depth(&entry, service, 1)
        && compatibility
            .contains("letmutoutcome=self.service_live_issue_scheduler_at(hart,tick)?;")
        && compatibility
            .contains("outcome=self.service_live_issue_queue_at(hart,tick)?;")
        && compatibility
            .matches("service_live_issue_scheduler_at(hart,tick)")
            .count()
            == 1
        && task6.contains("service_live_issue_scheduler_at(&hart, now)")
        && !task6.contains("state.o3_runtime.service_live_issue_queue_at(&hart, now)")
        && task6.contains(
            "Only `mark_o3_writeback_wake_fired` calls `service_live_issue_scheduler_at` in production",
        )
        && task6.contains(
            "only `service_live_issue_scheduler_at` calls `service_live_issue_queue_at` directly",
        )
}
