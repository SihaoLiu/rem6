#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3RegisterClass {
    Integer,
    FloatingPoint,
    Vector,
    ConditionCode,
    Misc,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct O3PhysicalRegisterId(u32);

impl O3PhysicalRegisterId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn invalid() -> Self {
        Self(u32::MAX)
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    pub const fn is_invalid(self) -> bool {
        self.0 == u32::MAX
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3SourceRegister {
    Invalid,
    Mapped {
        register_class: O3RegisterClass,
        physical: O3PhysicalRegisterId,
        scoreboard_ready: bool,
    },
}

impl O3SourceRegister {
    pub const fn invalid() -> Self {
        Self::Invalid
    }

    pub const fn mapped(
        register_class: O3RegisterClass,
        physical: O3PhysicalRegisterId,
        scoreboard_ready: bool,
    ) -> Self {
        Self::Mapped {
            register_class,
            physical,
            scoreboard_ready,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3SourceRenameReason {
    InvalidRegisterClassReady,
    ScoreboardReady,
    ScoreboardNotReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3SourceRenameDecision {
    source_index: usize,
    register_class: Option<O3RegisterClass>,
    physical: O3PhysicalRegisterId,
    consult_scoreboard: bool,
    mark_ready: bool,
    reason: O3SourceRenameReason,
}

impl O3SourceRenameDecision {
    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    pub const fn register_class(&self) -> Option<O3RegisterClass> {
        self.register_class
    }

    pub const fn physical(&self) -> O3PhysicalRegisterId {
        self.physical
    }

    pub const fn consults_scoreboard(&self) -> bool {
        self.consult_scoreboard
    }

    pub const fn mark_ready(&self) -> bool {
        self.mark_ready
    }

    pub const fn reason(&self) -> O3SourceRenameReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3SourceRenamePlan {
    decisions: Vec<O3SourceRenameDecision>,
}

impl O3SourceRenamePlan {
    pub fn for_sources<I>(sources: I) -> Self
    where
        I: IntoIterator<Item = O3SourceRegister>,
    {
        let decisions = sources
            .into_iter()
            .enumerate()
            .map(|(source_index, source)| match source {
                O3SourceRegister::Invalid => O3SourceRenameDecision {
                    source_index,
                    register_class: None,
                    physical: O3PhysicalRegisterId::invalid(),
                    consult_scoreboard: false,
                    mark_ready: true,
                    reason: O3SourceRenameReason::InvalidRegisterClassReady,
                },
                O3SourceRegister::Mapped {
                    register_class,
                    physical,
                    scoreboard_ready,
                } => O3SourceRenameDecision {
                    source_index,
                    register_class: Some(register_class),
                    physical,
                    consult_scoreboard: true,
                    mark_ready: scoreboard_ready,
                    reason: if scoreboard_ready {
                        O3SourceRenameReason::ScoreboardReady
                    } else {
                        O3SourceRenameReason::ScoreboardNotReady
                    },
                },
            })
            .collect();

        Self { decisions }
    }

    pub fn decisions(&self) -> &[O3SourceRenameDecision] {
        &self.decisions
    }

    pub fn scoreboard_lookup_count(&self) -> usize {
        self.decisions
            .iter()
            .filter(|decision| decision.consult_scoreboard)
            .count()
    }

    pub fn has_ready_source(&self) -> bool {
        self.decisions.iter().any(|decision| decision.mark_ready)
    }

    pub fn has_blocked_source(&self) -> bool {
        self.decisions.iter().any(|decision| !decision.mark_ready)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3DestinationVisibility {
    Writeback,
    Commit,
    AlwaysReady,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct O3DestinationRegister {
    register_class: O3RegisterClass,
    visibility: O3DestinationVisibility,
}

impl O3DestinationRegister {
    pub const fn writeback_visible(register_class: O3RegisterClass) -> Self {
        Self {
            register_class,
            visibility: O3DestinationVisibility::Writeback,
        }
    }

    pub const fn commit_visible_misc() -> Self {
        Self {
            register_class: O3RegisterClass::Misc,
            visibility: O3DestinationVisibility::Commit,
        }
    }

    pub const fn always_ready_misc() -> Self {
        Self {
            register_class: O3RegisterClass::Misc,
            visibility: O3DestinationVisibility::AlwaysReady,
        }
    }

    pub const fn register_class(self) -> O3RegisterClass {
        self.register_class
    }

    pub const fn visibility(self) -> O3DestinationVisibility {
        self.visibility
    }

    pub const fn is_misc(self) -> bool {
        matches!(self.register_class, O3RegisterClass::Misc)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3DependencyProducerKind {
    Compute,
    Memory,
    Barrier,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3DependencyReleaseStage {
    Writeback,
    Commit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3DependencyReleaseReason {
    WritebackVisibleDestinationPublished,
    CommitVisibleDestinationDeferred,
    CommitVisibleDestinationPublished,
    DestinationAlreadyPublished,
    AlwaysReadyFixedMapping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3DestinationRelease {
    destination: O3DestinationRegister,
    wake_dependents: bool,
    mark_scoreboard_ready: bool,
    reason: O3DependencyReleaseReason,
}

impl O3DestinationRelease {
    pub const fn destination(self) -> O3DestinationRegister {
        self.destination
    }

    pub const fn wake_dependents(self) -> bool {
        self.wake_dependents
    }

    pub const fn mark_scoreboard_ready(self) -> bool {
        self.mark_scoreboard_ready
    }

    pub const fn reason(self) -> O3DependencyReleaseReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3DependencyReleasePlan {
    stage: O3DependencyReleaseStage,
    producer_kind: O3DependencyProducerKind,
    complete_memory_dependencies: bool,
    destination_releases: Vec<O3DestinationRelease>,
}

impl O3DependencyReleasePlan {
    pub fn for_stage<I>(stage: O3DependencyReleaseStage, destinations: I) -> Self
    where
        I: IntoIterator<Item = O3DestinationRegister>,
    {
        Self::for_stage_with_producer(stage, O3DependencyProducerKind::Compute, destinations)
    }

    pub fn for_stage_with_producer<I>(
        stage: O3DependencyReleaseStage,
        producer_kind: O3DependencyProducerKind,
        destinations: I,
    ) -> Self
    where
        I: IntoIterator<Item = O3DestinationRegister>,
    {
        let complete_memory_dependencies =
            should_complete_memory_dependencies(stage, producer_kind);
        let destination_releases = destinations
            .into_iter()
            .map(|destination| destination_release(stage, destination))
            .collect();

        Self {
            stage,
            producer_kind,
            complete_memory_dependencies,
            destination_releases,
        }
    }

    pub const fn stage(&self) -> O3DependencyReleaseStage {
        self.stage
    }

    pub const fn producer_kind(&self) -> O3DependencyProducerKind {
        self.producer_kind
    }

    pub const fn complete_memory_dependencies(&self) -> bool {
        self.complete_memory_dependencies
    }

    pub fn destination_releases(&self) -> &[O3DestinationRelease] {
        &self.destination_releases
    }

    pub fn wakes_any_dependents(&self) -> bool {
        self.destination_releases
            .iter()
            .any(|release| release.wake_dependents)
    }

    pub fn marks_any_scoreboard_ready(&self) -> bool {
        self.destination_releases
            .iter()
            .any(|release| release.mark_scoreboard_ready)
    }
}

fn should_complete_memory_dependencies(
    stage: O3DependencyReleaseStage,
    producer_kind: O3DependencyProducerKind,
) -> bool {
    stage == O3DependencyReleaseStage::Writeback
        && matches!(
            producer_kind,
            O3DependencyProducerKind::Memory | O3DependencyProducerKind::Barrier
        )
}

fn destination_release(
    stage: O3DependencyReleaseStage,
    destination: O3DestinationRegister,
) -> O3DestinationRelease {
    let (wake_dependents, mark_scoreboard_ready, reason) = match destination.visibility {
        O3DestinationVisibility::AlwaysReady => (
            false,
            false,
            O3DependencyReleaseReason::AlwaysReadyFixedMapping,
        ),
        O3DestinationVisibility::Writeback => match stage {
            O3DependencyReleaseStage::Writeback => (
                true,
                true,
                O3DependencyReleaseReason::WritebackVisibleDestinationPublished,
            ),
            O3DependencyReleaseStage::Commit => (
                false,
                false,
                O3DependencyReleaseReason::DestinationAlreadyPublished,
            ),
        },
        O3DestinationVisibility::Commit => match stage {
            O3DependencyReleaseStage::Writeback => (
                false,
                false,
                O3DependencyReleaseReason::CommitVisibleDestinationDeferred,
            ),
            O3DependencyReleaseStage::Commit => (
                true,
                true,
                O3DependencyReleaseReason::CommitVisibleDestinationPublished,
            ),
        },
    };

    O3DestinationRelease {
        destination,
        wake_dependents,
        mark_scoreboard_ready,
        reason,
    }
}
