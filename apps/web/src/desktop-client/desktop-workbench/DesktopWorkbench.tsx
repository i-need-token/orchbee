import type { ReactNode } from 'react';

import {
	WorkbenchClientApplication,
	type WorkbenchClientApplicationProps,
} from '../../workbench-client';
import {
	DesktopApplicationLifetimeDialog,
	type DesktopApplicationLifetimeDialogController,
} from '../desktop-application-lifetime-dialog';

import styles from './DesktopWorkbench.module.css';

export interface DesktopWorkbenchProps extends Omit<WorkbenchClientApplicationProps, 'children'> {
	readonly applicationLifetimeCoordinator: DesktopApplicationLifetimeDialogController;
	readonly windowControls: 'leading' | 'trailing';
}

export function DesktopWorkbench({
	applicationLifetimeCoordinator,
	applicationLocaleSource,
	router,
	terminalSurfaceCapability,
	windowControls,
}: DesktopWorkbenchProps): ReactNode {
	return (
		<div
			className={styles['root']}
			data-slot='desktop-workbench'
			data-window-controls={windowControls}
		>
			<WorkbenchClientApplication
				applicationLocaleSource={applicationLocaleSource}
				router={router}
				terminalSurfaceCapability={terminalSurfaceCapability}
			>
				<DesktopApplicationLifetimeDialog controller={applicationLifetimeCoordinator} />
			</WorkbenchClientApplication>
		</div>
	);
}
