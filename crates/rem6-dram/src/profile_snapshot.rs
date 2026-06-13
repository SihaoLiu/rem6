use std::fmt;

use rem6_memory::{CacheLineLayout, MemoryTargetId};

use crate::{
    DramControllerSnapshot, DramGeometry, DramMemoryError, DramTiming, ExternalMemoryProfile,
    NvmMediaTiming,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DramProfileSnapshotMismatch {
    Target {
        profile: MemoryTargetId,
        snapshot: MemoryTargetId,
    },
    LineLayout {
        profile_bytes: u64,
        store_bytes: u64,
    },
    Geometry {
        profile: DramGeometry,
        controller: DramGeometry,
    },
    Timing {
        profile: Box<DramTiming>,
        controller: Box<DramTiming>,
    },
    ParallelPorts {
        profile: u32,
        controller: u32,
    },
    NvmMediaTiming {
        profile: Option<NvmMediaTiming>,
        controller: Option<NvmMediaTiming>,
    },
}

impl fmt::Display for DramProfileSnapshotMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target { profile, snapshot } => write!(
                formatter,
                "profile target {} does not match snapshot target {}",
                profile.get(),
                snapshot.get()
            ),
            Self::LineLayout {
                profile_bytes,
                store_bytes,
            } => write!(
                formatter,
                "profile line layout {profile_bytes} does not match memory layout {store_bytes}"
            ),
            Self::Geometry {
                profile,
                controller,
            } => write!(
                formatter,
                "profile geometry {profile:?} does not match controller geometry {controller:?}"
            ),
            Self::Timing {
                profile,
                controller,
            } => write!(
                formatter,
                "profile timing {profile:?} does not match controller timing {controller:?}"
            ),
            Self::ParallelPorts {
                profile,
                controller,
            } => write!(
                formatter,
                "profile parallel ports {profile} do not match controller parallel ports {controller}"
            ),
            Self::NvmMediaTiming {
                profile,
                controller,
            } => write!(
                formatter,
                "profile NVM media timing {profile:?} does not match controller NVM media timing {controller:?}"
            ),
        }
    }
}

pub(crate) fn validate_profile_snapshot(
    snapshot_target: MemoryTargetId,
    store_layout: CacheLineLayout,
    controller: &DramControllerSnapshot,
    profile: ExternalMemoryProfile,
) -> Result<(), DramMemoryError> {
    if profile.target() != snapshot_target {
        return Err(profile_snapshot_mismatch(
            snapshot_target,
            DramProfileSnapshotMismatch::Target {
                profile: profile.target(),
                snapshot: snapshot_target,
            },
        ));
    }
    if profile.line_layout() != store_layout {
        return Err(profile_snapshot_mismatch(
            snapshot_target,
            DramProfileSnapshotMismatch::LineLayout {
                profile_bytes: profile.line_layout().bytes(),
                store_bytes: store_layout.bytes(),
            },
        ));
    }
    if profile.geometry() != controller.geometry() {
        return Err(profile_snapshot_mismatch(
            snapshot_target,
            DramProfileSnapshotMismatch::Geometry {
                profile: profile.geometry(),
                controller: controller.geometry(),
            },
        ));
    }
    if profile.timing() != controller.timing() {
        return Err(profile_snapshot_mismatch(
            snapshot_target,
            DramProfileSnapshotMismatch::Timing {
                profile: Box::new(profile.timing()),
                controller: Box::new(controller.timing()),
            },
        ));
    }
    if profile.parallel_port_count() != controller.parallel_port_count() {
        return Err(profile_snapshot_mismatch(
            snapshot_target,
            DramProfileSnapshotMismatch::ParallelPorts {
                profile: profile.parallel_port_count(),
                controller: controller.parallel_port_count(),
            },
        ));
    }
    if profile.nvm_media_timing() != controller.nvm_media_timing() {
        return Err(profile_snapshot_mismatch(
            snapshot_target,
            DramProfileSnapshotMismatch::NvmMediaTiming {
                profile: profile.nvm_media_timing(),
                controller: controller.nvm_media_timing(),
            },
        ));
    }
    Ok(())
}

fn profile_snapshot_mismatch(
    target: MemoryTargetId,
    mismatch: DramProfileSnapshotMismatch,
) -> DramMemoryError {
    DramMemoryError::profile_snapshot_mismatch(target, mismatch)
}
