use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_kernel::Tick;
use rem6_memory::{Address, CacheLineLayout};
use rem6_traffic::{
    TrafficControllerEvent, TrafficControllerEventBatch, TrafficStateGeneratorSnapshot,
    TrafficTraceReplaySource,
};
use rem6_workload::{HostEventIntent, WorkloadHostEvent};

use super::data_cache_backend::WorkloadDataCacheBackend;
use super::memory_backend::WorkloadMemoryBackend;
use super::{RiscvWorkloadReplayError, RiscvWorkloadTrafficTraceReplay};
use crate::workload_replay_host::PlannedHostDeliveryContext;
use crate::{GuestEventDelivery, SystemActionOutcome, SystemHostController};

#[derive(Clone, Copy)]
pub(super) struct PlannedDataCacheTraceOverlap {
    pub(super) action: &'static str,
    pub(super) tick: Tick,
    pub(super) line: Address,
}

pub(super) fn planned_host_data_cache_checkpoint_action(
    event: &WorkloadHostEvent,
) -> Option<&'static str> {
    match event.intent() {
        HostEventIntent::Checkpoint { .. } => Some("checkpoint"),
        HostEventIntent::RestoreCheckpoint { .. } => Some("restore checkpoint"),
        _ => None,
    }
}

pub(super) fn planned_data_cache_trace_overlap(
    replay: &RiscvWorkloadTrafficTraceReplay,
    planned_ticks: &BTreeMap<Tick, &'static str>,
    max_planned_tick: Tick,
    layout: CacheLineLayout,
    lines: &BTreeSet<Address>,
) -> Result<Option<PlannedDataCacheTraceOverlap>, RiscvWorkloadReplayError> {
    let mut controller = replay.controller().clone();
    let mut now = 0;
    let start_batch = controller
        .start(now)
        .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
    if let Some(overlap) =
        planned_data_cache_trace_overlap_in_batch(&start_batch, planned_ticks, layout, lines)
    {
        return Ok(Some(overlap));
    }

    let scan_limit = traffic_trace_controller_scan_limit(replay);
    for _ in 0..scan_limit {
        let Some(batch) = controller
            .next_event(now, replay.retry_delay())
            .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?
        else {
            return Ok(None);
        };
        let Some(batch_tick) = traffic_controller_event_batch_min_tick(&batch) else {
            continue;
        };
        if batch_tick > max_planned_tick {
            return Ok(None);
        }
        now = batch_tick;
        if let Some(overlap) =
            planned_data_cache_trace_overlap_in_batch(&batch, planned_ticks, layout, lines)
        {
            return Ok(Some(overlap));
        }
    }

    Ok(None)
}

fn traffic_trace_controller_scan_limit(replay: &RiscvWorkloadTrafficTraceReplay) -> usize {
    replay
        .controller()
        .snapshot()
        .generators()
        .iter()
        .map(|entry| match entry.generator() {
            TrafficStateGeneratorSnapshot::Trace(snapshot) => snapshot.config().trace().len(),
            _ => 0,
        })
        .sum::<usize>()
        .saturating_mul(4)
        .saturating_add(16)
}

fn planned_data_cache_trace_overlap_in_batch(
    batch: &TrafficControllerEventBatch,
    planned_ticks: &BTreeMap<Tick, &'static str>,
    layout: CacheLineLayout,
    lines: &BTreeSet<Address>,
) -> Option<PlannedDataCacheTraceOverlap> {
    batch.events().iter().find_map(|event| {
        let (tick, line) = data_cache_trace_event_line(event, layout, lines)?;
        let action = planned_ticks.get(&tick).copied()?;
        Some(PlannedDataCacheTraceOverlap { action, tick, line })
    })
}

fn traffic_controller_event_batch_min_tick(batch: &TrafficControllerEventBatch) -> Option<Tick> {
    batch
        .events()
        .iter()
        .filter_map(traffic_controller_event_tick)
        .min()
}

fn traffic_controller_event_tick(event: &TrafficControllerEvent) -> Option<Tick> {
    match event {
        TrafficControllerEvent::Request(request) => Some(request.tick()),
        TrafficControllerEvent::Transition(transition) => Some(transition.tick()),
        TrafficControllerEvent::Exit(exit) => Some(exit.tick()),
        TrafficControllerEvent::TraceExit(_) => None,
        TrafficControllerEvent::TraceSync(sync) => Some(sync.tick()),
        TrafficControllerEvent::TraceTlb(tlb) => Some(tlb.tick()),
        TrafficControllerEvent::TraceCache(cache) => Some(cache.tick()),
        TrafficControllerEvent::TraceHtm(htm) => Some(htm.tick()),
        TrafficControllerEvent::TraceDiagnostic(diagnostic) => Some(diagnostic.tick()),
        TrafficControllerEvent::TraceResponse(response) => Some(response.tick()),
        TrafficControllerEvent::TraceError(error) => Some(error.tick()),
        TrafficControllerEvent::TraceResponseMatch(response) => Some(response.response().tick()),
        TrafficControllerEvent::TraceErrorMatch(error) => Some(error.error().tick()),
        TrafficControllerEvent::TraceReplayAction(action) => Some(action.tick()),
    }
}

fn data_cache_trace_event_line(
    event: &TrafficControllerEvent,
    layout: CacheLineLayout,
    lines: &BTreeSet<Address>,
) -> Option<(Tick, Address)> {
    match event {
        TrafficControllerEvent::Request(request) => address_span_overlapping_line(
            request.address(),
            Some(request.request().size().bytes()),
            layout,
            lines,
        )
        .map(|line| (request.tick(), line)),
        TrafficControllerEvent::TraceCache(cache) => {
            address_span_overlapping_line(cache.address(), Some(cache.size_bytes()), layout, lines)
                .map(|line| (cache.tick(), line))
        }
        TrafficControllerEvent::TraceHtm(htm) => {
            address_span_overlapping_line(htm.address()?, htm.size_bytes(), layout, lines)
                .map(|line| (htm.tick(), line))
        }
        TrafficControllerEvent::TraceDiagnostic(diagnostic) => address_span_overlapping_line(
            diagnostic.address()?,
            diagnostic.size_bytes(),
            layout,
            lines,
        )
        .map(|line| (diagnostic.tick(), line)),
        TrafficControllerEvent::TraceResponse(response) => {
            address_span_overlapping_line(response.address()?, response.size_bytes(), layout, lines)
                .map(|line| (response.tick(), line))
        }
        TrafficControllerEvent::TraceError(error) => {
            address_span_overlapping_line(error.address()?, error.size_bytes(), layout, lines)
                .map(|line| (error.tick(), line))
        }
        TrafficControllerEvent::TraceResponseMatch(response) => {
            trace_replay_source_line(response.source(), layout, lines)
                .map(|line| (response.response().tick(), line))
        }
        TrafficControllerEvent::TraceErrorMatch(error) => {
            trace_replay_source_line(error.source(), layout, lines)
                .map(|line| (error.error().tick(), line))
        }
        _ => None,
    }
}

fn trace_replay_source_line(
    source: &TrafficTraceReplaySource,
    layout: CacheLineLayout,
    lines: &BTreeSet<Address>,
) -> Option<Address> {
    match source {
        TrafficTraceReplaySource::Memory(request) => address_span_overlapping_line(
            request.address(),
            Some(request.request().size().bytes()),
            layout,
            lines,
        ),
        TrafficTraceReplaySource::Cache(cache) => {
            address_span_overlapping_line(cache.address(), Some(cache.size_bytes()), layout, lines)
        }
        TrafficTraceReplaySource::Htm(htm) => {
            address_span_overlapping_line(htm.address()?, htm.size_bytes(), layout, lines)
        }
        TrafficTraceReplaySource::Diagnostic(diagnostic) => address_span_overlapping_line(
            diagnostic.address()?,
            diagnostic.size_bytes(),
            layout,
            lines,
        ),
        TrafficTraceReplaySource::Sync(_) | TrafficTraceReplaySource::Tlb(_) => None,
    }
}

fn address_span_overlapping_line(
    address: Address,
    size_bytes: Option<u64>,
    layout: CacheLineLayout,
    lines: &BTreeSet<Address>,
) -> Option<Address> {
    let size = size_bytes.unwrap_or(1).max(1);
    let start = address.get();
    let end = start.saturating_add(size);
    lines.iter().copied().find(|line| {
        let line_start = line.get();
        let line_end = line_start.saturating_add(layout.bytes());
        start < line_end && end > line_start
    })
}

pub(super) fn planned_host_data_cache_sync_handler(
    memory: WorkloadMemoryBackend,
    data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    errors: Arc<Mutex<Vec<RiscvWorkloadReplayError>>>,
) -> impl for<'a> Fn(
    &WorkloadHostEvent,
    GuestEventDelivery,
    &Arc<Mutex<SystemHostController>>,
    PlannedHostDeliveryContext<'a>,
) + Clone
       + Send
       + Sync
       + 'static {
    move |event, delivery, controller, delivery_context| match event.intent() {
        HostEventIntent::Checkpoint { .. } => {
            if let Err(error) =
                sync_data_cache_lines_to_memory(data_cache.as_ref(), &memory, delivery.tick())
            {
                record_planned_host_data_cache_sync_error(&errors, error);
                return;
            }
            delivery_context.deliver(delivery, controller);
        }
        HostEventIntent::RestoreCheckpoint { .. } => {
            let outcomes = delivery_context.deliver(delivery, controller);
            let restored = outcomes
                .iter()
                .any(|outcome| matches!(outcome, SystemActionOutcome::CheckpointRestored { .. }));
            if restored {
                if let Err(error) = sync_data_cache_lines_from_memory(data_cache.as_ref(), &memory)
                {
                    record_planned_host_data_cache_sync_error(&errors, error);
                }
            }
        }
        _ => {
            delivery_context.deliver(delivery, controller);
        }
    }
}

fn sync_data_cache_lines_to_memory(
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    memory: &WorkloadMemoryBackend,
    tick: Tick,
) -> Result<(), RiscvWorkloadReplayError> {
    let Some(data_cache) = data_cache else {
        return Ok(());
    };
    let final_lines = data_cache
        .lock()
        .expect("workload data cache lock")
        .final_lines(tick)?;
    for (target, line, data) in final_lines {
        memory.insert_line(target, line, data)?;
    }
    Ok(())
}

fn sync_data_cache_lines_from_memory(
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    memory: &WorkloadMemoryBackend,
) -> Result<(), RiscvWorkloadReplayError> {
    let Some(data_cache) = data_cache else {
        return Ok(());
    };
    let line_locations = data_cache
        .lock()
        .expect("workload data cache lock")
        .line_locations();
    for (target, line) in line_locations {
        let data = memory.line_data(target, line)?;
        let replaced = data_cache
            .lock()
            .expect("workload data cache lock")
            .functional_replace_line_data(target, line, data)?;
        debug_assert!(
            replaced,
            "data cache line location came from the same backend"
        );
    }
    Ok(())
}

fn record_planned_host_data_cache_sync_error(
    errors: &Arc<Mutex<Vec<RiscvWorkloadReplayError>>>,
    error: RiscvWorkloadReplayError,
) {
    errors
        .lock()
        .expect("planned host data cache sync error lock")
        .push(error);
}

pub(super) fn take_planned_host_data_cache_sync_error(
    errors: &Arc<Mutex<Vec<RiscvWorkloadReplayError>>>,
) -> Option<RiscvWorkloadReplayError> {
    let mut errors = errors
        .lock()
        .expect("planned host data cache sync error lock");
    (!errors.is_empty()).then(|| errors.remove(0))
}
