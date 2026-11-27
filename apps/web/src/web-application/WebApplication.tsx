import type { ActionRuntime } from '@orchbee/actions';
import {
	Workbench,
	findActiveTerminalFocusTarget,
	invokeWorkbenchClientUpdate,
	type WorkbenchAgentNameSource,
	type WorkbenchAgentSource,
	type WorkbenchAuthority,
	type WorkbenchClientUpdate,
	type WorkbenchEnvironmentMarker,
	type WorkbenchNavigationPort,
	type WorkbenchOtherMachines,
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

import type { ApplicationLocaleController } from '../application-locale';
import { createBrowserLocalFontCatalog } from '../browser-local-fonts';
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
import {
	ColorThemeQuickPick,
	MachineLoginDialog,
	MachineLoginOfferDialog,
	MachineMoveDialog,
	MachineMoveStatus,
	TerminalSettingsSurface,
	type TerminalSettingsAbout,
	type TerminalSettingsMachines,
} from '../terminal-settings-surface';
import { isSettingsFileReadOnly } from '../user-settings';
import type { TerminalLaunchProfileControl } from '../terminal-launch-profile-control';
import type { TerminalDefaultSettingsControl } from '../terminal-default-settings-control';

import { useWebApplication } from './hooks/useWebApplication';
import styles from './WebApplication.module.css';

/** Desktop and capable browsers list installed fonts the same way; others type a family. */
const BROWSER_LOCAL_FONT_CATALOG = createBrowserLocalFontCatalog();
const ORCHBEE_INSTALL_URL = 'https://orchbee.com/#install';
/** Where a person asks for another Agent's routine confirmations to be answered. */
const ORCHBEE_ISSUES_URL = 'https://github.com/i-need-token/orchbee/issues';

export interface WebApplicationProps {
	readonly actionRuntime: ActionRuntime;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly agentNameSource?: WorkbenchAgentNameSource | undefined;
	readonly agentObservationSource?: WorkbenchAgentSource | undefined;
	readonly authority: WorkbenchAuthority;
	/**
	 * Which build this window is and the version it runs, which Settings' About shows; a served Web
	 * Client has none.
	 */
	readonly clientBuild?: WebApplicationClientBuild | undefined;
	/** A newer version the Desktop window can switch to; a served Web Client has none. */
	readonly clientUpdate?: WorkbenchClientUpdate | undefined;
	/** The build the sidebar marks; a release window and a served Web Client have none. */
	readonly environmentMarker?: WorkbenchEnvironmentMarker | undefined;
	/** Whether the window offers the File Bar, which the Workbench follows. It defaults to withheld. */
	readonly fileBarOffered?: boolean | undefined;
	readonly keyboardShortcutSettingsController: KeyboardShortcutSettingsController;
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	/** Settings › Machines, where the window offers saved machines. */
	readonly machines?: TerminalSettingsMachines | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	/** Opens an external HTTPS link in the person's browser; absent where no external browser port is wired. */
	readonly openExternal?: ((target: string) => void) | undefined;
	/** Other machines' Workspaces, which the sidebar lists where the window holds several Sessions. */
	readonly otherMachines?: WorkbenchOtherMachines | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly projectFolderAcquisition?: WorkbenchProjectFolderAcquisition | undefined;
	/** Whether the window offers Search, which the Workbench follows. Search defaults to withheld. */
	readonly searchOffered?: boolean | undefined;

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

/** The build a window is and the version it runs; null when it is not known. */
interface WebApplicationClientBuild {
	readonly distribution: 'development' | 'release' | 'test';
	readonly version: string | null;
}

// Every route change renders the router's matches again while the application's inputs keep
// their identity, so a Tab or Workspace switch stops here instead of rendering the whole window.
export const WebApplication = React.memo(function WebApplication({
	actionRuntime,
	applicationLocaleController,
	agentNameSource,
	agentObservationSource,
	authority,
	clientBuild,
	clientUpdate,
	environmentMarker,
	fileBarOffered,
	keyboardShortcutSettingsController,
	layoutStorage,
	macosTerminalInputSettingsController,
	machines,
	navigation,
	openExternal,
	otherMachines,
	projectWorkspaceRuntime,
	projectFolderAcquisition,
	searchOffered,
	shortcutPlatform,
	terminalSettingsController,
	terminalInteractionSettingsController,
	terminalFeedbackSettingsController,
	terminalLaunchProfileClient,
	terminalDefaultSettingsControl,
	terminalSurface,
	workspaceLocationSource,
}: WebApplicationProps): React.ReactNode {
	const view = useWebApplication({
		findActiveTerminal: () => findActiveTerminalFocusTarget(projectWorkspaceRuntime),
		terminalSettingsController,
	});
	const intl = useOrchbeeIntl();
	// Settings sits beside the Workbench, so Update registers in this scope, which both reach.
	const hostActionScope = React.useMemo(
		() => actionRuntime.createScope('web-application'),
		[actionRuntime],
	);
	const [aboutUpdateFailureCount, setAboutUpdateFailureCount] = React.useState(0);
	const [wasSettingsOpen, setWasSettingsOpen] = React.useState(view.settingsOpen);
	if (wasSettingsOpen !== view.settingsOpen) {
		setWasSettingsOpen(view.settingsOpen);
		if (view.settingsOpen) setAboutUpdateFailureCount(0);
	}
	const requestAgent = React.useMemo(
		() => (openExternal === undefined ? undefined : () => openExternal(ORCHBEE_ISSUES_URL)),
		[openExternal],
	);
	const about = React.useMemo<TerminalSettingsAbout>(
		() => ({
			build: clientBuild?.distribution ?? null,
			check: clientUpdate?.check,
			downloadInstaller:
				openExternal === undefined ? undefined : () => openExternal(ORCHBEE_INSTALL_URL),
			offer: clientUpdate,
			update: () =>
				invokeWorkbenchClientUpdate(actionRuntime, hostActionScope, {
					onUnavailable: () => setAboutUpdateFailureCount((count) => count + 1),
				}),
			updateFailureCount: aboutUpdateFailureCount,
			version: clientBuild?.version ?? null,
		}),
		[
			aboutUpdateFailureCount,
			actionRuntime,
			clientBuild,
			clientUpdate,
			hostActionScope,
			openExternal,
		],
	);
	const settingsFile = React.useSyncExternalStore(
		terminalSettingsController.userSettings.subscribe,
		terminalSettingsController.userSettings.getSnapshot,
		terminalSettingsController.userSettings.getSnapshot,
	);
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
					agentNameSource={agentNameSource}
					agentNotificationObserver={terminalFeedbackSettingsController}
					agentObservationSource={agentObservationSource}
					authority={authority}
					clientUpdate={clientUpdate}
					configuredCommandKeys={configuredCommandKeys}
					environmentMarker={environmentMarker}
					fileBarOffered={fileBarOffered}
					hostActionScope={hostActionScope}
					isShortcutScopeActive={!view.settingsOpen}
					layoutStorage={layoutStorage}
					navigation={navigation}
					onSettings={view.onSettings}
					onShortcuts={view.onShortcuts}
					onTerminalFontSizeChange={view.onTerminalFontSizeChange}
					preferencesUnreadable={
						!view.settingsOpen && isSettingsFileReadOnly(settingsFile.snapshot)
					}
					projectWorkspaceRuntime={projectWorkspaceRuntime}
					projectFolderAcquisition={projectFolderAcquisition}
					searchOffered={searchOffered}
					shortcutBindings={keyboardShortcuts.bindings}
					shortcutPlatform={shortcutPlatform}
					suppressClientUpdateFailureNotice={view.settingsOpen}
					terminalActionShortcutLabels={terminalActionShortcutLabels}
					terminalSurface={terminalSurface}
					workspaceLocationSource={workspaceLocationSource}
					otherMachines={otherMachines}
					onManageMachines={machines === undefined ? undefined : view.openMachineSettings}
				/>
			</div>
			<TerminalSettingsSurface
				about={about}
				agentCatalog={authority.agentCatalog}
				requestAgent={requestAgent}
				applicationLocaleController={applicationLocaleController}
				controller={terminalSettingsController}
				fontCatalog={BROWSER_LOCAL_FONT_CATALOG}
				terminalInteractionSettingsController={terminalInteractionSettingsController}
				keyboardShortcutController={keyboardShortcutSettingsController}
				keyboardShortcutsTakenByConfiguredCommands={configuredCommandKeys.takenBy}
				keyboardShortcutPlatform={shortcutPlatform}
				macosTerminalInputSettingsController={macosTerminalInputSettingsController}
				machines={machines}
				onRequestClose={view.closeSettings}
				terminalFeedbackSettingsController={terminalFeedbackSettingsController}
				terminalLaunchProfileClient={terminalLaunchProfileClient}
				terminalDefaultSettingsControl={terminalDefaultSettingsControl}
			/>
			<ColorThemeQuickPick controller={terminalSettingsController} />
			{machines === undefined ? null : <MachineLoginDialog machines={machines} />}
			{machines === undefined ? null : <MachineLoginOfferDialog machines={machines} />}
			{machines === undefined ? null : <MachineMoveDialog machines={machines} />}
			{machines === undefined ? null : <MachineMoveStatus machines={machines} />}
		</div>
	);
});
