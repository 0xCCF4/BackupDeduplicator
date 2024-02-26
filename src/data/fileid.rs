
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
    pub inode: DeviceIdType,
    pub drive: FileIdType,
}

pub fn from_path(path: impl AsRef<Path>) -> io::Result<HandleIdentifier> {
    match file_id::get_file_id(path)? {
        FileId::Inode { device_id, inode_number } => Ok(HandleIdentifier {
            inode: inode_number,
            drive: device_id,
        }),
        FileId::LowRes { volume_serial_number, file_index } => Ok(HandleIdentifier {
            inode: file_index,
            drive: volume_serial_number as DeviceIdType,
        }),
        FileId::HighRes { volume_serial_number, file_id } => Ok(HandleIdentifier {
            inode: file_id as FileIdType, // path windows only -> no downcast will happen
            drive: volume_serial_number as DeviceIdType,
        }),
    }
}
