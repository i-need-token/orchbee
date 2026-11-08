//! The Worker's part in Windows scrollback editor documents (ORC-397): the data root folder it gives
//! the Terminal Host it starts, created private before each editor launch, and removal of documents
//! more than a day old when the Worker starts and before each editor launch. Documents appear only
//! through editor launches, so those two moments bound the folder without a timer, and a Session
//! start also clears what an ended Session's Host could not remove. The Host names, writes and
//! removes each document; the Worker never opens one.
use orchbee_local_runtime::{DataRoot, create_private_directory, remove_stale_editor_documents};
use std::{io, path::PathBuf, time::SystemTime};

/// The folder the Host this Worker starts writes its documents to; it may not exist yet.
pub(crate) fn folder() -> io::Result<PathBuf> {
    data_root().map(|root| root.tmp())
}

/// Removes this environment's day-old documents, recording what it removed or why it could not.
pub(crate) fn remove_stale() {
    match folder() {
        Ok(folder) => remove_stale_in(&folder),
        Err(error) => crate::diagnostic_events::editor_document_removal_failed(&error),
    }
}

/// Readies the folder for one editor launch: removes day-old documents, then creates the folder as
/// a private directory when it is missing, so the Host can write the new document there.
pub(crate) fn prepare_launch() -> io::Result<()> {
    let folder = folder()?;
    remove_stale_in(&folder);
    create_private_directory(&folder).map(drop)
}

fn remove_stale_in(folder: &std::path::Path) {
    match remove_stale_editor_documents(folder, SystemTime::now()) {
        Ok(removal) => crate::diagnostic_events::editor_documents_removed(removal),
        Err(error) => crate::diagnostic_events::editor_document_removal_failed(&error),
    }
}

fn data_root() -> io::Result<DataRoot> {
    DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))
}
