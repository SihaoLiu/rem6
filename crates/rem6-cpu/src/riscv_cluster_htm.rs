use rem6_transport::MemoryRouteId;

use crate::{CpuId, HtmAbortRecord, HtmBeginRecord, HtmTransactionError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvClusterHtmAbortOutcome {
    NoMatchingDataRoute {
        route: MemoryRouteId,
    },
    NoActiveTransaction {
        cpu: CpuId,
        route: MemoryRouteId,
    },
    Aborted {
        cpu: CpuId,
        route: MemoryRouteId,
        abort: HtmAbortRecord,
    },
    Failed {
        cpu: CpuId,
        route: MemoryRouteId,
        error: HtmTransactionError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvClusterHtmBeginOutcome {
    NoMatchingDataRoute {
        route: MemoryRouteId,
    },
    Begun {
        cpu: CpuId,
        route: MemoryRouteId,
        begin: HtmBeginRecord,
    },
    Failed {
        cpu: CpuId,
        route: MemoryRouteId,
        error: HtmTransactionError,
    },
}
