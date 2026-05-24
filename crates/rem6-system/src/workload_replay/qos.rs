use rem6_fabric::{QosFixedPriorityPolicy, QosQueueArbiter, QosQueuePolicyKind};
use rem6_workload::{WorkloadQosPolicy, WorkloadQosQueuePolicyKind};

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

pub(super) fn queue_arbiter(policy: &WorkloadQosPolicy) -> QosQueueArbiter {
    QosQueueArbiter::new(queue_policy_kind(policy.queue_policy()))
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
