use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvDataAccessEvent, RiscvDataAccessEventKind};
use rem6_isa_riscv::MemoryAccessKind;
use rem6_memory::{Address, AddressRange, CacheLineLayout, MemoryOperation};
use rem6_stats::{
    CommMonitor, CommMonitorConfig, CommMonitorSnapshot, MemCheckerMonitor,
    MemCheckerMonitorSnapshot, MemFootprintProbe, MemFootprintProbeConfig,
    MemFootprintProbeSnapshot, MemProbePacket, MemProbePacketAccess, MemTraceProbe,
    MemTraceProbeConfig, MemTraceProbeSnapshot, ProbePayload, ProbePointId, ProbeRegistry,
    ProbeSnapshot, StackDistProbe, StackDistProbeConfig, StackDistProbeSnapshot, StatsError,
};

use crate::{RiscvSystemRun, RiscvSystemRunDriver, SystemError};

const RISCV_DATA_ACCESS_RETRY_RESPONSE_FLAG: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataAccessProbeSnapshot {
    probes: ProbeSnapshot,
    stack_distance: StackDistProbeSnapshot,
    memory_trace: Option<MemTraceProbeSnapshot>,
    memory_footprint: Option<MemFootprintProbeSnapshot>,
    communication_monitor: Option<CommMonitorSnapshot>,
    mem_checker_monitor: Option<MemCheckerMonitorSnapshot>,
    request_point: ProbePointId,
    response_point: Option<ProbePointId>,
}

impl RiscvDataAccessProbeSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        probes: ProbeSnapshot,
        stack_distance: StackDistProbeSnapshot,
        memory_trace: Option<MemTraceProbeSnapshot>,
        memory_footprint: Option<MemFootprintProbeSnapshot>,
        communication_monitor: Option<CommMonitorSnapshot>,
        mem_checker_monitor: Option<MemCheckerMonitorSnapshot>,
        request_point: ProbePointId,
        response_point: Option<ProbePointId>,
    ) -> Self {
        Self {
            probes,
            stack_distance,
            memory_trace,
            memory_footprint,
            communication_monitor,
            mem_checker_monitor,
            request_point,
            response_point,
        }
    }

    pub const fn probes(&self) -> &ProbeSnapshot {
        &self.probes
    }

    pub const fn stack_distance(&self) -> &StackDistProbeSnapshot {
        &self.stack_distance
    }

    pub const fn memory_trace(&self) -> Option<&MemTraceProbeSnapshot> {
        self.memory_trace.as_ref()
    }

    pub const fn memory_footprint(&self) -> Option<&MemFootprintProbeSnapshot> {
        self.memory_footprint.as_ref()
    }

    pub const fn communication_monitor(&self) -> Option<&CommMonitorSnapshot> {
        self.communication_monitor.as_ref()
    }

    pub const fn mem_checker_monitor(&self) -> Option<&MemCheckerMonitorSnapshot> {
        self.mem_checker_monitor.as_ref()
    }

    pub const fn request_point(&self) -> ProbePointId {
        self.request_point
    }

    pub const fn response_point(&self) -> Option<ProbePointId> {
        self.response_point
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvDataAccessProbeLineLayout {
    range: AddressRange,
    layout: CacheLineLayout,
}

impl RiscvDataAccessProbeLineLayout {
    pub(crate) const fn new(range: AddressRange, layout: CacheLineLayout) -> Self {
        Self { range, layout }
    }

    fn aligned_address(self, address: Address) -> Option<u64> {
        self.range
            .contains(address)
            .then(|| self.layout.line_address(address).get())
    }
}

#[derive(Debug)]
pub struct RiscvDataAccessStats {
    probes: Arc<Mutex<RiscvDataAccessProbeRecorder>>,
}

impl Clone for RiscvDataAccessStats {
    fn clone(&self) -> Self {
        let recorder = self.probes.lock().expect("data access probe recorder lock");
        let stack_distance_config = recorder.stack_distance_config().clone();
        let memory_trace_config = recorder.memory_trace_config().cloned();
        let memory_footprint_config = recorder.memory_footprint_config().cloned();
        let communication_monitor_config = recorder.communication_monitor_config().cloned();
        let mem_checker_monitor_enabled = recorder.mem_checker_monitor_enabled();
        let line_layouts = recorder.line_layouts().to_vec();
        Self {
            probes: Arc::new(Mutex::new(RiscvDataAccessProbeRecorder::new(
                stack_distance_config,
                memory_trace_config,
                memory_footprint_config,
                communication_monitor_config,
                mem_checker_monitor_enabled,
                line_layouts,
            ))),
        }
    }
}

impl RiscvDataAccessStats {
    pub fn with_stack_distance(config: StackDistProbeConfig) -> Self {
        Self::with_stack_distance_line_layouts(config, [])
    }

    pub(crate) fn with_stack_distance_line_layouts<I>(
        config: StackDistProbeConfig,
        line_layouts: I,
    ) -> Self
    where
        I: IntoIterator<Item = RiscvDataAccessProbeLineLayout>,
    {
        Self {
            probes: Arc::new(Mutex::new(RiscvDataAccessProbeRecorder::new(
                config,
                None,
                None,
                None,
                false,
                line_layouts.into_iter().collect(),
            ))),
        }
    }

    pub fn with_mem_trace(self, config: MemTraceProbeConfig) -> Self {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .set_memory_trace_config(config);
        self
    }

    pub fn with_mem_footprint(self, config: MemFootprintProbeConfig) -> Self {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .set_memory_footprint_config(config);
        self
    }

    pub fn with_comm_monitor(self, config: CommMonitorConfig) -> Self {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .set_communication_monitor_config(config);
        self
    }

    pub fn with_mem_checker_monitor(self) -> Self {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .enable_mem_checker_monitor();
        self
    }

    pub(crate) fn reset_for_run<I>(&self, cursors: I)
    where
        I: IntoIterator<Item = (CpuId, usize)>,
    {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .reset(cursors);
    }

    pub(crate) fn record_data_access_events<I>(&self, events_by_cpu: I) -> Result<(), StatsError>
    where
        I: IntoIterator<Item = (CpuId, usize, Vec<RiscvDataAccessEvent>)>,
    {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .record_data_access_events(events_by_cpu)
    }

    pub fn data_access_probe_snapshot(&self) -> RiscvDataAccessProbeSnapshot {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .snapshot()
    }

    pub(crate) fn cursors(&self) -> BTreeMap<CpuId, usize> {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .cursors()
    }
}

impl RiscvSystemRun {
    pub fn with_data_access_probes(
        mut self,
        data_access_probes: Option<RiscvDataAccessProbeSnapshot>,
    ) -> Self {
        self.data_access_probes = data_access_probes;
        self
    }

    pub const fn data_access_probes(&self) -> Option<&RiscvDataAccessProbeSnapshot> {
        self.data_access_probes.as_ref()
    }
}

impl RiscvSystemRunDriver {
    pub fn with_data_access_stats(mut self, data_access_stats: RiscvDataAccessStats) -> Self {
        self.data_access_stats = Some(data_access_stats);
        self
    }

    pub const fn data_access_stats(&self) -> Option<&RiscvDataAccessStats> {
        self.data_access_stats.as_ref()
    }

    pub(crate) fn reset_data_access_stats_for_run(
        &self,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        if let Some(data_access_stats) = &self.data_access_stats {
            data_access_stats.reset_for_run(data_access_event_cursors(cluster)?);
        }
        Ok(())
    }

    pub(crate) fn record_data_access_stats(
        &self,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        let Some(data_access_stats) = &self.data_access_stats else {
            return Ok(());
        };
        let cursors = data_access_stats.cursors();
        data_access_stats
            .record_data_access_events(data_access_event_snapshots_from_cursors(cluster, &cursors)?)
            .map_err(SystemError::Stats)
    }
}

fn data_access_event_cursors(cluster: &RiscvCluster) -> Result<Vec<(CpuId, usize)>, SystemError> {
    cluster
        .core_ids()
        .into_iter()
        .map(|cpu| {
            cluster
                .core(cpu)
                .map(|core| (cpu, core.data_access_event_count()))
                .map_err(SystemError::RiscvCluster)
        })
        .collect()
}

fn data_access_event_snapshots_from_cursors(
    cluster: &RiscvCluster,
    cursors: &BTreeMap<CpuId, usize>,
) -> Result<Vec<(CpuId, usize, Vec<RiscvDataAccessEvent>)>, SystemError> {
    cluster
        .core_ids()
        .into_iter()
        .map(|cpu| {
            let core = cluster.core(cpu).map_err(SystemError::RiscvCluster)?;
            let cursor = cursors.get(&cpu).copied().unwrap_or(0);
            let events = core.data_access_events_from(cursor);
            let next_cursor = cursor.saturating_add(events.len());
            Ok((cpu, next_cursor, events))
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvDataAccessProbeRecorder {
    stack_distance_config: StackDistProbeConfig,
    memory_trace_config: Option<MemTraceProbeConfig>,
    memory_footprint_config: Option<MemFootprintProbeConfig>,
    communication_monitor_config: Option<CommMonitorConfig>,
    mem_checker_monitor_enabled: bool,
    line_layouts: Vec<RiscvDataAccessProbeLineLayout>,
    probes: ProbeRegistry,
    stack_distance: StackDistProbe,
    memory_trace: Option<MemTraceProbe>,
    memory_footprint: Option<MemFootprintProbe>,
    communication_monitor: Option<CommMonitor>,
    mem_checker_monitor: Option<MemCheckerMonitor>,
    request_point: ProbePointId,
    response_point: Option<ProbePointId>,
    retry_response_point: Option<ProbePointId>,
    cursors: BTreeMap<CpuId, usize>,
}

impl RiscvDataAccessProbeRecorder {
    fn new(
        stack_distance_config: StackDistProbeConfig,
        memory_trace_config: Option<MemTraceProbeConfig>,
        memory_footprint_config: Option<MemFootprintProbeConfig>,
        communication_monitor_config: Option<CommMonitorConfig>,
        mem_checker_monitor_enabled: bool,
        line_layouts: Vec<RiscvDataAccessProbeLineLayout>,
    ) -> Self {
        let mut recorder = Self {
            stack_distance_config: stack_distance_config.clone(),
            memory_trace_config,
            memory_footprint_config,
            communication_monitor_config,
            mem_checker_monitor_enabled,
            line_layouts,
            probes: ProbeRegistry::new(),
            stack_distance: StackDistProbe::new(stack_distance_config),
            memory_trace: None,
            memory_footprint: None,
            communication_monitor: None,
            mem_checker_monitor: None,
            request_point: ProbePointId::new(0),
            response_point: None,
            retry_response_point: None,
            cursors: BTreeMap::new(),
        };
        recorder.reset([]);
        recorder
    }

    fn set_memory_trace_config(&mut self, config: MemTraceProbeConfig) {
        self.memory_trace_config = Some(config);
        self.reset(self.cursors.clone());
    }

    fn set_memory_footprint_config(&mut self, config: MemFootprintProbeConfig) {
        self.memory_footprint_config = Some(config);
        self.reset(self.cursors.clone());
    }

    fn set_communication_monitor_config(&mut self, config: CommMonitorConfig) {
        self.communication_monitor_config = Some(config);
        self.reset(self.cursors.clone());
    }

    fn enable_mem_checker_monitor(&mut self) {
        self.mem_checker_monitor_enabled = true;
        self.reset(self.cursors.clone());
    }

    fn reset<I>(&mut self, cursors: I)
    where
        I: IntoIterator<Item = (CpuId, usize)>,
    {
        self.probes = ProbeRegistry::new();
        self.stack_distance = StackDistProbe::new(self.stack_distance_config.clone());
        self.memory_trace = self.memory_trace_config.clone().map(MemTraceProbe::new);
        self.memory_footprint = self
            .memory_footprint_config
            .clone()
            .map(MemFootprintProbe::new);
        self.communication_monitor = self
            .communication_monitor_config
            .clone()
            .map(CommMonitor::new);
        self.mem_checker_monitor = self
            .mem_checker_monitor_enabled
            .then(MemCheckerMonitor::new);
        self.request_point = self
            .probes
            .register_point("riscv_data", "Request")
            .expect("generated data access probe point is valid");
        self.response_point =
            if self.communication_monitor.is_some() || self.mem_checker_monitor.is_some() {
                Some(
                    self.probes
                        .register_point("riscv_data", "Response")
                        .expect("generated data access probe point is valid"),
                )
            } else {
                None
            };
        self.retry_response_point = self.mem_checker_monitor.is_some().then(|| {
            self.probes
                .register_point("riscv_data", "RetryResponse")
                .expect("generated data access retry probe point is valid")
        });
        self.probes
            .add_listener(self.request_point, "stack_dist")
            .expect("generated data access probe listener is valid");
        if self.memory_trace.is_some() {
            self.probes
                .add_listener(self.request_point, "mem_trace")
                .expect("generated data access probe listener is valid");
        }
        if self.memory_footprint.is_some() {
            self.probes
                .add_listener(self.request_point, "mem_footprint")
                .expect("generated data access probe listener is valid");
        }
        if self.communication_monitor.is_some() {
            self.probes
                .add_listener(self.request_point, "comm_monitor")
                .expect("generated data access probe listener is valid");
            if let Some(response_point) = self.response_point {
                self.probes
                    .add_listener(response_point, "comm_monitor")
                    .expect("generated data access probe listener is valid");
            }
        }
        if self.mem_checker_monitor.is_some() {
            self.probes
                .add_listener(self.request_point, "mem_checker_monitor")
                .expect("generated data access probe listener is valid");
            if let Some(response_point) = self.response_point {
                self.probes
                    .add_listener(response_point, "mem_checker_monitor")
                    .expect("generated data access probe listener is valid");
            }
            if let Some(retry_response_point) = self.retry_response_point {
                self.probes
                    .add_listener(retry_response_point, "mem_checker_monitor")
                    .expect("generated data access probe listener is valid");
            }
        }
        self.cursors = cursors.into_iter().collect();
    }

    fn record_data_access_events<I>(&mut self, events_by_cpu: I) -> Result<(), StatsError>
    where
        I: IntoIterator<Item = (CpuId, usize, Vec<RiscvDataAccessEvent>)>,
    {
        let mut new_events = Vec::new();
        let mut new_cursors = Vec::new();
        for (cpu, next_cursor, events) in events_by_cpu {
            let current_cursor = self.cursors.get(&cpu).copied().unwrap_or(0);
            let event_start_cursor = next_cursor.saturating_sub(events.len());
            let skip = current_cursor
                .saturating_sub(event_start_cursor)
                .min(events.len());
            for event in events.iter().skip(skip) {
                new_events.push((cpu, event.clone()));
            }
            new_cursors.push((cpu, current_cursor.max(next_cursor)));
        }

        new_events.sort_by_key(|(cpu, event)| (event.tick(), *cpu));
        for (_cpu, event) in new_events {
            self.record_event(&event)?;
        }
        for (cpu, cursor) in new_cursors {
            self.cursors.insert(cpu, cursor);
        }
        Ok(())
    }

    fn record_event(&mut self, event: &RiscvDataAccessEvent) -> Result<(), StatsError> {
        match event.kind() {
            RiscvDataAccessEventKind::Issued => self.record_request_event(event),
            RiscvDataAccessEventKind::Completed => self.record_response_event(event, false),
            RiscvDataAccessEventKind::ConditionalFailed => self.record_response_event(event, true),
            RiscvDataAccessEventKind::Retry => self.record_retry_event(event),
            RiscvDataAccessEventKind::Failed => Ok(()),
        }
    }

    fn record_request_event(&mut self, event: &RiscvDataAccessEvent) -> Result<(), StatsError> {
        let Some(access) = packet_access(event.operation()) else {
            return Ok(());
        };
        let packet = MemProbePacket::request(event.physical_address().get())
            .with_access(access)
            .with_command(memory_operation_trace_command(event.operation()))
            .with_size(event.size().bytes())
            .with_packet_id(packet_id(event));
        let probe_event = self
            .probes
            .emit(
                event.tick(),
                self.request_point,
                ProbePayload::MemoryPacket(packet),
            )?
            .clone();
        let stack_distance_packet = self.stack_distance_packet(packet);
        self.stack_distance.observe_packet(&stack_distance_packet)?;
        if let Some(memory_trace) = &mut self.memory_trace {
            memory_trace.observe_probe_event(&probe_event, self.request_point)?;
        }
        if let Some(memory_footprint) = &mut self.memory_footprint {
            memory_footprint.observe_probe_event(&probe_event, self.request_point)?;
        }
        if let Some(communication_monitor) = &mut self.communication_monitor {
            communication_monitor.observe_request_probe_event(
                &probe_event,
                self.request_point,
                self.response_point.is_some(),
            )?;
        }
        let request_data = request_data(event.access(), event.size().bytes());
        if mem_checker_tracks_access(event.access()) {
            if let Some(mem_checker_monitor) = &mut self.mem_checker_monitor {
                mem_checker_monitor.observe_timing_request(
                    event.tick(),
                    &packet,
                    true,
                    true,
                    request_data.as_deref(),
                )?;
            }
        }
        Ok(())
    }

    fn record_retry_event(&mut self, event: &RiscvDataAccessEvent) -> Result<(), StatsError> {
        let Some(retry_response_point) = self.retry_response_point else {
            return Ok(());
        };
        let Some(access) = packet_access(event.operation()) else {
            return Ok(());
        };
        let packet = MemProbePacket::response(event.physical_address().get())
            .with_access(access)
            .with_command(memory_operation_trace_command(event.operation()))
            .with_flags(RISCV_DATA_ACCESS_RETRY_RESPONSE_FLAG)
            .with_size(event.size().bytes())
            .with_packet_id(packet_id(event));
        if !self.mem_checker_has_pending(packet.packet_id()) {
            return Ok(());
        }
        self.probes.emit(
            event.tick(),
            retry_response_point,
            ProbePayload::MemoryPacket(packet),
        )?;
        if let Some(mem_checker_monitor) = &mut self.mem_checker_monitor {
            mem_checker_monitor.observe_timing_response(
                event.tick(),
                &packet,
                false,
                event.data(),
                false,
            )?;
        }
        Ok(())
    }

    fn stack_distance_packet(&self, packet: MemProbePacket) -> MemProbePacket {
        let address = self.stack_distance_address(packet.address());
        MemProbePacket::new(address, packet.kind())
            .with_access(packet.access())
            .with_command(packet.command())
            .with_flags(packet.flags())
            .with_size(packet.size())
            .with_packet_id(packet.packet_id())
            .with_program_counter(packet.program_counter())
    }

    fn stack_distance_address(&self, address: u64) -> u64 {
        let address = Address::new(address);
        self.line_layouts
            .iter()
            .find_map(|layout| layout.aligned_address(address))
            .unwrap_or_else(|| address.get())
    }

    fn record_response_event(
        &mut self,
        event: &RiscvDataAccessEvent,
        store_conditional_failed: bool,
    ) -> Result<(), StatsError> {
        if self.mem_checker_monitor.is_none() && self.communication_monitor.is_none() {
            return Ok(());
        }
        let Some(response_point) = self.response_point else {
            return Ok(());
        };
        let operation = response_operation(event.operation(), store_conditional_failed);
        let Some(access) = packet_access(operation) else {
            return Ok(());
        };
        let packet = MemProbePacket::response(event.physical_address().get())
            .with_access(access)
            .with_command(memory_operation_trace_command(operation))
            .with_size(event.size().bytes())
            .with_packet_id(packet_id(event));
        if !self.has_pending_response(packet.packet_id()) {
            return Ok(());
        }
        let probe_event = self
            .probes
            .emit(
                event.tick(),
                response_point,
                ProbePayload::MemoryPacket(packet),
            )?
            .clone();
        if let Some(communication_monitor) = &mut self.communication_monitor {
            communication_monitor.observe_response_probe_event(&probe_event, response_point)?;
        }
        if self.mem_checker_has_pending(packet.packet_id()) {
            if let Some(mem_checker_monitor) = &mut self.mem_checker_monitor {
                mem_checker_monitor.observe_timing_response(
                    event.tick(),
                    &packet,
                    true,
                    event.data(),
                    store_conditional_failed,
                )?;
            }
        }
        Ok(())
    }

    fn has_pending_response(&self, packet_id: u64) -> bool {
        self.communication_has_pending(packet_id) || self.mem_checker_has_pending(packet_id)
    }

    fn communication_has_pending(&self, packet_id: u64) -> bool {
        self.communication_monitor.as_ref().is_some_and(|monitor| {
            monitor
                .pending()
                .iter()
                .any(|pending| pending.packet_id() == packet_id)
        })
    }

    fn mem_checker_has_pending(&self, packet_id: u64) -> bool {
        self.mem_checker_monitor.as_ref().is_some_and(|monitor| {
            monitor
                .pending()
                .any(|pending| pending.packet_id() == packet_id)
        })
    }

    fn snapshot(&self) -> RiscvDataAccessProbeSnapshot {
        RiscvDataAccessProbeSnapshot::new(
            self.probes.snapshot(),
            self.stack_distance.snapshot(),
            self.memory_trace.as_ref().map(MemTraceProbe::snapshot),
            self.memory_footprint
                .as_ref()
                .map(MemFootprintProbe::snapshot),
            self.communication_monitor
                .as_ref()
                .map(CommMonitor::snapshot),
            self.mem_checker_monitor
                .as_ref()
                .map(MemCheckerMonitor::snapshot),
            self.request_point,
            self.response_point,
        )
    }

    fn stack_distance_config(&self) -> &StackDistProbeConfig {
        &self.stack_distance_config
    }

    fn memory_trace_config(&self) -> Option<&MemTraceProbeConfig> {
        self.memory_trace_config.as_ref()
    }

    fn memory_footprint_config(&self) -> Option<&MemFootprintProbeConfig> {
        self.memory_footprint_config.as_ref()
    }

    fn communication_monitor_config(&self) -> Option<&CommMonitorConfig> {
        self.communication_monitor_config.as_ref()
    }

    const fn mem_checker_monitor_enabled(&self) -> bool {
        self.mem_checker_monitor_enabled
    }

    fn line_layouts(&self) -> &[RiscvDataAccessProbeLineLayout] {
        &self.line_layouts
    }

    fn cursors(&self) -> BTreeMap<CpuId, usize> {
        self.cursors.clone()
    }
}

fn mem_checker_tracks_access(access: &MemoryAccessKind) -> bool {
    matches!(
        access,
        MemoryAccessKind::Load { .. }
            | MemoryAccessKind::FloatLoad { .. }
            | MemoryAccessKind::LoadReserved { .. }
            | MemoryAccessKind::Store { .. }
            | MemoryAccessKind::FloatStore { .. }
            | MemoryAccessKind::StoreConditional { .. }
    )
}

fn request_data(access: &MemoryAccessKind, size: u64) -> Option<Vec<u8>> {
    let size = usize::try_from(size).ok()?;
    match access {
        MemoryAccessKind::Store { value, .. }
        | MemoryAccessKind::FloatStore { value, .. }
        | MemoryAccessKind::StoreConditional { value, .. } => {
            let bytes = value.to_le_bytes();
            bytes.get(..size).map(<[u8]>::to_vec)
        }
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::LoadReserved { .. }
        | MemoryAccessKind::AtomicMemory { .. } => None,
    }
}

fn response_operation(
    operation: MemoryOperation,
    store_conditional_failed: bool,
) -> MemoryOperation {
    if store_conditional_failed {
        MemoryOperation::StoreConditionalFail
    } else {
        operation
    }
}

fn packet_id(event: &RiscvDataAccessEvent) -> u64 {
    (u64::from(event.request_id().agent().get()) << 32)
        | (event.request_id().sequence() & u64::from(u32::MAX))
}

fn packet_access(operation: MemoryOperation) -> Option<MemProbePacketAccess> {
    match operation {
        MemoryOperation::ReadShared | MemoryOperation::ReadUnique | MemoryOperation::LoadLocked => {
            Some(MemProbePacketAccess::Read)
        }
        MemoryOperation::Write
        | MemoryOperation::StoreConditional
        | MemoryOperation::StoreConditionalFail
        | MemoryOperation::LockedRmwWrite
        | MemoryOperation::Atomic
        | MemoryOperation::AtomicNoReturn => Some(MemProbePacketAccess::Write),
        _ => None,
    }
}

fn memory_operation_trace_command(operation: MemoryOperation) -> u32 {
    match operation {
        MemoryOperation::NoAccess => 0,
        MemoryOperation::InstructionFetch => 1,
        MemoryOperation::ReadShared => 2,
        MemoryOperation::ReadUnique => 3,
        MemoryOperation::LoadLocked => 4,
        MemoryOperation::LockedRmwRead => 5,
        MemoryOperation::LockedRmwWrite => 6,
        MemoryOperation::Write => 7,
        MemoryOperation::CacheBlockZero => 8,
        MemoryOperation::StoreConditional => 9,
        MemoryOperation::StoreConditionalFail => 10,
        MemoryOperation::StoreConditionalUpgrade => 11,
        MemoryOperation::StoreConditionalUpgradeFail => 12,
        MemoryOperation::Upgrade => 13,
        MemoryOperation::Atomic => 14,
        MemoryOperation::AtomicNoReturn => 15,
        MemoryOperation::PrefetchRead => 16,
        MemoryOperation::PrefetchWrite => 17,
        MemoryOperation::WriteClean => 18,
        MemoryOperation::WritebackClean => 19,
        MemoryOperation::WritebackDirty => 20,
        MemoryOperation::CleanShared => 21,
        MemoryOperation::CleanEvict => 22,
        MemoryOperation::Invalidate => 23,
        MemoryOperation::InvalidateWritable => 24,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rem6_cpu::{RiscvDataAccessRecord, RiscvDataAccessTarget};
    use rem6_isa_riscv::{MemoryWidth, Register};
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId, MemoryRequestId};
    use rem6_stats::{MemTraceProbeHeader, StackDistProbeConfig};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    fn stack_distance_config() -> StackDistProbeConfig {
        StackDistProbeConfig::builder(16, 16).build().unwrap()
    }

    fn mem_trace_config() -> MemTraceProbeConfig {
        MemTraceProbeConfig::new(
            MemTraceProbeHeader::new("riscv_data", 1, vec![(0, "cpu0".to_string())]).unwrap(),
            true,
        )
    }

    fn load_event(sequence: u64) -> RiscvDataAccessEvent {
        RiscvDataAccessEvent::issued(RiscvDataAccessRecord::new(
            sequence,
            PartitionId::new(0),
            RiscvDataAccessTarget::Memory {
                route: MemoryRouteId::new(0),
                endpoint: TransportEndpointId::new("cpu0.dmem").unwrap(),
            },
            MemoryRequestId::new(AgentId::new(0), sequence),
            MemoryRequestId::new(AgentId::new(0), 100 + sequence),
            MemoryAccessKind::Load {
                rd: Register::new(10).unwrap(),
                address: 0x9000 + sequence,
                width: MemoryWidth::Doubleword,
                signed: false,
            },
            AccessSize::new(8).unwrap(),
            Address::new(0x9000 + sequence),
        ))
    }

    #[test]
    fn data_access_recorder_ignores_stale_cursor_overlap() {
        let mut recorder = RiscvDataAccessProbeRecorder::new(
            stack_distance_config(),
            Some(mem_trace_config()),
            None,
            None,
            false,
            Vec::new(),
        );
        let event = load_event(0);

        recorder
            .record_data_access_events([(CpuId::new(0), 1, vec![event.clone()])])
            .unwrap();
        recorder
            .record_data_access_events([(CpuId::new(0), 1, vec![event])])
            .unwrap();

        let snapshot = recorder.snapshot();
        assert_eq!(
            snapshot.memory_trace().unwrap().records().len(),
            1,
            "stale cursor replay must not duplicate probe events"
        );
    }
}
