import type { ReactNode } from 'react';

import { WebApplication } from '../../../web-application';
import { webApplicationRootRoute } from '../../web-application-root-route';

export function WorkbenchRoute(): ReactNode {
	const { application, coordinator } = webApplicationRootRoute.useRouteContext();
	return (
		<WebApplication
			actionRuntime={application.actionRuntime}
			environmentProjectConnection={application.environmentProjectConnection}
			keyboardShortcutSettingsController={application.keyboardShortcutSettingsController}
			layoutStorage={application.layoutStorage}
			macosTerminalInputSettingsController={application.macosTerminalInputSettingsController}
			navigation={coordinator.navigation}
			onConnectViaSsh={application.onConnectViaSsh}
			projectFolderAcquisition={application.projectFolderAcquisition}
			projectWorktree={application.projectWorktree}
			projectWorkspaceRuntime={application.projectWorkspaceRuntime}
			remoteTerminalCreation={application.remoteTerminalCreation}
			requestBrowserAccess={application.requestBrowserAccess}
			shortcutPlatform={application.shortcutPlatform}
			terminalSettingsController={application.terminalSettingsController}
			terminalInteractionSettingsController={application.terminalInteractionSettingsController}
			terminalFeedbackSettingsController={application.terminalFeedbackSettingsController}
			terminalLaunchProfileClient={application.terminalLaunchProfileClient}
			terminalDefaultSettingsControl={application.terminalDefaultSettingsControl}
			terminalSurface={application.terminalSurface}
			tabScope={application.tabScope}
			tabWindowControl={application.tabWindowControl}
		/>
	);
}
