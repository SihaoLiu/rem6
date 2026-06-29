use crate::elf::{checked_file_range, read_u32_at_u64, read_u64_at_u64, BootElfEndian};
use crate::error::{invalid_elf, BootElfError, BootError};

const PT_LOAD: u32 = 1;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_PLTRELSZ: u64 = 2;
const DT_HASH: u64 = 4;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const DT_STRSZ: u64 = 10;
const DT_SYMENT: u64 = 11;
const DT_INIT: u64 = 12;
const DT_FINI: u64 = 13;
const DT_SONAME: u64 = 14;
const DT_RPATH: u64 = 15;
const DT_REL: u64 = 17;
const DT_RELSZ: u64 = 18;
const DT_RELENT: u64 = 19;
const DT_PLTREL: u64 = 20;
const DT_JMPREL: u64 = 23;
const DT_INIT_ARRAY: u64 = 25;
const DT_FINI_ARRAY: u64 = 26;
const DT_INIT_ARRAYSZ: u64 = 27;
const DT_FINI_ARRAYSZ: u64 = 28;
const DT_RUNPATH: u64 = 29;
const DT_FLAGS: u64 = 30;
const DT_PREINIT_ARRAY: u64 = 32;
const DT_PREINIT_ARRAYSZ: u64 = 33;
const DT_GNU_HASH: u64 = 0x6fff_fef5;
const DT_FLAGS_1: u64 = 0x6fff_fffb;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ElfLoadMapping {
    file_offset: u64,
    virtual_address: u64,
    file_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ElfDynamicTableSummary {
    pub(crate) entry_count: u64,
    pub(crate) needed_libraries: Vec<String>,
    pub(crate) soname: Option<String>,
    pub(crate) rpath: Vec<String>,
    pub(crate) runpath: Vec<String>,
    pub(crate) string_table_virtual_address: Option<u64>,
    pub(crate) string_table_size: Option<u64>,
    pub(crate) symbol_table_virtual_address: Option<u64>,
    pub(crate) symbol_table_entry_size: Option<u64>,
    pub(crate) init_virtual_address: Option<u64>,
    pub(crate) fini_virtual_address: Option<u64>,
    pub(crate) init_array_virtual_address: Option<u64>,
    pub(crate) init_array_size: Option<u64>,
    pub(crate) fini_array_virtual_address: Option<u64>,
    pub(crate) fini_array_size: Option<u64>,
    pub(crate) preinit_array_virtual_address: Option<u64>,
    pub(crate) preinit_array_size: Option<u64>,
    pub(crate) flags: Option<u64>,
    pub(crate) flags_1: Option<u64>,
    pub(crate) sysv_hash_virtual_address: Option<u64>,
    pub(crate) gnu_hash_virtual_address: Option<u64>,
    pub(crate) rela_relocations: ElfDynamicRelocationSummary,
    pub(crate) rel_relocations: ElfDynamicRelocationSummary,
    pub(crate) plt_relocations: ElfDynamicRelocationSummary,
    pub(crate) plt_relocation_kind: Option<ElfDynamicPltRelocationKind>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfDynamicRelocationSummary {
    pub(crate) virtual_address: Option<u64>,
    pub(crate) byte_size: u64,
    pub(crate) entry_size: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ElfDynamicPltRelocationKind {
    Rel,
    Rela,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DynamicStringKind {
    Needed,
    Soname,
    Rpath,
    Runpath,
}

impl DynamicStringKind {
    const fn tag(self) -> u64 {
        match self {
            Self::Needed => DT_NEEDED,
            Self::Soname => DT_SONAME,
            Self::Rpath => DT_RPATH,
            Self::Runpath => DT_RUNPATH,
        }
    }
}

pub(crate) fn elf64_load_mappings(
    bytes: &[u8],
    program_header_offset: u64,
    program_header_size: u16,
    program_header_count: u64,
    endian: BootElfEndian,
) -> Result<Vec<ElfLoadMapping>, BootError> {
    let mut mappings = Vec::new();
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index * u64::from(program_header_size);
        if read_u32_at_u64(bytes, header_offset, endian)? == PT_LOAD {
            mappings.push(ElfLoadMapping {
                file_offset: read_u64_at_u64(bytes, header_offset + 8, endian)?,
                virtual_address: read_u64_at_u64(bytes, header_offset + 16, endian)?,
                file_size: read_u64_at_u64(bytes, header_offset + 32, endian)?,
            });
        }
    }
    Ok(mappings)
}

pub(crate) fn elf32_load_mappings(
    bytes: &[u8],
    program_header_offset: u64,
    program_header_size: u16,
    program_header_count: u64,
    endian: BootElfEndian,
) -> Result<Vec<ElfLoadMapping>, BootError> {
    let mut mappings = Vec::new();
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index * u64::from(program_header_size);
        if read_u32_at_u64(bytes, header_offset, endian)? == PT_LOAD {
            mappings.push(ElfLoadMapping {
                file_offset: u64::from(read_u32_at_u64(bytes, header_offset + 4, endian)?),
                virtual_address: u64::from(read_u32_at_u64(bytes, header_offset + 8, endian)?),
                file_size: u64::from(read_u32_at_u64(bytes, header_offset + 16, endian)?),
            });
        }
    }
    Ok(mappings)
}

pub(crate) fn dynamic_table_counts(
    bytes: &[u8],
    segment: u16,
    file_offset: u64,
    file_size: u64,
    entry_size: u16,
    endian: BootElfEndian,
    load_mappings: &[ElfLoadMapping],
) -> Result<ElfDynamicTableSummary, BootError> {
    if file_size % u64::from(entry_size) != 0 {
        return Err(invalid_elf(BootElfError::DynamicTableSizeMisaligned {
            segment,
            size: file_size,
            entry_size,
        }));
    }
    checked_file_range(bytes, file_offset, file_size).map_err(|_| {
        invalid_elf(BootElfError::DynamicTableFileRangeOutOfBounds {
            segment,
            offset: file_offset,
            size: file_size,
            image_size: bytes.len() as u64,
        })
    })?;

    let mut entries = 0;
    let mut needed_offsets = Vec::new();
    let mut soname_offset = None;
    let mut rpath_offsets = Vec::new();
    let mut runpath_offsets = Vec::new();
    let mut init_virtual_address = None;
    let mut fini_virtual_address = None;
    let mut init_array_virtual_address = None;
    let mut init_array_size = None;
    let mut fini_array_virtual_address = None;
    let mut fini_array_size = None;
    let mut preinit_array_virtual_address = None;
    let mut preinit_array_size = None;
    let mut flags = None;
    let mut flags_1 = None;
    let mut sysv_hash_virtual_address = None;
    let mut gnu_hash_virtual_address = None;
    let mut symbol_table_virtual_address = None;
    let mut symbol_table_entry_size = None;
    let mut rela_relocations = ElfDynamicRelocationSummary::default();
    let mut rel_relocations = ElfDynamicRelocationSummary::default();
    let mut plt_relocations = ElfDynamicRelocationSummary::default();
    let mut plt_relocation_kind = None;
    let mut string_table = None;
    let mut string_table_size = None;
    for index in 0..(file_size / u64::from(entry_size)) {
        let offset = file_offset + index * u64::from(entry_size);
        let tag = read_dynamic_tag(bytes, offset, entry_size, endian)?;
        let value = read_dynamic_value(bytes, offset, entry_size, endian)?;
        entries += 1;
        if tag == DT_NULL {
            let mut plt_relocations = plt_relocations;
            if let Some(kind) = plt_relocation_kind {
                plt_relocations.entry_size = match kind {
                    ElfDynamicPltRelocationKind::Rel => rel_relocations.entry_size,
                    ElfDynamicPltRelocationKind::Rela => rela_relocations.entry_size,
                };
            }
            let strings = dynamic_strings(
                bytes,
                segment,
                load_mappings,
                string_table,
                string_table_size,
                &needed_offsets,
                soname_offset,
                &rpath_offsets,
                &runpath_offsets,
            )?;
            return Ok(ElfDynamicTableSummary {
                entry_count: entries,
                needed_libraries: strings.needed_libraries,
                soname: strings.soname,
                rpath: strings.rpath,
                runpath: strings.runpath,
                string_table_virtual_address: string_table,
                string_table_size,
                symbol_table_virtual_address,
                symbol_table_entry_size,
                init_virtual_address,
                fini_virtual_address,
                init_array_virtual_address,
                init_array_size,
                fini_array_virtual_address,
                fini_array_size,
                preinit_array_virtual_address,
                preinit_array_size,
                flags,
                flags_1,
                sysv_hash_virtual_address,
                gnu_hash_virtual_address,
                rela_relocations,
                rel_relocations,
                plt_relocations,
                plt_relocation_kind,
            });
        }
        if tag == DT_NEEDED {
            needed_offsets.push(value);
        } else if tag == DT_HASH {
            sysv_hash_virtual_address = Some(value);
        } else if tag == DT_STRTAB {
            string_table = Some(value);
        } else if tag == DT_STRSZ {
            string_table_size = Some(value);
        } else if tag == DT_SYMTAB {
            symbol_table_virtual_address = Some(value);
        } else if tag == DT_SYMENT {
            symbol_table_entry_size = Some(value);
        } else if tag == DT_GNU_HASH {
            gnu_hash_virtual_address = Some(value);
        } else if tag == DT_SONAME {
            soname_offset = Some(value);
        } else if tag == DT_RPATH {
            rpath_offsets.push(value);
        } else if tag == DT_RUNPATH {
            runpath_offsets.push(value);
        } else if tag == DT_INIT {
            init_virtual_address = Some(value);
        } else if tag == DT_FINI {
            fini_virtual_address = Some(value);
        } else if tag == DT_INIT_ARRAY {
            init_array_virtual_address = Some(value);
        } else if tag == DT_INIT_ARRAYSZ {
            init_array_size = Some(value);
        } else if tag == DT_FINI_ARRAY {
            fini_array_virtual_address = Some(value);
        } else if tag == DT_FINI_ARRAYSZ {
            fini_array_size = Some(value);
        } else if tag == DT_PREINIT_ARRAY {
            preinit_array_virtual_address = Some(value);
        } else if tag == DT_PREINIT_ARRAYSZ {
            preinit_array_size = Some(value);
        } else if tag == DT_FLAGS {
            flags = Some(value);
        } else if tag == DT_FLAGS_1 {
            flags_1 = Some(value);
        } else if tag == DT_RELA {
            rela_relocations.virtual_address = Some(value);
        } else if tag == DT_RELASZ {
            rela_relocations.byte_size = value;
        } else if tag == DT_RELAENT {
            rela_relocations.entry_size = value;
        } else if tag == DT_REL {
            rel_relocations.virtual_address = Some(value);
        } else if tag == DT_RELSZ {
            rel_relocations.byte_size = value;
        } else if tag == DT_RELENT {
            rel_relocations.entry_size = value;
        } else if tag == DT_JMPREL {
            plt_relocations.virtual_address = Some(value);
        } else if tag == DT_PLTRELSZ {
            plt_relocations.byte_size = value;
        } else if tag == DT_PLTREL {
            if value == DT_RELA {
                plt_relocation_kind = Some(ElfDynamicPltRelocationKind::Rela);
            } else if value == DT_REL {
                plt_relocation_kind = Some(ElfDynamicPltRelocationKind::Rel);
            }
        }
    }
    Err(invalid_elf(BootElfError::UnterminatedDynamicTable {
        segment,
    }))
}

fn read_dynamic_tag(
    bytes: &[u8],
    offset: u64,
    entry_size: u16,
    endian: BootElfEndian,
) -> Result<u64, BootError> {
    if entry_size == 8 {
        return read_u32_at_u64(bytes, offset, endian).map(u64::from);
    }
    read_u64_at_u64(bytes, offset, endian)
}

fn read_dynamic_value(
    bytes: &[u8],
    offset: u64,
    entry_size: u16,
    endian: BootElfEndian,
) -> Result<u64, BootError> {
    read_dynamic_tag(
        bytes,
        offset + u64::from(entry_size / 2),
        entry_size,
        endian,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DynamicStrings {
    needed_libraries: Vec<String>,
    soname: Option<String>,
    rpath: Vec<String>,
    runpath: Vec<String>,
}

fn dynamic_strings(
    bytes: &[u8],
    segment: u16,
    load_mappings: &[ElfLoadMapping],
    string_table: Option<u64>,
    string_table_size: Option<u64>,
    needed_offsets: &[u64],
    soname_offset: Option<u64>,
    rpath_offsets: &[u64],
    runpath_offsets: &[u64],
) -> Result<DynamicStrings, BootError> {
    if needed_offsets.is_empty()
        && soname_offset.is_none()
        && rpath_offsets.is_empty()
        && runpath_offsets.is_empty()
    {
        return Ok(DynamicStrings {
            needed_libraries: Vec::new(),
            soname: None,
            rpath: Vec::new(),
            runpath: Vec::new(),
        });
    }
    let Some(virtual_address) = string_table else {
        return Err(invalid_elf(BootElfError::DynamicStringTableMissing {
            segment,
        }));
    };
    let Some(size) = string_table_size else {
        return Err(invalid_elf(BootElfError::DynamicStringTableMissing {
            segment,
        }));
    };
    let strings = dynamic_string_table(bytes, segment, load_mappings, virtual_address, size)?;
    let needed_libraries = needed_offsets
        .iter()
        .map(|offset| dynamic_string_value(segment, DynamicStringKind::Needed, strings, *offset))
        .collect::<Result<Vec<_>, _>>()?;
    let soname = soname_offset
        .map(|offset| dynamic_string_value(segment, DynamicStringKind::Soname, strings, offset))
        .transpose()?;
    let rpath = dynamic_string_values(segment, DynamicStringKind::Rpath, strings, rpath_offsets)?;
    let runpath = dynamic_string_values(
        segment,
        DynamicStringKind::Runpath,
        strings,
        runpath_offsets,
    )?;
    Ok(DynamicStrings {
        needed_libraries,
        soname,
        rpath,
        runpath,
    })
}

fn dynamic_string_table<'a>(
    bytes: &'a [u8],
    segment: u16,
    load_mappings: &[ElfLoadMapping],
    virtual_address: u64,
    size: u64,
) -> Result<&'a [u8], BootError> {
    for mapping in load_mappings {
        let Some(delta) = virtual_address.checked_sub(mapping.virtual_address) else {
            continue;
        };
        let Some(end_delta) = delta.checked_add(size) else {
            continue;
        };
        if end_delta <= mapping.file_size {
            let Some(file_offset) = mapping.file_offset.checked_add(delta) else {
                continue;
            };
            if let Ok(strings) = checked_file_range(bytes, file_offset, size) {
                return Ok(strings);
            }
        }
    }
    Err(invalid_elf(
        BootElfError::DynamicStringTableAddressOutOfBounds {
            segment,
            virtual_address,
            size,
        },
    ))
}

fn dynamic_string_values(
    segment: u16,
    kind: DynamicStringKind,
    string_table: &[u8],
    offsets: &[u64],
) -> Result<Vec<String>, BootError> {
    offsets
        .iter()
        .map(|offset| dynamic_string_value(segment, kind, string_table, *offset))
        .collect()
}

fn dynamic_string_value(
    segment: u16,
    kind: DynamicStringKind,
    string_table: &[u8],
    offset: u64,
) -> Result<String, BootError> {
    let Ok(start) = usize::try_from(offset) else {
        return Err(invalid_elf(dynamic_string_out_of_bounds_error(
            segment,
            kind,
            offset,
            string_table.len() as u64,
        )));
    };
    let Some(rest) = string_table.get(start..) else {
        return Err(invalid_elf(dynamic_string_out_of_bounds_error(
            segment,
            kind,
            offset,
            string_table.len() as u64,
        )));
    };
    let Some(end) = rest.iter().position(|byte| *byte == 0) else {
        return Err(invalid_elf(dynamic_string_unterminated_error(
            segment, kind, offset,
        )));
    };
    let name = std::str::from_utf8(&rest[..end])
        .map_err(|_| invalid_elf(dynamic_string_invalid_error(segment, kind, offset)))?;
    if name.is_empty() {
        return Err(invalid_elf(dynamic_string_invalid_error(
            segment, kind, offset,
        )));
    }
    Ok(name.to_string())
}

fn dynamic_string_out_of_bounds_error(
    segment: u16,
    kind: DynamicStringKind,
    offset: u64,
    string_table_size: u64,
) -> BootElfError {
    if kind == DynamicStringKind::Needed {
        return BootElfError::DynamicNeededStringOutOfBounds {
            segment,
            offset,
            string_table_size,
        };
    }
    BootElfError::DynamicStringOutOfBounds {
        segment,
        tag: kind.tag(),
        offset,
        string_table_size,
    }
}

fn dynamic_string_unterminated_error(
    segment: u16,
    kind: DynamicStringKind,
    offset: u64,
) -> BootElfError {
    if kind == DynamicStringKind::Needed {
        return BootElfError::UnterminatedDynamicNeededString { segment, offset };
    }
    BootElfError::UnterminatedDynamicString {
        segment,
        tag: kind.tag(),
        offset,
    }
}

fn dynamic_string_invalid_error(
    segment: u16,
    kind: DynamicStringKind,
    offset: u64,
) -> BootElfError {
    if kind == DynamicStringKind::Needed {
        return BootElfError::InvalidDynamicNeededString { segment, offset };
    }
    BootElfError::InvalidDynamicString {
        segment,
        tag: kind.tag(),
        offset,
    }
}
