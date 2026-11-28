//! `fs.grant` and `fs.ungrant`, which open and release files beyond the root for one connection.
//! A grant reads the file's entry first and records the grant only when that read finds a file,
//! or finds nothing at a Save As destination, which the save then creates; so a failed grant
//! grants nothing, and a directory is refused, since a grant opens one file.

use std::{ffi::OsStr, io};

use orchbee_files_protocol::{
    EntryKind, ErrorCode, FilesError, FilesReply, FsGrantResult, GrantSource, Operation,
    PathsRequest,
};

use crate::{
    blocking::Helpers,
    calls::Call,
    entry, error,
    scope::{Scopes, View, path::NativePath},
};

/// Answers a grant of `path` from `source`, which `view` admitted, and tells whether it granted
/// the file. It reads the filesystem, so it runs on the call's own thread.
pub(crate) fn grant(
    scopes: &Scopes,
    helpers: &Helpers,
    view: &View,
    call: Call,
    path: NativePath,
    source: GrantSource,
) -> bool {
    let read = {
        let path = path.clone();
        helpers.run(Operation::FsGrant, move || {
            entry::read(path.as_path(), OsStr::new(path.name()))
        })
    };
    let found = match read {
        Ok(Err(failure))
            if source == GrantSource::SaveAs && failure.kind() == io::ErrorKind::NotFound =>
        {
            Ok(Ok(None))
        }
        read => read.map(|read| read.map(Some)),
    };
    let reply = match found {
        Ok(Ok(Some(entry))) if entry.kind == EntryKind::Directory => {
            FilesReply::Error(FilesError::new(ErrorCode::IsDirectory))
        }
        Ok(Ok(entry)) => match scopes.grant(view, path, || call.cancelled()) {
            Some(Ok(())) => FilesReply::FsGrant(FsGrantResult { entry }),
            Some(Err(error)) => FilesReply::Error(error),
            // The Client gave the grant up or its connection ended; dropping the call closes it
            // as cancelled.
            None => return false,
        },
        Ok(Err(failure)) => FilesReply::Error(error::from_io(&failure)),
        Err(refused) => FilesReply::Error(refused),
    };
    let granted = matches!(reply, FilesReply::FsGrant(_));
    call.reply(&reply);
    granted
}

/// Answers a release of `request`'s paths.
pub(crate) fn ungrant(scopes: &Scopes, view: &View, call: Call, request: &PathsRequest) {
    scopes.ungrant(view, request.paths.as_slice());
    call.reply(&FilesReply::Empty);
}
