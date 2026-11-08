//! Directory paths. Command descendants are projected from COMMANDS.

use super::FamilySpec;

macro_rules! families {
    ($($name:ident => [$first:literal $(, $word:literal)*],)*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) enum Family { $($name,)* }
        impl Family {
            pub(crate) const fn as_str(self) -> &'static str { match self { $(Self::$name => concat!($first $(, " ", $word)*),)* } }
        }
        pub(crate) const FAMILIES: &[FamilySpec] = &[$(FamilySpec { id: Family::$name, words: &[$first $(, $word)*], key: concat!($first $(, ":", $word)*), },)*];
    };
}

families! {
    PanelGraphics => ["panel", "graphics"],
    Client => ["client"],
    Layout => ["layout"],
    Agent => ["agent"],
    AgentView => ["agent", "view"],
    AgentIntegration => ["agent", "integration"],
    Project => ["project"],
    Workspace => ["workspace"],
    Tab => ["tab"],
    Panel => ["panel"],
    Terminal => ["terminal"],
    WorkbenchClient => ["terminal", "session"],
    Api => ["api"],
    Server => ["server"],
    Config => ["config"],
    Diagnostics => ["diagnostics"],
    Channel => ["channel"],
    Machine => ["machine"],
    Completion => ["completion"],
    Worktree => ["worktree"],
    LaunchProfile => ["launch-profile"],
    Session => ["session"],
    TerminalTitle => ["terminal", "title"],
    Plugin => ["plugin"],
    Command => ["command"],
    Events => ["events"],
    PluginAction => ["plugin", "action"],
    PluginLog => ["plugin", "log"],
    PluginPane => ["plugin", "pane"],
    Popup => ["popup"],
    Notification => ["notification"],
}
