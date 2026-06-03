use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_dram::{
    DramMemoryController, DramMemoryError, DramMemoryOutcome, DramMemorySnapshot,
    DramMemoryWaitForMarker, DramQosRequest, DramQosSchedulingPolicy, DramTargetActivity,
};
use rem6_fabric::{QosPriorityPolicy, QosQueueArbiter, QosRequestorId};
use rem6_kernel::{Tick, WaitForGraph};
use rem6_memory::{
    Address, MemoryRequest, MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};
use rem6_transport::{RequestDelivery, TargetBatchOutcome, TargetOutcome};
use rem6_workload::WorkloadQosPolicy;

use super::qos::{dram_scheduling_policy, priority_policy, queue_arbiter};
use super::RiscvWorkloadReplayError;

#[derive(Clone)]
pub(super) enum WorkloadMemoryBackend {
    Store(Arc<Mutex<PartitionedMemoryStore>>),
    Dram(Arc<Mutex<WorkloadDramBackend>>),
}

#[derive(Clone, Debug)]
struct WorkloadDramQosState {
    priority_policy: QosPriorityPolicy,
    arbiter: QosQueueArbiter,
    scheduling_policy: DramQosSchedulingPolicy,
    next_order: u64,
}

impl WorkloadDramQosState {
    fn new(policy: &WorkloadQosPolicy) -> Self {
        Self {
            priority_policy: priority_policy(policy),
            arbiter: queue_arbiter(policy),
            scheduling_policy: dram_scheduling_policy(policy),
            next_order: 0,
        }
    }

    fn qos_request_with_policy<'a>(
        priority_policy: &mut QosPriorityPolicy,
        next_order: &mut u64,
        request: &'a MemoryRequest,
    ) -> Result<DramQosRequest<'a>, DramMemoryError> {
        let requestor = QosRequestorId::new(request.id().agent().get());
        let priority = priority_policy
            .priority_for(requestor, request.size().bytes())
            .map_err(|source| DramMemoryError::Qos { source })?;
        let order = *next_order;
        *next_order = next_order
            .checked_add(1)
            .expect("workload DRAM QoS order does not overflow");
        Ok(DramQosRequest::new(request, priority, order))
    }

    fn accept(
        &mut self,
        controller: &mut DramMemoryController,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        Ok(self
            .accept_requests(controller, arrival_cycle, [request])?
            .pop()
            .expect("single workload DRAM QoS request returns one outcome"))
    }

    fn accept_requests<'a, I>(
        &mut self,
        controller: &mut DramMemoryController,
        arrival_cycle: u64,
        requests: I,
    ) -> Result<Vec<DramMemoryOutcome>, DramMemoryError>
    where
        I: IntoIterator<Item = &'a MemoryRequest>,
    {
        let mut priority_policy = self.priority_policy.clone();
        let mut next_order = self.next_order;
        let requests = requests
            .into_iter()
            .map(|request| {
                Self::qos_request_with_policy(&mut priority_policy, &mut next_order, request)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let outcomes = controller.accept_qos_batch_with_policy(
            arrival_cycle,
            requests,
            &mut self.arbiter,
            self.scheduling_policy,
        )?;
        self.priority_policy = priority_policy;
        self.next_order = next_order;
        Ok(outcomes)
    }

    fn accept_batch(
        &mut self,
        controller: &mut DramMemoryController,
        arrival_cycle: u64,
        deliveries: &[RequestDelivery],
    ) -> Result<Vec<DramMemoryOutcome>, DramMemoryError> {
        self.accept_requests(
            controller,
            arrival_cycle,
            deliveries.iter().map(RequestDelivery::request),
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct WorkloadDramBackend {
    controller: DramMemoryController,
    qos: Option<WorkloadDramQosState>,
}

impl WorkloadDramBackend {
    pub(super) fn new(
        controller: DramMemoryController,
        qos_policy: Option<&WorkloadQosPolicy>,
    ) -> Self {
        Self {
            controller,
            qos: qos_policy.map(WorkloadDramQosState::new),
        }
    }

    fn accept(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        if let Some(qos) = self.qos.as_mut() {
            return qos.accept(&mut self.controller, arrival_cycle, request);
        }
        self.controller.accept(arrival_cycle, request)
    }

    fn accept_batch(
        &mut self,
        arrival_cycle: u64,
        deliveries: &[RequestDelivery],
    ) -> Option<Result<Vec<TargetBatchOutcome>, DramMemoryError>> {
        let qos = self.qos.as_mut()?;
        let ticks = deliveries
            .iter()
            .map(|delivery| (delivery.request().id(), delivery.tick()))
            .collect::<BTreeMap<_, _>>();
        let outcomes = match qos.accept_batch(&mut self.controller, arrival_cycle, deliveries) {
            Ok(outcomes) => outcomes,
            Err(error) => return Some(Err(error)),
        };
        Some(Ok(outcomes
            .into_iter()
            .map(|outcome| {
                let request = outcome.dram_access().request();
                let tick = ticks
                    .get(&request)
                    .copied()
                    .expect("DRAM batch outcome matches a delivered request");
                TargetBatchOutcome::new(request, dram_target_outcome(tick, outcome))
            })
            .collect()))
    }

    pub(super) fn controller(&self) -> &DramMemoryController {
        &self.controller
    }

    pub(super) fn controller_mut(&mut self) -> &mut DramMemoryController {
        &mut self.controller
    }
}

impl WorkloadMemoryBackend {
    pub(super) fn memory_snapshot(&self) -> PartitionedMemorySnapshot {
        match self {
            Self::Store(store) => store
                .lock()
                .expect("workload replay memory lock")
                .snapshot(),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .controller()
                .snapshot()
                .store()
                .clone(),
        }
    }

    pub(super) fn dram_snapshot(&self) -> Option<DramMemorySnapshot> {
        match self {
            Self::Store(_) => None,
            Self::Dram(dram) => Some(
                dram.lock()
                    .expect("workload replay DRAM lock")
                    .controller()
                    .snapshot(),
            ),
        }
    }

    pub(super) fn dram_target_activities(&self) -> Vec<DramTargetActivity> {
        match self {
            Self::Store(_) => Vec::new(),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .controller()
                .target_activities(),
        }
    }

    pub(super) fn mark_dram_wait_for(&self) -> Option<DramMemoryWaitForMarker> {
        match self {
            Self::Store(_) => None,
            Self::Dram(dram) => Some(
                dram.lock()
                    .expect("workload replay DRAM lock")
                    .controller()
                    .mark_wait_for(),
            ),
        }
    }

    pub(super) fn dram_wait_for_since(
        &self,
        marker: Option<DramMemoryWaitForMarker>,
    ) -> WaitForGraph {
        let Some(marker) = marker else {
            return WaitForGraph::new();
        };
        match self {
            Self::Store(_) => WaitForGraph::new(),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .controller()
                .wait_for_graph_since(&marker),
        }
    }

    pub(super) fn line_data(
        &self,
        target: MemoryTargetId,
        line: Address,
    ) -> Result<Vec<u8>, RiscvWorkloadReplayError> {
        match self {
            Self::Store(store) => store
                .lock()
                .expect("workload replay memory lock")
                .line_data(target, line)
                .map_err(RiscvWorkloadReplayError::Memory),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .controller()
                .line_data(target, line)
                .map_err(RiscvWorkloadReplayError::Dram),
        }
    }

    pub(super) fn insert_line(
        &self,
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    ) -> Result<(), RiscvWorkloadReplayError> {
        match self {
            Self::Store(store) => store
                .lock()
                .expect("workload replay memory lock")
                .insert_line(target, line, data)
                .map_err(RiscvWorkloadReplayError::Memory),
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .controller_mut()
                .insert_line(target, line, data)
                .map_err(RiscvWorkloadReplayError::Dram),
        }
    }

    pub(super) fn target_batch_response(
        &self,
        deliveries: Vec<RequestDelivery>,
    ) -> Option<Vec<TargetBatchOutcome>> {
        if deliveries.len() < 2 {
            return None;
        }
        let arrival_cycle = deliveries[0].tick();
        if deliveries
            .iter()
            .any(|delivery| delivery.tick() != arrival_cycle)
        {
            return None;
        }
        match self {
            Self::Store(_) => None,
            Self::Dram(dram) => dram
                .lock()
                .expect("workload replay DRAM lock")
                .accept_batch(arrival_cycle, &deliveries)
                .map(|result| result.expect("workload DRAM batch response")),
        }
    }
}

pub(super) fn memory_response(
    memory: &WorkloadMemoryBackend,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match memory {
        WorkloadMemoryBackend::Store(store) => {
            let response = store
                .lock()
                .expect("workload memory store lock")
                .respond(delivery.request())
                .expect("workload memory response")
                .response()
                .cloned()
                .expect("workload memory response payload");
            TargetOutcome::Respond(response)
        }
        WorkloadMemoryBackend::Dram(dram) => {
            let outcome = dram
                .lock()
                .expect("workload DRAM lock")
                .accept(delivery.tick(), delivery.request())
                .expect("workload DRAM response");
            dram_target_outcome(delivery.tick(), outcome)
        }
    }
}

fn dram_target_outcome(delivery_tick: Tick, outcome: DramMemoryOutcome) -> TargetOutcome {
    let Some(response) = outcome.response().cloned() else {
        return TargetOutcome::NoResponse;
    };
    let delay = outcome
        .ready_cycle()
        .checked_sub(delivery_tick)
        .expect("workload DRAM response is not ready before request arrival");
    if delay == 0 {
        TargetOutcome::Respond(response)
    } else {
        TargetOutcome::RespondAfter { delay, response }
    }
}

#[cfg(test)]
mod tests {
    use rem6_dram::{DramControllerConfig, DramGeometry, DramTiming};
    use rem6_fabric::{QosError, QosPriority, QosRequestorId};
    use rem6_memory::{
        AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId,
        MemoryTargetId,
    };
    use rem6_workload::WorkloadQosPolicy;

    use super::{DramMemoryController, DramMemoryError, WorkloadDramQosState};

    fn layout() -> CacheLineLayout {
        CacheLineLayout::new(64).unwrap()
    }

    fn controller() -> DramMemoryController {
        let target = MemoryTargetId::new(0);
        let mut controller = DramMemoryController::new();
        controller
            .add_target(DramControllerConfig::new(
                target,
                layout(),
                DramGeometry::new(4, 256, 64).unwrap(),
                DramTiming::new(3, 5, 7, 2, 4).unwrap(),
            ))
            .unwrap();
        controller
            .map_region(
                target,
                Address::new(0x0000),
                AccessSize::new(0x4000).unwrap(),
            )
            .unwrap();
        for address in [0x0000, 0x0040, 0x0080, 0x00c0] {
            controller
                .insert_line(target, Address::new(address), vec![0; 64])
                .unwrap();
        }
        controller
    }

    fn read(agent: u32, sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
        MemoryRequest::read_shared(
            MemoryRequestId::new(AgentId::new(agent), sequence),
            Address::new(address),
            AccessSize::new(bytes).unwrap(),
            layout(),
        )
        .unwrap()
    }

    #[test]
    fn workload_dram_qos_batch_failure_does_not_commit_proportional_fair_scores() {
        let mut controller = controller();
        let requestor_a = QosRequestorId::new(7);
        let requestor_b = QosRequestorId::new(8);
        let policy = WorkloadQosPolicy::proportional_fair(2, 1.0)
            .unwrap()
            .with_requestor_score(requestor_a, 100.0)
            .unwrap()
            .with_requestor_score(requestor_b, 1.0)
            .unwrap();
        let mut qos = WorkloadDramQosState::new(&policy);
        let failed_b = read(8, 1, 0x0000, 64);
        let failed_unknown = read(9, 2, 0x0040, 8);

        let error = qos
            .accept_requests(&mut controller, 0, [&failed_b, &failed_unknown])
            .unwrap_err();

        assert!(matches!(
            error,
            DramMemoryError::Qos {
                source: QosError::UnknownProportionalFairRequestor { requestor }
            } if requestor == QosRequestorId::new(9)
        ));

        let valid_a = read(7, 3, 0x0080, 8);
        let valid_b = read(8, 4, 0x00c0, 8);
        let outcomes = qos
            .accept_requests(&mut controller, 0, [&valid_a, &valid_b])
            .unwrap();

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].dram_access().request(), valid_b.id());
        assert_eq!(
            outcomes[0].dram_access().qos().unwrap().assigned_priority(),
            QosPriority::new(0)
        );
        assert_eq!(outcomes[1].dram_access().request(), valid_a.id());
        assert_eq!(
            outcomes[1].dram_access().qos().unwrap().assigned_priority(),
            QosPriority::new(1)
        );
    }
}
