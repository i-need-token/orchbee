import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';

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
	| 'about'
	| 'agents'
	| 'appearance'
	| 'general'
	| 'keyboard'
	| 'notifications'
	| 'terminal'
	| 'themes';

/** What Check for updates shows, and why it cannot run where it cannot. */
export type TerminalSettingsUpdateCheckStatus =
	| { readonly kind: 'checking' | 'failed' | 'idle' | 'up-to-date' }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'development' | 'installation' | 'session';
	  };

/** Check for updates, as About reads and runs it. */
interface TerminalSettingsUpdateCheck {
	readonly getSnapshot: () => TerminalSettingsUpdateCheckStatus;
	readonly start: () => void;
	/** About shows the status from now until the returned call. */
	readonly show: () => () => void;
	readonly subscribe: (listener: () => void) => () => void;
}

/** The version, the build, and the update control About shows. */
export interface TerminalSettingsAbout {
	/** Which build this window is; null where the host does not say. */
	readonly build: 'development' | 'release' | 'test' | null;
	/** Check for updates; absent where the host has no update at all, which About says. */
	readonly check: TerminalSettingsUpdateCheck | undefined;
	/** Opens orchbee.com's install section in the browser when a check fails; absent where no external browser port is wired. */
	readonly downloadInstaller?: (() => void) | undefined;
	/** The newer version the window can switch to now, which puts Update in Check's place. */
	readonly offer:
		| {
				readonly getSnapshot: () => string | null;
				readonly subscribe: (listener: () => void) => () => void;
				/** Calls back each time a requested switch fails to launch the newer window. */
				readonly subscribeFailed?: ((listener: () => void) => () => void) | undefined;
		  }
		| undefined;
	/** Runs Update, the same Action as the sidebar footer's control. */
	readonly update: () => void;
	/** Increments each time Update refuses synchronously or before launch; About re-shows its notice. */
	readonly updateFailureCount?: number | undefined;
	/** The version this window runs; null leaves the Version row out. */
	readonly version: string | null;
}

/** What the folder picker answered; a failure's reason does not change what the page does. */
type TerminalSettingsFolderSelection =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'failed' }
	| { readonly kind: 'selected'; readonly path: string };

/** Asks the person for an existing folder; Desktop answers with its native folder picker. */
export interface TerminalSettingsFolderPicker {
	selectExistingFolder(): Promise<TerminalSettingsFolderSelection>;
}

/** The device's installed font families; Desktop and capable browsers answer from Local Font Access. */
export interface TerminalSettingsFontCatalog {
	/** The families by name, or null when the listing was refused or failed. */
	listFamilies(): Promise<readonly string[] | null>;
}

export interface TerminalSettingsSurfaceProps {
	/** What About shows; without it Settings has no About category. */
	readonly about?: TerminalSettingsAbout | undefined;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly controller: TerminalSettingsController;
	/** Custom folder… of the new terminal folder row asks it; without it the row takes a typed folder. */
	readonly folderPicker?: TerminalSettingsFolderPicker | undefined;
	/** The font row lists installed families from it; without it the row takes a typed family. */
	readonly fontCatalog?: TerminalSettingsFontCatalog | undefined;
	/** Opens Settings on this category instead of General; stories and tests use it. */
	readonly initialCategory?: TerminalSettingsCategory | undefined;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	/** The configured command holding each default shortcut it took, by shortcut id. */
	readonly keyboardShortcutsTakenByConfiguredCommands?: ReadonlyMap<string, string> | undefined;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;
	/** Opens the page where more Agents are requested; absent where no external browser is wired. */
	readonly requestAgent?: (() => void) | undefined;
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
}
