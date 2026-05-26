use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_virtio::{
    VirtioBlockConfigSpec, VirtioBlockDevice, VirtioBlockMemoryBackend, VirtioBlockRequest,
    VirtioBlockRequestId, VirtioBlockStatus, VirtioQueueIndex, VIRTIO_BLOCK_SECTOR_SIZE,
    VIRTIO_BLOCK_T_GET_ID,
};

fn sector(byte: u8) -> Vec<u8> {
    vec![byte; VIRTIO_BLOCK_SECTOR_SIZE as usize]
}

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

#[test]
fn virtio_block_device_executes_serial_and_parallel_sector_requests() {
    let mut image = Vec::new();
    image.extend(sector(0x11));
    image.extend(sector(0x22));
    image.extend(sector(0x33));
    image.extend(sector(0x44));
    let backend = VirtioBlockMemoryBackend::from_bytes(image).unwrap();
    let device = VirtioBlockDevice::new(
        VirtioBlockConfigSpec::new(4)
            .with_flush(true)
            .with_queues(2),
        backend.clone(),
    )
    .unwrap();

    let completions = Arc::new(Mutex::new(Vec::new()));
    let cpu = PartitionId::new(0);
    let block = PartitionId::new(1);

    let mut serial_scheduler = PartitionedScheduler::new(2).unwrap();
    let serial_device = device.clone();
    let serial_completions = Arc::clone(&completions);
    serial_scheduler
        .schedule_at(cpu, 4, move |context| {
            let completion = serial_device
                .execute(
                    context,
                    VirtioBlockRequest::write(
                        VirtioBlockRequestId::new(10),
                        queue(0),
                        1,
                        sector(0xaa),
                    )
                    .unwrap(),
                )
                .unwrap();
            serial_completions.lock().unwrap().push(completion);
        })
        .unwrap();
    serial_scheduler.run_until_idle();

    let mut parallel_scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let read_device = device.clone();
    let read_completions = Arc::clone(&completions);
    parallel_scheduler
        .schedule_parallel_at(block, 20, move |context| {
            let completion = read_device
                .execute_parallel(
                    context,
                    VirtioBlockRequest::read(
                        VirtioBlockRequestId::new(11),
                        queue(1),
                        1,
                        VIRTIO_BLOCK_SECTOR_SIZE,
                    )
                    .unwrap(),
                )
                .unwrap();
            read_completions.lock().unwrap().push(completion);
        })
        .unwrap();
    let flush_device = device.clone();
    let flush_completions = Arc::clone(&completions);
    parallel_scheduler
        .schedule_parallel_at(block, 20, move |context| {
            let completion = flush_device
                .execute_parallel(
                    context,
                    VirtioBlockRequest::flush(VirtioBlockRequestId::new(12), queue(1)).unwrap(),
                )
                .unwrap();
            flush_completions.lock().unwrap().push(completion);
        })
        .unwrap();
    parallel_scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap().clone();
    assert_eq!(completions.len(), 3);
    assert_eq!(completions[0].request(), VirtioBlockRequestId::new(10));
    assert_eq!(completions[0].queue(), queue(0));
    assert_eq!(completions[0].tick(), 4);
    assert_eq!(completions[0].status(), VirtioBlockStatus::Ok);
    assert_eq!(completions[0].data(), None);

    let read = completions
        .iter()
        .find(|completion| completion.request() == VirtioBlockRequestId::new(11))
        .unwrap();
    assert_eq!(read.queue(), queue(1));
    assert_eq!(read.tick(), 20);
    assert_eq!(read.status(), VirtioBlockStatus::Ok);
    assert_eq!(read.data(), Some(sector(0xaa).as_slice()));

    let flush = completions
        .iter()
        .find(|completion| completion.request() == VirtioBlockRequestId::new(12))
        .unwrap();
    assert_eq!(flush.status(), VirtioBlockStatus::Ok);
    assert_eq!(flush.data(), None);
    assert_eq!(backend.flush_count(), 1);
    assert_eq!(backend.read_sector(1).unwrap(), sector(0xaa));

    assert_eq!(device.completions(), completions);
}

#[test]
fn virtio_block_device_reports_guest_visible_error_statuses() {
    let backend =
        VirtioBlockMemoryBackend::from_bytes([sector(0x55), sector(0x66)].concat()).unwrap();
    let device = VirtioBlockDevice::new(
        VirtioBlockConfigSpec::new(2)
            .with_read_only(true)
            .with_flush(true),
        backend.clone(),
    )
    .unwrap()
    .with_device_id("rem6-block0")
    .unwrap();

    let write = device
        .execute_at(
            7,
            VirtioBlockRequest::write(VirtioBlockRequestId::new(20), queue(0), 0, sector(0xaa))
                .unwrap(),
        )
        .unwrap();
    assert_eq!(write.status(), VirtioBlockStatus::IoErr);
    assert_eq!(backend.read_sector(0).unwrap(), sector(0x55));

    let read = device
        .execute_at(
            8,
            VirtioBlockRequest::read(
                VirtioBlockRequestId::new(21),
                queue(0),
                2,
                VIRTIO_BLOCK_SECTOR_SIZE,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(read.status(), VirtioBlockStatus::IoErr);
    assert_eq!(read.data(), None);

    let id = device
        .execute_at(
            9,
            VirtioBlockRequest::get_id(VirtioBlockRequestId::new(22), queue(0)),
        )
        .unwrap();
    assert_eq!(id.status(), VirtioBlockStatus::Ok);
    assert_eq!(id.data().unwrap().len(), 20);
    assert_eq!(&id.data().unwrap()[..10], b"rem6-block");
    assert_eq!(id.data().unwrap()[19], 0);

    let unsupported = device
        .execute_at(
            10,
            VirtioBlockRequest::unsupported(
                VirtioBlockRequestId::new(23),
                queue(0),
                VIRTIO_BLOCK_T_GET_ID + 99,
                Vec::new(),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(unsupported.status(), VirtioBlockStatus::Unsupported);

    assert!(matches!(
        VirtioBlockMemoryBackend::from_bytes(vec![0; 7]),
        Err(error) if error.to_string().contains("sector")
    ));
    assert!(matches!(
        VirtioBlockDevice::new(VirtioBlockConfigSpec::new(3), backend),
        Err(error) if error.to_string().contains("capacity")
    ));
    assert!(matches!(
        VirtioBlockRequest::read(VirtioBlockRequestId::new(24), queue(0), 0, 7),
        Err(error) if error.to_string().contains("512")
    ));
    assert!(matches!(
        VirtioBlockRequest::write(VirtioBlockRequestId::new(25), queue(0), 0, vec![0; 7]),
        Err(error) if error.to_string().contains("512")
    ));
    assert!(matches!(
        device.clone().with_device_id("this-id-is-longer-than-twenty-bytes"),
        Err(error) if error.to_string().contains("device id")
    ));
}
