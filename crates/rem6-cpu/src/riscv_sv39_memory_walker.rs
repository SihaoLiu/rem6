use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, PartitionedScheduler};
use rem6_memory::{
    CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse, TranslationRequestId,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome, TransportError,
};

use crate::riscv_translation::{
    RiscvSv39MemoryWalk, RiscvSv39MemoryWalkAdvance, RiscvSv39MemoryWalkError,
    RiscvSv39TranslationResult,
};
use crate::{
    CpuTranslationFrontend, CpuTranslationFrontendError, CpuTranslationOutcome,
    CpuTranslationRequest,
};

const RISCV_SV39_MAX_PTE_READS_PER_WALK: u64 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSv39MemoryWalker {
    root_table_ppn: u64,
    next_pte_request: MemoryRequestId,
    line_layout: CacheLineLayout,
    active_reads: BTreeMap<MemoryRequestId, RiscvSv39MemoryWalk>,
    pending_responses: VecDeque<MemoryResponse>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39MemoryWalkerAdvance {
    ReadPte(MemoryRequest),
    Complete(CpuTranslationOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39MemoryWalkerError {
    PteRequestSequenceOverflow { first: MemoryRequestId },
    UnexpectedResponse { response: MemoryRequestId },
    Walk(RiscvSv39MemoryWalkError),
    Frontend(CpuTranslationFrontendError),
    Transport(TransportError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSv39MemoryWalkerParallelSubmission {
    events: Vec<PartitionEventId>,
    completions: Vec<CpuTranslationOutcome>,
}

impl RiscvSv39MemoryWalkerParallelSubmission {
    pub fn new(events: Vec<PartitionEventId>, completions: Vec<CpuTranslationOutcome>) -> Self {
        Self {
            events,
            completions,
        }
    }

    pub fn events(&self) -> &[PartitionEventId] {
        &self.events
    }

    pub fn completions(&self) -> &[CpuTranslationOutcome] {
        &self.completions
    }
}

impl fmt::Display for RiscvSv39MemoryWalkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PteRequestSequenceOverflow { first } => write!(
                formatter,
                "Sv39 memory walker PTE request sequence starting at {} from agent {} overflows",
                first.sequence(),
                first.agent().get()
            ),
            Self::UnexpectedResponse { response } => write!(
                formatter,
                "Sv39 memory walker has no active PTE request {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::Walk(error) => write!(formatter, "{error}"),
            Self::Frontend(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvSv39MemoryWalkerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Walk(error) => Some(error),
            Self::Frontend(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RiscvSv39MemoryWalkError> for RiscvSv39MemoryWalkerError {
    fn from(error: RiscvSv39MemoryWalkError) -> Self {
        Self::Walk(error)
    }
}

impl From<CpuTranslationFrontendError> for RiscvSv39MemoryWalkerError {
    fn from(error: CpuTranslationFrontendError) -> Self {
        Self::Frontend(error)
    }
}

impl From<TransportError> for RiscvSv39MemoryWalkerError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

impl RiscvSv39MemoryWalker {
    pub fn new(
        root_table_ppn: u64,
        first_pte_request: MemoryRequestId,
        line_layout: CacheLineLayout,
    ) -> Self {
        Self {
            root_table_ppn,
            next_pte_request: first_pte_request,
            line_layout,
            active_reads: BTreeMap::new(),
            pending_responses: VecDeque::new(),
        }
    }

    pub const fn root_table_ppn(&self) -> u64 {
        self.root_table_ppn
    }

    pub const fn next_pte_request(&self) -> MemoryRequestId {
        self.next_pte_request
    }

    pub fn active_count(&self) -> usize {
        self.active_reads.len()
    }

    pub fn is_idle(&self) -> bool {
        self.active_reads.is_empty()
    }

    pub fn pending_response_count(&self) -> usize {
        self.pending_responses.len()
    }

    pub fn start_ready(
        &mut self,
        frontend: &mut CpuTranslationFrontend,
        tick: u64,
    ) -> Result<Vec<RiscvSv39MemoryWalkerAdvance>, RiscvSv39MemoryWalkerError> {
        let requests = frontend
            .ready_cpu_requests(tick)
            .into_iter()
            .filter(|request| !self.has_active_translation(request.translation_id()))
            .collect::<Vec<_>>();
        let pte_request_bases = self.reserve_pte_request_bases(requests.len())?;
        let mut advances = Vec::with_capacity(requests.len());
        for (request, first_pte_request) in requests.into_iter().zip(pte_request_bases) {
            advances.push(self.start_request(frontend, request, first_pte_request)?);
        }
        Ok(advances)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_ready_parallel<F>(
        walker: Arc<Mutex<Self>>,
        frontend: &mut CpuTranslationFrontend,
        tick: u64,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        route: MemoryRouteId,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<RiscvSv39MemoryWalkerParallelSubmission, RiscvSv39MemoryWalkerError>
    where
        F: Fn(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + Sync
            + 'static,
    {
        let walker_snapshot = walker.lock().expect("Sv39 memory walker lock").clone();
        let frontend_snapshot = frontend.clone();
        let advances = walker
            .lock()
            .expect("Sv39 memory walker lock")
            .start_ready(frontend, tick)?;
        match Self::submit_parallel_advances(
            &walker, advances, scheduler, transport, route, trace, responder,
        ) {
            Ok(submission) => Ok(submission),
            Err(error) => {
                *walker.lock().expect("Sv39 memory walker lock") = walker_snapshot;
                *frontend = frontend_snapshot;
                Err(error)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_next_response_parallel<F>(
        walker: Arc<Mutex<Self>>,
        frontend: &mut CpuTranslationFrontend,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        route: MemoryRouteId,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<RiscvSv39MemoryWalkerParallelSubmission, RiscvSv39MemoryWalkerError>
    where
        F: Fn(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + Sync
            + 'static,
    {
        let walker_snapshot = walker.lock().expect("Sv39 memory walker lock").clone();
        let frontend_snapshot = frontend.clone();
        let advances = walker
            .lock()
            .expect("Sv39 memory walker lock")
            .advance_next_response(frontend)?
            .into_iter()
            .collect::<Vec<_>>();
        match Self::submit_parallel_advances(
            &walker, advances, scheduler, transport, route, trace, responder,
        ) {
            Ok(submission) => Ok(submission),
            Err(error) => {
                *walker.lock().expect("Sv39 memory walker lock") = walker_snapshot;
                *frontend = frontend_snapshot;
                Err(error)
            }
        }
    }

    fn submit_parallel_advances<F>(
        walker: &Arc<Mutex<Self>>,
        advances: Vec<RiscvSv39MemoryWalkerAdvance>,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        route: MemoryRouteId,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<RiscvSv39MemoryWalkerParallelSubmission, RiscvSv39MemoryWalkerError>
    where
        F: Fn(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + Sync
            + 'static,
    {
        let mut completions = Vec::new();
        let mut transactions = Vec::new();
        let responder = Arc::new(responder);
        for advance in advances {
            match advance {
                RiscvSv39MemoryWalkerAdvance::ReadPte(request) => {
                    let response_walker = Arc::clone(walker);
                    let responder = Arc::clone(&responder);
                    transactions.push(ParallelMemoryTransaction::new(
                        route,
                        request,
                        trace.clone(),
                        move |delivery, context| responder(delivery, context),
                        move |delivery| {
                            response_walker
                                .lock()
                                .expect("Sv39 memory walker lock")
                                .record_response(delivery);
                        },
                    ));
                }
                RiscvSv39MemoryWalkerAdvance::Complete(outcome) => {
                    completions.push(outcome);
                }
            }
        }

        let events = transport.submit_parallel_batch(scheduler, transactions)?;
        Ok(RiscvSv39MemoryWalkerParallelSubmission::new(
            events,
            completions,
        ))
    }

    pub fn record_response(&mut self, delivery: ResponseDelivery) {
        self.record_memory_response(delivery.response().clone());
    }

    pub fn record_memory_response(&mut self, response: MemoryResponse) {
        self.pending_responses.push_back(response);
    }

    pub fn advance_next_response(
        &mut self,
        frontend: &mut CpuTranslationFrontend,
    ) -> Result<Option<RiscvSv39MemoryWalkerAdvance>, RiscvSv39MemoryWalkerError> {
        let Some(response) = self.pending_responses.pop_front() else {
            return Ok(None);
        };
        match self.advance(frontend, &response) {
            Ok(advance) => Ok(Some(advance)),
            Err(error @ RiscvSv39MemoryWalkerError::Frontend(_)) => {
                self.pending_responses.push_front(response);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    pub fn advance(
        &mut self,
        frontend: &mut CpuTranslationFrontend,
        response: &MemoryResponse,
    ) -> Result<RiscvSv39MemoryWalkerAdvance, RiscvSv39MemoryWalkerError> {
        let response_id = response.request_id();
        let walk = self.active_reads.remove(&response_id).ok_or(
            RiscvSv39MemoryWalkerError::UnexpectedResponse {
                response: response_id,
            },
        )?;

        match walk.clone().advance(response) {
            Ok(RiscvSv39MemoryWalkAdvance::ReadPte(next_walk)) => {
                let request = next_walk.pte_request().clone();
                self.active_reads.insert(request.id(), next_walk);
                Ok(RiscvSv39MemoryWalkerAdvance::ReadPte(request))
            }
            Ok(RiscvSv39MemoryWalkAdvance::Complete(result)) => {
                match complete_frontend_with_result(frontend, result) {
                    Ok(outcome) => Ok(RiscvSv39MemoryWalkerAdvance::Complete(outcome)),
                    Err(error) => {
                        self.active_reads.insert(response_id, walk);
                        Err(error.into())
                    }
                }
            }
            Err(error) => {
                self.active_reads.insert(response_id, walk);
                Err(error.into())
            }
        }
    }

    fn start_request(
        &mut self,
        frontend: &mut CpuTranslationFrontend,
        request: CpuTranslationRequest,
        first_pte_request: MemoryRequestId,
    ) -> Result<RiscvSv39MemoryWalkerAdvance, RiscvSv39MemoryWalkerError> {
        match RiscvSv39MemoryWalk::start(
            request.clone(),
            self.root_table_ppn,
            first_pte_request,
            self.line_layout,
        )? {
            RiscvSv39MemoryWalkAdvance::ReadPte(walk) => {
                let pte_request = walk.pte_request().clone();
                self.active_reads.insert(pte_request.id(), walk);
                Ok(RiscvSv39MemoryWalkerAdvance::ReadPte(pte_request))
            }
            RiscvSv39MemoryWalkAdvance::Complete(result) => {
                let outcome = complete_frontend_with_result(frontend, result)?;
                Ok(RiscvSv39MemoryWalkerAdvance::Complete(outcome))
            }
        }
    }

    fn has_active_translation(&self, request: TranslationRequestId) -> bool {
        self.active_reads
            .values()
            .any(|walk| walk.translation_request().translation_id() == request)
    }

    fn reserve_pte_request_bases(
        &mut self,
        count: usize,
    ) -> Result<Vec<MemoryRequestId>, RiscvSv39MemoryWalkerError> {
        let mut current = self.next_pte_request;
        let mut bases = Vec::with_capacity(count);
        for _ in 0..count {
            let first = current;
            bases.push(first);
            let next_sequence = first
                .sequence()
                .checked_add(RISCV_SV39_MAX_PTE_READS_PER_WALK)
                .ok_or(RiscvSv39MemoryWalkerError::PteRequestSequenceOverflow { first })?;
            current = MemoryRequestId::new(first.agent(), next_sequence);
        }
        self.next_pte_request = current;
        Ok(bases)
    }
}

fn complete_frontend_with_result(
    frontend: &mut CpuTranslationFrontend,
    result: RiscvSv39TranslationResult,
) -> Result<CpuTranslationOutcome, CpuTranslationFrontendError> {
    frontend.complete(
        translation_id_from_result(&result),
        result.resolution().clone(),
    )
}

fn translation_id_from_result(result: &RiscvSv39TranslationResult) -> TranslationRequestId {
    match result.outcome() {
        CpuTranslationOutcome::Mapped(mapped) => mapped.translation_id(),
        CpuTranslationOutcome::Fault(fault) => fault.translation_id(),
    }
}
