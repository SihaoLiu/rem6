use std::error::Error;
use std::fmt;

use rem6_memory::{MemoryOperation, MemoryRequest, MemoryRequestId};

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
