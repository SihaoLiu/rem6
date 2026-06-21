use rem6_system::{RiscvGuestWriteRecord, RiscvSbiIpiRecord, RiscvUnknownSyscallRecord};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvGuestWriteSummary {
    pub(crate) fd: u32,
    pub(crate) address: u64,
    pub(crate) tick: u64,
    pub(crate) bytes: Vec<u8>,
}

impl Rem6RiscvGuestWriteSummary {
    pub(crate) fn from_record(record: &RiscvGuestWriteRecord) -> Self {
        Self {
            fd: record.fd().get(),
            address: record.address(),
            tick: record.tick(),
            bytes: record.bytes().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiConsoleSummary {
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiTimerSummary {
    cpu: u32,
    deadline: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiIpiSummary {
    source_cpu: u32,
    hart_mask: u64,
    hart_mask_base: u64,
    targets: Vec<u64>,
}

impl Rem6RiscvSbiConsoleSummary {
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn byte_count(&self) -> u64 {
        self.bytes.len() as u64
    }
}

impl Rem6RiscvSbiTimerSummary {
    pub(crate) const fn new(cpu: u32, deadline: u64) -> Self {
        Self { cpu, deadline }
    }

    pub(crate) const fn cpu(&self) -> u32 {
        self.cpu
    }

    pub(crate) const fn deadline(&self) -> u64 {
        self.deadline
    }
}

impl Rem6RiscvSbiIpiSummary {
    pub(crate) fn from_record(record: &RiscvSbiIpiRecord) -> Self {
        Self {
            source_cpu: record.source_cpu().get(),
            hart_mask: record.hart_mask(),
            hart_mask_base: record.hart_mask_base(),
            targets: record.targets().to_vec(),
        }
    }

    pub(crate) const fn source_cpu(&self) -> u32 {
        self.source_cpu
    }

    pub(crate) const fn hart_mask(&self) -> u64 {
        self.hart_mask
    }

    pub(crate) const fn hart_mask_base(&self) -> u64 {
        self.hart_mask_base
    }

    pub(crate) fn targets(&self) -> &[u64] {
        &self.targets
    }

    pub(crate) fn target_count(&self) -> u64 {
        self.targets.len() as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvUnknownSyscallSummary {
    pub(crate) pc: u64,
    pub(crate) number: u64,
    pub(crate) arguments: [u64; 6],
    pub(crate) tick: u64,
}

impl Rem6RiscvUnknownSyscallSummary {
    pub(crate) fn from_record(record: &RiscvUnknownSyscallRecord) -> Self {
        Self {
            pc: record.pc(),
            number: record.number(),
            arguments: record.arguments(),
            tick: record.tick(),
        }
    }
}
