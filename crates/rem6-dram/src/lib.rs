use std::collections::BTreeMap;

mod activity;
mod error;
mod low_power;
mod memory_controller;
mod memory_error;
mod profile;
mod profile_snapshot;
mod qos;
mod timing;

use activity::{collect_dram_bank_activity, collect_dram_port_activity};
pub use activity::{
    DramActivityMarker, DramActivityProfile, DramBankActivity, DramMemoryActivityMarker,
    DramMemoryActivityProfile, DramPortActivity, DramTargetActivity,
};
pub use error::DramError;
pub use low_power::{
    DramLowPowerActivity, DramLowPowerEvent, DramLowPowerState, DramLowPowerTiming,
    DramLowPowerTimingField,
};
pub use memory_controller::{
    DramMemoryController, DramMemoryOutcome, DramMemorySnapshot, DramMemoryTargetSnapshot,
    DramMemoryWaitForMarker,
};
pub use memory_error::DramMemoryError;
pub use profile::{
    DramMemoryTechnology, DramProfileField, ExternalMemoryParallelResourceSummary,
    ExternalMemoryProfile, ExternalMemoryTopology, NvmMediaTiming, NvmMediaTimingField,
};
pub use profile_snapshot::DramProfileSnapshotMismatch;
pub use qos::{DramQosAccess, DramQosRequest, DramQosSchedulingPolicy, DramQosTurnaroundPolicy};
pub use timing::{DramCommandWindow, DramGeometry, DramTiming, DramTimingField};

use rem6_fabric::QosQueueArbiter;
use rem6_kernel::{WaitForEdgeKind, WaitForGraph, WaitForNode};
use rem6_memory::{
    CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId, MemoryTargetId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramWaitForMarker {
    offset: usize,
}

impl DramWaitForMarker {
    const fn new(offset: usize) -> Self {
        Self { offset }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramAccessKind {
    Read,
    Write,
}

impl DramAccessKind {
    fn from_operation(request: &MemoryRequest) -> Result<Self, DramError> {
        match request.operation() {
            MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::ReadUnique
            | MemoryOperation::PrefetchRead => Ok(Self::Read),
            MemoryOperation::Write
            | MemoryOperation::Atomic
            | MemoryOperation::PrefetchWrite
            | MemoryOperation::WriteClean
            | MemoryOperation::WritebackClean
            | MemoryOperation::WritebackDirty => Ok(Self::Write),
            operation => Err(DramError::UnsupportedOperation {
                request: request.id(),
                operation,
            }),
        }
    }

    fn command_kind(self) -> DramCommandKind {
        match self {
            Self::Read => DramCommandKind::Read,
            Self::Write => DramCommandKind::Write,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramCommandKind {
    Precharge,
    Activate,
    Read,
    Write,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramCommand {
    cycle: u64,
    parallel_port: u32,
    bank: u32,
    row: u64,
    kind: DramCommandKind,
}

impl DramCommand {
    fn new(cycle: u64, parallel_port: u32, bank: u32, row: u64, kind: DramCommandKind) -> Self {
        Self {
            cycle,
            parallel_port,
            bank,
            row,
            kind,
        }
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub const fn parallel_port(&self) -> u32 {
        self.parallel_port
    }

    pub const fn bank(&self) -> u32 {
        self.bank
    }

    pub const fn row(&self) -> u64 {
        self.row
    }

    pub const fn kind(&self) -> DramCommandKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramAccess {
    request: MemoryRequestId,
    kind: DramAccessKind,
    byte_count: u64,
    persistent_ready_cycle: Option<u64>,
    pending_nvm_read_count: usize,
    pending_persistent_write_count: usize,
    low_power_events: Vec<DramLowPowerEvent>,
    low_power_exit_latency_cycles: u64,
    parallel_port: u32,
    bank: u32,
    row: u64,
    row_hit: bool,
    arrival_cycle: u64,
    command_cycle: u64,
    ready_cycle: u64,
    commands: Vec<DramCommand>,
    qos: Option<DramQosAccess>,
}

impl DramAccess {
    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn kind(&self) -> DramAccessKind {
        self.kind
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn persistent_ready_cycle(&self) -> Option<u64> {
        self.persistent_ready_cycle
    }

    pub const fn pending_nvm_read_count(&self) -> usize {
        self.pending_nvm_read_count
    }

    pub const fn pending_persistent_write_count(&self) -> usize {
        self.pending_persistent_write_count
    }

    pub fn low_power_events(&self) -> &[DramLowPowerEvent] {
        &self.low_power_events
    }

    pub const fn low_power_exit_latency_cycles(&self) -> u64 {
        self.low_power_exit_latency_cycles
    }

    pub const fn parallel_port(&self) -> u32 {
        self.parallel_port
    }

    pub const fn bank(&self) -> u32 {
        self.bank
    }

    pub const fn row(&self) -> u64 {
        self.row
    }

    pub const fn row_hit(&self) -> bool {
        self.row_hit
    }

    pub const fn arrival_cycle(&self) -> u64 {
        self.arrival_cycle
    }

    pub const fn command_cycle(&self) -> u64 {
        self.command_cycle
    }

    pub const fn ready_cycle(&self) -> u64 {
        self.ready_cycle
    }

    pub fn commands(&self) -> &[DramCommand] {
        &self.commands
    }

    pub const fn qos(&self) -> Option<DramQosAccess> {
        self.qos
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DramWaitResource {
    Bank { parallel_port: u32, bank: u32 },
    Bus { parallel_port: u32 },
    NvmReadBuffer,
    NvmWriteQueue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DramWaitRecord {
    request: MemoryRequestId,
    resource: DramWaitResource,
    kind: WaitForEdgeKind,
    first_cycle: u64,
    last_cycle: u64,
}

impl DramWaitRecord {
    fn bank_queue(
        request: MemoryRequestId,
        parallel_port: u32,
        bank: u32,
        first_cycle: u64,
        last_cycle: u64,
    ) -> Self {
        Self {
            request,
            resource: DramWaitResource::Bank {
                parallel_port,
                bank,
            },
            kind: WaitForEdgeKind::Queue,
            first_cycle,
            last_cycle,
        }
    }

    fn bus_resource(
        request: MemoryRequestId,
        parallel_port: u32,
        first_cycle: u64,
        last_cycle: u64,
    ) -> Self {
        Self {
            request,
            resource: DramWaitResource::Bus { parallel_port },
            kind: WaitForEdgeKind::Resource,
            first_cycle,
            last_cycle,
        }
    }

    fn nvm_read_buffer(request: MemoryRequestId, first_cycle: u64, last_cycle: u64) -> Self {
        Self {
            request,
            resource: DramWaitResource::NvmReadBuffer,
            kind: WaitForEdgeKind::Resource,
            first_cycle,
            last_cycle,
        }
    }

    fn nvm_write_queue(request: MemoryRequestId, first_cycle: u64, last_cycle: u64) -> Self {
        Self {
            request,
            resource: DramWaitResource::NvmWriteQueue,
            kind: WaitForEdgeKind::Resource,
            first_cycle,
            last_cycle,
        }
    }
}

fn record_dram_wait_interval(
    graph: &mut WaitForGraph,
    wait: &DramWaitRecord,
    target: Option<MemoryTargetId>,
) {
    let source = dram_request_node(wait.request, target);
    let target = dram_resource_node(wait.resource, target);
    graph
        .record_wait(source.clone(), target.clone(), wait.kind, wait.first_cycle)
        .expect("DRAM wait-for labels are generated from typed ids");
    if wait.last_cycle != wait.first_cycle {
        graph
            .record_wait(source, target, wait.kind, wait.last_cycle)
            .expect("DRAM wait-for labels are generated from typed ids");
    }
}

fn dram_request_node(request: MemoryRequestId, target: Option<MemoryTargetId>) -> WaitForNode {
    let label = if let Some(target) = target {
        format!(
            "dram.target.{}.agent.{}.request.{}",
            target.get(),
            request.agent().get(),
            request.sequence()
        )
    } else {
        format!(
            "dram.agent.{}.request.{}",
            request.agent().get(),
            request.sequence()
        )
    };
    WaitForNode::transaction(label).expect("DRAM request wait-for label uses numeric ids")
}

fn dram_resource_node(resource: DramWaitResource, target: Option<MemoryTargetId>) -> WaitForNode {
    let label = match (target, resource) {
        (
            Some(target),
            DramWaitResource::Bank {
                parallel_port,
                bank,
            },
        ) => format!(
            "dram.target.{}.port.{}.bank.{}",
            target.get(),
            parallel_port,
            bank
        ),
        (Some(target), DramWaitResource::Bus { parallel_port }) => {
            format!("dram.target.{}.port.{}.bus", target.get(), parallel_port)
        }
        (Some(target), DramWaitResource::NvmReadBuffer) => {
            format!("dram.target.{}.nvm.read_buffer", target.get())
        }
        (Some(target), DramWaitResource::NvmWriteQueue) => {
            format!("dram.target.{}.nvm.write_queue", target.get())
        }
        (
            None,
            DramWaitResource::Bank {
                parallel_port,
                bank,
            },
        ) => format!("dram.port.{}.bank.{}", parallel_port, bank),
        (None, DramWaitResource::Bus { parallel_port }) => {
            format!("dram.port.{}.bus", parallel_port)
        }
        (None, DramWaitResource::NvmReadBuffer) => "dram.nvm.read_buffer".to_string(),
        (None, DramWaitResource::NvmWriteQueue) => "dram.nvm.write_queue".to_string(),
    };
    WaitForNode::resource(label).expect("DRAM resource wait-for label uses numeric ids")
}

fn merge_wait_for_graph(target: &mut WaitForGraph, source: WaitForGraph) {
    for edge in source.edges() {
        target
            .record_wait(
                edge.source().clone(),
                edge.target().clone(),
                edge.kind(),
                edge.first_observed_tick(),
            )
            .expect("merged wait-for graph already contains valid labels");
        if edge.last_observed_tick() != edge.first_observed_tick() {
            target
                .record_wait(
                    edge.source().clone(),
                    edge.target().clone(),
                    edge.kind(),
                    edge.last_observed_tick(),
                )
                .expect("merged wait-for graph already contains valid labels");
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramBankState {
    open_row: Option<u64>,
    available_cycle: u64,
}

impl DramBankState {
    fn new() -> Self {
        Self {
            open_row: None,
            available_cycle: 0,
        }
    }

    pub const fn from_snapshot(open_row: Option<u64>, available_cycle: u64) -> Self {
        Self {
            open_row,
            available_cycle,
        }
    }

    pub const fn open_row(self) -> Option<u64> {
        self.open_row
    }

    pub const fn available_cycle(self) -> u64 {
        self.available_cycle
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramPortState {
    bus_available_cycle: u64,
    last_access_kind: Option<DramAccessKind>,
    command_window_starts: Vec<u64>,
    last_data_command_cycle: Option<u64>,
    last_bank_group: Option<u32>,
}

impl DramPortState {
    fn new() -> Self {
        Self {
            bus_available_cycle: 0,
            last_access_kind: None,
            command_window_starts: Vec::new(),
            last_data_command_cycle: None,
            last_bank_group: None,
        }
    }

    pub fn from_snapshot(
        bus_available_cycle: u64,
        last_access_kind: Option<DramAccessKind>,
    ) -> Self {
        Self::from_snapshot_with_command_windows(bus_available_cycle, last_access_kind, Vec::new())
    }

    pub fn from_snapshot_with_command_windows(
        bus_available_cycle: u64,
        last_access_kind: Option<DramAccessKind>,
        command_window_starts: Vec<u64>,
    ) -> Self {
        Self::from_snapshot_with_port_history(
            bus_available_cycle,
            last_access_kind,
            command_window_starts,
            None,
            None,
        )
    }

    pub fn from_snapshot_with_port_history(
        bus_available_cycle: u64,
        last_access_kind: Option<DramAccessKind>,
        mut command_window_starts: Vec<u64>,
        last_data_command_cycle: Option<u64>,
        last_bank_group: Option<u32>,
    ) -> Self {
        command_window_starts.sort_unstable();
        Self {
            bus_available_cycle,
            last_access_kind,
            command_window_starts,
            last_data_command_cycle,
            last_bank_group,
        }
    }

    pub const fn bus_available_cycle(&self) -> u64 {
        self.bus_available_cycle
    }

    pub const fn last_access_kind(&self) -> Option<DramAccessKind> {
        self.last_access_kind
    }

    pub fn command_window_starts(&self) -> &[u64] {
        &self.command_window_starts
    }

    pub const fn last_data_command_cycle(&self) -> Option<u64> {
        self.last_data_command_cycle
    }

    pub const fn last_bank_group(&self) -> Option<u32> {
        self.last_bank_group
    }

    fn ready_cycle(
        &self,
        kind: DramAccessKind,
        timing: DramTiming,
        bank_group: Option<u32>,
    ) -> u64 {
        let direction_ready = if self
            .last_access_kind
            .is_some_and(|previous| previous != kind)
        {
            self.bus_available_cycle + timing.bus_turnaround()
        } else {
            self.bus_available_cycle
        };
        let same_group_ready = match (
            bank_group,
            self.last_bank_group,
            self.last_data_command_cycle,
            timing.same_bank_group_burst_spacing(),
        ) {
            (Some(bank_group), Some(last_bank_group), Some(command_cycle), Some(spacing))
                if bank_group == last_bank_group =>
            {
                command_cycle + spacing
            }
            _ => 0,
        };
        direction_ready.max(same_group_ready)
    }

    fn reserve_command_window(&mut self, timing: DramTiming, mut command_cycle: u64) -> u64 {
        let Some(command_window) = timing.command_window() else {
            return command_cycle;
        };
        loop {
            let window_cycles = command_window.window_cycles();
            let window_start = command_cycle - command_cycle % window_cycles;
            self.command_window_starts
                .retain(|start| *start + window_cycles > command_cycle);
            let command_count = self
                .command_window_starts
                .iter()
                .filter(|start| **start == window_start)
                .count();
            if command_count < command_window.max_commands() as usize {
                self.command_window_starts.push(window_start);
                self.command_window_starts.sort_unstable();
                return command_cycle;
            }
            command_cycle = window_start + window_cycles;
        }
    }

    fn set_bus_state(
        &mut self,
        bus_available_cycle: u64,
        last_access_kind: DramAccessKind,
        command_cycle: u64,
        bank_group: Option<u32>,
    ) {
        self.bus_available_cycle = bus_available_cycle;
        self.last_access_kind = Some(last_access_kind);
        self.last_data_command_cycle = Some(command_cycle);
        self.last_bank_group = bank_group;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramController {
    geometry: DramGeometry,
    timing: DramTiming,
    banks: Vec<DramBankState>,
    ports: Vec<DramPortState>,
    nvm_media_timing: Option<NvmMediaTiming>,
    nvm_pending_read_completions: Vec<u64>,
    nvm_pending_write_completions: Vec<u64>,
    activity_log: Vec<DramAccess>,
    wait_log: Vec<DramWaitRecord>,
}

impl DramController {
    pub fn new(geometry: DramGeometry, timing: DramTiming) -> Self {
        Self::with_parallel_port_count(geometry, timing, 1)
    }

    fn with_parallel_port_count(
        geometry: DramGeometry,
        timing: DramTiming,
        parallel_port_count: u32,
    ) -> Self {
        Self {
            geometry,
            timing,
            banks: vec![
                DramBankState::new();
                geometry.bank_count() as usize * parallel_port_count as usize
            ],
            ports: vec![DramPortState::new(); parallel_port_count as usize],
            nvm_media_timing: None,
            nvm_pending_read_completions: Vec::new(),
            nvm_pending_write_completions: Vec::new(),
            activity_log: Vec::new(),
            wait_log: Vec::new(),
        }
    }

    fn with_config(config: DramControllerConfig) -> Self {
        let mut controller = Self::with_parallel_port_count(
            config.geometry(),
            config.timing(),
            config.parallel_port_count(),
        );
        controller.nvm_media_timing = config.nvm_media_timing();
        controller
    }

    pub const fn geometry(&self) -> DramGeometry {
        self.geometry
    }

    pub const fn timing(&self) -> DramTiming {
        self.timing
    }

    pub fn bank_state(&self, bank: u32) -> Option<DramBankState> {
        self.banks.get(bank as usize).copied()
    }

    pub fn port_state(&self, parallel_port: u32) -> Option<DramPortState> {
        self.ports.get(parallel_port as usize).cloned()
    }

    pub fn parallel_port_count(&self) -> u32 {
        self.ports.len() as u32
    }

    pub const fn nvm_media_timing(&self) -> Option<NvmMediaTiming> {
        self.nvm_media_timing
    }

    pub fn nvm_pending_read_completions(&self) -> &[u64] {
        &self.nvm_pending_read_completions
    }

    pub fn nvm_pending_write_completions(&self) -> &[u64] {
        &self.nvm_pending_write_completions
    }

    pub fn snapshot(&self) -> DramControllerSnapshot {
        DramControllerSnapshot::with_ports(
            self.geometry,
            self.timing,
            self.banks.clone(),
            self.ports.clone(),
        )
        .with_nvm_media_state(
            self.nvm_media_timing,
            self.nvm_pending_read_completions.clone(),
            self.nvm_pending_write_completions.clone(),
        )
    }

    pub fn restore(&mut self, snapshot: &DramControllerSnapshot) {
        *self = Self::from_snapshot(snapshot);
    }

    pub fn from_snapshot(snapshot: &DramControllerSnapshot) -> Self {
        Self {
            geometry: snapshot.geometry(),
            timing: snapshot.timing(),
            banks: snapshot.banks().to_vec(),
            ports: if snapshot.ports().is_empty() {
                vec![DramPortState::new()]
            } else {
                snapshot.ports().to_vec()
            },
            nvm_media_timing: snapshot.nvm_media_timing(),
            nvm_pending_read_completions: snapshot.nvm_pending_read_completions().to_vec(),
            nvm_pending_write_completions: snapshot.nvm_pending_write_completions().to_vec(),
            activity_log: Vec::new(),
            wait_log: Vec::new(),
        }
    }

    pub fn mark_activity(&self) -> DramActivityMarker {
        DramActivityMarker::new(self.activity_log.len())
    }

    pub fn mark_wait_for(&self) -> DramWaitForMarker {
        DramWaitForMarker::new(self.wait_log.len())
    }

    pub fn bank_activities(&self) -> BTreeMap<(u32, u32), DramBankActivity> {
        collect_dram_bank_activity(&self.activity_log)
    }

    pub fn bank_activities_since(
        &self,
        marker: DramActivityMarker,
    ) -> BTreeMap<(u32, u32), DramBankActivity> {
        let Some(accesses) = self.activity_log.get(marker.offset..) else {
            return BTreeMap::new();
        };
        collect_dram_bank_activity(accesses)
    }

    pub fn bank_activity(&self, parallel_port: u32, bank: u32) -> Option<DramBankActivity> {
        self.bank_activities().remove(&(parallel_port, bank))
    }

    pub fn port_activities(&self) -> BTreeMap<u32, DramPortActivity> {
        collect_dram_port_activity(&self.activity_log)
    }

    pub fn port_activities_since(
        &self,
        marker: DramActivityMarker,
    ) -> BTreeMap<u32, DramPortActivity> {
        let Some(accesses) = self.activity_log.get(marker.offset..) else {
            return BTreeMap::new();
        };
        collect_dram_port_activity(accesses)
    }

    pub fn port_activity(&self, parallel_port: u32) -> Option<DramPortActivity> {
        self.port_activities().remove(&parallel_port)
    }

    pub fn activity_profile(&self) -> DramActivityProfile {
        DramActivityProfile::from_activities(&self.port_activities(), &self.bank_activities())
    }

    pub fn activity_profile_until(&self, end_cycle: u64) -> DramActivityProfile {
        let mut bank_activities = self.bank_activities();
        self.record_terminal_low_power_activity(&mut bank_activities, end_cycle);
        DramActivityProfile::from_activities(&self.port_activities(), &bank_activities)
    }

    pub fn activity_profile_since(&self, marker: DramActivityMarker) -> DramActivityProfile {
        DramActivityProfile::from_activities(
            &self.port_activities_since(marker),
            &self.bank_activities_since(marker),
        )
    }

    pub fn activity_profile_since_until(
        &self,
        marker: DramActivityMarker,
        end_cycle: u64,
    ) -> DramActivityProfile {
        let mut bank_activities = self.bank_activities_since(marker);
        self.record_terminal_low_power_activity(&mut bank_activities, end_cycle);
        DramActivityProfile::from_activities(&self.port_activities_since(marker), &bank_activities)
    }

    fn record_terminal_low_power_activity(
        &self,
        bank_activities: &mut BTreeMap<(u32, u32), DramBankActivity>,
        end_cycle: u64,
    ) {
        let Some(low_power_timing) = self.timing.low_power_timing() else {
            return;
        };
        let bank_count = self.geometry.bank_count() as usize;
        let active_banks = bank_activities.keys().copied().collect::<Vec<_>>();
        for (parallel_port, local_bank) in active_banks {
            let bank_index = parallel_port as usize * bank_count + local_bank as usize;
            let Some(bank) = self.banks.get(bank_index) else {
                continue;
            };
            let Some(port) = self.ports.get(parallel_port as usize) else {
                continue;
            };
            let events = low_power::events_for_idle_window(
                low_power_timing,
                parallel_port,
                port.bus_available_cycle().max(bank.available_cycle()),
                end_cycle,
                bank.open_row().is_some(),
            );
            if !events.is_empty() {
                bank_activities
                    .entry((parallel_port, local_bank))
                    .or_default()
                    .record_terminal_low_power_events(&events);
            }
        }
    }

    pub fn clear_activity(&mut self) {
        self.activity_log.clear();
    }

    pub fn wait_for_graph_since(&self, marker: DramWaitForMarker) -> WaitForGraph {
        self.wait_for_graph_since_with_target(marker, None)
    }

    fn wait_for_graph_since_with_target(
        &self,
        marker: DramWaitForMarker,
        target: Option<MemoryTargetId>,
    ) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        let Some(records) = self.wait_log.get(marker.offset..) else {
            return graph;
        };
        for wait in records {
            record_dram_wait_interval(&mut graph, wait, target);
        }
        graph
    }

    pub fn schedule(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramAccess, DramError> {
        self.schedule_with_qos(arrival_cycle, request, None)
    }

    pub(crate) fn schedule_with_qos(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
        qos: Option<DramQosAccess>,
    ) -> Result<DramAccess, DramError> {
        let kind = DramAccessKind::from_operation(request)?;
        let decoded = self
            .geometry
            .decode_request(self.parallel_port_count(), request)?;
        let port_index = decoded.parallel_port as usize;
        let mut port = self.ports[port_index].clone();
        let bank_index = port_index * self.geometry.bank_count() as usize + decoded.bank as usize;
        let bank = &mut self.banks[bank_index];
        let low_power_events = if let Some(low_power_timing) = self.timing.low_power_timing() {
            low_power::events_for_idle_window(
                low_power_timing,
                decoded.parallel_port,
                port.bus_available_cycle().max(bank.available_cycle()),
                arrival_cycle,
                bank.open_row.is_some(),
            )
        } else {
            Vec::new()
        };
        let low_power_exit_latency_cycles = low_power_events
            .last()
            .and_then(|event| {
                self.timing
                    .low_power_timing()
                    .map(|timing| timing.exit_latency_for_state(event.state()))
            })
            .unwrap_or(0);
        let effective_arrival_cycle = arrival_cycle.saturating_add(low_power_exit_latency_cycles);
        let bus_ready_cycle = port.ready_cycle(kind, self.timing, decoded.bank_group);
        let mut commands = Vec::new();
        let mut waits = Vec::new();
        if bank.available_cycle > effective_arrival_cycle {
            waits.push(DramWaitRecord::bank_queue(
                request.id(),
                decoded.parallel_port,
                decoded.bank,
                effective_arrival_cycle,
                bank.available_cycle - 1,
            ));
        }
        let mut next_cycle = effective_arrival_cycle.max(bank.available_cycle);
        let row_hit = bank.open_row == Some(decoded.row);

        if !row_hit {
            if let Some(open_row) = bank.open_row {
                let precharge_cycle = reserve_dram_command(
                    &mut port,
                    self.timing,
                    request.id(),
                    decoded.parallel_port,
                    next_cycle,
                    &mut waits,
                );
                commands.push(DramCommand::new(
                    precharge_cycle,
                    decoded.parallel_port,
                    decoded.bank,
                    open_row,
                    DramCommandKind::Precharge,
                ));
                next_cycle = precharge_cycle + self.timing.precharge_latency();
            }
            let activate_cycle = reserve_dram_command(
                &mut port,
                self.timing,
                request.id(),
                decoded.parallel_port,
                next_cycle,
                &mut waits,
            );
            commands.push(DramCommand::new(
                activate_cycle,
                decoded.parallel_port,
                decoded.bank,
                decoded.row,
                DramCommandKind::Activate,
            ));
            next_cycle = activate_cycle + self.timing.activate_latency();
            bank.open_row = Some(decoded.row);
        }

        if bus_ready_cycle > next_cycle {
            waits.push(DramWaitRecord::bus_resource(
                request.id(),
                decoded.parallel_port,
                next_cycle,
                bus_ready_cycle - 1,
            ));
        }
        let mut command_cycle = next_cycle.max(bus_ready_cycle);
        if let Some(nvm_media_timing) = self.nvm_media_timing {
            match kind {
                DramAccessKind::Read => {
                    let requested_cycle = command_cycle;
                    command_cycle = reserve_nvm_completion_slot(
                        &mut self.nvm_pending_read_completions,
                        nvm_media_timing.max_pending_reads(),
                        command_cycle,
                    );
                    if command_cycle > requested_cycle {
                        waits.push(DramWaitRecord::nvm_read_buffer(
                            request.id(),
                            requested_cycle,
                            command_cycle - 1,
                        ));
                    }
                }
                DramAccessKind::Write => {
                    let requested_cycle = command_cycle;
                    command_cycle = reserve_nvm_completion_slot(
                        &mut self.nvm_pending_write_completions,
                        nvm_media_timing.max_pending_writes(),
                        command_cycle,
                    );
                    if command_cycle > requested_cycle {
                        waits.push(DramWaitRecord::nvm_write_queue(
                            request.id(),
                            requested_cycle,
                            command_cycle - 1,
                        ));
                    }
                }
            }
        }
        command_cycle = reserve_dram_command(
            &mut port,
            self.timing,
            request.id(),
            decoded.parallel_port,
            command_cycle,
            &mut waits,
        );
        commands.push(DramCommand::new(
            command_cycle,
            decoded.parallel_port,
            decoded.bank,
            decoded.row,
            kind.command_kind(),
        ));
        let ready_cycle = match self.nvm_media_timing {
            Some(nvm_media_timing) => match kind {
                DramAccessKind::Read => {
                    command_cycle
                        + nvm_media_timing.read_media_latency()
                        + nvm_media_timing.send_latency()
                }
                DramAccessKind::Write => command_cycle + nvm_media_timing.send_latency(),
            },
            None => command_cycle + self.timing.data_latency(kind),
        };
        let persistent_ready_cycle = if kind == DramAccessKind::Write {
            self.nvm_media_timing.map(|nvm_media_timing| {
                ready_cycle.max(bank.available_cycle) + nvm_media_timing.write_media_latency()
            })
        } else {
            None
        };

        bank.available_cycle = persistent_ready_cycle.unwrap_or(ready_cycle);
        port.set_bus_state(
            command_cycle + self.timing.burst_spacing(),
            kind,
            command_cycle,
            decoded.bank_group,
        );
        self.ports[port_index] = port;
        let pending_nvm_read_count =
            if kind == DramAccessKind::Read && self.nvm_media_timing.is_some() {
                self.nvm_pending_read_completions.push(ready_cycle);
                self.nvm_pending_read_completions.sort_unstable();
                self.nvm_pending_read_completions.len()
            } else {
                0
            };
        let pending_persistent_write_count =
            if let Some(persistent_ready_cycle) = persistent_ready_cycle {
                self.nvm_pending_write_completions
                    .push(persistent_ready_cycle);
                self.nvm_pending_write_completions.sort_unstable();
                self.nvm_pending_write_completions.len()
            } else {
                0
            };

        let access = DramAccess {
            request: request.id(),
            kind,
            byte_count: request.size().bytes(),
            persistent_ready_cycle,
            pending_nvm_read_count,
            pending_persistent_write_count,
            low_power_events,
            low_power_exit_latency_cycles,
            parallel_port: decoded.parallel_port,
            bank: decoded.bank,
            row: decoded.row,
            row_hit,
            arrival_cycle,
            command_cycle,
            ready_cycle,
            commands,
            qos,
        };
        self.activity_log.push(access.clone());
        self.wait_log.extend(waits);
        Ok(access)
    }

    pub fn schedule_qos_batch<'a, I>(
        &mut self,
        arrival_cycle: u64,
        requests: I,
        arbiter: &mut QosQueueArbiter,
    ) -> Result<Vec<DramAccess>, DramError>
    where
        I: IntoIterator<Item = DramQosRequest<'a>>,
    {
        qos::schedule_qos_batch(
            self,
            arrival_cycle,
            requests,
            arbiter,
            DramQosTurnaroundPolicy::RequestOrder,
        )
    }

    pub fn schedule_qos_batch_with_turnaround_policy<'a, I>(
        &mut self,
        arrival_cycle: u64,
        requests: I,
        arbiter: &mut QosQueueArbiter,
        turnaround: DramQosTurnaroundPolicy,
    ) -> Result<Vec<DramAccess>, DramError>
    where
        I: IntoIterator<Item = DramQosRequest<'a>>,
    {
        qos::schedule_qos_batch(self, arrival_cycle, requests, arbiter, turnaround)
    }

    pub fn schedule_qos_batch_with_policy<'a, I>(
        &mut self,
        arrival_cycle: u64,
        requests: I,
        arbiter: &mut QosQueueArbiter,
        policy: DramQosSchedulingPolicy,
    ) -> Result<Vec<DramAccess>, DramError>
    where
        I: IntoIterator<Item = DramQosRequest<'a>>,
    {
        qos::schedule_qos_batch_with_policy(self, arrival_cycle, requests, arbiter, policy)
    }
}

fn reserve_nvm_completion_slot(
    pending_completions: &mut Vec<u64>,
    max_pending: u32,
    mut command_cycle: u64,
) -> u64 {
    pending_completions.retain(|completion| *completion > command_cycle);
    if pending_completions.len() >= max_pending as usize {
        if let Some(next_completion) = pending_completions.iter().copied().min() {
            command_cycle = command_cycle.max(next_completion);
            pending_completions.retain(|completion| *completion > command_cycle);
        }
    }
    command_cycle
}

fn reserve_dram_command(
    port: &mut DramPortState,
    timing: DramTiming,
    request: MemoryRequestId,
    parallel_port: u32,
    requested_cycle: u64,
    waits: &mut Vec<DramWaitRecord>,
) -> u64 {
    let command_cycle = port.reserve_command_window(timing, requested_cycle);
    if command_cycle > requested_cycle {
        waits.push(DramWaitRecord::bus_resource(
            request,
            parallel_port,
            requested_cycle,
            command_cycle - 1,
        ));
    }
    command_cycle
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramControllerSnapshot {
    geometry: DramGeometry,
    timing: DramTiming,
    banks: Vec<DramBankState>,
    ports: Vec<DramPortState>,
    nvm_media_timing: Option<NvmMediaTiming>,
    nvm_pending_read_completions: Vec<u64>,
    nvm_pending_write_completions: Vec<u64>,
}

impl DramControllerSnapshot {
    pub fn new(
        geometry: DramGeometry,
        timing: DramTiming,
        banks: Vec<DramBankState>,
        bus_available_cycle: u64,
        last_access_kind: Option<DramAccessKind>,
    ) -> Self {
        Self {
            geometry,
            timing,
            banks,
            ports: vec![DramPortState::from_snapshot(
                bus_available_cycle,
                last_access_kind,
            )],
            nvm_media_timing: None,
            nvm_pending_read_completions: Vec::new(),
            nvm_pending_write_completions: Vec::new(),
        }
    }

    pub const fn with_ports(
        geometry: DramGeometry,
        timing: DramTiming,
        banks: Vec<DramBankState>,
        ports: Vec<DramPortState>,
    ) -> Self {
        Self {
            geometry,
            timing,
            banks,
            ports,
            nvm_media_timing: None,
            nvm_pending_read_completions: Vec::new(),
            nvm_pending_write_completions: Vec::new(),
        }
    }

    pub fn with_nvm_media_state(
        mut self,
        nvm_media_timing: Option<NvmMediaTiming>,
        nvm_pending_read_completions: Vec<u64>,
        nvm_pending_write_completions: Vec<u64>,
    ) -> Self {
        self.nvm_media_timing = nvm_media_timing;
        self.nvm_pending_read_completions = nvm_pending_read_completions;
        self.nvm_pending_read_completions.sort_unstable();
        self.nvm_pending_write_completions = nvm_pending_write_completions;
        self.nvm_pending_write_completions.sort_unstable();
        self
    }

    pub const fn geometry(&self) -> DramGeometry {
        self.geometry
    }

    pub const fn timing(&self) -> DramTiming {
        self.timing
    }

    pub fn banks(&self) -> &[DramBankState] {
        &self.banks
    }

    pub fn bus_available_cycle(&self) -> u64 {
        self.ports
            .first()
            .map_or(0, |port| port.bus_available_cycle())
    }

    pub fn last_access_kind(&self) -> Option<DramAccessKind> {
        self.ports.first().and_then(|port| port.last_access_kind())
    }

    pub fn ports(&self) -> &[DramPortState] {
        &self.ports
    }

    pub fn parallel_port_count(&self) -> u32 {
        self.ports.len() as u32
    }

    pub const fn nvm_media_timing(&self) -> Option<NvmMediaTiming> {
        self.nvm_media_timing
    }

    pub fn nvm_pending_read_completions(&self) -> &[u64] {
        &self.nvm_pending_read_completions
    }

    pub fn nvm_pending_write_completions(&self) -> &[u64] {
        &self.nvm_pending_write_completions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramControllerConfig {
    target: MemoryTargetId,
    layout: CacheLineLayout,
    geometry: DramGeometry,
    timing: DramTiming,
    parallel_port_count: u32,
    nvm_media_timing: Option<NvmMediaTiming>,
}

impl DramControllerConfig {
    pub const fn new(
        target: MemoryTargetId,
        layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Self {
        Self {
            target,
            layout,
            geometry,
            timing,
            parallel_port_count: 1,
            nvm_media_timing: None,
        }
    }

    const fn with_profile_parallel_ports(mut self, parallel_port_count: u32) -> Self {
        self.parallel_port_count = parallel_port_count;
        self
    }

    pub const fn with_nvm_media_timing(mut self, nvm_media_timing: NvmMediaTiming) -> Self {
        self.nvm_media_timing = Some(nvm_media_timing);
        self
    }

    pub const fn target(self) -> MemoryTargetId {
        self.target
    }

    pub const fn layout(self) -> CacheLineLayout {
        self.layout
    }

    pub const fn geometry(self) -> DramGeometry {
        self.geometry
    }

    pub const fn timing(self) -> DramTiming {
        self.timing
    }

    pub const fn parallel_port_count(self) -> u32 {
        self.parallel_port_count
    }

    pub const fn nvm_media_timing(self) -> Option<NvmMediaTiming> {
        self.nvm_media_timing
    }
}
