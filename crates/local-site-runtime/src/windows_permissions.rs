//! Handle-based Windows runtime admission. Open ancestors without delete sharing
//! so an admitted path cannot be redirected by renaming one of its components.
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};
use windows_permissions::{
    LocalBox, SecurityDescriptor, Sid,
    constants::{AceType, SeObjectType, SecurityInformation},
    wrappers,
};

const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;

pub(crate) fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe native Windows runtime object",
    )
}

pub(crate) fn require_private(file: &File) -> io::Result<()> {
    let descriptor = wrappers::GetSecurityInfo(
        file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Owner | SecurityInformation::Dacl,
    )?;
    let user: LocalBox<Sid> = orchbee_local_ipc::current_user_sid()?.parse()?;
    let system: LocalBox<Sid> = "S-1-5-18".parse()?;
    let admins: LocalBox<Sid> = "S-1-5-32-544".parse()?;
    if descriptor.owner() != Some(&*user) {
        return Err(denied());
    }
    let acl = descriptor.dacl().ok_or_else(denied)?;
    for index in 0..acl.len() {
        let ace = acl.get_ace(index).ok_or_else(denied)?;
        if ace.ace_type() == AceType::ACCESS_DENIED_ACE_TYPE {
            continue;
        }
        if ace.ace_type() != AceType::ACCESS_ALLOWED_ACE_TYPE {
            return Err(denied());
        }
        let sid = ace.sid().ok_or_else(denied)?;
        if sid != &*user && sid != &*system && sid != &*admins {
            return Err(denied());
        }
    }
    Ok(())
}

pub(crate) fn protect_new(file: &mut File) -> io::Result<()> {
    require_private(file)?;
    let descriptor: LocalBox<SecurityDescriptor> = format!(
        "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)",
        orchbee_local_ipc::current_user_sid()?
    )
    .parse()?;
    wrappers::SetSecurityInfo(
        file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
        None,
        None,
        descriptor.dacl(),
        None,
    )?;
    require_private(file)
}

pub(crate) fn open_directory(path: &Path, writable_acl: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(3)
        .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS);
    if writable_acl {
        options.access_mode(0x0006_0001);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
        return Err(denied());
    }
    Ok(file)
}

pub(crate) fn open_tree(path: &Path) -> io::Result<Vec<File>> {
    if path.as_os_str().to_str().is_none_or(|text| {
        text.split(['\\', '/'])
            .any(|part| matches!(part, "." | ".."))
    }) || !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut prefix = PathBuf::new();
    let mut handles = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(value) => {
                if !matches!(
                    value.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) {
                    return Err(denied());
                }
                prefix.push(component.as_os_str());
            }
            Component::RootDir => {
                prefix.push(component.as_os_str());
                handles.push(open_directory(&prefix, false)?);
            }
            Component::Normal(value) => {
                prefix.push(value);
                handles.push(open_directory(&prefix, false)?);
            }
            _ => return Err(io::ErrorKind::InvalidInput.into()),
        }
    }
    require_private(handles.last().ok_or_else(denied)?)?;
    Ok(handles)
}

pub(crate) fn open_file(path: &Path, create: bool, write: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .create_new(create)
        .share_mode(3)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.file_attributes() & REPARSE_POINT != 0
        || winapi_util::file::information(&file)?.number_of_links() != 1
    {
        return Err(denied());
    }
    require_private(&file)?;
    Ok(file)
}
