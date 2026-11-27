//! The Linux adapter: one item per remembered answer in the Secret Service's default collection,
//! the login keyring that GNOME Keyring, KWallet or KeePassXC serve, found by its attributes: the
//! item's name exactly as `service` and `orchbee` as `username`, as the macOS adapter names its
//! Keychain service and account. The item carries the readable label. The calls run through
//! `oo7`'s D-Bus client on the session bus and never its file keyring, so nothing falls back to a
//! file; a login with no session bus, or no Secret Service on it, answers unavailable. A locked
//! collection or item is unlocked first, which shows the desktop's unlock prompt; a dismissed
//! prompt fails the call. Each call opens its own connection and waits for it on this thread, at
//! most `BOUND`.

use super::{Adapter, Deleted, Failure, Item, Remembering, Secret};
use async_io::Timer;
use futures_lite::future::{block_on, or};
use oo7::dbus::{Collection, Service};
use std::time::Duration;

/// The account every item carries; the service alone names the item.
const ACCOUNT: &str = "orchbee";
/// The alias of the collection Secret Service clients keep their items in.
const DEFAULT_COLLECTION: &str = "default";
/// The longest one call waits, its unlock prompt included: a locked keyring whose prompt nobody
/// can see, as in an SSH login to a computer whose desktop session holds the keyring, would
/// otherwise hold the call forever. The call is dropped at the bound together with its prompt, so
/// nothing is written after it failed.
const BOUND: Duration = Duration::from_secs(60);

/// The Secret Service on the person's session bus.
pub(super) struct SecretService;

/// Runs `call` on this thread until it ends or `BOUND` passes, which fails it.
fn bounded<T>(call: impl Future<Output = Result<T, Failure>>) -> Result<T, Failure> {
    block_on(or(call, async {
        Timer::after(BOUND).await;
        Err(Failure::Failed)
    }))
}

/// The attributes that find the item named `service`.
fn attributes(service: &str) -> [(&'static str, &str); 2] {
    [("service", service), ("username", ACCOUNT)]
}

/// The default collection of the service on the session bus, or none while it has none. No bus or
/// no service on it is unavailable; the service's own errors are dropped here unprinted.
async fn existing_collection() -> Result<Option<Collection>, Failure> {
    let service = Service::new().await.map_err(|_| Failure::Unavailable)?;
    service
        .with_alias(DEFAULT_COLLECTION)
        .await
        .map_err(|_| Failure::Unavailable)
}

impl Adapter for SecretService {
    fn remembering(&self, probe: &str) -> Remembering {
        // No item has the probe's name, so a reachable service answers that nothing is there; one
        // without a default collection yet makes it at the first write.
        let reachable = bounded(async {
            match existing_collection().await? {
                Some(collection) => collection
                    .search_items(&attributes(probe))
                    .await
                    .map(drop)
                    .map_err(|_| Failure::Unavailable),
                None => Ok(()),
            }
        });
        match reachable {
            Ok(()) => Remembering::Lasting,
            Err(_) => Remembering::Unavailable,
        }
    }

    fn read(&self, item: &Item) -> Result<Option<Secret>, Failure> {
        bounded(async {
            let Some(collection) = existing_collection().await? else {
                return Ok(None);
            };
            let found = collection
                .search_items(&attributes(&item.target_name()))
                .await
                .map_err(|_| Failure::Failed)?;
            let Some(stored) = found.first() else {
                return Ok(None);
            };
            if stored.is_locked().await.map_err(|_| Failure::Failed)? {
                stored.unlock(None).await.map_err(|_| Failure::Failed)?;
            }
            // oo7's secret wipes its own bytes when dropped at the end of this statement.
            let bytes = stored
                .secret()
                .await
                .map_err(|_| Failure::Failed)?
                .as_bytes()
                .to_vec();
            Ok(Some(Secret::new(bytes)))
        })
    }

    fn write(&self, item: &Item, secret: &[u8], label: &str) -> Result<(), Failure> {
        bounded(async {
            let service = Service::new().await.map_err(|_| Failure::Unavailable)?;
            // The default collection, made where the service has none yet.
            let collection = service
                .default_collection()
                .await
                .map_err(|_| Failure::Failed)?;
            if collection.is_locked().await.map_err(|_| Failure::Failed)? {
                collection.unlock(None).await.map_err(|_| Failure::Failed)?;
            }
            // The copy oo7 makes of the answer wipes itself when dropped.
            collection
                .create_item(label, &attributes(&item.target_name()), secret, true, None)
                .await
                .map(drop)
                .map_err(|_| Failure::Failed)
        })
    }

    fn delete(&self, item: &Item) -> Result<Deleted, Failure> {
        bounded(async {
            let Some(collection) = existing_collection().await? else {
                return Ok(Deleted::Absent);
            };
            let found = collection
                .search_items(&attributes(&item.target_name()))
                .await
                .map_err(|_| Failure::Failed)?;
            if found.is_empty() {
                return Ok(Deleted::Absent);
            }
            for stored in &found {
                stored.delete(None).await.map_err(|_| Failure::Failed)?;
            }
            Ok(Deleted::Removed)
        })
    }
}
