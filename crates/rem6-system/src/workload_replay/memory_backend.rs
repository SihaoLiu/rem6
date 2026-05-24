use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_dram::{
    DramMemoryController, DramMemoryError, DramMemoryOutcome, DramMemorySnapshot,
    DramMemoryWaitForMarker, DramQosRequest, DramQosSchedulingPolicy, DramTargetActivity,
};
use rem6_fabric::{QosFixedPriorityPolicy, QosQueueArbiter, QosRequestorId};
use rem6_kernel::{Tick, WaitForGraph};
use rem6_memory::{
    Address, MemoryRequest, MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};
use rem6_transport::{RequestDelivery, TargetBatchOutcome, TargetOutcome};
use rem6_workload::WorkloadQosPolicy;

use super::qos::{dram_scheduling_policy, fixed_priority_policy, queue_arbiter};
use super::RiscvWorkloadReplayError;

#[derive(Clone)]
pub(super) enum WorkloadMemoryBackend {
    Store(Arc<Mutex<PartitionedMemoryStore>>),
    Dram(Arc<Mutex<WorkloadDramBackend>>),
}

#[derive(Clone, Debug)]
struct WorkloadDramQosState {
    priority_policy: QosFixedPriorityPolicy,
    arbiter: QosQueueArbiter,
    scheduling_policy: DramQosSchedulingPolicy,
    next_order: u64,
}

impl WorkloadDramQosState {
    fn new(policy: &WorkloadQosPolicy) -> Self {
        Self {
            priority_policy: fixed_priority_policy(policy),
            arbiter: queue_arbiter(policy),
            scheduling_policy: dram_scheduling_policy(policy),
            next_order: 0,
        }
    }

    fn qos_request<'a>(&mut self, request: &'a MemoryRequest) -> DramQosRequest<'a> {
        let requestor = QosRequestorId::new(request.id().agent().get());
        let priority = self
            .priority_policy
            .priority_for(requestor, request.size().bytes());
        let order = self.next_order;
        self.next_order = self
            .next_order
            .checked_add(1)
            .expect("workload DRAM QoS order does not overflow");
        DramQosRequest::new(request, priority, order)
    }

    fn accept(
        &mut self,
        controller: &mut DramMemoryController,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let request = self.qos_request(request);
        Ok(controller
            .accept_qos_batch_with_policy(
                arrival_cycle,
                [request],
                &mut self.arbiter,
                self.scheduling_policy,
            )?
            .pop()
            .expect("single workload DRAM QoS request returns one outcome"))
    }

    fn accept_batch(
        &mut self,
        controller: &mut DramMemoryController,
        arrival_cycle: u64,
        deliveries: &[RequestDelivery],
    ) -> Result<Vec<DramMemoryOutcome>, DramMemoryError> {
        let requests = deliveries
            .iter()
            .map(|delivery| self.qos_request(delivery.request()))
            .collect::<Vec<_>>();
        controller.accept_qos_batch_with_policy(
            arrival_cycle,
            requests,
            &mut self.arbiter,
            self.scheduling_policy,
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
