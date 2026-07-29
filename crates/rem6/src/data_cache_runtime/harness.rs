use super::*;

pub(super) enum CliDataCacheHarness {
    Msi(Arc<Mutex<MsiBankDirectoryHarness>>),
    Mesi(CliMesiLineHarnesses),
    Moesi(CliMoesiLineHarnesses),
    Chi(CliChiLineHarnesses),
}

pub(super) struct CliMesiLineHarnesses {
    pub(super) agents: Vec<AgentId>,
    pub(super) lines: BTreeMap<Address, MesiDirectoryLineHarness>,
}

pub(super) struct CliMoesiLineHarnesses {
    pub(super) agents: Vec<AgentId>,
    pub(super) lines: BTreeMap<Address, MoesiDirectoryLineHarness>,
}

pub(super) struct CliChiLineHarnesses {
    pub(super) agents: Vec<AgentId>,
    pub(super) lines: BTreeMap<Address, ChiDirectoryLineHarness>,
}
