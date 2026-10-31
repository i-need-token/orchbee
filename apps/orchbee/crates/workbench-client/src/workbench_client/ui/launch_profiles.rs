//! A saved profile becomes one explicit creation request, without writing device defaults.
use super::{NoticeContext, WorkbenchClientNotice, WorkbenchClientUi};
use crate::client_view::ClientActionDisposition;
use orchbee_workbench_protocol::{LaunchSelection, UserLaunchCommand, UserLaunchProfile};
use std::time::Duration;

mod system_shell;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct Coordinator {
    local_system_shell: bool,
}

impl Coordinator {
    pub(super) fn set_origin(&mut self, origin: &crate::local_client::LocalClientOrigin) {
        // A managed Session admits only its same-device Worker. Until an origin
        // establishes direct local IPC, this Client cannot resolve that Worker's shell.
        self.local_system_shell = origin.is_local();
    }

    fn resolve(&self, profile: &UserLaunchProfile) -> Result<LaunchSelection, ()> {
        let (executable, arguments) = match &profile.command {
            UserLaunchCommand::CustomExecutable {
                executable,
                arguments,
            } => (executable.clone(), arguments.clone()),
            UserLaunchCommand::SystemShell { login: false } if self.local_system_shell => {
                let launch = system_shell::resolve().map_err(|_| ())?;
                (launch.executable, launch.arguments)
            }
            UserLaunchCommand::SystemShell { .. } => return Err(()),
        };
        LaunchSelection::new(executable, arguments).map_err(|_| ())
    }
}

impl WorkbenchClientUi {
    /// Submits one confirmed saved choice through ordinary creation without writing preferences.
    pub(super) fn create_tab_with_profile(
        &mut self,
        profile: &UserLaunchProfile,
    ) -> ClientActionDisposition {
        self.command_notice
            .set_context(NoticeContext::LaunchProfile(profile.name.clone()));
        // A removed or edited profile cannot be silently replaced by the new default.
        let launch = self
            .preferences
            .launch_profiles
            .profiles
            .iter()
            .find(|saved| *saved == profile)
            .and_then(|saved| self.launch_profiles.resolve(saved).ok());
        let Some(launch) = launch.filter(|_| !self.view().no_current_workspace()) else {
            self.command_notice.set(Some((
                WorkbenchClientNotice::LaunchProfileUnavailable,
                self.clock.now() + Duration::from_secs(5),
            )));
            return ClientActionDisposition::KeepNavigator;
        };
        let description = crate::workbench_state::LaunchDescription {
            profile_name: profile.name.clone(),
            executable: Some(launch.executable().to_owned()),
        };
        let disposition = self.create_tab_launch(None, Some(launch), Some(description));
        self.command_notice.set(Some((
            if disposition == ClientActionDisposition::DismissNavigator {
                WorkbenchClientNotice::LaunchProfileSelected
            } else {
                WorkbenchClientNotice::LaunchProfileUnavailable
            },
            self.clock.now() + Duration::from_secs(5),
        )));
        disposition
    }
}
