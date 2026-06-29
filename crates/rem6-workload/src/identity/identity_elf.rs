use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfDynamicPltRelocationKind, BootElfEndian,
    BootElfMetadata, BootElfOperatingSystem,
};

use super::{hash_str, hash_u64};

pub(super) fn hash_elf_metadata(hash: &mut u64, metadata: Option<&BootElfMetadata>) {
    match metadata {
        Some(metadata) => {
            hash_u64(hash, 1);
            hash_elf_class(hash, metadata.class());
            hash_elf_endian(hash, metadata.endian());
            hash_u64(hash, u64::from(metadata.machine()));
            hash_u64(hash, u64::from(metadata.os_abi()));
            hash_u64(hash, u64::from(metadata.flags()));
            hash_elf_architecture(hash, metadata.architecture());
            hash_elf_operating_system(hash, metadata.operating_system());
            if metadata.has_tls() {
                hash_str(hash, "elf.tls");
            }
            if metadata.note_segment_count() != 0 {
                hash_str(hash, "elf.notes");
                hash_u64(hash, metadata.note_segment_count());
                hash_u64(hash, metadata.note_file_size());
            }
            if let Some(executable) = metadata.gnu_stack_executable() {
                hash_str(hash, "elf.gnu_stack");
                hash_u64(hash, u64::from(executable));
            }
            if let Some(address) = metadata.gnu_relro_virtual_address() {
                hash_str(hash, "elf.gnu_relro");
                hash_u64(hash, address.get());
                hash_u64(hash, metadata.gnu_relro_memory_size().unwrap_or(0));
            }
            if let Some(address) = metadata.gnu_eh_frame_virtual_address() {
                hash_str(hash, "elf.gnu_eh_frame");
                hash_u64(hash, address.get());
                hash_u64(hash, metadata.gnu_eh_frame_memory_size().unwrap_or(0));
            }
            if let Some(address) = metadata.gnu_property_virtual_address() {
                hash_str(hash, "elf.gnu_property");
                hash_u64(hash, address.get());
                hash_u64(hash, metadata.gnu_property_memory_size().unwrap_or(0));
            }
            let symbols = [
                metadata.symbol_count(),
                metadata.function_symbol_count(),
                metadata.object_symbol_count(),
            ];
            if symbols.iter().any(|value| *value != 0) {
                hash_str(hash, "elf.symbols");
                symbols.iter().for_each(|value| hash_u64(hash, *value));
            }
            let section_header_table = metadata.section_header_table();
            if section_header_table.file_offset() != 0 || section_header_table.entry_count() != 0 {
                hash_str(hash, "elf.section_header_table");
                hash_u64(hash, section_header_table.file_offset());
                hash_u64(hash, u64::from(section_header_table.entry_size()));
                hash_u64(hash, section_header_table.entry_count());
                hash_u64(hash, section_header_table.string_table_index());
            }
            let section_name_table = metadata.section_name_table();
            if section_name_table.file_offset() != 0 || section_name_table.byte_size() != 0 {
                hash_str(hash, "elf.section_name_table");
                hash_u64(hash, section_name_table.file_offset());
                hash_u64(hash, section_name_table.byte_size());
            }
            let section_flags = metadata.section_flags();
            if section_flags.allocated_count() != 0
                || section_flags.writable_count() != 0
                || section_flags.executable_count() != 0
                || section_flags.nobits_count() != 0
            {
                hash_str(hash, "elf.section_flags");
                hash_u64(hash, section_flags.allocated_count());
                hash_u64(hash, section_flags.writable_count());
                hash_u64(hash, section_flags.executable_count());
                hash_u64(hash, section_flags.nobits_count());
            }
            let section_storage = metadata.section_storage();
            if section_storage.file_backed_bytes() != 0
                || section_storage.allocated_bytes() != 0
                || section_storage.writable_bytes() != 0
                || section_storage.executable_bytes() != 0
                || section_storage.nobits_bytes() != 0
            {
                hash_str(hash, "elf.section_storage");
                hash_u64(hash, section_storage.file_backed_bytes());
                hash_u64(hash, section_storage.allocated_bytes());
                hash_u64(hash, section_storage.writable_bytes());
                hash_u64(hash, section_storage.executable_bytes());
                hash_u64(hash, section_storage.nobits_bytes());
            }
            let section_address_range = metadata.section_address_range();
            if section_address_range.start_address().is_some()
                || section_address_range.end_address().is_some()
            {
                hash_str(hash, "elf.section_address_range");
                hash_u64(
                    hash,
                    section_address_range
                        .start_address()
                        .map_or(u64::MAX, |address| address.get()),
                );
                hash_u64(
                    hash,
                    section_address_range
                        .end_address()
                        .map_or(u64::MAX, |address| address.get()),
                );
            }
            let section_alignment = metadata.section_alignment();
            if section_alignment.max_alignment() != 0
                || section_alignment.allocated_max_alignment() != 0
                || section_alignment.misaligned_allocated_count() != 0
            {
                hash_str(hash, "elf.section_alignment");
                hash_u64(hash, section_alignment.max_alignment());
                hash_u64(hash, section_alignment.allocated_max_alignment());
                hash_u64(hash, section_alignment.misaligned_allocated_count());
            }
            let dynamic = metadata.dynamic_table();
            if dynamic.segment_count() != 0 {
                let address = dynamic.virtual_address().map_or(u64::MAX, |a| a.get());
                hash_str(hash, "elf.dynamic");
                for value in [
                    dynamic.segment_count(),
                    dynamic.file_offset().unwrap_or(u64::MAX),
                    address,
                    u64::from(dynamic.entry_size()),
                    dynamic.entry_count(),
                    dynamic.needed_count(),
                ] {
                    hash_u64(hash, value);
                }
                for library in dynamic.needed_libraries() {
                    hash_str(hash, library);
                }
                if let Some(soname) = dynamic.soname() {
                    hash_str(hash, "elf.dynamic.soname");
                    hash_str(hash, soname);
                }
                for rpath in dynamic.rpath() {
                    hash_str(hash, "elf.dynamic.rpath");
                    hash_str(hash, rpath);
                }
                for runpath in dynamic.runpath() {
                    hash_str(hash, "elf.dynamic.runpath");
                    hash_str(hash, runpath);
                }
                for auxiliary in dynamic.auxiliary_libraries() {
                    hash_str(hash, "elf.dynamic.auxiliary");
                    hash_str(hash, auxiliary);
                }
                for filter in dynamic.filter_libraries() {
                    hash_str(hash, "elf.dynamic.filter");
                    hash_str(hash, filter);
                }
                for audit in dynamic.audit_libraries() {
                    hash_str(hash, "elf.dynamic.audit");
                    hash_str(hash, audit);
                }
                for dependency_audit in dynamic.dependency_audit_libraries() {
                    hash_str(hash, "elf.dynamic.dependency_audit");
                    hash_str(hash, dependency_audit);
                }
                if let Some(address) = dynamic.string_table_virtual_address() {
                    hash_str(hash, "elf.dynamic.strtab");
                    hash_u64(hash, address.get());
                }
                if let Some(size) = dynamic.string_table_size() {
                    hash_str(hash, "elf.dynamic.strsz");
                    hash_u64(hash, size);
                }
                if let Some(address) = dynamic.symbol_table_virtual_address() {
                    hash_str(hash, "elf.dynamic.symtab");
                    hash_u64(hash, address.get());
                }
                if let Some(size) = dynamic.symbol_table_entry_size() {
                    hash_str(hash, "elf.dynamic.syment");
                    hash_u64(hash, size);
                }
                if let Some(address) = dynamic.init_virtual_address() {
                    hash_str(hash, "elf.dynamic.init");
                    hash_u64(hash, address.get());
                }
                if let Some(address) = dynamic.fini_virtual_address() {
                    hash_str(hash, "elf.dynamic.fini");
                    hash_u64(hash, address.get());
                }
                if let Some(address) = dynamic.init_array_virtual_address() {
                    hash_str(hash, "elf.dynamic.init_array");
                    hash_u64(hash, address.get());
                }
                if let Some(size) = dynamic.init_array_size() {
                    hash_str(hash, "elf.dynamic.init_arraysz");
                    hash_u64(hash, size);
                }
                if let Some(address) = dynamic.fini_array_virtual_address() {
                    hash_str(hash, "elf.dynamic.fini_array");
                    hash_u64(hash, address.get());
                }
                if let Some(size) = dynamic.fini_array_size() {
                    hash_str(hash, "elf.dynamic.fini_arraysz");
                    hash_u64(hash, size);
                }
                if let Some(address) = dynamic.preinit_array_virtual_address() {
                    hash_str(hash, "elf.dynamic.preinit_array");
                    hash_u64(hash, address.get());
                }
                if let Some(size) = dynamic.preinit_array_size() {
                    hash_str(hash, "elf.dynamic.preinit_arraysz");
                    hash_u64(hash, size);
                }
                if let Some(flags) = dynamic.flags() {
                    hash_str(hash, "elf.dynamic.dt_flags");
                    hash_u64(hash, flags);
                }
                if let Some(flags_1) = dynamic.flags_1() {
                    hash_str(hash, "elf.dynamic.dt_flags_1");
                    hash_u64(hash, flags_1);
                }
                for (label, address) in [
                    ("elf.dynamic.plt_got", dynamic.plt_got_virtual_address()),
                    ("elf.dynamic.debug", dynamic.debug_virtual_address()),
                ] {
                    if let Some(address) = address {
                        hash_str(hash, label);
                        hash_u64(hash, address.get());
                    }
                }
                for (label, enabled) in [
                    ("elf.dynamic.symbolic", dynamic.has_symbolic_binding()),
                    ("elf.dynamic.textrel", dynamic.has_text_relocations()),
                    ("elf.dynamic.bind_now", dynamic.bind_now()),
                ] {
                    if enabled {
                        hash_str(hash, label);
                    }
                }
                for (label, count) in [
                    (
                        "elf.dynamic.relative_relocations.rela",
                        dynamic.rela_relative_count(),
                    ),
                    (
                        "elf.dynamic.relative_relocations.rel",
                        dynamic.rel_relative_count(),
                    ),
                ] {
                    if let Some(count) = count {
                        hash_str(hash, label);
                        hash_u64(hash, count);
                    }
                }
                for (label, address) in [
                    ("elf.dynamic.hash.sysv", dynamic.sysv_hash_virtual_address()),
                    ("elf.dynamic.hash.gnu", dynamic.gnu_hash_virtual_address()),
                    (
                        "elf.dynamic.version.symbols",
                        dynamic.version_symbol_table_virtual_address(),
                    ),
                    (
                        "elf.dynamic.version.definitions",
                        dynamic.version_definition_table_virtual_address(),
                    ),
                    (
                        "elf.dynamic.version.needed",
                        dynamic.version_needed_table_virtual_address(),
                    ),
                ] {
                    if let Some(address) = address {
                        hash_str(hash, label);
                        hash_u64(hash, address.get());
                    }
                }
                for (label, count) in [
                    (
                        "elf.dynamic.version.definitions.count",
                        dynamic.version_definition_count(),
                    ),
                    (
                        "elf.dynamic.version.needed.count",
                        dynamic.version_needed_count(),
                    ),
                ] {
                    if let Some(count) = count {
                        hash_str(hash, label);
                        hash_u64(hash, count);
                    }
                }
                for (label, table) in [
                    ("elf.dynamic.rela", dynamic.rela_relocations()),
                    ("elf.dynamic.rel", dynamic.rel_relocations()),
                    ("elf.dynamic.plt", dynamic.plt_relocations()),
                ] {
                    hash_str(hash, label);
                    hash_u64(hash, table.virtual_address().map_or(u64::MAX, |a| a.get()));
                    hash_u64(hash, table.byte_size());
                    hash_u64(hash, table.entry_size());
                    hash_u64(hash, table.entry_count());
                }
                match dynamic.plt_relocation_kind() {
                    Some(BootElfDynamicPltRelocationKind::Rel) => {
                        hash_str(hash, "elf.dynamic.plt.rel")
                    }
                    Some(BootElfDynamicPltRelocationKind::Rela) => {
                        hash_str(hash, "elf.dynamic.plt.rela")
                    }
                    None => {}
                }
            }
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_elf_class(hash: &mut u64, class: BootElfClass) {
    let value = match class {
        BootElfClass::Class32 => 1,
        BootElfClass::Class64 => 2,
    };
    hash_u64(hash, value);
}

fn hash_elf_endian(hash: &mut u64, endian: BootElfEndian) {
    let value = match endian {
        BootElfEndian::Little => 1,
        BootElfEndian::Big => 2,
    };
    hash_u64(hash, value);
}

fn hash_elf_architecture(hash: &mut u64, architecture: BootElfArchitecture) {
    match architecture {
        BootElfArchitecture::Sparc32 => hash_u64(hash, 1),
        BootElfArchitecture::Sparc64 => hash_u64(hash, 2),
        BootElfArchitecture::Mips => hash_u64(hash, 3),
        BootElfArchitecture::I386 => hash_u64(hash, 4),
        BootElfArchitecture::X8664 => hash_u64(hash, 5),
        BootElfArchitecture::Arm => hash_u64(hash, 6),
        BootElfArchitecture::Thumb => hash_u64(hash, 7),
        BootElfArchitecture::Arm64 => hash_u64(hash, 8),
        BootElfArchitecture::Riscv32 => hash_u64(hash, 9),
        BootElfArchitecture::Riscv64 => hash_u64(hash, 10),
        BootElfArchitecture::Power => hash_u64(hash, 11),
        BootElfArchitecture::Power64 => hash_u64(hash, 12),
        BootElfArchitecture::Unknown { machine, class } => {
            hash_u64(hash, 13);
            hash_u64(hash, u64::from(machine));
            hash_elf_class(hash, class);
        }
    }
}

fn hash_elf_operating_system(hash: &mut u64, operating_system: BootElfOperatingSystem) {
    match operating_system {
        BootElfOperatingSystem::Linux => hash_u64(hash, 1),
        BootElfOperatingSystem::Solaris => hash_u64(hash, 2),
        BootElfOperatingSystem::Tru64 => hash_u64(hash, 3),
        BootElfOperatingSystem::LinuxArmOabi => hash_u64(hash, 4),
        BootElfOperatingSystem::LinuxPower64AbiV1 => hash_u64(hash, 5),
        BootElfOperatingSystem::LinuxPower64AbiV2 => hash_u64(hash, 6),
        BootElfOperatingSystem::FreeBsd => hash_u64(hash, 7),
        BootElfOperatingSystem::Unknown { os_abi } => {
            hash_u64(hash, 8);
            hash_u64(hash, u64::from(os_abi));
        }
    }
}
