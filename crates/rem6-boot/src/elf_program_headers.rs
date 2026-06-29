use rem6_memory::Address;

use crate::elf::{read_u32_at_u64, read_u64_at_u64, BootElfEndian};
use crate::elf_dynamic::{
    dynamic_table_counts, ElfDynamicPltRelocationKind, ElfDynamicRelocationSummary, ElfLoadMapping,
};
use crate::elf_interpreter::read_interpreter;
use crate::error::BootError;
use crate::metadata::{
    BootElfDynamicPltRelocationKind, BootElfDynamicRelocationTable, BootElfDynamicSegment,
    BootElfDynamicTable, BootElfInterpreter,
};

const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_NOTE: u32 = 4;
const PT_PHDR: u32 = 6;
const PT_TLS: u32 = 7;
const PT_GNU_EH_FRAME: u32 = 0x6474_e550;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const PT_GNU_PROPERTY: u32 = 0x6474_e553;
const PF_X: u32 = 0x1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ElfProgramHeaderAction {
    ConsiderLoad,
    Skip,
}

pub(crate) struct ElfProgramHeaderMetadata {
    pub(crate) interpreter: Option<BootElfInterpreter>,
    pub(crate) dynamic_table: BootElfDynamicTable,
    pub(crate) has_tls: bool,
    pub(crate) note_segment_count: u64,
    pub(crate) note_file_size: u64,
    pub(crate) gnu_stack_executable: Option<bool>,
    pub(crate) gnu_relro_virtual_address: Option<Address>,
    pub(crate) gnu_relro_memory_size: Option<u64>,
    pub(crate) gnu_eh_frame_virtual_address: Option<Address>,
    pub(crate) gnu_eh_frame_memory_size: Option<u64>,
    pub(crate) gnu_property_virtual_address: Option<Address>,
    pub(crate) gnu_property_memory_size: Option<u64>,
    pt_phdr_memory_address: Option<Address>,
    inferred_program_header_memory_address: Option<Address>,
}

impl ElfProgramHeaderMetadata {
    pub(crate) fn new(has_tls: bool) -> Self {
        Self {
            interpreter: None,
            dynamic_table: BootElfDynamicTable::new(),
            has_tls,
            note_segment_count: 0,
            note_file_size: 0,
            gnu_stack_executable: None,
            gnu_relro_virtual_address: None,
            gnu_relro_memory_size: None,
            gnu_eh_frame_virtual_address: None,
            gnu_eh_frame_memory_size: None,
            gnu_property_virtual_address: None,
            gnu_property_memory_size: None,
            pt_phdr_memory_address: None,
            inferred_program_header_memory_address: None,
        }
    }

    pub(crate) fn record_inferred_program_header_address(&mut self, address: Option<Address>) {
        if self.inferred_program_header_memory_address.is_none() {
            self.inferred_program_header_memory_address = address;
        }
    }

    pub(crate) const fn program_header_memory_address(&self) -> Option<Address> {
        match self.pt_phdr_memory_address {
            Some(address) => Some(address),
            None => self.inferred_program_header_memory_address,
        }
    }
}

pub(crate) fn summarize_elf64_program_header(
    bytes: &[u8],
    segment: u16,
    header_offset: u64,
    kind: u32,
    endian: BootElfEndian,
    load_mappings: &[ElfLoadMapping],
    metadata: &mut ElfProgramHeaderMetadata,
) -> Result<ElfProgramHeaderAction, BootError> {
    match kind {
        PT_DYNAMIC => {
            let file_offset = read_u64_at_u64(bytes, header_offset + 8, endian)?;
            let file_size = read_u64_at_u64(bytes, header_offset + 32, endian)?;
            let summary = dynamic_table_counts(
                bytes,
                segment,
                file_offset,
                file_size,
                16,
                endian,
                load_mappings,
            )?;
            metadata.dynamic_table =
                std::mem::take(&mut metadata.dynamic_table).with_segment(BootElfDynamicSegment {
                    file_offset,
                    virtual_address: Address::new(read_u64_at_u64(
                        bytes,
                        header_offset + 16,
                        endian,
                    )?),
                    entry_size: 16,
                    entry_count: summary.entry_count,
                    needed_count: summary.needed_libraries.len() as u64,
                    needed_libraries: summary.needed_libraries,
                    soname: summary.soname,
                    rpath: summary.rpath,
                    runpath: summary.runpath,
                    auxiliary_libraries: summary.auxiliary_libraries,
                    filter_libraries: summary.filter_libraries,
                    audit_libraries: summary.audit_libraries,
                    dependency_audit_libraries: summary.dependency_audit_libraries,
                    string_table_virtual_address: summary
                        .string_table_virtual_address
                        .map(Address::new),
                    string_table_size: summary.string_table_size,
                    symbol_table_virtual_address: summary
                        .symbol_table_virtual_address
                        .map(Address::new),
                    symbol_table_entry_size: summary.symbol_table_entry_size,
                    init_virtual_address: summary.init_virtual_address.map(Address::new),
                    fini_virtual_address: summary.fini_virtual_address.map(Address::new),
                    init_array_virtual_address: summary
                        .init_array_virtual_address
                        .map(Address::new),
                    init_array_size: summary.init_array_size,
                    fini_array_virtual_address: summary
                        .fini_array_virtual_address
                        .map(Address::new),
                    fini_array_size: summary.fini_array_size,
                    preinit_array_virtual_address: summary
                        .preinit_array_virtual_address
                        .map(Address::new),
                    preinit_array_size: summary.preinit_array_size,
                    flags: summary.flags,
                    flags_1: summary.flags_1,
                    plt_got_virtual_address: summary.plt_got_virtual_address.map(Address::new),
                    debug_virtual_address: summary.debug_virtual_address.map(Address::new),
                    symbolic_binding: summary.symbolic_binding,
                    text_relocations: summary.text_relocations,
                    bind_now: summary.bind_now,
                    rela_relative_count: summary.rela_relative_count,
                    rel_relative_count: summary.rel_relative_count,
                    sysv_hash_virtual_address: summary.sysv_hash_virtual_address.map(Address::new),
                    gnu_hash_virtual_address: summary.gnu_hash_virtual_address.map(Address::new),
                    version_symbol_table_virtual_address: summary
                        .version_symbol_table_virtual_address
                        .map(Address::new),
                    version_definition_table_virtual_address: summary
                        .version_definition_table_virtual_address
                        .map(Address::new),
                    version_definition_count: summary.version_definition_count,
                    version_needed_table_virtual_address: summary
                        .version_needed_table_virtual_address
                        .map(Address::new),
                    version_needed_count: summary.version_needed_count,
                    rela_relocations: relocation_table(summary.rela_relocations),
                    rel_relocations: relocation_table(summary.rel_relocations),
                    plt_relocations: relocation_table(summary.plt_relocations),
                    plt_relocation_kind: plt_relocation_kind(summary.plt_relocation_kind),
                });
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_INTERP => {
            if metadata.interpreter.is_none() {
                metadata.interpreter = Some(read_interpreter(
                    bytes,
                    segment,
                    read_u64_at_u64(bytes, header_offset + 8, endian)?,
                    read_u64_at_u64(bytes, header_offset + 32, endian)?,
                )?);
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_NOTE => {
            metadata.note_segment_count = metadata.note_segment_count.saturating_add(1);
            metadata.note_file_size = metadata.note_file_size.saturating_add(read_u64_at_u64(
                bytes,
                header_offset + 32,
                endian,
            )?);
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_PHDR => {
            if metadata.pt_phdr_memory_address.is_none() {
                metadata.pt_phdr_memory_address = Some(Address::new(read_u64_at_u64(
                    bytes,
                    header_offset + 16,
                    endian,
                )?));
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_TLS => {
            metadata.has_tls = true;
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_STACK => {
            let executable = read_u32_at_u64(bytes, header_offset + 4, endian)? & PF_X != 0;
            metadata.gnu_stack_executable =
                Some(metadata.gnu_stack_executable.unwrap_or(false) || executable);
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_RELRO => {
            if metadata.gnu_relro_virtual_address.is_none() {
                metadata.gnu_relro_virtual_address = Some(Address::new(read_u64_at_u64(
                    bytes,
                    header_offset + 16,
                    endian,
                )?));
                metadata.gnu_relro_memory_size =
                    Some(read_u64_at_u64(bytes, header_offset + 40, endian)?);
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_EH_FRAME => {
            if metadata.gnu_eh_frame_virtual_address.is_none() {
                metadata.gnu_eh_frame_virtual_address = Some(Address::new(read_u64_at_u64(
                    bytes,
                    header_offset + 16,
                    endian,
                )?));
                metadata.gnu_eh_frame_memory_size =
                    Some(read_u64_at_u64(bytes, header_offset + 40, endian)?);
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_PROPERTY => {
            if metadata.gnu_property_virtual_address.is_none() {
                metadata.gnu_property_virtual_address = Some(Address::new(read_u64_at_u64(
                    bytes,
                    header_offset + 16,
                    endian,
                )?));
                metadata.gnu_property_memory_size =
                    Some(read_u64_at_u64(bytes, header_offset + 40, endian)?);
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        _ => Ok(ElfProgramHeaderAction::ConsiderLoad),
    }
}

pub(crate) fn summarize_elf32_program_header(
    bytes: &[u8],
    segment: u16,
    header_offset: u64,
    kind: u32,
    endian: BootElfEndian,
    load_mappings: &[ElfLoadMapping],
    metadata: &mut ElfProgramHeaderMetadata,
) -> Result<ElfProgramHeaderAction, BootError> {
    match kind {
        PT_DYNAMIC => {
            let file_offset = u64::from(read_u32_at_u64(bytes, header_offset + 4, endian)?);
            let file_size = u64::from(read_u32_at_u64(bytes, header_offset + 16, endian)?);
            let summary = dynamic_table_counts(
                bytes,
                segment,
                file_offset,
                file_size,
                8,
                endian,
                load_mappings,
            )?;
            metadata.dynamic_table =
                std::mem::take(&mut metadata.dynamic_table).with_segment(BootElfDynamicSegment {
                    file_offset,
                    virtual_address: Address::new(u64::from(read_u32_at_u64(
                        bytes,
                        header_offset + 8,
                        endian,
                    )?)),
                    entry_size: 8,
                    entry_count: summary.entry_count,
                    needed_count: summary.needed_libraries.len() as u64,
                    needed_libraries: summary.needed_libraries,
                    soname: summary.soname,
                    rpath: summary.rpath,
                    runpath: summary.runpath,
                    auxiliary_libraries: summary.auxiliary_libraries,
                    filter_libraries: summary.filter_libraries,
                    audit_libraries: summary.audit_libraries,
                    dependency_audit_libraries: summary.dependency_audit_libraries,
                    string_table_virtual_address: summary
                        .string_table_virtual_address
                        .map(Address::new),
                    string_table_size: summary.string_table_size,
                    symbol_table_virtual_address: summary
                        .symbol_table_virtual_address
                        .map(Address::new),
                    symbol_table_entry_size: summary.symbol_table_entry_size,
                    init_virtual_address: summary.init_virtual_address.map(Address::new),
                    fini_virtual_address: summary.fini_virtual_address.map(Address::new),
                    init_array_virtual_address: summary
                        .init_array_virtual_address
                        .map(Address::new),
                    init_array_size: summary.init_array_size,
                    fini_array_virtual_address: summary
                        .fini_array_virtual_address
                        .map(Address::new),
                    fini_array_size: summary.fini_array_size,
                    preinit_array_virtual_address: summary
                        .preinit_array_virtual_address
                        .map(Address::new),
                    preinit_array_size: summary.preinit_array_size,
                    flags: summary.flags,
                    flags_1: summary.flags_1,
                    plt_got_virtual_address: summary.plt_got_virtual_address.map(Address::new),
                    debug_virtual_address: summary.debug_virtual_address.map(Address::new),
                    symbolic_binding: summary.symbolic_binding,
                    text_relocations: summary.text_relocations,
                    bind_now: summary.bind_now,
                    rela_relative_count: summary.rela_relative_count,
                    rel_relative_count: summary.rel_relative_count,
                    sysv_hash_virtual_address: summary.sysv_hash_virtual_address.map(Address::new),
                    gnu_hash_virtual_address: summary.gnu_hash_virtual_address.map(Address::new),
                    version_symbol_table_virtual_address: summary
                        .version_symbol_table_virtual_address
                        .map(Address::new),
                    version_definition_table_virtual_address: summary
                        .version_definition_table_virtual_address
                        .map(Address::new),
                    version_definition_count: summary.version_definition_count,
                    version_needed_table_virtual_address: summary
                        .version_needed_table_virtual_address
                        .map(Address::new),
                    version_needed_count: summary.version_needed_count,
                    rela_relocations: relocation_table(summary.rela_relocations),
                    rel_relocations: relocation_table(summary.rel_relocations),
                    plt_relocations: relocation_table(summary.plt_relocations),
                    plt_relocation_kind: plt_relocation_kind(summary.plt_relocation_kind),
                });
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_INTERP => {
            if metadata.interpreter.is_none() {
                metadata.interpreter = Some(read_interpreter(
                    bytes,
                    segment,
                    u64::from(read_u32_at_u64(bytes, header_offset + 4, endian)?),
                    u64::from(read_u32_at_u64(bytes, header_offset + 16, endian)?),
                )?);
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_NOTE => {
            metadata.note_segment_count = metadata.note_segment_count.saturating_add(1);
            metadata.note_file_size =
                metadata
                    .note_file_size
                    .saturating_add(u64::from(read_u32_at_u64(
                        bytes,
                        header_offset + 16,
                        endian,
                    )?));
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_PHDR => {
            if metadata.pt_phdr_memory_address.is_none() {
                metadata.pt_phdr_memory_address = Some(Address::new(u64::from(read_u32_at_u64(
                    bytes,
                    header_offset + 8,
                    endian,
                )?)));
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_TLS => {
            metadata.has_tls = true;
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_STACK => {
            let executable = read_u32_at_u64(bytes, header_offset + 24, endian)? & PF_X != 0;
            metadata.gnu_stack_executable =
                Some(metadata.gnu_stack_executable.unwrap_or(false) || executable);
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_RELRO => {
            if metadata.gnu_relro_virtual_address.is_none() {
                metadata.gnu_relro_virtual_address = Some(Address::new(u64::from(
                    read_u32_at_u64(bytes, header_offset + 8, endian)?,
                )));
                metadata.gnu_relro_memory_size = Some(u64::from(read_u32_at_u64(
                    bytes,
                    header_offset + 20,
                    endian,
                )?));
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_EH_FRAME => {
            if metadata.gnu_eh_frame_virtual_address.is_none() {
                metadata.gnu_eh_frame_virtual_address = Some(Address::new(u64::from(
                    read_u32_at_u64(bytes, header_offset + 8, endian)?,
                )));
                metadata.gnu_eh_frame_memory_size = Some(u64::from(read_u32_at_u64(
                    bytes,
                    header_offset + 20,
                    endian,
                )?));
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        PT_GNU_PROPERTY => {
            if metadata.gnu_property_virtual_address.is_none() {
                metadata.gnu_property_virtual_address = Some(Address::new(u64::from(
                    read_u32_at_u64(bytes, header_offset + 8, endian)?,
                )));
                metadata.gnu_property_memory_size = Some(u64::from(read_u32_at_u64(
                    bytes,
                    header_offset + 20,
                    endian,
                )?));
            }
            Ok(ElfProgramHeaderAction::Skip)
        }
        _ => Ok(ElfProgramHeaderAction::ConsiderLoad),
    }
}

fn relocation_table(summary: ElfDynamicRelocationSummary) -> BootElfDynamicRelocationTable {
    BootElfDynamicRelocationTable::new(
        summary.virtual_address.map(Address::new),
        summary.byte_size,
        summary.entry_size,
    )
}

fn plt_relocation_kind(
    kind: Option<ElfDynamicPltRelocationKind>,
) -> Option<BootElfDynamicPltRelocationKind> {
    match kind {
        Some(ElfDynamicPltRelocationKind::Rel) => Some(BootElfDynamicPltRelocationKind::Rel),
        Some(ElfDynamicPltRelocationKind::Rela) => Some(BootElfDynamicPltRelocationKind::Rela),
        None => None,
    }
}
