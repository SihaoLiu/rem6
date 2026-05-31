use std::collections::BTreeMap;

use crate::{
    fs9p::{
        VIRTIO_9P_DTDIR, VIRTIO_9P_DTREG, VIRTIO_9P_DTSYMLINK, VIRTIO_9P_EBADF, VIRTIO_9P_EEXIST,
        VIRTIO_9P_ENOENT, VIRTIO_9P_GETATTR_BASIC, VIRTIO_9P_NAME_MAX, VIRTIO_9P_QTDIR,
        VIRTIO_9P_QTFILE, VIRTIO_9P_QTSYMLINK, VIRTIO_9P_STATFS_BLOCK_SIZE, VIRTIO_9P_STATFS_TYPE,
        VIRTIO_9P_TLCREATE, VIRTIO_9P_TMKDIR, VIRTIO_9P_TRENAMEAT, VIRTIO_9P_TSYMLINK,
        VIRTIO_9P_TUNLINKAT, VIRTIO_9P_TWALK,
    },
    VirtioError,
};

const VIRTIO_9P_QID_BYTES: usize = 13;
const VIRTIO_9P_STATFS_BLOCKS: u64 = 1024;
const VIRTIO_9P_STATFS_FSID: u64 = 0x7265_6d36;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pQid {
    qtype: u8,
    version: u32,
    path: u64,
}

impl Virtio9pQid {
    const fn new(qtype: u8, path: u64) -> Self {
        Self {
            qtype,
            version: 0,
            path,
        }
    }

    pub(crate) fn to_le_bytes(self) -> [u8; VIRTIO_9P_QID_BYTES] {
        let version = self.version.to_le_bytes();
        let path = self.path.to_le_bytes();
        [
            self.qtype, version[0], version[1], version[2], version[3], path[0], path[1], path[2],
            path[3], path[4], path[5], path[6], path[7],
        ]
    }
}

pub(crate) fn qid_payload(qid: Virtio9pQid) -> Vec<u8> {
    qid.to_le_bytes().to_vec()
}

pub(crate) fn getattr_payload(metadata: Virtio9pNodeMetadata, request_mask: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(153);
    payload.extend((request_mask & VIRTIO_9P_GETATTR_BASIC).to_le_bytes());
    payload.extend(metadata.qid.to_le_bytes());
    payload.extend(metadata.mode.to_le_bytes());
    payload.extend(0_u32.to_le_bytes());
    payload.extend(0_u32.to_le_bytes());
    payload.extend(metadata.nlink.to_le_bytes());
    payload.extend(0_u64.to_le_bytes());
    payload.extend(metadata.size.to_le_bytes());
    payload.extend(u64::from(VIRTIO_9P_STATFS_BLOCK_SIZE).to_le_bytes());
    payload.extend(metadata.blocks.to_le_bytes());
    for _ in 0..10 {
        payload.extend(0_u64.to_le_bytes());
    }
    payload
}

fn counted_payload(data: Vec<u8>) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + data.len());
    payload.extend((data.len() as u32).to_le_bytes());
    payload.extend(data);
    payload
}

fn readdir_entry_bytes(qid: Virtio9pQid, next_offset: u64, dtype: u8, name: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + name.len());
    bytes.extend(qid.to_le_bytes());
    bytes.extend(next_offset.to_le_bytes());
    bytes.push(dtype);
    bytes.extend((name.len() as u16).to_le_bytes());
    bytes.extend(name.as_bytes());
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Virtio9pNodeId {
    Root,
    File(u64),
    Directory(u64),
    Symlink(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pNamespace {
    entries: BTreeMap<String, Virtio9pNode>,
    next_path: u64,
}

impl Virtio9pNamespace {
    pub(crate) fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_path: 2,
        }
    }

    pub(crate) const fn root_qid(&self) -> Virtio9pQid {
        Virtio9pQid::new(VIRTIO_9P_QTDIR, 1)
    }

    pub(crate) fn insert_file(&mut self, name: String, data: Vec<u8>) -> Result<(), VirtioError> {
        validate_file_name(VIRTIO_9P_TWALK, &name)?;
        let path = self.allocate_path()?;
        self.entries.insert(
            name,
            Virtio9pNode::File(Virtio9pFileNode {
                qid_path: path,
                data,
            }),
        );
        Ok(())
    }

    pub(crate) fn create_file(
        &mut self,
        parent: Virtio9pNodeId,
        name: String,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TLCREATE, &name)?;
        let Some(entries) = self.directory_entries(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if matches!(entries.get(&name), Some(Virtio9pNode::Directory(_))) {
            return Ok(Err(VIRTIO_9P_EEXIST));
        }
        let path = self.allocate_path()?;
        let entries = self
            .directory_entries_mut(parent)
            .expect("prevalidated 9p directory parent");
        entries.insert(
            name,
            Virtio9pNode::File(Virtio9pFileNode {
                qid_path: path,
                data: Vec::new(),
            }),
        );
        Ok(Ok(Virtio9pNodeId::File(path)))
    }

    pub(crate) fn mkdir(
        &mut self,
        parent: Virtio9pNodeId,
        name: String,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TMKDIR, &name)?;
        let Some(entries) = self.directory_entries(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if entries.contains_key(&name) {
            return Ok(Err(VIRTIO_9P_EEXIST));
        }
        let path = self.allocate_path()?;
        let entries = self
            .directory_entries_mut(parent)
            .expect("prevalidated 9p directory parent");
        entries.insert(
            name,
            Virtio9pNode::Directory(Virtio9pDirectoryNode {
                qid_path: path,
                entries: BTreeMap::new(),
            }),
        );
        Ok(Ok(Virtio9pNodeId::Directory(path)))
    }

    pub(crate) fn create_symlink(
        &mut self,
        parent: Virtio9pNodeId,
        name: String,
        target: String,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TSYMLINK, &name)?;
        let Some(entries) = self.directory_entries(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if entries.contains_key(&name) {
            return Ok(Err(VIRTIO_9P_EEXIST));
        }
        let path = self.allocate_path()?;
        let entries = self
            .directory_entries_mut(parent)
            .expect("prevalidated 9p directory parent");
        entries.insert(
            name,
            Virtio9pNode::Symlink(Virtio9pSymlinkNode {
                qid_path: path,
                target,
            }),
        );
        Ok(Ok(Virtio9pNodeId::Symlink(path)))
    }

    pub(crate) fn remove_file_by_name(
        &mut self,
        parent: Virtio9pNodeId,
        name: &str,
    ) -> Result<Result<Virtio9pNodeId, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TUNLINKAT, name)?;
        let Some(entries) = self.directory_entries_mut(parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        match entries.get(name).map(Virtio9pNode::id) {
            Some(node @ (Virtio9pNodeId::File(_) | Virtio9pNodeId::Symlink(_))) => {
                entries.remove(name);
                Ok(Ok(node))
            }
            Some(Virtio9pNodeId::Directory(_)) | Some(Virtio9pNodeId::Root) => {
                Ok(Err(VIRTIO_9P_EBADF))
            }
            None => Ok(Err(VIRTIO_9P_ENOENT)),
        }
    }

    pub(crate) fn remove_file_by_node(&mut self, node: Virtio9pNodeId) -> bool {
        match node {
            Virtio9pNodeId::File(path) | Virtio9pNodeId::Symlink(path) => {
                remove_file_by_path(&mut self.entries, path)
            }
            Virtio9pNodeId::Root | Virtio9pNodeId::Directory(_) => false,
        }
    }

    pub(crate) fn rename_file(
        &mut self,
        old_parent: Virtio9pNodeId,
        oldname: &str,
        new_parent: Virtio9pNodeId,
        newname: &str,
    ) -> Result<Result<Virtio9pRenameOutcome, u32>, VirtioError> {
        validate_file_name(VIRTIO_9P_TRENAMEAT, oldname)?;
        validate_file_name(VIRTIO_9P_TRENAMEAT, newname)?;
        if old_parent != new_parent {
            return Ok(Err(VIRTIO_9P_EBADF));
        }
        let Some(entries) = self.directory_entries_mut(old_parent) else {
            return Ok(Err(VIRTIO_9P_EBADF));
        };
        if oldname == newname {
            return match entries.get(oldname) {
                Some(Virtio9pNode::File(_) | Virtio9pNode::Symlink(_)) => {
                    Ok(Ok(Virtio9pRenameOutcome { replaced: None }))
                }
                Some(Virtio9pNode::Directory(_)) => Ok(Err(VIRTIO_9P_EBADF)),
                None => Ok(Err(VIRTIO_9P_ENOENT)),
            };
        }
        match entries.get(newname) {
            Some(Virtio9pNode::Directory(_)) => return Ok(Err(VIRTIO_9P_EEXIST)),
            Some(Virtio9pNode::File(_) | Virtio9pNode::Symlink(_)) | None => {}
        }
        let Some(node @ (Virtio9pNode::File(_) | Virtio9pNode::Symlink(_))) =
            entries.remove(oldname)
        else {
            return Ok(Err(VIRTIO_9P_ENOENT));
        };
        let replaced =
            entries
                .insert(newname.to_string(), node)
                .and_then(|replaced| match replaced {
                    Virtio9pNode::File(file) => Some(Virtio9pNodeId::File(file.qid_path)),
                    Virtio9pNode::Symlink(symlink) => {
                        Some(Virtio9pNodeId::Symlink(symlink.qid_path))
                    }
                    Virtio9pNode::Directory(_) => None,
                });
        Ok(Ok(Virtio9pRenameOutcome { replaced }))
    }

    pub(crate) fn walk(&self, node: Virtio9pNodeId, name: &str) -> Option<Virtio9pNodeId> {
        self.directory_entries(node)?
            .get(name)
            .map(Virtio9pNode::id)
    }

    pub(crate) fn qid(&self, node: Virtio9pNodeId) -> Virtio9pQid {
        match node {
            Virtio9pNodeId::Root => self.root_qid(),
            Virtio9pNodeId::File(path) => Virtio9pQid::new(VIRTIO_9P_QTFILE, path),
            Virtio9pNodeId::Directory(path) => Virtio9pQid::new(VIRTIO_9P_QTDIR, path),
            Virtio9pNodeId::Symlink(path) => Virtio9pQid::new(VIRTIO_9P_QTSYMLINK, path),
        }
    }

    pub(crate) fn metadata(&self, node: Virtio9pNodeId) -> Option<Virtio9pNodeMetadata> {
        match node {
            Virtio9pNodeId::Root => Some(Virtio9pNodeMetadata {
                qid: self.root_qid(),
                mode: 0o040755,
                nlink: 2 + self.entries.len() as u64,
                size: 0,
                blocks: 0,
            }),
            Virtio9pNodeId::File(path) => {
                let file = find_file(&self.entries, path)?;
                let size = file.data.len() as u64;
                Some(Virtio9pNodeMetadata {
                    qid: Virtio9pQid::new(VIRTIO_9P_QTFILE, path),
                    mode: 0o100644,
                    nlink: 1,
                    size,
                    blocks: size.div_ceil(512),
                })
            }
            Virtio9pNodeId::Directory(path) => {
                let entries = self.directory_entries(node)?;
                Some(Virtio9pNodeMetadata {
                    qid: Virtio9pQid::new(VIRTIO_9P_QTDIR, path),
                    mode: 0o040755,
                    nlink: 2 + entries.len() as u64,
                    size: 0,
                    blocks: 0,
                })
            }
            Virtio9pNodeId::Symlink(path) => {
                let symlink = find_symlink(&self.entries, path)?;
                let size = symlink.target.len() as u64;
                Some(Virtio9pNodeMetadata {
                    qid: Virtio9pQid::new(VIRTIO_9P_QTSYMLINK, path),
                    mode: 0o120777,
                    nlink: 1,
                    size,
                    blocks: size.div_ceil(512),
                })
            }
        }
    }

    pub(crate) fn statfs_payload(&self) -> Vec<u8> {
        let files = 1 + count_nodes(&self.entries);
        let mut payload = Vec::with_capacity(60);
        payload.extend(VIRTIO_9P_STATFS_TYPE.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCK_SIZE.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.to_le_bytes());
        payload.extend(files.to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_BLOCKS.saturating_sub(files).to_le_bytes());
        payload.extend(VIRTIO_9P_STATFS_FSID.to_le_bytes());
        payload.extend(VIRTIO_9P_NAME_MAX.to_le_bytes());
        payload
    }

    pub(crate) fn readdir_payload(
        &self,
        node: Virtio9pNodeId,
        offset: u64,
        count: u32,
    ) -> Option<Vec<u8>> {
        let children = self.directory_entries(node)?;
        let start = usize::try_from(offset).ok()?;
        let budget = usize::try_from(count).ok()?;
        let mut entries = Vec::with_capacity(2 + children.len());
        let mut next_offset = 0_u64;

        for (qid, dtype, name) in [
            (self.qid(node), VIRTIO_9P_DTDIR, "."),
            (self.root_qid(), VIRTIO_9P_DTDIR, ".."),
        ] {
            let entry_len = 24 + name.len();
            next_offset = next_offset.checked_add(entry_len as u64)?;
            entries.push(readdir_entry_bytes(qid, next_offset, dtype, name));
        }

        for (name, child) in children {
            let entry_len = 24 + name.len();
            next_offset = next_offset.checked_add(entry_len as u64)?;
            entries.push(readdir_entry_bytes(
                child.qid(),
                next_offset,
                child.dtype(),
                name,
            ));
        }

        let mut full_offset = 0_usize;
        let mut data = Vec::new();
        for entry in entries {
            let entry_start = full_offset;
            let entry_end = entry_start.checked_add(entry.len())?;
            full_offset = entry_end;
            if entry_start < start {
                continue;
            }
            if data.len().checked_add(entry.len())? > budget {
                break;
            }
            data.extend(entry);
        }
        Some(counted_payload(data))
    }

    pub(crate) fn read_file(
        &self,
        node: Virtio9pNodeId,
        offset: u64,
        count: u32,
    ) -> Option<Vec<u8>> {
        let Virtio9pNodeId::File(path) = node else {
            return None;
        };
        let file = find_file(&self.entries, path)?;
        let start = usize::try_from(offset).ok()?;
        if start >= file.data.len() {
            return Some(Vec::new());
        }
        let count = usize::try_from(count).ok()?;
        let end = start.saturating_add(count).min(file.data.len());
        Some(file.data[start..end].to_vec())
    }

    pub(crate) fn readlink(&self, node: Virtio9pNodeId) -> Option<&str> {
        let Virtio9pNodeId::Symlink(path) = node else {
            return None;
        };
        find_symlink(&self.entries, path).map(|symlink| symlink.target.as_str())
    }

    pub(crate) fn write_file(
        &mut self,
        node: Virtio9pNodeId,
        offset: u64,
        data: &[u8],
    ) -> Option<u32> {
        let Virtio9pNodeId::File(path) = node else {
            return None;
        };
        let file = find_file_mut(&mut self.entries, path)?;
        let start = usize::try_from(offset).ok()?;
        let end = start.checked_add(data.len())?;
        if file.data.len() < end {
            file.data.resize(end, 0);
        }
        file.data[start..end].copy_from_slice(data);
        u32::try_from(data.len()).ok()
    }

    fn directory_entries(&self, node: Virtio9pNodeId) -> Option<&BTreeMap<String, Virtio9pNode>> {
        match node {
            Virtio9pNodeId::Root => Some(&self.entries),
            Virtio9pNodeId::Directory(path) => {
                find_directory(&self.entries, path).map(|directory| &directory.entries)
            }
            Virtio9pNodeId::File(_) | Virtio9pNodeId::Symlink(_) => None,
        }
    }

    fn directory_entries_mut(
        &mut self,
        node: Virtio9pNodeId,
    ) -> Option<&mut BTreeMap<String, Virtio9pNode>> {
        match node {
            Virtio9pNodeId::Root => Some(&mut self.entries),
            Virtio9pNodeId::Directory(path) => {
                find_directory_mut(&mut self.entries, path).map(|directory| &mut directory.entries)
            }
            Virtio9pNodeId::File(_) | Virtio9pNodeId::Symlink(_) => None,
        }
    }

    fn allocate_path(&mut self) -> Result<u64, VirtioError> {
        let path = self.next_path;
        self.next_path = self
            .next_path
            .checked_add(1)
            .ok_or(VirtioError::Virtio9pPayloadLengthOverflow)?;
        Ok(path)
    }
}

fn count_nodes(entries: &BTreeMap<String, Virtio9pNode>) -> u64 {
    entries
        .values()
        .map(|node| match node {
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) => 1,
            Virtio9pNode::Directory(directory) => 1 + count_nodes(&directory.entries),
        })
        .sum()
}

fn find_file(entries: &BTreeMap<String, Virtio9pNode>, path: u64) -> Option<&Virtio9pFileNode> {
    for node in entries.values() {
        match node {
            Virtio9pNode::File(file) if file.qid_path == path => return Some(file),
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) => {}
            Virtio9pNode::Directory(directory) => {
                if let Some(file) = find_file(&directory.entries, path) {
                    return Some(file);
                }
            }
        }
    }
    None
}

fn find_file_mut(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&mut Virtio9pFileNode> {
    for node in entries.values_mut() {
        match node {
            Virtio9pNode::File(file) if file.qid_path == path => return Some(file),
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) => {}
            Virtio9pNode::Directory(directory) => {
                if let Some(file) = find_file_mut(&mut directory.entries, path) {
                    return Some(file);
                }
            }
        }
    }
    None
}

fn find_symlink(
    entries: &BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&Virtio9pSymlinkNode> {
    for node in entries.values() {
        match node {
            Virtio9pNode::Symlink(symlink) if symlink.qid_path == path => return Some(symlink),
            Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) => {}
            Virtio9pNode::Directory(directory) => {
                if let Some(symlink) = find_symlink(&directory.entries, path) {
                    return Some(symlink);
                }
            }
        }
    }
    None
}

fn find_directory(
    entries: &BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&Virtio9pDirectoryNode> {
    for node in entries.values() {
        let Virtio9pNode::Directory(directory) = node else {
            continue;
        };
        if directory.qid_path == path {
            return Some(directory);
        }
        if let Some(directory) = find_directory(&directory.entries, path) {
            return Some(directory);
        }
    }
    None
}

fn find_directory_mut(
    entries: &mut BTreeMap<String, Virtio9pNode>,
    path: u64,
) -> Option<&mut Virtio9pDirectoryNode> {
    for node in entries.values_mut() {
        let Virtio9pNode::Directory(directory) = node else {
            continue;
        };
        if directory.qid_path == path {
            return Some(directory);
        }
        if let Some(directory) = find_directory_mut(&mut directory.entries, path) {
            return Some(directory);
        }
    }
    None
}

fn remove_file_by_path(entries: &mut BTreeMap<String, Virtio9pNode>, path: u64) -> bool {
    if let Some(name) = entries.iter().find_map(|(name, node)| match node {
        Virtio9pNode::File(file) if file.qid_path == path => Some(name.clone()),
        Virtio9pNode::Symlink(symlink) if symlink.qid_path == path => Some(name.clone()),
        Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) | Virtio9pNode::Directory(_) => None,
    }) {
        return entries.remove(&name).is_some();
    }
    entries.values_mut().any(|node| match node {
        Virtio9pNode::Directory(directory) => remove_file_by_path(&mut directory.entries, path),
        Virtio9pNode::File(_) | Virtio9pNode::Symlink(_) => false,
    })
}

fn validate_file_name(message_type: u8, name: &str) -> Result<(), VirtioError> {
    if name.is_empty() || name.contains('/') {
        return Err(VirtioError::InvalidVirtio9pPayload {
            message_type,
            bytes: name.len(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pFileNode {
    qid_path: u64,
    data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pDirectoryNode {
    qid_path: u64,
    entries: BTreeMap<String, Virtio9pNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Virtio9pSymlinkNode {
    qid_path: u64,
    target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Virtio9pNode {
    File(Virtio9pFileNode),
    Directory(Virtio9pDirectoryNode),
    Symlink(Virtio9pSymlinkNode),
}

impl Virtio9pNode {
    const fn id(&self) -> Virtio9pNodeId {
        match self {
            Self::File(file) => Virtio9pNodeId::File(file.qid_path),
            Self::Directory(directory) => Virtio9pNodeId::Directory(directory.qid_path),
            Self::Symlink(symlink) => Virtio9pNodeId::Symlink(symlink.qid_path),
        }
    }

    const fn qid(&self) -> Virtio9pQid {
        match self {
            Self::File(file) => Virtio9pQid::new(VIRTIO_9P_QTFILE, file.qid_path),
            Self::Directory(directory) => Virtio9pQid::new(VIRTIO_9P_QTDIR, directory.qid_path),
            Self::Symlink(symlink) => Virtio9pQid::new(VIRTIO_9P_QTSYMLINK, symlink.qid_path),
        }
    }

    const fn dtype(&self) -> u8 {
        match self {
            Self::File(_) => VIRTIO_9P_DTREG,
            Self::Directory(_) => VIRTIO_9P_DTDIR,
            Self::Symlink(_) => VIRTIO_9P_DTSYMLINK,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pRenameOutcome {
    pub(crate) replaced: Option<Virtio9pNodeId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pNodeMetadata {
    qid: Virtio9pQid,
    mode: u32,
    nlink: u64,
    size: u64,
    blocks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Virtio9pFidState {
    node: Virtio9pNodeId,
    open: bool,
}

impl Virtio9pFidState {
    pub(crate) const fn new(node: Virtio9pNodeId) -> Self {
        Self { node, open: false }
    }

    pub(crate) const fn node(self) -> Virtio9pNodeId {
        self.node
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
    }

    pub(crate) const fn opened(node: Virtio9pNodeId) -> Self {
        Self { node, open: true }
    }

    pub(crate) const fn is_open(self) -> bool {
        self.open
    }
}
