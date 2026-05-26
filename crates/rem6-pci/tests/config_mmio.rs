use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioBus, MmioCompletion, MmioError, MmioRequest, MmioRequestId, MmioRoute};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarRange, PciBarSpec, PciClassCode, PciConfigAperture,
    PciConfigMmioDevice, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciFunctionAddress,
    PciInterruptPin,
};

fn storage_endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = PciEndpointConfig::new(
        function,
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
    )
    .with_interrupt(5, PciInterruptPin::IntA);
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory32 {
                    prefetchable: false,
                },
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
}

fn response_for(
    completions: &[MmioCompletion],
    request: MmioRequestId,
) -> &Result<rem6_mmio::MmioResponse, MmioError> {
    completions
        .iter()
        .find_map(|completion| match completion.response() {
            Ok(response) if response.request() == request => Some(completion.response()),
            Err(MmioError::DeviceError {
                request: failed, ..
            }) if *failed == request => Some(completion.response()),
            _ => None,
        })
        .expect("completion for request")
}

fn completed_data(completions: &[MmioCompletion], request: MmioRequestId) -> Vec<u8> {
    response_for(completions, request)
        .as_ref()
        .expect("successful MMIO response")
        .data()
        .expect("read response data")
        .to_vec()
}

fn expect_completed_write(completions: &[MmioCompletion], request: MmioRequestId) {
    assert_eq!(
        response_for(completions, request)
            .as_ref()
            .expect("successful MMIO response")
            .data(),
        None
    );
}

#[test]
fn pci_config_mmio_device_routes_ecam_accesses_on_serial_bus() {
    let aperture = PciConfigAperture::ecam(Address::new(0x2800_0000), 1).unwrap();
    let mut host = rem6_pci::PciHostBridge::new(aperture);
    let function = PciFunctionAddress::new(0, 1, 0).unwrap();
    host.register_endpoint(storage_endpoint(function)).unwrap();
    let device = PciConfigMmioDevice::new(host);

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(device.config_range(), route, device)
        .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 5, move |context| {
            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(21),
                    aperture
                        .config_address(function, PciConfigOffset::new(0x00).unwrap())
                        .unwrap(),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    let completions = completions.lock().unwrap();
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(21)),
        vec![0xf4, 0x1a, 0x01, 0x10]
    );
}

#[test]
fn pci_config_mmio_device_routes_ecam_accesses_on_parallel_bus() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let mut host = rem6_pci::PciHostBridge::new(aperture);
    let function = PciFunctionAddress::new(0, 3, 0).unwrap();
    let absent = PciFunctionAddress::new(0, 4, 0).unwrap();
    host.register_endpoint(storage_endpoint(function)).unwrap();
    let device = PciConfigMmioDevice::new(host);
    let shared_host = device.host();

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(device.config_range(), route, device)
        .unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let first_bus = Arc::clone(&bus);
    let first_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 10, move |context| {
            let read_completed = Arc::clone(&first_completed);
            first_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(1),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x00).unwrap())
                            .unwrap(),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| read_completed.lock().unwrap().push(completion),
                )
                .unwrap();
            let bar_completed = Arc::clone(&first_completed);
            first_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(2),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x10).unwrap())
                            .unwrap(),
                        0x8000_2000_u32.to_le_bytes().to_vec(),
                        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
                    )
                    .unwrap(),
                    move |completion| bar_completed.lock().unwrap().push(completion),
                )
                .unwrap();
            first_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(3),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x04).unwrap())
                            .unwrap(),
                        0x0002_u16.to_le_bytes().to_vec(),
                        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
                    )
                    .unwrap(),
                    move |completion| first_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let second_bus = Arc::clone(&bus);
    let second_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 20, move |context| {
            let absent_completed = Arc::clone(&second_completed);
            second_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(4),
                        aperture
                            .config_address(absent, PciConfigOffset::new(0x00).unwrap())
                            .unwrap(),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| absent_completed.lock().unwrap().push(completion),
                )
                .unwrap();
            second_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(5),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x10).unwrap())
                            .unwrap(),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| second_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap();
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(1)),
        vec![0xf4, 0x1a, 0x01, 0x10]
    );
    expect_completed_write(&completions, MmioRequestId::new(2));
    expect_completed_write(&completions, MmioRequestId::new(3));
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(4)),
        vec![0xff, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(5)),
        0x8000_2000_u32.to_le_bytes()
    );
    assert_eq!(
        shared_host
            .lock()
            .unwrap()
            .endpoint(function)
            .unwrap()
            .active_bar_ranges(),
        vec![PciBarRange::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::Memory32 {
                prefetchable: false,
            },
            Address::new(0x8000_2000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap()]
    );
}

#[test]
fn pci_config_mmio_device_applies_masks_and_reports_in_aperture_errors() {
    let aperture = PciConfigAperture::ecam(Address::new(0x4000_0000), 1).unwrap();
    let mut host = rem6_pci::PciHostBridge::new(aperture);
    let function = PciFunctionAddress::new(0, 2, 0).unwrap();
    host.register_endpoint(storage_endpoint(function)).unwrap();
    let device = PciConfigMmioDevice::new(host);

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 1, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(device.config_range(), route, device)
        .unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let first_bus = Arc::clone(&bus);
    let first_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            let command_completed = Arc::clone(&first_completed);
            first_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(11),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x04).unwrap())
                            .unwrap(),
                        vec![0x03, 0x00, 0xaa, 0xbb],
                        ByteMask::from_bits(vec![true, true, false, false]).unwrap(),
                    )
                    .unwrap(),
                    move |completion| command_completed.lock().unwrap().push(completion),
                )
                .unwrap();
            first_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(12),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x3c).unwrap())
                            .unwrap(),
                        vec![0x22],
                        ByteMask::from_bits(vec![false]).unwrap(),
                    )
                    .unwrap(),
                    move |completion| first_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let second_bus = Arc::clone(&bus);
    let second_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 10, move |context| {
            let command_read_completed = Arc::clone(&second_completed);
            second_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(13),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x04).unwrap())
                            .unwrap(),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| command_read_completed.lock().unwrap().push(completion),
                )
                .unwrap();
            let line_read_completed = Arc::clone(&second_completed);
            second_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(14),
                        aperture
                            .config_address(function, PciConfigOffset::new(0x3c).unwrap())
                            .unwrap(),
                        AccessSize::new(1).unwrap(),
                    )
                    .unwrap(),
                    move |completion| line_read_completed.lock().unwrap().push(completion),
                )
                .unwrap();
            second_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(15),
                        Address::new(
                            aperture
                                .endpoint_config_range(function)
                                .unwrap()
                                .start()
                                .get()
                                + 0x100,
                        ),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| second_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap();
    expect_completed_write(&completions, MmioRequestId::new(11));
    expect_completed_write(&completions, MmioRequestId::new(12));
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(13)),
        vec![0x03, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(14)),
        vec![5]
    );
    let invalid = response_for(&completions, MmioRequestId::new(15));
    assert!(matches!(
        invalid,
        Err(MmioError::DeviceError {
            request,
            message,
        }) if *request == MmioRequestId::new(15)
            && message.contains("unsupported offset")
    ));
}
