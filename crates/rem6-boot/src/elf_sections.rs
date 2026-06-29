use crate::elf::{BootElfClass, BootElfEndian, BootElfOperatingSystem};
use crate::elf_counts::section_table_layout;
use crate::error::{invalid_elf, BootElfError, BootError};
use crate::metadata::BootElfSectionHeaderTable;

const SHT_NOTE: u32 = 7;
const SHT_SYMTAB: u32 = 2;
const SHT_DYNSYM: u32 = 11;
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STB_WEAK: u8 = 2;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionSummary {
    operating_system: Option<BootElfOperatingSystem>,
    has_tls: bool,
    symbol_count: u64,
    function_symbol_count: u64,
    object_symbol_count: u64,
    section_header_table: BootElfSectionHeaderTable,
}

impl ElfSectionSummary {
    pub(crate) const fn operating_system(self) -> Option<BootElfOperatingSystem> {
        self.operating_system
    }

    pub(crate) const fn has_tls(self) -> bool {
        self.has_tls
    }

    pub(crate) const fn symbol_count(self) -> u64 {
        self.symbol_count
    }

    pub(crate) const fn function_symbol_count(self) -> u64 {
        self.function_symbol_count
    }

    pub(crate) const fn object_symbol_count(self) -> u64 {
        self.object_symbol_count
    }

    pub(crate) const fn section_header_table(self) -> BootElfSectionHeaderTable {
        self.section_header_table
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ElfSectionHeader {
    name: u32,
    kind: u32,
    offset: u64,
    size: u64,
    link: u32,
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

    if detect_operating_system && summary.operating_system.is_none() {
        summary.operating_system =
            detect_section_operating_system(bytes, string_table, section, endian)?;
    }
    Ok(())
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
        offset: read_u64(bytes, base + 24, endian)?,
        size: read_u64(bytes, base + 32, endian)?,
        link: read_u32(bytes, base + 40, endian)?,
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
        offset: u64::from(read_u32(bytes, base + 16, endian)?),
        size: u64::from(read_u32(bytes, base + 20, endian)?),
        link: read_u32(bytes, base + 24, endian)?,
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
    if !matches!(section.kind, SHT_SYMTAB | SHT_DYNSYM)
        || section.entry_size < 24
        || u64::from(section.link) >= section_count
    {
        return;
    }
    let Ok(symbols) = checked_file_range(bytes, section.offset, section.size) else {
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
    let Ok(strings) = checked_file_range(bytes, strings_header.offset, strings_header.size) else {
        return;
    };

    let count = section.size / section.entry_size;
    for index in 0..count {
        let offset = index * section.entry_size;
        let Ok(name) = read_u32(symbols, offset, endian) else {
            continue;
        };
        if !symbol_name_allowed(strings, name) {
            continue;
        }
        let info = symbols[offset as usize + 4];
        summarize_symbol_type(summary, info >> 4, info & 0xf);
    }
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
    if !matches!(section.kind, SHT_SYMTAB | SHT_DYNSYM)
        || section.entry_size < 16
        || u64::from(section.link) >= section_count
    {
        return;
    }
    let Ok(symbols) = checked_file_range(bytes, section.offset, section.size) else {
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
    let Ok(strings) = checked_file_range(bytes, strings_header.offset, strings_header.size) else {
        return;
    };

    let count = section.size / section.entry_size;
    for index in 0..count {
        let offset = index * section.entry_size;
        let Ok(name) = read_u32(symbols, offset, endian) else {
            continue;
        };
        if !symbol_name_allowed(strings, name) {
            continue;
        }
        let info = symbols[offset as usize + 12];
        summarize_symbol_type(summary, info >> 4, info & 0xf);
    }
}

fn summarize_symbol_type(summary: &mut ElfSectionSummary, binding: u8, kind: u8) {
    if !matches!(binding, STB_LOCAL | STB_GLOBAL | STB_WEAK) {
        return;
    }
    summary.symbol_count += 1;
    match kind {
        STT_FUNC => summary.function_symbol_count += 1,
        STT_OBJECT => summary.object_symbol_count += 1,
        _ => {}
    }
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

fn symbol_name_allowed(string_table: &[u8], name_offset: u32) -> bool {
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
    end != 0 && rest[0] != b'$'
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
