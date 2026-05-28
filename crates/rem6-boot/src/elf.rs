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
const ELF_DATA_BIG: u8 = 2;
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
pub enum BootElfEndian {
    Little,
    Big,
}

impl BootElfEndian {
    const fn from_encoding(encoding: u8) -> Option<Self> {
        match encoding {
            ELF_DATA_LITTLE => Some(Self::Little),
            ELF_DATA_BIG => Some(Self::Big),
            _ => None,
        }
    }

    const fn encoding(self) -> u8 {
        match self {
            Self::Little => ELF_DATA_LITTLE,
            Self::Big => ELF_DATA_BIG,
        }
    }

    const fn read_u16(self, data: [u8; 2]) -> u16 {
        match self {
            Self::Little => u16::from_le_bytes(data),
            Self::Big => u16::from_be_bytes(data),
        }
    }

    const fn read_u32(self, data: [u8; 4]) -> u32 {
        match self {
            Self::Little => u32::from_le_bytes(data),
            Self::Big => u32::from_be_bytes(data),
        }
    }

    const fn read_u64(self, data: [u8; 8]) -> u64 {
        match self {
            Self::Little => u64::from_le_bytes(data),
            Self::Big => u64::from_be_bytes(data),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ElfIdent {
    class: u8,
    endian: BootElfEndian,
}

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
    const fn from_header(machine: u16, endian: BootElfEndian, os_abi: u8, flags: u32) -> Self {
        if machine == EM_PPC64 {
            return match flags & 0x3 {
                0x1 => Self::LinuxPower64AbiV1,
                0x2 => Self::LinuxPower64AbiV2,
                _ => match endian {
                    BootElfEndian::Little => Self::LinuxPower64AbiV2,
                    BootElfEndian::Big => Self::LinuxPower64AbiV1,
                },
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
    endian: BootElfEndian,
    machine: u16,
    os_abi: u8,
    flags: u32,
    architecture: BootElfArchitecture,
    operating_system: BootElfOperatingSystem,
}

impl BootElfMetadata {
    const fn from_header(
        class: BootElfClass,
        endian: BootElfEndian,
        machine: u16,
        os_abi: u8,
        flags: u32,
        operating_system: BootElfOperatingSystem,
        entry: Address,
    ) -> Self {
        Self {
            class,
            endian,
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

    pub const fn endian(&self) -> BootElfEndian {
        self.endian
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
    let ident = detect_elf_ident(bytes)?;
    match ident.class {
        ELF_CLASS_32 => parse_elf32(bytes, ident.endian),
        ELF_CLASS_64 => parse_elf64(bytes, ident.endian),
        class => Err(invalid_elf(BootElfError::UnsupportedClass { class })),
    }
}

pub(crate) fn parse_elf64_le(bytes: &[u8]) -> Result<BootImage, BootError> {
    parse_elf64(bytes, BootElfEndian::Little)
}

fn parse_elf64(bytes: &[u8], endian: BootElfEndian) -> Result<BootImage, BootError> {
    validate_elf_ident(bytes, ELF_CLASS_64, endian)?;
    let header_size = read_u16(bytes, 52, endian)?;
    if header_size as usize != ELF64_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedHeaderSize {
            expected: ELF64_HEADER_SIZE as u16,
            actual: header_size,
        }));
    }

    let program_header_size = read_u16(bytes, 54, endian)?;
    if program_header_size != ELF64_PROGRAM_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedProgramHeaderSize {
            expected: ELF64_PROGRAM_HEADER_SIZE,
            actual: program_header_size,
        }));
    }

    let os_abi = bytes[7];
    let machine = read_u16(bytes, 18, endian)?;
    let flags = read_u32(bytes, 48, endian)?;
    let operating_system =
        detect_elf_operating_system(bytes, BootElfClass::Class64, endian, machine, os_abi, flags)?;
    let entry = Address::new(read_u64(bytes, 24, endian)?);
    let program_header_offset = read_u64(bytes, 32, endian)?;
    let program_header_count = read_u16(bytes, 56, endian)?;
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
        endian,
        machine,
        os_abi,
        flags,
        operating_system,
        entry,
    ));
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index as u64 * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset, endian)?;
        if kind != PT_LOAD {
            continue;
        }

        let file_offset = read_u64_at_u64(bytes, header_offset + 8, endian)?;
        let physical = read_u64_at_u64(bytes, header_offset + 24, endian)?;
        let file_size = read_u64_at_u64(bytes, header_offset + 32, endian)?;
        let memory_size = read_u64_at_u64(bytes, header_offset + 40, endian)?;
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
    parse_elf32(bytes, BootElfEndian::Little)
}

fn parse_elf32(bytes: &[u8], endian: BootElfEndian) -> Result<BootImage, BootError> {
    validate_elf_ident(bytes, ELF_CLASS_32, endian)?;
    let header_size = read_u16(bytes, 40, endian)?;
    if header_size as usize != ELF32_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedHeaderSize {
            expected: ELF32_HEADER_SIZE as u16,
            actual: header_size,
        }));
    }

    let program_header_size = read_u16(bytes, 42, endian)?;
    if program_header_size != ELF32_PROGRAM_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedProgramHeaderSize {
            expected: ELF32_PROGRAM_HEADER_SIZE,
            actual: program_header_size,
        }));
    }

    let os_abi = bytes[7];
    let machine = read_u16(bytes, 18, endian)?;
    let flags = read_u32(bytes, 36, endian)?;
    let operating_system =
        detect_elf_operating_system(bytes, BootElfClass::Class32, endian, machine, os_abi, flags)?;
    let entry = Address::new(u64::from(read_u32(bytes, 24, endian)?));
    let program_header_offset = u64::from(read_u32(bytes, 28, endian)?);
    let program_header_count = read_u16(bytes, 44, endian)?;
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
        endian,
        machine,
        os_abi,
        flags,
        operating_system,
        entry,
    ));
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index as u64 * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset, endian)?;
        if kind != PT_LOAD {
            continue;
        }

        let file_offset = u64::from(read_u32_at_u64(bytes, header_offset + 4, endian)?);
        let physical = u64::from(read_u32_at_u64(bytes, header_offset + 12, endian)?);
        let file_size = u64::from(read_u32_at_u64(bytes, header_offset + 16, endian)?);
        let memory_size = u64::from(read_u32_at_u64(bytes, header_offset + 20, endian)?);
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

fn validate_elf_ident(
    bytes: &[u8],
    expected_class: u8,
    expected_endian: BootElfEndian,
) -> Result<(), BootError> {
    let ident = detect_elf_ident(bytes)?;
    if ident.class != expected_class {
        return Err(invalid_elf(BootElfError::UnsupportedClass {
            class: ident.class,
        }));
    }
    if ident.endian != expected_endian {
        return Err(invalid_elf(BootElfError::UnsupportedEncoding {
            encoding: ident.endian.encoding(),
        }));
    }
    Ok(())
}

fn detect_elf_ident(bytes: &[u8]) -> Result<ElfIdent, BootError> {
    let ident = read_exact(bytes, 0, 16)?;
    if &ident[0..4] != b"\x7fELF" {
        return Err(invalid_elf(BootElfError::BadMagic));
    }
    let class = ident[4];
    if !matches!(class, ELF_CLASS_32 | ELF_CLASS_64) {
        return Err(invalid_elf(BootElfError::UnsupportedClass { class }));
    }
    let Some(endian) = BootElfEndian::from_encoding(ident[5]) else {
        return Err(invalid_elf(BootElfError::UnsupportedEncoding {
            encoding: ident[5],
        }));
    };
    if ident[6] != ELF_VERSION_CURRENT {
        return Err(invalid_elf(BootElfError::UnsupportedVersion {
            version: ident[6],
        }));
    }
    Ok(ElfIdent { class, endian })
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
    endian: BootElfEndian,
    machine: u16,
    os_abi: u8,
    flags: u32,
) -> Result<BootElfOperatingSystem, BootError> {
    let header_os = BootElfOperatingSystem::from_header(machine, endian, os_abi, flags);
    if !matches!(header_os, BootElfOperatingSystem::Unknown { .. }) {
        return Ok(header_os);
    }

    Ok(detect_elf_operating_system_from_sections(bytes, class, endian)?.unwrap_or(header_os))
}

fn detect_elf_operating_system_from_sections(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    match class {
        BootElfClass::Class32 => detect_elf32_operating_system_from_sections(bytes, endian),
        BootElfClass::Class64 => detect_elf64_operating_system_from_sections(bytes, endian),
    }
}

fn detect_elf64_operating_system_from_sections(
    bytes: &[u8],
    endian: BootElfEndian,
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    let section_table_offset = read_u64(bytes, 40, endian)?;
    let section_header_size = read_u16(bytes, 58, endian)?;
    let section_count = read_u16(bytes, 60, endian)?;
    let string_section = read_u16(bytes, 62, endian)?;
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

    for index in 1..section_count {
        let section = read_elf64_section_header(
            bytes,
            section_table_offset,
            section_header_size,
            index,
            endian,
        )?;
        if let Some(operating_system) =
            detect_section_operating_system(bytes, string_table, section, endian)?
        {
            return Ok(Some(operating_system));
        }
    }
    Ok(None)
}

fn detect_elf32_operating_system_from_sections(
    bytes: &[u8],
    endian: BootElfEndian,
) -> Result<Option<BootElfOperatingSystem>, BootError> {
    let section_table_offset = u64::from(read_u32(bytes, 32, endian)?);
    let section_header_size = read_u16(bytes, 46, endian)?;
    let section_count = read_u16(bytes, 48, endian)?;
    let string_section = read_u16(bytes, 50, endian)?;
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

    for index in 1..section_count {
        let section = read_elf32_section_header(
            bytes,
            section_table_offset,
            section_header_size,
            index,
            endian,
        )?;
        if let Some(operating_system) =
            detect_section_operating_system(bytes, string_table, section, endian)?
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
    endian: BootElfEndian,
) -> Result<ElfSectionHeader, BootError> {
    let base = table_offset + u64::from(index) * u64::from(header_size);
    Ok(ElfSectionHeader {
        name: read_u32_at_u64(bytes, base, endian)?,
        kind: read_u32_at_u64(bytes, base + 4, endian)?,
        offset: read_u64_at_u64(bytes, base + 24, endian)?,
        size: read_u64_at_u64(bytes, base + 32, endian)?,
    })
}

fn read_elf32_section_header(
    bytes: &[u8],
    table_offset: u64,
    header_size: u16,
    index: u16,
    endian: BootElfEndian,
) -> Result<ElfSectionHeader, BootError> {
    let base = table_offset + u64::from(index) * u64::from(header_size);
    Ok(ElfSectionHeader {
        name: read_u32_at_u64(bytes, base, endian)?,
        kind: read_u32_at_u64(bytes, base + 4, endian)?,
        offset: u64::from(read_u32_at_u64(bytes, base + 16, endian)?),
        size: u64::from(read_u32_at_u64(bytes, base + 20, endian)?),
    })
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
            let os = endian.read_u32([
                section_data[16],
                section_data[17],
                section_data[18],
                section_data[19],
            ]);
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

fn read_u16(bytes: &[u8], offset: usize, endian: BootElfEndian) -> Result<u16, BootError> {
    let data = read_exact(bytes, offset, 2)?;
    Ok(endian.read_u16([data[0], data[1]]))
}

fn read_u32(bytes: &[u8], offset: usize, endian: BootElfEndian) -> Result<u32, BootError> {
    let data = read_exact(bytes, offset, 4)?;
    Ok(endian.read_u32([data[0], data[1], data[2], data[3]]))
}

fn read_u32_at_u64(bytes: &[u8], offset: u64, endian: BootElfEndian) -> Result<u32, BootError> {
    let offset = usize::try_from(offset).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: 4,
            image_size: bytes.len() as u64,
        })
    })?;
    let data = read_exact(bytes, offset, 4)?;
    Ok(endian.read_u32([data[0], data[1], data[2], data[3]]))
}

fn read_u64(bytes: &[u8], offset: usize, endian: BootElfEndian) -> Result<u64, BootError> {
    let data = read_exact(bytes, offset, 8)?;
    Ok(endian.read_u64([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]))
}

fn read_u64_at_u64(bytes: &[u8], offset: u64, endian: BootElfEndian) -> Result<u64, BootError> {
    let offset = usize::try_from(offset).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: 8,
            image_size: bytes.len() as u64,
        })
    })?;
    read_u64(bytes, offset, endian)
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
