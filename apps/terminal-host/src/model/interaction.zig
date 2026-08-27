const protocol = @import("terminal-protocol");

pub const Detail = protocol.RowWindowDetail;
pub const max_row_window_rows = protocol.Limits.row_window_rows;

pub const Purpose = enum {
    buffer,
    search,
    viewport,
};

pub const RevisionPolicy = enum {
    current_view,
    exact,
};

pub const Origin = union(enum) {
    absolute: u64,
    committed: i32,
    oldest: u32,
    tail: u32,
};

pub const HistoryDemand = enum {
    following_bottom,
    viewing_history,
};

pub const Point = struct {
    column: u16,
    row: u64,
};

pub const ViewportPoint = struct {
    column: u16,
    row: u16,
};

pub const GestureGeometry = struct {
    cell_width: u16,
    columns: u16,
    padding_left: u16,
    screen_height: u16,
};

pub const GesturePosition = struct {
    x: i32,
    y: i32,
};

pub const GestureEvent = union(enum) {
    start: struct {
        click_count: u2,
        point: ViewportPoint,
        position: GesturePosition,
        select_output: bool,
    },
    update: struct {
        geometry: GestureGeometry,
        point: ViewportPoint,
        position: GesturePosition,
        rectangle: bool,
    },
    end: ViewportPoint,
    cancel,
};

pub const Command = union(enum) {
    clear,
    select_all,
    set_range: struct {
        end: Point,
        rectangle: bool,
        start: Point,
    },
    select_output: Point,
};

pub const ExportFormat = enum {
    html,
    plain,
    vt,
};

pub const SelectionPoint = Point;
pub const SelectionViewportPoint = ViewportPoint;
pub const SelectionGestureGeometry = GestureGeometry;
pub const SelectionGesturePosition = GesturePosition;
pub const SelectionGestureEvent = GestureEvent;
pub const SelectionCommand = Command;
