use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptEvent, InterruptEventKind, InterruptLineId, InterruptSourceId, InterruptTargetId,
    PLIC_MMIO_CLAIM_COMPLETE_OFFSET, PLIC_MMIO_CONTEXT_BASE_OFFSET, PLIC_MMIO_CONTEXT_STRIDE,
    PLIC_MMIO_ENABLE_BASE_OFFSET, PLIC_MMIO_ENABLE_CONTEXT_STRIDE, PLIC_MMIO_PRIORITY_STRIDE,
    PLIC_MMIO_REGISTER_BYTES,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_platform::{
    PlatformBuilder, PlatformClintConfig, PlatformClintHartConfig,
    PlatformInterruptControllerConfig, PlatformInterruptControllerContextConfig,
    PlatformRiscvDeviceTreeConfig, PlatformTimerConfig,
};
use rem6_timer::{ClintId, ClintResetPolicy, TimerExpiry, TimerId, TIMER_MMIO_DEADLINE_OFFSET};

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

#[test]
fn platform_builder_exposes_plic_context_routes_to_platform() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let plic_partition = PartitionId::new(2);
    let timer_partition = PartitionId::new(3);
    let target0 = InterruptTargetId::new(0);
    let target1 = InterruptTargetId::new(1);
    let plic_base = Address::new(0x0c00_0000);
    let timer_id = TimerId::new(11);
    let line = InterruptLineId::new(65);
    let source = InterruptSourceId::new(75);
    let plic_route = MmioRoute::new(cpu0, plic_partition, 2, 1).unwrap();
    let timer_route = MmioRoute::new(cpu1, timer_partition, 2, 1).unwrap();

    let platform = PlatformBuilder::new(4)
        .add_interrupt_controller(PlatformInterruptControllerConfig {
            base: plic_base,
            size: AccessSize::new(0x400_0000).unwrap(),
            route: plic_route,
            target: target0,
            contexts: vec![
                PlatformInterruptControllerContextConfig {
                    context: 0,
                    hart: 0,
                    interrupt: 0xB,
                    target: target0,
                    target_partition: cpu0,
                },
                PlatformInterruptControllerContextConfig {
                    context: 1,
                    hart: 1,
                    interrupt: 0x9,
                    target: target1,
                    target_partition: cpu1,
                },
            ],
        })
        .add_clint(PlatformClintConfig {
            id: ClintId::new(0),
            base: Address::new(0x0200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: MmioRoute::new(cpu0, plic_partition, 2, 1).unwrap(),
            reset_policy: ClintResetPolicy::preserve_mtimecmp(),
            harts: vec![
                PlatformClintHartConfig {
                    hart: 0,
                    target_partition: cpu0,
                    interrupt_target: target0,
                    software_interrupt_line: InterruptLineId::new(60),
                    software_interrupt_source: InterruptSourceId::new(70),
                    timer_interrupt_line: InterruptLineId::new(61),
                    timer_interrupt_source: InterruptSourceId::new(71),
                    interrupt_latency: 2,
                },
                PlatformClintHartConfig {
                    hart: 1,
                    target_partition: cpu1,
                    interrupt_target: target1,
                    software_interrupt_line: InterruptLineId::new(62),
                    software_interrupt_source: InterruptSourceId::new(72),
                    timer_interrupt_line: InterruptLineId::new(63),
                    timer_interrupt_source: InterruptSourceId::new(73),
                    interrupt_latency: 2,
                },
            ],
        })
        .add_timer(PlatformTimerConfig {
            id: timer_id,
            base: Address::new(0x5000),
            size: AccessSize::new(0x100).unwrap(),
            route: timer_route,
            interrupt_line: line,
            interrupt_target: target1,
            interrupt_source: source,
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    let config =
        PlatformRiscvDeviceTreeConfig::new(10_000_000, "rv64imafdc", "riscv,sv48", 0x384000)
            .unwrap();
    let tree = platform.riscv_device_tree(&config).unwrap();
    let cpus = tree.root().child("cpus").unwrap();
    let cpu0_phandle = cpu_interrupt_phandle(cpus, "cpu@0");
    let cpu1_phandle = cpu_interrupt_phandle(cpus, "cpu@1");
    let plic = tree
        .root()
        .child("soc")
        .unwrap()
        .child("interrupt-controller@c000000")
        .unwrap();
    assert_eq!(
        plic.property("interrupts-extended").unwrap().words(),
        Some(&[cpu0_phandle, 0xB, cpu1_phandle, 0x9][..])
    );

    let timer = platform.timer(timer_id).unwrap().clone();
    let controller = platform.interrupt_controller();
    let bus = platform.mmio_bus().clone();
    let timer_bus = bus.clone();
    let plic_bus = bus;
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu1, 1, move |context| {
            timer_bus
                .submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(30),
                        Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                        le64(7),
                        full_mask(8),
                    )
                    .unwrap(),
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu0, 10, move |context| {
            let pending_word = line.get() / 32;
            let pending_bit = 1u32 << (line.get() % 32);
            let context1_enable = PLIC_MMIO_ENABLE_BASE_OFFSET + PLIC_MMIO_ENABLE_CONTEXT_STRIDE;
            let context1_base = PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CONTEXT_STRIDE;
            for request in [
                MmioRequest::write(
                    MmioRequestId::new(31),
                    Address::new(plic_base.get() + line.get() * PLIC_MMIO_PRIORITY_STRIDE),
                    le32(5),
                    full_mask(PLIC_MMIO_REGISTER_BYTES),
                )
                .unwrap(),
                MmioRequest::write(
                    MmioRequestId::new(32),
                    Address::new(
                        plic_base.get() + context1_enable + pending_word * PLIC_MMIO_REGISTER_BYTES,
                    ),
                    le32(pending_bit),
                    full_mask(PLIC_MMIO_REGISTER_BYTES),
                )
                .unwrap(),
                MmioRequest::read(
                    MmioRequestId::new(33),
                    Address::new(plic_base.get() + PLIC_MMIO_CONTEXT_BASE_OFFSET),
                    AccessSize::new(PLIC_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                MmioRequest::read(
                    MmioRequestId::new(34),
                    Address::new(plic_base.get() + context1_base + PLIC_MMIO_CLAIM_COMPLETE_OFFSET),
                    AccessSize::new(PLIC_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                MmioRequest::write(
                    MmioRequestId::new(35),
                    Address::new(plic_base.get() + context1_base + PLIC_MMIO_CLAIM_COMPLETE_OFFSET),
                    le32(line.get() as u32),
                    full_mask(PLIC_MMIO_REGISTER_BYTES),
                )
                .unwrap(),
            ] {
                let sink = Arc::clone(&completed);
                plic_bus
                    .submit(context, request, move |completion| {
                        sink.lock().unwrap().push(completion);
                    })
                    .unwrap();
            }
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 13);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 7)]);
    let completions = completions.lock().unwrap();
    assert_eq!(completions.len(), 6);
    assert_eq!(
        completions[0],
        MmioCompletion::new(
            4,
            timer_route,
            Ok(MmioResponse::completed(MmioRequestId::new(30), None)),
        )
    );
    assert_eq!(completions[3].route(), plic_route);
    assert_eq!(
        completions[3].response(),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(33),
            Some(le32(0)),
        ))
    );
    assert_eq!(completions[4].route(), plic_route);
    assert_eq!(
        completions[4].response(),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(34),
            Some(le32(line.get() as u32)),
        ))
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(9, line, target1, cpu1, source, InterruptEventKind::Assert),
            InterruptEvent::routed(12, line, target1, cpu1, source, InterruptEventKind::Claim),
            InterruptEvent::routed(
                12,
                line,
                target1,
                cpu1,
                source,
                InterruptEventKind::Complete,
            ),
        ]
    );
}

fn cpu_interrupt_phandle(cpus: &rem6_platform::PlatformDeviceTreeNode, cpu: &str) -> u32 {
    cpus.child(cpu)
        .unwrap()
        .child("interrupt-controller")
        .unwrap()
        .property("phandle")
        .unwrap()
        .words()
        .unwrap()[0]
}
