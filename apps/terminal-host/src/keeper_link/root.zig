//! The Host's Keeper link: each Keeper attachment's connection, the holds of the Host's running
//! terminals there, and their release before any of a terminal's handles close. Keeper I/O runs on
//! one link thread per attachment, never on the dispatch thread or a reader thread; the dispatch
//! thread starts and detaches links, offers terminals, and collects notices.

const attachments = @import("attachments.zig");
const notice_module = @import("notice.zig");

pub const Attachments = attachments.Attachments;
pub const Attach = attachments.Attach;
pub const Detach = attachments.Detach;
pub const Notice = notice_module.Notice;
pub const Unheld = notice_module.Unheld;
pub const Keeper = notice_module.Keeper;
