use rem6_dram::{DramQosSchedulingPolicy, DramQosTurnaroundPolicy};
use rem6_fabric::{
    QosFixedPriorityPolicy, QosPriorityPolicy, QosProportionalFairPolicy, QosQueueArbiter,
    QosQueuePolicyKind,
};
use rem6_workload::{
    WorkloadQosPolicy, WorkloadQosPriorityPolicyKind, WorkloadQosQueuePolicyKind,
    WorkloadQosTurnaroundPolicyKind,
};

pub(super) fn fixed_priority_policy(policy: &WorkloadQosPolicy) -> QosFixedPriorityPolicy {
    let mut fixed =
        QosFixedPriorityPolicy::new(policy.priority_levels(), policy.default_priority())
            .expect("workload QoS policy validates priority levels");
    for requestor in policy.requestor_priorities() {
        fixed = fixed
            .with_requestor_priority(requestor.requestor(), requestor.priority())
            .expect("workload QoS policy validates requestor priorities");
    }
    fixed
}

pub(super) fn proportional_fair_policy(policy: &WorkloadQosPolicy) -> QosProportionalFairPolicy {
    let mut proportional = QosProportionalFairPolicy::new(
        policy.priority_levels(),
        policy
            .proportional_fair_weight()
            .expect("workload proportional-fair QoS policy validates weight"),
    )
    .expect("workload proportional-fair QoS policy validates priority levels and weight");
    for requestor in policy.requestor_scores() {
        proportional = proportional
            .with_requestor_score(requestor.requestor(), requestor.score())
            .expect("workload proportional-fair QoS policy validates requestor scores");
    }
    proportional
}

pub(super) fn priority_policy(policy: &WorkloadQosPolicy) -> QosPriorityPolicy {
    match policy.priority_policy_kind() {
        WorkloadQosPriorityPolicyKind::FixedPriority => fixed_priority_policy(policy).into(),
        WorkloadQosPriorityPolicyKind::ProportionalFair => proportional_fair_policy(policy).into(),
    }
}

pub(super) fn queue_arbiter(policy: &WorkloadQosPolicy) -> QosQueueArbiter {
    QosQueueArbiter::new(queue_policy_kind(policy.queue_policy()))
}

pub(super) fn dram_scheduling_policy(policy: &WorkloadQosPolicy) -> DramQosSchedulingPolicy {
    let mut scheduling = DramQosSchedulingPolicy::new()
        .with_turnaround(turnaround_policy_kind(policy.turnaround_policy()));
    if policy.priority_escalation_enabled() {
        scheduling = scheduling.with_priority_escalation();
    }
    scheduling
}

fn queue_policy_kind(policy: WorkloadQosQueuePolicyKind) -> QosQueuePolicyKind {
    match policy {
        WorkloadQosQueuePolicyKind::Fifo => QosQueuePolicyKind::Fifo,
        WorkloadQosQueuePolicyKind::Lifo => QosQueuePolicyKind::Lifo,
        WorkloadQosQueuePolicyKind::LeastRecentlyGranted => {
            QosQueuePolicyKind::LeastRecentlyGranted
        }
    }
}

fn turnaround_policy_kind(policy: WorkloadQosTurnaroundPolicyKind) -> DramQosTurnaroundPolicy {
    match policy {
        WorkloadQosTurnaroundPolicyKind::RequestOrder => DramQosTurnaroundPolicy::RequestOrder,
        WorkloadQosTurnaroundPolicyKind::PreferCurrentDirection => {
            DramQosTurnaroundPolicy::PreferCurrentDirection
        }
    }
}
