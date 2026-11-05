import type { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	type WorkbenchAgentSource,
	type WorkbenchAuthority,
	type WorkbenchNavigationPort,
	type WorkbenchProjectWorkspaceRuntime,
	type WorkbenchProjectFolderAcquisition,
	type WorkbenchShortcutPlatform,
	type WorkbenchShortcutClaim,
	type WorkbenchTerminalActionShortcutLabels,
	type WorkbenchTerminalSurface,
	type WorkbenchWorkspaceLocationSource,
	useWorkbenchConfiguredCommandKeys,
} from '../workbench';
import type { WorkbenchLayoutStorage } from '../workbench-layout-storage';
import { useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { AgentIntegrationManagementControl } from '../agent-integration-management';
import type { ApplicationLocaleController } from '../application-locale';
import {
	keyboardShortcutClaims,
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
import type { TerminalLaunchProfileControl } from '../terminal-launch-profile-control';
import type { TerminalDefaultSettingsControl } from '../terminal-default-settings-control';

import { useWebApplication } from './hooks/useWebApplication';
import styles from './WebApplication.module.css';

export interface WebApplicationProps {
	readonly actionRuntime: ActionRuntime;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly agentIntegrationManagementControl?: AgentIntegrationManagementControl | undefined;
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly authority: WorkbenchAuthority;
	readonly keyboardShortcutSettingsController: KeyboardShortcutSettingsController;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;

	readonly shortcutPlatform: WorkbenchShortcutPlatform;

	readonly terminalSettingsController: TerminalSettingsController;
	readonly terminalInteractionSettingsController?:
		| TerminalInteractionSettingsController
		| undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
	readonly terminalLaunchProfileClient?: TerminalLaunchProfileControl | undefined;
	readonly terminalDefaultSettingsControl?: TerminalDefaultSettingsControl | undefined;
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly workspaceLocationSource?: WorkbenchWorkspaceLocationSource | undefined;
}

export function WebApplication({
	actionRuntime,
	applicationLocaleController,
	agentIntegrationManagementControl,
	agentObservationSource,
	authority,
	keyboardShortcutSettingsController,
	layoutStorage,
	macosTerminalInputSettingsController,
	navigation,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	shortcutPlatform,
	terminalSettingsController,
	terminalInteractionSettingsController,
	terminalFeedbackSettingsController,
	terminalLaunchProfileClient,
	terminalDefaultSettingsControl,
	terminalSurface,
	workspaceLocationSource,
}: WebApplicationProps): React.ReactNode {
	const view = useWebApplication({ terminalSettingsController });
	const intl = useOrchbeeIntl();
	const keyboardShortcuts = useKeyboardShortcutSettings(keyboardShortcutSettingsController);
	const shortcutClaims: readonly WorkbenchShortcutClaim[] = React.useMemo(
		() => keyboardShortcutClaims(keyboardShortcuts, intl),
		[intl, keyboardShortcuts],
	);
	const configuredCommandKeys = useWorkbenchConfiguredCommandKeys({
		claims: shortcutClaims,
		commands: authority.configuredCommands,
		platform: shortcutPlatform,
	});
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
					agentNotificationObserver={terminalFeedbackSettingsController}
					agentObservationSource={agentObservationSource}
					authority={authority}
					configuredCommandKeys={configuredCommandKeys}
					isShortcutScopeActive={!view.settingsOpen}
					layoutStorage={layoutStorage}
					navigation={navigation}
					onSettings={view.onSettings}
					onTerminalFontSizeChange={view.onTerminalFontSizeChange}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
					projectFolderAcquisition={projectFolderAcquisition}
					shortcutBindings={keyboardShortcuts.bindings}
					shortcutPlatform={shortcutPlatform}
					terminalActionShortcutLabels={terminalActionShortcutLabels}
					terminalSurface={terminalSurface}
					workspaceLocationSource={workspaceLocationSource}
				/>
			</div>
			<TerminalSettingsSurface
				agentIntegrationManagementControl={agentIntegrationManagementControl}
				applicationLocaleController={applicationLocaleController}
				controller={terminalSettingsController}
				terminalInteractionSettingsController={terminalInteractionSettingsController}
				keyboardShortcutController={keyboardShortcutSettingsController}
				keyboardShortcutsTakenByConfiguredCommands={configuredCommandKeys.takenBy}
				keyboardShortcutPlatform={shortcutPlatform}
				macosTerminalInputSettingsController={macosTerminalInputSettingsController}
				onRequestClose={view.closeSettings}
				terminalFeedbackSettingsController={terminalFeedbackSettingsController}
				terminalLaunchProfileClient={terminalLaunchProfileClient}
				terminalDefaultSettingsControl={terminalDefaultSettingsControl}
			/>
		</div>
	);
}
