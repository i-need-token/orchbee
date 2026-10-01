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
