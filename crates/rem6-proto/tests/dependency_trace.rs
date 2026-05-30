use rem6_memory::Address;
use rem6_proto::{
    DependencyMemoryCompletionPolicy, DependencyRecord, DependencyRecordKind, DependencyTrace,
    DependencyTraceHeader, ProtoError, TraceSourceId,
};

fn header(window_size: u32) -> DependencyTraceHeader {
    DependencyTraceHeader::new(
        TraceSourceId::new("cpu0.o3.dep").unwrap(),
        1_000_000_000,
        window_size,
    )
    .unwrap()
}

fn load_record(seq: u64) -> DependencyRecord {
    DependencyRecord::new(seq, DependencyRecordKind::Load)
        .unwrap()
        .with_physical_address(Address::new(0x8000))
        .with_virtual_address(Address::new(0xffff_8000))
        .with_asid(1)
        .with_size(8)
        .unwrap()
        .with_flags(0x4)
        .with_order_dependency(seq - 1)
        .unwrap()
        .with_register_dependency(seq - 2)
        .unwrap()
        .with_compute_delay(3)
        .with_weight(2)
        .unwrap()
        .with_pc(0x1000)
}

#[test]
fn dependency_trace_accepts_typed_o3_dependency_records() {
    let trace = DependencyTrace::builder(header(64))
        .add_record(load_record(10))
        .add_record(
            DependencyRecord::new(11, DependencyRecordKind::Compute)
                .unwrap()
                .with_compute_delay(5)
                .with_order_dependency(10)
                .unwrap()
                .with_register_dependency(1)
                .unwrap()
                .with_weight(4)
                .unwrap()
                .with_pc(0x1004),
        )
        .build()
        .unwrap();

    assert_eq!(trace.header().window_size(), 64);
    assert_eq!(trace.records()[0].kind(), DependencyRecordKind::Load);
    assert_eq!(
        trace.records()[0].physical_address(),
        Some(Address::new(0x8000))
    );
    assert_eq!(trace.records()[0].order_dependencies(), &[9]);
    assert_eq!(trace.records()[1].compute_delay(), 5);
    assert!(!trace.identity().as_str().is_empty());

    let same_trace = DependencyTrace::builder(header(64))
        .add_record(load_record(10))
        .add_record(
            DependencyRecord::new(11, DependencyRecordKind::Compute)
                .unwrap()
                .with_compute_delay(5)
                .with_order_dependency(10)
                .unwrap()
                .with_register_dependency(1)
                .unwrap()
                .with_weight(4)
                .unwrap()
                .with_pc(0x1004),
        )
        .build()
        .unwrap();
    assert_eq!(trace.identity(), same_trace.identity());
}

#[test]
fn dependency_trace_models_prefetch_as_nonblocking_memory_record() {
    let trace = DependencyTrace::builder(header(16))
        .add_record(
            DependencyRecord::new(20, DependencyRecordKind::Compute)
                .unwrap()
                .with_compute_delay(2)
                .with_pc(0x2000),
        )
        .add_record(
            DependencyRecord::new(21, DependencyRecordKind::Prefetch)
                .unwrap()
                .with_physical_address(Address::new(0x9000))
                .with_virtual_address(Address::new(0xffff_9000))
                .with_asid(4)
                .with_size(64)
                .unwrap()
                .with_flags(0x20)
                .with_order_dependency(20)
                .unwrap()
                .with_pc(0x2004),
        )
        .add_record(
            DependencyRecord::new(22, DependencyRecordKind::Compute)
                .unwrap()
                .with_order_dependency(21)
                .unwrap()
                .with_compute_delay(1)
                .with_pc(0x2008),
        )
        .build()
        .unwrap();

    let prefetch = &trace.records()[1];
    assert_eq!(prefetch.kind(), DependencyRecordKind::Prefetch);
    assert_eq!(
        prefetch.memory_completion_policy(),
        DependencyMemoryCompletionPolicy::RetireAfterIssue
    );
    assert!(!prefetch.requires_memory_response_for_retirement());
    assert_eq!(prefetch.size(), Some(64));
    assert_eq!(prefetch.order_dependencies(), &[20]);

    assert_eq!(
        trace.records()[0].memory_completion_policy(),
        DependencyMemoryCompletionPolicy::NotMemory
    );
    assert!(load_record(30).requires_memory_response_for_retirement());
}

#[test]
fn dependency_trace_rejects_invalid_or_ambiguous_dependency_records() {
    assert_eq!(
        DependencyTraceHeader::new(TraceSourceId::new("cpu0").unwrap(), 1_000_000_000, 0)
            .unwrap_err(),
        ProtoError::ZeroDependencyWindowSize,
    );
    assert_eq!(
        DependencyRecord::new(0, DependencyRecordKind::Load).unwrap_err(),
        ProtoError::ZeroDependencySequence,
    );
    assert_eq!(
        DependencyRecord::new(1, DependencyRecordKind::Invalid).unwrap_err(),
        ProtoError::InvalidDependencyRecordKind,
    );
    assert_eq!(
        DependencyRecord::new(1, DependencyRecordKind::Load)
            .unwrap()
            .with_size(0)
            .unwrap_err(),
        ProtoError::ZeroDependencyAccessSize,
    );
    assert_eq!(
        DependencyRecord::new(1, DependencyRecordKind::Compute)
            .unwrap()
            .with_size(4)
            .unwrap_err(),
        ProtoError::UnexpectedDependencyMemoryAccess {
            kind: DependencyRecordKind::Compute,
        },
    );
    assert_eq!(
        DependencyRecord::new(1, DependencyRecordKind::Store)
            .unwrap()
            .with_order_dependency(1)
            .unwrap_err(),
        ProtoError::SelfDependency { sequence: 1 },
    );
    assert_eq!(
        DependencyTrace::builder(header(64))
            .add_record(DependencyRecord::new(1, DependencyRecordKind::Load).unwrap())
            .build()
            .unwrap_err(),
        ProtoError::MissingDependencyMemoryAccess {
            kind: DependencyRecordKind::Load,
        },
    );
    assert_eq!(
        DependencyTrace::builder(header(64))
            .add_record(
                DependencyRecord::new(2, DependencyRecordKind::Compute)
                    .unwrap()
                    .with_order_dependency(99)
                    .unwrap(),
            )
            .build()
            .unwrap_err(),
        ProtoError::UnknownDependency {
            sequence: 2,
            dependency: 99,
        },
    );
    assert_eq!(
        DependencyTrace::builder(header(64))
            .add_record(
                DependencyRecord::new(100, DependencyRecordKind::Compute)
                    .unwrap()
                    .with_order_dependency(1)
                    .unwrap(),
            )
            .build()
            .unwrap_err(),
        ProtoError::DependencyOutsideWindow {
            sequence: 100,
            dependency: 1,
            window_size: 64,
        },
    );
    assert_eq!(
        DependencyTrace::builder(header(64))
            .add_record(DependencyRecord::new(1, DependencyRecordKind::Compute).unwrap())
            .add_record(DependencyRecord::new(1, DependencyRecordKind::Compute).unwrap())
            .build()
            .unwrap_err(),
        ProtoError::DuplicateDependencyRecord { sequence: 1 },
    );
}
