use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_accelerator::{
    AcceleratorDmaCopy, AcceleratorDmaIssueRecord, AcceleratorDmaWriteRollback,
    AcceleratorEngineId, AcceleratorEngineSnapshot,
};
use rem6_gpu::{
    GpuDeviceId, GpuDeviceSnapshot, GpuDmaCopy, GpuDmaIssueRecord, GpuDmaWriteRollback,
};
use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, RecordedConservativeRunSummary};
use rem6_transport::{MemoryTrace, ParallelMemoryTransaction};

use super::coherence_data::{
    merge_chi_data_cache_activity, merge_mesi_data_cache_activity, merge_moesi_data_cache_activity,
    merge_msi_data_cache_activity, RiscvTopologyCachedDataCaches, RiscvTopologyChiDataCache,
    RiscvTopologyMesiDataCache, RiscvTopologyMoesiDataCache, RiscvTopologyMsiDataCache,
};
use super::{
    dram_activities_since, dram_wait_for_since, mark_dram_activity, mark_dram_wait_for,
    take_memory_error, topology_cached_memory_response, RiscvTopologyDmaCopy,
    RiscvTopologyDmaDeviceActivity, RiscvTopologyDmaRunSummary, RiscvTopologyDmaStageRunSummary,
    RiscvTopologySystem, RiscvTopologySystemError,
};

#[derive(Clone, Debug)]
struct RiscvTopologyDmaDeviceSnapshots {
    accelerators: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
    gpus: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RiscvTopologyDmaActivityCounts {
    trace_event_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
    accelerator_activity: BTreeMap<AcceleratorEngineId, RiscvTopologyDmaDeviceActivity>,
    gpu_activity: BTreeMap<GpuDeviceId, RiscvTopologyDmaDeviceActivity>,
}

enum RiscvTopologyDmaIssueRecord {
    Accelerator(AcceleratorDmaIssueRecord),
    Gpu(GpuDmaIssueRecord),
}

impl RiscvTopologyDmaIssueRecord {
    fn record(self) {
        match self {
            Self::Accelerator(record) => record.record(),
            Self::Gpu(record) => record.record(),
        }
    }
}

enum RiscvTopologyDmaWriteRollback {
    Accelerator(AcceleratorDmaWriteRollback),
    Gpu(GpuDmaWriteRollback),
}

impl RiscvTopologyDmaWriteRollback {
    fn restore(self) {
        match self {
            Self::Accelerator(rollback) => rollback.restore(),
            Self::Gpu(rollback) => rollback.restore(),
        }
    }
}

impl RiscvTopologySystem {
    pub fn run_accelerator_dma_copy_parallel(
        &mut self,
        engine: AcceleratorEngineId,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
    ) -> Result<(), RiscvTopologySystemError> {
        self.run_accelerator_dma_copy_parallel_recorded(engine, copy, trace)
            .map(|_| ())
    }

    pub fn run_accelerator_dma_copy_parallel_recorded(
        &mut self,
        engine: AcceleratorEngineId,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaRunSummary, RiscvTopologySystemError> {
        self.run_dma_copies_parallel_recorded(
            [RiscvTopologyDmaCopy::accelerator(engine, copy)],
            trace,
        )
    }

    pub fn run_gpu_dma_copy_parallel(
        &mut self,
        device: GpuDeviceId,
        copy: GpuDmaCopy,
        trace: MemoryTrace,
    ) -> Result<(), RiscvTopologySystemError> {
        self.run_gpu_dma_copy_parallel_recorded(device, copy, trace)
            .map(|_| ())
    }

    pub fn run_gpu_dma_copy_parallel_recorded(
        &mut self,
        device: GpuDeviceId,
        copy: GpuDmaCopy,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaRunSummary, RiscvTopologySystemError> {
        self.run_dma_copies_parallel_recorded([RiscvTopologyDmaCopy::gpu(device, copy)], trace)
    }

    pub fn run_dma_copy_reads_parallel<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<Vec<PartitionEventId>, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        if copies.is_empty() {
            return Ok(Vec::new());
        }

        self.run_dma_copy_reads_parallel_recorded(copies, trace)
            .map(|summary| summary.events().to_vec())
    }

    pub fn run_dma_copy_reads_parallel_recorded<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaStageRunSummary, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        if copies.is_empty() {
            return Ok(self.empty_dma_stage_run_summary());
        }

        let memory = self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
            .clone();
        let memory_error = Arc::new(Mutex::new(None));
        let before = self.dma_device_snapshots(&copies)?;
        let msi_bank_data_cache = self.msi_bank_data_cache.clone();
        let msi_data_cache = self.msi_data_cache.clone();
        let msi_data_run_start = msi_data_cache
            .as_ref()
            .map(RiscvTopologyMsiDataCache::mark_runs);
        let mesi_data_cache = self.mesi_data_cache.clone();
        let mesi_data_run_start = mesi_data_cache
            .as_ref()
            .map(RiscvTopologyMesiDataCache::mark_runs);
        let moesi_data_cache = self.moesi_data_cache.clone();
        let moesi_data_run_start = moesi_data_cache
            .as_ref()
            .map(RiscvTopologyMoesiDataCache::mark_runs);
        let chi_data_cache = self.chi_data_cache.clone();
        let chi_data_run_start = chi_data_cache
            .as_ref()
            .map(RiscvTopologyChiDataCache::mark_runs);
        let cluster = self.cluster.clone();
        let mut scheduler = self.lock_scheduler();
        let issued_at = scheduler.now();
        let fabric_activity_start = self.transport.mark_fabric_activity();
        let fabric_wait_for_start = self.transport.mark_fabric_wait_for();
        let dram_activity_start = mark_dram_activity(&memory);
        let dram_wait_for_start = mark_dram_wait_for(&memory);
        let mut issue_records = Vec::new();
        let mut transactions = Vec::<ParallelMemoryTransaction>::new();

        for copy in copies {
            match copy {
                RiscvTopologyDmaCopy::Accelerator { engine, copy } => {
                    let accelerator = self
                        .accelerators
                        .get(&engine)
                        .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?
                        .engine()
                        .clone();
                    let read_memory = memory.clone();
                    let read_error = Arc::clone(&memory_error);
                    let read_msi_bank_data_cache = msi_bank_data_cache.clone();
                    let read_msi_data_cache = msi_data_cache.clone();
                    let read_mesi_data_cache = mesi_data_cache.clone();
                    let read_moesi_data_cache = moesi_data_cache.clone();
                    let read_chi_data_cache = chi_data_cache.clone();
                    let read_cluster = cluster.clone();
                    let prepared = accelerator.prepare_dma_copy_read(
                        issued_at,
                        copy,
                        trace.clone(),
                        move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                            topology_cached_memory_response(
                                &read_memory,
                                &read_error,
                                RiscvTopologyCachedDataCaches {
                                    msi_bank: read_msi_bank_data_cache.as_ref(),
                                    msi: read_msi_data_cache.as_ref(),
                                    mesi: read_mesi_data_cache.as_ref(),
                                    moesi: read_moesi_data_cache.as_ref(),
                                    chi: read_chi_data_cache.as_ref(),
                                    cluster: &read_cluster,
                                },
                                &delivery,
                            )
                        },
                    );
                    let (issue, transaction) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Accelerator(issue));
                    transactions.push(transaction);
                }
                RiscvTopologyDmaCopy::Gpu { device, copy } => {
                    let gpu = self
                        .gpus
                        .get(&device)
                        .ok_or(RiscvTopologySystemError::UnknownGpu { device })?
                        .gpu()
                        .clone();
                    let read_memory = memory.clone();
                    let read_error = Arc::clone(&memory_error);
                    let read_msi_bank_data_cache = msi_bank_data_cache.clone();
                    let read_msi_data_cache = msi_data_cache.clone();
                    let read_mesi_data_cache = mesi_data_cache.clone();
                    let read_moesi_data_cache = moesi_data_cache.clone();
                    let read_chi_data_cache = chi_data_cache.clone();
                    let read_cluster = cluster.clone();
                    let prepared = gpu.prepare_dma_copy_read(
                        issued_at,
                        copy,
                        trace.clone(),
                        move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                            topology_cached_memory_response(
                                &read_memory,
                                &read_error,
                                RiscvTopologyCachedDataCaches {
                                    msi_bank: read_msi_bank_data_cache.as_ref(),
                                    msi: read_msi_data_cache.as_ref(),
                                    mesi: read_mesi_data_cache.as_ref(),
                                    moesi: read_moesi_data_cache.as_ref(),
                                    chi: read_chi_data_cache.as_ref(),
                                    cluster: &read_cluster,
                                },
                                &delivery,
                            )
                        },
                    );
                    let (issue, transaction) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Gpu(issue));
                    transactions.push(transaction);
                }
            }
        }

        let events = self
            .transport
            .submit_parallel_batch(&mut scheduler, transactions)
            .map_err(RiscvTopologySystemError::Transport)?;
        for issue in issue_records {
            issue.record();
        }
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        drop(scheduler);
        if let Some(error) = take_memory_error(&memory_error) {
            return Err(error);
        }
        let activity = self.dma_activity_since(&before)?;
        let fabric_activity = fabric_activity_start
            .and_then(|marker| self.transport.fabric_lane_activities_since(marker))
            .unwrap_or_default();
        let fabric_wait_for = fabric_wait_for_start
            .and_then(|marker| self.transport.fabric_wait_for_graph_since(marker))
            .unwrap_or_default();
        let final_tick = scheduler_run.summary().final_tick();
        let dram_activity = dram_activities_since(&memory, dram_activity_start, Some(final_tick));
        let dram_wait_for = dram_wait_for_since(&memory, dram_wait_for_start);
        let (fabric_activity, dram_activity) = merge_msi_data_cache_activity(
            fabric_activity,
            dram_activity,
            msi_data_cache.as_ref(),
            msi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_mesi_data_cache_activity(
            fabric_activity,
            dram_activity,
            mesi_data_cache.as_ref(),
            mesi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_moesi_data_cache_activity(
            fabric_activity,
            dram_activity,
            moesi_data_cache.as_ref(),
            moesi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_chi_data_cache_activity(
            fabric_activity,
            dram_activity,
            chi_data_cache.as_ref(),
            chi_data_run_start,
        );

        Ok(RiscvTopologyDmaStageRunSummary::new(
            events,
            scheduler_run,
            activity.trace_event_count,
            activity.pending_dma_write_count,
            activity.dma_completion_count,
        )
        .with_device_activity(activity.accelerator_activity, activity.gpu_activity)
        .with_fabric_activity(fabric_activity)
        .with_fabric_wait_for(fabric_wait_for)
        .with_dram_activity(dram_activity)
        .with_dram_wait_for(dram_wait_for))
    }

    pub fn run_dma_copies_parallel<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<(), RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        if copies.is_empty() {
            return Ok(());
        }

        self.run_dma_copies_parallel_recorded(copies, trace)
            .map(|_| ())
    }

    pub fn run_dma_copies_parallel_recorded<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaRunSummary, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        let read = self.run_dma_copy_reads_parallel_recorded(copies.clone(), trace.clone())?;
        let write = self.run_dma_copy_writes_parallel_recorded(copies, trace)?;
        Ok(RiscvTopologyDmaRunSummary::new(read, write))
    }

    fn run_dma_copy_writes_parallel_recorded(
        &mut self,
        copies: Vec<RiscvTopologyDmaCopy>,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaStageRunSummary, RiscvTopologySystemError> {
        if copies.is_empty() {
            return Ok(self.empty_dma_stage_run_summary());
        }

        let memory = self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
            .clone();
        let memory_error = Arc::new(Mutex::new(None));
        let before = self.dma_device_snapshots(&copies)?;
        let msi_bank_data_cache = self.msi_bank_data_cache.clone();
        let msi_data_cache = self.msi_data_cache.clone();
        let msi_data_run_start = msi_data_cache
            .as_ref()
            .map(RiscvTopologyMsiDataCache::mark_runs);
        let mesi_data_cache = self.mesi_data_cache.clone();
        let mesi_data_run_start = mesi_data_cache
            .as_ref()
            .map(RiscvTopologyMesiDataCache::mark_runs);
        let moesi_data_cache = self.moesi_data_cache.clone();
        let moesi_data_run_start = moesi_data_cache
            .as_ref()
            .map(RiscvTopologyMoesiDataCache::mark_runs);
        let chi_data_cache = self.chi_data_cache.clone();
        let chi_data_run_start = chi_data_cache
            .as_ref()
            .map(RiscvTopologyChiDataCache::mark_runs);
        let cluster = self.cluster.clone();
        let mut scheduler = self.lock_scheduler();
        let issued_at = scheduler.now();
        let fabric_activity_start = self.transport.mark_fabric_activity();
        let fabric_wait_for_start = self.transport.mark_fabric_wait_for();
        let dram_activity_start = mark_dram_activity(&memory);
        let dram_wait_for_start = mark_dram_wait_for(&memory);
        let mut issue_records = Vec::new();
        let mut rollbacks = Vec::new();
        let mut transactions = Vec::<ParallelMemoryTransaction>::new();

        for copy in copies {
            match copy {
                RiscvTopologyDmaCopy::Accelerator { engine, .. } => {
                    let accelerator = self
                        .accelerators
                        .get(&engine)
                        .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?
                        .engine()
                        .clone();
                    let write_memory = memory.clone();
                    let write_error = Arc::clone(&memory_error);
                    let write_msi_bank_data_cache = msi_bank_data_cache.clone();
                    let write_msi_data_cache = msi_data_cache.clone();
                    let write_mesi_data_cache = mesi_data_cache.clone();
                    let write_moesi_data_cache = moesi_data_cache.clone();
                    let write_chi_data_cache = chi_data_cache.clone();
                    let write_cluster = cluster.clone();
                    let Some(prepared) = accelerator
                        .prepare_next_dma_write(
                            issued_at,
                            trace.clone(),
                            move |delivery, _context| {
                                topology_cached_memory_response(
                                    &write_memory,
                                    &write_error,
                                    RiscvTopologyCachedDataCaches {
                                        msi_bank: write_msi_bank_data_cache.as_ref(),
                                        msi: write_msi_data_cache.as_ref(),
                                        mesi: write_mesi_data_cache.as_ref(),
                                        moesi: write_moesi_data_cache.as_ref(),
                                        chi: write_chi_data_cache.as_ref(),
                                        cluster: &write_cluster,
                                    },
                                    &delivery,
                                )
                            },
                        )
                        .map_err(RiscvTopologySystemError::Accelerator)?
                    else {
                        return Err(RiscvTopologySystemError::AcceleratorDmaWriteNotReady {
                            engine,
                        });
                    };
                    let (issue, transaction, rollback) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Accelerator(issue));
                    rollbacks.push(RiscvTopologyDmaWriteRollback::Accelerator(rollback));
                    transactions.push(transaction);
                }
                RiscvTopologyDmaCopy::Gpu { device, .. } => {
                    let gpu = self
                        .gpus
                        .get(&device)
                        .ok_or(RiscvTopologySystemError::UnknownGpu { device })?
                        .gpu()
                        .clone();
                    let write_memory = memory.clone();
                    let write_error = Arc::clone(&memory_error);
                    let write_msi_bank_data_cache = msi_bank_data_cache.clone();
                    let write_msi_data_cache = msi_data_cache.clone();
                    let write_mesi_data_cache = mesi_data_cache.clone();
                    let write_moesi_data_cache = moesi_data_cache.clone();
                    let write_chi_data_cache = chi_data_cache.clone();
                    let write_cluster = cluster.clone();
                    let Some(prepared) = gpu
                        .prepare_next_dma_write(
                            issued_at,
                            trace.clone(),
                            move |delivery, _context| {
                                topology_cached_memory_response(
                                    &write_memory,
                                    &write_error,
                                    RiscvTopologyCachedDataCaches {
                                        msi_bank: write_msi_bank_data_cache.as_ref(),
                                        msi: write_msi_data_cache.as_ref(),
                                        mesi: write_mesi_data_cache.as_ref(),
                                        moesi: write_moesi_data_cache.as_ref(),
                                        chi: write_chi_data_cache.as_ref(),
                                        cluster: &write_cluster,
                                    },
                                    &delivery,
                                )
                            },
                        )
                        .map_err(RiscvTopologySystemError::Gpu)?
                    else {
                        return Err(RiscvTopologySystemError::GpuDmaWriteNotReady { device });
                    };
                    let (issue, transaction, rollback) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Gpu(issue));
                    rollbacks.push(RiscvTopologyDmaWriteRollback::Gpu(rollback));
                    transactions.push(transaction);
                }
            }
        }

        let events = match self
            .transport
            .submit_parallel_batch(&mut scheduler, transactions)
        {
            Ok(events) => events,
            Err(error) => {
                for rollback in rollbacks {
                    rollback.restore();
                }
                return Err(RiscvTopologySystemError::Transport(error));
            }
        };
        for issue in issue_records {
            issue.record();
        }
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        drop(scheduler);
        if let Some(error) = take_memory_error(&memory_error) {
            return Err(error);
        }
        let activity = self.dma_activity_since(&before)?;
        let fabric_activity = fabric_activity_start
            .and_then(|marker| self.transport.fabric_lane_activities_since(marker))
            .unwrap_or_default();
        let fabric_wait_for = fabric_wait_for_start
            .and_then(|marker| self.transport.fabric_wait_for_graph_since(marker))
            .unwrap_or_default();
        let final_tick = scheduler_run.summary().final_tick();
        let dram_activity = dram_activities_since(&memory, dram_activity_start, Some(final_tick));
        let dram_wait_for = dram_wait_for_since(&memory, dram_wait_for_start);
        let (fabric_activity, dram_activity) = merge_msi_data_cache_activity(
            fabric_activity,
            dram_activity,
            msi_data_cache.as_ref(),
            msi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_mesi_data_cache_activity(
            fabric_activity,
            dram_activity,
            mesi_data_cache.as_ref(),
            mesi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_moesi_data_cache_activity(
            fabric_activity,
            dram_activity,
            moesi_data_cache.as_ref(),
            moesi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_chi_data_cache_activity(
            fabric_activity,
            dram_activity,
            chi_data_cache.as_ref(),
            chi_data_run_start,
        );

        Ok(RiscvTopologyDmaStageRunSummary::new(
            events,
            scheduler_run,
            activity.trace_event_count,
            activity.pending_dma_write_count,
            activity.dma_completion_count,
        )
        .with_device_activity(activity.accelerator_activity, activity.gpu_activity)
        .with_fabric_activity(fabric_activity)
        .with_fabric_wait_for(fabric_wait_for)
        .with_dram_activity(dram_activity)
        .with_dram_wait_for(dram_wait_for))
    }

    fn dma_device_snapshots(
        &self,
        copies: &[RiscvTopologyDmaCopy],
    ) -> Result<RiscvTopologyDmaDeviceSnapshots, RiscvTopologySystemError> {
        let mut accelerators = BTreeMap::new();
        let mut gpus = BTreeMap::new();

        for copy in copies {
            match copy {
                RiscvTopologyDmaCopy::Accelerator { engine, .. } => {
                    if !accelerators.contains_key(engine) {
                        let snapshot = self
                            .accelerators
                            .get(engine)
                            .ok_or(RiscvTopologySystemError::UnknownAccelerator {
                                engine: *engine,
                            })?
                            .engine()
                            .snapshot();
                        accelerators.insert(*engine, snapshot);
                    }
                }
                RiscvTopologyDmaCopy::Gpu { device, .. } => {
                    if !gpus.contains_key(device) {
                        let snapshot = self
                            .gpus
                            .get(device)
                            .ok_or(RiscvTopologySystemError::UnknownGpu { device: *device })?
                            .gpu()
                            .snapshot();
                        gpus.insert(*device, snapshot);
                    }
                }
            }
        }

        Ok(RiscvTopologyDmaDeviceSnapshots { accelerators, gpus })
    }

    fn empty_dma_stage_run_summary(&self) -> RiscvTopologyDmaStageRunSummary {
        let final_tick = self.scheduler().now();
        RiscvTopologyDmaStageRunSummary::new(
            Vec::new(),
            RecordedConservativeRunSummary::empty(final_tick),
            0,
            0,
            0,
        )
    }

    fn dma_activity_since(
        &self,
        before: &RiscvTopologyDmaDeviceSnapshots,
    ) -> Result<RiscvTopologyDmaActivityCounts, RiscvTopologySystemError> {
        let mut activity = RiscvTopologyDmaActivityCounts::default();

        for (engine, before) in &before.accelerators {
            let after = self
                .accelerators
                .get(engine)
                .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine: *engine })?
                .engine()
                .snapshot();
            let device_activity = RiscvTopologyDmaDeviceActivity::new(
                after.trace().len().saturating_sub(before.trace().len()),
                after.pending_dma_writes().len(),
                after
                    .dma_completions()
                    .len()
                    .saturating_sub(before.dma_completions().len()),
            );
            activity.trace_event_count += device_activity.trace_event_count();
            activity.pending_dma_write_count += device_activity.pending_dma_write_count();
            activity.dma_completion_count += device_activity.dma_completion_count();
            activity
                .accelerator_activity
                .insert(*engine, device_activity);
        }

        for (device, before) in &before.gpus {
            let after = self
                .gpus
                .get(device)
                .ok_or(RiscvTopologySystemError::UnknownGpu { device: *device })?
                .gpu()
                .snapshot();
            let device_activity = RiscvTopologyDmaDeviceActivity::new(
                after.trace().len().saturating_sub(before.trace().len()),
                after.pending_dma_writes().len(),
                after
                    .dma_completions()
                    .len()
                    .saturating_sub(before.dma_completions().len()),
            );
            activity.trace_event_count += device_activity.trace_event_count();
            activity.pending_dma_write_count += device_activity.pending_dma_write_count();
            activity.dma_completion_count += device_activity.dma_completion_count();
            activity.gpu_activity.insert(*device, device_activity);
        }

        Ok(activity)
    }
}
