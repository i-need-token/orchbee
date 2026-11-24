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
	| 'machines'
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

/** What a machine has free: the short form, its three facts (none before the first read) and the sentence. */
interface TerminalSettingsMachineCapacity {
	readonly long: string;
	readonly parts: readonly { readonly low: boolean; readonly text: string }[];
	readonly summary: string;
}

/** A saved machine's color, the same names in every locale. */
export type TerminalSettingsMachineColor = 'cyan' | 'grey' | 'pink' | 'purple';

/** What a machine's Status button does. */
type TerminalSettingsMachineAct =
	| 'connect'
	| 'disconnect'
	| 'log-in'
	| 'retry'
	| 'retry-now';

/** One saved machine as Settings › Machines shows it; every sentence is already in the page's language. */
export interface TerminalSettingsMachine {
	readonly action: { readonly act: TerminalSettingsMachineAct; readonly label: string } | null;
	readonly address: string | null;
	readonly agents: number | null;
	readonly capacity: TerminalSettingsMachineCapacity;
	readonly color: TerminalSettingsMachineColor;
	readonly label: string;
	readonly lines: readonly string[];
	readonly machine: string;
	readonly notConnected: string;
	readonly removeDescription: string;
	/** `online`, `connecting`, `reconnecting`, `offline`, `disabled`, `needs-login`, `needs-update`, `local-needs-update`, or another word's key. */
	readonly state: string;
	readonly status: string;
	readonly stopConfirmation: string;
	readonly stopping: boolean;
	readonly terminals: number | null;
	readonly version: string | null;
	readonly word: string;
	readonly workspaces: number | null;
}

/** The saved machines, this computer's capacity, and how the last request ended. */
export interface TerminalSettingsMachinesView {
	readonly added: string | null;
	readonly catalog: 'read' | 'unreadable';
	/** Why no machine connection is reached, or null. */
	readonly hub: string | null;
	readonly local: TerminalSettingsMachineCapacity;
	readonly machines: readonly TerminalSettingsMachine[];
	/** How the last request ended; a new number is a new outcome even with the same words. */
	readonly notice: { readonly number: number; readonly text: string } | null;
	/** Whether saved passwords can be kept, so Saved password applies. */
	readonly passwords: boolean;
}

/** One request about a saved machine, carried out as the TUI's Settings › Machines carries it. */
export type TerminalSettingsMachineIntent =
	| {
			readonly act: 'forget-password' | 'remove' | 'stop' | TerminalSettingsMachineAct;
			readonly machine: string;
	  }
	| { readonly act: 'rename'; readonly label: string; readonly machine: string }
	| {
			readonly act: 'set-color';
			readonly color: TerminalSettingsMachineColor;
			readonly machine: string;
	  };

/** Settings › Machines: the window's saved machines, which its Main's machines process reports. */
export interface TerminalSettingsMachines {
	/** Null until the first report; Settings has no Machines category until then. */
	getSnapshot(): TerminalSettingsMachinesView | null;
	request(intent: TerminalSettingsMachineIntent): void;
	/** Has the machines' free capacity read while shown, until the returned call. */
	showCapacity(): () => void;
	subscribe(listener: () => void): () => void;
}

/** The device's installed font families; Desktop and capable browsers answer from Local Font Access. */
export interface TerminalSettingsFontCatalog {
	/** The families by name, or null when the listing was refused or failed. */
	listFamilies(): Promise<readonly string[] | null>;
}

/** The Server catalog also supplies names for Agents that have not been installed yet. */
export interface AgentLaunchCatalogSource {
	getSnapshot(): Pick<WorkbenchAgentCatalog, 'agents'> | null;
	subscribe(listener: () => void): () => void;
}

export interface TerminalSettingsSurfaceProps {
	readonly agentCatalog?: AgentLaunchCatalogSource | undefined;
	/** What About shows; without it Settings has no About category. */
	readonly about?: TerminalSettingsAbout | undefined;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly controller: TerminalSettingsController;
	/** The font row lists installed families from it; without it the row takes a typed family. */
	readonly fontCatalog?: TerminalSettingsFontCatalog | undefined;
	/** Settings › Machines; without it, or before its first report, Settings has no Machines category. */
	readonly machines?: TerminalSettingsMachines | undefined;
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
import type { WorkbenchAgentCatalog } from '@orchbee/workbench-client-protocol';
