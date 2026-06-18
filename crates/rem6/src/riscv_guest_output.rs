use rem6_system::{RiscvGuestWriteRecord, RiscvUnknownSyscallRecord};

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
