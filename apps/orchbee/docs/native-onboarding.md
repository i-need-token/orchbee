# Native first-run guidance

## Ownership and scope

The Windows native Client owns a non-capturing welcome card. Its purpose is to expose the next available actions while terminal input retains focus. The card introduces Workspace creation, choosing and starting an Agent through the existing Agent board, action search, and shortcut help. It never launches an Agent, installs software or creates a Workspace by itself. The existing operation owners revalidate every explicit action. GUI clients retain their own discovery surfaces; native learning state is not shared because the native prefix vocabulary is Client-specific.

## Visibility and interaction

The first connected native Client displays the card until explicit dismissal. Ordinary terminal keys remain terminal input. Buttons use the same Workspace creation, Agent board, action menu and help owners as keyboard bindings. Hints come from the effective binding registry. The default dismissal sequence is the configured prefix followed by F1. The sidebar's terminal-mode footer advertises Help instead of Browse; sidebar navigation remains discoverable in Help.

The card temporarily yields to modal surfaces, prefix discovery, sidebar navigation and resize interaction. It does not change terminal geometry. Its painted cells block pointer fallthrough and cover native graphics; outside cells retain normal terminal behavior. At widths below 32 columns or content heights below 12 rows, a compact card retains Help and Dismiss. Below 20 columns or five rows it does not paint; the keyboard dismissal binding remains available. Returning to an empty Workspace never reopens a dismissed guide.

## Persistent state

The state belongs beside the resolved native configuration file, in `<config-filename>.native-state/onboarding.json`. A closed version 1 document contains `version` and `dismissed`, bounded to 1024 bytes. It is local to that configuration profile and survives Session changes; no Session checkpoint, user preference or server publication contains this bit.

Reading does not create files. Missing state shows the card. Unreadable, oversized, unknown-version or malformed state shows guidance with a localized warning and preserves the existing bytes. Explicit dismissal acquires the existing private cache lock, reads and validates the latest document, and atomically writes the dismissed state. Concurrent dismissal is idempotent. Invalid existing data is never repaired or replaced. If saving fails, the card closes in this Client and a localized notice explains it may return after restart. The private cache owner handles permissions, bounded reads, publication residue and handle release.

## Evidence and lifecycle

Unit coverage includes absent and versioned state, malformed and oversized documents, two concurrent dismissal intents, persistence across new state instances, failed saves, preservation of corrupt bytes, both locales, painted pointer controls and compact geometry. Render fixtures assert that opening guidance leaves the Client overlay and prefix inactive. The affected native host gate owns execution of these cases together with the existing input and pointer suites. Live terminal review should include ordinary typing, explicit clicks, dismissal and relaunch, custom bindings and a small viewport. Fixture owners release their private temporary directories after the final cache handle drops.
