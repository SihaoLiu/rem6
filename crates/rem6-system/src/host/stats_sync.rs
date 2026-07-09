use std::{fmt, sync::Arc};

use rem6_stats::StatsRegistry;

use crate::SystemError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatsSyncPhase {
    BeforeDump,
    BeforeReset,
    AfterReset,
}

type StatsSyncHookFn =
    dyn Fn(&mut StatsRegistry, StatsSyncPhase) -> Result<(), SystemError> + Send + Sync;

#[derive(Clone)]
pub(super) struct StatsSyncHook(Arc<StatsSyncHookFn>);

impl StatsSyncHook {
    pub(super) fn new<F>(hook: F) -> Self
    where
        F: Fn(&mut StatsRegistry, StatsSyncPhase) -> Result<(), SystemError>
            + Send
            + Sync
            + 'static,
    {
        Self(Arc::new(hook))
    }

    pub(super) fn sync(
        &self,
        registry: &mut StatsRegistry,
        phase: StatsSyncPhase,
    ) -> Result<(), SystemError> {
        (self.0)(registry, phase)
    }
}

impl fmt::Debug for StatsSyncHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StatsSyncHook")
    }
}

impl super::SystemActionExecutor {
    pub(crate) fn set_pre_stats_sync<F>(&mut self, hook: F)
    where
        F: Fn(&mut StatsRegistry, StatsSyncPhase) -> Result<(), SystemError>
            + Send
            + Sync
            + 'static,
    {
        self.pre_stats_sync = Some(StatsSyncHook::new(hook));
    }
}
