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
pub struct BootElfDynamicRelocationTable {
    virtual_address: Option<Address>,
    byte_size: u64,
    entry_size: u64,
}

impl BootElfDynamicRelocationTable {
    pub const fn new(virtual_address: Option<Address>, byte_size: u64, entry_size: u64) -> Self {
        Self {
            virtual_address,
            byte_size,
            entry_size,
        }
    }

    pub const fn virtual_address(self) -> Option<Address> {
        self.virtual_address
    }

    pub const fn byte_size(self) -> u64 {
        self.byte_size
    }

    pub const fn entry_size(self) -> u64 {
        self.entry_size
    }

    pub const fn entry_count(self) -> u64 {
        if self.entry_size == 0 {
            0
        } else {
            self.byte_size / self.entry_size
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootElfDynamicPltRelocationKind {
    Rel,
    Rela,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BootElfDynamicTable {
    segment_count: u64,
    file_offset: Option<u64>,
    virtual_address: Option<Address>,
    entry_size: u16,
    entry_count: u64,
    needed_count: u64,
    needed_libraries: Vec<String>,
    soname: Option<String>,
    rpath: Vec<String>,
    runpath: Vec<String>,
    sysv_hash_virtual_address: Option<Address>,
    gnu_hash_virtual_address: Option<Address>,
    rela_relocations: BootElfDynamicRelocationTable,
    rel_relocations: BootElfDynamicRelocationTable,
    plt_relocations: BootElfDynamicRelocationTable,
    plt_relocation_kind: Option<BootElfDynamicPltRelocationKind>,
}

impl BootElfDynamicTable {
    pub fn new() -> Self {
        Self {
            segment_count: 0,
            file_offset: None,
            virtual_address: None,
            entry_size: 0,
            entry_count: 0,
            needed_count: 0,
            needed_libraries: Vec::new(),
            soname: None,
            rpath: Vec::new(),
            runpath: Vec::new(),
            sysv_hash_virtual_address: None,
            gnu_hash_virtual_address: None,
            rela_relocations: BootElfDynamicRelocationTable::default(),
            rel_relocations: BootElfDynamicRelocationTable::default(),
            plt_relocations: BootElfDynamicRelocationTable::default(),
            plt_relocation_kind: None,
        }
    }

    pub fn with_segment(
        mut self,
        file_offset: u64,
        virtual_address: Address,
        entry_size: u16,
        entry_count: u64,
        needed_count: u64,
        needed_libraries: Vec<String>,
        soname: Option<String>,
        rpath: Vec<String>,
        runpath: Vec<String>,
        sysv_hash_virtual_address: Option<Address>,
        gnu_hash_virtual_address: Option<Address>,
        rela_relocations: BootElfDynamicRelocationTable,
        rel_relocations: BootElfDynamicRelocationTable,
        plt_relocations: BootElfDynamicRelocationTable,
        plt_relocation_kind: Option<BootElfDynamicPltRelocationKind>,
    ) -> Self {
        self.segment_count += 1;
        if self.file_offset.is_none() {
            self.file_offset = Some(file_offset);
            self.virtual_address = Some(virtual_address);
            self.entry_size = entry_size;
            self.entry_count = entry_count;
            self.needed_count = needed_count;
            self.needed_libraries = needed_libraries;
            self.soname = soname;
            self.rpath = rpath;
            self.runpath = runpath;
            self.sysv_hash_virtual_address = sysv_hash_virtual_address;
            self.gnu_hash_virtual_address = gnu_hash_virtual_address;
            self.rela_relocations = rela_relocations;
            self.rel_relocations = rel_relocations;
            self.plt_relocations = plt_relocations;
            self.plt_relocation_kind = plt_relocation_kind;
        }
        self
    }

    pub const fn segment_count(&self) -> u64 {
        self.segment_count
    }

    pub const fn file_offset(&self) -> Option<u64> {
        self.file_offset
    }

    pub const fn virtual_address(&self) -> Option<Address> {
        self.virtual_address
    }

    pub const fn entry_size(&self) -> u16 {
        self.entry_size
    }

    pub const fn entry_count(&self) -> u64 {
        self.entry_count
    }

    pub const fn needed_count(&self) -> u64 {
        self.needed_count
    }

    pub fn needed_libraries(&self) -> &[String] {
        &self.needed_libraries
    }

    pub fn needed_name_bytes(&self) -> u64 {
        self.needed_libraries
            .iter()
            .map(|library| library.len() as u64)
            .sum()
    }

    pub fn soname(&self) -> Option<&str> {
        self.soname.as_deref()
    }

    pub fn rpath(&self) -> &[String] {
        &self.rpath
    }

    pub fn runpath(&self) -> &[String] {
        &self.runpath
    }

    pub fn soname_name_bytes(&self) -> u64 {
        self.soname.as_ref().map_or(0, |name| name.len() as u64)
    }

    pub fn rpath_name_bytes(&self) -> u64 {
        self.rpath.iter().map(|path| path.len() as u64).sum()
    }

    pub fn runpath_name_bytes(&self) -> u64 {
        self.runpath.iter().map(|path| path.len() as u64).sum()
    }

    pub const fn sysv_hash_virtual_address(&self) -> Option<Address> {
        self.sysv_hash_virtual_address
    }

    pub const fn gnu_hash_virtual_address(&self) -> Option<Address> {
        self.gnu_hash_virtual_address
    }

    pub const fn rela_relocations(&self) -> BootElfDynamicRelocationTable {
        self.rela_relocations
    }

    pub const fn rel_relocations(&self) -> BootElfDynamicRelocationTable {
        self.rel_relocations
    }

    pub const fn plt_relocations(&self) -> BootElfDynamicRelocationTable {
        self.plt_relocations
    }

    pub const fn plt_relocation_kind(&self) -> Option<BootElfDynamicPltRelocationKind> {
        self.plt_relocation_kind
    }

    pub const fn rela_virtual_address(&self) -> Option<Address> {
        self.rela_relocations.virtual_address()
    }

    pub const fn rel_virtual_address(&self) -> Option<Address> {
        self.rel_relocations.virtual_address()
    }

    pub const fn plt_relocation_virtual_address(&self) -> Option<Address> {
        self.plt_relocations.virtual_address()
    }

    pub const fn rela_entry_count(&self) -> u64 {
        self.rela_relocations.entry_count()
    }

    pub const fn rel_entry_count(&self) -> u64 {
        self.rel_relocations.entry_count()
    }

    pub const fn plt_rela_entry_count(&self) -> u64 {
        match self.plt_relocation_kind {
            Some(BootElfDynamicPltRelocationKind::Rela) => self.plt_relocations.entry_count(),
            _ => 0,
        }
    }

    pub const fn plt_rel_entry_count(&self) -> u64 {
        match self.plt_relocation_kind {
            Some(BootElfDynamicPltRelocationKind::Rel) => self.plt_relocations.entry_count(),
            _ => 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootElfMetadata {
    class: BootElfClass,
    endian: BootElfEndian,
    machine: u16,
    os_abi: u8,
    flags: u32,
    architecture: BootElfArchitecture,
    operating_system: BootElfOperatingSystem,
    has_tls: bool,
    gnu_stack_executable: Option<bool>,
    symbol_count: u64,
    function_symbol_count: u64,
    object_symbol_count: u64,
    dynamic_table: BootElfDynamicTable,
    program_header_table: BootElfProgramHeaderTable,
}

impl BootElfMetadata {
    pub(crate) fn from_header(
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
            gnu_stack_executable: None,
            symbol_count: 0,
            function_symbol_count: 0,
            object_symbol_count: 0,
            dynamic_table: BootElfDynamicTable::new(),
            program_header_table: BootElfProgramHeaderTable::new(0, 0, 0),
        }
    }

    pub(crate) fn with_program_header_table(mut self, table: BootElfProgramHeaderTable) -> Self {
        self.program_header_table = table;
        self
    }

    pub(crate) fn with_dynamic_table(mut self, dynamic_table: BootElfDynamicTable) -> Self {
        self.dynamic_table = dynamic_table;
        self
    }

    pub(crate) const fn with_tls(mut self, has_tls: bool) -> Self {
        self.has_tls = has_tls;
        self
    }

    pub(crate) const fn with_gnu_stack_executable(mut self, executable: Option<bool>) -> Self {
        self.gnu_stack_executable = executable;
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

    pub const fn gnu_stack_executable(&self) -> Option<bool> {
        self.gnu_stack_executable
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

    pub const fn dynamic_table(&self) -> &BootElfDynamicTable {
        &self.dynamic_table
    }

    pub const fn program_header_table(&self) -> BootElfProgramHeaderTable {
        self.program_header_table
    }
}
