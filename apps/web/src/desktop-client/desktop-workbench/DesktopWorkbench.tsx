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
	actionRuntime,
	applicationLifetimeCoordinator,
	layoutStorage,
	locale,
	projectWorkspaceRuntime,
	shortcutPlatform,
	terminalSurfaceCapability,
}: DesktopWorkbenchProps): ReactNode {
	return (
		<WorkbenchClientApplication
			actionRuntime={actionRuntime}
			layoutStorage={layoutStorage}
			locale={locale}
			projectWorkspaceRuntime={projectWorkspaceRuntime}
			shortcutPlatform={shortcutPlatform}
			terminalSurfaceCapability={terminalSurfaceCapability}
		>
			<DesktopApplicationLifetimeDialog controller={applicationLifetimeCoordinator} />
		</WorkbenchClientApplication>
	);
}
