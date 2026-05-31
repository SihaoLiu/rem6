use rem6_boot::BootImage;
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryError, MemoryTargetId,
    PartitionedMemoryStore,
};

use crate::config::LoadBlobRequest;
use crate::{execute_error, Rem6CliError, Rem6LoadBlobSummary};

const CLI_MEMORY_TARGET: MemoryTargetId = MemoryTargetId::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LoadedBlob {
    pub(super) summary: Rem6LoadBlobSummary,
    data: Vec<u8>,
}

pub(super) fn read_load_blobs(
    requests: &[LoadBlobRequest],
) -> Result<Vec<LoadedBlob>, Rem6CliError> {
    requests
        .iter()
        .map(|request| {
            let data =
                std::fs::read(request.path()).map_err(|error| Rem6CliError::ReadLoadBlob {
                    path: request.path().to_path_buf(),
                    error: error.to_string(),
                })?;
            if data.is_empty() {
                return Err(Rem6CliError::EmptyLoadBlob {
                    path: request.path().to_path_buf(),
                });
            }
            let summary = Rem6LoadBlobSummary::new(
                request.address(),
                request.path().to_path_buf(),
                data.len() as u64,
            );
            Ok(LoadedBlob { summary, data })
        })
        .collect()
}

pub(super) fn build_cli_memory_store(
    image: &BootImage,
    load_blobs: &[LoadedBlob],
    line_layout: CacheLineLayout,
) -> Result<PartitionedMemoryStore, Rem6CliError> {
    let mut store = PartitionedMemoryStore::new();
    store
        .add_partition(CLI_MEMORY_TARGET, line_layout)
        .map_err(execute_error)?;
    for region in cli_memory_regions(image, load_blobs)? {
        store
            .map_region(CLI_MEMORY_TARGET, region.start(), region.size())
            .map_err(execute_error)?;
    }
    image
        .load_into_partitioned_store(&mut store, CLI_MEMORY_TARGET)
        .map_err(execute_error)?;
    for blob in load_blobs {
        load_blob_into_store(&mut store, line_layout, blob)?;
    }
    Ok(store)
}

fn cli_memory_regions(
    image: &BootImage,
    load_blobs: &[LoadedBlob],
) -> Result<Vec<AddressRange>, Rem6CliError> {
    let mut ranges = Vec::with_capacity(image.segments().len() + load_blobs.len());
    ranges.extend(image.segments().iter().map(|segment| segment.range()));
    for blob in load_blobs {
        ranges.push(
            AddressRange::new(
                Address::new(blob.summary.address()),
                AccessSize::new(blob.summary.bytes()).map_err(execute_error)?,
            )
            .map_err(execute_error)?,
        );
    }
    ranges.sort_by_key(|range| (range.start(), range.end()));

    let mut merged: Vec<AddressRange> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start().get() < last.end().get() {
                return Err(execute_error(MemoryError::OverlappingAddressRegion {
                    existing: *last,
                    requested: range,
                }));
            }
            if range.start().get() == last.end().get() {
                let bytes = range.end().get() - last.start().get();
                *last =
                    AddressRange::new(last.start(), AccessSize::new(bytes).map_err(execute_error)?)
                        .map_err(execute_error)?;
                continue;
            }
        }
        merged.push(range);
    }

    Ok(merged)
}

fn load_blob_into_store(
    store: &mut PartitionedMemoryStore,
    line_layout: CacheLineLayout,
    blob: &LoadedBlob,
) -> Result<(), Rem6CliError> {
    let mut cursor = blob.summary.address();
    let mut data_offset = 0usize;
    while data_offset < blob.data.len() {
        let address = Address::new(cursor);
        let line = line_layout.line_address(address);
        let line_offset = line_layout.line_offset(address);
        let available_in_line = line_layout.bytes() - line_offset;
        let remaining = (blob.data.len() - data_offset) as u64;
        let bytes = available_in_line.min(remaining);
        let next_data_offset = data_offset + bytes as usize;

        let mut line_data = store
            .line_data(CLI_MEMORY_TARGET, line)
            .unwrap_or_else(|_| vec![0; line_layout.bytes() as usize]);
        let start = line_offset as usize;
        line_data[start..start + bytes as usize]
            .copy_from_slice(&blob.data[data_offset..next_data_offset]);
        store
            .insert_line(CLI_MEMORY_TARGET, line, line_data)
            .map_err(execute_error)?;

        cursor += bytes;
        data_offset = next_data_offset;
    }
    Ok(())
}
