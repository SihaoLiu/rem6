use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterError, RiscvClusterTurn};
use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler};
use rem6_memory::TranslationPageMap;
use rem6_mmio::MmioBus;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{
    pending_trap_cores_from_turn, GuestEventId, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, SystemError,
};

impl RiscvSystemRunDriver {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_or_tick_limit_parallel_with_mmio_and_data_translation<
        F,
        D,
        FR,
        DR,
        E,
    >(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
        tick_limit: u64,
        event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        self.drive_until_host_stop_or_tick_limit_parallel_with_translation(
            cluster,
            scheduler,
            tick_limit,
            |scheduler, tick_limit| {
                cluster.drive_turn_parallel_with_mmio_and_data_translation_until_tick(
                    scheduler,
                    transport,
                    bus,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    page_map,
                    &mut fetch_responder,
                    &mut data_responder,
                    tick_limit,
                )
            },
            event_for,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_or_tick_limit_parallel_with_data_translation<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
        tick_limit: u64,
        event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        self.drive_until_host_stop_or_tick_limit_parallel_with_translation(
            cluster,
            scheduler,
            tick_limit,
            |scheduler, tick_limit| {
                cluster.drive_turn_parallel_with_data_translation_until_tick(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    page_map,
                    &mut fetch_responder,
                    &mut data_responder,
                    tick_limit,
                )
            },
            event_for,
        )
    }

    fn drive_until_host_stop_or_tick_limit_parallel_with_translation<T, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        tick_limit: u64,
        mut drive_turn: T,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        T: FnMut(
            &mut PartitionedScheduler,
            u64,
        ) -> Result<Option<RiscvClusterTurn>, RiscvClusterError>,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();
        self.reset_stats_for_run(cluster)?;

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        loop {
            if scheduler.now() >= tick_limit {
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::TickLimit {
                        tick: tick_limit,
                        limit: tick_limit,
                    },
                ));
            }

            self.snapshot_live_retire_gate_policy(cluster)?;
            let Some(turn) =
                drive_turn(scheduler, tick_limit).map_err(SystemError::RiscvCluster)?
            else {
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::TickLimit {
                        tick: tick_limit,
                        limit: tick_limit,
                    },
                ));
            };
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.schedule_riscv_system_events_from_turn_parallel(
                cluster,
                scheduler,
                &turn,
                &mut event_for,
            )?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_parallel_with_data_translation<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();
        self.reset_stats_for_run(cluster)?;

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            self.snapshot_live_retire_gate_policy(cluster)?;
            let turn = cluster
                .drive_turn_parallel_with_data_translation(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    page_map,
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.schedule_riscv_system_events_from_turn_parallel(
                cluster,
                scheduler,
                &turn,
                &mut event_for,
            )?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_parallel_with_mmio_and_data_translation<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();
        self.reset_stats_for_run(cluster)?;

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            self.snapshot_live_retire_gate_policy(cluster)?;
            let turn = cluster
                .drive_turn_parallel_with_mmio_and_data_translation(
                    scheduler,
                    transport,
                    bus,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    page_map,
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.schedule_riscv_system_events_from_turn_parallel(
                cluster,
                scheduler,
                &turn,
                &mut event_for,
            )?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }
}
