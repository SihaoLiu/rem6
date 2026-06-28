use crate::elf::{BootElfClass, BootElfEndian};
use crate::error::{invalid_elf, BootElfError, BootError};

const ELF64_SECTION_HEADER_SIZE: u16 = 64;
const ELF32_SECTION_HEADER_SIZE: u16 = 40;
const PN_XNUM: u16 = 0xffff;

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

fn section_header_table_out_of_bounds(bytes: &[u8], offset: u64, size: u64) -> BootError {
    invalid_elf(BootElfError::SectionHeaderTableOutOfBounds {
        offset,
        size,
        image_size: bytes.len() as u64,
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
