import type { ReactNode } from 'react';

import { WebApplication } from '../../../web-application';
import { webApplicationRootRoute } from '../../web-application-root-route';

export function WorkbenchRoute(): ReactNode {
	const { application, coordinator } = webApplicationRootRoute.useRouteContext();
	return (
		<WebApplication
			actionRuntime={application.actionRuntime}
			agentNameSource={application.agentNameSource}
			agentObservationSource={application.agentObservationSource}
			applicationLocaleController={application.applicationLocaleController}
			authority={application.authority}
			clientBuild={application.clientBuild}
			clientUpdate={application.clientUpdate}
			environmentMarker={application.environmentMarker}
			keyboardShortcutSettingsController={application.keyboardShortcutSettingsController}
			layoutStorage={application.layoutStorage}
			macosTerminalInputSettingsController={application.macosTerminalInputSettingsController}
			navigation={coordinator.navigation}
			openExternal={application.openExternal}
			projectFolderAcquisition={application.projectFolderAcquisition}
			projectWorkspaceRuntime={application.projectWorkspaceRuntime}
			shortcutPlatform={application.shortcutPlatform}
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
