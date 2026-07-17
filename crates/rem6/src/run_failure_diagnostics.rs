use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterError, RiscvCpuError};
use rem6_memory::CacheLineLayout;
use rem6_system::SystemError;
use rem6_transport::{MemoryTrace, MemoryTraceKind};

use crate::formatting::json_escape;
use crate::runtime_memory::{read_memory_dumps, CliMemoryRuntime};
use crate::{MemoryDumpRequest, Rem6MemoryDump};

const RISCV_DATA_PMP_FAILURE_SCHEMA: &str = "rem6.cli.riscv_data_pmp_failure.v1";

struct RiscvDataPmpFailureDiagnostic {
    completed_cpu_data_events: u64,
    data_channel_request_sent_events: u64,
    cores: Vec<RiscvDataPmpFailureCoreDiagnostic>,
    memory_dumps: Vec<Rem6MemoryDump>,
    capture_errors: Vec<String>,
}

struct RiscvDataPmpFailureCoreDiagnostic {
    cpu: u64,
    rob_entries: u64,
    lsq_entries: u64,
    writeback_reservations: u64,
}

pub(crate) fn capture_riscv_data_pmp_failure(
    error: &SystemError,
    cluster: &RiscvCluster,
    data_trace: &MemoryTrace,
    memory: &CliMemoryRuntime,
    line_layout: CacheLineLayout,
    memory_dump_requests: &[MemoryDumpRequest],
) -> Option<String> {
    if !is_riscv_data_pmp_failure(error) {
        return None;
    }
    let diagnostic = build_riscv_data_pmp_failure_diagnostic(
        cluster,
        data_trace,
        memory,
        line_layout,
        memory_dump_requests,
    );
    Some(diagnostic.to_json())
}

fn is_riscv_data_pmp_failure(error: &SystemError) -> bool {
    matches!(
        error,
        SystemError::RiscvCluster(RiscvClusterError::Core {
            error: RiscvCpuError::DataPmpAccess { .. },
            ..
        })
    )
}

fn build_riscv_data_pmp_failure_diagnostic(
    cluster: &RiscvCluster,
    data_trace: &MemoryTrace,
    memory: &CliMemoryRuntime,
    line_layout: CacheLineLayout,
    memory_dump_requests: &[MemoryDumpRequest],
) -> RiscvDataPmpFailureDiagnostic {
    let mut capture_errors = Vec::new();
    let data_channel_request_sent_events = capture_component(
        "data trace",
        &mut capture_errors,
        || -> Result<u64, String> {
            Ok(count_to_u64(
                data_trace
                    .try_snapshot()
                    .map_err(|error| error.to_string())?
                    .iter()
                    .filter(|event| event.kind() == MemoryTraceKind::RequestSent)
                    .count(),
            ))
        },
    )
    .unwrap_or(0);

    let mut completed_cpu_data_events = 0_u64;
    let mut cores = Vec::new();
    for cpu in cluster.core_ids() {
        if let Some((completed, core)) = capture_core(cluster, cpu, &mut capture_errors) {
            completed_cpu_data_events = completed_cpu_data_events.saturating_add(completed);
            cores.push(core);
        }
    }
    let memory_dumps = capture_component("memory dumps", &mut capture_errors, || {
        read_memory_dumps(memory, line_layout, memory_dump_requests)
            .map_err(|error| error.to_string())
    })
    .unwrap_or_default();

    RiscvDataPmpFailureDiagnostic {
        completed_cpu_data_events,
        data_channel_request_sent_events,
        cores,
        memory_dumps,
        capture_errors,
    }
}

fn capture_core(
    cluster: &RiscvCluster,
    cpu: CpuId,
    capture_errors: &mut Vec<String>,
) -> Option<(u64, RiscvDataPmpFailureCoreDiagnostic)> {
    capture_component(&format!("cpu{}", cpu.get()), capture_errors, || {
        let core = cluster.core(cpu).map_err(|error| error.to_string())?;
        let snapshot = core
            .try_failure_diagnostic_snapshot()
            .map_err(|error| error.to_string())?;
        Ok((
            count_to_u64(snapshot.completed_data_access_events()),
            RiscvDataPmpFailureCoreDiagnostic {
                cpu: u64::from(cpu.get()),
                rob_entries: count_to_u64(snapshot.rob_entries()),
                lsq_entries: count_to_u64(snapshot.lsq_entries()),
                writeback_reservations: count_to_u64(snapshot.writeback_reservations()),
            },
        ))
    })
}

fn capture_component<T>(
    label: &str,
    capture_errors: &mut Vec<String>,
    capture: impl FnOnce() -> Result<T, String>,
) -> Option<T> {
    match capture() {
        Ok(value) => Some(value),
        Err(error) => {
            capture_errors.push(format!("{label}: {error}"));
            None
        }
    }
}

const fn count_to_u64(count: usize) -> u64 {
    if count > u64::MAX as usize {
        u64::MAX
    } else {
        count as u64
    }
}

impl RiscvDataPmpFailureDiagnostic {
    fn to_json(&self) -> String {
        let cores = self
            .cores
            .iter()
            .map(RiscvDataPmpFailureCoreDiagnostic::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let memory_dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let capture_errors = self
            .capture_errors
            .iter()
            .map(|error| format!("\"{}\"", json_escape(error)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"{RISCV_DATA_PMP_FAILURE_SCHEMA}\",\"completed_cpu_data_events\":{},\"data_channel_request_sent_events\":{},\"cores\":[{cores}],\"memory_dumps\":[{memory_dumps}],\"capture_errors\":[{capture_errors}]}}",
            self.completed_cpu_data_events, self.data_channel_request_sent_events,
        )
    }
}

impl RiscvDataPmpFailureCoreDiagnostic {
    fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"rob_entries\":{},\"lsq_entries\":{},\"writeback_reservations\":{}}}",
            self.cpu, self.rob_entries, self.lsq_entries, self.writeback_reservations,
        )
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::RiscvPmpError;
    use rem6_memory::{AgentId, MemoryRequestId};

    use super::*;

    #[test]
    fn typed_match_accepts_only_nested_data_pmp_errors() {
        let pmp = SystemError::RiscvCluster(RiscvClusterError::Core {
            cpu: CpuId::new(0),
            error: RiscvCpuError::DataPmpAccess {
                fetch: MemoryRequestId::new(AgentId::new(0), 4),
                error: RiscvPmpError::ZeroAccessSize { address: 0x8000 },
            },
        });
        assert!(is_riscv_data_pmp_failure(&pmp));
        assert!(!is_riscv_data_pmp_failure(&SystemError::ZeroHostLatency));
    }

    #[test]
    fn capture_errors_are_serialized_without_masking_counts() {
        let mut capture_errors = Vec::new();
        let captured: Option<u64> = capture_component("probe", &mut capture_errors, || {
            Err("unavailable".to_string())
        });
        assert_eq!(captured, None);
        let diagnostic = RiscvDataPmpFailureDiagnostic {
            completed_cpu_data_events: 3,
            data_channel_request_sent_events: 2,
            cores: Vec::new(),
            memory_dumps: Vec::new(),
            capture_errors,
        };
        let json: serde_json::Value = serde_json::from_str(&diagnostic.to_json()).unwrap();
        assert_eq!(json["completed_cpu_data_events"], 3);
        assert_eq!(json["capture_errors"][0], "probe: unavailable");
    }

    #[test]
    fn provider_failures_are_aggregated_in_capture_errors() {
        let mut capture_errors = Vec::new();
        let cpu: Option<u64> = capture_component("cpu0", &mut capture_errors, || {
            Err("riscv core lock poisoned".to_string())
        });
        let memory: Option<u64> = capture_component("memory dumps", &mut capture_errors, || {
            Err("CLI memory store lock poisoned".to_string())
        });
        assert_eq!((cpu, memory), (None, None));

        let diagnostic = RiscvDataPmpFailureDiagnostic {
            completed_cpu_data_events: 0,
            data_channel_request_sent_events: 0,
            cores: Vec::new(),
            memory_dumps: Vec::new(),
            capture_errors,
        };
        let json: serde_json::Value = serde_json::from_str(&diagnostic.to_json()).unwrap();
        assert_eq!(
            json["capture_errors"],
            serde_json::json!([
                "cpu0: riscv core lock poisoned",
                "memory dumps: CLI memory store lock poisoned"
            ])
        );
    }
}
