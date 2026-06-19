use rem6_boot::BootImage;
use rem6_memory::{Address, PartitionedMemoryStore};

use super::RiscvWorkloadReplayError;

pub(super) fn load_payload_at(
    store: &mut PartitionedMemoryStore,
    address: Address,
    payload: &[u8],
) -> Result<(), RiscvWorkloadReplayError> {
    BootImage::new(address)
        .add_segment(address, payload.to_vec())
        .map_err(RiscvWorkloadReplayError::Boot)?
        .load_into_partitioned_store_by_address(store)
        .map(|_| ())
        .map_err(RiscvWorkloadReplayError::Boot)
}
