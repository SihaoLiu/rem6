use rem6_system::{
    RiscvGuestWriteRecord, RiscvSbiHsmRecord, RiscvSbiHsmWakeRecord, RiscvSbiIpiRecord,
    RiscvSbiResetRecord, RiscvSbiRfenceCompletionRecord, RiscvSbiRfenceRecord,
    RiscvUnknownSyscallRecord,
};

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
    dbcn_byte_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiTimerSummary {
    cpu: u32,
    deadline: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiHsmSummary {
    source_cpu: u32,
    function: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiHsmWakeSummary {
    source_cpu: u32,
    target_hart: u64,
    interrupt_bits: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiIpiSummary {
    source_cpu: u32,
    hart_mask: u64,
    hart_mask_base: u64,
    targets: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiRfenceSummary {
    source_cpu: u32,
    function: u64,
    hart_mask: u64,
    hart_mask_base: u64,
    start_addr: u64,
    size: u64,
    address_space: Option<u64>,
    targets: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiRfenceCompletionSummary {
    source_cpu: u32,
    target_hart: u64,
    function: u64,
    start_addr: u64,
    size: u64,
    address_space: Option<u64>,
    completed_tick: u64,
    flushed_entries: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6RiscvSbiResetSummary {
    cpu: u32,
    reset_type: u32,
    reset_reason: u32,
    code: i32,
}

impl Rem6RiscvSbiConsoleSummary {
    pub(crate) fn from_bytes_and_dbcn_byte_count(bytes: Vec<u8>, dbcn_byte_count: u64) -> Self {
        Self {
            bytes,
            dbcn_byte_count,
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn byte_count(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub(crate) fn dbcn_byte_count(&self) -> u64 {
        self.dbcn_byte_count
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

impl Rem6RiscvSbiHsmSummary {
    pub(crate) fn from_record(record: &RiscvSbiHsmRecord) -> Self {
        Self {
            source_cpu: record.source_cpu().get(),
            function: record.function(),
            arg0: record.arg0(),
            arg1: record.arg1(),
            arg2: record.arg2(),
        }
    }

    pub(crate) const fn source_cpu(&self) -> u32 {
        self.source_cpu
    }

    pub(crate) const fn function(&self) -> u64 {
        self.function
    }

    pub(crate) const fn arg0(&self) -> u64 {
        self.arg0
    }

    pub(crate) const fn arg1(&self) -> u64 {
        self.arg1
    }

    pub(crate) const fn arg2(&self) -> u64 {
        self.arg2
    }

    pub(crate) const fn is_hart_start(&self) -> bool {
        self.function == 0
    }

    pub(crate) const fn is_hart_stop(&self) -> bool {
        self.function == 1
    }

    pub(crate) const fn is_hart_suspend(&self) -> bool {
        self.function == 3
    }
}

impl Rem6RiscvSbiHsmWakeSummary {
    pub(crate) fn from_record(record: &RiscvSbiHsmWakeRecord) -> Self {
        Self {
            source_cpu: record.source_cpu().get(),
            target_hart: record.target_hart(),
            interrupt_bits: record.interrupt_bits(),
        }
    }

    pub(crate) const fn source_cpu(&self) -> u32 {
        self.source_cpu
    }

    pub(crate) const fn target_hart(&self) -> u64 {
        self.target_hart
    }

    pub(crate) const fn interrupt_bits(&self) -> u64 {
        self.interrupt_bits
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

impl Rem6RiscvSbiRfenceSummary {
    pub(crate) fn from_record(record: &RiscvSbiRfenceRecord) -> Self {
        Self {
            source_cpu: record.source_cpu().get(),
            function: record.function(),
            hart_mask: record.hart_mask(),
            hart_mask_base: record.hart_mask_base(),
            start_addr: record.start_addr(),
            size: record.size(),
            address_space: record.address_space(),
            targets: record.targets().to_vec(),
        }
    }

    pub(crate) const fn source_cpu(&self) -> u32 {
        self.source_cpu
    }

    pub(crate) const fn function(&self) -> u64 {
        self.function
    }

    pub(crate) const fn hart_mask(&self) -> u64 {
        self.hart_mask
    }

    pub(crate) const fn hart_mask_base(&self) -> u64 {
        self.hart_mask_base
    }

    pub(crate) const fn start_addr(&self) -> u64 {
        self.start_addr
    }

    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    pub(crate) const fn address_space(&self) -> Option<u64> {
        self.address_space
    }

    pub(crate) fn targets(&self) -> &[u64] {
        &self.targets
    }

    pub(crate) fn target_count(&self) -> u64 {
        self.targets.len() as u64
    }
}

impl Rem6RiscvSbiRfenceCompletionSummary {
    pub(crate) fn from_record(record: &RiscvSbiRfenceCompletionRecord) -> Self {
        Self {
            source_cpu: record.source_cpu().get(),
            target_hart: record.target_hart(),
            function: record.function(),
            start_addr: record.start_addr(),
            size: record.size(),
            address_space: record.address_space(),
            completed_tick: record.completed_tick(),
            flushed_entries: record.flushed_entries(),
        }
    }

    pub(crate) const fn source_cpu(&self) -> u32 {
        self.source_cpu
    }

    pub(crate) const fn target_hart(&self) -> u64 {
        self.target_hart
    }

    pub(crate) const fn function(&self) -> u64 {
        self.function
    }

    pub(crate) const fn start_addr(&self) -> u64 {
        self.start_addr
    }

    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    pub(crate) const fn address_space(&self) -> Option<u64> {
        self.address_space
    }

    pub(crate) const fn completed_tick(&self) -> u64 {
        self.completed_tick
    }

    pub(crate) const fn flushed_entries(&self) -> Option<u64> {
        self.flushed_entries
    }
}

impl Rem6RiscvSbiResetSummary {
    pub(crate) fn from_record(record: &RiscvSbiResetRecord) -> Self {
        Self {
            cpu: record.cpu().get(),
            reset_type: record.reset_type(),
            reset_reason: record.reset_reason(),
            code: record.code(),
        }
    }

    pub(crate) const fn cpu(&self) -> u32 {
        self.cpu
    }

    pub(crate) const fn reset_type(&self) -> u32 {
        self.reset_type
    }

    pub(crate) const fn reset_reason(&self) -> u32 {
        self.reset_reason
    }

    pub(crate) const fn code(&self) -> i32 {
        self.code
    }

    pub(crate) const fn is_shutdown(&self) -> bool {
        self.reset_type == 0
    }

    pub(crate) const fn is_cold_reboot(&self) -> bool {
        self.reset_type == 1
    }

    pub(crate) const fn is_warm_reboot(&self) -> bool {
        self.reset_type == 2
    }

    pub(crate) const fn is_system_failure(&self) -> bool {
        self.code != 0
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
