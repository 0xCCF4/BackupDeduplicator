use crate::file::{DirectoryInformation, File, FileInformation, OtherInformation, StubInformation, SymlinkInformation};
use crate::hash::GeneralHash;
use crate::stages::build::output::{SaveFileEntryTypeV1, SaveFileEntryV1, SaveFileEntryV1Ref};

impl From<FileInformation> for SaveFileEntryV1 {
    fn from(value: FileInformation) -> Self {
        Self {
            file_type: SaveFileEntryTypeV1::File,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<SymlinkInformation> for SaveFileEntryV1 {
    fn from(value: SymlinkInformation) -> Self {
        Self {
            file_type: SaveFileEntryTypeV1::Symlink,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<DirectoryInformation> for SaveFileEntryV1 {
    fn from(value: DirectoryInformation) -> Self {
        let mut result = Self {
            file_type: SaveFileEntryTypeV1::Directory,
            modified: value.modified,
            size: value.number_of_children,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(value.children.len()),
        };
        for child in value.children {
            result.children.push(child.get_content_hash().clone());
        }
        result
    }
}

impl From<OtherInformation> for SaveFileEntryV1 {
    fn from(value: OtherInformation) -> Self {
        Self {
            file_type: SaveFileEntryTypeV1::Other,
            modified: value.modified,
            size: value.content_size,
            hash: GeneralHash::NULL,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<StubInformation> for SaveFileEntryV1 {
    fn from(value: StubInformation) -> Self {
        Self {
            file_type: SaveFileEntryTypeV1::Other,
            modified: 0,
            size: 0,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a FileInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a FileInformation) -> Self {
        Self {
            file_type: &SaveFileEntryTypeV1::File,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a SymlinkInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a SymlinkInformation) -> Self {
        Self {
            file_type: &SaveFileEntryTypeV1::Symlink,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a DirectoryInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a DirectoryInformation) -> Self {
        let mut result = Self {
            file_type: &SaveFileEntryTypeV1::Directory,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.number_of_children,
            children: Vec::with_capacity(value.children.len()),
        };
        for child in &value.children {
            result.children.push(child.get_content_hash());
        }
        result
    }
}

impl<'a> From<&'a OtherInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a OtherInformation) -> Self {
        Self {
            file_type: &SaveFileEntryTypeV1::Other,
            modified: &0,
            hash: &GeneralHash::NULL,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a StubInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a StubInformation) -> Self {
        Self {
            file_type: &SaveFileEntryTypeV1::Other,
            modified: &0,
            hash: &value.content_hash,
            path: &value.path,
            size: &0,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<File> for SaveFileEntryV1 {
    fn from(value: File) -> Self {
        match value {
            File::File(info) => info.into(),
            File::Directory(info) => info.into(),
            File::Symlink(info) => info.into(),
            File::Other(info) => info.into(),
            File::Stub(info) => info.into(),
        }
    }
}

impl<'a> From<&'a File> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a File) -> Self {
        match value {
            File::File(info) => info.into(),
            File::Directory(info) => info.into(),
            File::Symlink(info) => info.into(),
            File::Other(info) => info.into(),
            File::Stub(info) => info.into(),
        }
    }
}

impl<'a> From<&'a SaveFileEntryV1> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a SaveFileEntryV1) -> Self {
        Self {
            file_type: &value.file_type,
            modified: &value.modified,
            hash: &value.hash,
            path: &value.path,
            size: &value.size,
            children: Vec::with_capacity(0),
        }
    }
}
