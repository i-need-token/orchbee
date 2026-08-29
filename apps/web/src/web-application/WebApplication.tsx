import type { ActionRuntime } from '@orchbee/actions';
import type { AgentSnapshotSource } from '@orchbee/agent';
import {
	Workbench,
	type WorkbenchEnvironmentProjectConnectionCapability,
	type WorkbenchLayoutStorage,
	type WorkbenchNavigationPort,
	type WorkbenchProjectWorkspaceRuntime,
	type WorkbenchProjectFolderAcquisition,
	type WorkbenchProjectWorktreeCapability,
	type WorkbenchRemoteTerminalCreationCapability,
	type WorkbenchShortcutPlatform,
	type WorkbenchTabScopeSource,
	type WorkbenchTabWindowControl,
	type WorkbenchTerminalActionShortcutLabels,
	type WorkbenchTerminalSurface,
} from '@orchbee/workbench';
import type { ReactNode } from 'react';

import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../agent-integration-management';
import { AgentIntegrationReminder } from '../agent-integration-reminder';
import type { ApplicationLocaleController } from '../application-locale';
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
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly agentIntegrationManagementControl?: AgentIntegrationManagementControl | undefined;
	readonly agentIntegrationReminderController?: AgentIntegrationReminderController | undefined;
	readonly agentObservationSource?: AgentSnapshotSource | undefined;
	readonly environmentProjectConnection?:
		| WorkbenchEnvironmentProjectConnectionCapability
		| undefined;
	readonly keyboardShortcutSettingsController: KeyboardShortcutSettingsController;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly onConnectViaSsh?: (() => void) | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	readonly projectWorktree?: WorkbenchProjectWorktreeCapability | undefined;
	readonly remoteTerminalCreation?: WorkbenchRemoteTerminalCreationCapability | undefined;
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
	readonly tabScope?: WorkbenchTabScopeSource | undefined;
	readonly tabWindowControl?: WorkbenchTabWindowControl | undefined;
}

export function WebApplication({
	actionRuntime,
	applicationLocaleController,
	agentIntegrationManagementControl,
	agentIntegrationReminderController,
	agentObservationSource,
	environmentProjectConnection,
	keyboardShortcutSettingsController,
	layoutStorage,
	macosTerminalInputSettingsController,
	navigation,
	onConnectViaSsh,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	projectWorktree,
	remoteTerminalCreation,
	requestBrowserAccess,
	shortcutPlatform,
	terminalSettingsController,
	terminalInteractionSettingsController,
	terminalFeedbackSettingsController,
	terminalLaunchProfileClient,
	terminalDefaultSettingsControl,
	terminalSurface,
	tabScope,
	tabWindowControl,
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
					agentNotificationObserver={terminalFeedbackSettingsController}
					agentObservationSource={agentObservationSource}
					environmentProjectConnection={environmentProjectConnection}
					isShortcutScopeActive={!view.settingsOpen}
					layoutStorage={layoutStorage}
					navigation={navigation}
					onConnectViaSsh={onConnectViaSsh}
					onSettings={view.onSettings}
					onTerminalFontSizeChange={view.onTerminalFontSizeChange}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
					projectFolderAcquisition={projectFolderAcquisition}
					projectWorktree={projectWorktree}
					remoteTerminalCreation={remoteTerminalCreation}
					shortcutBindings={keyboardShortcuts.bindings}
					shortcutPlatform={shortcutPlatform}
					terminalActionShortcutLabels={terminalActionShortcutLabels}
					terminalSurface={terminalSurface}
					tabScope={tabScope}
					tabWindowControl={tabWindowControl}
				/>
			</div>
			{agentIntegrationManagementControl === undefined ||
			agentIntegrationReminderController === undefined ? null : (
				<AgentIntegrationReminder
					control={agentIntegrationManagementControl}
					controller={agentIntegrationReminderController}
					onReview={view.openAgentIntegrationSettings}
				/>
			)}
			<TerminalSettingsSurface
				agentIntegrationManagementControl={agentIntegrationManagementControl}
				agentIntegrationReminderController={agentIntegrationReminderController}
				applicationLocaleController={applicationLocaleController}
				controller={terminalSettingsController}
				terminalInteractionSettingsController={terminalInteractionSettingsController}
				keyboardShortcutController={keyboardShortcutSettingsController}
				keyboardShortcutPlatform={shortcutPlatform}
				macosTerminalInputSettingsController={macosTerminalInputSettingsController}
				onRequestClose={view.closeSettings}
				requestBrowserAccess={requestBrowserAccess}
				requestedCategory={view.requestedSettingsCategory}
				terminalFeedbackSettingsController={terminalFeedbackSettingsController}
				terminalLaunchProfileClient={terminalLaunchProfileClient}
				terminalDefaultSettingsControl={terminalDefaultSettingsControl}
			/>
		</div>
	);
}
