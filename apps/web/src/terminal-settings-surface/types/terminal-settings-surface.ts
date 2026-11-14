import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';

import type { AgentIntegrationManagementControl } from '../../agent-integration-management';
import type { ApplicationLocaleController } from '../../application-locale';
import type { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import type { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import type { TerminalDefaultSettingsControl } from '../../terminal-default-settings-control';
import type { TerminalFeedbackSettingsController } from '../../terminal-feedback-settings';
import type { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
import type { TerminalLaunchProfileControl } from '../../terminal-launch-profile-control';
import type { TerminalSettingsController } from '../../terminal-settings';

/** The Settings page's categories, in navigation order. */
export type TerminalSettingsCategory =
	| 'agents'
	| 'appearance'
	| 'general'
	| 'keyboard'
	| 'notifications'
	| 'terminal'
	| 'themes';

/** What the folder picker answered; a failure's reason does not change what the page does. */
type TerminalSettingsFolderSelection =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'failed' }
	| { readonly kind: 'selected'; readonly path: string };

/** Asks the person for an existing folder; Desktop answers with its native folder picker. */
export interface TerminalSettingsFolderPicker {
	selectExistingFolder(): Promise<TerminalSettingsFolderSelection>;
}

export interface TerminalSettingsSurfaceProps {
	readonly agentIntegrationManagementControl?: AgentIntegrationManagementControl | undefined;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly controller: TerminalSettingsController;
	/** Custom folder… of the new terminal folder row asks it; without it the row takes a typed folder. */
	readonly folderPicker?: TerminalSettingsFolderPicker | undefined;
	/** Opens Settings on this category instead of General; stories and tests use it. */
	readonly initialCategory?: TerminalSettingsCategory | undefined;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	/** The configured command holding each default shortcut it took, by shortcut id. */
	readonly keyboardShortcutsTakenByConfiguredCommands?: ReadonlyMap<string, string> | undefined;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;
	readonly terminalDefaultSettingsControl?: TerminalDefaultSettingsControl | undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
	readonly terminalInteractionSettingsController?:
		| TerminalInteractionSettingsController
		| undefined;
	readonly terminalLaunchProfileClient?: TerminalLaunchProfileControl | undefined;
}

export interface TerminalColorThemeOptionView {
	readonly id: string;
	readonly name: string;
	/** The theme's background, foreground, and five ANSI colors, left to right. */
	readonly swatch: readonly { readonly color: string; readonly role: string }[];
}

export interface TerminalColorThemeListView {
	/** Whether the app appearance shows this list's themes now. */
	readonly inUse: boolean;
	readonly label: string;
	/** Why the list does not apply while the app appearance is fixed to the other polarity. */
	readonly note: string | null;
	readonly options: readonly TerminalColorThemeOptionView[];
	readonly polarity: 'dark' | 'light';
	readonly selectedId: string;
	readonly settingId: string;
	readonly onCommit: (themeId: string) => void;
	readonly onPreview: (themeId: string | null) => void;
}
