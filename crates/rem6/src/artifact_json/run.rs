use rem6_boot::{
    BootElfDynamicPltRelocationKind, BootElfDynamicRelocationTable, BootElfDynamicTable,
    BootElfInterpreter, BootElfLoadSegments, BootElfProgramHeaderTable, BootElfSectionAddressRange,
    BootElfSectionAlignment, BootElfSectionArrays, BootElfSectionFlags, BootElfSectionGroups,
    BootElfSectionHashes, BootElfSectionHeaderTable, BootElfSectionNameTable,
    BootElfSectionRelocations, BootElfSectionStorage, BootElfSymbolSummary,
};
use rem6_fabric::FabricHopActivity;
use rem6_memory::Address;
use rem6_system::RiscvDataCacheProtocol;

use super::optional_count_json;
use super::parallel::empty_parallel_json;
use super::transport::empty_transport_json;
use crate::formatting::{
    elf_architecture_name, elf_class_name, elf_endian_name, elf_os_name, json_escape,
};
use crate::{
    CliCachePrefetcher, Rem6DramSummary, Rem6ExecutionSummary, Rem6HostActionSummary,
    Rem6LoadBlobSummary, Rem6MemoryResourceSummary, Rem6ReadfileSummary,
    Rem6RiscvSbiConsoleSummary, Rem6RunArtifact, Rem6RunFabricSummary, RequestedIsa,
    RunFabricConfig,
};

impl Rem6RunArtifact {
    pub fn to_json(&self) -> String {
        let simulation = match &self.execution {
            Some(execution) => {
                execution.to_simulation_json(
                    self.config.max_tick(),
                    self.config.max_instructions(),
                    self.config.memory_route_delay(),
                    self.config.host_event_delay(),
                    self.config.memory_system(),
                )
            }
            None => format!(
                "{{\"status\":\"loaded\",\"max_tick\":{},\"instruction_limit\":{},\"memory_route_delay\":{},\"host_event_delay\":{},\"executed_ticks\":0,\"cores\":{}}}",
                self.config.max_tick(),
                optional_count_json(self.config.max_instructions()),
                self.config.memory_route_delay(),
                self.config.host_event_delay(),
                self.config.cores(),
            ),
        };
        let parallel = match &self.execution {
            Some(execution) => execution.to_parallel_json(
                self.config.parallel_workers(),
                self.config.min_remote_delay(),
            ),
            None => empty_parallel_json(
                self.config.parallel_workers(),
                self.config.min_remote_delay(),
            ),
        };
        let cores = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_cores_json)
            .unwrap_or_else(|| "[]".to_string());
        let memory = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_memory_json)
            .unwrap_or_else(|| "[]".to_string());
        let memory_resources = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_memory_resources_json)
            .unwrap_or_else(|| Rem6MemoryResourceSummary::default().to_json());
        let riscv_guest_writes = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_guest_writes_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_unknown_syscalls = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_unknown_syscalls_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_console = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_console_json)
            .unwrap_or_else(|| Rem6RiscvSbiConsoleSummary::default().to_json());
        let riscv_sbi_timers = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_timers_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_hsm_events = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_hsm_events_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_hsm_wakes = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_hsm_wakes_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_hsm_statuses = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_hsm_statuses_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_ipis = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_ipis_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_rfences = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_rfences_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_rfence_completions = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_rfence_completions_json)
            .unwrap_or_else(|| "[]".to_string());
        let riscv_sbi_resets = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_riscv_sbi_resets_json)
            .unwrap_or_else(|| "[]".to_string());
        let host_actions = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_host_actions_json)
            .unwrap_or_else(|| Rem6HostActionSummary::default().to_json());
        let dram = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_dram_json)
            .unwrap_or_else(|| Rem6DramSummary::default().to_json());
        let transport = self
            .execution
            .as_ref()
            .map(Rem6ExecutionSummary::to_transport_json)
            .unwrap_or_else(empty_transport_json);
        let empty_fabric = Rem6RunFabricSummary::default();
        let fabric = self
            .execution
            .as_ref()
            .map(|execution| execution.to_fabric_json(self.config.fabric()))
            .unwrap_or_else(|| run_fabric_json(self.config.fabric(), &empty_fabric));
        let debug = self
            .execution
            .as_ref()
            .and_then(Rem6ExecutionSummary::debug_json_field)
            .unwrap_or_default();
        let load_blobs = self
            .load_blobs
            .iter()
            .map(Rem6LoadBlobSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let readfiles = self
            .readfiles
            .iter()
            .map(Rem6ReadfileSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let riscv_boot = if self.config.isa() == RequestedIsa::Riscv {
            format!(
                ",\"riscv_boot\":{{\"a0\":\"0x{:x}\",\"a1\":\"0x{:x}\",\"sbi\":{},\"se\":{}}},\"riscv_sbi_inputs\":{},\"riscv_se_inputs\":{}",
                self.config.riscv_boot_a0(),
                self.config.riscv_boot_a1(),
                self.config.riscv_sbi(),
                self.config.riscv_se(),
                riscv_sbi_inputs_json(&self.config),
                riscv_se_inputs_json(&self.config)
            )
        } else {
            String::new()
        };
        let instruction_cache_protocol =
            optional_riscv_cache_protocol_json(self.config.instruction_cache_protocol());
        let instruction_cache_l2_protocol =
            optional_riscv_cache_protocol_json(self.config.instruction_cache_l2_protocol());
        let instruction_cache_l3_protocol =
            optional_riscv_cache_protocol_json(self.config.instruction_cache_l3_protocol());
        let instruction_cache_prefetcher =
            optional_cache_prefetcher_json(self.config.instruction_cache_prefetcher());
        let data_cache_protocol =
            optional_riscv_cache_protocol_json(self.config.data_cache_protocol());
        let data_cache_l2_protocol =
            optional_riscv_cache_protocol_json(self.config.data_cache_l2_protocol());
        let data_cache_l3_protocol =
            optional_riscv_cache_protocol_json(self.config.data_cache_l3_protocol());
        let data_cache_prefetcher =
            optional_cache_prefetcher_json(self.config.data_cache_prefetcher());
        let interpreter = elf_interpreter_json(self.interpreter.as_ref());
        let kernel_resource = self
            .config
            .kernel_resource()
            .map(|selector| format!("\"{}\"", json_escape(&selector.source_name())))
            .unwrap_or_else(|| "null".to_string());
        let power_analysis = self
            .power_analysis
            .as_ref()
            .map(|artifact| format!(",\"power_analysis\":{}", artifact.to_json()))
            .unwrap_or_default();
        let gnu_relro = elf_address_size_json(
            self.metadata.gnu_relro_virtual_address(),
            self.metadata.gnu_relro_memory_size(),
        );
        let gnu_eh_frame = elf_address_size_json(
            self.metadata.gnu_eh_frame_virtual_address(),
            self.metadata.gnu_eh_frame_memory_size(),
        );
        let gnu_property = elf_address_size_json(
            self.metadata.gnu_property_virtual_address(),
            self.metadata.gnu_property_memory_size(),
        );
        let notes = elf_notes_json(
            self.metadata.note_segment_count(),
            self.metadata.note_file_size(),
            self.metadata.note_section_count(),
            self.metadata.note_section_file_size(),
        );
        let symbols = elf_symbol_summary_json(self.metadata.symbol_summary());
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"kernel_resource\":{},\"entry\":\"0x{:x}\",\"start_address\":\"0x{:x}\"{},\"instruction_cache_protocol\":{},\"instruction_cache_l2_protocol\":{},\"instruction_cache_l3_protocol\":{},\"instruction_cache_prefetcher\":{},\"data_cache_protocol\":{},\"data_cache_l2_protocol\":{},\"data_cache_l3_protocol\":{},\"data_cache_prefetcher\":{},\"load_blobs\":[{}],\"readfiles\":[{}],\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{},\"tls\":{},\"load_segments\":{},\"notes\":{},\"gnu_stack\":{},\"gnu_relro\":{},\"gnu_eh_frame\":{},\"gnu_property\":{},\"symbols\":{},\"dynamic\":{},\"program_header_table\":{},\"section_header_table\":{},\"section_name_table\":{},\"section_flags\":{},\"section_storage\":{},\"section_relocations\":{},\"section_arrays\":{},\"section_hashes\":{},\"section_groups\":{},\"section_address_range\":{},\"section_alignment\":{},\"interpreter\":{}}},\"simulation\":{},\"parallel\":{},\"cores\":{},\"memory\":{},\"memory_resources\":{},\"riscv_guest_writes\":{},\"riscv_unknown_syscalls\":{},\"riscv_sbi_console\":{},\"riscv_sbi_timers\":{},\"riscv_sbi_hsm_events\":{},\"riscv_sbi_hsm_wakes\":{},\"riscv_sbi_hsm_statuses\":{},\"riscv_sbi_ipis\":{},\"riscv_sbi_rfences\":{},\"riscv_sbi_rfence_completions\":{},\"riscv_sbi_resets\":{},\"host_actions\":{},\"dram\":{},\"transport\":{},\"fabric\":{}{},\"stats\":{}{}}}\n",
            self.schema,
            self.config.isa().as_str(),
            json_escape(&self.config.binary().display().to_string()),
            kernel_resource,
            self.entry,
            self.start_address,
            riscv_boot,
            instruction_cache_protocol,
            instruction_cache_l2_protocol,
            instruction_cache_l3_protocol,
            instruction_cache_prefetcher,
            data_cache_protocol,
            data_cache_l2_protocol,
            data_cache_l3_protocol,
            data_cache_prefetcher,
            load_blobs,
            readfiles,
            elf_class_name(self.metadata.class()),
            elf_endian_name(self.metadata.endian()),
            elf_architecture_name(self.metadata.architecture()),
            elf_os_name(self.metadata.operating_system()),
            self.metadata.machine(),
            self.metadata.flags(),
            self.metadata.has_tls(),
            elf_load_segments_json(self.metadata.load_segments()),
            notes,
            elf_gnu_stack_json(self.metadata.gnu_stack_executable()),
            gnu_relro,
            gnu_eh_frame,
            gnu_property,
            symbols,
            elf_dynamic_table_json(self.metadata.dynamic_table()),
            elf_program_header_table_json(self.metadata.program_header_table()),
            elf_section_header_table_json(self.metadata.section_header_table()),
            elf_section_name_table_json(self.metadata.section_name_table()),
            elf_section_flags_json(self.metadata.section_flags()),
            elf_section_storage_json(self.metadata.section_storage()),
            elf_section_relocations_json(self.metadata.section_relocations()),
            elf_section_arrays_json(self.metadata.section_arrays()),
            elf_section_hashes_json(self.metadata.section_hashes()),
            elf_section_groups_json(self.metadata.section_groups()),
            elf_section_address_range_json(self.metadata.section_address_range()),
            elf_section_alignment_json(self.metadata.section_alignment()),
            interpreter,
            simulation,
            parallel,
            cores,
            memory,
            memory_resources,
            riscv_guest_writes,
            riscv_unknown_syscalls,
            riscv_sbi_console,
            riscv_sbi_timers,
            riscv_sbi_hsm_events,
            riscv_sbi_hsm_wakes,
            riscv_sbi_hsm_statuses,
            riscv_sbi_ipis,
            riscv_sbi_rfences,
            riscv_sbi_rfence_completions,
            riscv_sbi_resets,
            host_actions,
            dram,
            transport,
            fabric,
            debug,
            self.stats_json,
            power_analysis,
        )
    }

    pub const fn binary_bytes(&self) -> u64 {
        self.binary_bytes
    }

    pub const fn load_segments(&self) -> u64 {
        self.load_segments
    }
}

fn elf_gnu_stack_json(executable: Option<bool>) -> String {
    executable
        .map(|executable| format!("{{\"executable\":{executable}}}"))
        .unwrap_or_else(|| "null".to_string())
}

fn elf_notes_json(
    segment_count: u64,
    file_size: u64,
    section_count: u64,
    section_file_size: u64,
) -> String {
    format!(
        "{{\"segments\":{segment_count},\"bytes\":{file_size},\"sections\":{section_count},\"section_bytes\":{section_file_size}}}"
    )
}

fn elf_load_segments_json(load_segments: BootElfLoadSegments) -> String {
    format!(
        "{{\"count\":{},\"file_bytes\":{},\"memory_bytes\":{},\"writable\":{},\"executable\":{},\"max_alignment\":{},\"misaligned_alignment\":{}}}",
        load_segments.count(),
        load_segments.file_bytes(),
        load_segments.memory_bytes(),
        load_segments.writable_count(),
        load_segments.executable_count(),
        load_segments.max_alignment(),
        load_segments.misaligned_alignment_count(),
    )
}

fn elf_address_size_json(virtual_address: Option<Address>, memory_size: Option<u64>) -> String {
    match (virtual_address, memory_size) {
        (Some(address), Some(bytes)) => {
            format!(
                "{{\"virtual_address\":\"0x{:x}\",\"bytes\":{bytes}}}",
                address.get()
            )
        }
        _ => "null".to_string(),
    }
}

fn elf_interpreter_json(interpreter: Option<&BootElfInterpreter>) -> String {
    interpreter
        .map(|interpreter| {
            format!(
                "{{\"path\":\"{}\",\"file_offset\":{},\"file_size\":{}}}",
                json_escape(interpreter.path()),
                interpreter.file_offset(),
                interpreter.file_size()
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn elf_symbol_summary_json(symbols: BootElfSymbolSummary) -> String {
    format!(
        "{{\"total\":{},\"functions\":{},\"objects\":{},\"local\":{},\"global\":{},\"weak\":{},\"visibility\":{{\"default\":{},\"internal\":{},\"hidden\":{},\"protected\":{}}}}}",
        symbols.total_count(),
        symbols.function_count(),
        symbols.object_count(),
        symbols.local_count(),
        symbols.global_count(),
        symbols.weak_count(),
        symbols.default_visibility_count(),
        symbols.internal_visibility_count(),
        symbols.hidden_visibility_count(),
        symbols.protected_visibility_count()
    )
}

fn elf_dynamic_table_json(table: &BootElfDynamicTable) -> String {
    let file_offset = table
        .file_offset()
        .map(|offset| offset.to_string())
        .unwrap_or_else(|| "null".to_string());
    let virtual_address = table
        .virtual_address()
        .map(|address| format!("\"0x{:x}\"", address.get()))
        .unwrap_or_else(|| "null".to_string());
    let needed_libraries = json_string_array(table.needed_libraries());
    let soname = table
        .soname()
        .map(|name| format!("\"{}\"", json_escape(name)))
        .unwrap_or_else(|| "null".to_string());
    let rpath = json_string_array(table.rpath());
    let runpath = json_string_array(table.runpath());
    let auxiliary = json_string_array(table.auxiliary_libraries());
    let filter = json_string_array(table.filter_libraries());
    let audit = json_string_array(table.audit_libraries());
    let dependency_audit = json_string_array(table.dependency_audit_libraries());
    let tables = elf_dynamic_tables_json(table);
    let lifecycle = elf_dynamic_lifecycle_json(table);
    let flags = elf_dynamic_flags_json(table);
    let linker = elf_dynamic_linker_json(table);
    let hash = elf_dynamic_hash_json(table);
    let versioning = elf_dynamic_versioning_json(table);
    let relocations = elf_dynamic_relocations_json(table);
    format!(
        "{{\"segments\":{},\"file_offset\":{},\"virtual_address\":{},\"entry_size\":{},\"entry_count\":{},\"needed\":{},\"needed_libraries\":{},\"soname\":{},\"rpath\":{},\"runpath\":{},\"auxiliary\":{},\"filter\":{},\"audit\":{},\"dependency_audit\":{},\"tables\":{},\"lifecycle\":{},\"flags\":{},\"linker\":{},\"hash\":{},\"versioning\":{},\"relocations\":{}}}",
        table.segment_count(),
        file_offset,
        virtual_address,
        table.entry_size(),
        table.entry_count(),
        table.needed_count(),
        needed_libraries,
        soname,
        rpath,
        runpath,
        auxiliary,
        filter,
        audit,
        dependency_audit,
        tables,
        lifecycle,
        flags,
        linker,
        hash,
        versioning,
        relocations
    )
}

fn elf_dynamic_tables_json(table: &BootElfDynamicTable) -> String {
    format!(
        "{{\"string\":{{\"virtual_address\":{},\"bytes\":{}}},\"symbol\":{{\"virtual_address\":{},\"entry_size\":{}}}}}",
        address_json(table.string_table_virtual_address()),
        optional_value_json(table.string_table_size()),
        address_json(table.symbol_table_virtual_address()),
        optional_value_json(table.symbol_table_entry_size())
    )
}

fn elf_dynamic_lifecycle_json(table: &BootElfDynamicTable) -> String {
    format!(
        "{{\"init\":{},\"fini\":{},\"init_array\":{},\"fini_array\":{},\"preinit_array\":{}}}",
        address_json(table.init_virtual_address()),
        address_json(table.fini_virtual_address()),
        elf_dynamic_array_json(table.init_array_virtual_address(), table.init_array_size()),
        elf_dynamic_array_json(table.fini_array_virtual_address(), table.fini_array_size()),
        elf_dynamic_array_json(
            table.preinit_array_virtual_address(),
            table.preinit_array_size()
        )
    )
}

fn elf_dynamic_array_json(virtual_address: Option<Address>, size: Option<u64>) -> String {
    format!(
        "{{\"virtual_address\":{},\"bytes\":{}}}",
        address_json(virtual_address),
        optional_value_json(size)
    )
}

fn elf_dynamic_flags_json(table: &BootElfDynamicTable) -> String {
    format!(
        "{{\"dt_flags\":{},\"dt_flags_1\":{}}}",
        optional_value_json(table.flags()),
        optional_value_json(table.flags_1())
    )
}

fn elf_dynamic_linker_json(table: &BootElfDynamicTable) -> String {
    format!(
        "{{\"plt_got\":{},\"debug\":{},\"symbolic\":{},\"textrel\":{},\"bind_now\":{},\"relative_relocations\":{{\"rela\":{},\"rel\":{}}}}}",
        address_json(table.plt_got_virtual_address()),
        address_json(table.debug_virtual_address()),
        table.has_symbolic_binding(),
        table.has_text_relocations(),
        table.bind_now(),
        optional_value_json(table.rela_relative_count()),
        optional_value_json(table.rel_relative_count())
    )
}

fn elf_dynamic_hash_json(table: &BootElfDynamicTable) -> String {
    format!(
        "{{\"sysv\":{},\"gnu\":{}}}",
        address_json(table.sysv_hash_virtual_address()),
        address_json(table.gnu_hash_virtual_address())
    )
}

fn elf_dynamic_versioning_json(table: &BootElfDynamicTable) -> String {
    format!(
        "{{\"symbols\":{},\"definitions\":{},\"needed\":{}}}",
        address_json(table.version_symbol_table_virtual_address()),
        elf_dynamic_version_table_json(
            table.version_definition_table_virtual_address(),
            table.version_definition_count()
        ),
        elf_dynamic_version_table_json(
            table.version_needed_table_virtual_address(),
            table.version_needed_count()
        )
    )
}

fn elf_dynamic_version_table_json(virtual_address: Option<Address>, count: Option<u64>) -> String {
    format!(
        "{{\"virtual_address\":{},\"entries\":{}}}",
        address_json(virtual_address),
        optional_value_json(count)
    )
}

fn elf_dynamic_relocations_json(table: &BootElfDynamicTable) -> String {
    let plt = table.plt_relocations();
    let kind = table
        .plt_relocation_kind()
        .map(|kind| match kind {
            BootElfDynamicPltRelocationKind::Rel => "\"rel\"",
            BootElfDynamicPltRelocationKind::Rela => "\"rela\"",
        })
        .unwrap_or("null");
    format!(
        "{{\"rela\":{},\"rel\":{},\"relr\":{},\"plt\":{{\"kind\":{},\"virtual_address\":{},\"bytes\":{},\"entry_size\":{},\"entries\":{}}}}}",
        elf_dynamic_relocation_table_json(table.rela_relocations()),
        elf_dynamic_relocation_table_json(table.rel_relocations()),
        elf_dynamic_relocation_table_json(table.relr_relocations()),
        kind,
        address_json(plt.virtual_address()),
        plt.byte_size(),
        plt.entry_size(),
        plt.entry_count()
    )
}

fn elf_dynamic_relocation_table_json(table: BootElfDynamicRelocationTable) -> String {
    format!(
        "{{\"virtual_address\":{},\"bytes\":{},\"entry_size\":{},\"entries\":{}}}",
        address_json(table.virtual_address()),
        table.byte_size(),
        table.entry_size(),
        table.entry_count()
    )
}

fn address_json(address: Option<Address>) -> String {
    address
        .map(|address| format!("\"0x{:x}\"", address.get()))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_value_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn elf_program_header_table_json(table: BootElfProgramHeaderTable) -> String {
    let memory_address = table
        .memory_address()
        .map(|address| format!("\"0x{:x}\"", address.get()))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"file_offset\":{},\"entry_size\":{},\"entry_count\":{},\"memory_address\":{}}}",
        table.file_offset(),
        table.entry_size(),
        table.entry_count(),
        memory_address
    )
}

fn elf_section_header_table_json(table: BootElfSectionHeaderTable) -> String {
    format!(
        "{{\"file_offset\":{},\"entry_size\":{},\"entry_count\":{},\"string_table_index\":{}}}",
        table.file_offset(),
        table.entry_size(),
        table.entry_count(),
        table.string_table_index(),
    )
}

fn elf_section_name_table_json(table: BootElfSectionNameTable) -> String {
    format!(
        "{{\"file_offset\":{},\"bytes\":{}}}",
        table.file_offset(),
        table.byte_size(),
    )
}

fn elf_section_flags_json(flags: BootElfSectionFlags) -> String {
    format!(
        "{{\"allocated\":{},\"writable\":{},\"executable\":{},\"nobits\":{}}}",
        flags.allocated_count(),
        flags.writable_count(),
        flags.executable_count(),
        flags.nobits_count(),
    )
}

fn elf_section_storage_json(storage: BootElfSectionStorage) -> String {
    format!(
        "{{\"file_bytes\":{},\"allocated_bytes\":{},\"writable_bytes\":{},\"executable_bytes\":{},\"nobits_bytes\":{},\"string_tables\":{},\"string_table_bytes\":{}}}",
        storage.file_backed_bytes(),
        storage.allocated_bytes(),
        storage.writable_bytes(),
        storage.executable_bytes(),
        storage.nobits_bytes(),
        storage.string_table_count(),
        storage.string_table_bytes(),
    )
}

fn elf_section_relocations_json(relocations: BootElfSectionRelocations) -> String {
    format!(
        "{{\"sections\":{},\"bytes\":{},\"rela_sections\":{},\"rela_entries\":{},\"rel_sections\":{},\"rel_entries\":{},\"relr_sections\":{},\"relr_entries\":{}}}",
        relocations.section_count(),
        relocations.byte_size(),
        relocations.rela_section_count(),
        relocations.rela_entry_count(),
        relocations.rel_section_count(),
        relocations.rel_entry_count(),
        relocations.relr_section_count(),
        relocations.relr_entry_count(),
    )
}

fn elf_section_arrays_json(arrays: BootElfSectionArrays) -> String {
    format!(
        "{{\"init\":{{\"sections\":{},\"bytes\":{},\"entries\":{}}},\"fini\":{{\"sections\":{},\"bytes\":{},\"entries\":{}}},\"preinit\":{{\"sections\":{},\"bytes\":{},\"entries\":{}}}}}",
        arrays.init_array_section_count(),
        arrays.init_array_bytes(),
        arrays.init_array_entry_count(),
        arrays.fini_array_section_count(),
        arrays.fini_array_bytes(),
        arrays.fini_array_entry_count(),
        arrays.preinit_array_section_count(),
        arrays.preinit_array_bytes(),
        arrays.preinit_array_entry_count(),
    )
}

fn elf_section_hashes_json(hashes: BootElfSectionHashes) -> String {
    format!(
        "{{\"sysv\":{{\"sections\":{},\"bytes\":{}}},\"gnu\":{{\"sections\":{},\"bytes\":{}}}}}",
        hashes.sysv_section_count(),
        hashes.sysv_bytes(),
        hashes.gnu_section_count(),
        hashes.gnu_bytes(),
    )
}

fn elf_section_groups_json(groups: BootElfSectionGroups) -> String {
    format!(
        "{{\"sections\":{},\"bytes\":{},\"entries\":{}}}",
        groups.section_count(),
        groups.byte_size(),
        groups.entry_count(),
    )
}

fn elf_section_address_range_json(range: BootElfSectionAddressRange) -> String {
    format!(
        "{{\"start\":{},\"end\":{}}}",
        address_json(range.start_address()),
        address_json(range.end_address()),
    )
}

fn elf_section_alignment_json(alignment: BootElfSectionAlignment) -> String {
    format!(
        "{{\"max\":{},\"allocated_max\":{},\"misaligned_allocated\":{}}}",
        alignment.max_alignment(),
        alignment.allocated_max_alignment(),
        alignment.misaligned_allocated_count(),
    )
}

impl Rem6ExecutionSummary {
    fn to_fabric_json(&self, config: Option<&RunFabricConfig>) -> String {
        run_fabric_json(config, &self.fabric)
    }
}

fn run_fabric_json(config: Option<&RunFabricConfig>, summary: &Rem6RunFabricSummary) -> String {
    let Some(config) = config else {
        return "null".to_string();
    };
    let credit_depth = config
        .credit_depth()
        .map(|depth| depth.to_string())
        .unwrap_or_else(|| "null".to_string());
    let router_stage = config
        .router_stage()
        .map(|stage| {
            format!(
                "{{\"router\":\"{}\",\"input_port\":{},\"output_port\":{},\"virtual_channel\":{},\"latency_ticks\":{}}}",
                json_escape(stage.router()),
                stage.input_port(),
                stage.output_port(),
                stage.virtual_channel(),
                stage.latency(),
            )
        })
        .unwrap_or_else(|| "null".to_string());
    let qos_queue_policy = config
        .qos_queue_policy()
        .map(|policy| format!("\"{}\"", policy.as_str()))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"link\":\"{}\",\"bandwidth_bytes_per_tick\":{},\"request_virtual_network\":{},\"response_virtual_network\":{},\"credit_depth\":{},\"router_stage\":{},\"qos_queue_policy\":{},\"active_lanes\":{},\"active_virtual_networks\":{},\"transfers\":{},\"bytes\":{},\"flits\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_lanes\":{},\"link_activities\":[{}],\"lane_activities\":[{}],\"hop_activities\":[{}]}}",
        json_escape(config.link()),
        config.bandwidth_bytes_per_tick(),
        config.request_virtual_network(),
        config.response_virtual_network(),
        credit_depth,
        router_stage,
        qos_queue_policy,
        summary.active_lanes(),
        summary.active_virtual_networks(),
        summary.transfers(),
        summary.bytes(),
        summary.flits(),
        summary.occupied_ticks(),
        summary.queue_delay_ticks(),
        summary.max_queue_delay_ticks(),
        summary.credit_delay_ticks(),
        summary.max_credit_delay_ticks(),
        summary.contended_lanes(),
        run_fabric_link_activities_json(summary),
        run_fabric_lane_activities_json(summary),
        run_fabric_hop_activities_json(summary),
    )
}

fn run_fabric_link_activities_json(summary: &Rem6RunFabricSummary) -> String {
    summary
        .link_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"active_virtual_networks\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_virtual_networks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.active_virtual_network_count(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.contended_virtual_network_count(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn run_fabric_lane_activities_json(summary: &Rem6RunFabricSummary) -> String {
    summary
        .lane_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn run_fabric_hop_activities_json(summary: &Rem6RunFabricSummary) -> String {
    summary
        .hop_activities()
        .iter()
        .map(|activity| {
            let router = run_fabric_hop_router_json(activity);
            format!(
                "{{\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"router\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                activity.packet().get(),
                activity.hop_index(),
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                router,
                activity.bytes(),
                activity.flits(),
                activity.ready_tick(),
                activity.start_tick(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.depart_tick(),
                activity.arrival_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn run_fabric_hop_router_json(activity: &FabricHopActivity) -> String {
    match activity.router() {
        Some(router) => format!(
            "{{\"router\":\"{}\",\"input_port\":{},\"output_port\":{},\"virtual_channel\":{},\"ready_tick\":{},\"start_tick\":{},\"latency_ticks\":{},\"depart_tick\":{},\"queue_delay_ticks\":{}}}",
            json_escape(router.router().as_str()),
            router.input_port(),
            router.output_port(),
            router.virtual_channel(),
            router.ready_tick(),
            router.start_tick(),
            router.latency_ticks(),
            router.depart_tick(),
            router.queue_delay_ticks(),
        ),
        None => "null".to_string(),
    }
}

impl Rem6LoadBlobSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"address\":\"0x{:x}\",\"bytes\":{},\"path\":\"{}\"}}",
            self.address(),
            self.bytes(),
            json_escape(self.source())
        )
    }
}

impl Rem6ReadfileSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"base\":\"0x{:x}\",\"size\":{},\"bytes\":{},\"path\":\"{}\"}}",
            self.base(),
            self.size(),
            self.bytes(),
            json_escape(self.path())
        )
    }
}

fn riscv_se_inputs_json(config: &crate::Rem6RunConfig) -> String {
    let stdin = config
        .riscv_se_stdin()
        .map(riscv_input_source_json)
        .unwrap_or_else(|| "null".to_string());
    let files = config
        .riscv_se_files()
        .iter()
        .map(|file| {
            format!(
                "{{\"guest_path\":\"{}\",\"source\":\"{}\"}}",
                json_escape(file.guest_path()),
                json_escape(&file.source().source_name())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"stdin\":{},\"files\":[{}]}}", stdin, files)
}

fn riscv_sbi_inputs_json(config: &crate::Rem6RunConfig) -> String {
    let console_input = config
        .riscv_sbi_console_input()
        .map(riscv_input_source_json)
        .unwrap_or_else(|| "null".to_string());
    format!("{{\"console_input\":{console_input}}}")
}

fn riscv_input_source_json(source: &crate::config::RiscvSeInputSource) -> String {
    format!("{{\"source\":\"{}\"}}", json_escape(&source.source_name()))
}

fn optional_riscv_cache_protocol_json(value: Option<RiscvDataCacheProtocol>) -> String {
    value
        .map(|protocol| format!("\"{}\"", riscv_cache_protocol_name(protocol)))
        .unwrap_or_else(|| "null".to_string())
}

const fn riscv_cache_protocol_name(protocol: RiscvDataCacheProtocol) -> &'static str {
    match protocol {
        RiscvDataCacheProtocol::Msi => "msi",
        RiscvDataCacheProtocol::Mesi => "mesi",
        RiscvDataCacheProtocol::Moesi => "moesi",
        RiscvDataCacheProtocol::Chi => "chi",
    }
}

fn optional_cache_prefetcher_json(value: Option<CliCachePrefetcher>) -> String {
    value
        .map(|prefetcher| format!("\"{}\"", prefetcher.as_str()))
        .unwrap_or_else(|| "null".to_string())
}
