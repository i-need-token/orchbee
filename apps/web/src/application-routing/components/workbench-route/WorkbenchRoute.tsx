import type { ReactNode } from 'react';

import { WebApplication } from '../../../web-application';
import { webApplicationRootRoute } from '../../web-application-root-route';

export function WorkbenchRoute(): ReactNode {
	const { application, coordinator } = webApplicationRootRoute.useRouteContext();
	return (
		<WebApplication
			actionRuntime={application.actionRuntime}
			agentIntegrationManagementControl={application.agentIntegrationManagementControl}
			agentIntegrationReminderController={application.agentIntegrationReminderController}
			agentObservationSource={application.agentObservationSource}
			applicationLocaleController={application.applicationLocaleController}
			authority={application.authority}
			environmentProjectConnection={application.environmentProjectConnection}
			keyboardShortcutSettingsController={application.keyboardShortcutSettingsController}
			layoutStorage={application.layoutStorage}
			macosTerminalInputSettingsController={application.macosTerminalInputSettingsController}
			navigation={coordinator.navigation}
			onConnectViaSsh={application.onConnectViaSsh}
			onAddResourceFolderViaSsh={application.onAddResourceFolderViaSsh}
			projectFolderAcquisition={application.projectFolderAcquisition}
			projectWorkspaceRuntime={application.projectWorkspaceRuntime}
			remoteTerminalCreation={application.remoteTerminalCreation}
			shortcutPlatform={application.shortcutPlatform}
			standaloneTerminalAvailable={application.standaloneTerminalAvailable}
			terminalSettingsController={application.terminalSettingsController}
			terminalInteractionSettingsController={application.terminalInteractionSettingsController}
			terminalFeedbackSettingsController={application.terminalFeedbackSettingsController}
			terminalLaunchProfileClient={application.terminalLaunchProfileClient}
			terminalDefaultSettingsControl={application.terminalDefaultSettingsControl}
			terminalSurface={application.terminalSurface}
			workspaceLocationSource={application.workspaceLocationSource}
		/>
	);
}
