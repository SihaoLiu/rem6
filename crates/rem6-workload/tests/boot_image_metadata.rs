use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfMetadata, BootElfOperatingSystem,
    BootImage,
};
use rem6_memory::Address;
use rem6_workload::{WorkloadBootImage, WorkloadId, WorkloadManifest};

const PT_NOTE: u32 = 4;
const PT_GNU_EH_FRAME: u32 = 0x6474_e550;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const PT_GNU_PROPERTY: u32 = 0x6474_e553;
const SHT_NOBITS: u32 = 8;
const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;
const SHF_EXECINSTR: u64 = 4;
const DT_PLTGOT: u64 = 3;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_STRSZ: u64 = 10;
const DT_SYMENT: u64 = 11;
const DT_INIT: u64 = 12;
const DT_FINI: u64 = 13;
const DT_SYMBOLIC: u64 = 16;
const DT_DEBUG: u64 = 21;
const DT_TEXTREL: u64 = 22;
const DT_BIND_NOW: u64 = 24;
const DT_INIT_ARRAY: u64 = 25;
const DT_FINI_ARRAY: u64 = 26;
const DT_INIT_ARRAYSZ: u64 = 27;
const DT_FINI_ARRAYSZ: u64 = 28;
const DT_FLAGS: u64 = 30;
const DT_PREINIT_ARRAY: u64 = 32;
const DT_PREINIT_ARRAYSZ: u64 = 33;
const DT_DEPAUDIT: u64 = 0x6fff_fefb;
const DT_AUDIT: u64 = 0x6fff_fefc;
const DT_VERSYM: u64 = 0x6fff_fff0;
const DT_RELACOUNT: u64 = 0x6fff_fff9;
const DT_RELCOUNT: u64 = 0x6fff_fffa;
const DT_VERDEF: u64 = 0x6fff_fffc;
const DT_VERDEFNUM: u64 = 0x6fff_fffd;
const DT_VERNEED: u64 = 0x6fff_fffe;
const DT_VERNEEDNUM: u64 = 0x6fff_ffff;
const DT_FLAGS_1: u64 = 0x6fff_fffb;
const DT_AUXILIARY: u64 = 0x7fff_fffd;
const DT_FILTER: u64 = 0x7fff_ffff;
const DT_IGNORED_TEST_TAG: u64 = 0x6fff_ef00;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
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

fn dynamic_string_table(values: &[&str]) -> (Vec<u8>, Vec<u64>) {
    let mut strings = vec![0];
    let mut offsets = Vec::new();
    for value in values {
        offsets.push(strings.len() as u64);
        strings.extend_from_slice(value.as_bytes());
        strings.push(0);
    }
    (strings, offsets)
}

fn elf64_image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x104];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 1);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x100);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(&mut bytes, 96, 4);
    write_u64(&mut bytes, 104, 4);
    write_u64(&mut bytes, 112, 0x1000);
    bytes[0x100..0x104].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn elf64_image_with_interpreter(machine: u16, interpreter: &str) -> Vec<u8> {
    let mut bytes = vec![0; 0x204];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x200);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(&mut bytes, 96, 4);
    write_u64(&mut bytes, 104, 4);
    write_u64(&mut bytes, 112, 0x1000);

    let mut interpreter_bytes = interpreter.as_bytes().to_vec();
    interpreter_bytes.push(0);
    write_u32(&mut bytes, 120, 3);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0x180);
    write_u64(&mut bytes, 136, 0);
    write_u64(&mut bytes, 144, 0);
    write_u64(&mut bytes, 152, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 160, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 168, 1);

    bytes[0x180..0x180 + interpreter_bytes.len()].copy_from_slice(&interpreter_bytes);
    bytes[0x200..0x204].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn elf64_image_with_tbss(machine: u16) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    let names = b"\0.tbss\0.shstrtab\0";
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(names);

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);
    bytes.resize(section_table_offset + 3 * 64, 0);

    write_u32(&mut bytes, section_table_offset + 64, 1);
    write_u32(&mut bytes, section_table_offset + 68, 8);
    write_u64(&mut bytes, section_table_offset + 96, 16);

    write_u32(&mut bytes, section_table_offset + 128, 7);
    write_u32(&mut bytes, section_table_offset + 132, 3);
    write_u64(&mut bytes, section_table_offset + 152, shstr_offset as u64);
    write_u64(&mut bytes, section_table_offset + 160, names.len() as u64);
    bytes
}

fn elf64_image_with_named_sections(machine: u16, section_names: &[&str]) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    let mut name_data = vec![0];
    let mut name_offsets = Vec::new();
    for section_name in section_names {
        name_offsets.push(name_data.len() as u32);
        name_data.extend_from_slice(section_name.as_bytes());
        name_data.push(0);
    }
    let shstr_name = name_data.len() as u32;
    name_data.extend_from_slice(b".shstrtab\0");

    let shstr_offset = bytes.len();
    bytes.extend_from_slice(&name_data);

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, section_names.len() as u16 + 2);
    write_u16(&mut bytes, 62, section_names.len() as u16 + 1);
    bytes.resize(section_table_offset + (section_names.len() + 2) * 64, 0);

    for (index, name_offset) in name_offsets.iter().enumerate() {
        let base = section_table_offset + (index + 1) * 64;
        write_u32(&mut bytes, base, *name_offset);
        write_u32(&mut bytes, base + 4, 1);
    }

    let shstr_base = section_table_offset + (section_names.len() + 1) * 64;
    write_u32(&mut bytes, shstr_base, shstr_name);
    write_u32(&mut bytes, shstr_base + 4, 3);
    write_u64(&mut bytes, shstr_base + 24, shstr_offset as u64);
    write_u64(&mut bytes, shstr_base + 32, name_data.len() as u64);
    bytes
}

fn grow_elf64_section_name_table(bytes: &mut [u8], extra_bytes: u64) {
    let section_table_offset = read_u64(bytes, 40) as usize;
    let section_header_size = usize::from(read_u16(bytes, 58));
    let string_table_index = usize::from(read_u16(bytes, 62));
    let byte_size_offset = section_table_offset + string_table_index * section_header_size + 32;
    let byte_size = read_u64(bytes, byte_size_offset);
    write_u64(bytes, byte_size_offset, byte_size + extra_bytes);
}

fn set_elf64_section_kind_flags(bytes: &mut [u8], index: usize, kind: u32, flags: u64) {
    let section_table_offset = read_u64(bytes, 40) as usize;
    let section_header_size = usize::from(read_u16(bytes, 58));
    let section_base = section_table_offset + index * section_header_size;
    write_u32(bytes, section_base + 4, kind);
    write_u64(bytes, section_base + 8, flags);
}

fn elf64_image_with_pt_tls(machine: u16) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    write_u16(&mut bytes, 56, 2);
    write_u32(&mut bytes, 120, 7);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0);
    write_u64(&mut bytes, 136, 0x9000);
    write_u64(&mut bytes, 144, 0x9000);
    write_u64(&mut bytes, 152, 0);
    write_u64(&mut bytes, 160, 16);
    write_u64(&mut bytes, 168, 8);
    bytes
}

fn elf64_image_with_gnu_stack(machine: u16, executable: bool) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    write_u16(&mut bytes, 56, 2);
    write_u32(&mut bytes, 120, PT_GNU_STACK);
    write_u32(&mut bytes, 124, if executable { 5 } else { 6 });
    bytes
}

fn elf64_image_with_gnu_relro(
    machine: u16,
    virtual_address: u64,
    physical_address: u64,
    memory_size: u64,
) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    write_u16(&mut bytes, 56, 2);
    write_u32(&mut bytes, 120, PT_GNU_RELRO);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0);
    write_u64(&mut bytes, 136, virtual_address);
    write_u64(&mut bytes, 144, physical_address);
    write_u64(&mut bytes, 152, 0);
    write_u64(&mut bytes, 160, memory_size);
    write_u64(&mut bytes, 168, 8);
    bytes
}

fn elf64_image_with_gnu_eh_frame(
    machine: u16,
    virtual_address: u64,
    physical_address: u64,
    memory_size: u64,
) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    write_u16(&mut bytes, 56, 2);
    write_u32(&mut bytes, 120, PT_GNU_EH_FRAME);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0);
    write_u64(&mut bytes, 136, virtual_address);
    write_u64(&mut bytes, 144, physical_address);
    write_u64(&mut bytes, 152, 0);
    write_u64(&mut bytes, 160, memory_size);
    write_u64(&mut bytes, 168, 8);
    bytes
}

fn elf64_image_with_gnu_property(
    machine: u16,
    virtual_address: u64,
    physical_address: u64,
    memory_size: u64,
) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    write_u16(&mut bytes, 56, 2);
    write_u32(&mut bytes, 120, PT_GNU_PROPERTY);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0);
    write_u64(&mut bytes, 136, virtual_address);
    write_u64(&mut bytes, 144, physical_address);
    write_u64(&mut bytes, 152, 0);
    write_u64(&mut bytes, 160, memory_size);
    write_u64(&mut bytes, 168, 8);
    bytes
}

fn elf64_image_with_note_segments(machine: u16, first_size: u64, second_size: u64) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    write_u16(&mut bytes, 56, 3);
    write_u32(&mut bytes, 120, PT_NOTE);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0x180);
    write_u64(&mut bytes, 136, 0x9100);
    write_u64(&mut bytes, 144, 0x9100);
    write_u64(&mut bytes, 152, first_size);
    write_u64(&mut bytes, 160, first_size);
    write_u64(&mut bytes, 168, 8);
    write_u32(&mut bytes, 176, PT_NOTE);
    write_u32(&mut bytes, 180, 4);
    write_u64(&mut bytes, 184, 0x1a0);
    write_u64(&mut bytes, 192, 0x9200);
    write_u64(&mut bytes, 200, 0x9200);
    write_u64(&mut bytes, 208, second_size);
    write_u64(&mut bytes, 216, second_size);
    write_u64(&mut bytes, 224, 8);
    bytes
}

fn elf64_image_with_symbols(machine: u16) -> Vec<u8> {
    elf64_image_with_symbol_section(machine, ".symtab", 2, ".strtab")
}

fn elf64_image_with_dynamic_symbols(machine: u16) -> Vec<u8> {
    elf64_image_with_symbol_section(machine, ".dynsym", 11, ".dynstr")
}

fn elf64_image_with_symbol_section(
    machine: u16,
    symbol_section_name: &str,
    symbol_section_kind: u32,
    string_section_name: &str,
) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    let symbol_names = b"\0entry_func\0data_obj\0";
    let symbol_names_offset = bytes.len();
    bytes.extend_from_slice(symbol_names);

    let symbol_table_offset = bytes.len();
    bytes.resize(bytes.len() + 3 * 24, 0);
    let function_base = symbol_table_offset + 24;
    write_u32(&mut bytes, function_base, 1);
    bytes[function_base + 4] = 0x12;
    write_u16(&mut bytes, function_base + 6, 1);
    write_u64(&mut bytes, function_base + 8, 0x8004);
    write_u64(&mut bytes, function_base + 16, 4);
    let object_base = symbol_table_offset + 48;
    write_u32(&mut bytes, object_base, 12);
    bytes[object_base + 4] = 0x11;
    write_u16(&mut bytes, object_base + 6, 1);
    write_u64(&mut bytes, object_base + 8, 0x9000);
    write_u64(&mut bytes, object_base + 16, 8);

    let symbol_section_name_offset = 1;
    let string_section_name_offset =
        symbol_section_name_offset + symbol_section_name.len() as u32 + 1;
    let shstrtab_name_offset = string_section_name_offset + string_section_name.len() as u32 + 1;
    let section_names = format!("\0{symbol_section_name}\0{string_section_name}\0.shstrtab\0");
    let section_names_offset = bytes.len();
    bytes.extend_from_slice(section_names.as_bytes());

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 4);
    write_u16(&mut bytes, 62, 3);
    bytes.resize(section_table_offset + 4 * 64, 0);

    let symtab_base = section_table_offset + 64;
    write_u32(&mut bytes, symtab_base, symbol_section_name_offset);
    write_u32(&mut bytes, symtab_base + 4, symbol_section_kind);
    write_u64(&mut bytes, symtab_base + 24, symbol_table_offset as u64);
    write_u64(&mut bytes, symtab_base + 32, 3 * 24);
    write_u32(&mut bytes, symtab_base + 40, 2);
    write_u64(&mut bytes, symtab_base + 56, 24);

    let strtab_base = section_table_offset + 128;
    write_u32(&mut bytes, strtab_base, string_section_name_offset);
    write_u32(&mut bytes, strtab_base + 4, 3);
    write_u64(&mut bytes, strtab_base + 24, symbol_names_offset as u64);
    write_u64(&mut bytes, strtab_base + 32, symbol_names.len() as u64);

    let shstrtab_base = section_table_offset + 192;
    write_u32(&mut bytes, shstrtab_base, shstrtab_name_offset);
    write_u32(&mut bytes, shstrtab_base + 4, 3);
    write_u64(&mut bytes, shstrtab_base + 24, section_names_offset as u64);
    write_u64(&mut bytes, shstrtab_base + 32, section_names.len() as u64);
    bytes
}

fn elf64_image_with_dynamic_table(machine: u16, needed_count: usize) -> Vec<u8> {
    assert!(needed_count <= 2);
    let strtab = b"\0lib0.so\0lib1.so\0";
    let strtab_offset = 0x300usize;
    let mut bytes = vec![0; strtab_offset + strtab.len()];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x200);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(
        &mut bytes,
        96,
        (strtab_offset + strtab.len() - 0x200) as u64,
    );
    write_u64(
        &mut bytes,
        104,
        (strtab_offset + strtab.len() - 0x200) as u64,
    );
    write_u64(&mut bytes, 112, 0x1000);

    let dynamic_offset = 0x180usize;
    let dynamic_size = (needed_count + 3) * 16;
    write_u32(&mut bytes, 120, 2);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, dynamic_offset as u64);
    write_u64(&mut bytes, 136, 0x8180);
    write_u64(&mut bytes, 144, 0x8180);
    write_u64(&mut bytes, 152, dynamic_size as u64);
    write_u64(&mut bytes, 160, dynamic_size as u64);
    write_u64(&mut bytes, 168, 8);

    let needed_offsets = [1u64, 9u64];
    for index in 0..needed_count {
        let entry = dynamic_offset + index * 16;
        write_u64(&mut bytes, entry, 1);
        write_u64(&mut bytes, entry + 8, needed_offsets[index]);
    }
    let strtab_entry = dynamic_offset + needed_count * 16;
    write_u64(&mut bytes, strtab_entry, 5);
    write_u64(&mut bytes, strtab_entry + 8, 0x8100);
    write_u64(&mut bytes, strtab_entry + 16, 10);
    write_u64(&mut bytes, strtab_entry + 24, strtab.len() as u64);
    let null_entry = strtab_entry + 32;
    write_u64(&mut bytes, null_entry, 0);
    write_u64(&mut bytes, null_entry + 8, 0);
    bytes[0x200..0x204].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(strtab);
    bytes
}

fn elf64_image_with_dynamic_hashes(machine: u16, sysv_hash: u64, gnu_hash: u64) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 0x180, 4);
    write_u64(&mut bytes, 0x188, sysv_hash);
    write_u64(&mut bytes, 0x190, 0x6fff_fef5);
    write_u64(&mut bytes, 0x198, gnu_hash);
    write_u64(&mut bytes, 0x1a0, 0);
    write_u64(&mut bytes, 0x1a8, 0);
    bytes
}

fn elf64_image_with_dynamic_flags(machine: u16, flags: u64, flags_1: u64) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 0x180, DT_FLAGS);
    write_u64(&mut bytes, 0x188, flags);
    write_u64(&mut bytes, 0x190, DT_FLAGS_1);
    write_u64(&mut bytes, 0x198, flags_1);
    write_u64(&mut bytes, 0x1a0, 0);
    write_u64(&mut bytes, 0x1a8, 0);
    bytes
}

fn elf64_image_with_dynamic_lifecycle(machine: u16, init: u64, fini: u64) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 0x180, DT_INIT);
    write_u64(&mut bytes, 0x188, init);
    write_u64(&mut bytes, 0x190, DT_FINI);
    write_u64(&mut bytes, 0x198, fini);
    write_u64(&mut bytes, 0x1a0, 0);
    write_u64(&mut bytes, 0x1a8, 0);
    bytes
}

fn elf64_image_with_dynamic_lifecycle_arrays(
    machine: u16,
    init_array: u64,
    init_array_size: u64,
    fini_array: u64,
    fini_array_size: u64,
    preinit_array: u64,
    preinit_array_size: u64,
) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 152, 7 * 16);
    write_u64(&mut bytes, 160, 7 * 16);
    write_u64(&mut bytes, 0x180, DT_INIT_ARRAY);
    write_u64(&mut bytes, 0x188, init_array);
    write_u64(&mut bytes, 0x190, DT_INIT_ARRAYSZ);
    write_u64(&mut bytes, 0x198, init_array_size);
    write_u64(&mut bytes, 0x1a0, DT_FINI_ARRAY);
    write_u64(&mut bytes, 0x1a8, fini_array);
    write_u64(&mut bytes, 0x1b0, DT_FINI_ARRAYSZ);
    write_u64(&mut bytes, 0x1b8, fini_array_size);
    write_u64(&mut bytes, 0x1c0, DT_PREINIT_ARRAY);
    write_u64(&mut bytes, 0x1c8, preinit_array);
    write_u64(&mut bytes, 0x1d0, DT_PREINIT_ARRAYSZ);
    write_u64(&mut bytes, 0x1d8, preinit_array_size);
    write_u64(&mut bytes, 0x1e0, 0);
    write_u64(&mut bytes, 0x1e8, 0);
    bytes
}

fn elf64_image_with_dynamic_symbol_string_tables(
    machine: u16,
    string_table: u64,
    string_size: u64,
    symbol_table: u64,
    symbol_entry_size: u64,
) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 152, 5 * 16);
    write_u64(&mut bytes, 160, 5 * 16);
    write_u64(&mut bytes, 0x180, DT_STRTAB);
    write_u64(&mut bytes, 0x188, string_table);
    write_u64(&mut bytes, 0x190, DT_STRSZ);
    write_u64(&mut bytes, 0x198, string_size);
    write_u64(&mut bytes, 0x1a0, DT_SYMTAB);
    write_u64(&mut bytes, 0x1a8, symbol_table);
    write_u64(&mut bytes, 0x1b0, DT_SYMENT);
    write_u64(&mut bytes, 0x1b8, symbol_entry_size);
    write_u64(&mut bytes, 0x1c0, 0);
    write_u64(&mut bytes, 0x1c8, 0);
    bytes
}

fn elf64_image_with_dynamic_versioning(
    machine: u16,
    versym: u64,
    verdef: u64,
    verdef_count: u64,
    verneed: u64,
    verneed_count: u64,
) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 152, 6 * 16);
    write_u64(&mut bytes, 160, 6 * 16);
    write_u64(&mut bytes, 0x180, DT_VERSYM);
    write_u64(&mut bytes, 0x188, versym);
    write_u64(&mut bytes, 0x190, DT_VERDEF);
    write_u64(&mut bytes, 0x198, verdef);
    write_u64(&mut bytes, 0x1a0, DT_VERDEFNUM);
    write_u64(&mut bytes, 0x1a8, verdef_count);
    write_u64(&mut bytes, 0x1b0, DT_VERNEED);
    write_u64(&mut bytes, 0x1b8, verneed);
    write_u64(&mut bytes, 0x1c0, DT_VERNEEDNUM);
    write_u64(&mut bytes, 0x1c8, verneed_count);
    write_u64(&mut bytes, 0x1d0, 0);
    write_u64(&mut bytes, 0x1d8, 0);
    bytes
}

fn elf64_image_with_dynamic_linker_metadata(
    machine: u16,
    plt_got: u64,
    debug: u64,
    rela_relative_count: u64,
    rel_relative_count: u64,
) -> Vec<u8> {
    elf64_image_with_dynamic_linker_metadata_flags(
        machine,
        plt_got,
        debug,
        rela_relative_count,
        rel_relative_count,
        true,
        true,
        true,
    )
}

fn elf64_image_with_dynamic_linker_metadata_flags(
    machine: u16,
    plt_got: u64,
    debug: u64,
    rela_relative_count: u64,
    rel_relative_count: u64,
    symbolic: bool,
    textrel: bool,
    bind_now: bool,
) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, 0);
    write_u64(&mut bytes, 152, 8 * 16);
    write_u64(&mut bytes, 160, 8 * 16);
    let mut entry = 0x180usize;
    write_dynamic_entry(&mut bytes, &mut entry, DT_PLTGOT, plt_got);
    write_dynamic_entry(&mut bytes, &mut entry, DT_DEBUG, debug);
    write_dynamic_entry(
        &mut bytes,
        &mut entry,
        if symbolic {
            DT_SYMBOLIC
        } else {
            DT_IGNORED_TEST_TAG
        },
        0,
    );
    write_dynamic_entry(
        &mut bytes,
        &mut entry,
        if textrel {
            DT_TEXTREL
        } else {
            DT_IGNORED_TEST_TAG + 1
        },
        0,
    );
    write_dynamic_entry(
        &mut bytes,
        &mut entry,
        if bind_now {
            DT_BIND_NOW
        } else {
            DT_IGNORED_TEST_TAG + 2
        },
        0,
    );
    write_dynamic_entry(&mut bytes, &mut entry, DT_RELACOUNT, rela_relative_count);
    write_dynamic_entry(&mut bytes, &mut entry, DT_RELCOUNT, rel_relative_count);
    write_dynamic_entry(&mut bytes, &mut entry, 0, 0);
    bytes
}

fn write_dynamic_entry(bytes: &mut [u8], entry: &mut usize, tag: u64, value: u64) {
    write_u64(bytes, *entry, tag);
    write_u64(bytes, *entry + 8, value);
    *entry += 16;
}

fn elf64_image_with_dynamic_libraries(machine: u16, libraries: &[&str]) -> Vec<u8> {
    elf64_image_with_dynamic_strings(
        machine,
        libraries,
        "librem6.so",
        "/opt/rem6/lib",
        "$ORIGIN/lib",
    )
}

#[derive(Clone, Copy)]
struct DynamicRelocations {
    rela_address: u64,
    rela_size: u64,
    rela_entry_size: u64,
    rel_address: u64,
    rel_size: u64,
    rel_entry_size: u64,
    plt_address: u64,
    plt_size: u64,
    plt_kind: u64,
}

impl DynamicRelocations {
    const fn rela_size(mut self, rela_size: u64) -> Self {
        self.rela_size = rela_size;
        self
    }

    const fn rel_address(mut self, rel_address: u64) -> Self {
        self.rel_address = rel_address;
        self
    }

    const fn plt_kind(mut self, plt_kind: u64, plt_size: u64) -> Self {
        self.plt_kind = plt_kind;
        self.plt_size = plt_size;
        self
    }
}

impl Default for DynamicRelocations {
    fn default() -> Self {
        Self {
            rela_address: 0x8260,
            rela_size: 48,
            rela_entry_size: 24,
            rel_address: 0x82a0,
            rel_size: 16,
            rel_entry_size: 16,
            plt_address: 0x82c0,
            plt_size: 24,
            plt_kind: 7,
        }
    }
}

fn elf64_image_with_dynamic_strings(
    machine: u16,
    libraries: &[&str],
    soname: &str,
    rpath: &str,
    runpath: &str,
) -> Vec<u8> {
    elf64_image_with_dynamic_strings_and_relocations(
        machine,
        libraries,
        soname,
        rpath,
        runpath,
        DynamicRelocations::default(),
    )
}

fn elf64_image_with_dynamic_loader_strings(
    machine: u16,
    auxiliary: &str,
    filter: &str,
    audit: &str,
    dependency_audit: &str,
) -> Vec<u8> {
    let (strtab, offsets) = dynamic_string_table(&[auxiliary, filter, audit, dependency_audit]);
    let strtab_offset = 0x300usize;
    let dynamic_offset = 0x180usize;
    let payload_offset = 0x200usize;
    let dynamic_size = 7 * 16usize;
    let mut bytes = vec![0; strtab_offset + strtab.len()];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(
        &mut bytes,
        96,
        (strtab_offset + strtab.len() - payload_offset) as u64,
    );
    write_u64(
        &mut bytes,
        104,
        (strtab_offset + strtab.len() - payload_offset) as u64,
    );
    write_u64(&mut bytes, 112, 0x1000);

    write_u32(&mut bytes, 120, 2);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, dynamic_offset as u64);
    write_u64(&mut bytes, 136, 0x8180);
    write_u64(&mut bytes, 144, 0x8180);
    write_u64(&mut bytes, 152, dynamic_size as u64);
    write_u64(&mut bytes, 160, dynamic_size as u64);
    write_u64(&mut bytes, 168, 8);

    write_u64(&mut bytes, dynamic_offset, DT_AUXILIARY);
    write_u64(&mut bytes, dynamic_offset + 8, offsets[0]);
    write_u64(&mut bytes, dynamic_offset + 16, DT_FILTER);
    write_u64(&mut bytes, dynamic_offset + 24, offsets[1]);
    write_u64(&mut bytes, dynamic_offset + 32, DT_AUDIT);
    write_u64(&mut bytes, dynamic_offset + 40, offsets[2]);
    write_u64(&mut bytes, dynamic_offset + 48, DT_DEPAUDIT);
    write_u64(&mut bytes, dynamic_offset + 56, offsets[3]);
    write_u64(&mut bytes, dynamic_offset + 64, DT_STRTAB);
    write_u64(&mut bytes, dynamic_offset + 72, 0x8100);
    write_u64(&mut bytes, dynamic_offset + 80, DT_STRSZ);
    write_u64(&mut bytes, dynamic_offset + 88, strtab.len() as u64);
    write_u64(&mut bytes, dynamic_offset + 96, 0);
    write_u64(&mut bytes, dynamic_offset + 104, 0);
    bytes[payload_offset..payload_offset + 4].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(&strtab);
    bytes
}

fn elf64_image_with_dynamic_strings_and_relocations(
    machine: u16,
    libraries: &[&str],
    soname: &str,
    rpath: &str,
    runpath: &str,
    relocations: DynamicRelocations,
) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, libraries.len());
    let mut strtab = vec![0];
    let mut offsets = Vec::new();
    for library in libraries {
        offsets.push(strtab.len() as u64);
        strtab.extend_from_slice(library.as_bytes());
        strtab.push(0);
    }
    let soname_offset = strtab.len() as u64;
    strtab.extend_from_slice(soname.as_bytes());
    strtab.push(0);
    let rpath_offset = strtab.len() as u64;
    strtab.extend_from_slice(rpath.as_bytes());
    strtab.push(0);
    let runpath_offset = strtab.len() as u64;
    strtab.extend_from_slice(runpath.as_bytes());
    strtab.push(0);
    let strtab_offset = 0x300usize;
    bytes.resize(strtab_offset + strtab.len(), 0);
    let file_size = (strtab_offset + strtab.len() - 0x200) as u64;
    write_u64(&mut bytes, 96, file_size);
    write_u64(&mut bytes, 104, file_size);
    for (index, offset) in offsets.iter().enumerate() {
        write_u64(&mut bytes, 0x180 + index * 16 + 8, *offset);
    }
    let soname_entry = 0x180 + libraries.len() * 16;
    write_u64(&mut bytes, soname_entry, 14);
    write_u64(&mut bytes, soname_entry + 8, soname_offset);
    write_u64(&mut bytes, soname_entry + 16, 15);
    write_u64(&mut bytes, soname_entry + 24, rpath_offset);
    write_u64(&mut bytes, soname_entry + 32, 29);
    write_u64(&mut bytes, soname_entry + 40, runpath_offset);
    let rela_entry = soname_entry + 48;
    write_u64(&mut bytes, rela_entry, 7);
    write_u64(&mut bytes, rela_entry + 8, relocations.rela_address);
    write_u64(&mut bytes, rela_entry + 16, 8);
    write_u64(&mut bytes, rela_entry + 24, relocations.rela_size);
    write_u64(&mut bytes, rela_entry + 32, 9);
    write_u64(&mut bytes, rela_entry + 40, relocations.rela_entry_size);
    write_u64(&mut bytes, rela_entry + 48, 17);
    write_u64(&mut bytes, rela_entry + 56, relocations.rel_address);
    write_u64(&mut bytes, rela_entry + 64, 18);
    write_u64(&mut bytes, rela_entry + 72, relocations.rel_size);
    write_u64(&mut bytes, rela_entry + 80, 19);
    write_u64(&mut bytes, rela_entry + 88, relocations.rel_entry_size);
    write_u64(&mut bytes, rela_entry + 96, 23);
    write_u64(&mut bytes, rela_entry + 104, relocations.plt_address);
    write_u64(&mut bytes, rela_entry + 112, 2);
    write_u64(&mut bytes, rela_entry + 120, relocations.plt_size);
    write_u64(&mut bytes, rela_entry + 128, 20);
    write_u64(&mut bytes, rela_entry + 136, relocations.plt_kind);
    let strtab_entry = rela_entry + 144;
    write_u64(&mut bytes, strtab_entry, 5);
    write_u64(&mut bytes, strtab_entry + 8, 0x8100);
    write_u64(&mut bytes, strtab_entry + 16, 10);
    write_u64(&mut bytes, strtab_entry + 24, strtab.len() as u64);
    write_u64(&mut bytes, strtab_entry + 32, 0);
    write_u64(&mut bytes, strtab_entry + 40, 0);
    write_u64(&mut bytes, 152, ((libraries.len() + 15) * 16) as u64);
    write_u64(&mut bytes, 160, ((libraries.len() + 15) * 16) as u64);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(&strtab);
    bytes
}

fn elf64_be_image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x104];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 2;
    bytes[6] = 1;
    write_u16_be(&mut bytes, 16, 2);
    write_u16_be(&mut bytes, 18, machine);
    write_u32_be(&mut bytes, 20, 1);
    write_u64_be(&mut bytes, 24, 0x8004);
    write_u64_be(&mut bytes, 32, 64);
    write_u16_be(&mut bytes, 52, 64);
    write_u16_be(&mut bytes, 54, 56);
    write_u16_be(&mut bytes, 56, 1);

    write_u32_be(&mut bytes, 64, 1);
    write_u32_be(&mut bytes, 68, 5);
    write_u64_be(&mut bytes, 72, 0x100);
    write_u64_be(&mut bytes, 80, 0x8000);
    write_u64_be(&mut bytes, 88, 0x8000);
    write_u64_be(&mut bytes, 96, 4);
    write_u64_be(&mut bytes, 104, 4);
    write_u64_be(&mut bytes, 112, 0x1000);
    bytes[0x100..0x104].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn boot_image_with_metadata(metadata: BootElfMetadata) -> BootImage {
    BootImage::new(Address::new(0x9000))
        .add_segment(Address::new(0x9000), vec![0x13, 0, 0, 0])
        .unwrap()
        .with_elf_metadata(metadata)
}

#[test]
fn workload_boot_image_preserves_elf_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let metadata = image.elf_metadata().unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert_eq!(workload_image.elf_metadata(), Some(metadata));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_metadata()
            .unwrap()
            .architecture(),
        BootElfArchitecture::Riscv64,
    );
}

#[test]
fn workload_boot_image_preserves_elf_interpreter_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-linux-riscv64-lp64d.so.1",
    ))
    .unwrap();
    let interpreter = image.elf_interpreter().unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert_eq!(workload_image.elf_interpreter(), Some(interpreter));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_interpreter()
            .unwrap()
            .path(),
        "/lib/ld-linux-riscv64-lp64d.so.1",
    );
}

#[test]
fn workload_boot_image_preserves_elf_tls_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_tbss(243)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert!(workload_image.elf_metadata().unwrap().has_tls());
    let round_trip_image = workload_image.to_boot_image().unwrap();
    assert!(round_trip_image.elf_metadata().unwrap().has_tls());
}

#[test]
fn workload_boot_image_preserves_elf_program_header_tls_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_pt_tls(243)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert!(workload_image.elf_metadata().unwrap().has_tls());
    let round_trip_image = workload_image.to_boot_image().unwrap();
    assert!(round_trip_image.elf_metadata().unwrap().has_tls());
}

#[test]
fn workload_boot_image_preserves_elf_gnu_stack_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_gnu_stack(243, false)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let metadata = workload_image.elf_metadata().unwrap();

    assert_eq!(metadata.gnu_stack_executable(), Some(false));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_metadata()
            .unwrap()
            .gnu_stack_executable(),
        Some(false),
    );
}

#[test]
fn workload_boot_image_preserves_elf_gnu_relro_metadata_round_trip() {
    let image =
        BootImage::from_elf64_le(&elf64_image_with_gnu_relro(243, 0x9000, 0xa000, 32)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let metadata = workload_image.elf_metadata().unwrap();

    assert_eq!(
        metadata.gnu_relro_virtual_address(),
        Some(Address::new(0x9000)),
    );
    assert_eq!(metadata.gnu_relro_memory_size(), Some(32));
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(
        round_trip_metadata.gnu_relro_virtual_address(),
        Some(Address::new(0x9000)),
    );
    assert_eq!(round_trip_metadata.gnu_relro_memory_size(), Some(32));
}

#[test]
fn workload_boot_image_preserves_elf_gnu_eh_frame_metadata_round_trip() {
    let image =
        BootImage::from_elf64_le(&elf64_image_with_gnu_eh_frame(243, 0x9100, 0xa100, 40)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let metadata = workload_image.elf_metadata().unwrap();

    assert_eq!(
        metadata.gnu_eh_frame_virtual_address(),
        Some(Address::new(0x9100)),
    );
    assert_eq!(metadata.gnu_eh_frame_memory_size(), Some(40));
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(
        round_trip_metadata.gnu_eh_frame_virtual_address(),
        Some(Address::new(0x9100)),
    );
    assert_eq!(round_trip_metadata.gnu_eh_frame_memory_size(), Some(40));
}

#[test]
fn workload_boot_image_preserves_elf_gnu_property_metadata_round_trip() {
    let image =
        BootImage::from_elf64_le(&elf64_image_with_gnu_property(243, 0x9200, 0xa200, 48)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let metadata = workload_image.elf_metadata().unwrap();

    assert_eq!(
        metadata.gnu_property_virtual_address(),
        Some(Address::new(0x9200)),
    );
    assert_eq!(metadata.gnu_property_memory_size(), Some(48));
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(
        round_trip_metadata.gnu_property_virtual_address(),
        Some(Address::new(0x9200)),
    );
    assert_eq!(round_trip_metadata.gnu_property_memory_size(), Some(48));
}

#[test]
fn workload_boot_image_preserves_elf_note_segment_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_note_segments(243, 12, 20)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let metadata = workload_image.elf_metadata().unwrap();

    assert_eq!(metadata.note_segment_count(), 2);
    assert_eq!(metadata.note_file_size(), 32);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    assert_eq!(round_trip_metadata.note_segment_count(), 2);
    assert_eq!(round_trip_metadata.note_file_size(), 32);
}

#[test]
fn workload_boot_image_preserves_elf_symbol_summary_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_symbols(243)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(round_trip_metadata.symbol_count(), 2);
    assert_eq!(round_trip_metadata.function_symbol_count(), 1);
    assert_eq!(round_trip_metadata.object_symbol_count(), 1);
}

#[test]
fn workload_boot_image_preserves_elf_section_header_table_round_trip() {
    let elf = elf64_image_with_named_sections(243, &[".meta", ".debug"]);
    let image = BootImage::from_elf64_le(&elf).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let table = round_trip_metadata.section_header_table();

    assert_eq!(table.file_offset(), read_u64(&elf, 40));
    assert_eq!(table.entry_size(), read_u16(&elf, 58));
    assert_eq!(table.entry_count(), u64::from(read_u16(&elf, 60)));
    assert_eq!(table.string_table_index(), u64::from(read_u16(&elf, 62)));
    assert_eq!(table.entry_size(), 64);
    assert_eq!(table.entry_count(), 4);
    assert_eq!(table.string_table_index(), 3);

    let name_table = round_trip_metadata.section_name_table();
    let shstr_header = table.file_offset() as usize
        + table.string_table_index() as usize * usize::from(table.entry_size());
    assert_eq!(name_table.file_offset(), read_u64(&elf, shstr_header + 24));
    assert_eq!(name_table.byte_size(), read_u64(&elf, shstr_header + 32));
}

#[test]
fn workload_boot_image_preserves_elf_section_flags_round_trip() {
    let mut elf = elf64_image_with_named_sections(243, &[".text", ".data", ".bss"]);
    set_elf64_section_kind_flags(&mut elf, 1, 1, SHF_ALLOC | SHF_EXECINSTR);
    set_elf64_section_kind_flags(&mut elf, 2, 1, SHF_ALLOC | SHF_WRITE);
    set_elf64_section_kind_flags(&mut elf, 3, SHT_NOBITS, SHF_ALLOC | SHF_WRITE);
    let image = BootImage::from_elf64_le(&elf).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let flags = round_trip_metadata.section_flags();

    assert_eq!(flags.allocated_count(), 3);
    assert_eq!(flags.writable_count(), 2);
    assert_eq!(flags.executable_count(), 1);
    assert_eq!(flags.nobits_count(), 1);
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_symbol_summary_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_symbols(243)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(round_trip_metadata.symbol_count(), 2);
    assert_eq!(round_trip_metadata.function_symbol_count(), 1);
    assert_eq!(round_trip_metadata.object_symbol_count(), 1);
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_table_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_table(243, 2)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.segment_count(), 1);
    assert_eq!(dynamic.file_offset(), Some(0x180));
    assert_eq!(dynamic.virtual_address().unwrap().get(), 0x8180);
    assert_eq!(dynamic.entry_size(), 16);
    assert_eq!(dynamic.entry_count(), 5);
    assert_eq!(dynamic.needed_count(), 2);
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_hash_metadata_round_trip() {
    let image =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_hashes(243, 0x8240, 0x8260)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(
        dynamic.sysv_hash_virtual_address(),
        Some(Address::new(0x8240)),
    );
    assert_eq!(
        dynamic.gnu_hash_virtual_address(),
        Some(Address::new(0x8260)),
    );
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_flag_metadata_round_trip() {
    let image =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_flags(243, 0x15, 0x8000_0001)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.flags(), Some(0x15));
    assert_eq!(dynamic.flags_1(), Some(0x8000_0001));
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_lifecycle_metadata_round_trip() {
    let image =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle(243, 0x8220, 0x8240)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.init_virtual_address(), Some(Address::new(0x8220)));
    assert_eq!(dynamic.fini_virtual_address(), Some(Address::new(0x8240)));
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_lifecycle_array_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 24, 0x8260, 16, 0x82a0, 8,
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(
        dynamic.init_array_virtual_address(),
        Some(Address::new(0x8220)),
    );
    assert_eq!(dynamic.init_array_size(), Some(24));
    assert_eq!(
        dynamic.fini_array_virtual_address(),
        Some(Address::new(0x8260)),
    );
    assert_eq!(dynamic.fini_array_size(), Some(16));
    assert_eq!(
        dynamic.preinit_array_virtual_address(),
        Some(Address::new(0x82a0)),
    );
    assert_eq!(dynamic.preinit_array_size(), Some(8));
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_symbol_string_tables_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_symbol_string_tables(
        243, 0x8220, 0x30, 0x8260, 24,
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(
        dynamic.string_table_virtual_address(),
        Some(Address::new(0x8220)),
    );
    assert_eq!(dynamic.string_table_size(), Some(0x30));
    assert_eq!(
        dynamic.symbol_table_virtual_address(),
        Some(Address::new(0x8260)),
    );
    assert_eq!(dynamic.symbol_table_entry_size(), Some(24));
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_versioning_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8220, 0x8260, 2, 0x82a0, 3,
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(
        dynamic.version_symbol_table_virtual_address(),
        Some(Address::new(0x8220)),
    );
    assert_eq!(
        dynamic.version_definition_table_virtual_address(),
        Some(Address::new(0x8260)),
    );
    assert_eq!(dynamic.version_definition_count(), Some(2));
    assert_eq!(
        dynamic.version_needed_table_virtual_address(),
        Some(Address::new(0x82a0)),
    );
    assert_eq!(dynamic.version_needed_count(), Some(3));
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_linker_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata(
        243, 0x8220, 0x8260, 4, 5,
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(
        dynamic.plt_got_virtual_address(),
        Some(Address::new(0x8220)),
    );
    assert_eq!(dynamic.debug_virtual_address(), Some(Address::new(0x8260)));
    assert!(dynamic.has_symbolic_binding());
    assert!(dynamic.has_text_relocations());
    assert!(dynamic.bind_now());
    assert_eq!(dynamic.rela_relative_count(), Some(4));
    assert_eq!(dynamic.rel_relative_count(), Some(5));
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_loader_strings_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_loader_strings(
        243,
        "libbefore.so",
        "libfilter.so",
        "audit.so",
        "depaudit.so",
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.auxiliary_libraries(), &["libbefore.so".to_string()]);
    assert_eq!(dynamic.filter_libraries(), &["libfilter.so".to_string()]);
    assert_eq!(dynamic.audit_libraries(), &["audit.so".to_string()]);
    assert_eq!(
        dynamic.dependency_audit_libraries(),
        &["depaudit.so".to_string()],
    );
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_needed_names_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_libraries(
        243,
        &["libc.so.6", "libm.so.6"],
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.needed_count(), 2);
    assert_eq!(
        dynamic.needed_libraries(),
        &["libc.so.6".to_string(), "libm.so.6".to_string()],
    );
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_string_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_libraries(
        243,
        &["libc.so.6", "libm.so.6"],
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.soname(), Some("librem6.so"));
    assert_eq!(dynamic.rpath(), &["/opt/rem6/lib".to_string()]);
    assert_eq!(dynamic.runpath(), &["$ORIGIN/lib".to_string()]);
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_relocation_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_libraries(
        243,
        &["libc.so.6", "libm.so.6"],
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.rela_entry_count(), 2);
    assert_eq!(dynamic.rel_entry_count(), 1);
    assert_eq!(dynamic.plt_rela_entry_count(), 1);
}

#[test]
fn workload_manifest_identity_includes_elf_metadata() {
    let riscv = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let x86 = BootImage::from_elf64_le(&elf64_image(62)).unwrap();

    assert_eq!(riscv.entry(), x86.entry());
    assert_eq!(riscv.segments(), x86.segments());
    assert_eq!(riscv.elf_metadata().unwrap().class(), BootElfClass::Class64,);
    assert_ne!(riscv.elf_metadata(), x86.elf_metadata());

    let riscv_manifest = WorkloadManifest::builder(id("same"), riscv)
        .build()
        .unwrap();
    let x86_manifest = WorkloadManifest::builder(id("same"), x86).build().unwrap();

    assert_ne!(riscv_manifest.identity(), x86_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_tls_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let tls = BootImage::from_elf64_le(&elf64_image_with_tbss(243)).unwrap();

    assert_eq!(plain.entry(), tls.entry());
    assert_eq!(plain.segments(), tls.segments());
    assert!(!plain.elf_metadata().unwrap().has_tls());
    assert!(tls.elf_metadata().unwrap().has_tls());

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let tls_manifest = WorkloadManifest::builder(id("same"), tls).build().unwrap();

    assert_ne!(plain_manifest.identity(), tls_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_gnu_stack_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let non_executable = BootImage::from_elf64_le(&elf64_image_with_gnu_stack(243, false)).unwrap();
    let executable = BootImage::from_elf64_le(&elf64_image_with_gnu_stack(243, true)).unwrap();

    assert_eq!(plain.entry(), non_executable.entry());
    assert_eq!(plain.segments(), non_executable.segments());
    assert_eq!(plain.segments(), executable.segments());
    assert_eq!(plain.elf_metadata().unwrap().gnu_stack_executable(), None);
    assert_eq!(
        non_executable
            .elf_metadata()
            .unwrap()
            .gnu_stack_executable(),
        Some(false)
    );
    assert_eq!(
        executable.elf_metadata().unwrap().gnu_stack_executable(),
        Some(true)
    );

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let non_executable_manifest = WorkloadManifest::builder(id("same"), non_executable)
        .build()
        .unwrap();
    let executable_manifest = WorkloadManifest::builder(id("same"), executable)
        .build()
        .unwrap();

    assert_ne!(
        plain_manifest.identity(),
        non_executable_manifest.identity()
    );
    assert_ne!(
        non_executable_manifest.identity(),
        executable_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_gnu_relro_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let relro =
        BootImage::from_elf64_le(&elf64_image_with_gnu_relro(243, 0x9000, 0xa000, 32)).unwrap();
    let relro_alt =
        BootImage::from_elf64_le(&elf64_image_with_gnu_relro(243, 0xb000, 0xc000, 64)).unwrap();

    assert_eq!(plain.entry(), relro.entry());
    assert_eq!(plain.segments(), relro.segments());
    assert_eq!(relro.segments(), relro_alt.segments());
    assert_eq!(
        plain.elf_metadata().unwrap().gnu_relro_virtual_address(),
        None
    );
    assert_eq!(
        relro.elf_metadata().unwrap().gnu_relro_virtual_address(),
        Some(Address::new(0x9000)),
    );

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let relro_manifest = WorkloadManifest::builder(id("same"), relro)
        .build()
        .unwrap();
    let relro_alt_manifest = WorkloadManifest::builder(id("same"), relro_alt)
        .build()
        .unwrap();

    assert_ne!(plain_manifest.identity(), relro_manifest.identity());
    assert_ne!(relro_manifest.identity(), relro_alt_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_gnu_eh_frame_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let eh_frame =
        BootImage::from_elf64_le(&elf64_image_with_gnu_eh_frame(243, 0x9100, 0xa100, 40)).unwrap();
    let eh_frame_alt =
        BootImage::from_elf64_le(&elf64_image_with_gnu_eh_frame(243, 0xb100, 0xc100, 80)).unwrap();

    assert_eq!(plain.entry(), eh_frame.entry());
    assert_eq!(plain.segments(), eh_frame.segments());
    assert_eq!(eh_frame.segments(), eh_frame_alt.segments());
    assert_eq!(
        plain.elf_metadata().unwrap().gnu_eh_frame_virtual_address(),
        None
    );
    assert_eq!(
        eh_frame
            .elf_metadata()
            .unwrap()
            .gnu_eh_frame_virtual_address(),
        Some(Address::new(0x9100)),
    );

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let eh_frame_manifest = WorkloadManifest::builder(id("same"), eh_frame)
        .build()
        .unwrap();
    let eh_frame_alt_manifest = WorkloadManifest::builder(id("same"), eh_frame_alt)
        .build()
        .unwrap();

    assert_ne!(plain_manifest.identity(), eh_frame_manifest.identity());
    assert_ne!(
        eh_frame_manifest.identity(),
        eh_frame_alt_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_gnu_property_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let property =
        BootImage::from_elf64_le(&elf64_image_with_gnu_property(243, 0x9200, 0xa200, 48)).unwrap();
    let property_alt =
        BootImage::from_elf64_le(&elf64_image_with_gnu_property(243, 0xb200, 0xc200, 96)).unwrap();

    assert_eq!(plain.entry(), property.entry());
    assert_eq!(plain.segments(), property.segments());
    assert_eq!(property.segments(), property_alt.segments());
    assert_eq!(
        plain.elf_metadata().unwrap().gnu_property_virtual_address(),
        None
    );
    assert_eq!(
        property
            .elf_metadata()
            .unwrap()
            .gnu_property_virtual_address(),
        Some(Address::new(0x9200)),
    );

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let property_manifest = WorkloadManifest::builder(id("same"), property)
        .build()
        .unwrap();
    let property_alt_manifest = WorkloadManifest::builder(id("same"), property_alt)
        .build()
        .unwrap();

    assert_ne!(plain_manifest.identity(), property_manifest.identity());
    assert_ne!(
        property_manifest.identity(),
        property_alt_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_note_segment_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let notes = BootImage::from_elf64_le(&elf64_image_with_note_segments(243, 12, 20)).unwrap();
    let notes_alt = BootImage::from_elf64_le(&elf64_image_with_note_segments(243, 12, 28)).unwrap();

    assert_eq!(plain.entry(), notes.entry());
    assert_eq!(plain.segments(), notes.segments());
    assert_eq!(notes.segments(), notes_alt.segments());
    assert_eq!(plain.elf_metadata().unwrap().note_segment_count(), 0);
    assert_eq!(notes.elf_metadata().unwrap().note_segment_count(), 2);
    assert_eq!(notes.elf_metadata().unwrap().note_file_size(), 32);

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let notes_manifest = WorkloadManifest::builder(id("same"), notes)
        .build()
        .unwrap();
    let notes_alt_manifest = WorkloadManifest::builder(id("same"), notes_alt)
        .build()
        .unwrap();

    assert_ne!(plain_manifest.identity(), notes_manifest.identity());
    assert_ne!(notes_manifest.identity(), notes_alt_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_symbol_summary() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let symbols = BootImage::from_elf64_le(&elf64_image_with_symbols(243)).unwrap();
    let dynamic_symbols = BootImage::from_elf64_le(&elf64_image_with_dynamic_symbols(243)).unwrap();

    assert_eq!(plain.entry(), symbols.entry());
    assert_eq!(plain.segments(), symbols.segments());
    assert_eq!(plain.segments(), dynamic_symbols.segments());
    assert_eq!(plain.elf_metadata().unwrap().symbol_count(), 0);
    assert_eq!(symbols.elf_metadata().unwrap().symbol_count(), 2);
    assert_eq!(
        dynamic_symbols.elf_metadata().unwrap().symbol_count(),
        symbols.elf_metadata().unwrap().symbol_count()
    );

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let symbol_manifest = WorkloadManifest::builder(id("same"), symbols)
        .build()
        .unwrap();
    let dynamic_symbol_manifest = WorkloadManifest::builder(id("same"), dynamic_symbols)
        .build()
        .unwrap();

    assert_ne!(plain_manifest.identity(), symbol_manifest.identity());
    assert_ne!(
        plain_manifest.identity(),
        dynamic_symbol_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_section_header_table() {
    let one_section =
        BootImage::from_elf64_le(&elf64_image_with_named_sections(243, &[".meta"])).unwrap();
    let two_sections =
        BootImage::from_elf64_le(&elf64_image_with_named_sections(243, &[".meta", ".debug"]))
            .unwrap();

    assert_eq!(one_section.entry(), two_sections.entry());
    assert_eq!(one_section.segments(), two_sections.segments());
    assert_eq!(
        one_section
            .elf_metadata()
            .unwrap()
            .section_header_table()
            .entry_count(),
        3,
    );
    assert_eq!(
        two_sections
            .elf_metadata()
            .unwrap()
            .section_header_table()
            .entry_count(),
        4,
    );

    let one_manifest = WorkloadManifest::builder(id("same"), one_section)
        .build()
        .unwrap();
    let two_manifest = WorkloadManifest::builder(id("same"), two_sections)
        .build()
        .unwrap();

    assert_ne!(one_manifest.identity(), two_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_section_name_table() {
    let baseline_source =
        BootImage::from_elf64_le(&elf64_image_with_named_sections(243, &[".meta"])).unwrap();
    let mut larger_names_elf = elf64_image_with_named_sections(243, &[".meta"]);
    grow_elf64_section_name_table(&mut larger_names_elf, 1);
    let larger_names_source = BootImage::from_elf64_le(&larger_names_elf).unwrap();

    assert_eq!(baseline_source.entry(), larger_names_source.entry());
    assert_eq!(baseline_source.segments(), larger_names_source.segments());
    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .section_header_table(),
        larger_names_source
            .elf_metadata()
            .unwrap()
            .section_header_table(),
    );
    assert_ne!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .section_name_table()
            .byte_size(),
        larger_names_source
            .elf_metadata()
            .unwrap()
            .section_name_table()
            .byte_size(),
    );

    let baseline = WorkloadManifest::builder(id("same"), baseline_source)
        .build()
        .unwrap();
    let larger_names = WorkloadManifest::builder(id("same"), larger_names_source)
        .build()
        .unwrap();

    assert_ne!(baseline.identity(), larger_names.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_section_flags() {
    let mut baseline_elf = elf64_image_with_named_sections(243, &[".text"]);
    set_elf64_section_kind_flags(&mut baseline_elf, 1, 1, SHF_ALLOC);
    let mut executable_elf = elf64_image_with_named_sections(243, &[".text"]);
    set_elf64_section_kind_flags(&mut executable_elf, 1, 1, SHF_ALLOC | SHF_EXECINSTR);
    let baseline_source = BootImage::from_elf64_le(&baseline_elf).unwrap();
    let executable_source = BootImage::from_elf64_le(&executable_elf).unwrap();

    assert_eq!(baseline_source.entry(), executable_source.entry());
    assert_eq!(baseline_source.segments(), executable_source.segments());
    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .section_header_table(),
        executable_source
            .elf_metadata()
            .unwrap()
            .section_header_table(),
    );
    assert_ne!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .section_flags()
            .executable_count(),
        executable_source
            .elf_metadata()
            .unwrap()
            .section_flags()
            .executable_count(),
    );

    let baseline = WorkloadManifest::builder(id("same"), baseline_source)
        .build()
        .unwrap();
    let executable = WorkloadManifest::builder(id("same"), executable_source)
        .build()
        .unwrap();

    assert_ne!(baseline.identity(), executable.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_table_summary() {
    let one_needed = BootImage::from_elf64_le(&elf64_image_with_dynamic_table(243, 1)).unwrap();
    let two_needed = BootImage::from_elf64_le(&elf64_image_with_dynamic_table(243, 2)).unwrap();

    assert_eq!(one_needed.entry(), two_needed.entry());
    assert_eq!(one_needed.segments(), two_needed.segments());
    assert_eq!(
        one_needed
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_count(),
        1,
    );
    assert_eq!(
        two_needed
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_count(),
        2,
    );

    let one_manifest = WorkloadManifest::builder(id("same"), one_needed)
        .build()
        .unwrap();
    let two_manifest = WorkloadManifest::builder(id("same"), two_needed)
        .build()
        .unwrap();

    assert_ne!(one_manifest.identity(), two_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_hash_metadata() {
    let baseline_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_hashes(243, 0x8240, 0x8260)).unwrap();
    let sysv_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_hashes(243, 0x8250, 0x8260)).unwrap();
    let gnu_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_hashes(243, 0x8240, 0x8270)).unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .sysv_hash_virtual_address(),
        Some(Address::new(0x8240)),
    );
    assert_eq!(
        gnu_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .gnu_hash_virtual_address(),
        Some(Address::new(0x8270)),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let sysv = boot_image_with_metadata(sysv_source.elf_metadata().unwrap());
    let gnu = boot_image_with_metadata(gnu_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), sysv.entry());
    assert_eq!(baseline.segments(), sysv.segments());
    assert_eq!(baseline.segments(), gnu.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let sysv_manifest = WorkloadManifest::builder(id("same"), sysv).build().unwrap();
    let gnu_manifest = WorkloadManifest::builder(id("same"), gnu).build().unwrap();

    assert_ne!(baseline_manifest.identity(), sysv_manifest.identity());
    assert_ne!(baseline_manifest.identity(), gnu_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_flag_metadata() {
    let baseline_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_flags(243, 0x15, 0x8000_0001)).unwrap();
    let flags_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_flags(243, 0x16, 0x8000_0001)).unwrap();
    let flags_1_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_flags(243, 0x15, 0x8000_0002)).unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .flags(),
        Some(0x15),
    );
    assert_eq!(
        flags_1_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .flags_1(),
        Some(0x8000_0002),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let flags = boot_image_with_metadata(flags_source.elf_metadata().unwrap());
    let flags_1 = boot_image_with_metadata(flags_1_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), flags.entry());
    assert_eq!(baseline.segments(), flags.segments());
    assert_eq!(baseline.segments(), flags_1.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let flags_manifest = WorkloadManifest::builder(id("same"), flags)
        .build()
        .unwrap();
    let flags_1_manifest = WorkloadManifest::builder(id("same"), flags_1)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), flags_manifest.identity());
    assert_ne!(baseline_manifest.identity(), flags_1_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_lifecycle_metadata() {
    let baseline_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle(243, 0x8220, 0x8240)).unwrap();
    let init_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle(243, 0x8230, 0x8240)).unwrap();
    let fini_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle(243, 0x8220, 0x8250)).unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .init_virtual_address(),
        Some(Address::new(0x8220)),
    );
    assert_eq!(
        fini_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .fini_virtual_address(),
        Some(Address::new(0x8250)),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let init = boot_image_with_metadata(init_source.elf_metadata().unwrap());
    let fini = boot_image_with_metadata(fini_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), init.entry());
    assert_eq!(baseline.segments(), init.segments());
    assert_eq!(baseline.segments(), fini.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let init_manifest = WorkloadManifest::builder(id("same"), init).build().unwrap();
    let fini_manifest = WorkloadManifest::builder(id("same"), fini).build().unwrap();

    assert_ne!(baseline_manifest.identity(), init_manifest.identity());
    assert_ne!(baseline_manifest.identity(), fini_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_lifecycle_arrays() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 24, 0x8260, 16, 0x82a0, 8,
    ))
    .unwrap();
    let init_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8230, 24, 0x8260, 16, 0x82a0, 8,
    ))
    .unwrap();
    let init_size_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 32, 0x8260, 16, 0x82a0, 8,
    ))
    .unwrap();
    let fini_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 24, 0x8270, 16, 0x82a0, 8,
    ))
    .unwrap();
    let fini_size_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 24, 0x8260, 24, 0x82a0, 8,
    ))
    .unwrap();
    let preinit_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 24, 0x8260, 16, 0x82b0, 8,
    ))
    .unwrap();
    let preinit_size_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_lifecycle_arrays(
        243, 0x8220, 24, 0x8260, 16, 0x82a0, 16,
    ))
    .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .preinit_array_size(),
        Some(8),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let init = boot_image_with_metadata(init_source.elf_metadata().unwrap());
    let init_size = boot_image_with_metadata(init_size_source.elf_metadata().unwrap());
    let fini = boot_image_with_metadata(fini_source.elf_metadata().unwrap());
    let fini_size = boot_image_with_metadata(fini_size_source.elf_metadata().unwrap());
    let preinit = boot_image_with_metadata(preinit_source.elf_metadata().unwrap());
    let preinit_size = boot_image_with_metadata(preinit_size_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), init.entry());
    assert_eq!(baseline.segments(), init.segments());
    assert_eq!(baseline.segments(), init_size.segments());
    assert_eq!(baseline.segments(), fini.segments());
    assert_eq!(baseline.segments(), fini_size.segments());
    assert_eq!(baseline.segments(), preinit.segments());
    assert_eq!(baseline.segments(), preinit_size.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let init_manifest = WorkloadManifest::builder(id("same"), init).build().unwrap();
    let init_size_manifest = WorkloadManifest::builder(id("same"), init_size)
        .build()
        .unwrap();
    let fini_manifest = WorkloadManifest::builder(id("same"), fini).build().unwrap();
    let fini_size_manifest = WorkloadManifest::builder(id("same"), fini_size)
        .build()
        .unwrap();
    let preinit_manifest = WorkloadManifest::builder(id("same"), preinit)
        .build()
        .unwrap();
    let preinit_size_manifest = WorkloadManifest::builder(id("same"), preinit_size)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), init_manifest.identity());
    assert_ne!(baseline_manifest.identity(), init_size_manifest.identity());
    assert_ne!(baseline_manifest.identity(), fini_manifest.identity());
    assert_ne!(baseline_manifest.identity(), fini_size_manifest.identity());
    assert_ne!(baseline_manifest.identity(), preinit_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        preinit_size_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_symbol_string_tables() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_symbol_string_tables(
        243, 0x8220, 0x30, 0x8260, 24,
    ))
    .unwrap();
    let string_table_source = BootImage::from_elf64_le(
        &elf64_image_with_dynamic_symbol_string_tables(243, 0x8230, 0x30, 0x8260, 24),
    )
    .unwrap();
    let string_size_source = BootImage::from_elf64_le(
        &elf64_image_with_dynamic_symbol_string_tables(243, 0x8220, 0x38, 0x8260, 24),
    )
    .unwrap();
    let symbol_table_source = BootImage::from_elf64_le(
        &elf64_image_with_dynamic_symbol_string_tables(243, 0x8220, 0x30, 0x8270, 24),
    )
    .unwrap();
    let symbol_entry_size_source = BootImage::from_elf64_le(
        &elf64_image_with_dynamic_symbol_string_tables(243, 0x8220, 0x30, 0x8260, 32),
    )
    .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .symbol_table_entry_size(),
        Some(24),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let string_table = boot_image_with_metadata(string_table_source.elf_metadata().unwrap());
    let string_size = boot_image_with_metadata(string_size_source.elf_metadata().unwrap());
    let symbol_table = boot_image_with_metadata(symbol_table_source.elf_metadata().unwrap());
    let symbol_entry_size =
        boot_image_with_metadata(symbol_entry_size_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), string_table.entry());
    assert_eq!(baseline.segments(), string_table.segments());
    assert_eq!(baseline.segments(), string_size.segments());
    assert_eq!(baseline.segments(), symbol_table.segments());
    assert_eq!(baseline.segments(), symbol_entry_size.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let string_table_manifest = WorkloadManifest::builder(id("same"), string_table)
        .build()
        .unwrap();
    let string_size_manifest = WorkloadManifest::builder(id("same"), string_size)
        .build()
        .unwrap();
    let symbol_table_manifest = WorkloadManifest::builder(id("same"), symbol_table)
        .build()
        .unwrap();
    let symbol_entry_size_manifest = WorkloadManifest::builder(id("same"), symbol_entry_size)
        .build()
        .unwrap();

    assert_ne!(
        baseline_manifest.identity(),
        string_table_manifest.identity()
    );
    assert_ne!(
        baseline_manifest.identity(),
        string_size_manifest.identity()
    );
    assert_ne!(
        baseline_manifest.identity(),
        symbol_table_manifest.identity()
    );
    assert_ne!(
        baseline_manifest.identity(),
        symbol_entry_size_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_versioning_metadata() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8220, 0x8260, 2, 0x82a0, 3,
    ))
    .unwrap();
    let versym_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8230, 0x8260, 2, 0x82a0, 3,
    ))
    .unwrap();
    let verdef_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8220, 0x8270, 2, 0x82a0, 3,
    ))
    .unwrap();
    let verdef_count_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8220, 0x8260, 4, 0x82a0, 3,
    ))
    .unwrap();
    let verneed_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8220, 0x8260, 2, 0x82b0, 3,
    ))
    .unwrap();
    let verneed_count_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_versioning(
        243, 0x8220, 0x8260, 2, 0x82a0, 5,
    ))
    .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .version_needed_count(),
        Some(3),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let versym = boot_image_with_metadata(versym_source.elf_metadata().unwrap());
    let verdef = boot_image_with_metadata(verdef_source.elf_metadata().unwrap());
    let verdef_count = boot_image_with_metadata(verdef_count_source.elf_metadata().unwrap());
    let verneed = boot_image_with_metadata(verneed_source.elf_metadata().unwrap());
    let verneed_count = boot_image_with_metadata(verneed_count_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), versym.entry());
    assert_eq!(baseline.segments(), versym.segments());
    assert_eq!(baseline.segments(), verdef.segments());
    assert_eq!(baseline.segments(), verdef_count.segments());
    assert_eq!(baseline.segments(), verneed.segments());
    assert_eq!(baseline.segments(), verneed_count.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let versym_manifest = WorkloadManifest::builder(id("same"), versym)
        .build()
        .unwrap();
    let verdef_manifest = WorkloadManifest::builder(id("same"), verdef)
        .build()
        .unwrap();
    let verdef_count_manifest = WorkloadManifest::builder(id("same"), verdef_count)
        .build()
        .unwrap();
    let verneed_manifest = WorkloadManifest::builder(id("same"), verneed)
        .build()
        .unwrap();
    let verneed_count_manifest = WorkloadManifest::builder(id("same"), verneed_count)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), versym_manifest.identity());
    assert_ne!(baseline_manifest.identity(), verdef_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        verdef_count_manifest.identity()
    );
    assert_ne!(baseline_manifest.identity(), verneed_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        verneed_count_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_linker_metadata() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata(
        243, 0x8220, 0x8260, 4, 5,
    ))
    .unwrap();
    let plt_got_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata(
        243, 0x8230, 0x8260, 4, 5,
    ))
    .unwrap();
    let debug_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata(
        243, 0x8220, 0x8270, 4, 5,
    ))
    .unwrap();
    let rela_count_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata(
        243, 0x8220, 0x8260, 6, 5,
    ))
    .unwrap();
    let rel_count_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata(
        243, 0x8220, 0x8260, 4, 7,
    ))
    .unwrap();
    let no_symbolic_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata_flags(
            243, 0x8220, 0x8260, 4, 5, false, true, true,
        ))
        .unwrap();
    let no_textrel_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata_flags(
            243, 0x8220, 0x8260, 4, 5, true, false, true,
        ))
        .unwrap();
    let no_bind_now_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_linker_metadata_flags(
            243, 0x8220, 0x8260, 4, 5, true, true, false,
        ))
        .unwrap();

    assert!(baseline_source
        .elf_metadata()
        .unwrap()
        .dynamic_table()
        .bind_now());
    assert!(!no_symbolic_source
        .elf_metadata()
        .unwrap()
        .dynamic_table()
        .has_symbolic_binding());
    assert!(!no_textrel_source
        .elf_metadata()
        .unwrap()
        .dynamic_table()
        .has_text_relocations());
    assert!(!no_bind_now_source
        .elf_metadata()
        .unwrap()
        .dynamic_table()
        .bind_now());
    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .entry_count(),
        no_symbolic_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .entry_count(),
    );
    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .entry_count(),
        no_textrel_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .entry_count(),
    );
    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .entry_count(),
        no_bind_now_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .entry_count(),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let plt_got = boot_image_with_metadata(plt_got_source.elf_metadata().unwrap());
    let debug = boot_image_with_metadata(debug_source.elf_metadata().unwrap());
    let rela_count = boot_image_with_metadata(rela_count_source.elf_metadata().unwrap());
    let rel_count = boot_image_with_metadata(rel_count_source.elf_metadata().unwrap());
    let no_symbolic = boot_image_with_metadata(no_symbolic_source.elf_metadata().unwrap());
    let no_textrel = boot_image_with_metadata(no_textrel_source.elf_metadata().unwrap());
    let no_bind_now = boot_image_with_metadata(no_bind_now_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), plt_got.entry());
    assert_eq!(baseline.segments(), plt_got.segments());
    assert_eq!(baseline.segments(), debug.segments());
    assert_eq!(baseline.segments(), rela_count.segments());
    assert_eq!(baseline.segments(), rel_count.segments());
    assert_eq!(baseline.segments(), no_symbolic.segments());
    assert_eq!(baseline.segments(), no_textrel.segments());
    assert_eq!(baseline.segments(), no_bind_now.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let plt_got_manifest = WorkloadManifest::builder(id("same"), plt_got)
        .build()
        .unwrap();
    let debug_manifest = WorkloadManifest::builder(id("same"), debug)
        .build()
        .unwrap();
    let rela_count_manifest = WorkloadManifest::builder(id("same"), rela_count)
        .build()
        .unwrap();
    let rel_count_manifest = WorkloadManifest::builder(id("same"), rel_count)
        .build()
        .unwrap();
    let no_symbolic_manifest = WorkloadManifest::builder(id("same"), no_symbolic)
        .build()
        .unwrap();
    let no_textrel_manifest = WorkloadManifest::builder(id("same"), no_textrel)
        .build()
        .unwrap();
    let no_bind_now_manifest = WorkloadManifest::builder(id("same"), no_bind_now)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), plt_got_manifest.identity());
    assert_ne!(baseline_manifest.identity(), debug_manifest.identity());
    assert_ne!(baseline_manifest.identity(), rela_count_manifest.identity());
    assert_ne!(baseline_manifest.identity(), rel_count_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        no_symbolic_manifest.identity()
    );
    assert_ne!(baseline_manifest.identity(), no_textrel_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        no_bind_now_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_string_metadata() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "librem6.so",
        "/opt/rem6/lib",
        "$ORIGIN/lib",
    ))
    .unwrap();
    let soname_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "libalt6.so",
        "/opt/rem6/lib",
        "$ORIGIN/lib",
    ))
    .unwrap();
    let rpath_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "librem6.so",
        "/tmp/rem6/lib",
        "$ORIGIN/lib",
    ))
    .unwrap();
    let runpath_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "librem6.so",
        "/opt/rem6/lib",
        "$ORIGIN/alt",
    ))
    .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_libraries(),
        soname_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_libraries(),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let soname = boot_image_with_metadata(soname_source.elf_metadata().unwrap());
    let rpath = boot_image_with_metadata(rpath_source.elf_metadata().unwrap());
    let runpath = boot_image_with_metadata(runpath_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), soname.entry());
    assert_eq!(baseline.segments(), soname.segments());
    assert_eq!(baseline.segments(), rpath.segments());
    assert_eq!(baseline.segments(), runpath.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let soname_manifest = WorkloadManifest::builder(id("same"), soname)
        .build()
        .unwrap();
    let rpath_manifest = WorkloadManifest::builder(id("same"), rpath)
        .build()
        .unwrap();
    let runpath_manifest = WorkloadManifest::builder(id("same"), runpath)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), soname_manifest.identity());
    assert_ne!(baseline_manifest.identity(), rpath_manifest.identity());
    assert_ne!(baseline_manifest.identity(), runpath_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_loader_strings() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_loader_strings(
        243,
        "libbefore.so",
        "libfilter.so",
        "audit.so",
        "depaudit.so",
    ))
    .unwrap();
    let auxiliary_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_loader_strings(
        243,
        "libbefore-alt.so",
        "libfilter.so",
        "audit.so",
        "depaudit.so",
    ))
    .unwrap();
    let filter_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_loader_strings(
        243,
        "libbefore.so",
        "libfilter-alt.so",
        "audit.so",
        "depaudit.so",
    ))
    .unwrap();
    let audit_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_loader_strings(
        243,
        "libbefore.so",
        "libfilter.so",
        "audit-alt.so",
        "depaudit.so",
    ))
    .unwrap();
    let dependency_audit_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_loader_strings(
            243,
            "libbefore.so",
            "libfilter.so",
            "audit.so",
            "depaudit-alt.so",
        ))
        .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .auxiliary_libraries(),
        &["libbefore.so".to_string()],
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let auxiliary = boot_image_with_metadata(auxiliary_source.elf_metadata().unwrap());
    let filter = boot_image_with_metadata(filter_source.elf_metadata().unwrap());
    let audit = boot_image_with_metadata(audit_source.elf_metadata().unwrap());
    let dependency_audit =
        boot_image_with_metadata(dependency_audit_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), auxiliary.entry());
    assert_eq!(baseline.segments(), auxiliary.segments());
    assert_eq!(baseline.segments(), filter.segments());
    assert_eq!(baseline.segments(), audit.segments());
    assert_eq!(baseline.segments(), dependency_audit.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let auxiliary_manifest = WorkloadManifest::builder(id("same"), auxiliary)
        .build()
        .unwrap();
    let filter_manifest = WorkloadManifest::builder(id("same"), filter)
        .build()
        .unwrap();
    let audit_manifest = WorkloadManifest::builder(id("same"), audit)
        .build()
        .unwrap();
    let dependency_audit_manifest = WorkloadManifest::builder(id("same"), dependency_audit)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), auxiliary_manifest.identity());
    assert_ne!(baseline_manifest.identity(), filter_manifest.identity());
    assert_ne!(baseline_manifest.identity(), audit_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        dependency_audit_manifest.identity()
    );
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_relocation_metadata() {
    let baseline_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_strings_and_relocations(
            243,
            &["libc.so.6", "libm.so.6"],
            "librem6.so",
            "/opt/rem6/lib",
            "$ORIGIN/lib",
            DynamicRelocations::default(),
        ))
        .unwrap();
    let rela_size_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_strings_and_relocations(
            243,
            &["libc.so.6", "libm.so.6"],
            "librem6.so",
            "/opt/rem6/lib",
            "$ORIGIN/lib",
            DynamicRelocations::default().rela_size(72),
        ))
        .unwrap();
    let rel_address_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_strings_and_relocations(
            243,
            &["libc.so.6", "libm.so.6"],
            "librem6.so",
            "/opt/rem6/lib",
            "$ORIGIN/lib",
            DynamicRelocations::default().rel_address(0x82b0),
        ))
        .unwrap();
    let plt_rel_source =
        BootImage::from_elf64_le(&elf64_image_with_dynamic_strings_and_relocations(
            243,
            &["libc.so.6", "libm.so.6"],
            "librem6.so",
            "/opt/rem6/lib",
            "$ORIGIN/lib",
            DynamicRelocations::default().plt_kind(17, 16),
        ))
        .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .rela_entry_count(),
        2,
    );
    assert_eq!(
        rela_size_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .rela_entry_count(),
        3,
    );
    assert_eq!(
        rel_address_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .rel_virtual_address()
            .unwrap()
            .get(),
        0x82b0,
    );
    assert_eq!(
        plt_rel_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .plt_rel_entry_count(),
        1,
    );

    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let rela_size = boot_image_with_metadata(rela_size_source.elf_metadata().unwrap());
    let rel_address = boot_image_with_metadata(rel_address_source.elf_metadata().unwrap());
    let plt_rel = boot_image_with_metadata(plt_rel_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), rela_size.entry());
    assert_eq!(baseline.segments(), rela_size.segments());
    assert_eq!(baseline.segments(), rel_address.segments());
    assert_eq!(baseline.segments(), plt_rel.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let rela_size_manifest = WorkloadManifest::builder(id("same"), rela_size)
        .build()
        .unwrap();
    let rel_address_manifest = WorkloadManifest::builder(id("same"), rel_address)
        .build()
        .unwrap();
    let plt_rel_manifest = WorkloadManifest::builder(id("same"), plt_rel)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), rela_size_manifest.identity());
    assert_ne!(
        baseline_manifest.identity(),
        rel_address_manifest.identity()
    );
    assert_ne!(baseline_manifest.identity(), plt_rel_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_interpreter_metadata() {
    let musl = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-musl-riscv64.so.1",
    ))
    .unwrap();
    let glibc = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-linux-riscv64-lp64d.so.1",
    ))
    .unwrap();

    assert_eq!(musl.entry(), glibc.entry());
    assert_eq!(musl.segments(), glibc.segments());
    assert_eq!(musl.elf_metadata(), glibc.elf_metadata());
    assert_ne!(musl.elf_interpreter(), glibc.elf_interpreter());

    let musl_manifest = WorkloadManifest::builder(id("same"), musl).build().unwrap();
    let glibc_manifest = WorkloadManifest::builder(id("same"), glibc)
        .build()
        .unwrap();

    assert_ne!(musl_manifest.identity(), glibc_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_endian_metadata() {
    let little = BootImage::from_elf(&elf64_image(2)).unwrap();
    let big = BootImage::from_elf(&elf64_be_image(2)).unwrap();

    assert_eq!(little.entry(), big.entry());
    assert_eq!(little.segments(), big.segments());
    assert_eq!(
        little.elf_metadata().unwrap().endian(),
        BootElfEndian::Little
    );
    assert_eq!(big.elf_metadata().unwrap().endian(), BootElfEndian::Big);
    assert_ne!(little.elf_metadata(), big.elf_metadata());

    let little_manifest = WorkloadManifest::builder(id("same"), little)
        .build()
        .unwrap();
    let big_manifest = WorkloadManifest::builder(id("same"), big).build().unwrap();

    assert_ne!(little_manifest.identity(), big_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_power64_endian_default_abi() {
    let little = BootImage::from_elf(&elf64_image(21)).unwrap();
    let big = BootImage::from_elf(&elf64_be_image(21)).unwrap();

    assert_eq!(little.entry(), big.entry());
    assert_eq!(little.segments(), big.segments());
    assert_eq!(
        little.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV2,
    );
    assert_eq!(
        big.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV1,
    );

    let little_manifest = WorkloadManifest::builder(id("same"), little)
        .build()
        .unwrap();
    let big_manifest = WorkloadManifest::builder(id("same"), big).build().unwrap();

    assert_ne!(little_manifest.identity(), big_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_operating_system_metadata() {
    let mut linux_bytes = elf64_image(243);
    linux_bytes[7] = 3;
    let mut freebsd_bytes = elf64_image(243);
    freebsd_bytes[7] = 9;

    let linux = BootImage::from_elf64_le(&linux_bytes).unwrap();
    let freebsd = BootImage::from_elf64_le(&freebsd_bytes).unwrap();

    assert_eq!(linux.entry(), freebsd.entry());
    assert_eq!(linux.segments(), freebsd.segments());
    assert_eq!(
        linux.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::Linux,
    );
    assert_eq!(
        freebsd.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::FreeBsd,
    );

    let linux_manifest = WorkloadManifest::builder(id("same"), linux)
        .build()
        .unwrap();
    let freebsd_manifest = WorkloadManifest::builder(id("same"), freebsd)
        .build()
        .unwrap();

    assert_ne!(linux_manifest.identity(), freebsd_manifest.identity());
}
