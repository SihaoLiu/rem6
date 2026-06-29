use rem6_memory::Address;

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
pub struct BootElfSectionHeaderTable {
    file_offset: u64,
    entry_size: u16,
    entry_count: u64,
    string_table_index: u64,
}

impl BootElfSectionHeaderTable {
    pub const fn new(
        file_offset: u64,
        entry_size: u16,
        entry_count: u64,
        string_table_index: u64,
    ) -> Self {
        Self {
            file_offset,
            entry_size,
            entry_count,
            string_table_index,
        }
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

    pub const fn string_table_index(self) -> u64 {
        self.string_table_index
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BootElfSectionNameTable {
    file_offset: u64,
    byte_size: u64,
}

impl BootElfSectionNameTable {
    pub const fn new(file_offset: u64, byte_size: u64) -> Self {
        Self {
            file_offset,
            byte_size,
        }
    }

    pub const fn file_offset(self) -> u64 {
        self.file_offset
    }

    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BootElfSectionFlags {
    allocated_count: u64,
    writable_count: u64,
    executable_count: u64,
    nobits_count: u64,
}

impl BootElfSectionFlags {
    pub const fn new(
        allocated_count: u64,
        writable_count: u64,
        executable_count: u64,
        nobits_count: u64,
    ) -> Self {
        Self {
            allocated_count,
            writable_count,
            executable_count,
            nobits_count,
        }
    }

    pub const fn allocated_count(self) -> u64 {
        self.allocated_count
    }

    pub const fn writable_count(self) -> u64 {
        self.writable_count
    }

    pub const fn executable_count(self) -> u64 {
        self.executable_count
    }

    pub const fn nobits_count(self) -> u64 {
        self.nobits_count
    }
}
