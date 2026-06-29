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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BootElfDynamicTable {
    segment_count: u64,
    file_offset: Option<u64>,
    virtual_address: Option<Address>,
    entry_size: u16,
    entry_count: u64,
    needed_count: u64,
}

impl BootElfDynamicTable {
    pub const fn new() -> Self {
        Self {
            segment_count: 0,
            file_offset: None,
            virtual_address: None,
            entry_size: 0,
            entry_count: 0,
            needed_count: 0,
        }
    }

    pub const fn with_segment(
        mut self,
        file_offset: u64,
        virtual_address: Address,
        entry_size: u16,
        entry_count: u64,
        needed_count: u64,
    ) -> Self {
        self.segment_count += 1;
        if self.file_offset.is_none() {
            self.file_offset = Some(file_offset);
            self.virtual_address = Some(virtual_address);
            self.entry_size = entry_size;
            self.entry_count = entry_count;
            self.needed_count = needed_count;
        }
        self
    }

    pub const fn segment_count(self) -> u64 {
        self.segment_count
    }

    pub const fn file_offset(self) -> Option<u64> {
        self.file_offset
    }

    pub const fn virtual_address(self) -> Option<Address> {
        self.virtual_address
    }

    pub const fn entry_size(self) -> u16 {
        self.entry_size
    }

    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    pub const fn needed_count(self) -> u64 {
        self.needed_count
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
    has_tls: bool,
    symbol_count: u64,
    function_symbol_count: u64,
    object_symbol_count: u64,
    dynamic_table: BootElfDynamicTable,
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
            has_tls: false,
            symbol_count: 0,
            function_symbol_count: 0,
            object_symbol_count: 0,
            dynamic_table: BootElfDynamicTable::new(),
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

    pub(crate) const fn with_dynamic_table(mut self, dynamic_table: BootElfDynamicTable) -> Self {
        self.dynamic_table = dynamic_table;
        self
    }

    pub(crate) const fn with_tls(mut self, has_tls: bool) -> Self {
        self.has_tls = has_tls;
        self
    }

    pub(crate) const fn with_symbol_summary(
        mut self,
        symbol_count: u64,
        function_symbol_count: u64,
        object_symbol_count: u64,
    ) -> Self {
        self.symbol_count = symbol_count;
        self.function_symbol_count = function_symbol_count;
        self.object_symbol_count = object_symbol_count;
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

    pub const fn has_tls(&self) -> bool {
        self.has_tls
    }

    pub const fn symbol_count(&self) -> u64 {
        self.symbol_count
    }

    pub const fn function_symbol_count(&self) -> u64 {
        self.function_symbol_count
    }

    pub const fn object_symbol_count(&self) -> u64 {
        self.object_symbol_count
    }

    pub const fn dynamic_table(&self) -> BootElfDynamicTable {
        self.dynamic_table
    }

    pub const fn program_header_table(&self) -> BootElfProgramHeaderTable {
        self.program_header_table
    }
}
