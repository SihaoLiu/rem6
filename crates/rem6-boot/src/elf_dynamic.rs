use crate::elf::{checked_file_range, read_u32_at_u64, read_u64_at_u64, BootElfEndian};
use crate::error::{invalid_elf, BootElfError, BootError};

const PT_LOAD: u32 = 1;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
const DT_STRSZ: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ElfLoadMapping {
    file_offset: u64,
    virtual_address: u64,
    file_size: u64,
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
) -> Result<(u64, u64, Vec<String>), BootError> {
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
    let mut string_table = None;
    let mut string_table_size = None;
    for index in 0..(file_size / u64::from(entry_size)) {
        let offset = file_offset + index * u64::from(entry_size);
        let tag = read_dynamic_tag(bytes, offset, entry_size, endian)?;
        let value = read_dynamic_value(bytes, offset, entry_size, endian)?;
        entries += 1;
        if tag == DT_NULL {
            let needed_libraries = dynamic_needed_libraries(
                bytes,
                segment,
                load_mappings,
                string_table,
                string_table_size,
                &needed_offsets,
            )?;
            return Ok((entries, needed_offsets.len() as u64, needed_libraries));
        }
        if tag == DT_NEEDED {
            needed_offsets.push(value);
        } else if tag == DT_STRTAB {
            string_table = Some(value);
        } else if tag == DT_STRSZ {
            string_table_size = Some(value);
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

fn dynamic_needed_libraries(
    bytes: &[u8],
    segment: u16,
    load_mappings: &[ElfLoadMapping],
    string_table: Option<u64>,
    string_table_size: Option<u64>,
    needed_offsets: &[u64],
) -> Result<Vec<String>, BootError> {
    if needed_offsets.is_empty() {
        return Ok(Vec::new());
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
    needed_offsets
        .iter()
        .map(|offset| dynamic_needed_library(segment, strings, *offset))
        .collect()
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

fn dynamic_needed_library(
    segment: u16,
    string_table: &[u8],
    offset: u64,
) -> Result<String, BootError> {
    let Ok(start) = usize::try_from(offset) else {
        return Err(invalid_elf(BootElfError::DynamicNeededStringOutOfBounds {
            segment,
            offset,
            string_table_size: string_table.len() as u64,
        }));
    };
    let Some(rest) = string_table.get(start..) else {
        return Err(invalid_elf(BootElfError::DynamicNeededStringOutOfBounds {
            segment,
            offset,
            string_table_size: string_table.len() as u64,
        }));
    };
    let Some(end) = rest.iter().position(|byte| *byte == 0) else {
        return Err(invalid_elf(BootElfError::UnterminatedDynamicNeededString {
            segment,
            offset,
        }));
    };
    let name = std::str::from_utf8(&rest[..end])
        .map_err(|_| invalid_elf(BootElfError::InvalidDynamicNeededString { segment, offset }))?;
    if name.is_empty() {
        return Err(invalid_elf(BootElfError::InvalidDynamicNeededString {
            segment,
            offset,
        }));
    }
    Ok(name.to_string())
}
