# Terminal protocol Rust binding

## Shared SGR style projection

`terminal_sgr_style` formats one validated cell style without allocation. Native CLI terminal
export and Server event matching share its canonical reset, flags, underline and RGB sequences.
These sequences describe captured styles, without claiming historical PTY input-byte identity.

This crate decodes and validates the bounded terminal frame and row-window contracts used by native
runtime owners. It performs no process, viewer, file or terminal I/O.

## Bounded frame metadata

The text-frame decoder exposes each row's existing prompt class and the existing extension-4
completion fact: exactly 25 bytes, with positive sequence and completion timestamp, duration in
milliseconds, and one-byte exit code. Duplicate completion or graphics extensions reject the frame.
Extension-2 graphics remains opaque: only the exact valid empty OBKG header proves absence;
other payloads conservatively mark graphics present. An omitted graphics extension means unchanged
on a delta, not empty. Projection owners carry that state forward. Tail presentation uses these
facts without inventing a terminal-output heuristic. Completion eligibility, deduplication and
notification delivery belong to clients. Frame-extension tests own malformed and boundary cases.

## Revision-admitted text selection

`extract_terminal_text_selection` extracts inclusive endpoints from an already captured
`TerminalTextWindowRows`. It checks dimensions, total rows, contiguous row identity, endpoint
membership, the 4096-row span and the caller's UTF-8 byte bound. Wide glyphs and combining tails
are selected once; only proven soft-wrap continuations join without a newline. Hard line ends trim
trailing spaces. Invalid or incomplete windows return a protocol error without partial text.

The caller owns physical source membership, content-revision admission and viewer cleanup. Native
CLI selection and Server configured plugin commands share this pure extractor; their different
output/context byte budgets remain explicit at those boundaries.

## Transport registry lookup

The transport registry has unique, strictly increasing numeric kinds, checked at compile time
before binary-search lookup. Kind 79 admits the existing correlated `ReadProcessInfo` request and
response as control JSON; kinds 80/81 retain their graphics-image contracts. Process-info payload
round trips also pass through transport encoding and decoding with correlation and Host epoch,
so a registered payload cannot silently become an unknown required transport message. Event roles
remain rejected and the existing payload bounds remain unchanged.

## Read-only PNG decoding

Kinds 80/81 use graphics-image encoding (5) with borrowed `GraphicsDecodeRequest` and `GraphicsDecodeResult` views. The OBPI request has a 40-byte header with attachment, declared dimensions and exact PNG length; the OBPX result has a 56-byte header with attachment, terminal, dimensions and exact RGBA length. The binding checks nonzero identities, PNG header agreement, 1–4096 dimensions, 16 MiB input and 32 MiB decoded pixels. PNG content decoding and physical read authorization belong to Host; Worker and Server check envelope correlation, epoch and target before consuming pixels. This encoding has its own bound and does not broaden raw keyboard or paste limits.
