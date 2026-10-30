//! Painted list surfaces that accept bounded wheel scrolling.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    Setup(crate::workbench_client::SetupToken),
    Worktree(crate::workbench_client::WorktreeToken),
    Copy(crate::terminal_copy_mode::CopyModeToken),
    AgentBoard(crate::workbench_client::AgentBoardToken),
    Help(crate::client_view::help::HelpDrawSource),
    Settings(crate::native_settings::Token),
    Menu(crate::action_menu::MenuToken),
    Navigator(crate::client_view::navigator::Row),
    MachinePicker(crate::machine_picker::DrawnPickerList),
}
