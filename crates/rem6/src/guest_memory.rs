use rem6_boot::BootImage;
use rem6_dram::{
    DramGeometry, DramLowPowerTiming, DramMemoryController, DramRefreshTiming, DramTiming,
    ExternalMemoryProfile, NvmMediaTiming,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryError, MemoryTargetId,
    PartitionedMemoryStore,
};

use crate::config::{CliDramMemoryProfile, LoadBlobRequest, LoadBlobSource};
use crate::run_resource_config::RunResourcePayloads;
use crate::{execute_error, Rem6CliError, Rem6LoadBlobSummary};

pub(super) const CLI_MEMORY_TARGET: MemoryTargetId = MemoryTargetId::new(0);
const CLI_ELF_LOAD_PAGE_BYTES: u64 = 4096;
const CLI_VOLATILE_REFRESH_INTERVAL: u64 = 32;
const CLI_VOLATILE_REFRESH_RECOVERY: u64 = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LoadedBlob {
    pub(super) summary: Rem6LoadBlobSummary,
    data: Vec<u8>,
}

pub(super) fn read_load_blobs(
    requests: &[LoadBlobRequest],
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<LoadedBlob>, Rem6CliError> {
    requests
        .iter()
        .map(|request| {
            let data = read_load_blob_data(request, resource_payloads)?;
            if data.is_empty() {
                return Err(Rem6CliError::EmptyLoadBlob {
                    source: request.source_name(),
                });
            }
            let summary = Rem6LoadBlobSummary::new(
                request.address(),
                request.source_name(),
                data.len() as u64,
            );
            Ok(LoadedBlob { summary, data })
        })
        .collect()
}

fn read_load_blob_data(
    request: &LoadBlobRequest,
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<u8>, Rem6CliError> {
    match request.source() {
        LoadBlobSource::Path(path) => {
            std::fs::read(path).map_err(|error| Rem6CliError::ReadLoadBlob {
                path: path.to_path_buf(),
                error: error.to_string(),
            })
        }
        LoadBlobSource::Resource(resource) => {
            let payloads = resource_payloads.ok_or_else(|| Rem6CliError::Execute {
                error: format!("load blob resource {resource} requires --resource-config"),
            })?;
            Ok(payloads.blob_payload(resource)?.to_vec())
        }
        LoadBlobSource::SuiteResource(selector) => {
            let payloads = resource_payloads.ok_or_else(|| Rem6CliError::Execute {
                error: format!(
                    "load blob suite resource {} requires --resource-config",
                    selector.qualified_id()
                ),
            })?;
            Ok(payloads
                .blob_suite_payload(selector.workload_id(), selector.resource_id())?
                .to_vec())
        }
    }
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
    for region in cli_memory_regions(image, load_blobs, line_layout)? {
        store
            .map_region(CLI_MEMORY_TARGET, region.start(), region.size())
            .map_err(execute_error)?;
        zero_cli_memory_region(&mut store, line_layout, region)?;
    }
    image
        .load_into_partitioned_store(&mut store, CLI_MEMORY_TARGET)
        .map_err(execute_error)?;
    for blob in load_blobs {
        load_blob_into_store(&mut store, line_layout, blob)?;
    }
    Ok(store)
}

pub(super) fn cli_source_backed_cache_line_ranges(
    image: &BootImage,
    load_blobs: &[LoadedBlob],
    line_layout: CacheLineLayout,
) -> Result<Vec<AddressRange>, Rem6CliError> {
    fully_covered_line_ranges(checked_cli_memory_ranges(image, load_blobs)?, line_layout)
}

pub(super) fn cli_fully_covered_cache_line_ranges(
    address: u64,
    bytes: u64,
    line_layout: CacheLineLayout,
) -> Result<Vec<AddressRange>, Rem6CliError> {
    if bytes == 0 {
        return Ok(Vec::new());
    }
    let range = AddressRange::new(
        Address::new(address),
        AccessSize::new(bytes).map_err(execute_error)?,
    )
    .map_err(execute_error)?;
    fully_covered_line_ranges(vec![range], line_layout)
}

pub(super) fn build_cli_dram_memory(
    image: &BootImage,
    load_blobs: &[LoadedBlob],
    line_layout: CacheLineLayout,
    profile: CliDramMemoryProfile,
) -> Result<DramMemoryController, Rem6CliError> {
    let store = build_cli_memory_store(image, load_blobs, line_layout)?;
    let snapshot = store.snapshot();
    let profile = build_cli_dram_profile(line_layout, profile)?;
    let mut memory = DramMemoryController::new();
    memory.add_profile(profile).map_err(execute_error)?;
    for (target, region) in snapshot.regions() {
        memory
            .map_region(*target, region.start(), region.size())
            .map_err(execute_error)?;
    }
    for partition in snapshot.partitions() {
        for line in partition.lines() {
            memory
                .insert_line(partition.target(), line.line(), line.data().to_vec())
                .map_err(execute_error)?;
        }
    }
    Ok(memory)
}

pub(super) fn build_cli_dram_profile(
    line_layout: CacheLineLayout,
    profile: CliDramMemoryProfile,
) -> Result<ExternalMemoryProfile, Rem6CliError> {
    let geometry = DramGeometry::new(4, 64, line_layout.bytes()).map_err(execute_error)?;
    let timing = DramTiming::new(3, 5, 7, 2, 4).map_err(execute_error)?;
    let volatile_timing = timing
        .with_refresh_timing(
            DramRefreshTiming::new(CLI_VOLATILE_REFRESH_INTERVAL, CLI_VOLATILE_REFRESH_RECOVERY)
                .map_err(execute_error)?,
        )
        .map_err(execute_error)?;
    match profile {
        CliDramMemoryProfile::Ddr => ExternalMemoryProfile::ddr(
            CLI_MEMORY_TARGET,
            line_layout,
            1,
            1,
            geometry,
            volatile_timing,
        ),
        CliDramMemoryProfile::Ddr4_2400_8Gb => ExternalMemoryProfile::ddr4_2400_8gb(
            CLI_MEMORY_TARGET,
            line_layout,
            1,
            1,
            geometry,
            timing,
        ),
        CliDramMemoryProfile::Ddr5_4800_16Gb => ExternalMemoryProfile::ddr5_4800_16gb(
            CLI_MEMORY_TARGET,
            line_layout,
            1,
            1,
            geometry,
            timing,
        ),
        CliDramMemoryProfile::Hbm => {
            let geometry = geometry.with_bank_groups(2).map_err(execute_error)?;
            let timing = volatile_timing
                .with_same_bank_group_burst_spacing(6)
                .map_err(execute_error)?;
            ExternalMemoryProfile::hbm(CLI_MEMORY_TARGET, line_layout, 2, 2, geometry, timing)
        }
        CliDramMemoryProfile::Hbm2_2000_2Gb => {
            let geometry = geometry.with_bank_groups(2).map_err(execute_error)?;
            let timing = timing
                .with_same_bank_group_burst_spacing(6)
                .map_err(execute_error)?;
            ExternalMemoryProfile::hbm2_2000_2gb(
                CLI_MEMORY_TARGET,
                line_layout,
                2,
                2,
                geometry,
                timing,
            )
        }
        CliDramMemoryProfile::Lpddr => ExternalMemoryProfile::lpddr(
            CLI_MEMORY_TARGET,
            line_layout,
            2,
            2,
            geometry,
            volatile_timing,
        ),
        CliDramMemoryProfile::Nvm => {
            let low_power_timing = DramLowPowerTiming::new(20, 80, 7)
                .and_then(|timing| timing.with_self_refresh_exit_latency(17))
                .map_err(execute_error)?;
            let timing = timing.with_command_window(16, 2).map_err(execute_error)?;
            let timing = timing.with_low_power_timing(low_power_timing);
            ExternalMemoryProfile::nvm(CLI_MEMORY_TARGET, line_layout, 2, 4, geometry, timing)
                .and_then(|profile| {
                    profile.with_nvm_media_timing(NvmMediaTiming::new(30, 50, 6, 4, 1)?)
                })
        }
    }
    .map_err(execute_error)
}

fn cli_memory_regions(
    image: &BootImage,
    load_blobs: &[LoadedBlob],
    line_layout: CacheLineLayout,
) -> Result<Vec<AddressRange>, Rem6CliError> {
    let ranges = checked_cli_memory_backing_ranges(image, load_blobs)?;
    let mut line_ranges = ranges
        .into_iter()
        .map(|range| line_covered_range(range, line_layout))
        .collect::<Result<Vec<_>, _>>()?;
    line_ranges.sort_by_key(|range| (range.start(), range.end()));

    merge_line_ranges(line_ranges)
}

fn checked_cli_memory_ranges(
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

fn zero_cli_memory_region(
    store: &mut PartitionedMemoryStore,
    line_layout: CacheLineLayout,
    region: AddressRange,
) -> Result<(), Rem6CliError> {
    let zero_line = vec![0; line_layout.bytes() as usize];
    let mut line = region.start();
    while line.get() < region.end().get() {
        store
            .insert_line(CLI_MEMORY_TARGET, line, zero_line.clone())
            .map_err(execute_error)?;
        let next = line.get().checked_add(line_layout.bytes()).ok_or_else(|| {
            execute_error(MemoryError::AddressOverflow {
                start: line,
                size: AccessSize::new(line_layout.bytes()).expect("line size is nonzero"),
            })
        })?;
        line = Address::new(next);
    }
    Ok(())
}

fn checked_cli_memory_backing_ranges(
    image: &BootImage,
    load_blobs: &[LoadedBlob],
) -> Result<Vec<AddressRange>, Rem6CliError> {
    checked_cli_memory_ranges(image, load_blobs)?;

    let mut ranges = Vec::with_capacity(image.segments().len() + load_blobs.len());
    for segment in image.segments() {
        ranges.push(cli_image_segment_backing_range(image, segment.range())?);
    }
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
    merge_line_ranges(ranges)
}

fn cli_image_segment_backing_range(
    image: &BootImage,
    range: AddressRange,
) -> Result<AddressRange, Rem6CliError> {
    if image.elf_metadata().is_none() {
        return Ok(range);
    }
    let end = align_up_to_elf_load_page(range.end().get())?;
    let bytes = end - range.start().get();
    AddressRange::new(
        range.start(),
        AccessSize::new(bytes).map_err(execute_error)?,
    )
    .map_err(execute_error)
}

fn align_up_to_elf_load_page(value: u64) -> Result<u64, Rem6CliError> {
    value
        .checked_add(CLI_ELF_LOAD_PAGE_BYTES - 1)
        .map(|value| value & !(CLI_ELF_LOAD_PAGE_BYTES - 1))
        .ok_or_else(|| {
            execute_error(MemoryError::AddressOverflow {
                start: Address::new(value),
                size: AccessSize::new(CLI_ELF_LOAD_PAGE_BYTES)
                    .expect("ELF load page size is nonzero"),
            })
        })
}

pub(super) fn merge_line_ranges(
    ranges: Vec<AddressRange>,
) -> Result<Vec<AddressRange>, Rem6CliError> {
    let mut merged: Vec<AddressRange> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start().get() <= last.end().get() {
                let bytes = range.end().get().max(last.end().get()) - last.start().get();
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

fn fully_covered_line_ranges(
    ranges: Vec<AddressRange>,
    line_layout: CacheLineLayout,
) -> Result<Vec<AddressRange>, Rem6CliError> {
    let mut covered = Vec::new();
    for range in ranges {
        let start = first_fully_covered_line(range, line_layout)?;
        let end = line_layout.line_address(range.end());
        if start.get() >= end.get() {
            continue;
        }
        covered.push(
            AddressRange::new(
                start,
                AccessSize::new(end.get() - start.get()).map_err(execute_error)?,
            )
            .map_err(execute_error)?,
        );
    }
    covered.sort_by_key(|range| (range.start(), range.end()));
    merge_line_ranges(covered)
}

fn first_fully_covered_line(
    range: AddressRange,
    line_layout: CacheLineLayout,
) -> Result<Address, Rem6CliError> {
    if line_layout.line_offset(range.start()) == 0 {
        return Ok(range.start());
    }
    let partial = line_layout.line_address(range.start());
    let line_size = AccessSize::new(line_layout.bytes()).map_err(execute_error)?;
    let next = partial
        .get()
        .checked_add(line_layout.bytes())
        .ok_or_else(|| {
            execute_error(MemoryError::AddressOverflow {
                start: partial,
                size: line_size,
            })
        })?;
    Ok(Address::new(next))
}

fn line_covered_range(
    range: AddressRange,
    line_layout: CacheLineLayout,
) -> Result<AddressRange, Rem6CliError> {
    let start = line_layout.line_address(range.start());
    let last_byte = range
        .end()
        .get()
        .checked_sub(1)
        .expect("address ranges are nonempty");
    let end_line = line_layout.line_address(Address::new(last_byte));
    let line_size = AccessSize::new(line_layout.bytes()).map_err(execute_error)?;
    let end = end_line
        .get()
        .checked_add(line_layout.bytes())
        .ok_or_else(|| {
            execute_error(MemoryError::AddressOverflow {
                start: end_line,
                size: line_size,
            })
        })?;
    let bytes = end - start.get();
    AddressRange::new(start, AccessSize::new(bytes).map_err(execute_error)?).map_err(execute_error)
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

#[cfg(test)]
mod tests {
    use rem6_boot::BootImage;
    use rem6_memory::{MemoryRequest, MemoryRequestId};

    use super::*;

    #[test]
    fn cli_memory_store_maps_entire_loaded_lines() {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let image = BootImage::new(Address::new(0x8000))
            .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x70, 0x00])
            .unwrap();
        let mut store = build_cli_memory_store(&image, &[], line_layout).unwrap();
        let request = MemoryRequest::instruction_fetch(
            MemoryRequestId::new(rem6_memory::AgentId::new(1), 0),
            Address::new(0x8004),
            AccessSize::new(4).unwrap(),
            line_layout,
        )
        .unwrap();

        let response = store
            .respond(&request)
            .unwrap()
            .response()
            .cloned()
            .unwrap();

        assert_eq!(response.data(), Some(&[0, 0, 0, 0][..]));
    }

    #[test]
    fn cli_memory_store_maps_elf_load_page_tail_as_zeroes() {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let image = BootImage::from_elf64_le(&test_riscv64_elf(
            0x10000,
            0x10000,
            &[0x13, 0x00, 0x00, 0x00],
        ))
        .unwrap();
        let mut store = build_cli_memory_store(&image, &[], line_layout).unwrap();
        let request = MemoryRequest::read_shared(
            MemoryRequestId::new(rem6_memory::AgentId::new(1), 0),
            Address::new(0x10040),
            AccessSize::new(4).unwrap(),
            line_layout,
        )
        .unwrap();

        let response = store
            .respond(&request)
            .unwrap()
            .response()
            .cloned()
            .unwrap();

        assert_eq!(response.data(), Some(&[0, 0, 0, 0][..]));
    }

    #[test]
    fn source_backed_cache_line_ranges_exclude_elf_load_page_padding() {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let image = BootImage::from_elf64_le(&test_riscv64_elf(
            0x10000,
            0x10000,
            &[0x13, 0x00, 0x00, 0x00],
        ))
        .unwrap();

        let ranges = cli_source_backed_cache_line_ranges(&image, &[], line_layout).unwrap();

        assert_eq!(ranges, Vec::<AddressRange>::new());
    }

    #[test]
    fn cli_memory_store_merges_disjoint_ranges_sharing_a_line() {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let image = BootImage::new(Address::new(0x8000))
            .add_segment(Address::new(0x8000), vec![1, 2, 3, 4])
            .unwrap();
        let blob = LoadedBlob {
            summary: crate::Rem6LoadBlobSummary::new(0x8008, "blob.bin", 4),
            data: vec![5, 6, 7, 8],
        };

        let mut store = build_cli_memory_store(&image, &[blob], line_layout).unwrap();
        let request = MemoryRequest::instruction_fetch(
            MemoryRequestId::new(rem6_memory::AgentId::new(1), 0),
            Address::new(0x8004),
            AccessSize::new(8).unwrap(),
            line_layout,
        )
        .unwrap();

        let response = store
            .respond(&request)
            .unwrap()
            .response()
            .cloned()
            .unwrap();

        assert_eq!(response.data(), Some(&[0, 0, 0, 0, 5, 6, 7, 8][..]));
    }

    #[test]
    fn cli_memory_store_rejects_overlapping_raw_ranges() {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let image = BootImage::new(Address::new(0x8000))
            .add_segment(Address::new(0x8000), vec![1, 2, 3, 4])
            .unwrap();
        let blob = LoadedBlob {
            summary: crate::Rem6LoadBlobSummary::new(0x8002, "blob.bin", 4),
            data: vec![5, 6, 7, 8],
        };

        let error = build_cli_memory_store(&image, &[blob], line_layout).unwrap_err();

        assert!(format!("{error}").contains("overlaps existing region"));
    }

    #[test]
    fn source_backed_cache_line_ranges_include_only_fully_covered_lines() {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let image = BootImage::new(Address::new(0x8000))
            .add_segment(Address::new(0x8000), vec![0xaa; 24])
            .unwrap();
        let blob = LoadedBlob {
            summary: crate::Rem6LoadBlobSummary::new(0x8024, "blob.bin", 44),
            data: vec![0xbb; 44],
        };

        let ranges = cli_source_backed_cache_line_ranges(&image, &[blob], line_layout).unwrap();

        assert_eq!(
            ranges,
            vec![
                AddressRange::new(Address::new(0x8000), AccessSize::new(16).unwrap()).unwrap(),
                AddressRange::new(Address::new(0x8030), AccessSize::new(32).unwrap()).unwrap()
            ]
        );
    }

    fn test_riscv64_elf(entry: u64, physical: u64, payload: &[u8]) -> Vec<u8> {
        let payload_offset = 128usize;
        let mut bytes = vec![0; payload_offset + payload.len()];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        write_u16(&mut bytes, 16, 2);
        write_u16(&mut bytes, 18, 243);
        write_u32(&mut bytes, 20, 1);
        write_u64(&mut bytes, 24, entry);
        write_u64(&mut bytes, 32, 64);
        write_u16(&mut bytes, 52, 64);
        write_u16(&mut bytes, 54, 56);
        write_u16(&mut bytes, 56, 1);

        write_u32(&mut bytes, 64, 1);
        write_u32(&mut bytes, 68, 5);
        write_u64(&mut bytes, 72, payload_offset as u64);
        write_u64(&mut bytes, 80, physical);
        write_u64(&mut bytes, 88, physical);
        write_u64(&mut bytes, 96, payload.len() as u64);
        write_u64(&mut bytes, 104, payload.len() as u64);
        write_u64(&mut bytes, 112, 0x1000);
        bytes[payload_offset..].copy_from_slice(payload);
        bytes
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}
