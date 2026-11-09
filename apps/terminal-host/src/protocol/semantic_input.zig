//! Normalized semantic input shared by protocol decoding and terminal encoding.
//! Wire validation happens before these values are constructed, so adapters can
//! work with domain types instead of repeating numeric tag and bit-mask checks.

pub const KeyAction = enum(u8) {
    release = 0,
    press = 1,
    repeat = 2,
};

pub const Modifiers = packed struct(u16) {
    shift: bool = false,
    control: bool = false,
    alt: bool = false,
    meta: bool = false,
    caps_lock: bool = false,
    num_lock: bool = false,
    shift_right: bool = false,
    control_right: bool = false,
    alt_right: bool = false,
    meta_right: bool = false,
    _reserved: u6 = 0,

    /// The protocol boundary must reject reserved bits and a right-side bit
    /// whose corresponding base modifier is not active before calling this.
    pub fn fromValidatedBits(_bits: u16) @This() {
        return @bitCast(_bits);
    }

    pub fn bits(self: @This()) u16 {
        return @bitCast(self);
    }
};

pub const MouseButton = enum(u8) {
    left = 1,
    right = 2,
    middle = 3,
    four = 4,
    five = 5,
    six = 6,
    seven = 7,
    eight = 8,
    nine = 9,
    ten = 10,
    eleven = 11,
};

pub const MouseButtons = packed struct(u16) {
    left: bool = false,
    right: bool = false,
    middle: bool = false,
    four: bool = false,
    five: bool = false,
    six: bool = false,
    seven: bool = false,
    eight: bool = false,
    nine: bool = false,
    ten: bool = false,
    eleven: bool = false,
    _reserved: u5 = 0,

    /// The protocol boundary must reject bits above `eleven` before calling this.
    pub fn fromValidatedBits(_bits: u16) @This() {
        return @bitCast(_bits);
    }

    pub fn bits(self: @This()) u16 {
        return @bitCast(self);
    }

    pub fn any(self: @This()) bool {
        return self.bits() != 0;
    }

    pub fn contains(self: @This(), button: MouseButton) bool {
        return switch (button) {
            .left => self.left,
            .right => self.right,
            .middle => self.middle,
            .four => self.four,
            .five => self.five,
            .six => self.six,
            .seven => self.seven,
            .eight => self.eight,
            .nine => self.nine,
            .ten => self.ten,
            .eleven => self.eleven,
        };
    }
};

pub const KeyInput = struct {
    action: KeyAction,
    composing: bool,
    consumed_modifiers: Modifiers,
    key: []const u8,
    modifiers: Modifiers,
    option_as_alt: bool = false,
    text: []const u8,
    unshifted_codepoint: u21,
};

pub const MouseMotion = union(enum) {
    /// Pointer movement with no held mouse button.
    hover,
    /// Pointer movement while the named active button is held.
    drag: MouseButton,
};

pub const MouseAction = union(enum) {
    press: MouseButton,
    release: MouseButton,
    motion: MouseMotion,
};

pub const MouseInput = struct {
    action: MouseAction,
    modifiers: Modifiers,
    /// The complete current pressed-button state. It is empty for hover and
    /// contains the active drag button for drag; a wheel press is stateless.
    pressed_buttons: MouseButtons,
    x: f32,
    y: f32,
};

pub const FocusInput = struct {
    focused: bool,
};

pub const PasteInput = struct {
    bracketed: bool,
    text: []const u8,
};

pub const PasteAutoInput = struct {
    content: []const u8,
};

pub const TextInput = struct {
    text: []const u8,
};

/// One shell command line that the Host types only at a confirmed fresh, empty
/// prompt: the content as a paste, an optional Right press, then Enter.
pub const PromptLineInput = struct {
    /// UTF-8 text without control characters; Enter is the only submission.
    content: []const u8,
    right_arrow: bool,
};

pub const SemanticInput = union(enum) {
    focus: FocusInput,
    key: KeyInput,
    mouse: MouseInput,
    paste: PasteInput,
    paste_auto: PasteAutoInput,
    prompt_line: PromptLineInput,
    text: TextInput,
};
