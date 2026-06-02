use std::fmt;

use rem6_memory::Address;

use crate::WorkloadRouteId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadSinicPciTopologyError {
    DuplicateDevice {
        nic: u32,
    },
    DuplicateFunction {
        nic: u32,
        existing_nic: u32,
        bus: u8,
        device: u8,
        function: u8,
    },
    DuplicateBarBase {
        nic: u32,
        existing_nic: u32,
        bar_base: Address,
    },
    MissingMmioRoute {
        nic: u32,
        route: WorkloadRouteId,
    },
    MmioRouteTargetMismatch {
        nic: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    MmioRouteEndpointMismatch {
        nic: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    BarBaseMisaligned {
        nic: u32,
        bar_base: Address,
        alignment_bytes: u64,
    },
}

impl fmt::Display for WorkloadSinicPciTopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDevice { nic } => {
                write!(formatter, "SINIC PCI device {nic} is already defined")
            }
            Self::DuplicateFunction {
                nic,
                existing_nic,
                bus,
                device,
                function,
            } => write!(
                formatter,
                "SINIC PCI device {nic} uses PCI function {bus}:{device}.{function}, already used by SINIC PCI device {existing_nic}"
            ),
            Self::DuplicateBarBase {
                nic,
                existing_nic,
                bar_base,
            } => write!(
                formatter,
                "SINIC PCI device {nic} uses BAR base {:#x}, already used by SINIC PCI device {existing_nic}",
                bar_base.get()
            ),
            Self::MissingMmioRoute { nic, route } => write!(
                formatter,
                "SINIC PCI device {nic} MMIO route {} is not defined",
                route.as_str()
            ),
            Self::MmioRouteTargetMismatch {
                nic,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "SINIC PCI device {nic} MMIO route {} targets partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MmioRouteEndpointMismatch {
                nic,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "SINIC PCI device {nic} MMIO route {} targets endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::BarBaseMisaligned {
                nic,
                bar_base,
                alignment_bytes,
            } => write!(
                formatter,
                "SINIC PCI device {nic} BAR base {:#x} is not aligned to {alignment_bytes} bytes",
                bar_base.get()
            ),
        }
    }
}
