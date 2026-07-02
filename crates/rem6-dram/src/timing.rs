use rem6_memory::MemoryRequest;

use crate::{DramAccessKind, DramError, DramLowPowerTiming};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramTimingField {
    ActivateLatency,
    ReadLatency,
    WriteLatency,
    PrechargeLatency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramRefreshTimingField {
    Interval,
    Recovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramRefreshPolicy {
    PerBank,
    BankGroup,
    AllBank,
}

impl DramRefreshPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PerBank => "per-bank",
            Self::BankGroup => "bank-group",
            Self::AllBank => "all-bank",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramRefreshTiming {
    interval: u64,
    recovery: u64,
}

impl DramRefreshTiming {
    pub const fn new(interval: u64, recovery: u64) -> Result<Self, DramError> {
        if interval == 0 {
            return Err(DramError::ZeroRefreshTiming {
                field: DramRefreshTimingField::Interval,
            });
        }
        if recovery == 0 {
            return Err(DramError::ZeroRefreshTiming {
                field: DramRefreshTimingField::Recovery,
            });
        }
        Ok(Self { interval, recovery })
    }

    pub const fn interval(self) -> u64 {
        self.interval
    }

    pub const fn recovery(self) -> u64 {
        self.recovery
    }

    pub const fn t_refi_cycles(self) -> u64 {
        self.interval
    }

    pub const fn t_rfc_cycles(self) -> u64 {
        self.recovery
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramTiming {
    activate_latency: u64,
    read_latency: u64,
    write_latency: u64,
    precharge_latency: u64,
    bus_turnaround: u64,
    burst_spacing: u64,
    same_bank_group_burst_spacing: Option<u64>,
    command_window: Option<DramCommandWindow>,
    low_power_timing: Option<DramLowPowerTiming>,
    refresh_timing: Option<DramRefreshTiming>,
    refresh_policy: DramRefreshPolicy,
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
            burst_spacing: 0,
            same_bank_group_burst_spacing: None,
            command_window: None,
            low_power_timing: None,
            refresh_timing: None,
            refresh_policy: DramRefreshPolicy::PerBank,
        })
    }

    pub const fn with_burst_spacing(mut self, burst_spacing: u64) -> Result<Self, DramError> {
        self.burst_spacing = burst_spacing;
        Ok(self)
    }

    pub fn with_command_window(
        mut self,
        window_cycles: u64,
        max_commands: u32,
    ) -> Result<Self, DramError> {
        let command_window = DramCommandWindow::new(window_cycles, max_commands)?;
        if let Some(refresh_timing) = self.refresh_timing {
            validate_refresh_command_window(refresh_timing, command_window)?;
        }
        self.command_window = Some(command_window);
        Ok(self)
    }

    pub const fn with_low_power_timing(mut self, low_power_timing: DramLowPowerTiming) -> Self {
        self.low_power_timing = Some(low_power_timing);
        self
    }

    pub const fn with_refresh_timing(
        mut self,
        refresh_timing: DramRefreshTiming,
    ) -> Result<Self, DramError> {
        if refresh_timing
            .recovery()
            .saturating_add(self.activate_latency)
            >= refresh_timing.interval()
        {
            return Err(DramError::RefreshRecoveryLeavesNoActivateSlot {
                interval: refresh_timing.interval(),
                recovery: refresh_timing.recovery(),
                activate_latency: self.activate_latency,
            });
        }
        if let Some(command_window) = self.command_window {
            match validate_refresh_command_window(refresh_timing, command_window) {
                Ok(()) => {}
                Err(error) => return Err(error),
            }
        }
        self.refresh_timing = Some(refresh_timing);
        Ok(self)
    }

    pub const fn with_refresh_policy(
        mut self,
        refresh_policy: DramRefreshPolicy,
    ) -> Result<Self, DramError> {
        self.refresh_policy = refresh_policy;
        Ok(self)
    }

    pub const fn with_same_bank_group_burst_spacing(
        mut self,
        burst_spacing: u64,
    ) -> Result<Self, DramError> {
        if burst_spacing == 0 {
            return Err(DramError::ZeroSameBankGroupBurstSpacing);
        }
        self.same_bank_group_burst_spacing = Some(burst_spacing);
        Ok(self)
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

    pub const fn burst_spacing(self) -> u64 {
        self.burst_spacing
    }

    pub const fn same_bank_group_burst_spacing(self) -> Option<u64> {
        self.same_bank_group_burst_spacing
    }

    pub const fn command_window(self) -> Option<DramCommandWindow> {
        self.command_window
    }

    pub const fn low_power_timing(self) -> Option<DramLowPowerTiming> {
        self.low_power_timing
    }

    pub const fn refresh_timing(self) -> Option<DramRefreshTiming> {
        self.refresh_timing
    }

    pub const fn refresh_policy(self) -> DramRefreshPolicy {
        self.refresh_policy
    }

    pub(crate) fn data_latency(self, kind: DramAccessKind) -> u64 {
        match kind {
            DramAccessKind::Read => self.read_latency,
            DramAccessKind::Write => self.write_latency,
        }
    }
}

const fn validate_refresh_command_window(
    refresh_timing: DramRefreshTiming,
    command_window: DramCommandWindow,
) -> Result<(), DramError> {
    if command_window.max_commands() < 2
        && command_window.window_cycles() >= refresh_timing.interval()
    {
        return Err(DramError::RefreshCommandWindowLeavesNoDataSlot {
            interval: refresh_timing.interval(),
            window_cycles: command_window.window_cycles(),
            max_commands: command_window.max_commands(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramCommandWindow {
    window_cycles: u64,
    max_commands: u32,
}

impl DramCommandWindow {
    pub const fn new(window_cycles: u64, max_commands: u32) -> Result<Self, DramError> {
        if window_cycles == 0 {
            return Err(DramError::ZeroCommandWindow);
        }
        if max_commands == 0 {
            return Err(DramError::ZeroCommandWindowMaxCommands);
        }

        Ok(Self {
            window_cycles,
            max_commands,
        })
    }

    pub const fn window_cycles(self) -> u64 {
        self.window_cycles
    }

    pub const fn max_commands(self) -> u32 {
        self.max_commands
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramGeometry {
    bank_count: u32,
    row_size: u64,
    line_size: u64,
    lines_per_row: u64,
    bank_group_count: Option<u32>,
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
            bank_group_count: None,
        })
    }

    pub fn with_bank_groups(mut self, bank_group_count: u32) -> Result<Self, DramError> {
        if bank_group_count == 0 {
            return Err(DramError::ZeroBankGroupCount);
        }
        if bank_group_count > self.bank_count {
            return Err(DramError::BankGroupCountExceedsBankCount {
                bank_count: self.bank_count,
                bank_group_count,
            });
        }
        if !self.bank_count.is_multiple_of(bank_group_count) {
            return Err(DramError::BankCountNotBankGroupMultiple {
                bank_count: self.bank_count,
                bank_group_count,
            });
        }

        self.bank_group_count = Some(bank_group_count);
        Ok(self)
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

    pub const fn bank_group_count(self) -> Option<u32> {
        self.bank_group_count
    }

    pub const fn bank_group_for_bank(self, bank: u32) -> Option<u32> {
        match self.bank_group_count {
            Some(bank_group_count) => Some(bank % bank_group_count),
            None => None,
        }
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
            bank_group: self.bank_group_for_bank(bank),
            row,
        }
    }

    pub(crate) fn decode_request(
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
pub(crate) struct DecodedDramAddress {
    pub(crate) parallel_port: u32,
    pub(crate) bank: u32,
    pub(crate) bank_group: Option<u32>,
    pub(crate) row: u64,
}
