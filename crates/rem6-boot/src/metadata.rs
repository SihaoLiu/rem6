use rem6_memory::Address;

use crate::elf::{BootElfArchitecture, BootElfClass, BootElfEndian, BootElfOperatingSystem};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootElfInterpreter {
    path: String,
    file_offset: u64,
    file_size: u64,
}

impl BootElfInterpreter {
    pub fn new(path: impl Into<String>, file_offset: u64, file_size: u64) -> Self {
        Self {
            path: path.into(),
            file_offset,
            file_size,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub const fn file_size(&self) -> u64 {
        self.file_size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootElfProgramHeaderTable {
    file_offset: u64,
    entry_size: u16,
    entry_count: u64,
    memory_address: Option<Address>,
}

impl BootElfProgramHeaderTable {
    pub const fn new(file_offset: u64, entry_size: u16, entry_count: u64) -> Self {
        Self {
            file_offset,
            entry_size,
            entry_count,
            memory_address: None,
        }
    }

    pub const fn with_memory_address(mut self, memory_address: Option<Address>) -> Self {
        self.memory_address = memory_address;
        self
    }

    pub const fn file_offset(self) -> u64 {
        self.file_offset
    }

    pub const fn entry_size(self) -> u16 {
        self.entry_size
    }

    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    pub const fn memory_address(self) -> Option<Address> {
        self.memory_address
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootElfMetadata {
    class: BootElfClass,
    endian: BootElfEndian,
    machine: u16,
    os_abi: u8,
    flags: u32,
    architecture: BootElfArchitecture,
    operating_system: BootElfOperatingSystem,
    program_header_table: BootElfProgramHeaderTable,
}

impl BootElfMetadata {
    pub(crate) const fn from_header(
        class: BootElfClass,
        endian: BootElfEndian,
        machine: u16,
        os_abi: u8,
        flags: u32,
        architecture: BootElfArchitecture,
        operating_system: BootElfOperatingSystem,
    ) -> Self {
        Self {
            class,
            endian,
            machine,
            os_abi,
            flags,
            architecture,
            operating_system,
            program_header_table: BootElfProgramHeaderTable::new(0, 0, 0),
        }
    }

    pub(crate) const fn with_program_header_table(
        mut self,
        table: BootElfProgramHeaderTable,
    ) -> Self {
        self.program_header_table = table;
        self
    }

    pub const fn class(&self) -> BootElfClass {
        self.class
    }

    pub const fn endian(&self) -> BootElfEndian {
        self.endian
    }

    pub const fn machine(&self) -> u16 {
        self.machine
    }

    pub const fn os_abi(&self) -> u8 {
        self.os_abi
    }

    pub const fn flags(&self) -> u32 {
        self.flags
    }

    pub const fn architecture(&self) -> BootElfArchitecture {
        self.architecture
    }

    pub const fn operating_system(&self) -> BootElfOperatingSystem {
        self.operating_system
    }

    pub const fn program_header_table(&self) -> BootElfProgramHeaderTable {
        self.program_header_table
    }
}
