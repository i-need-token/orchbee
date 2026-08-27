import type { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	type WorkbenchLayoutStorage,
	type WorkbenchNavigationPort,
	type WorkbenchProjectWorkspaceRuntime,
	type WorkbenchProjectFolderAcquisition,
	type WorkbenchProjectWorktreeCapability,
	type WorkbenchShortcutPlatform,
	type WorkbenchTerminalActionShortcutLabels,
	type WorkbenchTerminalSurface,
} from '@orchbee/workbench';
import type { ReactNode } from 'react';

import {
	type KeyboardShortcutSettingsController,
	useKeyboardShortcutSettings,
} from '../keyboard-shortcuts';
import {
	PROJECTED_TERMINAL_SHORTCUTS,
	formatProjectedTerminalActionShortcut,
} from '../projected-terminal-keyboard-shortcuts';
import type { TerminalSettingsController } from '../terminal-settings';
import type { TerminalInteractionSettingsController } from '../terminal-interaction-settings';
import type { TerminalFeedbackSettingsController } from '../terminal-feedback-settings';
import type { MacosTerminalInputSettingsController } from '../macos-terminal-input-settings';
import { TerminalSettingsSurface } from '../terminal-settings-surface';
import type { DesktopTerminalLaunchProfileControl } from '../terminal-launch-profile-control';
import type { DesktopTerminalDefaultSettingsControl } from '../terminal-default-settings-control';
import { useWebApplication } from './hooks/useWebApplication';
import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly actionRuntime: ActionRuntime;
	readonly keyboardShortcutSettingsController: KeyboardShortcutSettingsController;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorktree?: WorkbenchProjectWorktreeCapability | undefined;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalSettingsController: TerminalSettingsController;
	readonly terminalInteractionSettingsController?:
		| TerminalInteractionSettingsController
		| undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
	readonly terminalLaunchProfileClient?: DesktopTerminalLaunchProfileControl | undefined;
	readonly terminalDefaultSettingsControl?: DesktopTerminalDefaultSettingsControl | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
}

export function WebApplication({
	actionRuntime,
	keyboardShortcutSettingsController,
	layoutStorage,
	macosTerminalInputSettingsController,
	navigation,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	projectWorktree,
	requestBrowserAccess,
	shortcutPlatform,
	terminalSettingsController,
	terminalInteractionSettingsController,
	terminalFeedbackSettingsController,
	terminalLaunchProfileClient,
	terminalDefaultSettingsControl,
	terminalSurface,
}: WebApplicationProps): ReactNode {
	const view = useWebApplication({ terminalSettingsController });
	const keyboardShortcuts = useKeyboardShortcutSettings(keyboardShortcutSettingsController);
	const terminalActionShortcutLabels: WorkbenchTerminalActionShortcutLabels = {
		[PROJECTED_TERMINAL_SHORTCUTS.clear.actionId]: formatProjectedTerminalActionShortcut(
			PROJECTED_TERMINAL_SHORTCUTS.clear.actionId,
			shortcutPlatform,
			keyboardShortcuts.bindings,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.copySelection.actionId]: formatProjectedTerminalActionShortcut(
			PROJECTED_TERMINAL_SHORTCUTS.copySelection.actionId,
			shortcutPlatform,
			keyboardShortcuts.bindings,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.findOpen.actionId]: formatProjectedTerminalActionShortcut(
			PROJECTED_TERMINAL_SHORTCUTS.findOpen.actionId,
			shortcutPlatform,
			keyboardShortcuts.bindings,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.paste.actionId]: formatProjectedTerminalActionShortcut(
			PROJECTED_TERMINAL_SHORTCUTS.paste.actionId,
			shortcutPlatform,
			keyboardShortcuts.bindings,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.selectAll.actionId]: formatProjectedTerminalActionShortcut(
			PROJECTED_TERMINAL_SHORTCUTS.selectAll.actionId,
			shortcutPlatform,
			keyboardShortcuts.bindings,
		),
	};
	return (
		<div className={styles['root']} ref={view.applicationReference}>
			<div
				className={styles['workbench']}
				data-slot='web-application-workbench'
				ref={view.workbenchReference}
			>
				<Workbench
					actionRuntime={actionRuntime}
					isShortcutScopeActive={!view.settingsOpen}
					layoutStorage={layoutStorage}
					navigation={navigation}
					onSettings={view.onSettings}
					onTerminalFontSizeChange={view.onTerminalFontSizeChange}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
					projectFolderAcquisition={projectFolderAcquisition}
					projectWorktree={projectWorktree}
					shortcutBindings={keyboardShortcuts.bindings}
					shortcutPlatform={shortcutPlatform}
					terminalActionShortcutLabels={terminalActionShortcutLabels}
					terminalSurface={terminalSurface}
				/>
			</div>
			<TerminalSettingsSurface
				controller={terminalSettingsController}
				terminalInteractionSettingsController={terminalInteractionSettingsController}
				keyboardShortcutController={keyboardShortcutSettingsController}
				keyboardShortcutPlatform={shortcutPlatform}
				macosTerminalInputSettingsController={macosTerminalInputSettingsController}
				onRequestClose={view.closeSettings}
				requestBrowserAccess={requestBrowserAccess}
				terminalFeedbackSettingsController={terminalFeedbackSettingsController}
				terminalLaunchProfileClient={terminalLaunchProfileClient}
				terminalDefaultSettingsControl={terminalDefaultSettingsControl}
			/>
		</div>
	);
}
