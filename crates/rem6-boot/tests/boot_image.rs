use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfError, BootElfOperatingSystem,
    BootError, BootImage, BootLineWrite, BootLoadReport,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, LineMemoryStore, MemoryError,
    MemoryTargetId, PartitionedMemoryStore,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn line(fill: u8) -> Vec<u8> {
    vec![fill; 16]
}

const OVERSIZED_VECTOR_LENGTH: u64 = isize::MAX as u64 + 1;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_u16_be(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn write_u64_be(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}

#[derive(Clone, Copy)]
struct ElfProgramHeaderSpec {
    kind: u32,
    offset: u64,
    physical: u64,
    file_size: u64,
    memory_size: u64,
}

#[derive(Clone, Copy)]
struct ElfSectionSpec<'a> {
    name: &'a str,
    kind: u32,
    data: &'a [u8],
}

fn elf64_image(entry: u64, headers: &[ElfProgramHeaderSpec], data: &[(usize, &[u8])]) -> Vec<u8> {
    let mut size = 64 + headers.len() * 56;
    for (offset, bytes) in data {
        size = size.max(offset + bytes.len());
    }
    let mut bytes = vec![0; size];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, 243);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, entry);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, headers.len() as u16);

    for (index, header) in headers.iter().enumerate() {
        let base = 64 + index * 56;
        write_u32(&mut bytes, base, header.kind);
        write_u32(&mut bytes, base + 4, 5);
        write_u64(&mut bytes, base + 8, header.offset);
        write_u64(&mut bytes, base + 16, header.physical);
        write_u64(&mut bytes, base + 24, header.physical);
        write_u64(&mut bytes, base + 32, header.file_size);
        write_u64(&mut bytes, base + 40, header.memory_size);
        write_u64(&mut bytes, base + 48, 0x1000);
    }

    for (offset, payload) in data {
        bytes[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    bytes
}

fn elf32_image(entry: u32, headers: &[ElfProgramHeaderSpec], data: &[(usize, &[u8])]) -> Vec<u8> {
    let mut size = 52 + headers.len() * 32;
    for (offset, bytes) in data {
        size = size.max(offset + bytes.len());
    }
    let mut bytes = vec![0; size];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 1;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, 243);
    write_u32(&mut bytes, 20, 1);
    write_u32(&mut bytes, 24, entry);
    write_u32(&mut bytes, 28, 52);
    write_u16(&mut bytes, 40, 52);
    write_u16(&mut bytes, 42, 32);
    write_u16(&mut bytes, 44, headers.len() as u16);

    for (index, header) in headers.iter().enumerate() {
        let base = 52 + index * 32;
        write_u32(&mut bytes, base, header.kind);
        write_u32(&mut bytes, base + 4, header.offset as u32);
        write_u32(&mut bytes, base + 8, header.physical as u32);
        write_u32(&mut bytes, base + 12, header.physical as u32);
        write_u32(&mut bytes, base + 16, header.file_size as u32);
        write_u32(&mut bytes, base + 20, header.memory_size as u32);
        write_u32(&mut bytes, base + 24, 5);
        write_u32(&mut bytes, base + 28, 0x1000);
    }

    for (offset, payload) in data {
        bytes[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    bytes
}

fn elf64_be_image(
    entry: u64,
    machine: u16,
    headers: &[ElfProgramHeaderSpec],
    data: &[(usize, &[u8])],
) -> Vec<u8> {
    let mut size = 64 + headers.len() * 56;
    for (offset, bytes) in data {
        size = size.max(offset + bytes.len());
    }
    let mut bytes = vec![0; size];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 2;
    bytes[6] = 1;
    write_u16_be(&mut bytes, 16, 2);
    write_u16_be(&mut bytes, 18, machine);
    write_u32_be(&mut bytes, 20, 1);
    write_u64_be(&mut bytes, 24, entry);
    write_u64_be(&mut bytes, 32, 64);
    write_u16_be(&mut bytes, 52, 64);
    write_u16_be(&mut bytes, 54, 56);
    write_u16_be(&mut bytes, 56, headers.len() as u16);

    for (index, header) in headers.iter().enumerate() {
        let base = 64 + index * 56;
        write_u32_be(&mut bytes, base, header.kind);
        write_u32_be(&mut bytes, base + 4, 5);
        write_u64_be(&mut bytes, base + 8, header.offset);
        write_u64_be(&mut bytes, base + 16, header.physical);
        write_u64_be(&mut bytes, base + 24, header.physical);
        write_u64_be(&mut bytes, base + 32, header.file_size);
        write_u64_be(&mut bytes, base + 40, header.memory_size);
        write_u64_be(&mut bytes, base + 48, 0x1000);
    }

    for (offset, payload) in data {
        bytes[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    bytes
}

fn elf32_be_image(
    entry: u32,
    machine: u16,
    headers: &[ElfProgramHeaderSpec],
    data: &[(usize, &[u8])],
) -> Vec<u8> {
    let mut size = 52 + headers.len() * 32;
    for (offset, bytes) in data {
        size = size.max(offset + bytes.len());
    }
    let mut bytes = vec![0; size];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 1;
    bytes[5] = 2;
    bytes[6] = 1;
    write_u16_be(&mut bytes, 16, 2);
    write_u16_be(&mut bytes, 18, machine);
    write_u32_be(&mut bytes, 20, 1);
    write_u32_be(&mut bytes, 24, entry);
    write_u32_be(&mut bytes, 28, 52);
    write_u16_be(&mut bytes, 40, 52);
    write_u16_be(&mut bytes, 42, 32);
    write_u16_be(&mut bytes, 44, headers.len() as u16);

    for (index, header) in headers.iter().enumerate() {
        let base = 52 + index * 32;
        write_u32_be(&mut bytes, base, header.kind);
        write_u32_be(&mut bytes, base + 4, header.offset as u32);
        write_u32_be(&mut bytes, base + 8, header.physical as u32);
        write_u32_be(&mut bytes, base + 12, header.physical as u32);
        write_u32_be(&mut bytes, base + 16, header.file_size as u32);
        write_u32_be(&mut bytes, base + 20, header.memory_size as u32);
        write_u32_be(&mut bytes, base + 24, 5);
        write_u32_be(&mut bytes, base + 28, 0x1000);
    }

    for (offset, payload) in data {
        bytes[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    bytes
}

fn add_elf64_sections(bytes: &mut Vec<u8>, sections: &[ElfSectionSpec<'_>]) {
    let mut name_data = vec![0];
    let mut name_offsets = Vec::new();
    for section in sections {
        name_offsets.push(name_data.len() as u32);
        name_data.extend_from_slice(section.name.as_bytes());
        name_data.push(0);
    }
    let shstr_name = name_data.len() as u32;
    name_data.extend_from_slice(b".shstrtab\0");

    let mut section_offsets = Vec::new();
    for section in sections {
        section_offsets.push(bytes.len() as u64);
        bytes.extend_from_slice(section.data);
    }
    let shstr_offset = bytes.len() as u64;
    bytes.extend_from_slice(&name_data);

    let section_table_offset = bytes.len() as u64;
    write_u64(bytes, 40, section_table_offset);
    write_u16(bytes, 58, 64);
    write_u16(bytes, 60, sections.len() as u16 + 2);
    write_u16(bytes, 62, sections.len() as u16 + 1);
    bytes.resize(bytes.len() + (sections.len() + 2) * 64, 0);

    for (index, section) in sections.iter().enumerate() {
        let base = section_table_offset as usize + (index + 1) * 64;
        write_u32(bytes, base, name_offsets[index]);
        write_u32(bytes, base + 4, section.kind);
        write_u64(bytes, base + 24, section_offsets[index]);
        write_u64(bytes, base + 32, section.data.len() as u64);
    }

    let shstr_base = section_table_offset as usize + (sections.len() + 1) * 64;
    write_u32(bytes, shstr_base, shstr_name);
    write_u32(bytes, shstr_base + 4, 3);
    write_u64(bytes, shstr_base + 24, shstr_offset);
    write_u64(bytes, shstr_base + 32, name_data.len() as u64);
}

fn add_elf64_symbol_table(bytes: &mut Vec<u8>) {
    let symbol_names = b"\0entry_func\0data_obj\0";
    let symbol_names_offset = bytes.len() as u64;
    bytes.extend_from_slice(symbol_names);

    let symbol_table_offset = bytes.len() as u64;
    bytes.resize(bytes.len() + 3 * 24, 0);
    let function_base = symbol_table_offset as usize + 24;
    write_u32(bytes, function_base, 1);
    bytes[function_base + 4] = 0x12;
    write_u16(bytes, function_base + 6, 1);
    write_u64(bytes, function_base + 8, 0x8004);
    write_u64(bytes, function_base + 16, 4);
    let object_base = symbol_table_offset as usize + 48;
    write_u32(bytes, object_base, 12);
    bytes[object_base + 4] = 0x11;
    write_u16(bytes, object_base + 6, 1);
    write_u64(bytes, object_base + 8, 0x9000);
    write_u64(bytes, object_base + 16, 8);

    let section_names = b"\0.symtab\0.strtab\0.shstrtab\0";
    let section_names_offset = bytes.len() as u64;
    bytes.extend_from_slice(section_names);

    let section_table_offset = bytes.len() as u64;
    write_u64(bytes, 40, section_table_offset);
    write_u16(bytes, 58, 64);
    write_u16(bytes, 60, 4);
    write_u16(bytes, 62, 3);
    bytes.resize(bytes.len() + 4 * 64, 0);

    let symtab_base = section_table_offset as usize + 64;
    write_u32(bytes, symtab_base, 1);
    write_u32(bytes, symtab_base + 4, 2);
    write_u64(bytes, symtab_base + 24, symbol_table_offset);
    write_u64(bytes, symtab_base + 32, 3 * 24);
    write_u32(bytes, symtab_base + 40, 2);
    write_u64(bytes, symtab_base + 56, 24);

    let strtab_base = section_table_offset as usize + 128;
    write_u32(bytes, strtab_base, 9);
    write_u32(bytes, strtab_base + 4, 3);
    write_u64(bytes, strtab_base + 24, symbol_names_offset);
    write_u64(bytes, strtab_base + 32, symbol_names.len() as u64);

    let shstrtab_base = section_table_offset as usize + 192;
    write_u32(bytes, shstrtab_base, 17);
    write_u32(bytes, shstrtab_base + 4, 3);
    write_u64(bytes, shstrtab_base + 24, section_names_offset);
    write_u64(bytes, shstrtab_base + 32, section_names.len() as u64);
}

fn add_elf32_sections(bytes: &mut Vec<u8>, sections: &[ElfSectionSpec<'_>]) {
    let mut name_data = vec![0];
    let mut name_offsets = Vec::new();
    for section in sections {
        name_offsets.push(name_data.len() as u32);
        name_data.extend_from_slice(section.name.as_bytes());
        name_data.push(0);
    }
    let shstr_name = name_data.len() as u32;
    name_data.extend_from_slice(b".shstrtab\0");

    let mut section_offsets = Vec::new();
    for section in sections {
        section_offsets.push(bytes.len() as u32);
        bytes.extend_from_slice(section.data);
    }
    let shstr_offset = bytes.len() as u32;
    bytes.extend_from_slice(&name_data);

    let section_table_offset = bytes.len() as u32;
    write_u32(bytes, 32, section_table_offset);
    write_u16(bytes, 46, 40);
    write_u16(bytes, 48, sections.len() as u16 + 2);
    write_u16(bytes, 50, sections.len() as u16 + 1);
    bytes.resize(bytes.len() + (sections.len() + 2) * 40, 0);

    for (index, section) in sections.iter().enumerate() {
        let base = section_table_offset as usize + (index + 1) * 40;
        write_u32(bytes, base, name_offsets[index]);
        write_u32(bytes, base + 4, section.kind);
        write_u32(bytes, base + 16, section_offsets[index]);
        write_u32(bytes, base + 20, section.data.len() as u32);
    }

    let shstr_base = section_table_offset as usize + (sections.len() + 1) * 40;
    write_u32(bytes, shstr_base, shstr_name);
    write_u32(bytes, shstr_base + 4, 3);
    write_u32(bytes, shstr_base + 16, shstr_offset);
    write_u32(bytes, shstr_base + 20, name_data.len() as u32);
}

fn abi_note(os: u32) -> [u8; 20] {
    let mut note = [0; 20];
    write_u32(&mut note, 0, 4);
    write_u32(&mut note, 4, 16);
    write_u32(&mut note, 8, 1);
    note[12..16].copy_from_slice(b"GNU\0");
    write_u32(&mut note, 16, os);
    note
}

#[test]
fn boot_image_loads_elf64_loadable_segments_with_zero_fill() {
    let elf = elf64_image(
        0x8004,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x100,
                physical: 0x8000,
                file_size: 4,
                memory_size: 8,
            },
            ElfProgramHeaderSpec {
                kind: 4,
                offset: 0x108,
                physical: 0x8800,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x110,
                physical: 0x9002,
                file_size: 3,
                memory_size: 3,
            },
        ],
        &[
            (0x100, &[0x13, 0x05, 0x00, 0x00]),
            (0x108, &[0xde, 0xad, 0xbe, 0xef]),
            (0x110, &[0xa0, 0xa1, 0xa2]),
        ],
    );

    let image = BootImage::from_elf64_le(&elf).unwrap();

    assert_eq!(image.entry(), Address::new(0x8004));
    assert_eq!(image.segments().len(), 2);
    assert_eq!(image.segments()[0].range().start(), Address::new(0x8000));
    assert_eq!(
        image.segments()[0].data(),
        &[0x13, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    );
    assert_eq!(image.segments()[1].range().start(), Address::new(0x9002));
    assert_eq!(image.segments()[1].data(), &[0xa0, 0xa1, 0xa2]);
}

#[test]
fn boot_image_loads_elf32_loadable_segments_with_zero_fill() {
    let elf = elf32_image(
        0x8040,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x100,
                physical: 0x8000,
                file_size: 4,
                memory_size: 8,
            },
            ElfProgramHeaderSpec {
                kind: 4,
                offset: 0x108,
                physical: 0x9000,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x110,
                physical: 0xa002,
                file_size: 3,
                memory_size: 3,
            },
        ],
        &[
            (0x100, &[0x13, 0x05, 0x00, 0x00]),
            (0x108, &[0xde, 0xad, 0xbe, 0xef]),
            (0x110, &[0xb0, 0xb1, 0xb2]),
        ],
    );

    let image = BootImage::from_elf32_le(&elf).unwrap();

    assert_eq!(image.entry(), Address::new(0x8040));
    assert_eq!(image.segments().len(), 2);
    assert_eq!(image.segments()[0].range().start(), Address::new(0x8000));
    assert_eq!(
        image.segments()[0].data(),
        &[0x13, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    );
    assert_eq!(image.segments()[1].range().start(), Address::new(0xa002));
    assert_eq!(image.segments()[1].data(), &[0xb0, 0xb1, 0xb2]);
}

#[test]
fn boot_image_detects_little_endian_elf_class() {
    let elf64 = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 8,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    let elf32 = elf32_image(
        0x8040,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x9000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x93, 0x05, 0x10, 0x00])],
    );

    assert_eq!(
        BootImage::from_elf(&elf64).unwrap(),
        BootImage::from_elf64_le(&elf64).unwrap(),
    );
    assert_eq!(
        BootImage::from_elf(&elf32).unwrap(),
        BootImage::from_elf32_le(&elf32).unwrap(),
    );
}

#[test]
fn boot_image_detects_big_endian_elf_class() {
    let elf64 = elf64_be_image(
        0x400008,
        2,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x400000,
            file_size: 4,
            memory_size: 6,
        }],
        &[(0x100, &[0xde, 0xad, 0xbe, 0xef])],
    );
    let image64 = BootImage::from_elf(&elf64).unwrap();

    assert_eq!(image64.entry(), Address::new(0x400008));
    assert_eq!(
        image64.segments()[0].range().start(),
        Address::new(0x400000)
    );
    assert_eq!(
        image64.segments()[0].data(),
        &[0xde, 0xad, 0xbe, 0xef, 0, 0],
    );
    assert_eq!(
        image64.elf_metadata().unwrap().architecture(),
        BootElfArchitecture::Sparc64,
    );
    assert_eq!(image64.elf_metadata().unwrap().endian(), BootElfEndian::Big);

    let elf32 = elf32_be_image(
        0x1000,
        8,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x2000,
            file_size: 3,
            memory_size: 4,
        }],
        &[(0x100, &[0xaa, 0xbb, 0xcc])],
    );
    let image32 = BootImage::from_elf(&elf32).unwrap();

    assert_eq!(image32.entry(), Address::new(0x1000));
    assert_eq!(image32.segments()[0].range().start(), Address::new(0x2000));
    assert_eq!(image32.segments()[0].data(), &[0xaa, 0xbb, 0xcc, 0]);
    assert_eq!(
        image32.elf_metadata().unwrap().architecture(),
        BootElfArchitecture::Mips,
    );
    assert_eq!(image32.elf_metadata().unwrap().endian(), BootElfEndian::Big);
}

#[test]
fn boot_image_detects_unsupported_elf_encoding() {
    let mut elf = elf64_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    elf[5] = 3;

    assert_eq!(
        BootImage::from_elf(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::UnsupportedEncoding { encoding: 3 },
        },
    );
}

#[test]
fn boot_image_records_elf_machine_metadata() {
    let elf64 = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    let elf32 = elf32_image(
        0x8040,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x9000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x93, 0x05, 0x10, 0x00])],
    );

    let metadata64 = BootImage::from_elf64_le(&elf64)
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(metadata64.class(), BootElfClass::Class64);
    assert_eq!(metadata64.machine(), 243);
    assert_eq!(metadata64.architecture(), BootElfArchitecture::Riscv64);

    let metadata32 = BootImage::from_elf32_le(&elf32)
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(metadata32.class(), BootElfClass::Class32);
    assert_eq!(metadata32.machine(), 243);
    assert_eq!(metadata32.architecture(), BootElfArchitecture::Riscv32);

    assert_eq!(BootImage::new(Address::new(0)).elf_metadata(), None);
}

#[test]
fn boot_image_records_loaded_program_header_table_metadata() {
    let elf = elf64_image(
        0x8000_0080,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0,
            physical: 0x8000_0000,
            file_size: 120,
            memory_size: 120,
        }],
        &[],
    );
    let mut elf = elf;
    write_u64(&mut elf, 80, 0x9000_0000);

    let metadata = BootImage::from_elf64_le(&elf)
        .unwrap()
        .elf_metadata()
        .unwrap();
    let table = metadata.program_header_table();
    assert_eq!(table.file_offset(), 64);
    assert_eq!(table.entry_size(), 56);
    assert_eq!(table.entry_count(), 1);
    assert_eq!(table.memory_address(), Some(Address::new(0x8000_0040)));
}

#[test]
fn boot_image_records_elf64_interpreter_metadata() {
    let interpreter = b"/lib/ld-linux-riscv64-lp64d.so.1\0";
    let elf = elf64_image(
        0x8000_0000,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x200,
                physical: 0x8000_0000,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 3,
                offset: 0x180,
                physical: 0,
                file_size: interpreter.len() as u64,
                memory_size: interpreter.len() as u64,
            },
        ],
        &[(0x180, interpreter), (0x200, &[0x13, 0, 0, 0])],
    );

    let image = BootImage::from_elf64_le(&elf).unwrap();
    let metadata = image.elf_interpreter().unwrap();

    assert_eq!(metadata.path(), "/lib/ld-linux-riscv64-lp64d.so.1");
    assert_eq!(metadata.file_offset(), 0x180);
    assert_eq!(metadata.file_size(), interpreter.len() as u64);
}

#[test]
fn boot_image_records_elf32_interpreter_metadata() {
    let interpreter = b"/lib/ld-linux-riscv32-ilp32d.so.1\0";
    let elf = elf32_image(
        0x8000_0000,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x200,
                physical: 0x8000_0000,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 3,
                offset: 0x180,
                physical: 0,
                file_size: interpreter.len() as u64,
                memory_size: interpreter.len() as u64,
            },
        ],
        &[(0x180, interpreter), (0x200, &[0x13, 0, 0, 0])],
    );

    let image = BootImage::from_elf32_le(&elf).unwrap();
    let metadata = image.elf_interpreter().unwrap();

    assert_eq!(metadata.path(), "/lib/ld-linux-riscv32-ilp32d.so.1");
    assert_eq!(metadata.file_offset(), 0x180);
    assert_eq!(metadata.file_size(), interpreter.len() as u64);
}

#[test]
fn boot_image_rejects_elf64_interpreter_without_nul_terminator() {
    let interpreter = b"/lib/ld-linux-riscv64-lp64d.so.1";
    let elf = elf64_image(
        0x8000_0000,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x200,
                physical: 0x8000_0000,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 3,
                offset: 0x180,
                physical: 0,
                file_size: interpreter.len() as u64,
                memory_size: interpreter.len() as u64,
            },
        ],
        &[(0x180, interpreter), (0x200, &[0x13, 0, 0, 0])],
    );

    assert_eq!(
        BootImage::from_elf64_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::UnterminatedInterpreterPath { segment: 1 },
        },
    );
}

#[test]
fn boot_image_rejects_extended_program_header_count_with_bad_section_header_size() {
    let mut elf = elf64_image(
        0x8000_0080,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000_0000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0, 0, 0])],
    );
    let section_table_offset = elf.len() as u64;
    write_u64(&mut elf, 40, section_table_offset);
    write_u16(&mut elf, 56, 0xffff);
    write_u16(&mut elf, 58, 63);
    write_u16(&mut elf, 60, 1);
    elf.resize(elf.len() + 64, 0);
    write_u32(&mut elf, section_table_offset as usize + 44, 1);

    assert_eq!(
        BootImage::from_elf64_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::UnsupportedSectionHeaderSize {
                expected: 64,
                actual: 63,
            },
        },
    );
}

#[test]
fn boot_image_rejects_extended_section_count_before_reading_section_zero() {
    let mut elf = elf64_image(
        0x8000_0080,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000_0000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0, 0, 0])],
    );
    write_u64(&mut elf, 40, u64::MAX - 7);
    write_u16(&mut elf, 58, 64);
    write_u16(&mut elf, 60, 0);
    write_u16(&mut elf, 62, 0xffff);

    assert_eq!(
        BootImage::from_elf64_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::SectionHeaderTableOutOfBounds {
                offset: u64::MAX - 7,
                size: 64,
                image_size: elf.len() as u64,
            },
        },
    );
}

#[test]
fn boot_image_maps_arm_thumb_from_elf32_entry_bit() {
    let mut arm = elf32_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x00, 0x00, 0xa0, 0xe3])],
    );
    write_u16(&mut arm, 18, 40);
    let mut thumb = elf32_image(
        0x8001,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x9000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x00, 0xbf, 0x00, 0xbf])],
    );
    write_u16(&mut thumb, 18, 40);

    assert_eq!(
        BootImage::from_elf32_le(&arm)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .architecture(),
        BootElfArchitecture::Arm,
    );
    assert_eq!(
        BootImage::from_elf32_le(&thumb)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .architecture(),
        BootElfArchitecture::Thumb,
    );
}

#[test]
fn boot_image_preserves_unknown_elf_machine_metadata() {
    let mut elf = elf64_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    write_u16(&mut elf, 18, 0xffff);

    assert_eq!(
        BootImage::from_elf64_le(&elf)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .architecture(),
        BootElfArchitecture::Unknown {
            machine: 0xffff,
            class: BootElfClass::Class64,
        },
    );
}

#[test]
fn boot_image_records_elf_operating_system_metadata() {
    let mut linux = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    linux[7] = 3;
    let mut freebsd = elf32_image(
        0x8040,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x9000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x93, 0x05, 0x10, 0x00])],
    );
    freebsd[7] = 9;

    let linux_metadata = BootImage::from_elf64_le(&linux)
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(linux_metadata.os_abi(), 3);
    assert_eq!(
        linux_metadata.operating_system(),
        BootElfOperatingSystem::Linux,
    );

    let freebsd_metadata = BootImage::from_elf32_le(&freebsd)
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(freebsd_metadata.os_abi(), 9);
    assert_eq!(
        freebsd_metadata.operating_system(),
        BootElfOperatingSystem::FreeBsd,
    );

    let unknown_metadata = BootImage::from_elf64_le(&elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    ))
    .unwrap()
    .elf_metadata()
    .unwrap();
    assert_eq!(
        unknown_metadata.operating_system(),
        BootElfOperatingSystem::Unknown { os_abi: 0 },
    );
}

#[test]
fn boot_image_maps_power64_abi_from_elf_flags() {
    let mut abi_v1 = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x60, 0x00, 0x00, 0x00])],
    );
    write_u16(&mut abi_v1, 18, 21);
    write_u32(&mut abi_v1, 48, 1);
    let mut abi_v2 = abi_v1.clone();
    write_u32(&mut abi_v2, 48, 2);
    let mut abi_default = abi_v1.clone();
    write_u32(&mut abi_default, 48, 0);

    let metadata_v1 = BootImage::from_elf64_le(&abi_v1)
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(metadata_v1.flags(), 1);
    assert_eq!(
        metadata_v1.operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV1,
    );
    assert_eq!(
        BootImage::from_elf64_le(&abi_v2)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV2,
    );
    assert_eq!(
        BootImage::from_elf64_le(&abi_default)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV2,
    );
}

#[test]
fn boot_image_maps_power64_default_abi_from_elf_endian() {
    let elf = elf64_be_image(
        0x8004,
        21,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x60, 0x00, 0x00, 0x00])],
    );

    let metadata = BootImage::from_elf(&elf).unwrap().elf_metadata().unwrap();

    assert_eq!(metadata.endian(), BootElfEndian::Big);
    assert_eq!(metadata.flags(), 0);
    assert_eq!(metadata.architecture(), BootElfArchitecture::Power64);
    assert_eq!(
        metadata.operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV1,
    );
}

#[test]
fn boot_image_derives_operating_system_from_abi_note_section() {
    let linux_note = abi_note(0);
    let mut linux = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    add_elf64_sections(
        &mut linux,
        &[ElfSectionSpec {
            name: ".note.ABI-tag",
            kind: 7,
            data: &linux_note,
        }],
    );
    let freebsd_note = abi_note(3);
    let mut freebsd = elf32_image(
        0x8040,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x9000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x93, 0x05, 0x10, 0x00])],
    );
    add_elf32_sections(
        &mut freebsd,
        &[ElfSectionSpec {
            name: ".note.ABI-tag",
            kind: 7,
            data: &freebsd_note,
        }],
    );

    assert_eq!(
        BootImage::from_elf64_le(&linux)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .operating_system(),
        BootElfOperatingSystem::Linux,
    );
    assert_eq!(
        BootImage::from_elf32_le(&freebsd)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .operating_system(),
        BootElfOperatingSystem::FreeBsd,
    );
}

#[test]
fn boot_image_derives_solaris_from_section_names() {
    let mut elf = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    add_elf64_sections(
        &mut elf,
        &[ElfSectionSpec {
            name: ".SUNW_version",
            kind: 1,
            data: &[],
        }],
    );

    assert_eq!(
        BootImage::from_elf64_le(&elf)
            .unwrap()
            .elf_metadata()
            .unwrap()
            .operating_system(),
        BootElfOperatingSystem::Solaris,
    );
}

#[test]
fn boot_image_records_tls_from_tbss_section_even_with_header_os() {
    let mut elf = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    elf[7] = 3;
    add_elf64_sections(
        &mut elf,
        &[ElfSectionSpec {
            name: ".tbss",
            kind: 8,
            data: &[],
        }],
    );

    let metadata = BootImage::from_elf64_le(&elf)
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(metadata.operating_system(), BootElfOperatingSystem::Linux);
    assert!(metadata.has_tls());
}

#[test]
fn boot_image_records_symbol_table_summary() {
    let mut elf = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    add_elf64_symbol_table(&mut elf);

    let metadata = BootImage::from_elf64_le(&elf)
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(metadata.symbol_count(), 2);
    assert_eq!(metadata.function_symbol_count(), 1);
    assert_eq!(metadata.object_symbol_count(), 1);
}

#[test]
fn boot_image_ignores_bad_symbol_table_contents() {
    let mut elf = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    elf[7] = 3;
    add_elf64_symbol_table(&mut elf);
    let section_table_offset = u64::from_le_bytes(elf[40..48].try_into().unwrap()) as usize;
    write_u64(&mut elf, section_table_offset + 64 + 24, u64::MAX - 7);

    let metadata = BootImage::from_elf64_le(&elf)
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(metadata.operating_system(), BootElfOperatingSystem::Linux);
    assert_eq!(metadata.symbol_count(), 0);
}

#[test]
fn boot_image_loads_header_os_elf_with_bad_section_table() {
    let mut elf = elf64_image(
        0x8004,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: 0x8000,
            file_size: 4,
            memory_size: 4,
        }],
        &[(0x100, &[0x13, 0x05, 0x00, 0x00])],
    );
    elf[7] = 3;
    write_u64(&mut elf, 40, u64::MAX - 7);
    write_u16(&mut elf, 58, 64);
    write_u16(&mut elf, 60, 0);
    write_u16(&mut elf, 62, 0xffff);

    let metadata = BootImage::from_elf64_le(&elf)
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(metadata.operating_system(), BootElfOperatingSystem::Linux);
    assert!(!metadata.has_tls());
}

#[test]
fn boot_image_rejects_elf64_segment_memory_overflow_with_segment_context() {
    let elf = elf64_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: u64::MAX - 1,
            file_size: 2,
            memory_size: 4,
        }],
        &[(0x100, &[0xaa, 0xbb])],
    );

    assert_eq!(
        BootImage::from_elf64_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::SegmentMemoryRangeOverflow {
                segment: 0,
                physical: u64::MAX - 1,
                memory_size: 4,
            },
        },
    );
}

#[test]
fn boot_image_rejects_elf64_segment_memory_above_vec_capacity_before_allocation() {
    let elf = elf64_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0,
            physical: 0,
            file_size: 0,
            memory_size: OVERSIZED_VECTOR_LENGTH,
        }],
        &[],
    );

    assert_eq!(
        BootImage::from_elf64_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::SegmentMemorySizeTooLarge {
                segment: 0,
                memory_size: OVERSIZED_VECTOR_LENGTH,
            },
        },
    );
}

#[test]
fn boot_image_rejects_elf32_segment_memory_overflow_with_segment_context() {
    let elf = elf32_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: u32::MAX as u64 - 1,
            file_size: 2,
            memory_size: 4,
        }],
        &[(0x100, &[0xaa, 0xbb])],
    );

    assert_eq!(
        BootImage::from_elf32_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::SegmentMemoryRangeOverflow {
                segment: 0,
                physical: u32::MAX as u64 - 1,
                memory_size: 4,
            },
        },
    );
}

#[test]
fn boot_image_loads_segments_across_lines_and_preserves_existing_bytes() {
    let mut store = LineMemoryStore::new(layout());
    store.insert_line(Address::new(0x1000), line(0x55)).unwrap();
    let image = BootImage::new(Address::new(0x1004))
        .add_segment(Address::new(0x100e), vec![0xa0, 0xa1, 0xa2, 0xa3])
        .unwrap()
        .add_segment(Address::new(0x1020), vec![0xb0, 0xb1, 0xb2])
        .unwrap();

    let report = image.load_into_line_store(&mut store).unwrap();

    assert_eq!(
        report,
        BootLoadReport::new(
            Address::new(0x1004),
            vec![
                BootLineWrite::new(Address::new(0x1000), 14, 2),
                BootLineWrite::new(Address::new(0x1010), 0, 2),
                BootLineWrite::new(Address::new(0x1020), 0, 3),
            ],
        )
    );
    let first = store.line_data(Address::new(0x1000)).unwrap();
    assert_eq!(&first[0..14], &[0x55; 14]);
    assert_eq!(&first[14..16], &[0xa0, 0xa1]);
    let second = store.line_data(Address::new(0x1010)).unwrap();
    assert_eq!(&second[0..2], &[0xa2, 0xa3]);
    assert_eq!(&second[2..16], &[0; 14]);
    let third = store.line_data(Address::new(0x1020)).unwrap();
    assert_eq!(&third[0..3], &[0xb0, 0xb1, 0xb2]);
    assert_eq!(&third[3..16], &[0; 13]);
}

#[test]
fn boot_image_reports_loaded_segment_end() {
    assert_eq!(
        BootImage::new(Address::new(0x1004)).loaded_segment_end(),
        Address::new(0)
    );

    let image = BootImage::new(Address::new(0x1004))
        .add_segment(Address::new(0x100e), vec![0xa0, 0xa1, 0xa2, 0xa3])
        .unwrap()
        .add_segment(Address::new(0x3020), vec![0xb0, 0xb1, 0xb2])
        .unwrap();

    assert_eq!(image.loaded_segment_end(), Address::new(0x3023));
}

#[test]
fn boot_image_loads_into_partitioned_store_target() {
    let target = MemoryTargetId::new(7);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    let image = BootImage::new(Address::new(0x8004))
        .add_segment(Address::new(0x8008), vec![1, 2, 3, 4])
        .unwrap();

    let report = image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();

    assert_eq!(
        report,
        BootLoadReport::new(
            Address::new(0x8004),
            vec![BootLineWrite::new(Address::new(0x8000), 8, 4)],
        )
    );
    let data = store.line_data(target, Address::new(0x8000)).unwrap();
    assert_eq!(&data[0..8], &[0; 8]);
    assert_eq!(&data[8..12], &[1, 2, 3, 4]);
    assert_eq!(&data[12..16], &[0; 4]);
}

#[test]
fn boot_image_loads_partitioned_store_segments_by_address_region() {
    let code = MemoryTargetId::new(1);
    let data = MemoryTargetId::new(2);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(code, layout()).unwrap();
    store.add_partition(data, layout()).unwrap();
    store
        .map_region(code, Address::new(0x8000), AccessSize::new(0x1000).unwrap())
        .unwrap();
    store
        .map_region(data, Address::new(0xa000), AccessSize::new(0x1000).unwrap())
        .unwrap();
    store
        .insert_line(data, Address::new(0xa000), line(0x77))
        .unwrap();
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8004), vec![1, 2, 3, 4])
        .unwrap()
        .add_segment(Address::new(0xa00e), vec![0xa0, 0xa1, 0xa2, 0xa3])
        .unwrap();

    let report = image
        .load_into_partitioned_store_by_address(&mut store)
        .unwrap();

    assert_eq!(
        report,
        BootLoadReport::new(
            Address::new(0x8000),
            vec![
                BootLineWrite::new(Address::new(0x8000), 4, 4),
                BootLineWrite::new(Address::new(0xa000), 14, 2),
                BootLineWrite::new(Address::new(0xa010), 0, 2),
            ],
        )
    );
    assert_eq!(
        &store.line_data(code, Address::new(0x8000)).unwrap()[4..8],
        &[1, 2, 3, 4],
    );
    let first_data = store.line_data(data, Address::new(0xa000)).unwrap();
    assert_eq!(&first_data[0..14], &[0x77; 14]);
    assert_eq!(&first_data[14..16], &[0xa0, 0xa1]);
    let second_data = store.line_data(data, Address::new(0xa010)).unwrap();
    assert_eq!(&second_data[0..2], &[0xa2, 0xa3]);
    assert_eq!(&second_data[2..16], &[0; 14]);
}

#[test]
fn boot_image_rejects_bad_segments_and_unknown_partition() {
    assert_eq!(
        BootImage::new(Address::new(0)).add_segment(Address::new(0x1000), Vec::new()),
        Err(BootError::EmptySegment {
            start: Address::new(0x1000),
        })
    );
    assert_eq!(
        BootImage::new(Address::new(0))
            .add_segment(Address::new(u64::MAX), vec![1, 2])
            .unwrap_err(),
        BootError::Memory(MemoryError::AddressOverflow {
            start: Address::new(u64::MAX),
            size: AccessSize::new(2).unwrap(),
        })
    );

    let overlap = BootImage::new(Address::new(0))
        .add_segment(Address::new(0x2000), vec![0; 8])
        .unwrap()
        .add_segment(Address::new(0x2004), vec![0; 8])
        .unwrap_err();
    assert_eq!(
        overlap,
        BootError::OverlappingSegment {
            existing: AddressRange::new(Address::new(0x2000), AccessSize::new(8).unwrap()).unwrap(),
            requested: AddressRange::new(Address::new(0x2004), AccessSize::new(8).unwrap())
                .unwrap(),
        }
    );

    let unknown = MemoryTargetId::new(9);
    let mut store = PartitionedMemoryStore::new();
    let image = BootImage::new(Address::new(0))
        .add_segment(Address::new(0x3000), vec![0xaa])
        .unwrap();
    assert_eq!(
        image
            .load_into_partitioned_store(&mut store, unknown)
            .unwrap_err(),
        BootError::Memory(MemoryError::UnknownMemoryTarget { target: unknown })
    );
}
