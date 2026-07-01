use rem6_memory::Address;

use crate::elf::{BootElfClass, BootElfEndian, BootElfOperatingSystem};
use crate::elf_counts::section_table_layout;
use crate::elf_section_flags::ElfSectionExtraFlagSummary;
use crate::elf_section_indexes::ElfSectionIndexTableSummary;
use crate::elf_section_versions::ElfSectionVersionSummary;
use crate::elf_symbols::{
    summarize_elf32_symbol_table as summarize_elf32_symbols,
    summarize_elf64_symbol_table as summarize_elf64_symbols, symbol_section, ElfSymbolStringTable,
};
use crate::error::{invalid_elf, BootElfError, BootError};
use crate::metadata_tables::{
    BootElfSectionAddressRange, BootElfSectionAlignment, BootElfSectionArrays, BootElfSectionFlags,
    BootElfSectionGroups, BootElfSectionHashes, BootElfSectionHeaderTable,
    BootElfSectionIndexTables, BootElfSectionNameTable, BootElfSectionRelocations,
    BootElfSectionStorage, BootElfSectionVersions, BootElfSymbolSummary,
};

const SHT_INIT_ARRAY: u32 = 14;
const SHT_FINI_ARRAY: u32 = 15;
const SHT_PREINIT_ARRAY: u32 = 16;
const SHT_HASH: u32 = 5;
const SHT_GNU_HASH: u32 = 0x6fff_fff6;
const SHT_GROUP: u32 = 17;
const SHT_RELA: u32 = 4;
const SHT_NOTE: u32 = 7;
const SHT_NOBITS: u32 = 8;
const SHT_REL: u32 = 9;
const SHT_RELR: u32 = 19;
const SHT_STRTAB: u32 = 3;
const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;
const SHF_EXECINSTR: u64 = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionSummary {
    operating_system: Option<BootElfOperatingSystem>,
    has_tls: bool,
    symbol_count: u64,
    function_symbol_count: u64,
    ifunc_symbol_count: u64,
    object_symbol_count: u64,
    tls_symbol_count: u64,
    local_symbol_count: u64,
    global_symbol_count: u64,
    weak_symbol_count: u64,
    unique_symbol_count: u64,
    undefined_symbol_section_count: u64,
    absolute_symbol_section_count: u64,
    common_symbol_section_count: u64,
    default_visibility_symbol_count: u64,
    internal_visibility_symbol_count: u64,
    hidden_visibility_symbol_count: u64,
    protected_visibility_symbol_count: u64,
    allocated_section_count: u64,
    writable_section_count: u64,
    executable_section_count: u64,
    nobits_section_count: u64,
    extra_flags: ElfSectionExtraFlagSummary,
    note_section_count: u64,
    relocation_section_count: u64,
    file_backed_section_bytes: u64,
    allocated_section_bytes: u64,
    writable_section_bytes: u64,
    executable_section_bytes: u64,
    nobits_section_bytes: u64,
    string_table_count: u64,
    string_table_bytes: u64,
    note_section_file_size: u64,
    relocation_section_file_size: u64,
    rela_section_count: u64,
    rela_entry_count: u64,
    rel_section_count: u64,
    rel_entry_count: u64,
    relr_section_count: u64,
    relr_entry_count: u64,
    init_array_section_count: u64,
    init_array_bytes: u64,
    init_array_entry_count: u64,
    fini_array_section_count: u64,
    fini_array_bytes: u64,
    fini_array_entry_count: u64,
    preinit_array_section_count: u64,
    preinit_array_bytes: u64,
    preinit_array_entry_count: u64,
    sysv_hash_section_count: u64,
    sysv_hash_bytes: u64,
    gnu_hash_section_count: u64,
    gnu_hash_bytes: u64,
    section_index_tables: ElfSectionIndexTableSummary,
    section_versions: ElfSectionVersionSummary,
    group_section_count: u64,
    group_section_bytes: u64,
    group_entry_count: u64,
    section_address_start: Option<u64>,
    section_address_end: Option<u64>,
    max_section_alignment: u64,
    allocated_max_section_alignment: u64,
    misaligned_allocated_section_count: u64,
    section_header_table: BootElfSectionHeaderTable,
    section_name_table: BootElfSectionNameTable,
}

impl ElfSectionSummary {
    pub(crate) const fn operating_system(self) -> Option<BootElfOperatingSystem> {
        self.operating_system
    }
    pub(crate) const fn has_tls(self) -> bool {
        self.has_tls
    }
    pub(crate) const fn symbol_summary(self) -> BootElfSymbolSummary {
        BootElfSymbolSummary::new(
            self.symbol_count,
            self.function_symbol_count,
            self.ifunc_symbol_count,
            self.object_symbol_count,
            self.tls_symbol_count,
            self.local_symbol_count,
            self.global_symbol_count,
            self.weak_symbol_count,
            self.unique_symbol_count,
            self.undefined_symbol_section_count,
            self.absolute_symbol_section_count,
            self.common_symbol_section_count,
            self.default_visibility_symbol_count,
            self.internal_visibility_symbol_count,
            self.hidden_visibility_symbol_count,
            self.protected_visibility_symbol_count,
        )
    }
    pub(crate) const fn note_section_count(self) -> u64 {
        self.note_section_count
    }
    pub(crate) const fn note_section_file_size(self) -> u64 {
        self.note_section_file_size
    }
    pub(crate) const fn section_relocations(self) -> BootElfSectionRelocations {
        BootElfSectionRelocations::new(
            self.relocation_section_count,
            self.relocation_section_file_size,
            self.rela_section_count,
            self.rela_entry_count,
            self.rel_section_count,
            self.rel_entry_count,
            self.relr_section_count,
            self.relr_entry_count,
        )
    }
    pub(crate) const fn section_arrays(self) -> BootElfSectionArrays {
        BootElfSectionArrays::new(
            self.init_array_section_count,
            self.init_array_bytes,
            self.init_array_entry_count,
            self.fini_array_section_count,
            self.fini_array_bytes,
            self.fini_array_entry_count,
            self.preinit_array_section_count,
            self.preinit_array_bytes,
            self.preinit_array_entry_count,
        )
    }
    pub(crate) const fn section_hashes(self) -> BootElfSectionHashes {
        BootElfSectionHashes::new(
            self.sysv_hash_section_count,
            self.sysv_hash_bytes,
            self.gnu_hash_section_count,
            self.gnu_hash_bytes,
        )
    }
    pub(crate) const fn section_versions(self) -> BootElfSectionVersions {
        self.section_versions.into_metadata()
    }
    pub(crate) const fn section_groups(self) -> BootElfSectionGroups {
        BootElfSectionGroups::new(
            self.group_section_count,
            self.group_section_bytes,
            self.group_entry_count,
        )
    }
    pub(crate) const fn section_header_table(self) -> BootElfSectionHeaderTable {
        self.section_header_table
    }
    pub(crate) const fn section_name_table(self) -> BootElfSectionNameTable {
        self.section_name_table
    }
    pub(crate) const fn section_flags(self) -> BootElfSectionFlags {
        self.extra_flags.into_metadata(
            self.allocated_section_count,
            self.writable_section_count,
            self.executable_section_count,
            self.nobits_section_count,
        )
    }
    pub(crate) const fn section_storage(self) -> BootElfSectionStorage {
        BootElfSectionStorage::new(
            self.file_backed_section_bytes,
            self.allocated_section_bytes,
            self.writable_section_bytes,
            self.executable_section_bytes,
            self.nobits_section_bytes,
            self.string_table_count,
            self.string_table_bytes,
        )
    }
    pub(crate) fn section_address_range(self) -> BootElfSectionAddressRange {
        BootElfSectionAddressRange::new(
            self.section_address_start.map(Address::new),
            self.section_address_end.map(Address::new),
        )
    }
    pub(crate) const fn section_alignment(self) -> BootElfSectionAlignment {
        BootElfSectionAlignment::new(
            self.max_section_alignment,
            self.allocated_max_section_alignment,
            self.misaligned_allocated_section_count,
        )
    }
    pub(crate) const fn section_index_tables(self) -> BootElfSectionIndexTables {
        self.section_index_tables.into_metadata()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ElfSectionHeader {
    name: u32,
    kind: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    alignment: u64,
    link: u32,
    info: u32,
    entry_size: u64,
}

pub(crate) fn elf_section_summary(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
    detect_operating_system: bool,
) -> Result<ElfSectionSummary, BootError> {
    match class {
        BootElfClass::Class32 => elf32_section_summary(bytes, endian, detect_operating_system),
        BootElfClass::Class64 => elf64_section_summary(bytes, endian, detect_operating_system),
    }
}

fn elf64_section_summary(
    bytes: &[u8],
    endian: BootElfEndian,
    detect_operating_system: bool,
) -> Result<ElfSectionSummary, BootError> {
    let section_table_offset = read_u64(bytes, 40, endian)?;
    let section_header_size = read_u16(bytes, 58, endian)?;
    let Some(table) = section_table_layout(
        bytes,
        BootElfClass::Class64,
        endian,
        section_table_offset,
        section_header_size,
        read_u16(bytes, 60, endian)?,
        read_u16(bytes, 62, endian)?,
    )?
    else {
        return Ok(ElfSectionSummary::default());
    };
    validate_section_table_range(bytes, table.offset, table.header_size, table.count)?;
    let string_header = read_elf64_section_header(
        bytes,
        table.offset,
        table.header_size,
        table.string_section,
        endian,
    )?;
    let string_table = checked_file_range(bytes, string_header.offset, string_header.size)
        .map_err(|_| {
            invalid_elf(BootElfError::SectionDataRangeOutOfBounds {
                offset: string_header.offset,
                size: string_header.size,
                image_size: bytes.len() as u64,
            })
        })?;
    let mut summary = ElfSectionSummary {
        section_header_table: BootElfSectionHeaderTable::new(
            table.offset,
            table.header_size,
            table.count,
            table.string_section,
        ),
        section_name_table: BootElfSectionNameTable::new(string_header.offset, string_header.size),
        ..ElfSectionSummary::default()
    };
    for index in 1..table.count {
        let section =
            read_elf64_section_header(bytes, table.offset, table.header_size, index, endian)?;
        summarize_elf64_section(
            bytes,
            string_table,
            table.offset,
            table.header_size,
            table.count,
            section,
            endian,
            detect_operating_system,
            &mut summary,
        )?;
    }
    Ok(summary)
}

fn elf32_section_summary(
    bytes: &[u8],
    endian: BootElfEndian,
    detect_operating_system: bool,
) -> Result<ElfSectionSummary, BootError> {
    let section_table_offset = u64::from(read_u32(bytes, 32, endian)?);
    let section_header_size = read_u16(bytes, 46, endian)?;
    let Some(table) = section_table_layout(
        bytes,
        BootElfClass::Class32,
        endian,
        section_table_offset,
        section_header_size,
        read_u16(bytes, 48, endian)?,
        read_u16(bytes, 50, endian)?,
    )?
    else {
        return Ok(ElfSectionSummary::default());
    };
    validate_section_table_range(bytes, table.offset, table.header_size, table.count)?;
    let string_header = read_elf32_section_header(
        bytes,
        table.offset,
        table.header_size,
        table.string_section,
        endian,
    )?;
    let string_table = checked_file_range(bytes, string_header.offset, string_header.size)
        .map_err(|_| {
            invalid_elf(BootElfError::SectionDataRangeOutOfBounds {
                offset: string_header.offset,
                size: string_header.size,
                image_size: bytes.len() as u64,
            })
        })?;

    let mut summary = ElfSectionSummary {
        section_header_table: BootElfSectionHeaderTable::new(
            table.offset,
            table.header_size,
            table.count,
            table.string_section,
        ),
        section_name_table: BootElfSectionNameTable::new(string_header.offset, string_header.size),
        ..ElfSectionSummary::default()
    };
    for index in 1..table.count {
        let section =
            read_elf32_section_header(bytes, table.offset, table.header_size, index, endian)?;
        summarize_elf32_section(
            bytes,
            string_table,
            table.offset,
            table.header_size,
            table.count,
            section,
            endian,
            detect_operating_system,
            &mut summary,
        )?;
    }
    Ok(summary)
}

fn summarize_elf64_section(
    bytes: &[u8],
    string_table: &[u8],
    table_offset: u64,
    header_size: u16,
    section_count: u64,
    section: ElfSectionHeader,
    endian: BootElfEndian,
    detect_operating_system: bool,
    summary: &mut ElfSectionSummary,
) -> Result<(), BootError> {
    summarize_common_section(
        bytes,
        string_table,
        section,
        endian,
        detect_operating_system,
        summary,
    )?;

    summarize_elf64_symbol_table(
        bytes,
        table_offset,
        header_size,
        section_count,
        section,
        endian,
        summary,
    );
    Ok(())
}

fn summarize_elf32_section(
    bytes: &[u8],
    string_table: &[u8],
    table_offset: u64,
    header_size: u16,
    section_count: u64,
    section: ElfSectionHeader,
    endian: BootElfEndian,
    detect_operating_system: bool,
    summary: &mut ElfSectionSummary,
) -> Result<(), BootError> {
    summarize_common_section(
        bytes,
        string_table,
        section,
        endian,
        detect_operating_system,
        summary,
    )?;

    summarize_elf32_symbol_table(
        bytes,
        table_offset,
        header_size,
        section_count,
        section,
        endian,
        summary,
    );
    Ok(())
}

fn summarize_common_section(
    bytes: &[u8],
    string_table: &[u8],
    section: ElfSectionHeader,
    endian: BootElfEndian,
    detect_operating_system: bool,
    summary: &mut ElfSectionSummary,
) -> Result<(), BootError> {
    if section_name_matches(string_table, section.name, b".tbss") {
        summary.has_tls = true;
    }
    if summary.extra_flags.record(section.flags) {
        summary.has_tls = true;
    }
    if section.kind != SHT_NOBITS {
        summary.file_backed_section_bytes = summary
            .file_backed_section_bytes
            .saturating_add(section.size);
    }
    summary.max_section_alignment = summary.max_section_alignment.max(section.alignment);
    if section.flags & SHF_ALLOC != 0 {
        summary.allocated_section_count += 1;
        summary.allocated_section_bytes =
            summary.allocated_section_bytes.saturating_add(section.size);
        summary.allocated_max_section_alignment = summary
            .allocated_max_section_alignment
            .max(section.alignment);
        if section.size != 0 {
            let end = section.address.saturating_add(section.size);
            summary.section_address_start = Some(
                summary
                    .section_address_start
                    .map_or(section.address, |start| start.min(section.address)),
            );
            summary.section_address_end = Some(
                summary
                    .section_address_end
                    .map_or(end, |current_end| current_end.max(end)),
            );
            if section.alignment > 1 && section.address % section.alignment != 0 {
                summary.misaligned_allocated_section_count += 1;
            }
        }
    }
    if section.flags & SHF_WRITE != 0 {
        summary.writable_section_count += 1;
        summary.writable_section_bytes =
            summary.writable_section_bytes.saturating_add(section.size);
    }
    if section.flags & SHF_EXECINSTR != 0 {
        summary.executable_section_count += 1;
        summary.executable_section_bytes = summary
            .executable_section_bytes
            .saturating_add(section.size);
    }
    if section.kind == SHT_NOBITS {
        summary.nobits_section_count += 1;
        summary.nobits_section_bytes = summary.nobits_section_bytes.saturating_add(section.size);
    }
    if section.kind == SHT_STRTAB {
        summary.string_table_count += 1;
        summary.string_table_bytes = summary.string_table_bytes.saturating_add(section.size);
    }
    summarize_array_section(summary, section.kind, section.size, section.entry_size);
    summarize_hash_section(summary, section.kind, section.size);
    summary
        .section_index_tables
        .record(section.kind, section.size, section.entry_size);
    summary.section_versions.record_section(
        section.kind,
        section.size,
        section.entry_size,
        section.info,
    );
    summarize_group_section(summary, section.kind, section.size, section.entry_size);
    if section.kind == SHT_NOTE {
        summary.note_section_count += 1;
        summary.note_section_file_size =
            summary.note_section_file_size.saturating_add(section.size);
    }
    match section.kind {
        SHT_RELA => summarize_relocation_section(
            summary,
            section.size,
            section.entry_size,
            RelocationSectionKind::Rela,
        ),
        SHT_REL => summarize_relocation_section(
            summary,
            section.size,
            section.entry_size,
            RelocationSectionKind::Rel,
        ),
        SHT_RELR => summarize_relocation_section(
            summary,
            section.size,
            section.entry_size,
            RelocationSectionKind::Relr,
        ),
        _ => {}
    }

    if detect_operating_system && summary.operating_system.is_none() {
        summary.operating_system =
            detect_section_operating_system(bytes, string_table, section, endian)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RelocationSectionKind {
    Rela,
    Rel,
    Relr,
}

fn summarize_array_section(summary: &mut ElfSectionSummary, kind: u32, size: u64, entry_size: u64) {
    let entry_count = section_entry_count(size, entry_size);
    match kind {
        SHT_INIT_ARRAY => {
            summary.init_array_section_count += 1;
            summary.init_array_bytes = summary.init_array_bytes.saturating_add(size);
            summary.init_array_entry_count =
                summary.init_array_entry_count.saturating_add(entry_count);
        }
        SHT_FINI_ARRAY => {
            summary.fini_array_section_count += 1;
            summary.fini_array_bytes = summary.fini_array_bytes.saturating_add(size);
            summary.fini_array_entry_count =
                summary.fini_array_entry_count.saturating_add(entry_count);
        }
        SHT_PREINIT_ARRAY => {
            summary.preinit_array_section_count += 1;
            summary.preinit_array_bytes = summary.preinit_array_bytes.saturating_add(size);
            summary.preinit_array_entry_count = summary
                .preinit_array_entry_count
                .saturating_add(entry_count);
        }
        _ => {}
    }
}

fn summarize_hash_section(summary: &mut ElfSectionSummary, kind: u32, size: u64) {
    match kind {
        SHT_HASH => {
            summary.sysv_hash_section_count += 1;
            summary.sysv_hash_bytes = summary.sysv_hash_bytes.saturating_add(size);
        }
        SHT_GNU_HASH => {
            summary.gnu_hash_section_count += 1;
            summary.gnu_hash_bytes = summary.gnu_hash_bytes.saturating_add(size);
        }
        _ => {}
    }
}

fn summarize_group_section(summary: &mut ElfSectionSummary, kind: u32, size: u64, entry_size: u64) {
    if kind == SHT_GROUP {
        summary.group_section_count += 1;
        summary.group_section_bytes = summary.group_section_bytes.saturating_add(size);
        summary.group_entry_count = summary
            .group_entry_count
            .saturating_add(section_entry_count(size, entry_size));
    }
}

fn summarize_relocation_section(
    summary: &mut ElfSectionSummary,
    size: u64,
    entry_size: u64,
    kind: RelocationSectionKind,
) {
    summary.relocation_section_count += 1;
    summary.relocation_section_file_size =
        summary.relocation_section_file_size.saturating_add(size);
    let entry_count = section_entry_count(size, entry_size);
    match kind {
        RelocationSectionKind::Rela => {
            summary.rela_section_count += 1;
            summary.rela_entry_count = summary.rela_entry_count.saturating_add(entry_count);
        }
        RelocationSectionKind::Rel => {
            summary.rel_section_count += 1;
            summary.rel_entry_count = summary.rel_entry_count.saturating_add(entry_count);
        }
        RelocationSectionKind::Relr => {
            summary.relr_section_count += 1;
            summary.relr_entry_count = summary.relr_entry_count.saturating_add(entry_count);
        }
    }
}

fn section_entry_count(size: u64, entry_size: u64) -> u64 {
    if entry_size == 0 {
        0
    } else {
        size / entry_size
    }
}

fn validate_section_table_range(
    bytes: &[u8],
    offset: u64,
    header_size: u16,
    count: u64,
) -> Result<(), BootError> {
    let size = u64::from(header_size).checked_mul(count).ok_or_else(|| {
        invalid_elf(BootElfError::SectionHeaderTableOutOfBounds {
            offset,
            size: u64::MAX,
            image_size: bytes.len() as u64,
        })
    })?;
    checked_file_range(bytes, offset, size).map_err(|_| {
        invalid_elf(BootElfError::SectionHeaderTableOutOfBounds {
            offset,
            size,
            image_size: bytes.len() as u64,
        })
    })?;
    Ok(())
}

fn read_elf64_section_header(
    bytes: &[u8],
    table_offset: u64,
    header_size: u16,
    index: u64,
    endian: BootElfEndian,
) -> Result<ElfSectionHeader, BootError> {
    let base = table_offset + index * u64::from(header_size);
    Ok(ElfSectionHeader {
        name: read_u32(bytes, base, endian)?,
        kind: read_u32(bytes, base + 4, endian)?,
        flags: read_u64(bytes, base + 8, endian)?,
        address: read_u64(bytes, base + 16, endian)?,
        offset: read_u64(bytes, base + 24, endian)?,
        size: read_u64(bytes, base + 32, endian)?,
        alignment: read_u64(bytes, base + 48, endian)?,
        link: read_u32(bytes, base + 40, endian)?,
        info: read_u32(bytes, base + 44, endian)?,
        entry_size: read_u64(bytes, base + 56, endian)?,
    })
}

fn read_elf32_section_header(
    bytes: &[u8],
    table_offset: u64,
    header_size: u16,
    index: u64,
    endian: BootElfEndian,
) -> Result<ElfSectionHeader, BootError> {
    let base = table_offset + index * u64::from(header_size);
    Ok(ElfSectionHeader {
        name: read_u32(bytes, base, endian)?,
        kind: read_u32(bytes, base + 4, endian)?,
        flags: u64::from(read_u32(bytes, base + 8, endian)?),
        address: u64::from(read_u32(bytes, base + 12, endian)?),
        offset: u64::from(read_u32(bytes, base + 16, endian)?),
        size: u64::from(read_u32(bytes, base + 20, endian)?),
        alignment: u64::from(read_u32(bytes, base + 32, endian)?),
        link: read_u32(bytes, base + 24, endian)?,
        info: read_u32(bytes, base + 28, endian)?,
        entry_size: u64::from(read_u32(bytes, base + 36, endian)?),
    })
}

fn summarize_elf64_symbol_table(
    bytes: &[u8],
    table_offset: u64,
    header_size: u16,
    section_count: u64,
    section: ElfSectionHeader,
    endian: BootElfEndian,
    summary: &mut ElfSectionSummary,
) {
    let Some(section) = symbol_section(
        section.kind,
        section.offset,
        section.size,
        section.link,
        section.entry_size,
        24,
        section_count,
    ) else {
        return;
    };
    let Ok(strings_header) = read_elf64_section_header(
        bytes,
        table_offset,
        header_size,
        u64::from(section.link),
        endian,
    ) else {
        return;
    };
    let strings = ElfSymbolStringTable {
        offset: strings_header.offset,
        size: strings_header.size,
    };
    add_symbol_summary(
        summary,
        summarize_elf64_symbols(bytes, section, strings, endian),
    );
}

fn summarize_elf32_symbol_table(
    bytes: &[u8],
    table_offset: u64,
    header_size: u16,
    section_count: u64,
    section: ElfSectionHeader,
    endian: BootElfEndian,
    summary: &mut ElfSectionSummary,
) {
    let Some(section) = symbol_section(
        section.kind,
        section.offset,
        section.size,
        section.link,
        section.entry_size,
        16,
        section_count,
    ) else {
        return;
    };
    let Ok(strings_header) = read_elf32_section_header(
        bytes,
        table_offset,
        header_size,
        u64::from(section.link),
        endian,
    ) else {
        return;
    };
    let strings = ElfSymbolStringTable {
        offset: strings_header.offset,
        size: strings_header.size,
    };
    add_symbol_summary(
        summary,
        summarize_elf32_symbols(bytes, section, strings, endian),
    );
}

fn add_symbol_summary(summary: &mut ElfSectionSummary, symbols: BootElfSymbolSummary) {
    summary.symbol_count += symbols.total_count();
    summary.function_symbol_count += symbols.function_count();
    summary.ifunc_symbol_count += symbols.ifunc_count();
    summary.object_symbol_count += symbols.object_count();
    summary.tls_symbol_count += symbols.tls_count();
    summary.local_symbol_count += symbols.local_count();
    summary.global_symbol_count += symbols.global_count();
    summary.weak_symbol_count += symbols.weak_count();
    summary.unique_symbol_count += symbols.unique_count();
    summary.undefined_symbol_section_count += symbols.undefined_section_count();
    summary.absolute_symbol_section_count += symbols.absolute_section_count();
    summary.common_symbol_section_count += symbols.common_section_count();
    summary.default_visibility_symbol_count += symbols.default_visibility_count();
    summary.internal_visibility_symbol_count += symbols.internal_visibility_count();
    summary.hidden_visibility_symbol_count += symbols.hidden_visibility_count();
    summary.protected_visibility_symbol_count += symbols.protected_visibility_count();
}

fn detect_section_operating_system(
    bytes: &[u8],
    string_table: &[u8],
    section: ElfSectionHeader,
    endian: BootElfEndian,
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    if section.kind == SHT_NOTE
        && section_name_matches(string_table, section.name, b".note.ABI-tag")
    {
        let section_data =
            checked_file_range(bytes, section.offset, section.size).map_err(|_| {
                invalid_elf(BootElfError::SectionDataRangeOutOfBounds {
                    offset: section.offset,
                    size: section.size,
                    image_size: bytes.len() as u64,
                })
            })?;
        if section_data.len() >= 20 {
            let os = read_u32(section_data, 16, endian)?;
            return Ok(match os {
                0 => Some(BootElfOperatingSystem::Linux),
                2 => Some(BootElfOperatingSystem::Solaris),
                3 => Some(BootElfOperatingSystem::FreeBsd),
                _ => None,
            });
        }
    }

    if section_name_matches(string_table, section.name, b".SUNW_version")
        || section_name_matches(string_table, section.name, b".stab.index")
    {
        return Ok(Some(BootElfOperatingSystem::Solaris));
    }

    Ok(None)
}

fn section_name_matches(string_table: &[u8], name_offset: u32, expected: &[u8]) -> bool {
    let Ok(start) = usize::try_from(name_offset) else {
        return false;
    };
    let Some(rest) = string_table.get(start..) else {
        return false;
    };
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rest.len());
    &rest[..end] == expected
}

fn read_u16(bytes: &[u8], offset: u64, endian: BootElfEndian) -> Result<u16, BootError> {
    let data = read_exact(bytes, offset, 2)?;
    Ok(match endian {
        BootElfEndian::Little => u16::from_le_bytes([data[0], data[1]]),
        BootElfEndian::Big => u16::from_be_bytes([data[0], data[1]]),
    })
}

fn read_u32(bytes: &[u8], offset: u64, endian: BootElfEndian) -> Result<u32, BootError> {
    let data = read_exact(bytes, offset, 4)?;
    Ok(match endian {
        BootElfEndian::Little => u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        BootElfEndian::Big => u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
    })
}

fn read_u64(bytes: &[u8], offset: u64, endian: BootElfEndian) -> Result<u64, BootError> {
    let data = read_exact(bytes, offset, 8)?;
    Ok(match endian {
        BootElfEndian::Little => u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]),
        BootElfEndian::Big => u64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]),
    })
}

fn read_exact(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], BootError> {
    checked_file_range(bytes, offset, size).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size,
            image_size: bytes.len() as u64,
        })
    })
}

fn checked_file_range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], ()> {
    let start = usize::try_from(offset).map_err(|_| ())?;
    let len = usize::try_from(size).map_err(|_| ())?;
    let end = start.checked_add(len).ok_or(())?;
    bytes.get(start..end).ok_or(())
}
