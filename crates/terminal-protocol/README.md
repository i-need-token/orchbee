# Terminal protocol Rust binding

## Shared SGR style projection

`terminal_sgr_style` formats one validated cell style without allocation. Native CLI terminal
export and Server event matching share its canonical reset, flags, underline and RGB sequences.
These sequences describe captured styles, without claiming historical PTY input-byte identity.

This crate decodes and validates the bounded terminal frame and row-window contracts used by native
runtime owners. It performs no process, viewer, file or terminal I/O.

## Revision-admitted text selection

`extract_terminal_text_selection` extracts inclusive endpoints from an already captured
`TerminalTextWindowRows`. It checks dimensions, total rows, contiguous row identity, endpoint
membership, the 4096-row span and the caller's UTF-8 byte bound. Wide glyphs and combining tails
are selected once; only proven soft-wrap continuations join without a newline. Hard line ends trim
trailing spaces. Invalid or incomplete windows return a protocol error without partial text.

The caller owns physical source membership, content-revision admission and viewer cleanup. Native
CLI selection and Server configured plugin commands share this pure extractor; their different
output/context byte budgets remain explicit at those boundaries.

## Read-only PNG decoding

Kinds 80/81 use graphics-image encoding (5) with borrowed `GraphicsDecodeRequest` and `GraphicsDecodeResult` views. The OBPI request has a 40-byte header with attachment, declared dimensions and exact PNG length; the OBPX result has a 56-byte header with attachment, terminal, dimensions and exact RGBA length. The binding checks nonzero identities, PNG header agreement, 1–4096 dimensions, 16 MiB input and 32 MiB decoded pixels. PNG content decoding and physical read authorization belong to Host; Worker and Server check envelope correlation, epoch and target before consuming pixels. This encoding has its own bound and does not broaden raw keyboard or paste limits.
