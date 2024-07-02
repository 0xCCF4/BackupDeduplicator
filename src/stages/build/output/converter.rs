use crate::hash::GeneralHash;
use crate::stages::build::intermediary_build_data::{
    BuildArchiveFileInformation, BuildDirectoryInformation, BuildFile, BuildFileInformation,
    BuildOtherInformation, BuildStubInformation, BuildSymlinkInformation,
};
use crate::stages::build::output::{
    HashTreeFileEntry, HashTreeFileEntryRef, HashTreeFileEntryType,
};

impl From<BuildFileInformation> for HashTreeFileEntry {
    /// Convert a [BuildFileInformation] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildFileInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildFileInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::File,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl From<BuildArchiveFileInformation> for HashTreeFileEntry {
    /// Convert a [BuildArchiveFileInformation] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildArchiveFileInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildArchiveFileInformation) -> Self {
        let mut result = Self {
            file_type: HashTreeFileEntryType::File,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(value.children.len()),
        };
        for child in value.children {
            result.archive_children.push(child.into());
        }
        result
    }
}

impl From<BuildSymlinkInformation> for HashTreeFileEntry {
    /// Convert a [BuildSymlinkInformation] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildSymlinkInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildSymlinkInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::Symlink,
            modified: value.modified,
            size: value.content_size,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl From<BuildDirectoryInformation> for HashTreeFileEntry {
    /// Convert a [BuildDirectoryInformation] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildDirectoryInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildDirectoryInformation) -> Self {
        let mut result = Self {
            file_type: HashTreeFileEntryType::Directory,
            modified: value.modified,
            size: value.number_of_children,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(value.children.len()),
            archive_children: Vec::with_capacity(0),
        };
        for child in value.children {
            result.children.push(child.get_content_hash().clone());
        }
        result
    }
}

impl From<BuildOtherInformation> for HashTreeFileEntry {
    /// Convert a [BuildOtherInformation] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildOtherInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildOtherInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::Other,
            modified: value.modified,
            size: value.content_size,
            hash: GeneralHash::NULL,
            path: value.path,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl From<BuildStubInformation> for HashTreeFileEntry {
    /// Convert a [BuildStubInformation] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildStubInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildStubInformation) -> Self {
        Self {
            file_type: HashTreeFileEntryType::Other,
            modified: 0,
            size: 0,
            hash: value.content_hash,
            path: value.path,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a BuildFileInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildFileInformation] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildFileInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildFileInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::File,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a BuildArchiveFileInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildArchiveFileInformation] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildArchiveFileInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildArchiveFileInformation) -> Self {
        let mut result = Self {
            file_type: &HashTreeFileEntryType::File,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(value.children.len()),
        };
        for child in &value.children {
            result.archive_children.push(child.clone().into());
        }
        result
    }
}

impl<'a> From<&'a BuildSymlinkInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildSymlinkInformation] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildSymlinkInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildSymlinkInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::Symlink,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a BuildDirectoryInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildDirectoryInformation] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildDirectoryInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildDirectoryInformation) -> Self {
        let mut result = Self {
            file_type: &HashTreeFileEntryType::Directory,
            modified: &value.modified,
            hash: &value.content_hash,
            path: &value.path,
            size: &value.number_of_children,
            children: Vec::with_capacity(value.children.len()),
            archive_children: Vec::with_capacity(0),
        };
        for child in &value.children {
            result.children.push(child.get_content_hash());
        }
        result
    }
}

impl<'a> From<&'a BuildOtherInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildOtherInformation] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildOtherInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildOtherInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::Other,
            modified: &0,
            hash: &GeneralHash::NULL,
            path: &value.path,
            size: &value.content_size,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl<'a> From<&'a BuildStubInformation> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildStubInformation] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildStubInformation] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildStubInformation) -> Self {
        Self {
            file_type: &HashTreeFileEntryType::Other,
            modified: &0,
            hash: &value.content_hash,
            path: &value.path,
            size: &0,
            children: Vec::with_capacity(0),
            archive_children: Vec::with_capacity(0),
        }
    }
}

impl From<BuildFile> for HashTreeFileEntry {
    /// Convert a [BuildFile] into a [HashTreeFileEntry].
    ///
    /// # Arguments
    /// * `value` - The [BuildFile] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntry].
    fn from(value: BuildFile) -> Self {
        match value {
            BuildFile::File(info) => info.into(),
            BuildFile::ArchiveFile(info) => info.into(),
            BuildFile::Directory(info) => info.into(),
            BuildFile::Symlink(info) => info.into(),
            BuildFile::Other(info) => info.into(),
            BuildFile::Stub(info) => info.into(),
        }
    }
}

impl<'a> From<&'a BuildFile> for HashTreeFileEntryRef<'a> {
    /// Convert a [BuildFile] into a [HashTreeFileEntryRef].
    ///
    /// # Arguments
    /// * `value` - The reference to the [BuildFile] to convert.
    ///
    /// # Returns
    /// The converted [HashTreeFileEntryRef].
    fn from(value: &'a BuildFile) -> Self {
        match value {
            BuildFile::File(info) => info.into(),
            BuildFile::ArchiveFile(info) => info.into(),
            BuildFile::Directory(info) => info.into(),
            BuildFile::Symlink(info) => info.into(),
            BuildFile::Other(info) => info.into(),
            BuildFile::Stub(info) => info.into(),
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
            archive_children: Vec::with_capacity(0),
        }
    }
}
