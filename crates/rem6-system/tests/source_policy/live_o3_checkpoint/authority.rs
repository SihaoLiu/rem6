use super::*;

#[test]
fn scheduler_authorized_restore_and_install_are_closed_contracts() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = read(crate_dir, "src/riscv_checkpoint/restore_authority.rs");
    let authorized = unconditional_method_body(
        &source,
        "RiscvCoreCheckpointBank",
        "restore_all_from_with_scheduler_authority",
    );
    let install = unconditional_method_body(
        &source,
        "RiscvCoreCheckpointBank",
        "install_decoded_restores",
    );
    let decode =
        unconditional_method_body(&source, "RiscvCoreCheckpointBank", "decode_and_prepare_all");
    assert_eq!(
        authorized,
        concat!(
            "pub(crate)fnrestore_all_from_with_scheduler_authority(&self,",
            "registry:&CheckpointRegistry,)->Result<Vec<RiscvCoreCheckpointRecord>,",
            "RiscvCoreCheckpointError>{letdecoded=self.decode_and_prepare_all(registry)?;",
            "Ok(Self::install_decoded_restores(decoded))}"
        )
    );
    assert_eq!(
        install,
        concat!(
            "fninstall_decoded_restores(decoded:Vec<(&RiscvCoreCheckpointPort,",
            "RiscvCoreCheckpointRecord,PreparedRiscvCoreRestore,)>,)->",
            "Vec<RiscvCoreCheckpointRecord>{letmutrestored=Vec::new();",
            "for(port,record,prepared)indecoded{",
            "port.core.install_prepared_checkpoint_restore(prepared);",
            "restored.push(record);}restored}"
        )
    );
    assert_eq!(
        decode,
        concat!(
            "fndecode_and_prepare_all<'a>(&'aself,registry:&CheckpointRegistry,)->",
            "Result<Vec<(&'aRiscvCoreCheckpointPort,RiscvCoreCheckpointRecord,",
            "PreparedRiscvCoreRestore,)>,RiscvCoreCheckpointError,>{",
            "letmutdecoded=Vec::with_capacity(self.ports.len());",
            "forportinself.ports.values(){decoded.push((port,port.decode_from(registry)?));}",
            "decoded.into_iter().map(|(port,record)|{",
            "letprepared=port.prepare_record(&record)?;",
            "Ok((port,record,prepared))}).collect()}"
        )
    );
}

#[test]
fn wake_authority_payload_decode_and_equality_are_one_closed_contract() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = read(crate_dir, "src/riscv_checkpoint/live_wake.rs");
    let validate = unconditional_function_body(&source, "validate");
    assert_eq!(
        validate,
        concat!(
            "pub(super)fnvalidate(component:&CheckpointComponentId,payload:&[u8],",
            "expected:RiscvO3LiveCheckpointWake,)->Result<(),RiscvCoreCheckpointError>{",
            "letinvalid=|reason:&str|RiscvCoreCheckpointError::InvalidO3LiveWakeAuthority{",
            "component:component.clone(),reason:reason.to_string(),};",
            "ifpayload.len()!=ENCODED_BYTES{returnErr(invalid());}",
            "if&payload[..4]!=MAGIC{returnErr(invalid());}",
            "ifpayload[4]!=VERSION{returnErr(invalid());}",
            "letkind=matchpayload[33]{0=>ScheduledEventKind::Serial,",
            "1=>ScheduledEventKind::Parallel,_=>returnErr(invalid()),};",
            "letauthority=RiscvO3LiveCheckpointWake{",
            "scheduler_instance_raw:u64::from_le_bytes(payload[5..13].try_into().unwrap()),",
            "partition:PartitionId::new(u32::from_le_bytes(payload[13..17].try_into().unwrap())),",
            "tick:u64::from_le_bytes(payload[17..25].try_into().unwrap()),",
            "scheduler_order:u64::from_le_bytes(payload[25..33].try_into().unwrap()),kind,};",
            "ifauthority!=expected{returnErr(",
            "RiscvCoreCheckpointError::MismatchedO3LiveWakeAuthority{",
            "component:component.clone(),});}Ok(())}"
        )
    );
}
