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
	/** A Server Client has no lifetime to coordinate: leaving disconnects it and the Session stays. */
	readonly applicationLifetimeCoordinator?: DesktopApplicationLifetimeDialogController | undefined;
	/** What a Server Client shows about its Session connection; the Main-owned renderer has none. */
	readonly connectionStatus?: ReactNode;
	readonly windowControls: 'leading' | 'trailing';
}

export function DesktopWorkbench({
	applicationLifetimeCoordinator,
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
				{applicationLifetimeCoordinator === undefined ? null : (
					<DesktopApplicationLifetimeDialog controller={applicationLifetimeCoordinator} />
				)}
			</WorkbenchClientApplication>
		</div>
	);
}
