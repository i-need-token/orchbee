//! A saved profile becomes one explicit creation request, without writing device defaults.
use super::{NativeNoticeContext, NativeSessionNotice, NativeUi};
use crate::client_view::SessionActionDisposition;
use orchbee_workbench_protocol::{LaunchSelection, UserLaunchCommand, UserLaunchProfile};
use std::time::Duration;

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
                let launch = orchbee_local_site_runtime::resolve_default_interactive_launch(
                    &orchbee_local_site_runtime::WindowsShellSettings::default(),
                )
                .map_err(|_| ())?;
                (launch.executable, launch.arguments)
            }
            UserLaunchCommand::SystemShell { .. } => return Err(()),
        };
        LaunchSelection::new(executable, arguments).map_err(|_| ())
    }
}

impl NativeUi {
    /// Submits one confirmed saved choice through ordinary creation without writing preferences.
    pub(super) fn create_tab_with_profile(
        &mut self,
        profile: &UserLaunchProfile,
    ) -> SessionActionDisposition {
        self.notice_context = Some(NativeNoticeContext::LaunchProfile(profile.name.clone()));
        // A removed or edited profile cannot be silently replaced by the new default.
        let launch = self
            .preferences
            .launch_profiles
            .profiles
            .iter()
            .find(|saved| *saved == profile)
            .and_then(|saved| self.launch_profiles.resolve(saved).ok());
        let Some(launch) = launch.filter(|_| !self.view().no_current_workspace()) else {
            self.command_notice = Some((
                NativeSessionNotice::LaunchProfileUnavailable,
                self.clock.now() + Duration::from_secs(5),
            ));
            return SessionActionDisposition::KeepNavigator;
        };
        let description = crate::terminal_session::state::LaunchDescription {
            profile_name: profile.name.clone(),
            executable: Some(launch.executable().to_owned()),
        };
        let disposition = self.create_tab_launch(None, Some(launch), Some(description));
        self.command_notice = Some((
            if disposition == SessionActionDisposition::DismissNavigator {
                NativeSessionNotice::LaunchProfileSelected
            } else {
                NativeSessionNotice::LaunchProfileUnavailable
            },
            self.clock.now() + Duration::from_secs(5),
        ));
        disposition
    }
}
