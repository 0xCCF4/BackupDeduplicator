// device id

use file_id::FileId;
use serde::Serialize;
use std::io;
use std::path::Path;

/// Device id type.
#[cfg(target_family = "unix")]
type DeviceIdType = u64;

/// Device id type.
#[cfg(target_family = "windows")]
type DeviceIdType = u64; // high-res file-id

/// File id type
#[cfg(target_family = "unix")]
type FileIdType = u64;

/// File id type
#[cfg(target_family = "windows")]
type FileIdType = u128; // high-res file-id

/// A file id handle.
///
/// # Fields
/// * `inode` - The inode of the file.
/// * `drive` - The device id of the file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HandleIdentifier {
    /// Value identifying the file.
    pub inode: FileIdType,
    /// Value identifying the device.
    pub drive: DeviceIdType,
}

impl HandleIdentifier {
    /// Create a new handle identifier from a path.
    ///
    /// # Arguments
    /// * `path` - The path to the file.
    ///
    /// # Returns
    /// The handle identifier.
    ///
    /// # Errors
    /// If the file id cannot be retrieved.
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<HandleIdentifier> {
        match file_id::get_file_id(path)? {
            FileId::Inode {
                device_id,
                inode_number,
            } => Ok(HandleIdentifier {
                // unix
                inode: inode_number as FileIdType,
                drive: device_id as DeviceIdType,
            }),
            FileId::LowRes {
                volume_serial_number,
                file_index,
            } => Ok(HandleIdentifier {
                // windows
                inode: file_index as FileIdType,
                drive: volume_serial_number as DeviceIdType,
            }),
            FileId::HighRes {
                volume_serial_number,
                file_id,
            } => Ok(HandleIdentifier {
                // windows
                inode: file_id as FileIdType,
                drive: volume_serial_number as DeviceIdType,
            }),
        }
    }
}
