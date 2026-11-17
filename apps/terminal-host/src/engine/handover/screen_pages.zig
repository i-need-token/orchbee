//! Bounds the page memory a screen section makes Ghostty allocate, before Ghostty allocates it.
//! Ghostty sizes each page from its PAGE header's capacities, then allocates and, in a safe build,
//! zeroes that memory before it checks the record's checksum, and it asserts that one page stays
//! under 4 GiB. Unchecked, one forged capacity could make an import take gigabytes or end the Host.
//! The scan reads only the framing, through Ghostty's own decoders for its envelope, each record
//! header, and each PAGE header, and leaves every other check to Ghostty's snapshot decoder.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

const snapshot = ghostty_vt.snapshot;
const Page = ghostty_vt.page.Page;

/// A quiesce refuses a terminal whose screen and history memory exceeds this as `too-large`, so an
/// export refuses a screen section whose pages would, and an import refuses such a record.
pub const max_page_memory_bytes: usize = 128 * 1024 * 1024;

/// The memory of one pooled page, which Ghostty gives every page whose layout fits it.
const standard_page_bytes = Page.layout(ghostty_vt.page.std_capacity).total_size;

pub const Error = error{
    /// The snapshot's framing is malformed or truncated.
    InvalidRecord,
    /// The pages would take more than `max_page_memory_bytes`.
    TooLarge,
};

/// The memory Ghostty allocates for the pages of the snapshot `screen`: a pooled page for each page
/// whose layout fits one, and its exact layout for each larger page.
pub fn pageMemory(screen: []const u8) Error!usize {
    var reader: std.Io.Reader = .fixed(screen);
    snapshot.envelope.decode(&reader) catch return error.InvalidRecord;
    var total: usize = 0;
    while (true) {
        const header = snapshot.record.Header.decode(&reader) catch return error.InvalidRecord;
        const payload = reader.take(header.payload_len) catch return error.InvalidRecord;
        switch (header.tag) {
            .finish => return total,
            .page => {
                var payload_reader: std.Io.Reader = .fixed(payload);
                const page = snapshot.page.Header.decode(&payload_reader) catch return error.InvalidRecord;
                if (page.columns == 0 or page.rows == 0) return error.InvalidRecord;
                const layout = Page.layout(.{
                    .cols = page.columns,
                    .rows = page.rows,
                    .styles = page.style_capacity,
                    .hyperlink_bytes = page.hyperlink_capacity_bytes,
                    .grapheme_bytes = page.grapheme_capacity_bytes,
                    .string_bytes = page.string_capacity_bytes,
                });
                total = std.math.add(usize, total, @max(layout.total_size, standard_page_bytes)) catch
                    return error.TooLarge;
                if (total > max_page_memory_bytes) return error.TooLarge;
            },
            else => {},
        }
    }
}
