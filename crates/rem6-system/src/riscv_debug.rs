use rem6_debug::{
    GdbRemoteFeature, GdbRemoteFeatureValue, GdbRemoteSession, DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
};
use rem6_isa_riscv::{RiscvGdbTargetDescription, RiscvGdbXlen};

pub fn riscv_gdb_remote_session(xlen: RiscvGdbXlen) -> GdbRemoteSession {
    let mut session = GdbRemoteSession::new(vec![
        GdbRemoteFeature::new(
            b"PacketSize".to_vec(),
            GdbRemoteFeatureValue::Value(
                format!("{DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES:x}").into_bytes(),
            ),
        ),
        GdbRemoteFeature::new(
            b"qXfer:features:read".to_vec(),
            GdbRemoteFeatureValue::Supported,
        ),
    ]);

    for document in RiscvGdbTargetDescription::new(xlen).into_documents() {
        let (annex, content) = document.into_parts();
        let registered = session.set_xfer_feature(annex.as_bytes().to_vec(), content);
        debug_assert!(registered);
    }

    session
}
