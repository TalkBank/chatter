//! Linux replacement preserves observable POSIX metadata and extended attributes
//! (including POSIX ACLs). Inode identity, ctime and birthtime necessarily change.

use super::{PublicationError, Result};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, FileTimes, Metadata, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use xattr::FileExt;

type Attributes = BTreeMap<OsString, Vec<u8>>;

fn attributes(file: &File) -> Result<Attributes> {
    file.list_xattr()?
        .map(|name| {
            let value = file.get_xattr(&name)?.ok_or(PublicationError::Changed)?;
            Ok((name, value))
        })
        .collect()
}

// EXTENTS is a storage implementation flag, not a user policy. Other flags
// (immutable, append-only, encryption, verity, compression, project policy,
// and unknown future flags) are refused rather than silently discarded.
fn admit_flags(file: &File) -> Result<()> {
    let mut flags: libc::c_long = 0;
    // SAFETY: a live descriptor and writable native-long ioctl result buffer.
    if unsafe { libc::ioctl(file.as_raw_fd(), libc::FS_IOC_GETFLAGS, &mut flags) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if flags & !0x0008_0000 != 0 {
        return Err(PublicationError::FileFlags);
    }
    Ok(())
}

pub(super) struct ExistingFile {
    path: PathBuf,
    file: File,
    admitted: Metadata,
    attributes: Attributes,
}

impl ExistingFile {
    pub(super) fn admit(path: PathBuf, admitted: Metadata) -> Result<Self> {
        if admitted.nlink() != 1 {
            return Err(PublicationError::MultipleLinks);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)?;
        if !same_snapshot(&admitted, &file.metadata()?) {
            return Err(PublicationError::Changed);
        }
        admit_flags(&file)?;
        let attributes = attributes(&file)?;
        Ok(Self {
            path,
            file,
            admitted,
            attributes,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn preserve_on(&self, stage: &File) -> Result<()> {
        let metadata = stage.metadata()?;
        if metadata.uid() != self.admitted.uid() || metadata.gid() != self.admitted.gid() {
            // SAFETY: the staging descriptor is live. Failure leaves the source
            // untouched; ownership is verified again below.
            if unsafe { libc::fchown(stage.as_raw_fd(), self.admitted.uid(), self.admitted.gid()) }
                < 0
            {
                return Err(io::Error::last_os_error().into());
            }
        }
        // Remove inherited attributes absent from the source, then copy source
        // ACLs and other observable attributes after ownership/mode operations.
        stage.set_permissions(self.admitted.permissions())?;
        for name in stage.list_xattr()? {
            if !self.attributes.contains_key(&name) {
                stage.remove_xattr(name)?;
            }
        }
        for (name, value) in &self.attributes {
            stage.set_xattr(name, value)?;
        }
        stage.set_times(
            FileTimes::new()
                .set_accessed(self.admitted.accessed()?)
                .set_modified(self.admitted.modified()?),
        )?;
        let copied = stage.metadata()?;
        if !same_policy(&self.admitted, &copied)
            || self.admitted.accessed()? != copied.accessed()?
            || self.attributes != attributes(stage)?
        {
            return Err(PublicationError::MetadataPreservation);
        }
        admit_flags(stage)?;
        Ok(())
    }

    pub(super) fn recheck(self) -> Result<PathBuf> {
        if !same_snapshot(&self.admitted, &self.file.metadata()?)
            || !same_snapshot(&self.admitted, &fs::symlink_metadata(&self.path)?)
            || self.attributes != attributes(&self.file)?
        {
            return Err(PublicationError::Changed);
        }
        admit_flags(&self.file)?;
        Ok(self.path)
    }
}

fn same_policy(left: &Metadata, right: &Metadata) -> bool {
    left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

fn same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    right.is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && same_policy(left, right)
}
