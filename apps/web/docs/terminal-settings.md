# Terminal Settings

## Ownership and scope

`@orchbee/web` owns one local terminal-appearance preference record, its Browser storage codec,
the Settings overlay, and the stable presentation-configuration source observed by projected
terminal attachments. The record is local to one Browser origin or Desktop renderer user-data
partition. It is not part of the shared Workbench document and is never sent to a Host, Gateway,
or remote terminal.

`@orchbee/terminal` owns the source-neutral renderer contract that consumes the derived complete
presentation configuration. Host frame metadata remains authoritative for terminal palette,
cursor, VT state, terminal color-scheme replies, and OSC behavior. A non-Host display theme is an
explicit per-client rendering override for default-coded foreground and background cells only.

This feature may analyze pasted Ghostty configuration into an ephemeral migration report. It does
not apply or persist that report, read Ghostty files, mutate a Host profile, expose a complete
palette editor, make an Electron window transparent, or synchronize preferences across clients.

## Version 1 preference record

The feature owns the key `orchbee:web:terminal-appearance:v1`. Its value is one complete JSON
object with no unknown fields:

```ts
interface StoredTerminalAppearancePreferencesV1 {
	readonly applicationTheme: 'light' | 'dark';
	readonly backgroundOpacityPercent: number;
	readonly customBackground: `#${string}`;
	readonly customForeground: `#${string}`;
	readonly fontFamily: string;
	readonly fontSize: number;
	readonly terminalDisplayTheme: 'host' | 'light' | 'dark' | 'custom';
}
```

The font family is trimmed and contains 1–256 characters. Font size is an integer from 8 through 48. Custom colors are exactly `#RRGGBB` and are normalized to lowercase. Background opacity is an
integer percentage from 10 through 100.

The complete product default is:

| Field                  | Default                                                            |
| ---------------------- | ------------------------------------------------------------------ |
| Application theme      | `light`                                                            |
| Font family            | `"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace` |
| Font size              | `13`                                                               |
| Terminal display theme | `host`                                                             |
| Custom foreground      | `#141414`                                                          |
| Custom background      | `#fcfcfc`                                                          |
| Background opacity     | `100`                                                              |

Terminal display themes derive the following default-color override without modifying the stored
custom pair:

| Theme    | Derived foreground | Derived background |
| -------- | ------------------ | ------------------ |
| `host`   | no override        | no override        |
| `light`  | `#141414`          | `#fcfcfc`          |
| `dark`   | `#d4d4d4`          | `#1e1e1e`          |
| `custom` | `customForeground` | `customBackground` |

Changing among Host, Light, and Dark retains the Custom values. Custom color controls are editable
only while Custom is selected. Application theme changes `data-theme` only and never publish a
terminal presentation update. Terminal settings never change `data-theme`.

## Read, commit, and recovery model

Startup reads the key once through injected `getItem` and parses `JSON.parse(...)` directly into a
strict Zod schema. A missing record uses the product default without a warning. A read exception,
malformed JSON, unknown field, old shape, or out-of-range value uses the complete default and
records a recoverable persistence issue; partial recovery is forbidden.

One semantic update follows this order:

1. Validate a complete candidate preference record.
2. Derive an immutable complete preference snapshot and terminal presentation configuration.
3. Commit the in-memory snapshot, apply application theme when it changed, and publish one terminal
   presentation snapshot only when terminal-derived values changed.
4. Attempt one write of the complete preference record.

The transaction boundary is the single publication of one immutable complete terminal
presentation snapshot. Browser storage and the asynchronous preparation performed by multiple
terminal attachments are not one cross-object atomic transaction.

Synchronous listener reentry is serialized as immutable presentation snapshots in first-in,
first-out order. A newer committed revision cannot be overwritten by an older revision's later
storage write. Disposal during publication stops the remaining callbacks and prevents any pending
publication or persistence work.

An invalid candidate does not change memory, theme, storage, or subscribers. A valid no-op does
nothing when persistence is healthy. A write exception does not roll back the session value; the
snapshot reports that the setting is applied for this session but is not saved. The next valid
update retries a complete write and clears that issue after success. One terminal preparation
failure retains that terminal's previously committed presentation without rolling back the
preference or other terminals.

Reset behaves as follows:

- Different preferences become the complete product default, write once, and publish at most one
  terminal presentation snapshot when terminal-derived values changed.
- Default preferences with a persistence issue rewrite the default record once without publishing
  a terminal presentation snapshot.
- Default preferences with healthy persistence are unchanged and perform no write or publication.

Disposal clears subscribers and rejects later semantic updates as disposed. No late update can
change theme, storage, or a presentation source after disposal.

## Effective terminal colors and opacity

Host metadata is never rewritten. A renderer resolves each packed color token first. Only
`TerminalColorEncoding.default` selects the derived effective foreground or background; explicit
RGB, projected ANSI/palette colors, `none`, and explicit black retain their Host-projected value.
Inverse swaps the already resolved display roles. The default underline uses the effective
foreground. The cursor always uses the Host cursor color.

Background opacity is derived from `backgroundOpacityPercent`; `allowTransparency` is not a second
user preference. At 100%, renderer clear and the terminal root are opaque. From 10% through 99%,
the renderer clears the default background transparently and the terminal root presents the
effective background with the selected alpha. Explicit backgrounds, inverse cells, selection, and
search highlights remain opaque. The implementation must not use whole-root CSS `opacity`, because
terminal text must remain opaque.

## Overlay and focus model

The Settings experience is an application-owned full-window overlay, not a route. Keeping the
Workbench mounted preserves terminal surface and attachment identity. On first open it captures a
still-connected active element and focuses Settings search. Repeated open is unchanged and does not
replace the return target. The Workbench subtree is genuinely inert while Settings is open, so its
terminal and Action shortcuts cannot consume Settings input.

Escape or Back closes Settings and restores the captured element, then the Settings Action target,
then the Workbench root as fallbacks. Desktop may inject only a semantic Browser-access request.
The existing Browser-access dialog remains above Settings; closing it returns focus to its Settings
entry, while Settings remains open. Browser and Gateway compositions do not receive this Desktop
capability.

The interaction structure follows the accepted Cursor reference: Back and search in a left
navigation column, only real product categories, grouped rows in a centered scrolling content
column, and immediate application without a Save button. Cursor branding, copy, categories, and
pixel geometry are not compatibility targets.

## Ghostty migration report

The Settings migration category analyzes user-pasted text without reading a file, invoking
Ghostty, logging the source, changing a preference, or writing Browser storage. The source and its
report exist only while that category is mounted; leaving the category or closing Settings clears
both. There is no Apply or Import action.

The analyzer is a deliberately bounded compatibility projection of Ghostty revision
`91f66da24527fa02d92b5fd0b41cd020f553a64c`, not a Ghostty configuration parser. It recognizes
newline-delimited `key = value` assignments, surrounding whitespace, whole-line comments, CRLF,
the first equals sign, and one pair of surrounding double quotes. It does not load `config-file`,
resolve themes, merge files, run commands, or model CLI precedence and platform defaults.

Every non-empty, non-comment line produces exactly one source-ordered finding:

| Finding | Meaning |
| ------- | ------- |
| Supported | The normalized value has one lossless current Orchbee setting target; it has not been applied. |
| Manual | A nearby user outcome exists, but the Ghostty value or combined semantics cannot be represented safely. |
| Ignored | The line is superseded, resets a source default, or is outside the migration whitelist; an unknown key may also be a typo. |
| Invalid | The bounded analyzer cannot interpret the line or value. Other valid lines still receive findings. |

Only these final effective values can be Supported:

| Ghostty key | Orchbee target | Supported source values |
| ----------- | -------------- | ----------------------- |
| `font-family` | Terminal font family | Exactly one active non-empty family of at most 256 characters without a CSS list delimiter |
| `font-size` | Terminal font size | A decimal value that is an integer from 8 through 48 |
| `foreground` | Custom foreground | Three- or six-digit hexadecimal RGB, with or without `#`; requires Custom display theme |
| `background` | Custom background | Three- or six-digit hexadecimal RGB, with or without `#`; requires Custom display theme |
| `background-opacity` | Background opacity | A value from 0.1 through 1 whose percentage is an integer, unless effective `background-opacity-cells` is `true` |

Later supported scalar assignments supersede earlier accepted assignments for the same key within
the pasted text. `font-family` retains Ghostty's empty-value reset and repeatable fallback-list
meaning; multiple active families are Manual instead of being guessed into a CSS list. Named
colors, named or light/dark theme files, `window-theme`, key bindings, working directories,
included configuration, font-style details, and cell-opacity semantics are Manual. Other keys are
Ignored with a non-whitelist diagnostic.

A supported color finding identifies one lossless value target, not a complete theme conversion.
The report tells the user to select Custom display theme and preserves the other current Orchbee
Custom color; it never invents a counterpart color that was absent from the pasted text.

Analysis rejects the complete source rather than truncating it when it exceeds 262,144 JavaScript
UTF-16 code units, 4,096 lines, or 4,096 code units on one line. These are Orchbee resource bounds,
not claims about Ghostty's byte limits.

## Required evidence

- Storage tests cover missing, malformed, unknown, old, out-of-range, read-throw, write-fail then
  success, Reset repair, and healthy no-op behavior.
- Source tests prove complete single publication, application-theme isolation, terminal-setting
  isolation, stable subscription, and disposal.
- Renderer tests cover Host and override modes, explicit black/RGB/palette, inverse, underline,
  cursor, Canvas 2D and WebGPU alpha, and unchanged Host metadata.
- Component tests cover both locales, search and empty results, invalid drafts, immediate updates,
  Reset, migration analysis and disposal, inertness, idempotent open, focus restoration, and nested
  Browser-access behavior.
- Migration analyzer tests cover accepted syntax, exact target boundaries, scalar supersession,
  font resets and fallback lists, opacity-cell interaction, malformed lines, resource limits, and
  one-finding-per-assignment ordering without depending on React or storage.
- Browser, Gateway, and Desktop composition tests prove the shared source and Settings entry; a
  Desktop Electron witness proves an existing projected terminal stays attached while Settings is
  opened and closed.
