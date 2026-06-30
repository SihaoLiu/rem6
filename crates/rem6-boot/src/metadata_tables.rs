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
pub struct BootElfLoadSegments {
    count: u64,
    file_bytes: u64,
    memory_bytes: u64,
    writable_count: u64,
    executable_count: u64,
    max_alignment: u64,
    misaligned_alignment_count: u64,
}

impl BootElfLoadSegments {
    pub const fn new(
        count: u64,
        file_bytes: u64,
        memory_bytes: u64,
        writable_count: u64,
        executable_count: u64,
        max_alignment: u64,
        misaligned_alignment_count: u64,
    ) -> Self {
        Self {
            count,
            file_bytes,
            memory_bytes,
            writable_count,
            executable_count,
            max_alignment,
            misaligned_alignment_count,
        }
    }

    pub const fn count(self) -> u64 {
        self.count
    }

    pub const fn file_bytes(self) -> u64 {
        self.file_bytes
    }

    pub const fn memory_bytes(self) -> u64 {
        self.memory_bytes
    }

    pub const fn writable_count(self) -> u64 {
        self.writable_count
    }

    pub const fn executable_count(self) -> u64 {
        self.executable_count
    }

    pub const fn max_alignment(self) -> u64 {
        self.max_alignment
    }

    pub const fn misaligned_alignment_count(self) -> u64 {
        self.misaligned_alignment_count
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BootElfSectionStorage {
    file_backed_bytes: u64,
    allocated_bytes: u64,
    writable_bytes: u64,
    executable_bytes: u64,
    nobits_bytes: u64,
}

impl BootElfSectionStorage {
    pub const fn new(
        file_backed_bytes: u64,
        allocated_bytes: u64,
        writable_bytes: u64,
        executable_bytes: u64,
        nobits_bytes: u64,
    ) -> Self {
        Self {
            file_backed_bytes,
            allocated_bytes,
            writable_bytes,
            executable_bytes,
            nobits_bytes,
        }
    }

    pub const fn file_backed_bytes(self) -> u64 {
        self.file_backed_bytes
    }

    pub const fn allocated_bytes(self) -> u64 {
        self.allocated_bytes
    }

    pub const fn writable_bytes(self) -> u64 {
        self.writable_bytes
    }

    pub const fn executable_bytes(self) -> u64 {
        self.executable_bytes
    }

    pub const fn nobits_bytes(self) -> u64 {
        self.nobits_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BootElfSectionAddressRange {
    start_address: Option<Address>,
    end_address: Option<Address>,
}

impl BootElfSectionAddressRange {
    pub const fn new(start_address: Option<Address>, end_address: Option<Address>) -> Self {
        Self {
            start_address,
            end_address,
        }
    }

    pub const fn start_address(self) -> Option<Address> {
        self.start_address
    }

    pub const fn end_address(self) -> Option<Address> {
        self.end_address
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BootElfSectionAlignment {
    max_alignment: u64,
    allocated_max_alignment: u64,
    misaligned_allocated_count: u64,
}

impl BootElfSectionAlignment {
    pub const fn new(
        max_alignment: u64,
        allocated_max_alignment: u64,
        misaligned_allocated_count: u64,
    ) -> Self {
        Self {
            max_alignment,
            allocated_max_alignment,
            misaligned_allocated_count,
        }
    }

    pub const fn max_alignment(self) -> u64 {
        self.max_alignment
    }

    pub const fn allocated_max_alignment(self) -> u64 {
        self.allocated_max_alignment
    }

    pub const fn misaligned_allocated_count(self) -> u64 {
        self.misaligned_allocated_count
    }
}
