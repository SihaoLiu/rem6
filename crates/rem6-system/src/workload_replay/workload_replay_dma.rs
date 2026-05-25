use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_accelerator::{AcceleratorCommandId, AcceleratorDmaCopy, AcceleratorEngineId};
use rem6_kernel::{PartitionedScheduler, Tick, WaitForGraph};
use rem6_memory::{
    AccessSize, AddressRange, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId,
};
use rem6_transport::{MemoryRouteId, MemoryTrace, MemoryTransport};
use rem6_workload::{WorkloadAcceleratorDmaCopy, WorkloadError, WorkloadRouteId, WorkloadTopology};

use super::{
    cached_memory_response, dma_scheduler_batch_worker_count_ticks, merge_dma_scheduler_run,
    RiscvWorkloadReplayError, WorkloadDataCacheBackend, WorkloadMemoryBackend,
};
use crate::workload_replay_heterogeneous::{
    accelerator_snapshots, merge_wait_for_graph, WorkloadAcceleratorRuntime,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkloadAcceleratorDmaActivity {
    pub(super) copy_count: usize,
    pub(super) completion_count: usize,
    pub(super) active_device_count: usize,
    pub(super) scheduler_epoch_count: usize,
    pub(super) scheduler_dispatch_count: usize,
    pub(super) scheduler_batch_count: usize,
    pub(super) scheduler_batch_worker_count_ticks: Vec<(usize, Tick)>,
    pub(super) wait_for_edge_count: usize,
    pub(super) deadlock_diagnostic_count: usize,
}

impl WorkloadAcceleratorDmaActivity {
    fn with_wait_for_graph(mut self, wait_for: WaitForGraph) -> Self {
        self.wait_for_edge_count = wait_for.edge_count();
        self.deadlock_diagnostic_count = wait_for.deadlock_diagnostic().into_iter().count();
        self
    }
}

pub(super) fn run_accelerator_dma_copies(
    topology: &WorkloadTopology,
    route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
    devices: &BTreeMap<AcceleratorEngineId, WorkloadAcceleratorRuntime>,
    transport: &MemoryTransport,
    memory: &WorkloadMemoryBackend,
    data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
) -> Result<WorkloadAcceleratorDmaActivity, RiscvWorkloadReplayError> {
    if topology.accelerator_dma_copies().is_empty() {
        return Ok(WorkloadAcceleratorDmaActivity::default());
    }

    let fabric_wait_for_start = transport.mark_fabric_wait_for();
    let dram_wait_for_start = memory.mark_dram_wait_for();
    let before = accelerator_snapshots(devices);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(
        topology.partition_count(),
        topology.min_remote_delay(),
        topology.parallel_worker_limit(),
    )
    .map_err(RiscvWorkloadReplayError::Scheduler)?;
    let mut scheduler_epoch_count = 0;
    let mut scheduler_dispatch_count = 0;
    let mut scheduler_batch_count = 0;
    let mut scheduler_batch_worker_count_ticks = BTreeMap::<usize, Tick>::new();

    for copy in topology.accelerator_dma_copies() {
        let engine = AcceleratorEngineId::new(copy.engine());
        let runtime = devices.get(&engine).ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingAcceleratorDevice {
                engine: copy.engine(),
            })
        })?;
        let dma = accelerator_dma_copy(topology, route_map, copy)?;
        let read_memory = memory.clone();
        let read_data_cache = data_cache.clone();
        runtime
            .engine
            .submit_dma_copy_read(
                &mut scheduler,
                transport,
                dma,
                MemoryTrace::new(),
                move |delivery, _context| {
                    cached_memory_response(read_data_cache.as_ref(), &read_memory, &delivery)
                },
            )
            .map_err(RiscvWorkloadReplayError::Accelerator)?;
    }
    let read_run = scheduler
        .run_until_idle_parallel_recorded()
        .map_err(RiscvWorkloadReplayError::Scheduler)?;
    merge_dma_scheduler_run(
        &read_run,
        &mut scheduler_epoch_count,
        &mut scheduler_dispatch_count,
        &mut scheduler_batch_count,
        &mut scheduler_batch_worker_count_ticks,
    );

    for copy in topology.accelerator_dma_copies() {
        let engine = AcceleratorEngineId::new(copy.engine());
        let runtime = devices.get(&engine).ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingAcceleratorDevice {
                engine: copy.engine(),
            })
        })?;
        let write_memory = memory.clone();
        let write_data_cache = data_cache.clone();
        if runtime
            .engine
            .issue_next_dma_write(
                &mut scheduler,
                transport,
                MemoryTrace::new(),
                move |delivery, _context| {
                    cached_memory_response(write_data_cache.as_ref(), &write_memory, &delivery)
                },
            )
            .map_err(RiscvWorkloadReplayError::Accelerator)?
            .is_none()
        {
            return Err(RiscvWorkloadReplayError::MissingAcceleratorDmaWrite { engine });
        }
    }
    let write_run = scheduler
        .run_until_idle_parallel_recorded()
        .map_err(RiscvWorkloadReplayError::Scheduler)?;
    merge_dma_scheduler_run(
        &write_run,
        &mut scheduler_epoch_count,
        &mut scheduler_dispatch_count,
        &mut scheduler_batch_count,
        &mut scheduler_batch_worker_count_ticks,
    );

    let after = accelerator_snapshots(devices);
    let mut active_device_count = 0;
    let mut completion_count = 0;
    for (engine, after) in &after {
        let Some(before) = before.get(engine) else {
            continue;
        };
        let device_completions = after
            .dma_completions()
            .len()
            .saturating_sub(before.dma_completions().len());
        if device_completions != 0 {
            active_device_count += 1;
        }
        completion_count += device_completions;
    }

    let mut wait_for = memory.dram_wait_for_since(dram_wait_for_start);
    if let Some(fabric_wait_for) =
        fabric_wait_for_start.and_then(|marker| transport.fabric_wait_for_graph_since(marker))
    {
        merge_wait_for_graph(&mut wait_for, fabric_wait_for);
    }

    Ok(WorkloadAcceleratorDmaActivity {
        copy_count: topology.accelerator_dma_copies().len(),
        completion_count,
        active_device_count,
        scheduler_epoch_count,
        scheduler_dispatch_count,
        scheduler_batch_count,
        scheduler_batch_worker_count_ticks: dma_scheduler_batch_worker_count_ticks(
            scheduler_batch_worker_count_ticks,
        ),
        wait_for_edge_count: 0,
        deadlock_diagnostic_count: 0,
    }
    .with_wait_for_graph(wait_for))
}

fn accelerator_dma_copy(
    topology: &WorkloadTopology,
    route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
    copy: &WorkloadAcceleratorDmaCopy,
) -> Result<AcceleratorDmaCopy, RiscvWorkloadReplayError> {
    let route = route_map.get(copy.route()).copied().ok_or_else(|| {
        RiscvWorkloadReplayError::MissingRoute {
            route: copy.route().clone(),
        }
    })?;
    let size = AccessSize::new(copy.bytes()).map_err(RiscvWorkloadReplayError::Memory)?;
    let layout = accelerator_dma_layout(topology, copy, size)?;
    let read_sequence = copy.transfer().checked_mul(2).ok_or(
        RiscvWorkloadReplayError::AcceleratorDmaRequestSequenceOverflow {
            transfer: copy.transfer(),
        },
    )?;
    let write_sequence = read_sequence.checked_add(1).ok_or(
        RiscvWorkloadReplayError::AcceleratorDmaRequestSequenceOverflow {
            transfer: copy.transfer(),
        },
    )?;
    let read_request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(copy.agent()), read_sequence),
        copy.source(),
        size,
        layout,
    )
    .map_err(RiscvWorkloadReplayError::Memory)?;

    AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(copy.transfer()),
        route,
        read_request,
        route,
        MemoryRequestId::new(AgentId::new(copy.agent()), write_sequence),
        copy.destination(),
    )
    .map_err(RiscvWorkloadReplayError::Accelerator)
}

fn accelerator_dma_layout(
    topology: &WorkloadTopology,
    copy: &WorkloadAcceleratorDmaCopy,
    size: AccessSize,
) -> Result<CacheLineLayout, RiscvWorkloadReplayError> {
    let source_range =
        AddressRange::new(copy.source(), size).map_err(RiscvWorkloadReplayError::Memory)?;
    let destination_range =
        AddressRange::new(copy.destination(), size).map_err(RiscvWorkloadReplayError::Memory)?;
    let target = topology
        .memory_targets()
        .iter()
        .find(|target| {
            target.range().contains_range(source_range)
                && target.range().contains_range(destination_range)
        })
        .ok_or(RiscvWorkloadReplayError::MissingMemoryTarget)?;
    CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)
}
