import type { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchProjectWorkspaceRuntime,
	type WorkbenchShortcutPlatform,
	type WorkbenchTerminalSurface,
} from '@orchbee/workbench';
import * as React from 'react';

import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly actionRuntime: ActionRuntime;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
}

export function WebApplication({
	actionRuntime,
	layoutStorage,
	projectWorkspaceRuntime,
	shortcutPlatform,
	terminalSurface,
}: WebApplicationProps): React.ReactNode {
	return (
		<div className={styles['root']}>
			<Workbench
				actionRuntime={actionRuntime}
				layoutStorage={layoutStorage}
				projectWorkspaceRuntime={projectWorkspaceRuntime}
				shortcutPlatform={shortcutPlatform}
				terminalSurface={terminalSurface}
			/>
		</div>
	);
}
