use crate::data::{DirectoryInformation, File, FileInformation, GeneralHash, OtherInformation, SaveFileEntryTypeV1, SaveFileEntryV1, SaveFileEntryV1Ref, StubInformation, SymlinkInformation};

impl From<FileInformation> for SaveFileEntryV1 {
    fn from(value: FileInformation) -> Self {
        SaveFileEntryV1 {
            file_type: SaveFileEntryTypeV1::File,
            modified: value.modified,
            hash: value.content_hash,
            path: value.path
        }
    }
}

impl From<SymlinkInformation> for SaveFileEntryV1 {
    fn from(value: SymlinkInformation) -> Self {
        SaveFileEntryV1 {
            file_type: SaveFileEntryTypeV1::Symlink,
            modified: value.modified,
            hash: value.content_hash,
            path: value.path
        }
    }
}

impl From<DirectoryInformation> for SaveFileEntryV1 {
    fn from(value: DirectoryInformation) -> Self {
        SaveFileEntryV1 {
            file_type: SaveFileEntryTypeV1::Directory,
            modified: value.modified,
            hash: value.content_hash,
            path: value.path
        }
    }
}

impl From<OtherInformation> for SaveFileEntryV1 {
    fn from(value: OtherInformation) -> Self {
        SaveFileEntryV1 {
            file_type: SaveFileEntryTypeV1::Other,
            modified: 0,
            hash: GeneralHash::NULL,
            path: value.path
        }
    }
}

impl From<StubInformation> for SaveFileEntryV1 {
    fn from(value: StubInformation) -> Self {
        SaveFileEntryV1 {
            file_type: SaveFileEntryTypeV1::Other,
            modified: 0,
            hash: value.content_hash,
            path: value.path
        }
    }
}

impl<'a> From<&'a FileInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a FileInformation) -> Self {
        SaveFileEntryV1Ref {
            file_type: &SaveFileEntryTypeV1::File,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path
        }
    }
}

impl<'a> From<&'a SymlinkInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a SymlinkInformation) -> Self {
        SaveFileEntryV1Ref {
            file_type: &SaveFileEntryTypeV1::Symlink,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path
        }
    }
}

impl<'a> From<&'a DirectoryInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a DirectoryInformation) -> Self {
        SaveFileEntryV1Ref {
            file_type: &SaveFileEntryTypeV1::Directory,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path
        }
    }
}

impl<'a> From<&'a OtherInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a OtherInformation) -> Self {
        SaveFileEntryV1Ref {
            file_type: &SaveFileEntryTypeV1::Other,
            modified: &0,
            hash: &GeneralHash::NULL,
            path: &value.path
        }
    }
}

impl<'a> From<&'a StubInformation> for SaveFileEntryV1Ref<'a> {
    fn from(value: &'a StubInformation) -> Self {
        SaveFileEntryV1Ref {
            file_type: &SaveFileEntryTypeV1::Other,
            modified: &0,
            hash: &value.content_hash,
            path: &value.path
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
        SaveFileEntryV1Ref {
            file_type: &value.file_type,
            modified: &value.modified,
            hash: &value.hash,
            path: &value.path,
        }
    }
}
