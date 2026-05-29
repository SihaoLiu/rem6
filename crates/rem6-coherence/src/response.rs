use std::sync::{Arc, Mutex};

use rem6_cache::{CacheControllerResultKind, MshrQosClass};
use rem6_directory::DirectoryDecision;
use rem6_memory::{MemoryRequestId, MemoryResponse, ResponseStatus};
use rem6_transport::TargetOutcome;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitKind {
    ImmediateHit,
    ScheduledMiss,
    CoalescedMiss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitResult {
    kind: SubmitKind,
    cache_result: CacheControllerResultKind,
    directory_decision: Option<DirectoryDecision>,
    cache_mshr_effective_qos: Option<MshrQosClass>,
}

impl SubmitResult {
    pub(crate) fn new(kind: SubmitKind, cache_result: CacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
            cache_mshr_effective_qos: None,
        }
    }

    pub(crate) fn with_directory_decision(mut self, decision: DirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub(crate) fn with_cache_mshr_effective_qos(mut self, qos: Option<MshrQosClass>) -> Self {
        self.cache_mshr_effective_qos = qos;
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> CacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&DirectoryDecision> {
        self.directory_decision.as_ref()
    }

    pub const fn cache_mshr_effective_qos(&self) -> Option<MshrQosClass> {
        self.cache_mshr_effective_qos
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuResponseRecord {
    tick: u64,
    cache_result: CacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl CpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: CacheControllerResultKind,
        request: MemoryRequestId,
        status: ResponseStatus,
        data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            tick,
            cache_result,
            request,
            status,
            data,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn cache_result(&self) -> CacheControllerResultKind {
        self.cache_result
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

fn response_record(
    tick: u64,
    cache_result: CacheControllerResultKind,
    response: &MemoryResponse,
) -> CpuResponseRecord {
    CpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

pub(crate) fn push_response_records_from_outcomes(
    responses: &mut Vec<CpuResponseRecord>,
    tick: u64,
    cache_result: CacheControllerResultKind,
    outcomes: &[TargetOutcome],
) -> usize {
    let before = responses.len();
    for outcome in outcomes {
        if let TargetOutcome::Respond(response) = outcome {
            responses.push(response_record(tick, cache_result, response));
        }
    }
    responses.len() - before
}

pub(crate) fn push_locked_response_records_from_outcomes(
    responses: &Arc<Mutex<Vec<CpuResponseRecord>>>,
    tick: u64,
    cache_result: CacheControllerResultKind,
    outcomes: &[TargetOutcome],
) -> usize {
    let mut responses = responses.lock().expect("response lock");
    push_response_records_from_outcomes(&mut responses, tick, cache_result, outcomes)
}
