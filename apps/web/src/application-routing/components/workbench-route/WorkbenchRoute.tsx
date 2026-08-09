import type { ReactNode } from 'react';

import { WebApplication } from '../../../web-application';
import { webApplicationRootRoute } from '../../web-application-root-route';

export function WorkbenchRoute(): ReactNode {
	const { application, coordinator } = webApplicationRootRoute.useRouteContext();
	return (
		<WebApplication
			actionRuntime={application.actionRuntime}
			layoutStorage={application.layoutStorage}
			navigation={coordinator.navigation}
			projectWorkspaceRuntime={application.projectWorkspaceRuntime}
			shortcutPlatform={application.shortcutPlatform}
			terminalSurface={application.terminalSurface}
		/>
	);
}
