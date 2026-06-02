use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse, TranslationRequestId,
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
        }
    }
}

impl Error for RiscvSv39MemoryWalkerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Walk(error) => Some(error),
            Self::Frontend(error) => Some(error),
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
