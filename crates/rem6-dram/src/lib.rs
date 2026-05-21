use std::error::Error;
use std::fmt;

use std::collections::BTreeMap;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramMemoryTechnology {
    Ddr,
    Hbm,
    Lpddr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramProfileField {
    Channels,
    RanksPerChannel,
    Stacks,
    PseudoChannelsPerStack,
    DiesPerChannel,
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
        }
    }
}

impl Error for DramError {}

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
pub enum ExternalMemoryTopology {
    Ddr {
        channels: u32,
        ranks_per_channel: u32,
    },
    Hbm {
        stacks: u32,
        pseudo_channels_per_stack: u32,
    },
    Lpddr {
        channels: u32,
        dies_per_channel: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalMemoryProfile {
    target: MemoryTargetId,
    line_layout: CacheLineLayout,
    geometry: DramGeometry,
    timing: DramTiming,
    technology: DramMemoryTechnology,
    topology: ExternalMemoryTopology,
}

impl ExternalMemoryProfile {
    pub fn ddr(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        channels: u32,
        ranks_per_channel: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(
            DramMemoryTechnology::Ddr,
            DramProfileField::Channels,
            channels,
        )?;
        validate_profile_count(
            DramMemoryTechnology::Ddr,
            DramProfileField::RanksPerChannel,
            ranks_per_channel,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Ddr,
            ExternalMemoryTopology::Ddr {
                channels,
                ranks_per_channel,
            },
        ))
    }

    pub fn hbm(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        stacks: u32,
        pseudo_channels_per_stack: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(DramMemoryTechnology::Hbm, DramProfileField::Stacks, stacks)?;
        validate_profile_count(
            DramMemoryTechnology::Hbm,
            DramProfileField::PseudoChannelsPerStack,
            pseudo_channels_per_stack,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Hbm,
            ExternalMemoryTopology::Hbm {
                stacks,
                pseudo_channels_per_stack,
            },
        ))
    }

    pub fn lpddr(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        channels: u32,
        dies_per_channel: u32,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, DramError> {
        validate_profile_count(
            DramMemoryTechnology::Lpddr,
            DramProfileField::Channels,
            channels,
        )?;
        validate_profile_count(
            DramMemoryTechnology::Lpddr,
            DramProfileField::DiesPerChannel,
            dies_per_channel,
        )?;
        Ok(Self::new(
            target,
            line_layout,
            geometry,
            timing,
            DramMemoryTechnology::Lpddr,
            ExternalMemoryTopology::Lpddr {
                channels,
                dies_per_channel,
            },
        ))
    }

    const fn new(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
        technology: DramMemoryTechnology,
        topology: ExternalMemoryTopology,
    ) -> Self {
        Self {
            target,
            line_layout,
            geometry,
            timing,
            technology,
            topology,
        }
    }

    pub const fn target(self) -> MemoryTargetId {
        self.target
    }

    pub const fn line_layout(self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn geometry(self) -> DramGeometry {
        self.geometry
    }

    pub const fn timing(self) -> DramTiming {
        self.timing
    }

    pub const fn technology(self) -> DramMemoryTechnology {
        self.technology
    }

    pub const fn topology(self) -> ExternalMemoryTopology {
        self.topology
    }

    pub const fn controller_config(self) -> DramControllerConfig {
        DramControllerConfig::new(self.target, self.line_layout, self.geometry, self.timing)
    }
}

fn validate_profile_count(
    technology: DramMemoryTechnology,
    field: DramProfileField,
    value: u32,
) -> Result<(), DramError> {
    if value == 0 {
        return Err(DramError::ZeroProfileTopology { technology, field });
    }

    Ok(())
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

    fn decode_address(self, address: u64) -> DecodedDramAddress {
        let line = address / self.line_size;
        let bank = (line % u64::from(self.bank_count)) as u32;
        let row = line / (u64::from(self.bank_count) * self.lines_per_row);
        DecodedDramAddress { bank, row }
    }

    fn decode_request(self, request: &MemoryRequest) -> Result<DecodedDramAddress, DramError> {
        if request.line_layout().bytes() != self.line_size {
            return Err(DramError::LineSizeMismatch {
                request: request.id(),
                expected: self.line_size,
                actual: request.line_layout().bytes(),
            });
        }

        let start = self.decode_address(request.range().start().get());
        let end = self.decode_address(request.range().end().get() - 1);
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
    bank: u32,
    row: u64,
    kind: DramCommandKind,
}

impl DramCommand {
    fn new(cycle: u64, bank: u32, row: u64, kind: DramCommandKind) -> Self {
        Self {
            cycle,
            bank,
            row,
            kind,
        }
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
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
    bank: u32,
    row: u64,
    row_hit: bool,
    arrival_cycle: u64,
    command_cycle: u64,
    ready_cycle: u64,
    commands: Vec<DramCommand>,
}

impl DramAccess {
    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn kind(&self) -> DramAccessKind {
        self.kind
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
pub struct DramController {
    geometry: DramGeometry,
    timing: DramTiming,
    banks: Vec<DramBankState>,
    bus_available_cycle: u64,
    last_access_kind: Option<DramAccessKind>,
}

impl DramController {
    pub fn new(geometry: DramGeometry, timing: DramTiming) -> Self {
        Self {
            geometry,
            timing,
            banks: vec![DramBankState::new(); geometry.bank_count() as usize],
            bus_available_cycle: 0,
            last_access_kind: None,
        }
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

    pub fn snapshot(&self) -> DramControllerSnapshot {
        DramControllerSnapshot::new(
            self.geometry,
            self.timing,
            self.banks.clone(),
            self.bus_available_cycle,
            self.last_access_kind,
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
            bus_available_cycle: snapshot.bus_available_cycle(),
            last_access_kind: snapshot.last_access_kind(),
        }
    }

    pub fn schedule(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramAccess, DramError> {
        let kind = DramAccessKind::from_operation(request)?;
        let decoded = self.geometry.decode_request(request)?;
        let bus_ready_cycle = self.bus_ready_cycle(kind);
        let bank = &mut self.banks[decoded.bank as usize];
        let mut commands = Vec::new();
        let mut next_cycle = arrival_cycle.max(bank.available_cycle);
        let row_hit = bank.open_row == Some(decoded.row);

        if !row_hit {
            if let Some(open_row) = bank.open_row {
                commands.push(DramCommand::new(
                    next_cycle,
                    decoded.bank,
                    open_row,
                    DramCommandKind::Precharge,
                ));
                next_cycle += self.timing.precharge_latency();
            }
            commands.push(DramCommand::new(
                next_cycle,
                decoded.bank,
                decoded.row,
                DramCommandKind::Activate,
            ));
            next_cycle += self.timing.activate_latency();
            bank.open_row = Some(decoded.row);
        }

        let command_cycle = next_cycle.max(bus_ready_cycle);
        commands.push(DramCommand::new(
            command_cycle,
            decoded.bank,
            decoded.row,
            kind.command_kind(),
        ));
        let ready_cycle = command_cycle + self.timing.data_latency(kind);

        bank.available_cycle = ready_cycle;
        self.bus_available_cycle = command_cycle;
        self.last_access_kind = Some(kind);

        Ok(DramAccess {
            request: request.id(),
            kind,
            bank: decoded.bank,
            row: decoded.row,
            row_hit,
            arrival_cycle,
            command_cycle,
            ready_cycle,
            commands,
        })
    }

    fn bus_ready_cycle(&self, kind: DramAccessKind) -> u64 {
        if self
            .last_access_kind
            .is_some_and(|previous| previous != kind)
        {
            self.bus_available_cycle + self.timing.bus_turnaround()
        } else {
            self.bus_available_cycle
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramControllerSnapshot {
    geometry: DramGeometry,
    timing: DramTiming,
    banks: Vec<DramBankState>,
    bus_available_cycle: u64,
    last_access_kind: Option<DramAccessKind>,
}

impl DramControllerSnapshot {
    pub const fn new(
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
            bus_available_cycle,
            last_access_kind,
        }
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

    pub const fn bus_available_cycle(&self) -> u64 {
        self.bus_available_cycle
    }

    pub const fn last_access_kind(&self) -> Option<DramAccessKind> {
        self.last_access_kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramControllerConfig {
    target: MemoryTargetId,
    layout: CacheLineLayout,
    geometry: DramGeometry,
    timing: DramTiming,
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
        }
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
        self.dram.insert(
            config.target(),
            DramController::new(config.geometry(), config.timing()),
        );
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

    pub fn accept(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let target = self
            .store
            .decode_request(request)
            .map_err(DramMemoryError::Memory)?;
        self.preflight_storage(target, request)
            .map_err(DramMemoryError::Memory)?;
        let dram_access = self
            .dram
            .get_mut(&target)
            .expect("DRAM target is inserted with memory target")
            .schedule(arrival_cycle, request)
            .map_err(|source| DramMemoryError::Dram { target, source })?;
        let response = self
            .store
            .respond(request)
            .map_err(DramMemoryError::Memory)?
            .response()
            .cloned();

        Ok(DramMemoryOutcome::new(target, dram_access, response))
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

    fn preflight_storage(
        &self,
        target: MemoryTargetId,
        request: &MemoryRequest,
    ) -> Result<(), MemoryError> {
        if request.line_span() != 1 {
            return Err(MemoryError::CrossLineAccess {
                request: request.id(),
                start: request.range().start(),
                size: request.size(),
                line_size: request.line_layout().bytes(),
            });
        }

        if matches!(
            request.operation(),
            MemoryOperation::WritebackClean | MemoryOperation::WritebackDirty
        ) {
            return Ok(());
        }

        self.store
            .line_data(target, request.line_address())
            .map(|_| ())
    }
}
