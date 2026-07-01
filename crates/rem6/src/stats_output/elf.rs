use rem6_boot::{BootElfDynamicTable, BootElfInterpreter, BootElfMetadata};
use rem6_memory::Address;
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{increment_stat, Rem6CliError};

pub(super) fn emit_elf_run_stats(
    stats: &mut StatsRegistry,
    binary_bytes: u64,
    metadata: &BootElfMetadata,
    interpreter: Option<&BootElfInterpreter>,
) -> Result<(), Rem6CliError> {
    let load_segments = metadata.load_segments();
    increment_stat(
        stats,
        "sim.binary.bytes",
        "Byte",
        StatResetPolicy::Constant,
        binary_bytes,
    )?;
    increment_stat(
        stats,
        "sim.elf.load_segments",
        "Count",
        StatResetPolicy::Constant,
        load_segments.count(),
    )?;
    emit_elf_load_segment_stats(stats, metadata)?;
    increment_stat(
        stats,
        "sim.elf.machine",
        "Value",
        StatResetPolicy::Constant,
        u64::from(metadata.machine()),
    )?;
    increment_stat(
        stats,
        "sim.elf.flags",
        "Value",
        StatResetPolicy::Constant,
        u64::from(metadata.flags()),
    )?;
    increment_stat(
        stats,
        "sim.elf.tls",
        "Count",
        StatResetPolicy::Constant,
        u64::from(metadata.has_tls()),
    )?;
    increment_stat(
        stats,
        "sim.elf.notes.segments",
        "Count",
        StatResetPolicy::Constant,
        metadata.note_segment_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.notes.bytes",
        "Byte",
        StatResetPolicy::Constant,
        metadata.note_file_size(),
    )?;
    increment_stat(
        stats,
        "sim.elf.notes.sections",
        "Count",
        StatResetPolicy::Constant,
        metadata.note_section_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.notes.section_bytes",
        "Byte",
        StatResetPolicy::Constant,
        metadata.note_section_file_size(),
    )?;
    increment_stat(
        stats,
        "sim.elf.gnu_stack.present",
        "Count",
        StatResetPolicy::Constant,
        u64::from(metadata.gnu_stack_executable().is_some()),
    )?;
    increment_stat(
        stats,
        "sim.elf.gnu_stack.executable",
        "Count",
        StatResetPolicy::Constant,
        u64::from(metadata.gnu_stack_executable().unwrap_or(false)),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.gnu_relro",
        metadata.gnu_relro_virtual_address(),
        metadata.gnu_relro_memory_size(),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.gnu_eh_frame",
        metadata.gnu_eh_frame_virtual_address(),
        metadata.gnu_eh_frame_memory_size(),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.gnu_property",
        metadata.gnu_property_virtual_address(),
        metadata.gnu_property_memory_size(),
    )?;
    let symbols = metadata.symbol_summary();
    increment_stat(
        stats,
        "sim.elf.symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.function_symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.function_symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.ifunc_symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.ifunc_symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.object_symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.object_symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.tls_symbols",
        "Count",
        StatResetPolicy::Constant,
        symbols.tls_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.local_symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.local_symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.global_symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.global_symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.weak_symbols",
        "Count",
        StatResetPolicy::Constant,
        metadata.weak_symbol_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_section.undefined",
        "Count",
        StatResetPolicy::Constant,
        symbols.undefined_section_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_section.absolute",
        "Count",
        StatResetPolicy::Constant,
        symbols.absolute_section_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_section.common",
        "Count",
        StatResetPolicy::Constant,
        symbols.common_section_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_visibility.default",
        "Count",
        StatResetPolicy::Constant,
        symbols.default_visibility_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_visibility.internal",
        "Count",
        StatResetPolicy::Constant,
        symbols.internal_visibility_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_visibility.hidden",
        "Count",
        StatResetPolicy::Constant,
        symbols.hidden_visibility_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.symbol_visibility.protected",
        "Count",
        StatResetPolicy::Constant,
        symbols.protected_visibility_count(),
    )?;
    emit_elf_dynamic_stats(stats, metadata.dynamic_table())?;
    emit_elf_program_header_stats(stats, metadata)?;
    emit_elf_section_header_stats(stats, metadata)?;
    emit_elf_section_name_stats(stats, metadata)?;
    emit_elf_section_flags_stats(stats, metadata)?;
    emit_elf_section_storage_stats(stats, metadata)?;
    emit_elf_section_relocation_stats(stats, metadata)?;
    emit_elf_section_array_stats(stats, metadata)?;
    emit_elf_section_hash_stats(stats, metadata)?;
    emit_elf_section_group_stats(stats, metadata)?;
    emit_elf_section_address_stats(stats, metadata)?;
    emit_elf_section_alignment_stats(stats, metadata)?;
    emit_elf_interpreter_stats(stats, interpreter)
}

fn emit_elf_load_segment_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let load_segments = metadata.load_segments();
    for (name, unit, value) in [
        (
            "sim.elf.load_segment.file_bytes",
            "Byte",
            load_segments.file_bytes(),
        ),
        (
            "sim.elf.load_segment.memory_bytes",
            "Byte",
            load_segments.memory_bytes(),
        ),
        (
            "sim.elf.load_segment.writable",
            "Count",
            load_segments.writable_count(),
        ),
        (
            "sim.elf.load_segment.executable",
            "Count",
            load_segments.executable_count(),
        ),
        (
            "sim.elf.load_segment.max_alignment",
            "Byte",
            load_segments.max_alignment(),
        ),
        (
            "sim.elf.load_segment.misaligned_alignment",
            "Count",
            load_segments.misaligned_alignment_count(),
        ),
    ] {
        increment_stat(stats, name, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_dynamic_stats(
    stats: &mut StatsRegistry,
    dynamic_table: &BootElfDynamicTable,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.elf.dynamic.segments",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.segment_count(),
    )?;
    if let Some(file_offset) = dynamic_table.file_offset() {
        increment_stat(
            stats,
            "sim.elf.dynamic.file_offset",
            "Byte",
            StatResetPolicy::Constant,
            file_offset,
        )?;
    }
    if let Some(virtual_address) = dynamic_table.virtual_address() {
        increment_stat(
            stats,
            "sim.elf.dynamic.virtual_address",
            "Address",
            StatResetPolicy::Constant,
            virtual_address.get(),
        )?;
    }
    increment_stat(
        stats,
        "sim.elf.dynamic.entry_size",
        "Byte",
        StatResetPolicy::Constant,
        u64::from(dynamic_table.entry_size()),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.entries",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.entry_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.needed",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.needed_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.needed_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.needed_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.soname_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.soname_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.rpath_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.rpath_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.runpath_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.runpath_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.auxiliary",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.auxiliary_libraries().len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.auxiliary_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.auxiliary_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.filter",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.filter_libraries().len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.filter_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.filter_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.audit",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.audit_libraries().len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.audit_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.audit_name_bytes(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.dependency_audit",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.dependency_audit_libraries().len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.dependency_audit_name_bytes",
        "Byte",
        StatResetPolicy::Constant,
        dynamic_table.dependency_audit_name_bytes(),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.dynamic.string_table",
        dynamic_table.string_table_virtual_address(),
        dynamic_table.string_table_size(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.symbol_table",
        dynamic_table.symbol_table_virtual_address(),
    )?;
    if let Some(entry_size) = dynamic_table.symbol_table_entry_size() {
        increment_stat(
            stats,
            "sim.elf.dynamic.symbol_table.entry_size",
            "Byte",
            StatResetPolicy::Constant,
            entry_size,
        )?;
    }
    increment_optional_value_stat(stats, "sim.elf.dynamic.dt_flags", dynamic_table.flags())?;
    increment_optional_value_stat(stats, "sim.elf.dynamic.dt_flags_1", dynamic_table.flags_1())?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.plt_got",
        dynamic_table.plt_got_virtual_address(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.debug",
        dynamic_table.debug_virtual_address(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.symbolic",
        "Count",
        StatResetPolicy::Constant,
        u64::from(dynamic_table.has_symbolic_binding()),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.textrel",
        "Count",
        StatResetPolicy::Constant,
        u64::from(dynamic_table.has_text_relocations()),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.bind_now",
        "Count",
        StatResetPolicy::Constant,
        u64::from(dynamic_table.bind_now()),
    )?;
    increment_optional_count_stat(
        stats,
        "sim.elf.dynamic.relative_relocations.rela",
        dynamic_table.rela_relative_count(),
    )?;
    increment_optional_count_stat(
        stats,
        "sim.elf.dynamic.relative_relocations.rel",
        dynamic_table.rel_relative_count(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.init",
        dynamic_table.init_virtual_address(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.fini",
        dynamic_table.fini_virtual_address(),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.dynamic.init_array",
        dynamic_table.init_array_virtual_address(),
        dynamic_table.init_array_size(),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.dynamic.fini_array",
        dynamic_table.fini_array_virtual_address(),
        dynamic_table.fini_array_size(),
    )?;
    increment_optional_address_bytes_stats(
        stats,
        "sim.elf.dynamic.preinit_array",
        dynamic_table.preinit_array_virtual_address(),
        dynamic_table.preinit_array_size(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.hash.sysv",
        dynamic_table.sysv_hash_virtual_address(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.hash.gnu",
        dynamic_table.gnu_hash_virtual_address(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.version.symbols",
        dynamic_table.version_symbol_table_virtual_address(),
    )?;
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.version.definitions",
        dynamic_table.version_definition_table_virtual_address(),
    )?;
    if let Some(count) = dynamic_table.version_definition_count() {
        increment_stat(
            stats,
            "sim.elf.dynamic.version.definitions.entries",
            "Count",
            StatResetPolicy::Constant,
            count,
        )?;
    }
    increment_optional_address_stat(
        stats,
        "sim.elf.dynamic.version.needed",
        dynamic_table.version_needed_table_virtual_address(),
    )?;
    if let Some(count) = dynamic_table.version_needed_count() {
        increment_stat(
            stats,
            "sim.elf.dynamic.version.needed.entries",
            "Count",
            StatResetPolicy::Constant,
            count,
        )?;
    }
    increment_stat(
        stats,
        "sim.elf.dynamic.rela.entries",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.rela_entry_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.rel.entries",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.rel_entry_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.relr.entries",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.relr_entry_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.dynamic.plt_relocations.entries",
        "Count",
        StatResetPolicy::Constant,
        dynamic_table.plt_relocations().entry_count(),
    )
}

fn emit_elf_program_header_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let program_header_table = metadata.program_header_table();
    increment_stat(
        stats,
        "sim.elf.program_header.file_offset",
        "Byte",
        StatResetPolicy::Constant,
        program_header_table.file_offset(),
    )?;
    increment_stat(
        stats,
        "sim.elf.program_header.entry_size",
        "Byte",
        StatResetPolicy::Constant,
        u64::from(program_header_table.entry_size()),
    )?;
    increment_stat(
        stats,
        "sim.elf.program_header.entry_count",
        "Count",
        StatResetPolicy::Constant,
        program_header_table.entry_count(),
    )?;
    if let Some(memory_address) = program_header_table.memory_address() {
        increment_stat(
            stats,
            "sim.elf.program_header.memory_address",
            "Address",
            StatResetPolicy::Constant,
            memory_address.get(),
        )?;
    }
    Ok(())
}

fn emit_elf_section_header_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let section_header_table = metadata.section_header_table();
    increment_stat(
        stats,
        "sim.elf.section_header.file_offset",
        "Byte",
        StatResetPolicy::Constant,
        section_header_table.file_offset(),
    )?;
    increment_stat(
        stats,
        "sim.elf.section_header.entry_size",
        "Byte",
        StatResetPolicy::Constant,
        u64::from(section_header_table.entry_size()),
    )?;
    increment_stat(
        stats,
        "sim.elf.section_header.entry_count",
        "Count",
        StatResetPolicy::Constant,
        section_header_table.entry_count(),
    )?;
    increment_stat(
        stats,
        "sim.elf.section_header.string_table_index",
        "Count",
        StatResetPolicy::Constant,
        section_header_table.string_table_index(),
    )
}

fn emit_elf_section_name_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let section_name_table = metadata.section_name_table();
    increment_stat(
        stats,
        "sim.elf.section_name_table.file_offset",
        "Byte",
        StatResetPolicy::Constant,
        section_name_table.file_offset(),
    )?;
    increment_stat(
        stats,
        "sim.elf.section_name_table.bytes",
        "Byte",
        StatResetPolicy::Constant,
        section_name_table.byte_size(),
    )
}

fn emit_elf_section_flags_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let section_flags = metadata.section_flags();
    for (path, value) in [
        (
            "sim.elf.section_flags.allocated",
            section_flags.allocated_count(),
        ),
        (
            "sim.elf.section_flags.writable",
            section_flags.writable_count(),
        ),
        (
            "sim.elf.section_flags.executable",
            section_flags.executable_count(),
        ),
        ("sim.elf.section_flags.nobits", section_flags.nobits_count()),
    ] {
        increment_stat(stats, path, "Count", StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_section_storage_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let section_storage = metadata.section_storage();
    for (path, value, unit) in [
        (
            "sim.elf.section_storage.file_bytes",
            section_storage.file_backed_bytes(),
            "Byte",
        ),
        (
            "sim.elf.section_storage.allocated_bytes",
            section_storage.allocated_bytes(),
            "Byte",
        ),
        (
            "sim.elf.section_storage.writable_bytes",
            section_storage.writable_bytes(),
            "Byte",
        ),
        (
            "sim.elf.section_storage.executable_bytes",
            section_storage.executable_bytes(),
            "Byte",
        ),
        (
            "sim.elf.section_storage.nobits_bytes",
            section_storage.nobits_bytes(),
            "Byte",
        ),
        (
            "sim.elf.section_storage.string_tables",
            section_storage.string_table_count(),
            "Count",
        ),
        (
            "sim.elf.section_storage.string_table_bytes",
            section_storage.string_table_bytes(),
            "Byte",
        ),
    ] {
        increment_stat(stats, path, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_section_relocation_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let relocations = metadata.section_relocations();
    for (name, unit, value) in [
        (
            "sim.elf.section_relocations.sections",
            "Count",
            relocations.section_count(),
        ),
        (
            "sim.elf.section_relocations.bytes",
            "Byte",
            relocations.byte_size(),
        ),
        (
            "sim.elf.section_relocations.rela.sections",
            "Count",
            relocations.rela_section_count(),
        ),
        (
            "sim.elf.section_relocations.rela.entries",
            "Count",
            relocations.rela_entry_count(),
        ),
        (
            "sim.elf.section_relocations.rel.sections",
            "Count",
            relocations.rel_section_count(),
        ),
        (
            "sim.elf.section_relocations.rel.entries",
            "Count",
            relocations.rel_entry_count(),
        ),
        (
            "sim.elf.section_relocations.relr.sections",
            "Count",
            relocations.relr_section_count(),
        ),
        (
            "sim.elf.section_relocations.relr.entries",
            "Count",
            relocations.relr_entry_count(),
        ),
    ] {
        increment_stat(stats, name, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_section_array_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let arrays = metadata.section_arrays();
    for (name, unit, value) in [
        (
            "sim.elf.section_arrays.init.sections",
            "Count",
            arrays.init_array_section_count(),
        ),
        (
            "sim.elf.section_arrays.init.bytes",
            "Byte",
            arrays.init_array_bytes(),
        ),
        (
            "sim.elf.section_arrays.init.entries",
            "Count",
            arrays.init_array_entry_count(),
        ),
        (
            "sim.elf.section_arrays.fini.sections",
            "Count",
            arrays.fini_array_section_count(),
        ),
        (
            "sim.elf.section_arrays.fini.bytes",
            "Byte",
            arrays.fini_array_bytes(),
        ),
        (
            "sim.elf.section_arrays.fini.entries",
            "Count",
            arrays.fini_array_entry_count(),
        ),
        (
            "sim.elf.section_arrays.preinit.sections",
            "Count",
            arrays.preinit_array_section_count(),
        ),
        (
            "sim.elf.section_arrays.preinit.bytes",
            "Byte",
            arrays.preinit_array_bytes(),
        ),
        (
            "sim.elf.section_arrays.preinit.entries",
            "Count",
            arrays.preinit_array_entry_count(),
        ),
    ] {
        increment_stat(stats, name, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_section_hash_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let hashes = metadata.section_hashes();
    for (name, unit, value) in [
        (
            "sim.elf.section_hashes.sysv.sections",
            "Count",
            hashes.sysv_section_count(),
        ),
        (
            "sim.elf.section_hashes.sysv.bytes",
            "Byte",
            hashes.sysv_bytes(),
        ),
        (
            "sim.elf.section_hashes.gnu.sections",
            "Count",
            hashes.gnu_section_count(),
        ),
        (
            "sim.elf.section_hashes.gnu.bytes",
            "Byte",
            hashes.gnu_bytes(),
        ),
    ] {
        increment_stat(stats, name, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_section_group_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let groups = metadata.section_groups();
    for (name, unit, value) in [
        (
            "sim.elf.section_groups.sections",
            "Count",
            groups.section_count(),
        ),
        ("sim.elf.section_groups.bytes", "Byte", groups.byte_size()),
        (
            "sim.elf.section_groups.entries",
            "Count",
            groups.entry_count(),
        ),
    ] {
        increment_stat(stats, name, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_section_address_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let range = metadata.section_address_range();
    if let Some(start_address) = range.start_address() {
        increment_stat(
            stats,
            "sim.elf.section_address.start",
            "Address",
            StatResetPolicy::Constant,
            start_address.get(),
        )?;
    }
    if let Some(end_address) = range.end_address() {
        increment_stat(
            stats,
            "sim.elf.section_address.end",
            "Address",
            StatResetPolicy::Constant,
            end_address.get(),
        )?;
    }
    Ok(())
}

fn emit_elf_section_alignment_stats(
    stats: &mut StatsRegistry,
    metadata: &BootElfMetadata,
) -> Result<(), Rem6CliError> {
    let alignment = metadata.section_alignment();
    for (name, unit, value) in [
        (
            "sim.elf.section_alignment.max",
            "Byte",
            alignment.max_alignment(),
        ),
        (
            "sim.elf.section_alignment.allocated_max",
            "Byte",
            alignment.allocated_max_alignment(),
        ),
        (
            "sim.elf.section_alignment.misaligned_allocated",
            "Count",
            alignment.misaligned_allocated_count(),
        ),
    ] {
        increment_stat(stats, name, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}

fn emit_elf_interpreter_stats(
    stats: &mut StatsRegistry,
    interpreter: Option<&BootElfInterpreter>,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.elf.interpreter.count",
        "Count",
        StatResetPolicy::Constant,
        u64::from(interpreter.is_some()),
    )?;
    if let Some(interpreter) = interpreter {
        increment_stat(
            stats,
            "sim.elf.interpreter.file_offset",
            "Byte",
            StatResetPolicy::Constant,
            interpreter.file_offset(),
        )?;
        increment_stat(
            stats,
            "sim.elf.interpreter.file_size",
            "Byte",
            StatResetPolicy::Constant,
            interpreter.file_size(),
        )?;
        increment_stat(
            stats,
            "sim.elf.interpreter.path_bytes",
            "Byte",
            StatResetPolicy::Constant,
            interpreter.path().len() as u64,
        )?;
    }
    Ok(())
}

pub(super) fn increment_optional_address_bytes_stats(
    stats: &mut StatsRegistry,
    path: &str,
    address: Option<Address>,
    bytes: Option<u64>,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{path}.present"),
        "Count",
        StatResetPolicy::Constant,
        u64::from(address.is_some()),
    )?;
    if let Some(address) = address {
        increment_stat(
            stats,
            &format!("{path}.virtual_address"),
            "Address",
            StatResetPolicy::Constant,
            address.get(),
        )?;
    }
    if let Some(bytes) = bytes {
        increment_stat(
            stats,
            &format!("{path}.bytes"),
            "Byte",
            StatResetPolicy::Constant,
            bytes,
        )?;
    }
    Ok(())
}

pub(super) fn increment_optional_address_stat(
    stats: &mut StatsRegistry,
    path: &str,
    address: Option<Address>,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{path}.present"),
        "Count",
        StatResetPolicy::Constant,
        u64::from(address.is_some()),
    )?;
    if let Some(address) = address {
        increment_stat(
            stats,
            &format!("{path}.virtual_address"),
            "Address",
            StatResetPolicy::Constant,
            address.get(),
        )?;
    }
    Ok(())
}

pub(super) fn increment_optional_value_stat(
    stats: &mut StatsRegistry,
    path: &str,
    value: Option<u64>,
) -> Result<(), Rem6CliError> {
    increment_optional_stat(stats, path, "Value", value)
}

fn increment_optional_count_stat(
    stats: &mut StatsRegistry,
    path: &str,
    value: Option<u64>,
) -> Result<(), Rem6CliError> {
    increment_optional_stat(stats, path, "Count", value)
}

fn increment_optional_stat(
    stats: &mut StatsRegistry,
    path: &str,
    unit: &str,
    value: Option<u64>,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{path}.present"),
        "Count",
        StatResetPolicy::Constant,
        u64::from(value.is_some()),
    )?;
    if let Some(value) = value {
        increment_stat(stats, path, unit, StatResetPolicy::Constant, value)?;
    }
    Ok(())
}
