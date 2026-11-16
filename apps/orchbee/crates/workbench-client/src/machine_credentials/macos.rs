//! The macOS adapter: one generic password per item in the login Keychain, its service the item's
//! name exactly and its account `orchbee`. The calls run through `apple-native-keyring-store`'s
//! Keychain store. It writes no label or comment, so Keychain Access names the item by its service.
//! The item's access list trusts the program that created it by its code signature, so a build
//! with another signature, such as an unsigned update, is asked about again.

use super::{Adapter, Deleted, Failure, Item, Remembering, Secret};
use keyring_core::{Entry, Error, api::CredentialStoreApi};

/// The account every item carries; the service alone names the item.
const ACCOUNT: &str = "orchbee";

/// The login Keychain of the signed-in user.
pub(super) struct LoginKeychain;

impl LoginKeychain {
    /// The entry whose service is `service`; building it touches nothing in the store.
    fn entry(service: &str) -> Result<Entry, Failure> {
        let store =
            apple_native_keyring_store::keychain::Store::new().map_err(|_| Failure::Failed)?;
        store
            .build(service, ACCOUNT, None)
            .map_err(|_| Failure::Failed)
    }
}

/// The store's error as the port reports it. The error is dropped here unprinted.
fn failure(error: &Error) -> Failure {
    match error {
        Error::NoStorageAccess(_) => Failure::Unavailable,
        _ => Failure::Failed,
    }
}

impl Adapter for LoginKeychain {
    fn remembering(&self, probe: &str) -> Remembering {
        let Ok(entry) = Self::entry(probe) else {
            return Remembering::Unavailable;
        };
        // No item has the probe's name, so a reachable Keychain answers that nothing is there.
        match entry.get_credential() {
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

    fn write(&self, item: &Item, secret: &[u8], _label: &str) -> Result<(), Failure> {
        Self::entry(&item.target_name())?
            .set_secret(secret)
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
