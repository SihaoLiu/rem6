use rem6_cpu::CpuId;
use rem6_memory::CacheLineLayout;
use rem6_stats::{StackDistProbeConfig, StatsRegistry};
use rem6_workload::WorkloadTopology;

use crate::riscv_data_access_stats::RiscvDataAccessProbeLineLayout;
use crate::{RiscvDataAccessStats, RiscvInstructionStats, RiscvWorkloadReplayError, SystemError};

pub(super) fn workload_instruction_stats(
    topology: &WorkloadTopology,
    stats: &mut StatsRegistry,
) -> Result<RiscvInstructionStats, RiscvWorkloadReplayError> {
    topology
        .riscv_cores()
        .iter()
        .map(|core| {
            let stat = stats
                .register_counter(format!("cpu{}.committed_insts", core.cpu()), "count")
                .map_err(|error| RiscvWorkloadReplayError::System(SystemError::Stats(error)))?;
            Ok((CpuId::new(core.cpu()), stat))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(RiscvInstructionStats::new)
}

pub(super) fn workload_data_access_stats(
    topology: &WorkloadTopology,
) -> Result<Option<RiscvDataAccessStats>, RiscvWorkloadReplayError> {
    if !topology
        .riscv_cores()
        .iter()
        .any(|core| core.data_route().is_some())
    {
        return Ok(None);
    }

    let mut line_size = None;
    let mut line_layouts = Vec::new();
    for target in topology.memory_targets() {
        let layout =
            CacheLineLayout::new(target.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)?;
        line_size = Some(
            line_size
                .unwrap_or(target.line_bytes())
                .min(target.line_bytes()),
        );
        line_layouts.push(RiscvDataAccessProbeLineLayout::new(target.range(), layout));
    }
    let Some(line_size) = line_size else {
        return Ok(None);
    };
    let config = StackDistProbeConfig::builder(line_size, line_size)
        .build()
        .map_err(|error| RiscvWorkloadReplayError::System(SystemError::Stats(error)))?;
    Ok(Some(
        RiscvDataAccessStats::with_stack_distance_line_layouts(config, line_layouts),
    ))
}
