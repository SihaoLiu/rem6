use crate::elf::{BootElfClass, BootElfEndian};
use crate::error::{invalid_elf, BootElfError, BootError};

const ELF64_SECTION_HEADER_SIZE: u16 = 64;
const ELF32_SECTION_HEADER_SIZE: u16 = 40;
const PN_XNUM: u16 = 0xffff;
const SHN_XINDEX: u16 = 0xffff;

pub(crate) struct SectionTableLayout {
    pub(crate) offset: u64,
    pub(crate) header_size: u16,
    pub(crate) count: u64,
    pub(crate) string_section: u64,
}

pub(crate) fn resolve_program_header_count(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
    raw_count: u16,
) -> Result<u64, BootError> {
    if raw_count != PN_XNUM {
        return Ok(u64::from(raw_count));
    }
    let (section_offset, header_size, info_offset, expected_size) = match class {
        BootElfClass::Class64 => (
            read_u64(bytes, 40, endian)?,
            read_u16(bytes, 58, endian)?,
            44,
            ELF64_SECTION_HEADER_SIZE,
        ),
        BootElfClass::Class32 => (
            u64::from(read_u32(bytes, 32, endian)?),
            read_u16(bytes, 46, endian)?,
            28,
            ELF32_SECTION_HEADER_SIZE,
        ),
    };
    if header_size != expected_size {
        return Err(invalid_elf(BootElfError::UnsupportedSectionHeaderSize {
            expected: expected_size,
            actual: header_size,
        }));
    }
    if section_offset == 0 {
        return Err(section_header_table_out_of_bounds(
            bytes,
            section_offset,
            u64::from(expected_size),
        ));
    }
    checked_file_range(bytes, section_offset, u64::from(expected_size)).map_err(|_| {
        section_header_table_out_of_bounds(bytes, section_offset, u64::from(expected_size))
    })?;
    Ok(u64::from(read_u32(
        bytes,
        section_offset + info_offset,
        endian,
    )?))
}

pub(crate) fn program_header_table_size(
    bytes: &[u8],
    offset: u64,
    entry_size: u16,
    count: u64,
) -> Result<u64, BootError> {
    let size = u64::from(entry_size).checked_mul(count).ok_or_else(|| {
        invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
            offset,
            size: u64::MAX,
            image_size: bytes.len() as u64,
        })
    })?;
    checked_file_range(bytes, offset, size).map_err(|_| {
        invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
            offset,
            size,
            image_size: bytes.len() as u64,
        })
    })?;
    Ok(size)
}

pub(crate) fn section_table_layout(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
    offset: u64,
    header_size: u16,
    raw_count: u16,
    raw_string_section: u16,
) -> Result<Option<SectionTableLayout>, BootError> {
    let expected_size = expected_section_header_size(class);
    if offset == 0 || header_size != expected_size {
        return Ok(None);
    }
    validate_section_zero(bytes, offset, expected_size)?;
    let count = if raw_count == 0 {
        read_section_zero_count(bytes, class, endian, offset)?
    } else {
        u64::from(raw_count)
    };
    let string_section = if raw_string_section == SHN_XINDEX {
        u64::from(read_section_zero_string_index(
            bytes, class, endian, offset,
        )?)
    } else {
        u64::from(raw_string_section)
    };
    if count == 0 || string_section == 0 || string_section >= count {
        return Ok(None);
    }
    Ok(Some(SectionTableLayout {
        offset,
        header_size,
        count,
        string_section,
    }))
}

fn validate_section_zero(bytes: &[u8], offset: u64, header_size: u16) -> Result<(), BootError> {
    checked_file_range(bytes, offset, u64::from(header_size))
        .map(|_| ())
        .map_err(|_| section_header_table_out_of_bounds(bytes, offset, u64::from(header_size)))
}

fn expected_section_header_size(class: BootElfClass) -> u16 {
    match class {
        BootElfClass::Class64 => ELF64_SECTION_HEADER_SIZE,
        BootElfClass::Class32 => ELF32_SECTION_HEADER_SIZE,
    }
}

fn read_section_zero_count(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
    offset: u64,
) -> Result<u64, BootError> {
    match class {
        BootElfClass::Class64 => read_u64(bytes, checked_offset(bytes, offset, 32)?, endian),
        BootElfClass::Class32 => {
            read_u32(bytes, checked_offset(bytes, offset, 20)?, endian).map(u64::from)
        }
    }
}

fn read_section_zero_string_index(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
    offset: u64,
) -> Result<u32, BootError> {
    let link_offset = match class {
        BootElfClass::Class64 => 40,
        BootElfClass::Class32 => 24,
    };
    read_u32(bytes, checked_offset(bytes, offset, link_offset)?, endian)
}

fn section_header_table_out_of_bounds(bytes: &[u8], offset: u64, size: u64) -> BootError {
    invalid_elf(BootElfError::SectionHeaderTableOutOfBounds {
        offset,
        size,
        image_size: bytes.len() as u64,
    })
}

fn checked_offset(bytes: &[u8], offset: u64, delta: u64) -> Result<u64, BootError> {
    offset.checked_add(delta).ok_or_else(|| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: delta,
            image_size: bytes.len() as u64,
        })
    })
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
