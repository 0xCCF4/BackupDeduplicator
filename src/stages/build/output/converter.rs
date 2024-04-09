use crate::file::{DirectoryInformation, File, FileInformation, OtherInformation, StubInformation, SymlinkInformation};
use crate::hash::GeneralHash;
use crate::stages::build::output::{HashTreeFileEntryType, HashTreeFileEntry, HashTreeFileEntryRef};

impl From<FileInformation> for HashTreeFileEntry {
    /// Convert a [FileInformation] into a [HashTreeFileEntry].
    /// 
    /// # Arguments
    /// * `value` - The [FileInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: FileInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::File,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<SymlinkInformation> for HashTreeFileEntry {
    /// Convert a [SymlinkInformation] into a [HashTreeFileEntry].
    /// 
    /// # Arguments
    /// * `value` - The [SymlinkInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: SymlinkInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::Symlink,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<DirectoryInformation> for HashTreeFileEntry {
    /// Convert a [DirectoryInformation] into a [HashTreeFileEntry].
    /// 
    /// # Arguments
    /// * `value` - The [DirectoryInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: DirectoryInformation) -> Self {
        let mut result = Self {
            file_type: HashTreeFileEntryType::Directory,
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

impl From<OtherInformation> for HashTreeFileEntry {
    /// Convert a [OtherInformation] into a [HashTreeFileEntry].
    /// 
    /// # Arguments
    /// * `value` - The [OtherInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: OtherInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::Other,
            modified: value.modified,
            size: value.content_size,
            hash: GeneralHash::NULL,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<StubInformation> for HashTreeFileEntry {
    /// Convert a [StubInformation] into a [HashTreeFileEntry].
    /// 
    /// # Arguments
    /// * `value` - The [StubInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: StubInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::Other,
            modified: 0,
            size: 0,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a FileInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [FileInformation] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [FileInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a FileInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::File,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a SymlinkInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [SymlinkInformation] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [SymlinkInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a SymlinkInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::Symlink,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a DirectoryInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [DirectoryInformation] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [DirectoryInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a DirectoryInformation) -> Self {
        let mut result = Self {
            file_type: &HashTreeFileEntryType::Directory,
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

impl<'a> From<&'a OtherInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [OtherInformation] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [OtherInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a OtherInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::Other,
            modified: &0,
            hash: &GeneralHash::NULL,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a StubInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [StubInformation] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [StubInformation] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a StubInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::Other,
            modified: &0,
            hash: &value.content_hash,
            path: &value.path,
            size: &0,
            children: Vec::with_capacity(0),
        }
    }
}

impl From<File> for HashTreeFileEntry {
    /// Convert a [File] into a [HashTreeFileEntry].
    /// 
    /// # Arguments
    /// * `value` - The [File] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntry].
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

impl<'a> From<&'a File> for HashTreeFileEntryRef<'a> {
    /// Convert a [File] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [File] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
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

impl<'a> From<&'a HashTreeFileEntry> for HashTreeFileEntryRef<'a> {
    /// Convert a [HashTreeFileEntry] into a [HashTreeFileEntryRef].
    /// 
    /// # Arguments
    /// * `value` - The reference to the [HashTreeFileEntry] to convert.
    /// 
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a HashTreeFileEntry) -> Self {
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
