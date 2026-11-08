# Native first-run guidance

## Ownership and scope

The Windows native Client owns non-capturing welcome guidance in the sidebar. Its purpose is to expose the next available actions while terminal input retains focus. The card introduces Workspace creation, choosing and starting an Agent through the existing Agent board, action search, and shortcut help. It never launches an Agent, installs software or creates a Workspace by itself. The existing operation owners revalidate every explicit action. GUI clients retain their own discovery surfaces; native learning state is not shared because the native prefix vocabulary is Client-specific.

## Visibility and interaction

The first connected native Client displays guidance in available sidebar space until explicit dismissal. Ordinary terminal keys remain terminal input. Buttons use the same Workspace creation, Agent board, action menu and help owners as keyboard bindings. The Help hint comes from the effective binding registry; each action does not repeat the common prefix. The default dismissal sequence is the configured prefix followed by F1. The prefix status advertises the effective sidebar-focus binding; the guidance itself keeps Help available.

Guidance reserves rows above the sidebar's Settings action, outside the scrolling tree. It never paints over terminal cells, changes terminal geometry, or covers native graphics. Modal surfaces, prefix discovery, sidebar navigation and resize interaction temporarily hide it. At least four tree rows remain. An available sidebar region of at least 24 columns and 15 rows shows all actions in seven rows; smaller regions of at least 18 columns and eight rows show Help and Dismiss in four rows. Otherwise guidance does not paint; the existing keyboard Help and dismissal bindings remain available. A hidden sidebar can be reopened through its existing control. Compact, hidden and temporarily suppressed guidance remain undismissed; returning from a modal never introduces a terminal overlay. Returning to an empty Workspace never reopens a dismissed guide.

## Persistent state

The state is `internal/state/onboarding.json` in the data root; an explicit `ORCHBEE_CONFIG_FILE` does not move it. A closed version 1 document contains `version`, `dismissed`, and an optional `extensions` object that a later release fills and this one keeps unread and writes back unchanged, bounded to 1024 bytes. A record of an earlier version is migrated in memory when it loads, and
dismissing the notice backs the original up under `internal/backups/` and saves the migration
first. A record with a version this build cannot use, saved by a newer version or not updated from an earlier one, is never rewritten: the notice shows again, a dismissal lasts only until the process ends, and the TUI shows the record message once. It is local to that data root and survives Session changes; no Session checkpoint, user preference or server publication contains this bit.

Reading does not create files. Missing state shows the card. Unreadable, oversized, unknown-version or malformed state shows guidance with a localized warning and preserves the existing bytes. Explicit dismissal acquires the existing private cache lock, reads and validates the latest document, and atomically writes the dismissed state. Concurrent dismissal is idempotent. Invalid existing data is never repaired or replaced. If saving fails, the card closes in this Client and a localized notice explains it may return after restart. The private cache owner handles permissions, bounded reads, publication residue and handle release.

## Evidence and lifecycle

Unit coverage includes absent and versioned state, malformed and oversized documents, two concurrent dismissal intents, persistence across new state instances, failed saves, preservation of corrupt bytes, both locales, painted pointer controls, compact geometry and unchanged cells and pointer ownership throughout the adjacent terminal. Render fixtures assert that opening guidance leaves the Client overlay and prefix inactive. The affected native host gate owns execution of these cases together with the existing input and pointer suites. Live terminal review should include ordinary typing, explicit clicks, dismissal and relaunch, custom bindings and a small viewport. Fixture owners release their private temporary directories after the final cache handle drops.
