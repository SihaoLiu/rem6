use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfOperatingSystem, BootImage,
};
use rem6_workload::{WorkloadBootImage, WorkloadId, WorkloadManifest};

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_u16_be(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn write_u64_be(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}

fn elf64_image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x104];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 1);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x100);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(&mut bytes, 96, 4);
    write_u64(&mut bytes, 104, 4);
    write_u64(&mut bytes, 112, 0x1000);
    bytes[0x100..0x104].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn elf64_image_with_interpreter(machine: u16, interpreter: &str) -> Vec<u8> {
    let mut bytes = vec![0; 0x204];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x200);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(&mut bytes, 96, 4);
    write_u64(&mut bytes, 104, 4);
    write_u64(&mut bytes, 112, 0x1000);

    let mut interpreter_bytes = interpreter.as_bytes().to_vec();
    interpreter_bytes.push(0);
    write_u32(&mut bytes, 120, 3);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0x180);
    write_u64(&mut bytes, 136, 0);
    write_u64(&mut bytes, 144, 0);
    write_u64(&mut bytes, 152, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 160, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 168, 1);

    bytes[0x180..0x180 + interpreter_bytes.len()].copy_from_slice(&interpreter_bytes);
    bytes[0x200..0x204].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn elf64_be_image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x104];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 2;
    bytes[6] = 1;
    write_u16_be(&mut bytes, 16, 2);
    write_u16_be(&mut bytes, 18, machine);
    write_u32_be(&mut bytes, 20, 1);
    write_u64_be(&mut bytes, 24, 0x8004);
    write_u64_be(&mut bytes, 32, 64);
    write_u16_be(&mut bytes, 52, 64);
    write_u16_be(&mut bytes, 54, 56);
    write_u16_be(&mut bytes, 56, 1);

    write_u32_be(&mut bytes, 64, 1);
    write_u32_be(&mut bytes, 68, 5);
    write_u64_be(&mut bytes, 72, 0x100);
    write_u64_be(&mut bytes, 80, 0x8000);
    write_u64_be(&mut bytes, 88, 0x8000);
    write_u64_be(&mut bytes, 96, 4);
    write_u64_be(&mut bytes, 104, 4);
    write_u64_be(&mut bytes, 112, 0x1000);
    bytes[0x100..0x104].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

#[test]
fn workload_boot_image_preserves_elf_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let metadata = image.elf_metadata().unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert_eq!(workload_image.elf_metadata(), Some(metadata));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_metadata()
            .unwrap()
            .architecture(),
        BootElfArchitecture::Riscv64,
    );
}

#[test]
fn workload_boot_image_preserves_elf_interpreter_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-linux-riscv64-lp64d.so.1",
    ))
    .unwrap();
    let interpreter = image.elf_interpreter().unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert_eq!(workload_image.elf_interpreter(), Some(interpreter));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_interpreter()
            .unwrap()
            .path(),
        "/lib/ld-linux-riscv64-lp64d.so.1",
    );
}

#[test]
fn workload_manifest_identity_includes_elf_metadata() {
    let riscv = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let x86 = BootImage::from_elf64_le(&elf64_image(62)).unwrap();

    assert_eq!(riscv.entry(), x86.entry());
    assert_eq!(riscv.segments(), x86.segments());
    assert_eq!(riscv.elf_metadata().unwrap().class(), BootElfClass::Class64,);
    assert_ne!(riscv.elf_metadata(), x86.elf_metadata());

    let riscv_manifest = WorkloadManifest::builder(id("same"), riscv)
        .build()
        .unwrap();
    let x86_manifest = WorkloadManifest::builder(id("same"), x86).build().unwrap();

    assert_ne!(riscv_manifest.identity(), x86_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_interpreter_metadata() {
    let musl = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-musl-riscv64.so.1",
    ))
    .unwrap();
    let glibc = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-linux-riscv64-lp64d.so.1",
    ))
    .unwrap();

    assert_eq!(musl.entry(), glibc.entry());
    assert_eq!(musl.segments(), glibc.segments());
    assert_eq!(musl.elf_metadata(), glibc.elf_metadata());
    assert_ne!(musl.elf_interpreter(), glibc.elf_interpreter());

    let musl_manifest = WorkloadManifest::builder(id("same"), musl).build().unwrap();
    let glibc_manifest = WorkloadManifest::builder(id("same"), glibc)
        .build()
        .unwrap();

    assert_ne!(musl_manifest.identity(), glibc_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_endian_metadata() {
    let little = BootImage::from_elf(&elf64_image(2)).unwrap();
    let big = BootImage::from_elf(&elf64_be_image(2)).unwrap();

    assert_eq!(little.entry(), big.entry());
    assert_eq!(little.segments(), big.segments());
    assert_eq!(
        little.elf_metadata().unwrap().endian(),
        BootElfEndian::Little
    );
    assert_eq!(big.elf_metadata().unwrap().endian(), BootElfEndian::Big);
    assert_ne!(little.elf_metadata(), big.elf_metadata());

    let little_manifest = WorkloadManifest::builder(id("same"), little)
        .build()
        .unwrap();
    let big_manifest = WorkloadManifest::builder(id("same"), big).build().unwrap();

    assert_ne!(little_manifest.identity(), big_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_power64_endian_default_abi() {
    let little = BootImage::from_elf(&elf64_image(21)).unwrap();
    let big = BootImage::from_elf(&elf64_be_image(21)).unwrap();

    assert_eq!(little.entry(), big.entry());
    assert_eq!(little.segments(), big.segments());
    assert_eq!(
        little.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV2,
    );
    assert_eq!(
        big.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV1,
    );

    let little_manifest = WorkloadManifest::builder(id("same"), little)
        .build()
        .unwrap();
    let big_manifest = WorkloadManifest::builder(id("same"), big).build().unwrap();

    assert_ne!(little_manifest.identity(), big_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_operating_system_metadata() {
    let mut linux_bytes = elf64_image(243);
    linux_bytes[7] = 3;
    let mut freebsd_bytes = elf64_image(243);
    freebsd_bytes[7] = 9;

    let linux = BootImage::from_elf64_le(&linux_bytes).unwrap();
    let freebsd = BootImage::from_elf64_le(&freebsd_bytes).unwrap();

    assert_eq!(linux.entry(), freebsd.entry());
    assert_eq!(linux.segments(), freebsd.segments());
    assert_eq!(
        linux.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::Linux,
    );
    assert_eq!(
        freebsd.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::FreeBsd,
    );

    let linux_manifest = WorkloadManifest::builder(id("same"), linux)
        .build()
        .unwrap();
    let freebsd_manifest = WorkloadManifest::builder(id("same"), freebsd)
        .build()
        .unwrap();

    assert_ne!(linux_manifest.identity(), freebsd_manifest.identity());
}
