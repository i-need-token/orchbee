# Terminal Settings

## Ownership and scope

`@orchbee/web` owns separate local terminal-appearance, terminal-feedback, and macOS terminal-input
preference records, their Browser storage codecs, the Settings overlay, and the stable client-side
sources observed by projected terminal attachments. The records are local to one Browser origin or
Desktop renderer user-data partition. They are not part of the shared Workbench document and are
never sent to a Gateway or remote terminal. Only the per-key semantic Option-as-Alt decision crosses
the Host protocol; the stored preference does not.

`@orchbee/terminal` owns the source-neutral renderer contract that consumes the derived complete
presentation configuration. Host frame metadata remains authoritative for terminal palette,
cursor, VT state, terminal color-scheme replies, and OSC behavior. A non-Host display theme is an
explicit per-client rendering override for default-coded foreground and background cells only.

This feature may analyze pasted Ghostty configuration into an ephemeral migration report. It does
not apply or persist that report, read Ghostty files, mutate a Host profile, expose a complete
palette editor, make an Electron window transparent, or synchronize preferences across clients.
It also projects a read-only remote-development diagnostic. That projection neither owns an SSH
process nor infers whether the current terminal is remote.

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

## macOS Option-as-Alt preference

The Desktop renderer on macOS additionally owns `orchbee:web:macos-terminal-input:v1`. The strict
record contains only `optionAsAlt: 'off' | 'left' | 'right' | 'both'`; `off` is the product default.
Browser, Gateway, Windows, and Linux compositions do not expose the category and retain the default.
Invalid or unreadable records fail to `off`, while a failed write keeps the accepted session value
and shows the existing persistence warning.

The client tracks physical `AltLeft` and `AltRight` ownership and resolves the selected side for
each admitted key event. It does not rewrite committed text or guess from the character produced by
the macOS layout. A complete input-configuration source updates every mounted projected terminal
immediately without replacing its attachment. `KeyInput.optionAsAlt` then carries only that event's
semantic decision; Terminal Host remains responsible for Ghostty's mode-dependent legacy,
modifyOtherKeys, and Kitty keyboard encoding.

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

## Remote-development diagnostic

The Remote development category documents the Desktop launch baseline
`TERM=xterm-256color` and exposes one constant POSIX-shell diagnostic command. The command reports a
version marker, the observed `TERM`, `COLORTERM`, and `TERM_PROGRAM`, whether TERM matches the
baseline, and exactly one terminfo outcome:

| Outcome                        | Meaning                                                |
| ------------------------------ | ------------------------------------------------------ |
| `TERMINFO=term-unset`          | The remote shell has no TERM value.                    |
| `TERMINFO=unsafe-term`         | TERM is not safe to print or pass to `infocmp`.        |
| `TERMINFO=infocmp-unavailable` | The remote shell cannot find an `infocmp` executable.  |
| `TERMINFO=available`           | `infocmp "$TERM"` succeeds.                            |
| `TERMINFO=missing`             | `infocmp` exists but cannot resolve the observed TERM. |

The command contains no user interpolation and runs its helper and temporary value inside one
subshell. It only reads environment values, prints values of at most 128 characters that match a
conservative display-safe character set, checks `command -v infocmp`, and evaluates
`command infocmp`'s exit status. Unsafe or oversized values receive a stable marker instead of being
echoed as potential terminal control input. It does not invoke SSH, access a network, install
terminfo, create a directory, read a configuration file, write remote state, or leave a helper
function in the user's interactive shell.
Settings does not execute or paste the command; the read-only field uses normal text selection so
the user remains responsible for choosing where and whether to run it.

Orchbee does not wrap the system SSH client, add `SendEnv` rules, persist remote hosts or
credentials, or maintain a remote cache. A standard requested SSH PTY carries TERM according to
OpenSSH behavior. Extra environment values cross the connection only when the user's client and
server configuration explicitly allow them. OSC 7 hostnames, process titles, and terminal output
are not remote-identity authority and do not alter this policy.

## Shell integration and completion reminders

The Shell integration category presents exact manual `source` commands for the Orchbee-owned Zsh,
Bash, and Fish resources shipped by Desktop. It never reads, creates, edits, or backs up a shell
startup file and never executes a displayed command. Zsh uses `preexec` and `precmd`; Fish uses
`fish_preexec` and `fish_postexec`; Bash requires version 4.4 or newer. The Bash 3.2 shipped by
macOS is explicitly unsupported. Removing the user-added `source` line and starting a new shell is
the complete rollback.

Desktop supplies `ORCHBEE_SHELL_INTEGRATION_DIR` from its trusted application-resource path for
new POSIX terminals. The scripts emit standard OSC 133 command boundaries and OSC 7 current
directory state. Orchbee does not accept an ambient override for this path, auto-source a script,
or transport command text. Terminal Host projects only the latest positive sequence, duration,
completion timestamp, and normalized exit code.

Completion reminders use the separate key `orchbee:web:terminal-feedback:v1`, whose strict JSON
value contains `completionReminderMode` (`never`, `unfocused`, or `always`) and
`completionReminderMinimumSeconds` (an integer from 1 through 3,600). The defaults are `never` and
5 seconds. Invalid or unreadable storage falls back to the complete default with a recoverable
warning; a failed write retains the current-session value and reports that it was not saved. Reset
restores both defaults.

Each client deduplicates by terminal identity and positive completion sequence, ignores facts more
than 60 seconds old or over 5 seconds in the future, and limits system notifications for one
terminal to one every 30 seconds. `unfocused` requires the document or that terminal root to lack
focus. Browser notification permission can be requested only by the dedicated Settings button;
terminal output, startup, storage restoration, and configuration analysis never request it. A
qualifying event still presents a localized status inside the terminal when the system capability
is unavailable. The notification contains duration and exit status only—never command text,
arguments, directory, environment, output, or user identity.

The current Workbench unmounts terminal Surfaces that belong entirely to inactive tabs. A mounted
but unfocused terminal can remind immediately, and reattachment can consider a completion fact
within the 60-second freshness window. This is not a claim of complete background monitoring:
completion while a Surface remains unmounted beyond that window does not produce an immediate
notification. A future all-tab monitor requires a separate lightweight Workbench/Host event
subscription rather than hidden renderers.

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

| Finding   | Meaning                                                                                                                    |
| --------- | -------------------------------------------------------------------------------------------------------------------------- |
| Supported | The normalized value has one lossless current Orchbee setting target; it has not been applied.                             |
| Manual    | A nearby user outcome exists, but the Ghostty value or combined semantics cannot be represented safely.                    |
| Ignored   | The line is superseded, resets a source default, or is outside the migration whitelist; an unknown key may also be a typo. |
| Invalid   | The bounded analyzer cannot interpret the line or value. Other valid lines still receive findings.                         |

Only these final effective values can be Supported:

| Ghostty key           | Orchbee target       | Supported source values                                                                                          |
| --------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `font-family`         | Terminal font family | Exactly one active non-empty family of at most 256 characters without a CSS list delimiter                       |
| `font-size`           | Terminal font size   | A decimal value that is an integer from 8 through 48                                                             |
| `foreground`          | Custom foreground    | Three- or six-digit hexadecimal RGB, with or without `#`; requires Custom display theme                          |
| `background`          | Custom background    | Three- or six-digit hexadecimal RGB, with or without `#`; requires Custom display theme                          |
| `background-opacity`  | Background opacity   | A value from 0.1 through 1 whose percentage is an integer, unless effective `background-opacity-cells` is `true` |
| `macos-option-as-alt` | Use Option as Alt    | `false`, `true`, `left`, or `right`, mapped to Off, Both, Left, or Right                                         |

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
- macOS input tests cover strict storage recovery, all four Option-side values, live source
  publication, physical-side resolution, mounted-terminal updates, and Host encoding with the
  semantic flag both disabled and enabled.
- Renderer tests cover Host and override modes, explicit black/RGB/palette, inverse, underline,
  cursor, Canvas 2D and WebGPU alpha, and unchanged Host metadata.
- Component tests cover both locales, search and empty results, invalid drafts, immediate updates,
  Reset, read-only remote diagnostics, migration analysis and disposal, inertness, idempotent open,
  focus restoration, and nested Browser-access behavior.
- Terminal-feedback model tests cover default-off behavior, strict persistence recovery, duration
  and focus policy, stale and future facts, sequence deduplication, notification rate limiting,
  explicit permission requests, and disposal. Component tests cover both locales, all three manual
  source commands, the Bash boundary, reminder controls, permission state, and Reset.
- Migration analyzer tests cover accepted syntax, exact target boundaries, scalar supersession,
  font resets and fallback lists, opacity-cell interaction, malformed lines, resource limits, and
  one-finding-per-assignment ordering without depending on React or storage.
- Browser, Gateway, and Desktop composition tests prove the shared source and Settings entry; a
  Desktop Electron witness proves an existing projected terminal stays attached while Settings is
  opened and closed. The current macOS system witness additionally starts a temporary loopback
  `sshd` with isolated keys and configuration, then proves an ordinary SSH PTY receives
  `TERM=xterm-256color`, resolves its terminfo entry, and does not receive `COLORTERM` or
  `TERM_PROGRAM` without explicit forwarding. A separate macOS Electron witness sources the
  shipped Zsh resource, observes one real semantic completion and localized reminder, and proves
  the projected attachment identity is unchanged.
