#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadManifestIdentity(String);

impl WorkloadManifestIdentity {
    pub(crate) fn new(hash: u64) -> Self {
        Self(format!("wl-{hash:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
