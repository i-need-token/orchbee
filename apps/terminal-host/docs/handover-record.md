# Handover record

A handover record carries one terminal's complete state from the Terminal Host that exports it to
the Host that imports it. This reference owns the record's format, frame format `obhr` of the
[generation registry](../../../schemas/connection/README.md#generation-registry);
`src/engine/handover/` is its only encoder and decoder. The
[protocol reference](../../../schemas/terminal/references/protocol.md#host-handover) owns the
chunks that carry a record between a Worker and a Host, and the
[runtime model](runtime-model.md#handover-export-and-import) owns when a Host exports or imports
one.

## Compatibility

- This reference describes generation 1. An import refuses a record of another generation, and the
  registry keeps generation 1 supported in every 1.x release.
- The `screen` section is the pinned Ghostty's binary snapshot, which Ghostty may change without a
  new version number, so an import refuses a record whose pin is not its own Ghostty package hash.
- An import skips an unknown section whose required flag is clear and refuses an unknown required
  section.
- Each section's encoder refuses what its decoder refuses, so an export never writes a section that
  an import of the same build cannot decode.
- Any change to the framing or to a section's layout is a new generation with its own stored sample.
  [`handover-record-1.obhr`](../src/engine/__tests__/fixtures/handover-record-1.obhr) is generation
  1's sample, which every later Host must import. It joins the registry's
  [frozen fixtures](../../../schemas/connection/README.md#frozen-fixtures) with the change that
  makes a Host offer `handover`.
- The stored sample names the Ghostty pin it was exported under, so a pin change keeps a reader that
  imports that pin's snapshots.

## Encoding

- Integers are little-endian; `i32` is signed and the others unsigned.
- A flag is one byte, `0` or `1`.
- An optional value is a flag, followed by the value when the flag is `1`.
- A counted value is a length of the stated width followed by that many bytes.
- A code is one byte that numbers a set's members from `1` in the order this reference lists them.
- An id is 16 bytes and a digest 32 bytes.
- A geometry is `u16` columns, `u16` rows, `u16` cell pixel width, and `u16` cell pixel height
  within the protocol's [hard bounds](../../../schemas/terminal/references/protocol.md#hard-bounds),
  with both pixel sizes zero, meaning unknown, or both nonzero.
- A section's fields fill its payload exactly. A flag, code, or numbered state out of range, a count
  or length beyond its bound, a field that runs past its section, or bytes after its last field
  refuse the record.

## Envelope

| Field           | Encoding        | Rule                                                              |
| --------------- | --------------- | ----------------------------------------------------------------- |
| Magic           | 4 bytes         | `OBHR`                                                            |
| Generation      | `u16`           | `1`                                                               |
| Reserved        | `u16`           | `0`                                                               |
| Product version | Counted by `u8` | The exporting Host's version: 1 to 64 bytes from `0x21` to `0x7e` |
| Pin             | Counted by `u8` | The exporting Host's Ghostty package hash: 1 to 128 such bytes    |
| Section count   | `u16`           | 1 to 32                                                           |
| Header checksum | `u32`           | CRC-32C of every earlier byte of the record                       |

The sections follow in strictly ascending tag order, and the record ends with the last payload:

| Field    | Encoding       | Rule                                                             |
| -------- | -------------- | ---------------------------------------------------------------- |
| Tag      | `u16`          | Greater than the previous section's tag                          |
| Flags    | `u16`          | Bit `0` marks the section required; every other bit is zero      |
| Length   | `u32`          | The payload's length                                             |
| Checksum | `u32`          | CRC-32C of the tag, flags, and length bytes and then the payload |
| Payload  | `length` bytes | The section                                                      |

- A record is at most 128 MiB. CRC-32C is the Castagnoli CRC that iSCSI uses.
- An import checks the framing and every checksum before the pin, so a damaged pin never reads as a
  foreign one, and the pin before the sections' tags.
- An import refuses a record without a `terminal`, `configuration`, `screen`, or `engine` section.

| Tag | Section         | An H1 Host writes it    |
| --- | --------------- | ----------------------- |
| `1` | `terminal`      | Always, as required     |
| `2` | `configuration` | Always, as required     |
| `3` | `screen`        | Always, as required     |
| `4` | `engine`        | Always, as required     |
| `5` | `console`       | On Windows, as required |

## `terminal`

The Host's facts about the terminal that its engine does not hold.

| Field                      | Encoding          | Rule                                                                |
| -------------------------- | ----------------- | ------------------------------------------------------------------- |
| Holding id                 | Id                | The Keeper holding; all zero until a Keeper holds the terminal      |
| Mark id                    | Id                | The quiesce's mark; all zero until a quiesce marks the holding      |
| Creation id                | Id                | The exporting Host's terminal identity, with the next two fields    |
| Host epoch                 | Id                |                                                                     |
| Terminal id                | Id                |                                                                     |
| Lifecycle                  | Below             |                                                                     |
| Revision                   | `u64`             | The content revision                                                |
| Generation                 | `u64`             | Nonzero                                                             |
| Geometry                   | Geometry          | Its columns and rows equal the snapshot's                           |
| Viewport geometry          | Geometry          | The viewport restore target                                         |
| Start geometry             | Optional geometry |                                                                     |
| Launch kind                | Code              | `managed_powershell`, `cmd`, `managed_zsh`, `managed_bash`, `other` |
| Interactive shell          | Flag              |                                                                     |
| Launch fingerprint         | Digest            |                                                                     |
| Environment overlay digest | Digest            |                                                                     |

The lifecycle is a `u8` state and its fields:

- `1` running: a termination request code, `none`, `graceful`, or `force`.
- `2` exited: a `u64` final revision, a termination request code, and an exit result.
- `3` failed: a `u64` final revision; a failure code, `admit_terminal_reply`, `apply_resize`,
  `dispatch_termination`, `observe_process`, `read_output`, `synchronize_worker`, or `write_input`;
  and a `u8` process observation, `1` observed and then an exit result, or `2` unavailable.
- An exit result is a `u8` kind, `1` exit code, `2` signal, or `3` unknown, and its `u32` value.

## `configuration`

| Field                  | Encoding       | Rule                                               |
| ---------------------- | -------------- | -------------------------------------------------- |
| Scrollback bytes       | `u64`          | At most the protocol's scrollback bound            |
| Configuration revision | `u64`          |                                                    |
| Theme operation id     | Optional id    | Present exactly when the theme revision is nonzero |
| Theme revision         | `u64`          |                                                    |
| Color scheme           | Code           | `dark`, `light`                                    |
| Default background     | `u32`          |                                                    |
| Default foreground     | `u32`          |                                                    |
| Default cursor         | Optional `u32` |                                                    |
| Palette                | 256 `u32`      | Entries `0` to `255`                               |

Every color is a `u32` of the form `0x00RRGGBB`.

## `screen`

- The pinned Ghostty's binary snapshot, version 1: the screens, history, styles, hyperlinks, modes,
  cursors, dynamic and default colors, title, and working directory, with Ghostty's continuation
  record.
- An H1 Host exports only at ground and writes the continuation empty; an import admits no
  continuation bytes.
- Before Ghostty allocates a page, an import adds up the memory of every `PAGE` record as Ghostty
  lays it out from the header's capacities, counting a pooled standard page for a page that fits
  one, and refuses the record past 128 MiB. An export refuses such a screen the same way. Ghostty
  would otherwise allocate and zero a forged capacity before checking the record, and it asserts
  that one page stays under 4 GiB.
- Kitty images are not in the snapshot, and a terminal that holds or is loading one is not exported.

## `engine`

The state the engine keeps beside Ghostty's, in this order:

| Field                                        | Encoding           | Rule                                                                                      |
| -------------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------- |
| Profile color scheme                         | Code               | `dark`, `light`                                                                           |
| Default cursor blinking                      | Flag               |                                                                                           |
| Default cursor shape                         | Code               | `block`, `underline`, `bar`                                                               |
| Primary device attributes conformance level  | `u16`              |                                                                                           |
| Primary device attributes features           | `u8` count, `u16`s | At most 64                                                                                |
| Secondary device attributes type             | `u16`              |                                                                                           |
| Secondary device attributes firmware version | `u16`              |                                                                                           |
| Secondary device attributes ROM cartridge    | `u16`              |                                                                                           |
| Tertiary device attributes unit id           | `u32`              |                                                                                           |
| Enquiry response                             | Counted by `u16`   | At most 256 bytes                                                                         |
| Version response                             | Counted by `u16`   | At most 256 bytes                                                                         |
| Bell sequence                                | `u64`              |                                                                                           |
| Bell overflow pending                        | Flag               |                                                                                           |
| Feedback overflow pending                    | Flag               |                                                                                           |
| Metadata revision                            | `u64`              | The revision of the last title or directory change, zero when never; at most the revision |
| Progress state                               | Code               | `remove`, `set`, `error`, `indeterminate`, `pause`                                        |
| Progress value                               | Optional `u8`      | At most 100; absent for `remove` and `indeterminate`                                      |
| Program status                               | Optional, below    |                                                                                           |
| Command completion                           | Optional, below    |                                                                                           |
| Command sequence                             | `u64`              |                                                                                           |
| Command elapsed                              | Optional `u64`     | Nanoseconds the running command had run at the export                                     |
| Clipboard sequence                           | `u64`              |                                                                                           |
| Pending clipboard write                      | Optional, below    |                                                                                           |
| Application signal sequence                  | `u64`              |                                                                                           |
| Application signals                          | `u8` count, below  | At most 64, oldest first                                                                  |
| Mouse input epoch                            | `u64`              | Nonzero                                                                                   |
| Mouse input epoch exhausted                  | Flag               |                                                                                           |
| Typed input sequence                         | `u64`              |                                                                                           |
| Held input                                   | Below              |                                                                                           |
| Prompt gate                                  | Below              |                                                                                           |
| Command observation                          | Below              |                                                                                           |
| Scanner partial state                        | Counted by `u32`   | At most 64 KiB; empty from an H1 Host, and an import refuses any other                    |
| Synchronized output remaining                | Optional `u64`     | Nanoseconds left before the mode-2026 deadline                                            |
| Cursor paint remaining                       | Optional `u64`     | Nanoseconds left before the cursor-paint deadline                                         |
| ConPTY size                                  | Optional size      | A queued resize: `u16` columns and `u16` rows                                             |
| ConPTY clear                                 | Flag               | A queued clear                                                                            |
| ConPTY size after clear                      | Optional size      | A resize queued behind the clear                                                          |

- The profile fields pass the checks of a launch profile.
- An H1 Host exports only once its signal sender is idle, so it writes no queued ConPTY signal, and
  an import refuses a record that names one.
- A program status is a code, `idle`, `working`, `blocked`, `done`, or `error`, and a nonzero `u64`
  report time in nanoseconds.
- A command completion is a `u64` sequence, a `u64` duration and a `u64` completion time in
  milliseconds, and a `u8` exit code.
- A pending clipboard write is a `u64` sequence, from 1 to the clipboard sequence, and text counted
  by `u32`, at most 16 KiB, that a clipboard write admits.
- Each application signal is a `u64` sequence, ascending and at most the application signal
  sequence, and a payload counted by `u32`: 1 byte to 4 KiB of UTF-8 without NUL.
- Dynamic and default colors are in the snapshot and are not repeated here.

**Held input** is what the program was last told of focus, keys, and buttons, per attachment of the
exporting Host: a `u16` count, at most 4096, and for each attachment

- a focused flag;
- a `u16` count, at most 1024, of held keys in press order, each counted by `u16`: the protocol's
  `KeyInput` payload from byte `24`, after its authority header, which must be a press;
- a `u8` count of held buttons in press order, each a distinct protocol mouse button value; and
- an optional mouse sample, present exactly when a button is held: the 16 bytes of the protocol's
  `MouseInput` payload from byte `24`.

**Prompt gate**: a phase code, `unmarked`, `prompting`, `input`, or `away`; a `u64` serial; and an
anchor, present exactly in phase `input`. The anchor is an input-pending flag, a `u64` input
sequence, a `u64` line digest, an optional `u64` age in nanoseconds of its input start, a
pending-wrap flag, and a `u8` place: `1` holds, then a screen code, `primary` or `alternate`, a `u16`
column, and a `u32` row counted from the screen's oldest history row; or `2` stale, for a pin whose
screen was replaced or whose row was discarded.

**Command observation**: a nonzero `u64` output epoch; a ready flag; an optional command, which is a
16-byte id that is not all zero, a completed flag, an `i32` exit code, a `u64` start row, a `u16`
start column, a `u64` end row, a `u16` end column, a `u64` generation, a `u64` output epoch, and a
`u64` start time in nanoseconds; a `u8` origin, `0` none, `1` holds and then a screen code, or `2`
moved; and an exhausted flag.

## `console`

| Field                     | Encoding        | Rule                                |
| ------------------------- | --------------- | ----------------------------------- |
| ConPTY version            | Counted by `u8` | 1 to 64 bytes from `0x21` to `0x7e` |
| OpenConsole SHA-256       | Digest          |                                     |
| Signal format             | `u16`           | Nonzero                             |
| Program process id        | `u32`           | Nonzero                             |
| Program creation time     | `u64`           | As the platform reports it          |
| OpenConsole process id    | `u32`           | Nonzero                             |
| OpenConsole creation time | `u64`           | As the platform reports it          |

## Not carried

Attachments, viewer leases, delivery sequences, the creation binding, and the compression cursor
stay with the exporting Host, and the Worker makes attachments again. Held input travels without
its attachment ids: the importing Host keys each entry under a fresh id and detaches it once it
claims the terminal, so the program hears the releases a detached attachment's repair sends. Kitty
images wait for an image section.
