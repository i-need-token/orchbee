import type { ReactNode } from 'react';

import {
	WorkbenchClientApplication,
	type WorkbenchClientApplicationProps,
} from '../../workbench-client';
import {
	DesktopApplicationLifetimeDialog,
	type DesktopApplicationLifetimeDialogController,
} from '../desktop-application-lifetime-dialog';
import {
	DesktopBrowserAccessDialog,
	type DesktopBrowserAccessController,
} from '../desktop-browser-access-dialog';

export interface DesktopWorkbenchProps extends Omit<WorkbenchClientApplicationProps, 'children'> {
	readonly applicationLifetimeCoordinator: DesktopApplicationLifetimeDialogController;
	readonly browserAccessController: DesktopBrowserAccessController;
}

export function DesktopWorkbench({
	applicationLifetimeCoordinator,
	browserAccessController,
	locale,
	router,
	terminalSurfaceCapability,
}: DesktopWorkbenchProps): ReactNode {
	return (
		<WorkbenchClientApplication
			locale={locale}
			router={router}
			terminalSurfaceCapability={terminalSurfaceCapability}
		>
			<DesktopApplicationLifetimeDialog controller={applicationLifetimeCoordinator} />
			<DesktopBrowserAccessDialog controller={browserAccessController} />
		</WorkbenchClientApplication>
	);
}
