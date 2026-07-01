use crate::elf::{checked_file_range, read_u32_at_u64, BootElfEndian};
use crate::metadata_tables::BootElfSymbolSummary;

const SHT_SYMTAB: u32 = 2;
const SHT_DYNSYM: u32 = 11;
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STB_WEAK: u8 = 2;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STT_TLS: u8 = 6;
const STV_DEFAULT: u8 = 0;
const STV_INTERNAL: u8 = 1;
const STV_HIDDEN: u8 = 2;
const STV_PROTECTED: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ElfSymbolSection {
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) link: u32,
    pub(crate) entry_size: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ElfSymbolStringTable {
    pub(crate) offset: u64,
    pub(crate) size: u64,
}

pub(crate) fn symbol_section(
    kind: u32,
    offset: u64,
    size: u64,
    link: u32,
    entry_size: u64,
    minimum_entry_size: u64,
    section_count: u64,
) -> Option<ElfSymbolSection> {
    (matches!(kind, SHT_SYMTAB | SHT_DYNSYM)
        && entry_size >= minimum_entry_size
        && u64::from(link) < section_count)
        .then_some(ElfSymbolSection {
            offset,
            size,
            link,
            entry_size,
        })
}

pub(crate) fn summarize_elf64_symbol_table(
    bytes: &[u8],
    section: ElfSymbolSection,
    strings: ElfSymbolStringTable,
    endian: BootElfEndian,
) -> BootElfSymbolSummary {
    summarize_symbol_table(bytes, section, strings, endian, 4, 5)
}

pub(crate) fn summarize_elf32_symbol_table(
    bytes: &[u8],
    section: ElfSymbolSection,
    strings: ElfSymbolStringTable,
    endian: BootElfEndian,
) -> BootElfSymbolSummary {
    summarize_symbol_table(bytes, section, strings, endian, 12, 13)
}

fn summarize_symbol_table(
    bytes: &[u8],
    section: ElfSymbolSection,
    strings: ElfSymbolStringTable,
    endian: BootElfEndian,
    info_offset: u64,
    visibility_offset: u64,
) -> BootElfSymbolSummary {
    let Ok(symbols) = checked_file_range(bytes, section.offset, section.size) else {
        return BootElfSymbolSummary::default();
    };
    let Ok(strings) = checked_file_range(bytes, strings.offset, strings.size) else {
        return BootElfSymbolSummary::default();
    };

    let mut summary = BootElfSymbolSummary::default();
    for index in 0..(section.size / section.entry_size) {
        let offset = index * section.entry_size;
        let Ok(name) = read_u32_at_u64(symbols, offset, endian) else {
            continue;
        };
        if !symbol_name_allowed(strings, name) {
            continue;
        }
        let info = symbols[(offset + info_offset) as usize];
        let visibility = symbols[(offset + visibility_offset) as usize] & 0x3;
        summary = summarize_symbol_type(summary, info >> 4, info & 0xf, visibility);
    }
    summary
}

fn summarize_symbol_type(
    summary: BootElfSymbolSummary,
    binding: u8,
    kind: u8,
    visibility: u8,
) -> BootElfSymbolSummary {
    if !matches!(binding, STB_LOCAL | STB_GLOBAL | STB_WEAK) {
        return summary;
    }
    let total = summary.total_count() + 1;
    let mut functions = summary.function_count();
    let mut objects = summary.object_count();
    let mut tls = summary.tls_count();
    let mut locals = summary.local_count();
    let mut globals = summary.global_count();
    let mut weaks = summary.weak_count();
    let mut default_visibility = summary.default_visibility_count();
    let mut internal_visibility = summary.internal_visibility_count();
    let mut hidden_visibility = summary.hidden_visibility_count();
    let mut protected_visibility = summary.protected_visibility_count();

    if binding == STB_LOCAL {
        locals += 1;
    } else if binding == STB_GLOBAL {
        globals += 1;
    } else {
        weaks += 1;
    }
    match kind {
        STT_FUNC => functions += 1,
        STT_OBJECT => objects += 1,
        STT_TLS => tls += 1,
        _ => {}
    }
    match visibility {
        STV_DEFAULT => default_visibility += 1,
        STV_INTERNAL => internal_visibility += 1,
        STV_HIDDEN => hidden_visibility += 1,
        STV_PROTECTED => protected_visibility += 1,
        _ => {}
    }
    BootElfSymbolSummary::new(
        total,
        functions,
        objects,
        tls,
        locals,
        globals,
        weaks,
        default_visibility,
        internal_visibility,
        hidden_visibility,
        protected_visibility,
    )
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
