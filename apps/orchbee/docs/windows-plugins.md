# Windows native plugins

## Catalog model

The native plugin catalog is shared user configuration, independent of a Site lifetime. Windows `plugin link PATH`, `unlink ID`, `enable ID`, `disable ID`, `list [ID]`, `config-dir ID` and `action list [ID]` use the catalog without starting a Site. Linking records a manifest; it does not execute build or startup commands. Unlinking removes registration and retains the linked source and user configuration/state directories.

The local-site-runtime plugin boundary owns bounded TOML validation and the persistent catalog. A directory resolves `orchbee-plugin.toml`, or `herdr-plugin.toml` when the native filename is absent. An explicit manifest file is also accepted. Herdr metadata import does not imply compatibility with Herdr-specific executables or API calls. Identifiers are trimmed, case-sensitive ASCII, at most 120 characters; plugin IDs admit letters, digits, colon, dot, underscore and hyphen, while action IDs exclude dots. Arguments stay separate argv elements. Unknown fields, duplicate entry IDs, invalid commands and invalid platform names fail before registration.

Manifest files are limited to 128 KiB, catalogs to 32 entries and 1 MiB. Entries retain canonical source paths, enabled state and the last valid manifest. Reads refresh manifests; unavailable or invalid sources retain inspection metadata with an explicit warning and cannot authorize command execution. Entry platform lists override the plugin platform list; absent lists admit Windows. Other platform labels are metadata only.

## Persistence and transitions

Catalog storage lives under the admitted user configuration parent in `plugins`; the existing private Windows cache supplies ancestor validation, exclusive writer locking, flushed atomic replacement and interrupted-write cleanup. Reads never create directories. Mutations reread under the lock, validate the complete next catalog and increment its safe-integer revision only on a change. Corruption, contention and capacity limits are errors; they never become an empty catalog. Unlink of an absent ID is a no-op; enabling an absent ID fails. Hashed, prefixed ID path components keep config/state directories stable and safe for Windows reserved names. User directories remain after unlink.

## Delivery and evidence

This catalog is the first integrated plugin slice. Action execution and logs, managed installation/builds, startup/event hooks, link handlers and plugin Pane lifecycle require their own runtime integration before complete plugin parity. Catalog acceptance is not an execution receipt. Reference behavior is pinned Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, especially `cli/plugin.rs`, `app/api/plugins/manifest.rs` and `persist/plugin_registry.rs`.

Focused evidence covers parser limits, identity/path collisions, unavailable-source retention, read-only absence, atomic catalog transitions and actual Windows CLI receipts. Test fixtures own and remove their temporary paths. Full Windows visual/provider/multi-Client and aggregate regression acceptance remain separate.
