import type { ReactNode } from 'react';

import {
	WorkbenchClientApplication,
	type WorkbenchClientApplicationProps,
} from '../../workbench-client';
import {
	DesktopApplicationLifetimeDialog,
	type DesktopApplicationLifetimeDialogController,
} from '../desktop-application-lifetime-dialog';

export interface DesktopWorkbenchProps extends Omit<WorkbenchClientApplicationProps, 'children'> {
	readonly applicationLifetimeCoordinator: DesktopApplicationLifetimeDialogController;
}

export function DesktopWorkbench({
	applicationLifetimeCoordinator,
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
		</WorkbenchClientApplication>
	);
}
