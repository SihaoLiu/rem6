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
    pub(crate) fn new(
        cycle: u64,
        parallel_port: u32,
        bank: u32,
        row: u64,
        kind: DramCommandKind,
    ) -> Self {
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
