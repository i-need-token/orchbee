//! Reuses pinned ancestors, private ACL admission and the existing OS-held configuration lock.
use super::{CommitError, RECORD_NAME, UserPreferencesScope};
use crate::{WindowsPrivateCache, private_cache_windows::ConfigurationWriteFailure};
use orchbee_workbench_protocol::{MAXIMUM_USER_PREFERENCES_BYTES, PreferencesFailure};
use std::io;

pub(super) fn read(scope: &UserPreferencesScope) -> Result<Option<Vec<u8>>, PreferencesFailure> {
    WindowsPrivateCache::read_existing(
        &scope.directory,
        RECORD_NAME,
        MAXIMUM_USER_PREFERENCES_BYTES,
    )
    .map_err(|_| PreferencesFailure::InvalidSource)
}
pub(super) struct Writer {
    cache: WindowsPrivateCache,
}
impl Writer {
    pub(super) fn acquire(scope: &UserPreferencesScope) -> Result<Self, PreferencesFailure> {
        WindowsPrivateCache::acquire(&scope.directory)
            .map(|cache| Self { cache })
            .map_err(|error| {
                if error.kind() == io::ErrorKind::WouldBlock {
                    PreferencesFailure::Busy
                } else {
                    PreferencesFailure::WriteFailed
                }
            })
    }
    pub(super) fn read(&self) -> Result<Option<Vec<u8>>, PreferencesFailure> {
        self.cache
            .read(RECORD_NAME, MAXIMUM_USER_PREFERENCES_BYTES)
            .map_err(|_| PreferencesFailure::InvalidSource)
    }
    pub(super) fn commit(
        &mut self,
        before: Option<&[u8]>,
        after: &[u8],
    ) -> Result<(), CommitError> {
        self.cache
            .publish_configuration_bounded(
                RECORD_NAME,
                before,
                after,
                MAXIMUM_USER_PREFERENCES_BYTES,
            )
            .map(|_| ())
            .map_err(|error| {
                CommitError::Refused(match error {
                    ConfigurationWriteFailure::SourceChanged => PreferencesFailure::SourceChanged,
                    ConfigurationWriteFailure::Io => PreferencesFailure::WriteFailed,
                })
            })
    }
}
