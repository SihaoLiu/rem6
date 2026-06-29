use std::{
    env, fs,
    path::{Path, PathBuf},
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const PT_NOTE: u32 = 4;
const PT_PHDR: u32 = 6;
const PT_GNU_EH_FRAME: u32 = 0x6474_e550;
const PT_GNU_STACK: u32 = 0x6474_e551;
const PT_GNU_RELRO: u32 = 0x6474_e552;
const PT_GNU_PROPERTY: u32 = 0x6474_e553;
const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;
const SHF_EXECINSTR: u64 = 4;
const DT_PLTGOT: u64 = 3;
const DT_SYMTAB: u64 = 6;
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
const DT_FLAGS_1: u64 = 0x6fff_fffb;
const DT_VERDEF: u64 = 0x6fff_fffc;
const DT_VERDEFNUM: u64 = 0x6fff_fffd;
const DT_VERNEED: u64 = 0x6fff_fffe;
const DT_VERNEEDNUM: u64 = 0x6fff_ffff;
const DT_AUXILIARY: u64 = 0x7fff_fffd;
const DT_FILTER: u64 = 0x7fff_ffff;

pub(crate) const GEM5_READ_REQ: u32 = 1;
pub(crate) const GEM5_READ_RESP: u32 = 2;
pub(crate) const GEM5_WRITE_REQ: u32 = 4;
pub(crate) const GEM5_WRITE_RESP: u32 = 5;
pub(crate) const GEM5_WRITE_COMPLETE_RESP: u32 = 6;
pub(crate) const GEM5_CLEAN_SHARED_REQ: u32 = 42;
pub(crate) const GEM5_CLEAN_SHARED_RESP: u32 = 43;
pub(crate) const GEM5_INVALIDATE_REQ: u32 = 54;
pub(crate) const GEM5_INVALIDATE_RESP: u32 = 55;
pub(crate) const GEM5_MEM_FENCE_REQ: u32 = 38;
pub(crate) const GEM5_MEM_FENCE_RESP: u32 = 41;
pub(crate) const GEM5_READ_ERROR: u32 = 48;
pub(crate) const GEM5_WRITE_ERROR: u32 = 49;
pub(crate) const GEM5_PRINT_REQ: u32 = 52;
pub(crate) const GEM5_FLUSH_REQ: u32 = 53;
pub(crate) const GEM5_HTM_REQ: u32 = 56;
pub(crate) const GEM5_HTM_REQ_RESP: u32 = 57;
pub(crate) const GEM5_HTM_ABORT: u32 = 58;
pub(crate) const GEM5_TLBI_EXT_SYNC: u32 = 59;

#[derive(Clone, Copy)]
pub(crate) struct PacketFields {
    pub(crate) tick: u64,
    pub(crate) command: u32,
    pub(crate) address: Option<u64>,
    pub(crate) size: Option<u32>,
    pub(crate) packet_id: Option<u64>,
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn push_dynamic_string(strings: &mut Vec<u8>, value: &str) -> u64 {
    let offset = strings.len() as u64;
    strings.extend_from_slice(value.as_bytes());
    strings.push(0);
    offset
}

pub(crate) fn riscv64_elf(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let payload_offset = 128usize;
    let mut bytes = vec![0; payload_offset + payload.len()];
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
    write_u16(&mut bytes, 56, 1);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, physical);
    write_u64(&mut bytes, 88, physical);
    write_u64(&mut bytes, 96, payload.len() as u64);
    write_u64(&mut bytes, 104, payload.len() as u64);
    write_u64(&mut bytes, 112, 0x1000);
    bytes[payload_offset..payload_offset + payload.len()].copy_from_slice(payload);
    bytes
}

pub(crate) fn riscv64_elf_with_interpreter(
    entry: u64,
    physical: u64,
    payload: &[u8],
    interpreter: &str,
) -> Vec<u8> {
    let payload_offset = 0x200usize;
    let interpreter_offset = 0x180usize;
    let mut interpreter_bytes = interpreter.as_bytes().to_vec();
    interpreter_bytes.push(0);
    let mut bytes = vec![0; payload_offset + payload.len()];
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
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, physical);
    write_u64(&mut bytes, 88, physical);
    write_u64(&mut bytes, 96, payload.len() as u64);
    write_u64(&mut bytes, 104, payload.len() as u64);
    write_u64(&mut bytes, 112, 0x1000);

    write_u32(&mut bytes, 120, 3);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, interpreter_offset as u64);
    write_u64(&mut bytes, 136, 0);
    write_u64(&mut bytes, 144, 0);
    write_u64(&mut bytes, 152, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 160, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 168, 1);

    bytes[interpreter_offset..interpreter_offset + interpreter_bytes.len()]
        .copy_from_slice(&interpreter_bytes);
    bytes[payload_offset..].copy_from_slice(payload);
    bytes
}

pub(crate) fn riscv64_elf_with_tbss(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
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

pub(crate) fn riscv64_elf_with_pt_tls(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    riscv64_elf_with_extra_program_header(entry, physical, payload, 7, 4, 16)
}

pub(crate) fn riscv64_elf_with_gnu_stack(
    entry: u64,
    physical: u64,
    payload: &[u8],
    executable: bool,
) -> Vec<u8> {
    riscv64_elf_with_extra_program_header(
        entry,
        physical,
        payload,
        PT_GNU_STACK,
        if executable { 5 } else { 6 },
        0,
    )
}

pub(crate) fn riscv64_elf_with_gnu_relro(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    riscv64_elf_with_extra_program_header(entry, physical, payload, PT_GNU_RELRO, 4, 32)
}

pub(crate) fn riscv64_elf_with_gnu_eh_frame(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    riscv64_elf_with_extra_program_header(entry, physical, payload, PT_GNU_EH_FRAME, 4, 40)
}

pub(crate) fn riscv64_elf_with_gnu_property(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    riscv64_elf_with_extra_program_header(entry, physical, payload, PT_GNU_PROPERTY, 4, 48)
}

pub(crate) fn riscv64_elf_with_pt_phdr(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes =
        riscv64_elf_with_extra_program_header(entry, physical, payload, PT_PHDR, 4, 112);
    write_u64(&mut bytes, 128, 64);
    write_u64(&mut bytes, 136, physical + 0x1000);
    write_u64(&mut bytes, 144, physical + 0x2000);
    bytes
}

pub(crate) fn riscv64_elf_with_note_segment(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let payload_offset = 0x100usize;
    let note_offset = 0x180usize;
    let note_size = 24usize;
    let mut bytes = vec![0; note_offset + note_size];
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
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, physical);
    write_u64(&mut bytes, 88, physical);
    write_u64(&mut bytes, 96, payload.len() as u64);
    write_u64(&mut bytes, 104, payload.len() as u64);
    write_u64(&mut bytes, 112, 0x1000);

    write_u32(&mut bytes, 120, PT_NOTE);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, note_offset as u64);
    write_u64(&mut bytes, 136, physical + 0x1000);
    write_u64(&mut bytes, 144, physical + 0x1000);
    write_u64(&mut bytes, 152, note_size as u64);
    write_u64(&mut bytes, 160, note_size as u64);
    write_u64(&mut bytes, 168, 8);
    bytes[payload_offset..payload_offset + payload.len()].copy_from_slice(payload);
    bytes
}

fn riscv64_elf_with_extra_program_header(
    entry: u64,
    physical: u64,
    payload: &[u8],
    extra_kind: u32,
    extra_flags: u32,
    extra_memory_size: u64,
) -> Vec<u8> {
    let payload_offset = 0x100usize;
    let mut bytes = vec![0; payload_offset + payload.len()];
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
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, physical);
    write_u64(&mut bytes, 88, physical);
    write_u64(&mut bytes, 96, payload.len() as u64);
    write_u64(&mut bytes, 104, payload.len() as u64);
    write_u64(&mut bytes, 112, 0x1000);

    write_u32(&mut bytes, 120, extra_kind);
    write_u32(&mut bytes, 124, extra_flags);
    write_u64(&mut bytes, 128, 0);
    write_u64(&mut bytes, 136, physical + 0x1000);
    write_u64(&mut bytes, 144, physical + 0x1000);
    write_u64(&mut bytes, 152, 0);
    write_u64(&mut bytes, 160, extra_memory_size);
    write_u64(&mut bytes, 168, 8);
    bytes[payload_offset..payload_offset + payload.len()].copy_from_slice(payload);
    bytes
}

pub(crate) fn riscv64_elf_with_symbols(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    riscv64_elf_with_symbol_section(entry, physical, payload, ".symtab", 2, ".strtab")
}

pub(crate) fn riscv64_elf_with_dynamic_symbols(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    riscv64_elf_with_symbol_section(entry, physical, payload, ".dynsym", 11, ".dynstr")
}

fn riscv64_elf_with_symbol_section(
    entry: u64,
    physical: u64,
    payload: &[u8],
    symbol_section_name: &str,
    symbol_section_kind: u32,
    string_section_name: &str,
) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
    let symbol_names = b"\0entry_func\0data_obj\0";
    let symbol_names_offset = bytes.len();
    bytes.extend_from_slice(symbol_names);

    let symbol_table_offset = bytes.len();
    bytes.resize(bytes.len() + 3 * 24, 0);
    let function_base = symbol_table_offset + 24;
    write_u32(&mut bytes, function_base, 1);
    bytes[function_base + 4] = 0x12;
    write_u16(&mut bytes, function_base + 6, 1);
    write_u64(&mut bytes, function_base + 8, entry);
    write_u64(&mut bytes, function_base + 16, payload.len() as u64);
    let object_base = symbol_table_offset + 48;
    write_u32(&mut bytes, object_base, 12);
    bytes[object_base + 4] = 0x11;
    write_u16(&mut bytes, object_base + 6, 1);
    write_u64(&mut bytes, object_base + 8, physical + 0x1000);
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

pub(crate) fn riscv64_elf_with_dynamic_table(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let payload_offset = 0x300usize;
    let dynamic_offset = 0x180usize;
    let strtab_offset = 0x380usize;
    let strtab = b"\0libc.so.6\0libm.so.6\0librem6.so\0/opt/rem6/lib\0$ORIGIN/lib\0";
    let dynamic_size = 17 * 16usize;
    let mut bytes = vec![0; (payload_offset + payload.len()).max(strtab_offset + strtab.len())];
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
    write_u16(&mut bytes, 56, 3);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, payload_offset as u64);
    write_u64(&mut bytes, 80, physical);
    write_u64(&mut bytes, 88, physical);
    write_u64(&mut bytes, 96, payload.len() as u64);
    write_u64(&mut bytes, 104, payload.len() as u64);
    write_u64(&mut bytes, 112, 0x1000);

    write_u32(&mut bytes, 120, 2);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, dynamic_offset as u64);
    write_u64(&mut bytes, 136, physical + dynamic_offset as u64);
    write_u64(&mut bytes, 144, physical + dynamic_offset as u64);
    write_u64(&mut bytes, 152, dynamic_size as u64);
    write_u64(&mut bytes, 160, dynamic_size as u64);
    write_u64(&mut bytes, 168, 8);

    write_u32(&mut bytes, 176, 1);
    write_u32(&mut bytes, 180, 4);
    write_u64(&mut bytes, 184, strtab_offset as u64);
    write_u64(&mut bytes, 192, physical + strtab_offset as u64);
    write_u64(&mut bytes, 200, physical + strtab_offset as u64);
    write_u64(&mut bytes, 208, strtab.len() as u64);
    write_u64(&mut bytes, 216, strtab.len() as u64);
    write_u64(&mut bytes, 224, 8);

    write_u64(&mut bytes, dynamic_offset, 1);
    write_u64(&mut bytes, dynamic_offset + 8, 1);
    write_u64(&mut bytes, dynamic_offset + 16, 1);
    write_u64(&mut bytes, dynamic_offset + 24, 11);
    write_u64(&mut bytes, dynamic_offset + 32, 14);
    write_u64(&mut bytes, dynamic_offset + 40, 21);
    write_u64(&mut bytes, dynamic_offset + 48, 15);
    write_u64(&mut bytes, dynamic_offset + 56, 32);
    write_u64(&mut bytes, dynamic_offset + 64, 29);
    write_u64(&mut bytes, dynamic_offset + 72, 46);
    write_u64(&mut bytes, dynamic_offset + 80, 7);
    write_u64(&mut bytes, dynamic_offset + 88, physical + 0x300);
    write_u64(&mut bytes, dynamic_offset + 96, 8);
    write_u64(&mut bytes, dynamic_offset + 104, 48);
    write_u64(&mut bytes, dynamic_offset + 112, 9);
    write_u64(&mut bytes, dynamic_offset + 120, 24);
    write_u64(&mut bytes, dynamic_offset + 128, 17);
    write_u64(&mut bytes, dynamic_offset + 136, physical + 0x340);
    write_u64(&mut bytes, dynamic_offset + 144, 18);
    write_u64(&mut bytes, dynamic_offset + 152, 16);
    write_u64(&mut bytes, dynamic_offset + 160, 19);
    write_u64(&mut bytes, dynamic_offset + 168, 16);
    write_u64(&mut bytes, dynamic_offset + 176, 23);
    write_u64(&mut bytes, dynamic_offset + 184, physical + 0x360);
    write_u64(&mut bytes, dynamic_offset + 192, 2);
    write_u64(&mut bytes, dynamic_offset + 200, 24);
    write_u64(&mut bytes, dynamic_offset + 208, 20);
    write_u64(&mut bytes, dynamic_offset + 216, 7);
    write_u64(&mut bytes, dynamic_offset + 224, 5);
    write_u64(
        &mut bytes,
        dynamic_offset + 232,
        physical + strtab_offset as u64,
    );
    write_u64(&mut bytes, dynamic_offset + 240, 10);
    write_u64(&mut bytes, dynamic_offset + 248, strtab.len() as u64);
    write_u64(&mut bytes, dynamic_offset + 256, 0);
    write_u64(&mut bytes, dynamic_offset + 264, 0);
    bytes[payload_offset..payload_offset + payload.len()].copy_from_slice(payload);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(strtab);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_hashes(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let hash_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 19 * 16);
    write_u64(&mut bytes, 160, 19 * 16);
    write_u64(&mut bytes, hash_entry, 4);
    write_u64(&mut bytes, hash_entry + 8, physical + 0x3c0);
    write_u64(&mut bytes, hash_entry + 16, 0x6fff_fef5);
    write_u64(&mut bytes, hash_entry + 24, physical + 0x3e0);
    write_u64(&mut bytes, hash_entry + 32, 0);
    write_u64(&mut bytes, hash_entry + 40, 0);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_flags(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let flags_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 19 * 16);
    write_u64(&mut bytes, 160, 19 * 16);
    write_u64(&mut bytes, flags_entry, DT_FLAGS);
    write_u64(&mut bytes, flags_entry + 8, 0x15);
    write_u64(&mut bytes, flags_entry + 16, DT_FLAGS_1);
    write_u64(&mut bytes, flags_entry + 24, 0x8000_0001);
    write_u64(&mut bytes, flags_entry + 32, 0);
    write_u64(&mut bytes, flags_entry + 40, 0);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_lifecycle(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let lifecycle_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 19 * 16);
    write_u64(&mut bytes, 160, 19 * 16);
    write_u64(&mut bytes, lifecycle_entry, DT_INIT);
    write_u64(&mut bytes, lifecycle_entry + 8, physical + 0x3a0);
    write_u64(&mut bytes, lifecycle_entry + 16, DT_FINI);
    write_u64(&mut bytes, lifecycle_entry + 24, physical + 0x3b0);
    write_u64(&mut bytes, lifecycle_entry + 32, 0);
    write_u64(&mut bytes, lifecycle_entry + 40, 0);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_lifecycle_arrays(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let array_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 23 * 16);
    write_u64(&mut bytes, 160, 23 * 16);
    write_u64(&mut bytes, array_entry, DT_INIT_ARRAY);
    write_u64(&mut bytes, array_entry + 8, physical + 0x3a0);
    write_u64(&mut bytes, array_entry + 16, DT_INIT_ARRAYSZ);
    write_u64(&mut bytes, array_entry + 24, 24);
    write_u64(&mut bytes, array_entry + 32, DT_FINI_ARRAY);
    write_u64(&mut bytes, array_entry + 40, physical + 0x3c0);
    write_u64(&mut bytes, array_entry + 48, DT_FINI_ARRAYSZ);
    write_u64(&mut bytes, array_entry + 56, 16);
    write_u64(&mut bytes, array_entry + 64, DT_PREINIT_ARRAY);
    write_u64(&mut bytes, array_entry + 72, physical + 0x3e0);
    write_u64(&mut bytes, array_entry + 80, DT_PREINIT_ARRAYSZ);
    write_u64(&mut bytes, array_entry + 88, 8);
    write_u64(&mut bytes, array_entry + 96, 0);
    write_u64(&mut bytes, array_entry + 104, 0);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_symbol_table(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let symbol_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 19 * 16);
    write_u64(&mut bytes, 160, 19 * 16);
    write_u64(&mut bytes, symbol_entry, DT_SYMTAB);
    write_u64(&mut bytes, symbol_entry + 8, physical + 0x3a0);
    write_u64(&mut bytes, symbol_entry + 16, DT_SYMENT);
    write_u64(&mut bytes, symbol_entry + 24, 24);
    write_u64(&mut bytes, symbol_entry + 32, 0);
    write_u64(&mut bytes, symbol_entry + 40, 0);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_versioning(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let version_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 22 * 16);
    write_u64(&mut bytes, 160, 22 * 16);
    write_u64(&mut bytes, version_entry, DT_VERSYM);
    write_u64(&mut bytes, version_entry + 8, physical + 0x3a0);
    write_u64(&mut bytes, version_entry + 16, DT_VERDEF);
    write_u64(&mut bytes, version_entry + 24, physical + 0x3c0);
    write_u64(&mut bytes, version_entry + 32, DT_VERDEFNUM);
    write_u64(&mut bytes, version_entry + 40, 2);
    write_u64(&mut bytes, version_entry + 48, DT_VERNEED);
    write_u64(&mut bytes, version_entry + 56, physical + 0x3e0);
    write_u64(&mut bytes, version_entry + 64, DT_VERNEEDNUM);
    write_u64(&mut bytes, version_entry + 72, 3);
    write_u64(&mut bytes, version_entry + 80, 0);
    write_u64(&mut bytes, version_entry + 88, 0);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_loader_strings(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let strtab_offset = 0x380usize;
    let loader_entry = dynamic_offset + 16 * 16;
    let mut strtab = b"\0libc.so.6\0libm.so.6\0librem6.so\0/opt/rem6/lib\0$ORIGIN/lib\0".to_vec();
    let auxiliary_offset = push_dynamic_string(&mut strtab, "libbefore.so");
    let filter_offset = push_dynamic_string(&mut strtab, "libfilter.so");
    let audit_offset = push_dynamic_string(&mut strtab, "audit.so");
    let dependency_audit_offset = push_dynamic_string(&mut strtab, "depaudit.so");

    bytes.resize(bytes.len().max(strtab_offset + strtab.len()), 0);
    write_u64(&mut bytes, 208, strtab.len() as u64);
    write_u64(&mut bytes, 216, strtab.len() as u64);
    write_u64(&mut bytes, 152, 21 * 16);
    write_u64(&mut bytes, 160, 21 * 16);
    write_u64(&mut bytes, dynamic_offset + 248, strtab.len() as u64);
    write_u64(&mut bytes, loader_entry, DT_AUXILIARY);
    write_u64(&mut bytes, loader_entry + 8, auxiliary_offset);
    write_u64(&mut bytes, loader_entry + 16, DT_FILTER);
    write_u64(&mut bytes, loader_entry + 24, filter_offset);
    write_u64(&mut bytes, loader_entry + 32, DT_AUDIT);
    write_u64(&mut bytes, loader_entry + 40, audit_offset);
    write_u64(&mut bytes, loader_entry + 48, DT_DEPAUDIT);
    write_u64(&mut bytes, loader_entry + 56, dependency_audit_offset);
    write_u64(&mut bytes, loader_entry + 64, 0);
    write_u64(&mut bytes, loader_entry + 72, 0);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(&strtab);
    bytes
}

pub(crate) fn riscv64_elf_with_dynamic_linker_metadata(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf_with_dynamic_table(entry, physical, payload);
    let dynamic_offset = 0x180usize;
    let linker_entry = dynamic_offset + 16 * 16;
    write_u64(&mut bytes, 152, 24 * 16);
    write_u64(&mut bytes, 160, 24 * 16);
    write_u64(&mut bytes, linker_entry, DT_PLTGOT);
    write_u64(&mut bytes, linker_entry + 8, physical + 0x3a0);
    write_u64(&mut bytes, linker_entry + 16, DT_DEBUG);
    write_u64(&mut bytes, linker_entry + 24, physical + 0x3c0);
    write_u64(&mut bytes, linker_entry + 32, DT_SYMBOLIC);
    write_u64(&mut bytes, linker_entry + 40, 0);
    write_u64(&mut bytes, linker_entry + 48, DT_TEXTREL);
    write_u64(&mut bytes, linker_entry + 56, 0);
    write_u64(&mut bytes, linker_entry + 64, DT_BIND_NOW);
    write_u64(&mut bytes, linker_entry + 72, 0);
    write_u64(&mut bytes, linker_entry + 80, DT_RELACOUNT);
    write_u64(&mut bytes, linker_entry + 88, 4);
    write_u64(&mut bytes, linker_entry + 96, DT_RELCOUNT);
    write_u64(&mut bytes, linker_entry + 104, 5);
    write_u64(&mut bytes, linker_entry + 112, 0);
    write_u64(&mut bytes, linker_entry + 120, 0);
    bytes
}

pub(crate) fn riscv64_elf_extended_phnum(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 56, 0xffff);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 1);
    write_u16(&mut bytes, 62, 0);
    bytes.resize(section_table_offset + 64, 0);
    write_u32(&mut bytes, section_table_offset + 44, 1);
    bytes
}

pub(crate) fn riscv64_elf_with_section_header_table(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
    let names = b"\0.text\0.meta\0.shstrtab\0";
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(names);
    while bytes.len() % 8 != 0 {
        bytes.push(0);
    }

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 4);
    write_u16(&mut bytes, 62, 3);
    bytes.resize(section_table_offset + 4 * 64, 0);

    write_u32(&mut bytes, section_table_offset + 64, 1);
    write_u32(&mut bytes, section_table_offset + 68, 1);
    write_u64(
        &mut bytes,
        section_table_offset + 72,
        SHF_ALLOC | SHF_EXECINSTR,
    );
    write_u64(&mut bytes, section_table_offset + 80, physical);
    write_u64(&mut bytes, section_table_offset + 88, 128);
    write_u64(&mut bytes, section_table_offset + 96, payload.len() as u64);
    write_u64(&mut bytes, section_table_offset + 112, 0x1000);

    write_u32(&mut bytes, section_table_offset + 128, 7);
    write_u32(&mut bytes, section_table_offset + 132, 1);
    write_u64(
        &mut bytes,
        section_table_offset + 136,
        SHF_ALLOC | SHF_WRITE,
    );
    write_u64(&mut bytes, section_table_offset + 144, physical + 0x40);
    write_u64(&mut bytes, section_table_offset + 176, 8);

    write_u32(&mut bytes, section_table_offset + 192, 13);
    write_u32(&mut bytes, section_table_offset + 196, 3);
    write_u64(&mut bytes, section_table_offset + 216, shstr_offset as u64);
    write_u64(&mut bytes, section_table_offset + 224, names.len() as u64);
    bytes
}

pub(crate) fn riscv64_elf_extended_section_note_os(
    entry: u64,
    physical: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
    let mut note = vec![0; 32];
    write_u32(&mut note, 0, 4);
    write_u32(&mut note, 4, 16);
    write_u32(&mut note, 8, 1);
    note[12..16].copy_from_slice(b"GNU\0");
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    let names = b"\0.note.ABI-tag\0.shstrtab\0";
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(names);

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 0);
    write_u16(&mut bytes, 62, 0xffff);
    bytes.resize(section_table_offset + 3 * 64, 0);
    write_u64(&mut bytes, section_table_offset + 32, 3);
    write_u32(&mut bytes, section_table_offset + 40, 2);
    write_u32(&mut bytes, section_table_offset + 64, 1);
    write_u32(&mut bytes, section_table_offset + 68, 7);
    write_u64(&mut bytes, section_table_offset + 88, note_offset as u64);
    write_u64(&mut bytes, section_table_offset + 96, note.len() as u64);
    write_u32(&mut bytes, section_table_offset + 128, 15);
    write_u32(&mut bytes, section_table_offset + 132, 3);
    write_u64(&mut bytes, section_table_offset + 152, shstr_offset as u64);
    write_u64(&mut bytes, section_table_offset + 160, names.len() as u64);
    bytes
}

pub(crate) fn riscv32_elf(entry: u32, physical: u32, payload: &[u8]) -> Vec<u8> {
    let payload_offset = 128usize;
    let mut bytes = vec![0; payload_offset + payload.len()];
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
    write_u16(&mut bytes, 44, 1);

    write_u32(&mut bytes, 52, 1);
    write_u32(&mut bytes, 56, payload_offset as u32);
    write_u32(&mut bytes, 60, physical);
    write_u32(&mut bytes, 64, physical);
    write_u32(&mut bytes, 68, payload.len() as u32);
    write_u32(&mut bytes, 72, payload.len() as u32);
    write_u32(&mut bytes, 76, 5);
    write_u32(&mut bytes, 80, 0x1000);
    bytes[payload_offset..].copy_from_slice(payload);
    bytes
}

pub(crate) fn riscv32_elf_extended_phnum(entry: u32, physical: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = riscv32_elf(entry, physical, payload);
    let section_table_offset = bytes.len();
    write_u32(&mut bytes, 32, section_table_offset as u32);
    write_u16(&mut bytes, 44, 0xffff);
    write_u16(&mut bytes, 46, 40);
    write_u16(&mut bytes, 48, 1);
    write_u16(&mut bytes, 50, 0);
    bytes.resize(section_table_offset + 40, 0);
    write_u32(&mut bytes, section_table_offset + 28, 1);
    bytes
}

pub(crate) fn x86_64_elf(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = riscv64_elf(entry, physical, payload);
    write_u16(&mut bytes, 18, 62);
    bytes
}

pub(crate) fn riscv64_program(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

pub(crate) fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

pub(crate) fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

pub(crate) fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
}

pub(crate) fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
}

pub(crate) fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

pub(crate) fn atomic_type(
    funct5: u32,
    aq: bool,
    rl: bool,
    rs2: u8,
    rs1: u8,
    funct3: u32,
    rd: u8,
) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

pub(crate) fn csr_read(csr: u32, rd: u8) -> u32 {
    (csr << 20) | (0x2 << 12) | (u32::from(rd) << 7) | 0x73
}

pub(crate) fn temp_binary(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.elf", std::process::id()));
    fs::write(&path, bytes).unwrap();
    path
}

pub(crate) fn temp_output(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

pub(crate) fn temp_config(name: &str, text: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.toml", std::process::id()));
    fs::write(&path, text).unwrap();
    path
}

pub(crate) fn temp_workspace(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

pub(crate) fn temp_trace(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rem6-{name}-{}.pb", std::process::id()));
    fs::write(&path, bytes).unwrap();
    path
}

pub(crate) fn find_riscv_tool(name: &str) -> Option<PathBuf> {
    find_tool_on_path(name).or_else(|| {
        let module_candidate =
            Path::new("/mnt/nas0/software/riscv/riscv64-elf-ubuntu-24.04-gcc/bin").join(name);
        module_candidate.is_file().then_some(module_candidate)
    })
}

fn find_tool_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

pub(crate) fn packet_trace_bytes(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut bytes = GEM5_MAGIC.to_vec();
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    for packet in packets {
        let mut message = Vec::new();
        append_key(&mut message, 1, 0);
        append_varint(&mut message, packet.tick);
        append_key(&mut message, 2, 0);
        append_varint(&mut message, u64::from(packet.command));
        if let Some(address) = packet.address {
            append_key(&mut message, 3, 0);
            append_varint(&mut message, address);
        }
        if let Some(size) = packet.size {
            append_key(&mut message, 4, 0);
            append_varint(&mut message, u64::from(size));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        append_record(&mut bytes, &message);
    }
    bytes
}

fn append_record(bytes: &mut Vec<u8>, record: &[u8]) {
    append_varint(bytes, record.len() as u64);
    bytes.extend_from_slice(record);
}

fn append_key(bytes: &mut Vec<u8>, field: u64, wire_type: u8) {
    append_varint(bytes, (field << 3) | u64::from(wire_type));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

pub(crate) fn assert_stat(stdout: &str, path: &str, unit: &str, value: u64, reset_policy: &str) {
    let sample = stat_sample(stdout, path);
    let (scope, name) = stat_scope_and_name(path);
    let scope_json = scope
        .iter()
        .map(|segment| format!("\"{segment}\""))
        .collect::<Vec<_>>()
        .join(",");

    let expected_fields = [
        format!("\"path\":\"{path}\""),
        format!("\"scope\":[{scope_json}]"),
        format!("\"name\":\"{name}\""),
        format!("\"unit\":\"{unit}\""),
        format!("\"value\":{value}"),
        format!("\"reset_policy\":\"{reset_policy}\""),
        "\"description\":null".to_string(),
    ];
    for expected in expected_fields {
        assert!(
            sample.contains(&expected),
            "missing stat field {expected} in {sample}"
        );
    }
}

pub(crate) fn stat_path_segment(segment: &str) -> String {
    let mut output = String::new();
    for (index, character) in segment.chars().enumerate() {
        if index == 0 {
            if character.is_ascii_alphabetic() || character == '_' {
                output.push(character);
            } else {
                output.push('_');
                if character.is_ascii_alphanumeric() {
                    output.push(character);
                }
            }
        } else if character.is_ascii_alphanumeric() || character == '_' {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "_".to_string()
    } else {
        output
    }
}

pub(crate) fn assert_stat_greater_than(
    stdout: &str,
    path: &str,
    unit: &str,
    minimum: u64,
    reset_policy: &str,
) {
    let sample = stat_sample(stdout, path);
    assert!(
        sample.contains(&format!("\"unit\":\"{unit}\"")),
        "missing stat unit {unit} in {sample}"
    );
    assert!(
        sample.contains(&format!("\"reset_policy\":\"{reset_policy}\"")),
        "missing stat reset policy {reset_policy} in {sample}"
    );
    let value = stat_value(stdout, path);
    assert!(
        value > minimum,
        "expected {path} value greater than {minimum}, got {value} in {sample}"
    );
}

pub(crate) fn stat_value(stdout: &str, path: &str) -> u64 {
    let sample = stat_sample(stdout, path);
    let Some(value_tail) = sample.split("\"value\":").nth(1) else {
        panic!("missing stat value in {sample}");
    };
    let value_end = value_tail
        .find(',')
        .or_else(|| value_tail.find('}'))
        .expect("stat value terminator");
    value_tail[..value_end]
        .parse::<u64>()
        .expect("numeric stat value")
}

pub(crate) fn assert_histogram_stat(
    stdout: &str,
    path: &str,
    unit: &str,
    value: u64,
    reset_policy: &str,
    buckets: &[(u64, u64)],
) {
    let sample = stat_sample(stdout, path);
    let expected = [
        "\"kind\":\"histogram\"".to_string(),
        format!("\"unit\":\"{unit}\""),
        format!("\"value\":{value}"),
        format!("\"reset_policy\":\"{reset_policy}\""),
    ];
    for field in expected {
        assert!(
            sample.contains(&field),
            "missing stat field {field} in {sample}"
        );
    }
    for (bucket, count) in buckets {
        let expected_bucket = format!("{{\"bucket\":{bucket},\"count\":{count}}}");
        assert!(
            sample.contains(&expected_bucket),
            "missing histogram bucket {expected_bucket} in {sample}"
        );
    }
}

pub(crate) fn assert_stat_id(stdout: &str, path: &str, id: u64) {
    let sample = stat_sample(stdout, path);
    let expected = format!("\"id\":{id}");
    assert!(
        sample.contains(&expected),
        "missing stat field {expected} in {sample}"
    );
}

fn stat_sample<'a>(stdout: &'a str, path: &str) -> &'a str {
    let path_field = format!("\"path\":\"{path}\"");
    let path_index = stdout
        .find(&path_field)
        .unwrap_or_else(|| panic!("missing stat path {path} in {stdout}"));
    let sample_start = stdout[..path_index]
        .rfind('{')
        .unwrap_or_else(|| panic!("missing stat object start for {path} in {stdout}"));
    let sample_end = json_object_end(stdout, sample_start)
        .unwrap_or_else(|| panic!("missing stat object end for {path} in {stdout}"));
    &stdout[sample_start..sample_end]
}

fn json_object_end(json: &str, start: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in json[start..].bytes().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(start + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn stat_scope_and_name(path: &str) -> (Vec<&str>, &str) {
    let mut segments = path.split('.').collect::<Vec<_>>();
    let name = segments.pop().unwrap_or(path);
    (segments, name)
}

pub(crate) fn assert_transport_stats(
    stdout: &str,
    prefix: &str,
    requests: u64,
    round_trip_ticks: u64,
    max_round_trip_ticks: u64,
) {
    assert_stat(
        stdout,
        &format!("{prefix}.requests"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.request_arrivals"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.responses"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.response_arrivals"),
        "Count",
        requests,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.round_trip_ticks"),
        "Tick",
        round_trip_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_round_trip_ticks"),
        "Tick",
        max_round_trip_ticks,
        "monotonic",
    );
}
