use crate::elf::{checked_file_range, read_u16_at_u64, read_u32_at_u64, BootElfEndian};
use crate::metadata_tables::BootElfSymbolSummary;

const SHT_SYMTAB: u32 = 2;
const SHT_DYNSYM: u32 = 11;
const SHN_UNDEF: u16 = 0;
const SHN_ABS: u16 = 0xfff1;
const SHN_COMMON: u16 = 0xfff2;
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STB_WEAK: u8 = 2;
const STB_GNU_UNIQUE: u8 = 10;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STT_TLS: u8 = 6;
const STT_GNU_IFUNC: u8 = 10;
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
    summarize_symbol_table(bytes, section, strings, endian, 4, 5, 6)
}

pub(crate) fn summarize_elf32_symbol_table(
    bytes: &[u8],
    section: ElfSymbolSection,
    strings: ElfSymbolStringTable,
    endian: BootElfEndian,
) -> BootElfSymbolSummary {
    summarize_symbol_table(bytes, section, strings, endian, 12, 13, 14)
}

fn summarize_symbol_table(
    bytes: &[u8],
    section: ElfSymbolSection,
    strings: ElfSymbolStringTable,
    endian: BootElfEndian,
    info_offset: u64,
    visibility_offset: u64,
    section_index_offset: u64,
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
        let Ok(section_index) = read_u16_at_u64(symbols, offset + section_index_offset, endian)
        else {
            continue;
        };
        summary = summarize_symbol_type(summary, info >> 4, info & 0xf, visibility, section_index);
    }
    summary
}

fn summarize_symbol_type(
    summary: BootElfSymbolSummary,
    binding: u8,
    kind: u8,
    visibility: u8,
    section_index: u16,
) -> BootElfSymbolSummary {
    if !matches!(binding, STB_LOCAL | STB_GLOBAL | STB_WEAK | STB_GNU_UNIQUE) {
        return summary;
    }
    let total = summary.total_count() + 1;
    let mut functions = summary.function_count();
    let mut ifuncs = summary.ifunc_count();
    let mut objects = summary.object_count();
    let mut tls = summary.tls_count();
    let mut locals = summary.local_count();
    let mut globals = summary.global_count();
    let mut weaks = summary.weak_count();
    let mut uniques = summary.unique_count();
    let mut undefined_sections = summary.undefined_section_count();
    let mut absolute_sections = summary.absolute_section_count();
    let mut common_sections = summary.common_section_count();
    let mut default_visibility = summary.default_visibility_count();
    let mut internal_visibility = summary.internal_visibility_count();
    let mut hidden_visibility = summary.hidden_visibility_count();
    let mut protected_visibility = summary.protected_visibility_count();

    if binding == STB_LOCAL {
        locals += 1;
    } else if binding == STB_GLOBAL {
        globals += 1;
    } else if binding == STB_WEAK {
        weaks += 1;
    } else {
        uniques += 1;
    }
    match kind {
        STT_FUNC => functions += 1,
        STT_GNU_IFUNC => ifuncs += 1,
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
    match section_index {
        SHN_UNDEF => undefined_sections += 1,
        SHN_ABS => absolute_sections += 1,
        SHN_COMMON => common_sections += 1,
        _ => {}
    }
    BootElfSymbolSummary::new(
        total,
        functions,
        ifuncs,
        objects,
        tls,
        locals,
        globals,
        weaks,
        uniques,
        undefined_sections,
        absolute_sections,
        common_sections,
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
