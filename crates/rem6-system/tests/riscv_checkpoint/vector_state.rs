use super::*;
use rem6_isa_riscv::{
    RiscvVectorArchitecturalState, RiscvVectorConfig, RiscvVectorFixedPointState,
    RiscvVectorFixedRoundingMode, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
    RISCV_VECTOR_REGISTER_COUNT,
};

const RISCV_STATE_VERSION_CHUNK: &str = "riscv-state-version";
const VECTOR_STATE_CHUNK: &str = "vector-state";
const VECTOR_STATE_BYTES: usize = 526;
const VECTOR_REGISTERS_OFFSET: usize = 14;

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn register_pattern(seed: u8) -> [u8; RISCV_VECTOR_REGISTER_BYTES] {
    std::array::from_fn(|index| seed.wrapping_add((index as u8).wrapping_mul(0x13)))
}

fn patterned_registers(
    seed: u8,
) -> [[u8; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT] {
    std::array::from_fn(|index| {
        register_pattern(seed.wrapping_add((index as u8).wrapping_mul(0x29)))
    })
}

fn fixed_point(
    rounding_mode: RiscvVectorFixedRoundingMode,
    saturated: bool,
) -> RiscvVectorFixedPointState {
    let mut state = RiscvVectorFixedPointState::new(rounding_mode);
    state.write_vxsat_bit(saturated);
    state
}

fn vector_state(
    seed: u8,
    config: RiscvVectorConfig,
    rounding_mode: RiscvVectorFixedRoundingMode,
    saturated: bool,
) -> RiscvVectorArchitecturalState {
    RiscvVectorArchitecturalState::new(
        config,
        fixed_point(rounding_mode, saturated),
        patterned_registers(seed),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreWitness {
    pc: Address,
    integer: u64,
    float: u64,
    vector: RiscvVectorArchitecturalState,
}

fn core_witness(core: &RiscvCore) -> CoreWitness {
    CoreWitness {
        pc: core.pc(),
        integer: core.read_register(reg(5)),
        float: core.read_float_register(freg(6)),
        vector: core.vector_architectural_state(),
    }
}

fn install_witness(core: &RiscvCore, seed: u8) -> CoreWitness {
    core.redirect_pc(Address::new(0xa000 + u64::from(seed) * 0x10));
    core.write_register(reg(5), 0x1111_0000_0000_0000 | u64::from(seed));
    core.write_float_register(freg(6), 0x2222_0000_0000_0000 | u64::from(seed));
    core.restore_vector_architectural_state(&vector_state(
        seed,
        RiscvVectorConfig::new(2, 0x48),
        RiscvVectorFixedRoundingMode::RoundDown,
        false,
    ));
    core_witness(core)
}

fn assert_witness(core: &RiscvCore, expected: &CoreWitness) {
    assert_eq!(&core_witness(core), expected);
}

struct CapturedFixture {
    core: RiscvCore,
    component: CheckpointComponentId,
    port: RiscvCoreCheckpointPort,
    registry: CheckpointRegistry,
}

fn captured_fixture() -> CapturedFixture {
    let core = riscv_core();
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(5), 0x5555);
    core.write_float_register(freg(6), 0x6666);
    core.restore_vector_architectural_state(&vector_state(
        0x21,
        RiscvVectorConfig::new(3, 0xd0),
        RiscvVectorFixedRoundingMode::RoundToOdd,
        true,
    ));
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();
    port.capture_into(&mut registry).unwrap();
    CapturedFixture {
        core,
        component,
        port,
        registry,
    }
}

fn rewrite_chunk(
    registry: &mut CheckpointRegistry,
    component: &CheckpointComponentId,
    name: &str,
    mutate: impl FnOnce(&mut Vec<u8>),
) {
    let mut payload = registry
        .chunk(component, name)
        .unwrap_or_else(|| panic!("missing captured chunk {name}"))
        .to_vec();
    mutate(&mut payload);
    registry.write_chunk(component, name, payload).unwrap();
}

#[derive(Clone, Copy, Debug)]
enum VectorPayloadCase {
    Valid,
    InvalidVersion,
    Absent,
}

fn arrange_vector_payload(
    registry: &mut CheckpointRegistry,
    component: &CheckpointComponentId,
    case: VectorPayloadCase,
) {
    match case {
        VectorPayloadCase::Valid => {}
        VectorPayloadCase::InvalidVersion => {
            rewrite_chunk(registry, component, VECTOR_STATE_CHUNK, |payload| {
                payload[0] = 7;
            });
        }
        VectorPayloadCase::Absent => {
            registry.remove_chunk(component, VECTOR_STATE_CHUNK);
        }
    }
}

fn assert_restore_error_without_mutation(
    corrupt: impl FnOnce(&CheckpointComponentId, &mut CheckpointRegistry),
    expected: impl FnOnce(CheckpointComponentId) -> RiscvCoreCheckpointError,
) {
    let mut fixture = captured_fixture();
    corrupt(&fixture.component, &mut fixture.registry);
    let before = install_witness(&fixture.core, 0xe1);

    let error = fixture.port.restore_from(&fixture.registry).unwrap_err();

    assert_eq!(error, expected(fixture.component.clone()));
    assert_witness(&fixture.core, &before);
}

fn execution_core(entry: u64) -> (RiscvCore, PartitionedScheduler, MemoryTransport) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    (core, scheduler, transport)
}

fn vmv_x_s_unmasked(vs2: u8, rd: u8) -> u32 {
    (0b010000 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (0b010 << 12)
        | (u32::from(rd) << 7)
        | 0x57
}

#[test]
fn riscv_checkpoint_captures_exact_versioned_vector_state_and_restores_all_architecture() {
    let core = riscv_core();
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let expected = vector_state(
        0x31,
        RiscvVectorConfig::new(3, 0xd0),
        RiscvVectorFixedRoundingMode::RoundToOdd,
        true,
    );
    core.redirect_pc(Address::new(0x8040));
    core.write_register(reg(5), 0x1122_3344_5566_7788);
    core.write_float_register(freg(6), 0x8877_6655_4433_2211);
    core.restore_vector_architectural_state(&expected);
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();

    let captured = port.capture_into(&mut registry).unwrap();

    let manifest = registry.capture("vector-chunk-inventory", 0).unwrap();
    let chunk_names = manifest
        .states()
        .iter()
        .find(|state| state.component() == &component)
        .expect("captured RISC-V core component")
        .chunks()
        .iter()
        .map(|chunk| chunk.name())
        .collect::<Vec<_>>();
    assert_eq!(
        chunk_names,
        [
            "bimode-branch-predictor",
            "branch-predictor",
            "fregs",
            "gshare-branch-predictor",
            "hart-run-state",
            "in-order-pipeline",
            "multiperspective-perceptron",
            "o3-runtime-state",
            "pc",
            "pmp",
            RISCV_STATE_VERSION_CHUNK,
            "tage-sc-l-branch-predictor",
            "tournament-branch-predictor",
            VECTOR_STATE_CHUNK,
            "xregs",
        ],
        "direct RISC-V capture must emit one exact checkpoint authority inventory"
    );
    assert_eq!(captured.vector_architectural_state(), &expected);
    assert_eq!(
        registry.chunk(&component, RISCV_STATE_VERSION_CHUNK),
        Some(&[1][..])
    );
    let payload = registry.chunk(&component, VECTOR_STATE_CHUNK).unwrap();
    assert_eq!(payload.len(), VECTOR_STATE_BYTES);
    assert_eq!(payload[0], 1);
    assert_eq!(&payload[1..5], &3_u32.to_le_bytes());
    assert_eq!(&payload[5..13], &0xd0_u64.to_le_bytes());
    assert_eq!(payload[13], expected.fixed_point().vcsr_bits());
    for index in 0..RISCV_VECTOR_REGISTER_COUNT {
        let offset = VECTOR_REGISTERS_OFFSET + index * RISCV_VECTOR_REGISTER_BYTES;
        assert_eq!(
            &payload[offset..offset + RISCV_VECTOR_REGISTER_BYTES],
            &expected.register(vreg(index as u8)),
            "vector payload register offset for v{index}"
        );
    }

    install_witness(&core, 0xf1);
    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(restored.vector_architectural_state(), &expected);
    assert_eq!(core.pc(), Address::new(0x8040));
    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    assert_eq!(core.read_float_register(freg(6)), 0x8877_6655_4433_2211);
    assert_eq!(core.vector_architectural_state(), expected);
}

#[test]
fn riscv_checkpoint_restored_vector_state_feeds_real_vmv_x_s_consumer() {
    let (core, mut scheduler, transport) = execution_core(0x8000);
    let component = CheckpointComponentId::new("cpu0").unwrap();
    let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
    let mut registers = patterned_registers(0x42);
    registers[vreg(8).index() as usize][..4].copy_from_slice(&(-128_i32).to_le_bytes());
    let expected = RiscvVectorArchitecturalState::new(
        RiscvVectorConfig::new(3, 0xd0),
        fixed_point(RiscvVectorFixedRoundingMode::RoundNearestEven, false),
        registers,
    );
    core.restore_vector_architectural_state(&expected);
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();
    port.capture_into(&mut registry).unwrap();
    install_witness(&core, 0xd1);

    port.restore_from(&registry).unwrap();
    assert_eq!(core.vector_architectural_state(), expected);
    let encoding = vmv_x_s_unmasked(8, 6);
    assert_eq!(encoding, 0x4280_2357);
    fetch_and_execute_one(
        &core,
        loaded_store(0x8000, encoding),
        &mut scheduler,
        &transport,
    );

    assert_eq!(core.read_register(reg(6)), 0xffff_ffff_ffff_ff80);
}

#[test]
fn riscv_checkpoint_legacy_pair_absence_restores_architectural_vector_defaults() {
    let mut fixture = captured_fixture();
    fixture
        .registry
        .remove_chunk(&fixture.component, RISCV_STATE_VERSION_CHUNK);
    fixture
        .registry
        .remove_chunk(&fixture.component, VECTOR_STATE_CHUNK);
    install_witness(&fixture.core, 0xc1);

    let restored = fixture.port.restore_from(&fixture.registry).unwrap();

    let expected = RiscvVectorArchitecturalState::default();
    assert_eq!(restored.vector_architectural_state(), &expected);
    assert_eq!(fixture.core.vector_architectural_state(), expected);
    assert_eq!(fixture.core.pc(), Address::new(0x8040));
    assert_eq!(fixture.core.read_register(reg(5)), 0x5555);
    assert_eq!(fixture.core.read_float_register(freg(6)), 0x6666);
}

#[test]
fn riscv_checkpoint_rejects_unpaired_marker_and_vector_without_mutation() {
    assert_restore_error_without_mutation(
        |component, registry| {
            registry.remove_chunk(component, VECTOR_STATE_CHUNK);
        },
        |component| RiscvCoreCheckpointError::MissingChunk {
            component,
            name: VECTOR_STATE_CHUNK.to_string(),
        },
    );
    assert_restore_error_without_mutation(
        |component, registry| {
            registry.remove_chunk(component, RISCV_STATE_VERSION_CHUNK);
        },
        |component| RiscvCoreCheckpointError::UnexpectedVectorStateWithoutVersion { component },
    );
}

#[test]
fn riscv_checkpoint_marker_errors_precede_vector_presence_policy() {
    for vector_payload in [
        VectorPayloadCase::Valid,
        VectorPayloadCase::InvalidVersion,
        VectorPayloadCase::Absent,
    ] {
        assert_restore_error_without_mutation(
            move |component, registry| {
                arrange_vector_payload(registry, component, vector_payload);
                registry
                    .write_chunk(component, RISCV_STATE_VERSION_CHUNK, vec![1, 0])
                    .unwrap();
            },
            |component| RiscvCoreCheckpointError::InvalidChunkSize {
                component,
                name: RISCV_STATE_VERSION_CHUNK.to_string(),
                expected: 1,
                actual: 2,
            },
        );
        assert_restore_error_without_mutation(
            move |component, registry| {
                arrange_vector_payload(registry, component, vector_payload);
                registry
                    .write_chunk(component, RISCV_STATE_VERSION_CHUNK, vec![9])
                    .unwrap();
            },
            |component| RiscvCoreCheckpointError::UnsupportedRiscvStateVersion {
                component,
                version: 9,
            },
        );
    }
}

#[test]
fn riscv_checkpoint_rejects_invalid_vector_payload_without_mutation() {
    assert_restore_error_without_mutation(
        |component, registry| {
            rewrite_chunk(registry, component, VECTOR_STATE_CHUNK, |payload| {
                payload.pop();
            });
        },
        |component| RiscvCoreCheckpointError::InvalidChunkSize {
            component,
            name: VECTOR_STATE_CHUNK.to_string(),
            expected: VECTOR_STATE_BYTES,
            actual: VECTOR_STATE_BYTES - 1,
        },
    );
    assert_restore_error_without_mutation(
        |component, registry| {
            rewrite_chunk(registry, component, VECTOR_STATE_CHUNK, |payload| {
                payload[0] = 7;
            });
        },
        |component| RiscvCoreCheckpointError::UnsupportedVectorStateVersion {
            component,
            version: 7,
        },
    );
    assert_restore_error_without_mutation(
        |component, registry| {
            rewrite_chunk(registry, component, VECTOR_STATE_CHUNK, |payload| {
                payload[13] = 0x87;
            });
        },
        |component| RiscvCoreCheckpointError::InvalidVectorStateVcsr {
            component,
            value: 0x87,
        },
    );
}

#[test]
fn riscv_checkpoint_bank_orders_and_restores_distinct_vector_authorities() {
    let core0 = riscv_core_with(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    let core1 = riscv_core_with(CpuId::new(1), PartitionId::new(1), AgentId::new(8), 0x9000);
    let state0 = vector_state(
        0x11,
        RiscvVectorConfig::new(3, 0xd0),
        RiscvVectorFixedRoundingMode::RoundToOdd,
        true,
    );
    let state1 = vector_state(
        0x91,
        RiscvVectorConfig::new(2, 0x48),
        RiscvVectorFixedRoundingMode::RoundDown,
        false,
    );
    core0.redirect_pc(Address::new(0x8040));
    core0.write_register(reg(1), 0x1111);
    core0.restore_vector_architectural_state(&state0);
    core1.redirect_pc(Address::new(0x9040));
    core1.write_register(reg(2), 0x2222);
    core1.restore_vector_architectural_state(&state1);
    let component0 = CheckpointComponentId::new("cpu0").unwrap();
    let component1 = CheckpointComponentId::new("cpu1").unwrap();
    let bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(component1.clone(), core1.clone()),
        RiscvCoreCheckpointPort::new(component0.clone(), core0.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();

    let captured = bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(captured[0].component(), &component0);
    assert_eq!(captured[0].vector_architectural_state(), &state0);
    assert_eq!(captured[1].component(), &component1);
    assert_eq!(captured[1].vector_architectural_state(), &state1);
    assert_ne!(
        registry.chunk(&component0, VECTOR_STATE_CHUNK),
        registry.chunk(&component1, VECTOR_STATE_CHUNK)
    );

    install_witness(&core0, 0xa1);
    install_witness(&core1, 0xa2);
    let restored = bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(core0.pc(), Address::new(0x8040));
    assert_eq!(core0.read_register(reg(1)), 0x1111);
    assert_eq!(core0.vector_architectural_state(), state0);
    assert_eq!(core1.pc(), Address::new(0x9040));
    assert_eq!(core1.read_register(reg(2)), 0x2222);
    assert_eq!(core1.vector_architectural_state(), state1);
}

#[test]
fn riscv_checkpoint_bank_rejects_cpu1_vector_during_predecode_without_cpu_mutation() {
    let core0 = riscv_core_with(CpuId::new(0), PartitionId::new(0), AgentId::new(7), 0x8000);
    let core1 = riscv_core_with(CpuId::new(1), PartitionId::new(1), AgentId::new(8), 0x9000);
    core0.restore_vector_architectural_state(&vector_state(
        0x12,
        RiscvVectorConfig::new(3, 0xd0),
        RiscvVectorFixedRoundingMode::RoundToOdd,
        true,
    ));
    core1.restore_vector_architectural_state(&vector_state(
        0x92,
        RiscvVectorConfig::new(2, 0x48),
        RiscvVectorFixedRoundingMode::RoundDown,
        false,
    ));
    let component0 = CheckpointComponentId::new("cpu0").unwrap();
    let component1 = CheckpointComponentId::new("cpu1").unwrap();
    let bank = RiscvCoreCheckpointBank::new([
        RiscvCoreCheckpointPort::new(component1.clone(), core1.clone()),
        RiscvCoreCheckpointPort::new(component0, core0.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    rewrite_chunk(&mut registry, &component1, VECTOR_STATE_CHUNK, |payload| {
        payload.pop();
    });
    let core0_before = install_witness(&core0, 0xd1);
    let core1_before = install_witness(&core1, 0xd2);

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(
        error,
        RiscvCoreCheckpointError::InvalidChunkSize {
            component: component1,
            name: VECTOR_STATE_CHUNK.to_string(),
            expected: VECTOR_STATE_BYTES,
            actual: VECTOR_STATE_BYTES - 1,
        }
    );
    assert_witness(&core0, &core0_before);
    assert_witness(&core1, &core1_before);
}
