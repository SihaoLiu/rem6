use rem6_fabric::{QosPriority, QosQueueArbiter};
use rem6_memory::{MemoryError, MemoryOperation, MemoryRequest, MemoryTargetId};

use crate::{
    DramMemoryController, DramMemoryError, DramMemoryOutcome, DramQosRequest,
    DramQosSchedulingPolicy,
};

impl DramMemoryController {
    pub fn accept(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let target = self
            .store
            .decode_request(request)
            .map_err(DramMemoryError::Memory)?;
        self.preflight_storage(target, request)
            .map_err(DramMemoryError::Memory)?;
        let dram_access = self
            .dram
            .get_mut(&target)
            .expect("DRAM target is inserted with memory target")
            .schedule(arrival_cycle, request)
            .map_err(|source| DramMemoryError::Dram { target, source })?;
        let response = self
            .store
            .respond(request)
            .map_err(DramMemoryError::Memory)?
            .response()
            .cloned();

        Ok(DramMemoryOutcome::new(target, dram_access, response))
    }

    pub fn accept_qos_with_policy(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
        priority: QosPriority,
        order: u64,
        arbiter: &mut QosQueueArbiter,
        policy: DramQosSchedulingPolicy,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let target = self
            .store
            .decode_request(request)
            .map_err(DramMemoryError::Memory)?;
        self.preflight_storage(target, request)
            .map_err(DramMemoryError::Memory)?;
        let mut accesses = self
            .dram
            .get_mut(&target)
            .expect("DRAM target is inserted with memory target")
            .schedule_qos_batch_with_policy(
                arrival_cycle,
                [DramQosRequest::new(request, priority, order)],
                arbiter,
                policy,
            )
            .map_err(|source| DramMemoryError::Dram { target, source })?;
        let response = self
            .store
            .respond(request)
            .map_err(DramMemoryError::Memory)?
            .response()
            .cloned();
        let dram_access = accesses
            .pop()
            .expect("single DRAM QoS request returns one access");

        Ok(DramMemoryOutcome::new(target, dram_access, response))
    }

    fn preflight_storage(
        &self,
        target: MemoryTargetId,
        request: &MemoryRequest,
    ) -> Result<(), MemoryError> {
        if request.line_span() != 1 {
            return Err(MemoryError::CrossLineAccess {
                request: request.id(),
                start: request.range().start(),
                size: request.size(),
                line_size: request.line_layout().bytes(),
            });
        }

        if matches!(
            request.operation(),
            MemoryOperation::WritebackClean | MemoryOperation::WritebackDirty
        ) {
            return Ok(());
        }

        self.store
            .line_data(target, request.line_address())
            .map(|_| ())
    }
}
