//! Rename the staged object with POSIX replacement semantics. MoveFileEx refuses
//! an open destination even when its readers share DELETE access. Closing those
//! readers would discard the identity evidence retained by our callers.

use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use tempfile::NamedTempFile;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::*;

// FILE_RENAME_INFO.Flags with FileRenameInfoEx. Do not include the flag that
// ignores read-only targets. Windows SDK documents these bits as
// FILE_RENAME_REPLACE_IF_EXISTS and FILE_RENAME_POSIX_SEMANTICS:
// https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information
const REPLACE_WITH_OPEN_READERS: u32 = 0x00000001 | 0x00000002;

/// An aligned, length-checked native rename request, bound to one destination.
struct RenameRequest {
    storage: Vec<usize>,
    bytes: u32,
}

impl RenameRequest {
    fn new(destination: &Path) -> io::Result<Self> {
        let absolute = std::path::absolute(destination)?;
        let name: Vec<u16> = absolute.as_os_str().encode_wide().collect();
        if name.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NUL in output path",
            ));
        }
        let name_bytes = name.len().checked_mul(size_of::<u16>()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "output path is too long")
        })?;
        let bytes = std::mem::offset_of!(FILE_RENAME_INFO, FileName)
            .checked_add(name_bytes)
            .and_then(|size| size.checked_add(size_of::<u16>()))
            .and_then(|size| u32::try_from(size.max(size_of::<FILE_RENAME_INFO>())).ok())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "output path is too long")
            })?;
        let mut storage = vec![0_usize; (bytes as usize).div_ceil(size_of::<usize>())];
        // SAFETY: usize storage has the alignment of FILE_RENAME_INFO (whose
        // strictest field is HANDLE), and room for the complete header, UTF-16
        // name and trailing NUL. FileNameLength excludes that NUL. Neither the
        // structure pointer nor the variable-length name escapes this allocation.
        unsafe {
            let request = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
            request.write(FILE_RENAME_INFO::default());
            (*request).Anonymous.Flags = REPLACE_WITH_OPEN_READERS;
            (*request).FileNameLength = name_bytes as u32;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                std::ptr::addr_of_mut!((*request).FileName).cast::<u16>(),
                name.len(),
            );
        }
        Ok(Self { storage, bytes })
    }
}

/// Only construction can acquire DELETE access to the exact staged object.
/// The owned temporary name remains guarded on preparation/publication failure.
pub(super) struct PreparedRename {
    _staged: NamedTempFile,
    handle: File,
    request: RenameRequest,
}

impl PreparedRename {
    pub(super) fn prepare(staged: NamedTempFile, destination: &Path) -> io::Result<Self> {
        let request = RenameRequest::new(destination)?;
        // tempfile marks fresh stages TEMPORARY. Clear only that staging hint;
        // unlike tempfile::persist, do not reset copied attributes to NORMAL.
        let attributes = staged.as_file().metadata()?.file_attributes();
        if attributes & FILE_ATTRIBUTE_TEMPORARY != 0 {
            let retained = attributes & !FILE_ATTRIBUTE_TEMPORARY;
            let basic = FILE_BASIC_INFO {
                FileAttributes: if retained == 0 {
                    FILE_ATTRIBUTE_NORMAL
                } else {
                    retained
                },
                ..FILE_BASIC_INFO::default()
            };
            // SAFETY: live writable stage and exact native structure. Zero time
            // fields mean leave those timestamps unchanged, not reset to epoch.
            let result = unsafe {
                SetFileInformationByHandle(
                    staged.as_file().as_raw_handle(),
                    FileBasicInfo,
                    (&basic as *const FILE_BASIC_INFO).cast(),
                    size_of::<FILE_BASIC_INFO>() as u32,
                )
            };
            if result == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        // SAFETY: live source handle; ReOpenFile binds to that same object rather
        // than resolving its temporary pathname again. No attributes are changed.
        let handle = unsafe {
            ReOpenFile(
                staged.as_file().as_raw_handle(),
                DELETE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                0,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful ReOpenFile returns a new owned handle.
        let handle = unsafe { File::from_raw_handle(handle) };
        Ok(Self {
            _staged: staged,
            handle,
            request,
        })
    }

    pub(super) fn publish(self) -> io::Result<()> {
        // SAFETY: owned DELETE-capable handle and aligned, fully initialized
        // native request retained throughout this synchronous call. There is no
        // delete/copy fallback if the filesystem does not support this operation.
        let result = unsafe {
            SetFileInformationByHandle(
                self.handle.as_raw_handle(),
                FileRenameInfoEx,
                self.request.storage.as_ptr().cast(),
                self.request.bytes,
            )
        };
        if result == 0 {
            Err(io::Error::last_os_error())
        } else {
            // TempPath cleanup concerns only the now-absent temporary name.
            Ok(())
        }
    }
}
