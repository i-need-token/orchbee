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
import {
	DesktopSshProjectEnrollmentDialog,
	type DesktopSshProjectEnrollmentController,
} from '../desktop-ssh-project-enrollment-dialog';
import styles from './DesktopWorkbench.module.css';

export interface DesktopWorkbenchProps extends Omit<WorkbenchClientApplicationProps, 'children'> {
	readonly applicationLifetimeCoordinator: DesktopApplicationLifetimeDialogController;
	readonly browserAccessFinalFocus: () => HTMLElement | null;
	readonly browserAccessController: DesktopBrowserAccessController;
	readonly sshProjectEnrollmentController: DesktopSshProjectEnrollmentController;
	readonly windowControls: 'leading' | 'trailing';
}

export function DesktopWorkbench({
	applicationLifetimeCoordinator,
	applicationLocaleSource,
	browserAccessFinalFocus,
	browserAccessController,
	router,
	terminalSurfaceCapability,
	sshProjectEnrollmentController,
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
				<DesktopBrowserAccessDialog
					controller={browserAccessController}
					finalFocus={browserAccessFinalFocus}
				/>
				<DesktopSshProjectEnrollmentDialog controller={sshProjectEnrollmentController} />
			</WorkbenchClientApplication>
		</div>
	);
}
