use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    InOrderPipelineConfig, RiscvCore,
};
use rem6_isa_riscv::{Register, RiscvGdbXlen, RiscvPrivilegeMode};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_system::RiscvSeStartupImage;
use rem6_transport::MemoryTransport;

use crate::config::Rem6RunConfig;
use crate::riscv_sbi_runtime::configure_cli_riscv_sbi_core;
use crate::{
    add_memory_route, execute_error, transport_endpoint, Rem6CliError, RISCV_BOOT_A0_REGISTER,
    RISCV_BOOT_A1_REGISTER, RISCV_STACK_POINTER_REGISTER,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn build_cli_riscv_cores(
    config: &Rem6RunConfig,
    transport: &mut MemoryTransport,
    core_count: u32,
    memory_partition: PartitionId,
    start_address: Address,
    line_layout: CacheLineLayout,
    gdb_xlen: RiscvGdbXlen,
    riscv_se_startup: Option<&RiscvSeStartupImage>,
    in_order_pipeline_config: &InOrderPipelineConfig,
) -> Result<Vec<RiscvCore>, Rem6CliError> {
    let mut cores = Vec::new();
    for cpu_index in 0..core_count {
        let cpu_partition = PartitionId::new(cpu_index);
        let fetch_route = add_memory_route(
            transport,
            format!("cpu{cpu_index}.ifetch"),
            cpu_partition,
            memory_partition,
            config.memory_route_delay(),
            config.fabric(),
        )?;
        let data_route = add_memory_route(
            transport,
            format!("cpu{cpu_index}.dmem"),
            cpu_partition,
            memory_partition,
            config.memory_route_delay(),
            config.fabric(),
        )?;
        let cpu_core = CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu_index),
                cpu_partition,
                AgentId::new(cpu_index),
                start_address,
            ),
            CpuFetchConfig::new(
                transport_endpoint(format!("cpu{cpu_index}.ifetch"))?,
                fetch_route,
                line_layout,
                AccessSize::new(4).map_err(execute_error)?,
            ),
        )
        .map_err(execute_error)?;
        let data_config = CpuDataConfig::new(
            transport_endpoint(format!("cpu{cpu_index}.dmem"))?,
            data_route,
            line_layout,
        );
        let core = match config.riscv_data_translation() {
            Some(translation) => {
                let frontend = match translation.tlb() {
                    Some(tlb) => CpuTranslationFrontend::with_tlb(translation.queue(), tlb),
                    None => CpuTranslationFrontend::new(translation.queue()),
                };
                RiscvCore::with_data_translation(cpu_core, data_config, frontend)
            }
            None => RiscvCore::with_data(cpu_core, data_config),
        };
        core.set_xlen(gdb_xlen);
        core.write_register(
            Register::new(RISCV_BOOT_A0_REGISTER).map_err(execute_error)?,
            config.riscv_boot_a0(),
        );
        core.write_register(
            Register::new(RISCV_BOOT_A1_REGISTER).map_err(execute_error)?,
            config.riscv_boot_a1(),
        );
        if let Some(startup) = riscv_se_startup {
            core.set_privilege_mode(RiscvPrivilegeMode::User);
            core.write_register(
                Register::new(RISCV_STACK_POINTER_REGISTER).map_err(execute_error)?,
                startup.initial_stack_pointer().get(),
            );
        }
        configure_cli_riscv_sbi_core(config, cpu_index, &core, start_address);
        if config.checker_cpu() {
            core.enable_checker_cpu();
        }
        core.reset_in_order_pipeline_config(in_order_pipeline_config.clone());
        core.set_branch_lookahead(config.riscv_branch_lookahead());
        core.set_o3_scalar_memory_depth(config.riscv_o3_scalar_memory_depth());
        core.set_o3_issue_width(config.riscv_o3_issue_width());
        core.set_o3_writeback_width(config.riscv_o3_writeback_width());
        core.set_branch_predictor_kind(config.riscv_branch_predictor());
        cores.push(core);
    }
    Ok(cores)
}
