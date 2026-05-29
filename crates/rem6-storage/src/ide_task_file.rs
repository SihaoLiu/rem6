#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdeTaskFile {
    pub(crate) error: u8,
    pub(crate) sector_count: u8,
    pub(crate) sector_number: u8,
    pub(crate) cylinder_low: u8,
    pub(crate) cylinder_high: u8,
    pub(crate) drive: u8,
    pub(crate) command: u8,
}

impl IdeTaskFile {
    pub(crate) const fn reset() -> Self {
        Self {
            error: 0x01,
            sector_count: 0,
            sector_number: 0,
            cylinder_low: 0,
            cylinder_high: 0,
            drive: 0,
            command: 0,
        }
    }

    pub(crate) const fn from_registers(
        error: u8,
        sector_count: u8,
        sector_number: u8,
        cylinder_low: u8,
        cylinder_high: u8,
        drive: u8,
        command: u8,
    ) -> Self {
        Self {
            error,
            sector_count,
            sector_number,
            cylinder_low,
            cylinder_high,
            drive,
            command,
        }
    }

    pub(crate) fn lba_base(self) -> u64 {
        (u64::from(self.drive & 0x0f) << 24)
            | (u64::from(self.cylinder_high) << 16)
            | (u64::from(self.cylinder_low) << 8)
            | u64::from(self.sector_number)
    }

    pub const fn error(self) -> u8 {
        self.error
    }

    pub const fn sector_count(self) -> u8 {
        self.sector_count
    }

    pub const fn sector_number(self) -> u8 {
        self.sector_number
    }

    pub const fn cylinder_low(self) -> u8 {
        self.cylinder_low
    }

    pub const fn cylinder_high(self) -> u8 {
        self.cylinder_high
    }

    pub const fn drive(self) -> u8 {
        self.drive
    }

    pub const fn command(self) -> u8 {
        self.command
    }
}
