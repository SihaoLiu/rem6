use rem6_memory::Address;

use crate::elf::{BootElfArchitecture, BootElfClass, BootElfEndian, BootElfOperatingSystem};
use crate::metadata_tables::{
    BootElfLoadSegments, BootElfProgramHeaderTable, BootElfSectionAddressRange,
    BootElfSectionAlignment, BootElfSectionArrays, BootElfSectionFlags, BootElfSectionGroups,
    BootElfSectionHashes, BootElfSectionHeaderTable, BootElfSectionNameTable,
    BootElfSectionRelocations, BootElfSectionStorage, BootElfSymbolSummary,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BootElfDynamicSegment {
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: Address,
    pub(crate) entry_size: u16,
    pub(crate) entry_count: u64,
    pub(crate) needed_count: u64,
    pub(crate) needed_libraries: Vec<String>,
    pub(crate) soname: Option<String>,
    pub(crate) rpath: Vec<String>,
    pub(crate) runpath: Vec<String>,
    pub(crate) auxiliary_libraries: Vec<String>,
    pub(crate) filter_libraries: Vec<String>,
    pub(crate) audit_libraries: Vec<String>,
    pub(crate) dependency_audit_libraries: Vec<String>,
    pub(crate) string_table_virtual_address: Option<Address>,
    pub(crate) string_table_size: Option<u64>,
    pub(crate) symbol_table_virtual_address: Option<Address>,
    pub(crate) symbol_table_entry_size: Option<u64>,
    pub(crate) init_virtual_address: Option<Address>,
    pub(crate) fini_virtual_address: Option<Address>,
    pub(crate) init_array_virtual_address: Option<Address>,
    pub(crate) init_array_size: Option<u64>,
    pub(crate) fini_array_virtual_address: Option<Address>,
    pub(crate) fini_array_size: Option<u64>,
    pub(crate) preinit_array_virtual_address: Option<Address>,
    pub(crate) preinit_array_size: Option<u64>,
    pub(crate) flags: Option<u64>,
    pub(crate) flags_1: Option<u64>,
    pub(crate) plt_got_virtual_address: Option<Address>,
    pub(crate) debug_virtual_address: Option<Address>,
    pub(crate) symbolic_binding: bool,
    pub(crate) text_relocations: bool,
    pub(crate) bind_now: bool,
    pub(crate) rela_relative_count: Option<u64>,
    pub(crate) rel_relative_count: Option<u64>,
    pub(crate) sysv_hash_virtual_address: Option<Address>,
    pub(crate) gnu_hash_virtual_address: Option<Address>,
    pub(crate) version_symbol_table_virtual_address: Option<Address>,
    pub(crate) version_definition_table_virtual_address: Option<Address>,
    pub(crate) version_definition_count: Option<u64>,
    pub(crate) version_needed_table_virtual_address: Option<Address>,
    pub(crate) version_needed_count: Option<u64>,
    pub(crate) rela_relocations: BootElfDynamicRelocationTable,
    pub(crate) rel_relocations: BootElfDynamicRelocationTable,
    pub(crate) relr_relocations: BootElfDynamicRelocationTable,
    pub(crate) plt_relocations: BootElfDynamicRelocationTable,
    pub(crate) plt_relocation_kind: Option<BootElfDynamicPltRelocationKind>,
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
    auxiliary_libraries: Vec<String>,
    filter_libraries: Vec<String>,
    audit_libraries: Vec<String>,
    dependency_audit_libraries: Vec<String>,
    string_table_virtual_address: Option<Address>,
    string_table_size: Option<u64>,
    symbol_table_virtual_address: Option<Address>,
    symbol_table_entry_size: Option<u64>,
    init_virtual_address: Option<Address>,
    fini_virtual_address: Option<Address>,
    init_array_virtual_address: Option<Address>,
    init_array_size: Option<u64>,
    fini_array_virtual_address: Option<Address>,
    fini_array_size: Option<u64>,
    preinit_array_virtual_address: Option<Address>,
    preinit_array_size: Option<u64>,
    flags: Option<u64>,
    flags_1: Option<u64>,
    plt_got_virtual_address: Option<Address>,
    debug_virtual_address: Option<Address>,
    symbolic_binding: bool,
    text_relocations: bool,
    bind_now: bool,
    rela_relative_count: Option<u64>,
    rel_relative_count: Option<u64>,
    sysv_hash_virtual_address: Option<Address>,
    gnu_hash_virtual_address: Option<Address>,
    version_symbol_table_virtual_address: Option<Address>,
    version_definition_table_virtual_address: Option<Address>,
    version_definition_count: Option<u64>,
    version_needed_table_virtual_address: Option<Address>,
    version_needed_count: Option<u64>,
    rela_relocations: BootElfDynamicRelocationTable,
    rel_relocations: BootElfDynamicRelocationTable,
    relr_relocations: BootElfDynamicRelocationTable,
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
            auxiliary_libraries: Vec::new(),
            filter_libraries: Vec::new(),
            audit_libraries: Vec::new(),
            dependency_audit_libraries: Vec::new(),
            string_table_virtual_address: None,
            string_table_size: None,
            symbol_table_virtual_address: None,
            symbol_table_entry_size: None,
            init_virtual_address: None,
            fini_virtual_address: None,
            init_array_virtual_address: None,
            init_array_size: None,
            fini_array_virtual_address: None,
            fini_array_size: None,
            preinit_array_virtual_address: None,
            preinit_array_size: None,
            flags: None,
            flags_1: None,
            plt_got_virtual_address: None,
            debug_virtual_address: None,
            symbolic_binding: false,
            text_relocations: false,
            bind_now: false,
            rela_relative_count: None,
            rel_relative_count: None,
            sysv_hash_virtual_address: None,
            gnu_hash_virtual_address: None,
            version_symbol_table_virtual_address: None,
            version_definition_table_virtual_address: None,
            version_definition_count: None,
            version_needed_table_virtual_address: None,
            version_needed_count: None,
            rela_relocations: BootElfDynamicRelocationTable::default(),
            rel_relocations: BootElfDynamicRelocationTable::default(),
            relr_relocations: BootElfDynamicRelocationTable::default(),
            plt_relocations: BootElfDynamicRelocationTable::default(),
            plt_relocation_kind: None,
        }
    }

    pub(crate) fn with_segment(mut self, segment: BootElfDynamicSegment) -> Self {
        self.segment_count += 1;
        if self.file_offset.is_none() {
            self.file_offset = Some(segment.file_offset);
            self.virtual_address = Some(segment.virtual_address);
            self.entry_size = segment.entry_size;
            self.entry_count = segment.entry_count;
            self.needed_count = segment.needed_count;
            self.needed_libraries = segment.needed_libraries;
            self.soname = segment.soname;
            self.rpath = segment.rpath;
            self.runpath = segment.runpath;
            self.auxiliary_libraries = segment.auxiliary_libraries;
            self.filter_libraries = segment.filter_libraries;
            self.audit_libraries = segment.audit_libraries;
            self.dependency_audit_libraries = segment.dependency_audit_libraries;
            self.string_table_virtual_address = segment.string_table_virtual_address;
            self.string_table_size = segment.string_table_size;
            self.symbol_table_virtual_address = segment.symbol_table_virtual_address;
            self.symbol_table_entry_size = segment.symbol_table_entry_size;
            self.init_virtual_address = segment.init_virtual_address;
            self.fini_virtual_address = segment.fini_virtual_address;
            self.init_array_virtual_address = segment.init_array_virtual_address;
            self.init_array_size = segment.init_array_size;
            self.fini_array_virtual_address = segment.fini_array_virtual_address;
            self.fini_array_size = segment.fini_array_size;
            self.preinit_array_virtual_address = segment.preinit_array_virtual_address;
            self.preinit_array_size = segment.preinit_array_size;
            self.flags = segment.flags;
            self.flags_1 = segment.flags_1;
            self.plt_got_virtual_address = segment.plt_got_virtual_address;
            self.debug_virtual_address = segment.debug_virtual_address;
            self.symbolic_binding = segment.symbolic_binding;
            self.text_relocations = segment.text_relocations;
            self.bind_now = segment.bind_now;
            self.rela_relative_count = segment.rela_relative_count;
            self.rel_relative_count = segment.rel_relative_count;
            self.sysv_hash_virtual_address = segment.sysv_hash_virtual_address;
            self.gnu_hash_virtual_address = segment.gnu_hash_virtual_address;
            self.version_symbol_table_virtual_address =
                segment.version_symbol_table_virtual_address;
            self.version_definition_table_virtual_address =
                segment.version_definition_table_virtual_address;
            self.version_definition_count = segment.version_definition_count;
            self.version_needed_table_virtual_address =
                segment.version_needed_table_virtual_address;
            self.version_needed_count = segment.version_needed_count;
            self.rela_relocations = segment.rela_relocations;
            self.rel_relocations = segment.rel_relocations;
            self.relr_relocations = segment.relr_relocations;
            self.plt_relocations = segment.plt_relocations;
            self.plt_relocation_kind = segment.plt_relocation_kind;
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
    pub fn auxiliary_libraries(&self) -> &[String] {
        &self.auxiliary_libraries
    }

    pub fn filter_libraries(&self) -> &[String] {
        &self.filter_libraries
    }

    pub fn audit_libraries(&self) -> &[String] {
        &self.audit_libraries
    }

    pub fn dependency_audit_libraries(&self) -> &[String] {
        &self.dependency_audit_libraries
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

    pub fn auxiliary_name_bytes(&self) -> u64 {
        self.auxiliary_libraries
            .iter()
            .map(|library| library.len() as u64)
            .sum()
    }

    pub fn filter_name_bytes(&self) -> u64 {
        self.filter_libraries
            .iter()
            .map(|library| library.len() as u64)
            .sum()
    }

    pub fn audit_name_bytes(&self) -> u64 {
        self.audit_libraries
            .iter()
            .map(|library| library.len() as u64)
            .sum()
    }

    pub fn dependency_audit_name_bytes(&self) -> u64 {
        self.dependency_audit_libraries
            .iter()
            .map(|library| library.len() as u64)
            .sum()
    }

    pub const fn string_table_virtual_address(&self) -> Option<Address> {
        self.string_table_virtual_address
    }

    pub const fn string_table_size(&self) -> Option<u64> {
        self.string_table_size
    }

    pub const fn symbol_table_virtual_address(&self) -> Option<Address> {
        self.symbol_table_virtual_address
    }

    pub const fn symbol_table_entry_size(&self) -> Option<u64> {
        self.symbol_table_entry_size
    }

    pub const fn init_virtual_address(&self) -> Option<Address> {
        self.init_virtual_address
    }

    pub const fn fini_virtual_address(&self) -> Option<Address> {
        self.fini_virtual_address
    }

    pub const fn init_array_virtual_address(&self) -> Option<Address> {
        self.init_array_virtual_address
    }

    pub const fn init_array_size(&self) -> Option<u64> {
        self.init_array_size
    }

    pub const fn fini_array_virtual_address(&self) -> Option<Address> {
        self.fini_array_virtual_address
    }

    pub const fn fini_array_size(&self) -> Option<u64> {
        self.fini_array_size
    }

    pub const fn preinit_array_virtual_address(&self) -> Option<Address> {
        self.preinit_array_virtual_address
    }

    pub const fn preinit_array_size(&self) -> Option<u64> {
        self.preinit_array_size
    }

    pub const fn flags(&self) -> Option<u64> {
        self.flags
    }

    pub const fn flags_1(&self) -> Option<u64> {
        self.flags_1
    }

    pub const fn plt_got_virtual_address(&self) -> Option<Address> {
        self.plt_got_virtual_address
    }

    pub const fn debug_virtual_address(&self) -> Option<Address> {
        self.debug_virtual_address
    }

    pub const fn has_symbolic_binding(&self) -> bool {
        self.symbolic_binding
    }

    pub const fn has_text_relocations(&self) -> bool {
        self.text_relocations
    }

    pub const fn bind_now(&self) -> bool {
        self.bind_now
    }

    pub const fn rela_relative_count(&self) -> Option<u64> {
        self.rela_relative_count
    }

    pub const fn rel_relative_count(&self) -> Option<u64> {
        self.rel_relative_count
    }

    pub const fn sysv_hash_virtual_address(&self) -> Option<Address> {
        self.sysv_hash_virtual_address
    }

    pub const fn gnu_hash_virtual_address(&self) -> Option<Address> {
        self.gnu_hash_virtual_address
    }

    pub const fn version_symbol_table_virtual_address(&self) -> Option<Address> {
        self.version_symbol_table_virtual_address
    }

    pub const fn version_definition_table_virtual_address(&self) -> Option<Address> {
        self.version_definition_table_virtual_address
    }

    pub const fn version_definition_count(&self) -> Option<u64> {
        self.version_definition_count
    }

    pub const fn version_needed_table_virtual_address(&self) -> Option<Address> {
        self.version_needed_table_virtual_address
    }

    pub const fn version_needed_count(&self) -> Option<u64> {
        self.version_needed_count
    }

    pub const fn rela_relocations(&self) -> BootElfDynamicRelocationTable {
        self.rela_relocations
    }

    pub const fn rel_relocations(&self) -> BootElfDynamicRelocationTable {
        self.rel_relocations
    }

    pub const fn relr_relocations(&self) -> BootElfDynamicRelocationTable {
        self.relr_relocations
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

    pub const fn relr_virtual_address(&self) -> Option<Address> {
        self.relr_relocations.virtual_address()
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

    pub const fn relr_entry_count(&self) -> u64 {
        self.relr_relocations.entry_count()
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
    note_segment_count: u64,
    note_file_size: u64,
    note_section_count: u64,
    note_section_file_size: u64,
    gnu_stack_executable: Option<bool>,
    gnu_relro_virtual_address: Option<Address>,
    gnu_relro_memory_size: Option<u64>,
    gnu_eh_frame_virtual_address: Option<Address>,
    gnu_eh_frame_memory_size: Option<u64>,
    gnu_property_virtual_address: Option<Address>,
    gnu_property_memory_size: Option<u64>,
    symbol_summary: BootElfSymbolSummary,
    dynamic_table: BootElfDynamicTable,
    load_segments: BootElfLoadSegments,
    program_header_table: BootElfProgramHeaderTable,
    section_header_table: BootElfSectionHeaderTable,
    section_name_table: BootElfSectionNameTable,
    section_flags: BootElfSectionFlags,
    section_storage: BootElfSectionStorage,
    section_relocations: BootElfSectionRelocations,
    section_arrays: BootElfSectionArrays,
    section_hashes: BootElfSectionHashes,
    section_groups: BootElfSectionGroups,
    section_address_range: BootElfSectionAddressRange,
    section_alignment: BootElfSectionAlignment,
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
            note_segment_count: 0,
            note_file_size: 0,
            note_section_count: 0,
            note_section_file_size: 0,
            gnu_stack_executable: None,
            gnu_relro_virtual_address: None,
            gnu_relro_memory_size: None,
            gnu_eh_frame_virtual_address: None,
            gnu_eh_frame_memory_size: None,
            gnu_property_virtual_address: None,
            gnu_property_memory_size: None,
            symbol_summary: BootElfSymbolSummary::new(0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
            dynamic_table: BootElfDynamicTable::new(),
            load_segments: BootElfLoadSegments::new(0, 0, 0, 0, 0, 0, 0),
            program_header_table: BootElfProgramHeaderTable::new(0, 0, 0),
            section_header_table: BootElfSectionHeaderTable::new(0, 0, 0, 0),
            section_name_table: BootElfSectionNameTable::new(0, 0),
            section_flags: BootElfSectionFlags::new(0, 0, 0, 0),
            section_storage: BootElfSectionStorage::new(0, 0, 0, 0, 0, 0, 0),
            section_relocations: BootElfSectionRelocations::new(0, 0, 0, 0, 0, 0, 0, 0),
            section_arrays: BootElfSectionArrays::new(0, 0, 0, 0, 0, 0, 0, 0, 0),
            section_hashes: BootElfSectionHashes::new(0, 0, 0, 0),
            section_groups: BootElfSectionGroups::new(0, 0, 0),
            section_address_range: BootElfSectionAddressRange::new(None, None),
            section_alignment: BootElfSectionAlignment::new(0, 0, 0),
        }
    }

    pub(crate) fn with_program_header_table(mut self, table: BootElfProgramHeaderTable) -> Self {
        self.program_header_table = table;
        self
    }

    pub(crate) const fn with_load_segments(mut self, load_segments: BootElfLoadSegments) -> Self {
        self.load_segments = load_segments;
        self
    }

    pub(crate) const fn with_section_metadata(
        mut self,
        section_header_table: BootElfSectionHeaderTable,
        section_name_table: BootElfSectionNameTable,
        section_flags: BootElfSectionFlags,
        section_storage: BootElfSectionStorage,
        section_relocations: BootElfSectionRelocations,
        section_arrays: BootElfSectionArrays,
        section_hashes: BootElfSectionHashes,
        section_groups: BootElfSectionGroups,
        section_address_range: BootElfSectionAddressRange,
        section_alignment: BootElfSectionAlignment,
    ) -> Self {
        self.section_header_table = section_header_table;
        self.section_name_table = section_name_table;
        self.section_flags = section_flags;
        self.section_storage = section_storage;
        self.section_relocations = section_relocations;
        self.section_arrays = section_arrays;
        self.section_hashes = section_hashes;
        self.section_groups = section_groups;
        self.section_address_range = section_address_range;
        self.section_alignment = section_alignment;
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

    pub(crate) const fn with_note_segments(mut self, segment_count: u64, file_size: u64) -> Self {
        self.note_segment_count = segment_count;
        self.note_file_size = file_size;
        self
    }

    pub(crate) const fn with_note_sections(mut self, section_count: u64, file_size: u64) -> Self {
        self.note_section_count = section_count;
        self.note_section_file_size = file_size;
        self
    }

    pub(crate) const fn with_gnu_stack_executable(mut self, executable: Option<bool>) -> Self {
        self.gnu_stack_executable = executable;
        self
    }

    pub(crate) const fn with_gnu_relro(
        mut self,
        virtual_address: Option<Address>,
        memory_size: Option<u64>,
    ) -> Self {
        self.gnu_relro_virtual_address = virtual_address;
        self.gnu_relro_memory_size = memory_size;
        self
    }

    pub(crate) const fn with_gnu_eh_frame(
        mut self,
        virtual_address: Option<Address>,
        memory_size: Option<u64>,
    ) -> Self {
        self.gnu_eh_frame_virtual_address = virtual_address;
        self.gnu_eh_frame_memory_size = memory_size;
        self
    }

    pub(crate) const fn with_gnu_property(
        mut self,
        virtual_address: Option<Address>,
        memory_size: Option<u64>,
    ) -> Self {
        self.gnu_property_virtual_address = virtual_address;
        self.gnu_property_memory_size = memory_size;
        self
    }

    pub(crate) const fn with_symbol_summary(
        mut self,
        symbol_summary: BootElfSymbolSummary,
    ) -> Self {
        self.symbol_summary = symbol_summary;
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

    pub const fn note_segment_count(&self) -> u64 {
        self.note_segment_count
    }

    pub const fn note_file_size(&self) -> u64 {
        self.note_file_size
    }

    pub const fn note_section_count(&self) -> u64 {
        self.note_section_count
    }

    pub const fn note_section_file_size(&self) -> u64 {
        self.note_section_file_size
    }

    pub const fn gnu_stack_executable(&self) -> Option<bool> {
        self.gnu_stack_executable
    }

    pub const fn gnu_relro_virtual_address(&self) -> Option<Address> {
        self.gnu_relro_virtual_address
    }

    pub const fn gnu_relro_memory_size(&self) -> Option<u64> {
        self.gnu_relro_memory_size
    }

    pub const fn gnu_eh_frame_virtual_address(&self) -> Option<Address> {
        self.gnu_eh_frame_virtual_address
    }

    pub const fn gnu_eh_frame_memory_size(&self) -> Option<u64> {
        self.gnu_eh_frame_memory_size
    }

    pub const fn gnu_property_virtual_address(&self) -> Option<Address> {
        self.gnu_property_virtual_address
    }

    pub const fn gnu_property_memory_size(&self) -> Option<u64> {
        self.gnu_property_memory_size
    }

    pub const fn symbol_count(&self) -> u64 {
        self.symbol_summary.total_count()
    }
    pub const fn function_symbol_count(&self) -> u64 {
        self.symbol_summary.function_count()
    }
    pub const fn object_symbol_count(&self) -> u64 {
        self.symbol_summary.object_count()
    }
    pub const fn local_symbol_count(&self) -> u64 {
        self.symbol_summary.local_count()
    }
    pub const fn global_symbol_count(&self) -> u64 {
        self.symbol_summary.global_count()
    }
    pub const fn weak_symbol_count(&self) -> u64 {
        self.symbol_summary.weak_count()
    }
    pub const fn symbol_summary(&self) -> BootElfSymbolSummary {
        self.symbol_summary
    }
    pub const fn dynamic_table(&self) -> &BootElfDynamicTable {
        &self.dynamic_table
    }

    pub const fn load_segments(&self) -> BootElfLoadSegments {
        self.load_segments
    }

    pub const fn program_header_table(&self) -> BootElfProgramHeaderTable {
        self.program_header_table
    }

    pub const fn section_header_table(&self) -> BootElfSectionHeaderTable {
        self.section_header_table
    }

    pub const fn section_name_table(&self) -> BootElfSectionNameTable {
        self.section_name_table
    }

    pub const fn section_flags(&self) -> BootElfSectionFlags {
        self.section_flags
    }

    pub const fn section_storage(&self) -> BootElfSectionStorage {
        self.section_storage
    }

    pub const fn section_relocations(&self) -> BootElfSectionRelocations {
        self.section_relocations
    }

    pub const fn section_arrays(&self) -> BootElfSectionArrays {
        self.section_arrays
    }

    pub const fn section_hashes(&self) -> BootElfSectionHashes {
        self.section_hashes
    }

    pub const fn section_groups(&self) -> BootElfSectionGroups {
        self.section_groups
    }

    pub const fn section_address_range(&self) -> BootElfSectionAddressRange {
        self.section_address_range
    }

    pub const fn section_alignment(&self) -> BootElfSectionAlignment {
        self.section_alignment
    }
}
