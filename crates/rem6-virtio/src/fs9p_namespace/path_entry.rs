use std::collections::BTreeMap;

use super::{Virtio9pFidPath, Virtio9pNode, Virtio9pNodeId};

pub(super) fn take_file_node_at_fid_path(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    fid_path: &Virtio9pFidPath,
    expected: Virtio9pNodeId,
) -> Option<Virtio9pNode> {
    take_file_node_at_components(entries, fid_path.components(), expected)
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
