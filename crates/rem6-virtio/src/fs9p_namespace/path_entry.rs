use std::collections::BTreeMap;

use crate::fs9p_protocol::{VIRTIO_9P_EEXIST, VIRTIO_9P_ENOTEMPTY};

use super::{Virtio9pFidPath, Virtio9pNode, Virtio9pNodeId, Virtio9pRenameOutcome};

pub(super) fn node_exists_at_fid_path(
    entries: &BTreeMap<String, Virtio9pNode>,
    fid_path: &Virtio9pFidPath,
    expected: Virtio9pNodeId,
) -> bool {
    node_exists_at_components(entries, fid_path.components(), expected)
}

pub(super) fn rename_node_at_fid_path(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    fid_path: &Virtio9pFidPath,
    expected: Virtio9pNodeId,
    newname: &str,
) -> Option<Result<Virtio9pRenameOutcome, u32>> {
    rename_node_at_components(entries, fid_path.components(), expected, newname)
}

pub(super) fn remove_node_at_fid_path(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    fid_path: &Virtio9pFidPath,
    expected: Virtio9pNodeId,
) -> Option<Result<(), u32>> {
    remove_node_at_components(entries, fid_path.components(), expected)
}

pub(super) fn take_file_node_at_fid_path(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    fid_path: &Virtio9pFidPath,
    expected: Virtio9pNodeId,
) -> Option<Virtio9pNode> {
    take_file_node_at_components(entries, fid_path.components(), expected)
}

fn rename_node_at_components(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    components: &[String],
    expected: Virtio9pNodeId,
    newname: &str,
) -> Option<Result<Virtio9pRenameOutcome, u32>> {
    let (name, remaining) = components.split_first()?;
    if remaining.is_empty() {
        let node = entries.get(name)?;
        if node.id() != expected {
            return None;
        }
        return Some(rename_node_in_entries(entries, name, expected, newname));
    }
    let Virtio9pNode::Directory(directory) = entries.get_mut(name)? else {
        return None;
    };
    rename_node_at_components(&mut directory.entries, remaining, expected, newname)
}

fn node_exists_at_components(
    entries: &BTreeMap<String, Virtio9pNode>,
    components: &[String],
    expected: Virtio9pNodeId,
) -> bool {
    let Some((name, remaining)) = components.split_first() else {
        return false;
    };
    let Some(node) = entries.get(name) else {
        return false;
    };
    if remaining.is_empty() {
        return node.id() == expected;
    }
    let Virtio9pNode::Directory(directory) = node else {
        return false;
    };
    node_exists_at_components(&directory.entries, remaining, expected)
}

pub(super) fn rename_node_in_entries(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    oldname: &str,
    expected: Virtio9pNodeId,
    newname: &str,
) -> Result<Virtio9pRenameOutcome, u32> {
    if oldname == newname {
        return Ok(Virtio9pRenameOutcome {
            moved: false,
            replaced: None,
        });
    }
    let source_is_directory = entries
        .get(oldname)
        .is_some_and(|node| matches!(node, Virtio9pNode::Directory(_)));
    match entries.get(newname) {
        Some(existing) if existing.id() == expected => {
            return Ok(Virtio9pRenameOutcome {
                moved: false,
                replaced: None,
            });
        }
        Some(Virtio9pNode::Directory(directory)) if source_is_directory => {
            if !directory.entries.is_empty() {
                return Err(VIRTIO_9P_ENOTEMPTY);
            }
        }
        Some(Virtio9pNode::Directory(_)) => return Err(VIRTIO_9P_EEXIST),
        Some(Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) | Virtio9pNode::Special(_))
            if source_is_directory =>
        {
            return Err(VIRTIO_9P_EEXIST);
        }
        Some(Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) | Virtio9pNode::Special(_))
        | None => {}
    }
    let node = entries
        .remove(oldname)
        .expect("prevalidated 9p rename node");
    let replaced = entries
        .insert(newname.to_string(), node)
        .map(|node| node.id());
    Ok(Virtio9pRenameOutcome {
        moved: true,
        replaced,
    })
}

fn remove_node_at_components(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    components: &[String],
    expected: Virtio9pNodeId,
) -> Option<Result<(), u32>> {
    let (name, remaining) = components.split_first()?;
    if remaining.is_empty() {
        let node = entries.get(name)?;
        if node.id() != expected {
            return None;
        }
        return Some(match node {
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) | Virtio9pNode::Special(_) => {
                entries.remove(name);
                Ok(())
            }
            Virtio9pNode::Directory(directory) if directory.entries.is_empty() => {
                entries.remove(name);
                Ok(())
            }
            Virtio9pNode::Directory(_) => Err(VIRTIO_9P_ENOTEMPTY),
        });
    }
    let Virtio9pNode::Directory(directory) = entries.get_mut(name)? else {
        return None;
    };
    remove_node_at_components(&mut directory.entries, remaining, expected)
}

fn take_file_node_at_components(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    components: &[String],
    expected: Virtio9pNodeId,
) -> Option<Virtio9pNode> {
    let (name, remaining) = components.split_first()?;
    if remaining.is_empty() {
        let node = entries.get(name)?;
        if node.id() != expected {
            return None;
        }
        return match node {
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) | Virtio9pNode::Special(_) => {
                entries.remove(name)
            }
            Virtio9pNode::Directory(_) => None,
        };
    }
    let Virtio9pNode::Directory(directory) = entries.get_mut(name)? else {
        return None;
    };
    take_file_node_at_components(&mut directory.entries, remaining, expected)
}
