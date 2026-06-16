use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse,
    MmioRoute,
};
use rem6_platform::{
    Platform, PlatformBuilder, PlatformError, PlatformReadfileConfig, PlatformReadfileError,
};

const READFILE_MAX_TRANSFER_BYTES: u64 = 4096;

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn build_error(result: Result<Platform, PlatformError>) -> PlatformError {
    match result {
        Ok(_) => panic!("platform build unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn platform_readfile_payload_is_visible_through_mmio_bus() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let route = MmioRoute::new(cpu, device, 2, 3).unwrap();
    let base = Address::new(0x1200_0000);
    let payload = b"bootargs\n".to_vec();

    let platform = PlatformBuilder::new(2)
        .add_readfile(PlatformReadfileConfig {
            base,
            size: AccessSize::new(0x20).unwrap(),
            route,
            payload: payload.clone(),
        })
        .build()
        .unwrap();

    assert_eq!(platform.readfile(base).unwrap().payload(), payload);

    let bus = platform.mmio_bus().clone();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            let first_read = Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::read(MmioRequestId::new(1), base, AccessSize::new(8).unwrap())
                    .unwrap(),
                move |completion| first_read.lock().unwrap().push(completion),
            )
            .unwrap();

            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(2),
                    Address::new(base.get() + 7),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 6);
    let completions = completions.lock().unwrap();
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].route(), route);
    assert_eq!(
        completions[0].response(),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(1),
            Some(b"bootargs".to_vec())
        ))
    );
    assert_eq!(completions[1].route(), route);
    assert_eq!(
        completions[1].response(),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(2),
            Some(vec![b's', b'\n', 0, 0])
        ))
    );
}

#[test]
fn platform_readfile_rejects_guest_writes() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let route = MmioRoute::new(cpu, device, 2, 3).unwrap();
    let base = Address::new(0x1200_0000);

    let platform = PlatformBuilder::new(2)
        .add_readfile(PlatformReadfileConfig {
            base,
            size: AccessSize::new(0x20).unwrap(),
            route,
            payload: b"readonly".to_vec(),
        })
        .build()
        .unwrap();

    let bus = platform.mmio_bus().clone();
    let completions = Arc::new(Mutex::new(Vec::<MmioCompletion>::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            bus.submit(
                context,
                MmioRequest::write(MmioRequestId::new(3), base, vec![0xff], full_mask(1)).unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 6);
    let completions = completions.lock().unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].route(), route);
    assert_eq!(
        completions[0].response(),
        &Err(MmioError::AccessDenied {
            request: MmioRequestId::new(3),
            operation: MmioOperation::Write,
            access: MmioAccess::ReadOnly,
        })
    );
}

#[test]
fn platform_readfile_rejects_oversized_mmio_reads_before_allocating_response() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let route = MmioRoute::new(cpu, device, 2, 3).unwrap();
    let base = Address::new(0x1200_0000);

    let platform = PlatformBuilder::new(2)
        .add_readfile(PlatformReadfileConfig {
            base,
            size: AccessSize::new(READFILE_MAX_TRANSFER_BYTES + 1).unwrap(),
            route,
            payload: Vec::new(),
        })
        .build()
        .unwrap();

    let bus = platform.mmio_bus().clone();
    let completions = Arc::new(Mutex::new(Vec::<MmioCompletion>::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(4),
                    base,
                    AccessSize::new(READFILE_MAX_TRANSFER_BYTES + 1).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 6);
    let completions = completions.lock().unwrap();
    assert_eq!(completions.len(), 1);
    assert_eq!(
        completions[0].response(),
        &Err(MmioError::TransferTooLarge {
            request: MmioRequestId::new(4),
            bytes: READFILE_MAX_TRANSFER_BYTES + 1,
            maximum: READFILE_MAX_TRANSFER_BYTES,
        })
    );
}

#[test]
fn platform_readfile_rejects_payload_larger_than_window() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let base = Address::new(0x1200_0000);

    let error = build_error(
        PlatformBuilder::new(2)
            .add_readfile(PlatformReadfileConfig {
                base,
                size: AccessSize::new(4).unwrap(),
                route: MmioRoute::new(cpu, device, 2, 3).unwrap(),
                payload: b"too large".to_vec(),
            })
            .build(),
    );

    assert_eq!(
        error,
        PlatformError::Readfile(PlatformReadfileError::PayloadExceedsWindow {
            payload_bytes: 9,
            window_bytes: 4,
        })
    );
}

#[test]
fn platform_readfile_rejects_unknown_route_partition() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(2);
    let base = Address::new(0x1200_0000);

    let error = build_error(
        PlatformBuilder::new(2)
            .add_readfile(PlatformReadfileConfig {
                base,
                size: AccessSize::new(0x20).unwrap(),
                route: MmioRoute::new(cpu, device, 2, 3).unwrap(),
                payload: b"payload".to_vec(),
            })
            .build(),
    );

    assert_eq!(
        error,
        PlatformError::UnknownPartition {
            partition: device,
            partitions: 2,
        }
    );
}

#[test]
fn platform_readfile_rejects_overlapping_mmio_window_for_same_source() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let route = MmioRoute::new(cpu, device, 2, 3).unwrap();

    let error = build_error(
        PlatformBuilder::new(2)
            .add_readfile(PlatformReadfileConfig {
                base: Address::new(0x1200_0000),
                size: AccessSize::new(0x20).unwrap(),
                route,
                payload: b"first".to_vec(),
            })
            .add_readfile(PlatformReadfileConfig {
                base: Address::new(0x1200_0010),
                size: AccessSize::new(0x20).unwrap(),
                route,
                payload: b"second".to_vec(),
            })
            .build(),
    );

    assert_eq!(
        error,
        PlatformError::Mmio(MmioError::OverlappingDeviceRegion {
            existing_start: Address::new(0x1200_0000),
            existing_end: Address::new(0x1200_0020),
            requested_start: Address::new(0x1200_0010),
            requested_end: Address::new(0x1200_0030),
        })
    );
}
