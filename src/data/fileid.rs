
// device id

use std::io;
use std::path::Path;
use file_id::FileId;
use serde::Serialize;

#[cfg(target_family = "unix")]
type DeviceIdType = u64;

#[cfg(target_family = "windows")]
type DeviceIdType = u64; // high-res file-id

// file id

#[cfg(target_family = "unix")]
type FileIdType = u64;

#[cfg(target_family = "windows")]
type FileIdType = u128; // high-res file-id

// structs

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HandleIdentifier {
    pub inode: FileIdType,
    pub drive: DeviceIdType,
}

pub fn from_path(path: impl AsRef<Path>) -> io::Result<HandleIdentifier> {
    match file_id::get_file_id(path)? {
        FileId::Inode { device_id, inode_number } => Ok(HandleIdentifier {
            inode: inode_number as FileIdType,
            drive: device_id as DeviceIdType,
        }),
        FileId::LowRes { volume_serial_number, file_index } => Ok(HandleIdentifier {
            inode: file_index as FileIdType,
            drive: volume_serial_number as DeviceIdType,
        }),
        FileId::HighRes { volume_serial_number, file_id } => Ok(HandleIdentifier {
            inode: file_id as FileIdType, // path windows only -> no downcast will happen
            drive: volume_serial_number as DeviceIdType,
        }),
    }
}
