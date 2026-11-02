import type { ReactNode } from 'react';

import {
	WorkbenchClientApplication,
	type WorkbenchClientApplicationProps,
} from '../../workbench-client';

import styles from './DesktopWorkbench.module.css';

export interface DesktopWorkbenchProps extends Omit<WorkbenchClientApplicationProps, 'children'> {
	/** What the window shows about its Session connection. */
	readonly connectionStatus?: ReactNode;
	readonly windowControls: 'leading' | 'trailing';
}

export function DesktopWorkbench({
	applicationLocaleSource,
	connectionStatus,
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
				{connectionStatus}
			</WorkbenchClientApplication>
		</div>
	);
}
