//! macOS metadata preservation using the system's descriptor-based copy API.
//! No data-copy flag is passed: the stage already contains the checked rewrite.

use super::{PublicationError, Result};
use std::collections::BTreeMap;
use std::ffi::{CString, c_void};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::macos::fs::MetadataExt as MacMetadataExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

// Darwin SDK <copyfile.h>: COPYFILE_STAT | COPYFILE_ACL | COPYFILE_XATTR.
const COPYFILE_METADATA: u32 = (1 << 0) | (1 << 1) | (1 << 2);
// Darwin <sys/fcntl.h>: refuse a raced symlink; do not block on a raced FIFO.
const READ_NOFOLLOW_NONBLOCK: i32 = 0x100 | 0x4;

unsafe extern "C" {
    fn fcopyfile(from: i32, to: i32, state: *mut c_void, flags: u32) -> i32;
    fn fsetattrlist(
        fd: i32,
        attributes: *mut AttributeList,
        values: *mut Timespec,
        size: usize,
        options: u32,
    ) -> i32;
    fn flistxattr(fd: i32, names: *mut c_void, size: usize, options: i32) -> isize;
    fn fgetxattr(
        fd: i32,
        name: *const std::ffi::c_char,
        value: *mut c_void,
        size: usize,
        position: u32,
        options: i32,
    ) -> isize;
    fn acl_get_fd(fd: i32) -> *mut c_void;
    fn acl_size(acl: *mut c_void) -> isize;
    fn acl_copy_ext(buffer: *mut c_void, acl: *mut c_void, size: isize) -> isize;
    fn acl_free(acl: *mut c_void) -> i32;
}

/// Darwin SDK `sys/attr.h`, selecting creation time only.
#[repr(C)]
struct AttributeList {
    bitmap_count: u16,
    reserved: u16,
    common: u32,
    volume: u32,
    directory: u32,
    file: u32,
    fork: u32,
}

/// Darwin's 64-bit timespec ABI (all supported macOS targets are 64-bit).
#[repr(C)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

/// Observable ACL and extended attributes, captured without decoding their values.
#[derive(PartialEq, Eq)]
struct ExtendedMetadata {
    attributes: BTreeMap<CString, Vec<u8>>,
    acl: AclSnapshot,
}

/// No extended ACL is different from a failed ACL observation.
#[derive(PartialEq, Eq)]
enum AclSnapshot {
    Absent,
    Present(Vec<u32>),
}

impl AclSnapshot {
    fn read(file: &File) -> Result<Self> {
        // SAFETY: a live descriptor; only nonnull results become owned ACLs.
        let raw = unsafe { acl_get_fd(file.as_raw_fd()) };
        match std::ptr::NonNull::new(raw) {
            Some(raw) => NativeAcl(raw).external().map(Self::Present),
            None => {
                let error = io::Error::last_os_error();
                // Darwin filesec_get_property reports ENOENT for an absent ACL.
                // This is a descriptor operation, not a missing pathname.
                if error.kind() == io::ErrorKind::NotFound {
                    Ok(Self::Absent)
                } else {
                    Err(error.into())
                }
            }
        }
    }
}

impl ExtendedMetadata {
    fn read(file: &File) -> Result<Self> {
        let fd = file.as_raw_fd();
        let names = native_bytes(|buffer, size| {
            // SAFETY: the helper supplies either null for sizing or a live
            // writable buffer of exactly size bytes. The file remains open.
            unsafe { flistxattr(fd, buffer, size, 0) }
        })?;
        let mut attributes = BTreeMap::new();
        for name in names.split_inclusive(|byte| *byte == 0) {
            let name = CString::from_vec_with_nul(name.to_vec())
                .map_err(|_| PublicationError::MetadataPreservation)?;
            let value = native_bytes(|buffer, size| {
                // SAFETY: same buffer contract, with a live NUL-terminated name.
                unsafe { fgetxattr(fd, name.as_ptr(), buffer, size, 0, 0) }
            })?;
            attributes.insert(name, value);
        }
        Ok(Self {
            attributes,
            acl: AclSnapshot::read(file)?,
        })
    }
}

/// Owns the ACL allocation; null/error pointers never enter this type.
struct NativeAcl(std::ptr::NonNull<c_void>);

impl NativeAcl {
    fn external(&self) -> Result<Vec<u32>> {
        // SAFETY: this owner holds a live acl_get_fd allocation.
        let size = unsafe { acl_size(self.0.as_ptr()) };
        let bytes = native_size(size)?;
        if bytes % size_of::<u32>() != 0 {
            return Err(PublicationError::MetadataPreservation);
        }
        // acl_copy_ext writes a native C structure. u32 storage supplies its
        // required alignment and a zeroed, deterministic padding representation.
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(bytes / size_of::<u32>())
            .map_err(|_| PublicationError::MetadataPreservation)?;
        buffer.resize(bytes / size_of::<u32>(), 0_u32);
        // SAFETY: the valid ACL and aligned writable storage remain live; size
        // is the exact size returned for this same ACL allocation.
        let copied = unsafe { acl_copy_ext(buffer.as_mut_ptr().cast(), self.0.as_ptr(), size) };
        if native_size(copied)? != bytes {
            return Err(PublicationError::MetadataPreservation);
        }
        Ok(buffer)
    }
}

impl Drop for NativeAcl {
    fn drop(&mut self) {
        // SAFETY: exactly the allocation admitted by acl_get_fd, freed once.
        unsafe {
            acl_free(self.0.as_ptr());
        }
    }
}

fn native_size(size: isize) -> Result<usize> {
    usize::try_from(size).map_err(|_| io::Error::last_os_error().into())
}

fn native_bytes(mut read: impl FnMut(*mut c_void, usize) -> isize) -> Result<Vec<u8>> {
    let size = native_size(read(std::ptr::null_mut(), 0))?;
    if size == 0 {
        return Ok(Vec::new());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| PublicationError::MetadataPreservation)?;
    bytes.resize(size, 0_u8);
    if native_size(read(bytes.as_mut_ptr().cast(), size))? != size {
        return Err(PublicationError::MetadataPreservation);
    }
    Ok(bytes)
}

/// A single-link ordinary file, held read-only with its admitted metadata.
pub(super) struct ExistingFile {
    path: PathBuf,
    file: File,
    admitted: Metadata,
    extended: ExtendedMetadata,
}

impl ExistingFile {
    pub(super) fn admit(path: PathBuf, attributes: Metadata) -> Result<Self> {
        if attributes.nlink() != 1 {
            return Err(PublicationError::MultipleLinks);
        }
        if attributes.st_flags() != 0 {
            return Err(PublicationError::FileFlags);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(READ_NOFOLLOW_NONBLOCK)
            .open(&path)?;
        let opened = file.metadata()?;
        if !same_snapshot(&attributes, &opened) {
            return Err(PublicationError::Changed);
        }
        let extended = ExtendedMetadata::read(&file)?;
        if !same_snapshot(&opened, &file.metadata()?) {
            return Err(PublicationError::Changed);
        }
        Ok(Self {
            path,
            file,
            admitted: opened,
            extended,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn preserve_on(&self, stage: &File) -> Result<()> {
        // SAFETY: both descriptors remain live for the synchronous call. The
        // documented null state selects default operation; the bitmask copies
        // POSIX metadata, ACLs and xattrs, never the old file's data.
        let result = unsafe {
            fcopyfile(
                self.file.as_raw_fd(),
                stage.as_raw_fd(),
                std::ptr::null_mut(),
                COPYFILE_METADATA,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error().into());
        }
        // COPYFILE_STAT's timestamp operation can make birthtime equal to
        // mtime rather than retaining the original nanosecond creation time.
        let mut attributes = AttributeList {
            bitmap_count: 5,
            reserved: 0,
            common: 0x0000_0200,
            volume: 0,
            directory: 0,
            file: 0,
            fork: 0,
        };
        let mut creation_time = Timespec {
            seconds: self.admitted.st_birthtime(),
            nanoseconds: self.admitted.st_birthtime_nsec(),
        };
        // SAFETY: SDK-layout structures select exactly one timespec; both are
        // live writable values of the exact advertised size; the FD is live.
        let result = unsafe {
            fsetattrlist(
                stage.as_raw_fd(),
                &mut attributes,
                &mut creation_time,
                size_of::<Timespec>(),
                0,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error().into());
        }
        // copyfile can succeed without privileged ownership changes. Never
        // convert that into successful publication with silently lost metadata.
        if !same_preserved_metadata(&self.admitted, &stage.metadata()?)
            || self.extended != ExtendedMetadata::read(stage)?
        {
            return Err(PublicationError::MetadataPreservation);
        }
        Ok(())
    }

    pub(super) fn recheck(self) -> Result<PathBuf> {
        if !same_snapshot(&self.admitted, &self.file.metadata()?)
            || !same_snapshot(&self.admitted, &fs::symlink_metadata(&self.path)?)
            || self.extended != ExtendedMetadata::read(&self.file)?
        {
            return Err(PublicationError::Changed);
        }
        Ok(self.path)
    }
}

fn same_preserved_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.atime() == right.atime()
        && left.atime_nsec() == right.atime_nsec()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.st_birthtime() == right.st_birthtime()
        && left.st_birthtime_nsec() == right.st_birthtime_nsec()
        && left.st_flags() == right.st_flags()
}

fn same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        // Reading a retained source may update atime. It does not invalidate
        // the content/identity evidence; the admitted atime is still copied.
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.st_birthtime() == right.st_birthtime()
        && left.st_birthtime_nsec() == right.st_birthtime_nsec()
        && left.st_flags() == right.st_flags()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_copy_is_exact_before_publication() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("old.cha");
        fs::write(&path, b"old output").unwrap();
        let existing = ExistingFile::admit(path.clone(), fs::metadata(&path).unwrap()).unwrap();
        let stage = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        let result = existing.preserve_on(stage.as_file());
        assert!(
            result.is_ok(),
            "{result:?}; original={:?}; staged={:?}",
            existing.admitted,
            stage.as_file().metadata().unwrap()
        );
    }
}
