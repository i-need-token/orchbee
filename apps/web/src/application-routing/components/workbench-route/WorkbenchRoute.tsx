import type { ReactNode } from 'react';

import { WebApplication } from '../../../web-application';
import { webApplicationRootRoute } from '../../web-application-root-route';

export function WorkbenchRoute(): ReactNode {
	const { application, coordinator } = webApplicationRootRoute.useRouteContext();
	return (
		<WebApplication
			actionRuntime={application.actionRuntime}
			keyboardShortcutSettingsController={application.keyboardShortcutSettingsController}
			layoutStorage={application.layoutStorage}
			macosTerminalInputSettingsController={application.macosTerminalInputSettingsController}
			navigation={coordinator.navigation}
			projectFolderAcquisition={application.projectFolderAcquisition}
			projectWorkspaceRuntime={application.projectWorkspaceRuntime}
			requestBrowserAccess={application.requestBrowserAccess}
			shortcutPlatform={application.shortcutPlatform}
			terminalSettingsController={application.terminalSettingsController}
			terminalFeedbackSettingsController={application.terminalFeedbackSettingsController}
			terminalSurface={application.terminalSurface}
		/>
	);
}
