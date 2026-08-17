import type { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchNavigationPort,
	type WorkbenchProjectWorkspaceRuntime,
	type WorkbenchShortcutPlatform,
	type WorkbenchTerminalSurface,
} from '@orchbee/workbench';
import type { ReactNode } from 'react';

import {
	type KeyboardShortcutSettingsController,
	useKeyboardShortcutSettings,
} from '../keyboard-shortcuts';
import type { TerminalSettingsController } from '../terminal-settings';
import { TerminalSettingsSurface } from '../terminal-settings-surface';
import { useWebApplication } from './hooks/useWebApplication';
import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly actionRuntime: ActionRuntime;
	readonly keyboardShortcutSettingsController: KeyboardShortcutSettingsController;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalSettingsController: TerminalSettingsController;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
}

export function WebApplication({
	actionRuntime,
	keyboardShortcutSettingsController,
	layoutStorage,
	navigation,
	projectWorkspaceRuntime,
	requestBrowserAccess,
	shortcutPlatform,
	terminalSettingsController,
	terminalSurface,
}: WebApplicationProps): ReactNode {
	const view = useWebApplication({ terminalSettingsController });
	const keyboardShortcuts = useKeyboardShortcutSettings(keyboardShortcutSettingsController);
	return (
		<div className={styles['root']} ref={view.applicationReference}>
			<div
				className={styles['workbench']}
				data-slot='web-application-workbench'
				ref={view.workbenchReference}
			>
				<Workbench
					actionRuntime={actionRuntime}
					layoutStorage={layoutStorage}
					navigation={navigation}
					onSettings={view.onSettings}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
					shortcutBindings={keyboardShortcuts.bindings}
					shortcutPlatform={shortcutPlatform}
					terminalSurface={terminalSurface}
				/>
			</div>
			<TerminalSettingsSurface
				controller={terminalSettingsController}
				keyboardShortcutController={keyboardShortcutSettingsController}
				keyboardShortcutPlatform={shortcutPlatform}
				onRequestClose={view.closeSettings}
				requestBrowserAccess={requestBrowserAccess}
			/>
		</div>
	);
}
