use rem6_memory::{AccessSize, Address, AddressRange};

use crate::error::{invalid_elf, BootElfError, BootError};
use crate::image::BootImage;

const ELF64_HEADER_SIZE: usize = 64;
const ELF64_PROGRAM_HEADER_SIZE: u16 = 56;
const ELF64_SECTION_HEADER_SIZE: u16 = 64;
const ELF32_HEADER_SIZE: usize = 52;
const ELF32_PROGRAM_HEADER_SIZE: u16 = 32;
const ELF32_SECTION_HEADER_SIZE: u16 = 40;
const ELF_CLASS_32: u8 = 1;
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LITTLE: u8 = 1;
const ELF_VERSION_CURRENT: u8 = 1;
const ELF_OSABI_LINUX: u8 = 3;
const ELF_OSABI_SOLARIS: u8 = 6;
const ELF_OSABI_FREEBSD: u8 = 9;
const ELF_OSABI_TRU64: u8 = 10;
const ELF_OSABI_ARM: u8 = 97;
const PT_LOAD: u32 = 1;
const SHT_NOTE: u32 = 7;
const EM_SPARC: u16 = 2;
const EM_386: u16 = 3;
const EM_MIPS: u16 = 8;
const EM_SPARC64: u16 = 11;
const EM_SPARC32PLUS: u16 = 18;
const EM_PPC: u16 = 20;
const EM_PPC64: u16 = 21;
const EM_ARM: u16 = 40;
const EM_SPARCV9: u16 = 43;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;
const EM_RISCV: u16 = 243;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootElfClass {
    Class32,
    Class64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootElfArchitecture {
    Sparc32,
    Sparc64,
    Mips,
    I386,
    X8664,
    Arm,
    Thumb,
    Arm64,
    Riscv32,
    Riscv64,
    Power,
    Power64,
    Unknown { machine: u16, class: BootElfClass },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootElfOperatingSystem {
    Linux,
    Solaris,
    Tru64,
    LinuxArmOabi,
    LinuxPower64AbiV1,
    LinuxPower64AbiV2,
    FreeBsd,
    Unknown { os_abi: u8 },
}

impl BootElfOperatingSystem {
    const fn from_header(machine: u16, os_abi: u8, flags: u32) -> Self {
        if machine == EM_PPC64 {
            return match flags & 0x3 {
                0x1 => Self::LinuxPower64AbiV1,
                0x2 => Self::LinuxPower64AbiV2,
                _ => Self::LinuxPower64AbiV2,
            };
        }

        match os_abi {
            ELF_OSABI_LINUX => Self::Linux,
            ELF_OSABI_SOLARIS => Self::Solaris,
            ELF_OSABI_TRU64 => Self::Tru64,
            ELF_OSABI_ARM => Self::LinuxArmOabi,
            ELF_OSABI_FREEBSD => Self::FreeBsd,
            _ => Self::Unknown { os_abi },
        }
    }
}

impl BootElfArchitecture {
    const fn from_machine(class: BootElfClass, machine: u16, entry: Address) -> Self {
        match (machine, class) {
            (EM_SPARC64, _) | (EM_SPARCV9, _) | (EM_SPARC, BootElfClass::Class64) => Self::Sparc64,
            (EM_SPARC32PLUS, _) | (EM_SPARC, BootElfClass::Class32) => Self::Sparc32,
            (EM_MIPS, BootElfClass::Class32) => Self::Mips,
            (EM_X86_64, BootElfClass::Class64) => Self::X8664,
            (EM_386, BootElfClass::Class32) => Self::I386,
            (EM_ARM, BootElfClass::Class32) => {
                if entry.get() & 1 == 0 {
                    Self::Arm
                } else {
                    Self::Thumb
                }
            }
            (EM_AARCH64, BootElfClass::Class64) => Self::Arm64,
            (EM_RISCV, BootElfClass::Class64) => Self::Riscv64,
            (EM_RISCV, BootElfClass::Class32) => Self::Riscv32,
            (EM_PPC, BootElfClass::Class32) => Self::Power,
            (EM_PPC64, BootElfClass::Class64) => Self::Power64,
            _ => Self::Unknown { machine, class },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootElfMetadata {
    class: BootElfClass,
    machine: u16,
    os_abi: u8,
    flags: u32,
    architecture: BootElfArchitecture,
    operating_system: BootElfOperatingSystem,
}

impl BootElfMetadata {
    const fn from_header(
        class: BootElfClass,
        machine: u16,
        os_abi: u8,
        flags: u32,
        operating_system: BootElfOperatingSystem,
        entry: Address,
    ) -> Self {
        Self {
            class,
            machine,
            os_abi,
            flags,
            architecture: BootElfArchitecture::from_machine(class, machine, entry),
            operating_system,
        }
    }

    pub const fn class(&self) -> BootElfClass {
        self.class
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
}

pub(crate) fn parse_elf(bytes: &[u8]) -> Result<BootImage, BootError> {
    match detect_elf_class(bytes)? {
        ELF_CLASS_32 => parse_elf32_le(bytes),
        ELF_CLASS_64 => parse_elf64_le(bytes),
        class => Err(invalid_elf(BootElfError::UnsupportedClass { class })),
    }
}

pub(crate) fn parse_elf64_le(bytes: &[u8]) -> Result<BootImage, BootError> {
    validate_elf_ident(bytes, ELF_CLASS_64)?;
    let header_size = read_u16(bytes, 52)?;
    if header_size as usize != ELF64_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedHeaderSize {
            expected: ELF64_HEADER_SIZE as u16,
            actual: header_size,
        }));
    }

    let program_header_size = read_u16(bytes, 54)?;
    if program_header_size != ELF64_PROGRAM_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedProgramHeaderSize {
            expected: ELF64_PROGRAM_HEADER_SIZE,
            actual: program_header_size,
        }));
    }

    let os_abi = bytes[7];
    let machine = read_u16(bytes, 18)?;
    let flags = read_u32(bytes, 48)?;
    let operating_system =
        detect_elf_operating_system(bytes, BootElfClass::Class64, machine, os_abi, flags)?;
    let entry = Address::new(read_u64(bytes, 24)?);
    let program_header_offset = read_u64(bytes, 32)?;
    let program_header_count = read_u16(bytes, 56)?;
    let table_size = (program_header_size as u64)
        .checked_mul(program_header_count as u64)
        .ok_or_else(|| {
            invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
                offset: program_header_offset,
                size: u64::MAX,
                image_size: bytes.len() as u64,
            })
        })?;
    checked_file_range(bytes, program_header_offset, table_size).map_err(|_| {
        invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
            offset: program_header_offset,
            size: table_size,
            image_size: bytes.len() as u64,
        })
    })?;

    let mut image = BootImage::new(entry).with_elf_metadata(BootElfMetadata::from_header(
        BootElfClass::Class64,
        machine,
        os_abi,
        flags,
        operating_system,
        entry,
    ));
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index as u64 * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset)?;
        if kind != PT_LOAD {
            continue;
        }

        let file_offset = read_u64_at_u64(bytes, header_offset + 8)?;
        let physical = read_u64_at_u64(bytes, header_offset + 24)?;
        let file_size = read_u64_at_u64(bytes, header_offset + 32)?;
        let memory_size = read_u64_at_u64(bytes, header_offset + 40)?;
        if memory_size == 0 {
            continue;
        }
        if file_size > memory_size {
            return Err(invalid_elf(
                BootElfError::SegmentFileSizeExceedsMemorySize {
                    segment: index,
                    file_size,
                    memory_size,
                },
            ));
        }

        let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;
        let memory_len = usize::try_from(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
                segment: index,
                memory_size,
            })
        })?;
        let memory_access_size = AccessSize::new(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        AddressRange::new(Address::new(physical), memory_access_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        let file_len = usize::try_from(file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;

        let mut data = vec![0; memory_len];
        data[..file_len].copy_from_slice(file_range);
        image = image.add_segment(Address::new(physical), data)?;
        loaded_segments += 1;
    }

    if loaded_segments == 0 {
        return Err(invalid_elf(BootElfError::NoLoadableSegments));
    }

    Ok(image)
}

pub(crate) fn parse_elf32_le(bytes: &[u8]) -> Result<BootImage, BootError> {
    validate_elf_ident(bytes, ELF_CLASS_32)?;
    let header_size = read_u16(bytes, 40)?;
    if header_size as usize != ELF32_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedHeaderSize {
            expected: ELF32_HEADER_SIZE as u16,
            actual: header_size,
        }));
    }

    let program_header_size = read_u16(bytes, 42)?;
    if program_header_size != ELF32_PROGRAM_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedProgramHeaderSize {
            expected: ELF32_PROGRAM_HEADER_SIZE,
            actual: program_header_size,
        }));
    }

    let os_abi = bytes[7];
    let machine = read_u16(bytes, 18)?;
    let flags = read_u32(bytes, 36)?;
    let operating_system =
        detect_elf_operating_system(bytes, BootElfClass::Class32, machine, os_abi, flags)?;
    let entry = Address::new(u64::from(read_u32(bytes, 24)?));
    let program_header_offset = u64::from(read_u32(bytes, 28)?);
    let program_header_count = read_u16(bytes, 44)?;
    let table_size = (program_header_size as u64)
        .checked_mul(program_header_count as u64)
        .ok_or_else(|| {
            invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
                offset: program_header_offset,
                size: u64::MAX,
                image_size: bytes.len() as u64,
            })
        })?;
    checked_file_range(bytes, program_header_offset, table_size).map_err(|_| {
        invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
            offset: program_header_offset,
            size: table_size,
            image_size: bytes.len() as u64,
        })
    })?;

    let mut image = BootImage::new(entry).with_elf_metadata(BootElfMetadata::from_header(
        BootElfClass::Class32,
        machine,
        os_abi,
        flags,
        operating_system,
        entry,
    ));
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index as u64 * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset)?;
        if kind != PT_LOAD {
            continue;
        }

        let file_offset = u64::from(read_u32_at_u64(bytes, header_offset + 4)?);
        let physical = u64::from(read_u32_at_u64(bytes, header_offset + 12)?);
        let file_size = u64::from(read_u32_at_u64(bytes, header_offset + 16)?);
        let memory_size = u64::from(read_u32_at_u64(bytes, header_offset + 20)?);
        if memory_size == 0 {
            continue;
        }
        if file_size > memory_size {
            return Err(invalid_elf(
                BootElfError::SegmentFileSizeExceedsMemorySize {
                    segment: index,
                    file_size,
                    memory_size,
                },
            ));
        }

        let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;
        let memory_end = physical.checked_add(memory_size).ok_or_else(|| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        if memory_end > u64::from(u32::MAX) + 1 {
            return Err(invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            }));
        }
        let memory_len = usize::try_from(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
                segment: index,
                memory_size,
            })
        })?;
        let memory_access_size = AccessSize::new(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        AddressRange::new(Address::new(physical), memory_access_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        let file_len = usize::try_from(file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;

        let mut data = vec![0; memory_len];
        data[..file_len].copy_from_slice(file_range);
        image = image.add_segment(Address::new(physical), data)?;
        loaded_segments += 1;
    }

    if loaded_segments == 0 {
        return Err(invalid_elf(BootElfError::NoLoadableSegments));
    }

    Ok(image)
}

fn validate_elf_ident(bytes: &[u8], expected_class: u8) -> Result<(), BootError> {
    let class = detect_elf_class(bytes)?;
    if class != expected_class {
        return Err(invalid_elf(BootElfError::UnsupportedClass { class }));
    }
    Ok(())
}

fn detect_elf_class(bytes: &[u8]) -> Result<u8, BootError> {
    let ident = read_exact(bytes, 0, 16)?;
    if &ident[0..4] != b"\x7fELF" {
        return Err(invalid_elf(BootElfError::BadMagic));
    }
    let class = ident[4];
    if !matches!(class, ELF_CLASS_32 | ELF_CLASS_64) {
        return Err(invalid_elf(BootElfError::UnsupportedClass { class }));
    }
    if ident[5] != ELF_DATA_LITTLE {
        return Err(invalid_elf(BootElfError::UnsupportedEncoding {
            encoding: ident[5],
        }));
    }
    if ident[6] != ELF_VERSION_CURRENT {
        return Err(invalid_elf(BootElfError::UnsupportedVersion {
            version: ident[6],
        }));
    }
    Ok(class)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ElfSectionHeader {
    name: u32,
    kind: u32,
    offset: u64,
    size: u64,
}

fn detect_elf_operating_system(
    bytes: &[u8],
    class: BootElfClass,
    machine: u16,
    os_abi: u8,
    flags: u32,
) -> Result<BootElfOperatingSystem, BootError> {
    let header_os = BootElfOperatingSystem::from_header(machine, os_abi, flags);
    if !matches!(header_os, BootElfOperatingSystem::Unknown { .. }) {
        return Ok(header_os);
    }

    Ok(detect_elf_operating_system_from_sections(bytes, class)?.unwrap_or(header_os))
}

fn detect_elf_operating_system_from_sections(
    bytes: &[u8],
    class: BootElfClass,
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    match class {
        BootElfClass::Class32 => detect_elf32_operating_system_from_sections(bytes),
        BootElfClass::Class64 => detect_elf64_operating_system_from_sections(bytes),
    }
}

fn detect_elf64_operating_system_from_sections(
    bytes: &[u8],
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    let section_table_offset = read_u64(bytes, 40)?;
    let section_header_size = read_u16(bytes, 58)?;
    let section_count = read_u16(bytes, 60)?;
    let string_section = read_u16(bytes, 62)?;
    if section_table_offset == 0
        || section_count == 0
        || string_section == 0
        || string_section >= section_count
        || section_header_size != ELF64_SECTION_HEADER_SIZE
    {
        return Ok(None);
    }

    validate_section_table_range(
        bytes,
        section_table_offset,
        section_header_size,
        section_count,
    )?;
    let string_header = read_elf64_section_header(
        bytes,
        section_table_offset,
        section_header_size,
        string_section,
    )?;
    let string_table = checked_file_range(bytes, string_header.offset, string_header.size)
        .map_err(|_| {
            invalid_elf(BootElfError::SectionDataRangeOutOfBounds {
                offset: string_header.offset,
                size: string_header.size,
                image_size: bytes.len() as u64,
            })
        })?;

    for index in 1..section_count {
        let section =
            read_elf64_section_header(bytes, section_table_offset, section_header_size, index)?;
        if let Some(operating_system) =
            detect_section_operating_system(bytes, string_table, section)?
        {
            return Ok(Some(operating_system));
        }
    }
    Ok(None)
}

fn detect_elf32_operating_system_from_sections(
    bytes: &[u8],
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    let section_table_offset = u64::from(read_u32(bytes, 32)?);
    let section_header_size = read_u16(bytes, 46)?;
    let section_count = read_u16(bytes, 48)?;
    let string_section = read_u16(bytes, 50)?;
    if section_table_offset == 0
        || section_count == 0
        || string_section == 0
        || string_section >= section_count
        || section_header_size != ELF32_SECTION_HEADER_SIZE
    {
        return Ok(None);
    }

    validate_section_table_range(
        bytes,
        section_table_offset,
        section_header_size,
        section_count,
    )?;
    let string_header = read_elf32_section_header(
        bytes,
        section_table_offset,
        section_header_size,
        string_section,
    )?;
    let string_table = checked_file_range(bytes, string_header.offset, string_header.size)
        .map_err(|_| {
            invalid_elf(BootElfError::SectionDataRangeOutOfBounds {
                offset: string_header.offset,
                size: string_header.size,
                image_size: bytes.len() as u64,
            })
        })?;

    for index in 1..section_count {
        let section =
            read_elf32_section_header(bytes, section_table_offset, section_header_size, index)?;
        if let Some(operating_system) =
            detect_section_operating_system(bytes, string_table, section)?
        {
            return Ok(Some(operating_system));
        }
    }
    Ok(None)
}

fn validate_section_table_range(
    bytes: &[u8],
    offset: u64,
    header_size: u16,
    count: u16,
) -> Result<(), BootError> {
    let size = u64::from(header_size)
        .checked_mul(u64::from(count))
        .ok_or_else(|| {
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
    index: u16,
) -> Result<ElfSectionHeader, BootError> {
    let base = table_offset + u64::from(index) * u64::from(header_size);
    Ok(ElfSectionHeader {
        name: read_u32_at_u64(bytes, base)?,
        kind: read_u32_at_u64(bytes, base + 4)?,
        offset: read_u64_at_u64(bytes, base + 24)?,
        size: read_u64_at_u64(bytes, base + 32)?,
    })
}

fn read_elf32_section_header(
    bytes: &[u8],
    table_offset: u64,
    header_size: u16,
    index: u16,
) -> Result<ElfSectionHeader, BootError> {
    let base = table_offset + u64::from(index) * u64::from(header_size);
    Ok(ElfSectionHeader {
        name: read_u32_at_u64(bytes, base)?,
        kind: read_u32_at_u64(bytes, base + 4)?,
        offset: u64::from(read_u32_at_u64(bytes, base + 16)?),
        size: u64::from(read_u32_at_u64(bytes, base + 20)?),
    })
}

fn detect_section_operating_system(
    bytes: &[u8],
    string_table: &[u8],
    section: ElfSectionHeader,
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
            return Ok(
                match u32::from_le_bytes([
                    section_data[16],
                    section_data[17],
                    section_data[18],
                    section_data[19],
                ]) {
                    0 => Some(BootElfOperatingSystem::Linux),
                    2 => Some(BootElfOperatingSystem::Solaris),
                    3 => Some(BootElfOperatingSystem::FreeBsd),
                    _ => None,
                },
            );
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

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, BootError> {
    let data = read_exact(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, BootError> {
    let data = read_exact(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

fn read_u32_at_u64(bytes: &[u8], offset: u64) -> Result<u32, BootError> {
    let offset = usize::try_from(offset).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: 4,
            image_size: bytes.len() as u64,
        })
    })?;
    let data = read_exact(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, BootError> {
    let data = read_exact(bytes, offset, 8)?;
    Ok(u64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]))
}

fn read_u64_at_u64(bytes: &[u8], offset: u64) -> Result<u64, BootError> {
    let offset = usize::try_from(offset).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: 8,
            image_size: bytes.len() as u64,
        })
    })?;
    read_u64(bytes, offset)
}

fn read_exact(bytes: &[u8], offset: usize, size: usize) -> Result<&[u8], BootError> {
    bytes
        .get(offset..offset.saturating_add(size))
        .ok_or_else(|| {
            invalid_elf(BootElfError::TruncatedField {
                offset: offset as u64,
                size: size as u64,
                image_size: bytes.len() as u64,
            })
        })
}

fn checked_file_range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], ()> {
    let end = offset.checked_add(size).ok_or(())?;
    let start = usize::try_from(offset).map_err(|_| ())?;
    let end = usize::try_from(end).map_err(|_| ())?;
    bytes.get(start..end).ok_or(())
}
