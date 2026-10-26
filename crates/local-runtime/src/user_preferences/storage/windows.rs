//! Reuses pinned ancestors, private ACL admission and the existing OS-held configuration lock.
use super::super::{CommitError, RECORD_NAME, UserPreferencesScope};
use crate::{PrivateCache, private_cache::ConfigurationWriteFailure};
use orchbee_workbench_protocol::{MAXIMUM_USER_PREFERENCES_BYTES, PreferencesFailure};
use std::io;

pub(in crate::user_preferences) fn read(
    scope: &UserPreferencesScope,
) -> Result<Option<Vec<u8>>, PreferencesFailure> {
    PrivateCache::read_existing(
        &scope.directory,
        RECORD_NAME,
        MAXIMUM_USER_PREFERENCES_BYTES,
    )
    .map_err(|_| PreferencesFailure::InvalidSource)
}
pub(in crate::user_preferences) struct Writer {
    cache: PrivateCache,
}
impl Writer {
    pub(in crate::user_preferences) fn acquire(
        scope: &UserPreferencesScope,
    ) -> Result<Self, PreferencesFailure> {
        PrivateCache::acquire(&scope.directory)
            .map(|cache| Self { cache })
            .map_err(|error| {
                if error.kind() == io::ErrorKind::WouldBlock {
                    PreferencesFailure::Busy
                } else {
                    PreferencesFailure::WriteFailed
                }
            })
    }
    pub(in crate::user_preferences) fn read(&self) -> Result<Option<Vec<u8>>, PreferencesFailure> {
        self.cache
            .read(RECORD_NAME, MAXIMUM_USER_PREFERENCES_BYTES)
            .map_err(|_| PreferencesFailure::InvalidSource)
    }
    pub(in crate::user_preferences) fn commit(
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
