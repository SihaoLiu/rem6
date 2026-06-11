use super::*;
use rem6_memory::Address;

fn startup_segment(startup: &RiscvSeStartupImage) -> (u64, &[u8]) {
    (startup.stack_range().start().get(), startup.stack_data())
}

fn read_stack_u64(startup: &RiscvSeStartupImage, address: u64) -> u64 {
    let (base, bytes) = startup_segment(startup);
    let offset = usize::try_from(address.checked_sub(base).unwrap()).unwrap();
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn read_stack_bytes(startup: &RiscvSeStartupImage, address: u64, len: usize) -> Vec<u8> {
    let (base, bytes) = startup_segment(startup);
    let offset = usize::try_from(address.checked_sub(base).unwrap()).unwrap();
    bytes[offset..offset + len].to_vec()
}

fn read_stack_c_string(startup: &RiscvSeStartupImage, address: u64) -> Vec<u8> {
    let (base, bytes) = startup_segment(startup);
    let offset = usize::try_from(address.checked_sub(base).unwrap()).unwrap();
    let end = bytes[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|pos| offset + pos + 1)
        .unwrap();
    bytes[offset..end].to_vec()
}

fn auxv_pairs(startup: &RiscvSeStartupImage, start: u64) -> Vec<RiscvSeAuxvEntry> {
    let mut pairs = Vec::new();
    let mut cursor = start;
    loop {
        let key = read_stack_u64(startup, cursor);
        let value = read_stack_u64(startup, cursor + 8);
        pairs.push(RiscvSeAuxvEntry::new(key, value));
        if key == RISCV_LINUX_AT_NULL {
            return pairs;
        }
        cursor += 16;
    }
}

#[test]
fn startup_stack_builds_arg_env_auxv_frame() {
    let startup = RiscvSeStartupConfig::new(Address::new(0xa000))
        .with_arg("hello")
        .with_env("LC_ALL=C")
        .with_auxv_entry(RiscvSeAuxvEntry::new(RISCV_LINUX_AT_ENTRY, 0x8000))
        .with_random_bytes([0xa5; 16])
        .build()
        .unwrap();

    let sp = startup.initial_stack_pointer().get();
    assert_eq!(sp % 16, 0);
    assert_eq!(read_stack_u64(&startup, sp), 1);

    let argv0 = read_stack_u64(&startup, sp + 8);
    assert_eq!(read_stack_u64(&startup, sp + 16), 0);
    assert_eq!(read_stack_c_string(&startup, argv0), b"hello\0");

    let env0 = read_stack_u64(&startup, sp + 24);
    assert_eq!(read_stack_u64(&startup, sp + 32), 0);
    assert_eq!(read_stack_c_string(&startup, env0), b"LC_ALL=C\0");

    let auxv = auxv_pairs(&startup, sp + 40);
    assert_eq!(
        auxv,
        vec![
            RiscvSeAuxvEntry::new(RISCV_LINUX_AT_ENTRY, 0x8000),
            RiscvSeAuxvEntry::new(RISCV_LINUX_AT_PAGESZ, RISCV_PAGE_BYTES),
            RiscvSeAuxvEntry::new(RISCV_LINUX_AT_SECURE, 0),
            RiscvSeAuxvEntry::new(RISCV_LINUX_AT_RANDOM, startup.random_address().get()),
            RiscvSeAuxvEntry::new(RISCV_LINUX_AT_NULL, 0),
        ]
    );
    assert_eq!(
        read_stack_bytes(&startup, startup.random_address().get(), 16),
        vec![0xa5; 16]
    );
}

#[test]
fn startup_stack_rejects_interior_nul_argument() {
    let error = RiscvSeStartupConfig::new(Address::new(0xa000))
        .with_arg(b"bad\0arg")
        .build()
        .unwrap_err();

    assert_eq!(
        error,
        RiscvSeStartupError::InteriorNul {
            field: RiscvSeStartupStringField::Argument,
            index: 0
        }
    );
}

#[test]
fn startup_stack_rejects_explicit_default_auxv_key() {
    let error = RiscvSeStartupConfig::new(Address::new(0xa000))
        .with_auxv_entry(RiscvSeAuxvEntry::new(RISCV_LINUX_AT_NULL, 0))
        .build()
        .unwrap_err();

    assert_eq!(
        error,
        RiscvSeStartupError::ReservedAuxvEntry {
            key: RISCV_LINUX_AT_NULL,
            index: 0
        }
    );
}
