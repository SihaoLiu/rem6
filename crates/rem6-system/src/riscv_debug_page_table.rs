use std::fmt::Write as _;

use rem6_memory::{TranslationPageMap, TranslationPageMappingScope, TranslationPagePermissions};

pub fn riscv_gdb_page_table_dump_from_translation_map(map: &TranslationPageMap) -> Vec<u8> {
    let mut dump = String::new();
    writeln!(dump, "page_size={:#x}", map.page_size().bytes())
        .expect("page table dump writes into string");
    for mapping in map.mappings() {
        writeln!(
            dump,
            "vaddr={:#x} paddr={:#x} pages={} flags={} scope={}",
            mapping.virtual_start().get(),
            mapping.physical_start().get(),
            mapping.page_count(),
            riscv_gdb_page_permission_flags(mapping.permissions()),
            riscv_gdb_page_mapping_scope(mapping.scope()),
        )
        .expect("page table dump writes into string");
    }
    dump.into_bytes()
}

fn riscv_gdb_page_permission_flags(permissions: TranslationPagePermissions) -> &'static str {
    match (
        permissions.read(),
        permissions.write(),
        permissions.execute(),
    ) {
        (false, false, false) => "---",
        (false, false, true) => "--x",
        (false, true, false) => "-w-",
        (false, true, true) => "-wx",
        (true, false, false) => "r--",
        (true, false, true) => "r-x",
        (true, true, false) => "rw-",
        (true, true, true) => "rwx",
    }
}

fn riscv_gdb_page_mapping_scope(scope: TranslationPageMappingScope) -> &'static str {
    match scope {
        TranslationPageMappingScope::Global => "global",
        TranslationPageMappingScope::NonGlobal => "non-global",
    }
}
