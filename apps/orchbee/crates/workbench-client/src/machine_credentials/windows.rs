//! The Windows adapter: one Credential Manager generic credential per item, its target name the
//! item's name exactly, kept on this computer only (`CRED_PERSIST_LOCAL_MACHINE`) and carrying the
//! readable label as its comment. The calls run through `windows-native-keyring-store`, which
//! wipes the system's copy of a credential it read before freeing it.

use super::{Adapter, Deleted, Failure, Item, Remembering, Secret};
use keyring_core::{Entry, Error, api::CredentialStoreApi};
use std::collections::HashMap;

/// The Credential Manager of the signed-in user.
pub(super) struct CredentialManager;

impl CredentialManager {
    /// The entry whose target name is `target`; building it touches nothing in the store.
    fn entry(target: &str) -> Result<Entry, Failure> {
        let store = windows_native_keyring_store::Store::new().map_err(|_| Failure::Failed)?;
        let modifiers = HashMap::from([("target", target), ("persistence", "Local")]);
        store
            .build("orchbee", "", Some(&modifiers))
            .map_err(|_| Failure::Failed)
    }
}

/// The store's error as the port reports it. The error is dropped here unprinted: some variants
/// carry the bytes the store held.
fn failure(error: &Error) -> Failure {
    match error {
        Error::NoStorageAccess(_) => Failure::Unavailable,
        _ => Failure::Failed,
    }
}

impl Adapter for CredentialManager {
    fn remembering(&self, probe: &str) -> Remembering {
        let Ok(entry) = Self::entry(probe) else {
            return Remembering::Unavailable;
        };
        // No item has the probe's name, so a reachable store answers that nothing is there.
        match entry.get_attributes() {
            Ok(_) | Err(Error::NoEntry) => Remembering::Lasting,
            Err(_) => Remembering::Unavailable,
        }
    }

    fn read(&self, item: &Item) -> Result<Option<Secret>, Failure> {
        match Self::entry(&item.target_name())?.get_secret() {
            Ok(bytes) => Ok(Some(Secret::new(bytes))),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(failure(&error)),
        }
    }

    fn write(&self, item: &Item, secret: &[u8], label: &str) -> Result<(), Failure> {
        let entry = Self::entry(&item.target_name())?;
        entry.set_secret(secret).map_err(|error| failure(&error))?;
        // The label is written after the secret, since only an existing credential takes one.
        let comment = HashMap::from([("comment", label)]);
        entry
            .update_attributes(&comment)
            .map_err(|error| failure(&error))
    }

    fn delete(&self, item: &Item) -> Result<Deleted, Failure> {
        match Self::entry(&item.target_name())?.delete_credential() {
            Ok(()) => Ok(Deleted::Removed),
            Err(Error::NoEntry) => Ok(Deleted::Absent),
            Err(error) => Err(failure(&error)),
        }
    }
}
