use std::error::Error;

use crate::error::WorkloadError;

pub(crate) fn format_partition_indexes(partitions: &[u32]) -> String {
    let values = partitions
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

impl Error for WorkloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Boot(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}
