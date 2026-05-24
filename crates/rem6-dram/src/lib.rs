use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

mod activity;
mod memory_controller;
mod profile;
mod qos;

use activity::{collect_dram_bank_activity, collect_dram_port_activity};
pub use activity::{
    DramActivityMarker, DramActivityProfile, DramBankActivity, DramMemoryActivityMarker,
    DramMemoryActivityProfile, DramPortActivity, DramTargetActivity,
};
pub use profile::{
    DramMemoryTechnology, DramProfileField, ExternalMemoryProfile, ExternalMemoryTopology,
    NvmMediaTiming, NvmMediaTimingField,
};
pub use qos::{DramQosAccess, DramQosRequest, DramQosSchedulingPolicy, DramQosTurnaroundPolicy};

use rem6_fabric::{QosError, QosQueueArbiter};
use rem6_kernel::{WaitForEdgeKind, WaitForGraph, WaitForNode};
use rem6_memory::{
    AccessSize, Address, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse, MemoryTargetId, PartitionedMemorySnapshot,
    PartitionedMemoryStore,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramTimingField {
    ActivateLatency,
    ReadLatency,
    WriteLatency,
    PrechargeLatency,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DramError {
    ZeroBankCount,
    ZeroRowSize,
    ZeroLineSize,
    RowSizeNotLineMultiple {
        row_size: u64,
        line_size: u64,
    },
    ZeroTimingLatency {
        field: DramTimingField,
    },
    ZeroProfileTopology {
        technology: DramMemoryTechnology,
        field: DramProfileField,
    },
    ZeroNvmMediaTiming {
        field: NvmMediaTimingField,
    },
    NvmMediaTimingOnVolatileProfile {
        technology: DramMemoryTechnology,
    },
    LineSizeMismatch {
        request: MemoryRequestId,
        expected: u64,
        actual: u64,
    },
    RequestCrossesRow {
        request: MemoryRequestId,
        start_bank: u32,
        start_row: u64,
        end_bank: u32,
        end_row: u64,
    },
    UnsupportedOperation {
        request: MemoryRequestId,
        operation: MemoryOperation,
    },
    Qos {
        source: QosError,
    },
}

impl fmt::Display for DramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBankCount => write!(formatter, "DRAM bank count must be nonzero"),
            Self::ZeroRowSize => write!(formatter, "DRAM row size must be nonzero"),
            Self::ZeroLineSize => write!(formatter, "DRAM line size must be nonzero"),
            Self::RowSizeNotLineMultiple {
                row_size,
                line_size,
            } => write!(
                formatter,
                "DRAM row size {row_size} is not a multiple of line size {line_size}"
            ),
            Self::ZeroTimingLatency { field } => {
                write!(formatter, "DRAM timing field {field:?} must be nonzero")
            }
            Self::ZeroProfileTopology { technology, field } => write!(
                formatter,
                "DRAM profile {technology:?} topology field {field:?} must be nonzero"
            ),
            Self::ZeroNvmMediaTiming { field } => {
                write!(formatter, "NVM media timing field {field:?} must be nonzero")
            }
            Self::NvmMediaTimingOnVolatileProfile { technology } => write!(
                formatter,
                "NVM media timing cannot be attached to {technology:?} memory profiles"
            ),
            Self::LineSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "request {} from agent {} uses {actual}-byte lines but DRAM expects {expected}",
                request.sequence(),
                request.agent().get()
            ),
            Self::RequestCrossesRow {
                request,
                start_bank,
                start_row,
                end_bank,
                end_row,
            } => write!(
                formatter,
                "request {} from agent {} crosses DRAM row from bank {start_bank} row {start_row} to bank {end_bank} row {end_row}",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnsupportedOperation { request, operation } => write!(
                formatter,
                "request {} from agent {} uses unsupported DRAM operation {operation:?}",
                request.sequence(),
                request.agent().get()
            ),
            Self::Qos { source } => write!(formatter, "DRAM QoS scheduling failed: {source}"),
        }
    }
}

impl Error for DramError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Qos { source } => Some(source),
            Self::ZeroBankCount
            | Self::ZeroRowSize
            | Self::ZeroLineSize
            | Self::RowSizeNotLineMultiple { .. }
            | Self::ZeroTimingLatency { .. }
            | Self::ZeroProfileTopology { .. }
            | Self::ZeroNvmMediaTiming { .. }
            | Self::NvmMediaTimingOnVolatileProfile { .. }
            | Self::LineSizeMismatch { .. }
            | Self::RequestCrossesRow { .. }
            | Self::UnsupportedOperation { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DramMemoryError {
    Memory(MemoryError),
    Dram {
        target: MemoryTargetId,
        source: DramError,
    },
    TargetLineSizeMismatch {
        target: MemoryTargetId,
        layout: u64,
        geometry: u64,
    },
    MissingDramTarget {
        target: MemoryTargetId,
    },
}

impl fmt::Display for DramMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Dram { target, source } => {
                write!(formatter, "DRAM target {} rejected request: {source}", target.get())
            }
            Self::TargetLineSizeMismatch {
                target,
                layout,
                geometry,
            } => write!(
                formatter,
                "DRAM target {} uses {geometry}-byte geometry lines but memory layout uses {layout}",
                target.get()
            ),
            Self::MissingDramTarget { target } => {
                write!(formatter, "DRAM target {} is missing timing state", target.get())
            }
        }
    }
}

impl Error for DramMemoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Dram { source, .. } => Some(source),
            Self::TargetLineSizeMismatch { .. } | Self::MissingDramTarget { .. } => None,
        }
    }
}

impl From<MemoryError> for DramMemoryError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramWaitForMarker {
    offset: usize,
}

impl DramWaitForMarker {
    const fn new(offset: usize) -> Self {
        Self { offset }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryWaitForMarker {
    targets: BTreeMap<MemoryTargetId, DramWaitForMarker>,
}

impl DramMemoryWaitForMarker {
    fn new(targets: BTreeMap<MemoryTargetId, DramWaitForMarker>) -> Self {
        Self { targets }
    }

    fn marker_for(&self, target: MemoryTargetId) -> Option<DramWaitForMarker> {
        self.targets.get(&target).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramTiming {
    activate_latency: u64,
    read_latency: u64,
    write_latency: u64,
    precharge_latency: u64,
    bus_turnaround: u64,
}

impl DramTiming {
    pub fn new(
        activate_latency: u64,
        read_latency: u64,
        write_latency: u64,
        precharge_latency: u64,
        bus_turnaround: u64,
    ) -> Result<Self, DramError> {
        if activate_latency == 0 {
            return Err(DramError::ZeroTimingLatency {
                field: DramTimingField::ActivateLatency,
            });
        }
        if read_latency == 0 {
            return Err(DramError::ZeroTimingLatency {
                field: DramTimingField::ReadLatency,
            });
        }
        if write_latency == 0 {
            return Err(DramError::ZeroTimingLatency {
                field: DramTimingField::WriteLatency,
            });
        }
        if precharge_latency == 0 {
            return Err(DramError::ZeroTimingLatency {
                field: DramTimingField::PrechargeLatency,
            });
        }

        Ok(Self {
            activate_latency,
            read_latency,
            write_latency,
            precharge_latency,
            bus_turnaround,
        })
    }

    pub const fn activate_latency(self) -> u64 {
        self.activate_latency
    }

    pub const fn read_latency(self) -> u64 {
        self.read_latency
    }

    pub const fn write_latency(self) -> u64 {
        self.write_latency
    }

    pub const fn precharge_latency(self) -> u64 {
        self.precharge_latency
    }

    pub const fn bus_turnaround(self) -> u64 {
        self.bus_turnaround
    }

    fn data_latency(self, kind: DramAccessKind) -> u64 {
        match kind {
            DramAccessKind::Read => self.read_latency,
            DramAccessKind::Write => self.write_latency,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramGeometry {
    bank_count: u32,
    row_size: u64,
    line_size: u64,
    lines_per_row: u64,
}

impl DramGeometry {
    pub fn new(bank_count: u32, row_size: u64, line_size: u64) -> Result<Self, DramError> {
        if bank_count == 0 {
            return Err(DramError::ZeroBankCount);
        }
        if row_size == 0 {
            return Err(DramError::ZeroRowSize);
        }
        if line_size == 0 {
            return Err(DramError::ZeroLineSize);
        }
        if !row_size.is_multiple_of(line_size) {
            return Err(DramError::RowSizeNotLineMultiple {
                row_size,
                line_size,
            });
        }

        Ok(Self {
            bank_count,
            row_size,
            line_size,
            lines_per_row: row_size / line_size,
        })
    }

    pub const fn bank_count(self) -> u32 {
        self.bank_count
    }

    pub const fn row_size(self) -> u64 {
        self.row_size
    }

    pub const fn line_size(self) -> u64 {
        self.line_size
    }

    pub const fn lines_per_row(self) -> u64 {
        self.lines_per_row
    }

    fn decode_address(self, parallel_port_count: u32, address: u64) -> DecodedDramAddress {
        let line = address / self.line_size;
        let parallel_port = (line % u64::from(parallel_port_count)) as u32;
        let port_line = line / u64::from(parallel_port_count);
        let bank = (port_line % u64::from(self.bank_count)) as u32;
        let row = port_line / (u64::from(self.bank_count) * self.lines_per_row);
        DecodedDramAddress {
            parallel_port,
            bank,
            row,
        }
    }

    fn decode_request(
        self,
        parallel_port_count: u32,
        request: &MemoryRequest,
    ) -> Result<DecodedDramAddress, DramError> {
        if request.line_layout().bytes() != self.line_size {
            return Err(DramError::LineSizeMismatch {
                request: request.id(),
                expected: self.line_size,
                actual: request.line_layout().bytes(),
            });
        }

        let start = self.decode_address(parallel_port_count, request.range().start().get());
        let end = self.decode_address(parallel_port_count, request.range().end().get() - 1);
        if start != end {
            return Err(DramError::RequestCrossesRow {
                request: request.id(),
                start_bank: start.bank,
                start_row: start.row,
                end_bank: end.bank,
                end_row: end.row,
            });
        }

        Ok(start)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedDramAddress {
    parallel_port: u32,
    bank: u32,
    row: u64,
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
    pending_persistent_write_count: usize,
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

    pub const fn pending_persistent_write_count(&self) -> usize {
        self.pending_persistent_write_count
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramPortState {
    bus_available_cycle: u64,
    last_access_kind: Option<DramAccessKind>,
}

impl DramPortState {
    fn new() -> Self {
        Self {
            bus_available_cycle: 0,
            last_access_kind: None,
        }
    }

    pub const fn from_snapshot(
        bus_available_cycle: u64,
        last_access_kind: Option<DramAccessKind>,
    ) -> Self {
        Self {
            bus_available_cycle,
            last_access_kind,
        }
    }

    pub const fn bus_available_cycle(self) -> u64 {
        self.bus_available_cycle
    }

    pub const fn last_access_kind(self) -> Option<DramAccessKind> {
        self.last_access_kind
    }

    fn ready_cycle(self, kind: DramAccessKind, timing: DramTiming) -> u64 {
        if self
            .last_access_kind
            .is_some_and(|previous| previous != kind)
        {
            self.bus_available_cycle + timing.bus_turnaround()
        } else {
            self.bus_available_cycle
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramController {
    geometry: DramGeometry,
    timing: DramTiming,
    banks: Vec<DramBankState>,
    ports: Vec<DramPortState>,
    nvm_media_timing: Option<NvmMediaTiming>,
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
        self.ports.get(parallel_port as usize).copied()
    }

    pub fn parallel_port_count(&self) -> u32 {
        self.ports.len() as u32
    }

    pub const fn nvm_media_timing(&self) -> Option<NvmMediaTiming> {
        self.nvm_media_timing
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

    pub fn activity_profile_since(&self, marker: DramActivityMarker) -> DramActivityProfile {
        DramActivityProfile::from_activities(
            &self.port_activities_since(marker),
            &self.bank_activities_since(marker),
        )
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
        let bus_ready_cycle = self.ports[port_index].ready_cycle(kind, self.timing);
        let bank_index = port_index * self.geometry.bank_count() as usize + decoded.bank as usize;
        let bank = &mut self.banks[bank_index];
        let mut commands = Vec::new();
        let mut waits = Vec::new();
        if bank.available_cycle > arrival_cycle {
            waits.push(DramWaitRecord::bank_queue(
                request.id(),
                decoded.parallel_port,
                decoded.bank,
                arrival_cycle,
                bank.available_cycle - 1,
            ));
        }
        let mut next_cycle = arrival_cycle.max(bank.available_cycle);
        let row_hit = bank.open_row == Some(decoded.row);

        if !row_hit {
            if let Some(open_row) = bank.open_row {
                commands.push(DramCommand::new(
                    next_cycle,
                    decoded.parallel_port,
                    decoded.bank,
                    open_row,
                    DramCommandKind::Precharge,
                ));
                next_cycle += self.timing.precharge_latency();
            }
            commands.push(DramCommand::new(
                next_cycle,
                decoded.parallel_port,
                decoded.bank,
                decoded.row,
                DramCommandKind::Activate,
            ));
            next_cycle += self.timing.activate_latency();
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
        if kind == DramAccessKind::Write {
            if let Some(nvm_media_timing) = self.nvm_media_timing {
                command_cycle = reserve_nvm_write_slot(
                    &mut self.nvm_pending_write_completions,
                    nvm_media_timing.max_pending_writes(),
                    command_cycle,
                );
            }
        }
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
        self.ports[port_index] = DramPortState::from_snapshot(command_cycle, Some(kind));
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
            pending_persistent_write_count,
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

fn reserve_nvm_write_slot(
    pending_write_completions: &mut Vec<u64>,
    max_pending_writes: u32,
    mut command_cycle: u64,
) -> u64 {
    pending_write_completions.retain(|completion| *completion > command_cycle);
    if pending_write_completions.len() >= max_pending_writes as usize {
        if let Some(next_completion) = pending_write_completions.iter().copied().min() {
            command_cycle = command_cycle.max(next_completion);
            pending_write_completions.retain(|completion| *completion > command_cycle);
        }
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
            nvm_pending_write_completions: Vec::new(),
        }
    }

    pub fn with_nvm_media_state(
        mut self,
        nvm_media_timing: Option<NvmMediaTiming>,
        nvm_pending_write_completions: Vec<u64>,
    ) -> Self {
        self.nvm_media_timing = nvm_media_timing;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryOutcome {
    target: MemoryTargetId,
    dram_access: DramAccess,
    response: Option<MemoryResponse>,
}

impl DramMemoryOutcome {
    fn new(
        target: MemoryTargetId,
        dram_access: DramAccess,
        response: Option<MemoryResponse>,
    ) -> Self {
        Self {
            target,
            dram_access,
            response,
        }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn arrival_cycle(&self) -> u64 {
        self.dram_access.arrival_cycle()
    }

    pub const fn ready_cycle(&self) -> u64 {
        self.dram_access.ready_cycle()
    }

    pub const fn dram_access(&self) -> &DramAccess {
        &self.dram_access
    }

    pub fn response(&self) -> Option<&MemoryResponse> {
        self.response.as_ref()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DramMemoryController {
    store: PartitionedMemoryStore,
    dram: BTreeMap<MemoryTargetId, DramController>,
    profiles: BTreeMap<MemoryTargetId, ExternalMemoryProfile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemorySnapshot {
    store: PartitionedMemorySnapshot,
    targets: Vec<DramMemoryTargetSnapshot>,
}

impl DramMemorySnapshot {
    pub fn new(store: PartitionedMemorySnapshot, targets: Vec<DramMemoryTargetSnapshot>) -> Self {
        Self { store, targets }
    }

    pub const fn store(&self) -> &PartitionedMemorySnapshot {
        &self.store
    }

    pub fn targets(&self) -> &[DramMemoryTargetSnapshot] {
        &self.targets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryTargetSnapshot {
    target: MemoryTargetId,
    controller: DramControllerSnapshot,
    profile: Option<ExternalMemoryProfile>,
}

impl DramMemoryTargetSnapshot {
    pub const fn new(target: MemoryTargetId, controller: DramControllerSnapshot) -> Self {
        Self {
            target,
            controller,
            profile: None,
        }
    }

    pub const fn with_profile(
        target: MemoryTargetId,
        controller: DramControllerSnapshot,
        profile: ExternalMemoryProfile,
    ) -> Self {
        Self {
            target,
            controller,
            profile: Some(profile),
        }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn controller(&self) -> &DramControllerSnapshot {
        &self.controller
    }

    pub const fn profile(&self) -> Option<&ExternalMemoryProfile> {
        self.profile.as_ref()
    }
}

impl DramMemoryController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_target(&mut self, config: DramControllerConfig) -> Result<(), DramMemoryError> {
        if config.layout().bytes() != config.geometry().line_size() {
            return Err(DramMemoryError::TargetLineSizeMismatch {
                target: config.target(),
                layout: config.layout().bytes(),
                geometry: config.geometry().line_size(),
            });
        }

        self.store
            .add_partition(config.target(), config.layout())
            .map_err(DramMemoryError::Memory)?;
        self.dram
            .insert(config.target(), DramController::with_config(config));
        Ok(())
    }

    pub fn add_profile(&mut self, profile: ExternalMemoryProfile) -> Result<(), DramMemoryError> {
        self.add_target(profile.controller_config())?;
        self.profiles.insert(profile.target(), profile);
        Ok(())
    }

    pub fn map_region(
        &mut self,
        target: MemoryTargetId,
        start: Address,
        size: AccessSize,
    ) -> Result<(), DramMemoryError> {
        self.store
            .map_region(target, start, size)
            .map_err(DramMemoryError::Memory)
    }

    pub fn insert_line(
        &mut self,
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    ) -> Result<(), DramMemoryError> {
        self.store
            .insert_line(target, line, data)
            .map_err(DramMemoryError::Memory)
    }

    pub fn line_data(
        &self,
        target: MemoryTargetId,
        line: Address,
    ) -> Result<Vec<u8>, DramMemoryError> {
        self.store
            .line_data(target, line)
            .map_err(DramMemoryError::Memory)
    }

    pub fn line_count(&self, target: MemoryTargetId) -> Result<usize, DramMemoryError> {
        self.store
            .line_count(target)
            .map_err(DramMemoryError::Memory)
    }

    pub fn target_count(&self) -> usize {
        self.dram.len()
    }

    pub fn dram_controller(&self, target: MemoryTargetId) -> Option<&DramController> {
        self.dram.get(&target)
    }

    pub fn memory_profile(&self, target: MemoryTargetId) -> Option<&ExternalMemoryProfile> {
        self.profiles.get(&target)
    }

    pub fn mark_activity(&self) -> DramMemoryActivityMarker {
        DramMemoryActivityMarker::new(
            self.dram
                .iter()
                .map(|(target, controller)| (*target, controller.mark_activity()))
                .collect(),
        )
    }

    pub fn mark_wait_for(&self) -> DramMemoryWaitForMarker {
        DramMemoryWaitForMarker::new(
            self.dram
                .iter()
                .map(|(target, controller)| (*target, controller.mark_wait_for()))
                .collect(),
        )
    }

    pub fn target_activity(&self, target: MemoryTargetId) -> Option<DramTargetActivity> {
        self.dram.get(&target).map(|controller| {
            let activity = DramTargetActivity::new(target, controller.activity_profile());
            match self.profiles.get(&target).copied() {
                Some(profile) => activity.with_memory_profile(profile),
                None => activity,
            }
        })
    }

    pub fn target_activity_since(
        &self,
        marker: &DramMemoryActivityMarker,
        target: MemoryTargetId,
    ) -> Option<DramTargetActivity> {
        self.dram.get(&target).map(|controller| {
            let profile = marker.marker_for(target).map_or_else(
                || controller.activity_profile(),
                |marker| controller.activity_profile_since(marker),
            );
            let activity = DramTargetActivity::new(target, profile);
            match self.profiles.get(&target).copied() {
                Some(profile) => activity.with_memory_profile(profile),
                None => activity,
            }
        })
    }

    pub fn target_activities(&self) -> Vec<DramTargetActivity> {
        self.dram
            .keys()
            .filter_map(|target| self.target_activity(*target))
            .collect()
    }

    pub fn target_activities_since(
        &self,
        marker: &DramMemoryActivityMarker,
    ) -> Vec<DramTargetActivity> {
        self.dram
            .keys()
            .filter_map(|target| self.target_activity_since(marker, *target))
            .filter(|activity| !activity.profile().is_empty())
            .collect()
    }

    pub fn activity_profile(&self) -> DramMemoryActivityProfile {
        DramMemoryActivityProfile::from_target_activities(self.target_activities().iter())
    }

    pub fn activity_profile_since(
        &self,
        marker: &DramMemoryActivityMarker,
    ) -> DramMemoryActivityProfile {
        DramMemoryActivityProfile::from_target_activities(
            self.target_activities_since(marker).iter(),
        )
    }

    pub fn target_wait_for_graph_since(
        &self,
        marker: &DramMemoryWaitForMarker,
        target: MemoryTargetId,
    ) -> Option<WaitForGraph> {
        self.dram.get(&target).map(|controller| {
            let marker = marker
                .marker_for(target)
                .unwrap_or_else(|| DramWaitForMarker::new(0));
            controller.wait_for_graph_since_with_target(marker, Some(target))
        })
    }

    pub fn wait_for_graph_since(&self, marker: &DramMemoryWaitForMarker) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        for target in self.dram.keys() {
            let Some(target_graph) = self.target_wait_for_graph_since(marker, *target) else {
                continue;
            };
            merge_wait_for_graph(&mut graph, target_graph);
        }
        graph
    }

    pub fn snapshot(&self) -> DramMemorySnapshot {
        DramMemorySnapshot::new(
            self.store.snapshot(),
            self.dram
                .iter()
                .map(|(target, controller)| {
                    if let Some(profile) = self.profiles.get(target).copied() {
                        DramMemoryTargetSnapshot::with_profile(
                            *target,
                            controller.snapshot(),
                            profile,
                        )
                    } else {
                        DramMemoryTargetSnapshot::new(*target, controller.snapshot())
                    }
                })
                .collect(),
        )
    }

    pub fn restore(&mut self, snapshot: &DramMemorySnapshot) -> Result<(), DramMemoryError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn from_snapshot(snapshot: &DramMemorySnapshot) -> Result<Self, DramMemoryError> {
        let store = PartitionedMemoryStore::from_snapshot(snapshot.store())
            .map_err(DramMemoryError::Memory)?;
        let mut dram = BTreeMap::new();
        let mut profiles = BTreeMap::new();
        for target in snapshot.targets() {
            if !store.contains_partition(target.target()) {
                return Err(DramMemoryError::Memory(MemoryError::UnknownMemoryTarget {
                    target: target.target(),
                }));
            }
            if dram
                .insert(
                    target.target(),
                    DramController::from_snapshot(target.controller()),
                )
                .is_some()
            {
                return Err(DramMemoryError::Memory(
                    MemoryError::DuplicateMemoryTarget {
                        target: target.target(),
                    },
                ));
            }
            if let Some(profile) = target.profile().copied() {
                profiles.insert(target.target(), profile);
            }
        }
        for partition in store.snapshot().partitions() {
            if !dram.contains_key(&partition.target()) {
                return Err(DramMemoryError::MissingDramTarget {
                    target: partition.target(),
                });
            }
        }

        Ok(Self {
            store,
            dram,
            profiles,
        })
    }
}
