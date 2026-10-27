//! Windows stages an OS copy before editing, preserving streams and security.
//! Promotion uses a same-volume rename, never ReplaceFile's multi-step merge.

use super::{PublicationError, Result};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;

const SECURITY: u32 =
    OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;

fn wide(path: &Path) -> Result<Vec<u16>> {
    let mut path: Vec<_> = path.as_os_str().encode_wide().collect();
    if path.contains(&0) {
        return Err(PublicationError::OutputPath);
    }
    path.push(0);
    Ok(path)
}

fn checked(result: i32) -> Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

fn single_link(file: &File) -> Result<()> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: live handle and initialized, writable native structure.
    checked(unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) })?;
    if information.nNumberOfLinks != 1 {
        return Err(PublicationError::MultipleLinks);
    }
    Ok(())
}

/// DWORD-aligned self-relative security descriptor returned by the kernel.
#[derive(PartialEq, Eq)]
struct Security(Vec<u32>);

impl Security {
    fn read(file: &File) -> Result<Self> {
        let mut bytes = 0;
        // SAFETY: live handle and sizing-only null buffer.
        unsafe {
            GetKernelObjectSecurity(
                file.as_raw_handle(),
                SECURITY,
                std::ptr::null_mut(),
                0,
                &mut bytes,
            );
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) || bytes == 0 {
            return Err(error.into());
        }
        let mut descriptor = vec![0_u32; (bytes as usize).div_ceil(4)];
        // SAFETY: aligned writable storage of at least bytes bytes, live handle.
        checked(unsafe {
            GetKernelObjectSecurity(
                file.as_raw_handle(),
                SECURITY,
                descriptor.as_mut_ptr().cast(),
                bytes,
                &mut bytes,
            )
        })?;
        Ok(Self(descriptor))
    }

    fn copy_to(&self, path: &Path) -> Result<()> {
        let mut control = 0;
        let mut revision = 0;
        let descriptor = self.0.as_ptr().cast_mut().cast();
        // SAFETY: the retained kernel-produced self-relative descriptor is valid.
        checked(unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) })?;
        let inheritance = if control & SE_DACL_PROTECTED != 0 {
            PROTECTED_DACL_SECURITY_INFORMATION
        } else {
            UNPROTECTED_DACL_SECURITY_INFORMATION
        };
        // SAFETY: live NUL-terminated pathname and descriptor for a synchronous call.
        checked(unsafe {
            SetFileSecurityW(wide(path)?.as_ptr(), SECURITY | inheritance, descriptor)
        })
    }
}

pub(super) struct ExistingFile {
    path: PathBuf,
    file: File,
    admitted: Metadata,
    security: Security,
}

impl ExistingFile {
    pub(super) fn admit(path: PathBuf, admitted: Metadata) -> Result<Self> {
        // Refuse reparse, sparse, compressed, encrypted, offline and other
        // specialist states. Ordinary hidden/system/archive attributes survive.
        let ordinary = FILE_ATTRIBUTE_NORMAL
            | FILE_ATTRIBUTE_ARCHIVE
            | FILE_ATTRIBUTE_HIDDEN
            | FILE_ATTRIBUTE_SYSTEM
            | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;
        if admitted.file_attributes() & !ordinary != 0 {
            return Err(PublicationError::FileFlags);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)?;
        single_link(&file)?;
        if !same_snapshot(&admitted, &file.metadata()?) {
            return Err(PublicationError::Changed);
        }
        let security = Security::read(&file)?;
        Ok(Self {
            path,
            file,
            admitted,
            security,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn prepare_stage(&self, stage: NamedTempFile) -> Result<NamedTempFile> {
        let (file, path) = stage.into_parts();
        drop(file);
        // CopyFileEx copies streams and extended attributes. Do not permit
        // decryption fallback or opening the source with write access.
        // SAFETY: both paths are live NUL-terminated strings; callbacks are absent.
        checked(unsafe {
            CopyFileExW(
                wide(&self.path)?.as_ptr(),
                wide(&path)?.as_ptr(),
                None,
                std::ptr::null(),
                std::ptr::null_mut(),
                COPY_FILE_COPY_SYMLINK,
            )
        })?;
        self.security.copy_to(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)?;
        if !file.metadata()?.is_file() || Security::read(&file)? != self.security {
            return Err(PublicationError::MetadataPreservation);
        }
        // Only the privately owned stage is truncated; named streams survive.
        file.set_len(0)?;
        Ok(NamedTempFile::from_parts(file, path))
    }

    pub(super) fn preserve_on(&self, stage: &File) -> Result<()> {
        let basic = FILE_BASIC_INFO {
            CreationTime: self.admitted.creation_time() as i64,
            LastAccessTime: self.admitted.last_access_time() as i64,
            LastWriteTime: self.admitted.last_write_time() as i64,
            ChangeTime: 0,
            FileAttributes: self.admitted.file_attributes(),
        };
        // SAFETY: live handle and native-layout FILE_BASIC_INFO of exact size.
        checked(unsafe {
            SetFileInformationByHandle(
                stage.as_raw_handle(),
                FileBasicInfo,
                (&basic as *const FILE_BASIC_INFO).cast(),
                size_of::<FILE_BASIC_INFO>() as u32,
            )
        })?;
        let copied = stage.metadata()?;
        if !same_policy(&self.admitted, &copied)
            || self.admitted.last_access_time() != copied.last_access_time()
            || Security::read(stage)? != self.security
        {
            return Err(PublicationError::MetadataPreservation);
        }
        Ok(())
    }

    pub(super) fn recheck(self) -> Result<PathBuf> {
        single_link(&self.file)?;
        let original = same_file::Handle::from_file(self.file.try_clone()?)?;
        let current = same_file::Handle::from_path(&self.path)?;
        if original != current
            || !same_snapshot(&self.admitted, &self.file.metadata()?)
            || !same_snapshot(&self.admitted, &fs::symlink_metadata(&self.path)?)
            || Security::read(&self.file)? != self.security
        {
            return Err(PublicationError::Changed);
        }
        Ok(self.path)
    }
}

/// tempfile's Windows persist resets file attributes to NORMAL. Keep the
/// copied attributes by using the underlying rename directly for replacement.
pub(super) fn persist(file: NamedTempFile, path: &Path) -> Result<()> {
    // SAFETY: live NUL-terminated same-volume paths; no copy fallback is enabled.
    checked(unsafe {
        MoveFileExW(
            wide(file.path())?.as_ptr(),
            wide(path)?.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    })?;
    // TempPath cleanup sees the now-absent temporary name, never the destination.
    Ok(())
}

fn same_policy(left: &Metadata, right: &Metadata) -> bool {
    left.file_attributes() == right.file_attributes()
        && left.creation_time() == right.creation_time()
        && left.last_write_time() == right.last_write_time()
}

fn same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    right.is_file() && left.file_size() == right.file_size() && same_policy(left, right)
}
