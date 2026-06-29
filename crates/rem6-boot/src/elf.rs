use rem6_memory::{AccessSize, Address, AddressRange};

use crate::elf_counts::{program_header_table_size, resolve_program_header_count};
use crate::elf_dynamic::{elf32_load_mappings, elf64_load_mappings};
use crate::elf_program_headers::{
    summarize_elf32_program_header, summarize_elf64_program_header, ElfProgramHeaderAction,
    ElfProgramHeaderMetadata,
};
use crate::elf_sections::{elf_section_summary, ElfSectionSummary};
use crate::error::{invalid_elf, BootElfError, BootError};
use crate::image::BootImage;
use crate::metadata::{BootElfMetadata, BootElfProgramHeaderTable};

const ELF64_HEADER_SIZE: usize = 64;
const ELF64_PROGRAM_HEADER_SIZE: u16 = 56;
const ELF32_HEADER_SIZE: usize = 52;
const ELF32_PROGRAM_HEADER_SIZE: u16 = 32;
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
const MAX_VEC_BYTES: usize = isize::MAX as usize;

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
    let header_os = BootElfOperatingSystem::from_header(machine, endian, os_abi, flags);
    let section_summary = section_summary(bytes, BootElfClass::Class64, endian, header_os)?;
    let operating_system =
        detect_elf_operating_system(header_os, section_summary.operating_system());
    let entry = Address::new(read_u64(bytes, 24, endian)?);
    let program_header_offset = read_u64(bytes, 32, endian)?;
    let program_header_count = resolve_program_header_count(
        bytes,
        BootElfClass::Class64,
        endian,
        read_u16(bytes, 56, endian)?,
    )?;
    let table_size = program_header_table_size(
        bytes,
        program_header_offset,
        program_header_size,
        program_header_count,
    )?;
    let load_mappings = elf64_load_mappings(
        bytes,
        program_header_offset,
        program_header_size,
        program_header_count,
        endian,
    )?;

    let mut image = BootImage::new(entry);
    let mut header_metadata = ElfProgramHeaderMetadata::new(section_summary.has_tls());
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let segment = segment_index(index);
        let header_offset = program_header_offset + index * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset, endian)?;
        if summarize_elf64_program_header(
            bytes,
            segment,
            header_offset,
            kind,
            endian,
            &load_mappings,
            &mut header_metadata,
        )? == ElfProgramHeaderAction::Skip
        {
            continue;
        }
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
                    segment,
                    file_size,
                    memory_size,
                },
            ));
        }

        let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;
        let memory_access_size = AccessSize::new(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            })
        })?;
        AddressRange::new(Address::new(physical), memory_access_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            })
        })?;
        header_metadata.record_inferred_program_header_address(loaded_file_address(
            program_header_offset,
            table_size,
            file_offset,
            file_size,
            physical,
        ));
        let data = zeroed_segment_data(segment, memory_size, file_range)?;
        image = image.add_segment(Address::new(physical), data)?;
        loaded_segments += 1;
    }

    if loaded_segments == 0 {
        return Err(invalid_elf(BootElfError::NoLoadableSegments));
    }

    let program_header_memory_address = header_metadata.program_header_memory_address();
    let has_tls = header_metadata.has_tls;
    let note_segment_count = header_metadata.note_segment_count;
    let note_file_size = header_metadata.note_file_size;
    let gnu_stack_executable = header_metadata.gnu_stack_executable;
    let gnu_relro_virtual_address = header_metadata.gnu_relro_virtual_address;
    let gnu_relro_memory_size = header_metadata.gnu_relro_memory_size;
    let gnu_eh_frame_virtual_address = header_metadata.gnu_eh_frame_virtual_address;
    let gnu_eh_frame_memory_size = header_metadata.gnu_eh_frame_memory_size;
    let gnu_property_virtual_address = header_metadata.gnu_property_virtual_address;
    let gnu_property_memory_size = header_metadata.gnu_property_memory_size;
    let dynamic_table = header_metadata.dynamic_table;
    let interpreter = header_metadata.interpreter;

    let image = image.with_elf_metadata(
        BootElfMetadata::from_header(
            BootElfClass::Class64,
            endian,
            machine,
            os_abi,
            flags,
            BootElfArchitecture::from_machine(BootElfClass::Class64, machine, entry),
            operating_system,
        )
        .with_tls(has_tls)
        .with_note_segments(note_segment_count, note_file_size)
        .with_gnu_stack_executable(gnu_stack_executable)
        .with_gnu_relro(gnu_relro_virtual_address, gnu_relro_memory_size)
        .with_gnu_eh_frame(gnu_eh_frame_virtual_address, gnu_eh_frame_memory_size)
        .with_gnu_property(gnu_property_virtual_address, gnu_property_memory_size)
        .with_symbol_summary(
            section_summary.symbol_count(),
            section_summary.function_symbol_count(),
            section_summary.object_symbol_count(),
        )
        .with_dynamic_table(dynamic_table)
        .with_program_header_table(
            BootElfProgramHeaderTable::new(
                program_header_offset,
                program_header_size,
                program_header_count,
            )
            .with_memory_address(program_header_memory_address),
        ),
    );
    Ok(match interpreter {
        Some(interpreter) => image.with_elf_interpreter(interpreter),
        None => image,
    })
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
    let header_os = BootElfOperatingSystem::from_header(machine, endian, os_abi, flags);
    let section_summary = section_summary(bytes, BootElfClass::Class32, endian, header_os)?;
    let operating_system =
        detect_elf_operating_system(header_os, section_summary.operating_system());
    let entry = Address::new(u64::from(read_u32(bytes, 24, endian)?));
    let program_header_offset = u64::from(read_u32(bytes, 28, endian)?);
    let program_header_count = resolve_program_header_count(
        bytes,
        BootElfClass::Class32,
        endian,
        read_u16(bytes, 44, endian)?,
    )?;
    let table_size = program_header_table_size(
        bytes,
        program_header_offset,
        program_header_size,
        program_header_count,
    )?;
    let load_mappings = elf32_load_mappings(
        bytes,
        program_header_offset,
        program_header_size,
        program_header_count,
        endian,
    )?;

    let mut image = BootImage::new(entry);
    let mut header_metadata = ElfProgramHeaderMetadata::new(section_summary.has_tls());
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let segment = segment_index(index);
        let header_offset = program_header_offset + index * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset, endian)?;
        if summarize_elf32_program_header(
            bytes,
            segment,
            header_offset,
            kind,
            endian,
            &load_mappings,
            &mut header_metadata,
        )? == ElfProgramHeaderAction::Skip
        {
            continue;
        }
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
                    segment,
                    file_size,
                    memory_size,
                },
            ));
        }

        let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;
        let memory_end = physical.checked_add(memory_size).ok_or_else(|| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            })
        })?;
        if memory_end > u64::from(u32::MAX) + 1 {
            return Err(invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            }));
        }
        let memory_access_size = AccessSize::new(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            })
        })?;
        AddressRange::new(Address::new(physical), memory_access_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            })
        })?;
        header_metadata.record_inferred_program_header_address(loaded_file_address(
            program_header_offset,
            table_size,
            file_offset,
            file_size,
            physical,
        ));
        let data = zeroed_segment_data(segment, memory_size, file_range)?;
        image = image.add_segment(Address::new(physical), data)?;
        loaded_segments += 1;
    }

    if loaded_segments == 0 {
        return Err(invalid_elf(BootElfError::NoLoadableSegments));
    }

    let program_header_memory_address = header_metadata.program_header_memory_address();
    let has_tls = header_metadata.has_tls;
    let note_segment_count = header_metadata.note_segment_count;
    let note_file_size = header_metadata.note_file_size;
    let gnu_stack_executable = header_metadata.gnu_stack_executable;
    let gnu_relro_virtual_address = header_metadata.gnu_relro_virtual_address;
    let gnu_relro_memory_size = header_metadata.gnu_relro_memory_size;
    let gnu_eh_frame_virtual_address = header_metadata.gnu_eh_frame_virtual_address;
    let gnu_eh_frame_memory_size = header_metadata.gnu_eh_frame_memory_size;
    let gnu_property_virtual_address = header_metadata.gnu_property_virtual_address;
    let gnu_property_memory_size = header_metadata.gnu_property_memory_size;
    let dynamic_table = header_metadata.dynamic_table;
    let interpreter = header_metadata.interpreter;

    let image = image.with_elf_metadata(
        BootElfMetadata::from_header(
            BootElfClass::Class32,
            endian,
            machine,
            os_abi,
            flags,
            BootElfArchitecture::from_machine(BootElfClass::Class32, machine, entry),
            operating_system,
        )
        .with_tls(has_tls)
        .with_note_segments(note_segment_count, note_file_size)
        .with_gnu_stack_executable(gnu_stack_executable)
        .with_gnu_relro(gnu_relro_virtual_address, gnu_relro_memory_size)
        .with_gnu_eh_frame(gnu_eh_frame_virtual_address, gnu_eh_frame_memory_size)
        .with_gnu_property(gnu_property_virtual_address, gnu_property_memory_size)
        .with_symbol_summary(
            section_summary.symbol_count(),
            section_summary.function_symbol_count(),
            section_summary.object_symbol_count(),
        )
        .with_dynamic_table(dynamic_table)
        .with_program_header_table(
            BootElfProgramHeaderTable::new(
                program_header_offset,
                program_header_size,
                program_header_count,
            )
            .with_memory_address(program_header_memory_address),
        ),
    );
    Ok(match interpreter {
        Some(interpreter) => image.with_elf_interpreter(interpreter),
        None => image,
    })
}

fn segment_index(index: u64) -> u16 {
    u16::try_from(index).unwrap_or(u16::MAX)
}

fn loaded_file_address(
    table_offset: u64,
    table_size: u64,
    segment_offset: u64,
    segment_file_size: u64,
    segment_loaded_start: u64,
) -> Option<Address> {
    if table_size == 0 || table_offset < segment_offset {
        return None;
    }
    let table_end = table_offset.checked_add(table_size)?;
    let segment_end = segment_offset.checked_add(segment_file_size)?;
    if table_end > segment_end {
        return None;
    }
    let delta = table_offset - segment_offset;
    Some(Address::new(segment_loaded_start.checked_add(delta)?))
}

fn zeroed_segment_data(
    segment: u16,
    memory_size: u64,
    file_range: &[u8],
) -> Result<Vec<u8>, BootError> {
    let memory_len = segment_memory_len(segment, memory_size)?;
    let mut data = Vec::new();
    data.try_reserve_exact(memory_len).map_err(|_| {
        invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
            segment,
            memory_size,
        })
    })?;
    data.resize(memory_len, 0);
    data[..file_range.len()].copy_from_slice(file_range);
    Ok(data)
}

fn segment_memory_len(segment: u16, memory_size: u64) -> Result<usize, BootError> {
    let memory_len = usize::try_from(memory_size).map_err(|_| {
        invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
            segment,
            memory_size,
        })
    })?;
    if memory_len > MAX_VEC_BYTES {
        return Err(invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
            segment,
            memory_size,
        }));
    }
    Ok(memory_len)
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

fn detect_elf_operating_system(
    header_os: BootElfOperatingSystem,
    section_os: Option<BootElfOperatingSystem>,
) -> BootElfOperatingSystem {
    if !matches!(header_os, BootElfOperatingSystem::Unknown { .. }) {
        return header_os;
    }
    section_os.unwrap_or(header_os)
}

fn section_summary(
    bytes: &[u8],
    class: BootElfClass,
    endian: BootElfEndian,
    header_os: BootElfOperatingSystem,
) -> Result<ElfSectionSummary, BootError> {
    let detect_operating_system = matches!(header_os, BootElfOperatingSystem::Unknown { .. });
    match elf_section_summary(bytes, class, endian, detect_operating_system) {
        Ok(summary) => Ok(summary),
        Err(error) if detect_operating_system => Err(error),
        Err(_) => Ok(ElfSectionSummary::default()),
    }
}

fn read_u16(bytes: &[u8], offset: usize, endian: BootElfEndian) -> Result<u16, BootError> {
    let data = read_exact(bytes, offset, 2)?;
    Ok(endian.read_u16([data[0], data[1]]))
}

fn read_u32(bytes: &[u8], offset: usize, endian: BootElfEndian) -> Result<u32, BootError> {
    let data = read_exact(bytes, offset, 4)?;
    Ok(endian.read_u32([data[0], data[1], data[2], data[3]]))
}

pub(crate) fn read_u32_at_u64(
    bytes: &[u8],
    offset: u64,
    endian: BootElfEndian,
) -> Result<u32, BootError> {
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

pub(crate) fn read_u64_at_u64(
    bytes: &[u8],
    offset: u64,
    endian: BootElfEndian,
) -> Result<u64, BootError> {
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

pub(crate) fn checked_file_range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], ()> {
    let end = offset.checked_add(size).ok_or(())?;
    let start = usize::try_from(offset).map_err(|_| ())?;
    let end = usize::try_from(end).map_err(|_| ())?;
    bytes.get(start..end).ok_or(())
}
