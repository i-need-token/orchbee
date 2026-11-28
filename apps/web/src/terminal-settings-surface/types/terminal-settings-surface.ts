import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';

import type { ApplicationLocaleController } from '../../application-locale';
import type { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import type { MachineColor } from '../../machine-color';
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
	| 'file-bar'
	| 'general'
	| 'keyboard'
	| 'machines'
	| 'notifications'
	| 'terminal'
	| 'themes';

/**
 * What Check for updates shows: a run checking, downloading `version` with the whole `percent` that
 * has arrived, or installing, how it ended, a failure by the phase it ended in, and why it cannot
 * run where it cannot.
 */
export type TerminalSettingsUpdateCheckStatus =
	| { readonly kind: 'checking' | 'idle' | 'installing' | 'up-to-date' }
	| { readonly kind: 'downloading'; readonly percent: number; readonly version: string }
	| { readonly kind: 'failed'; readonly phase: 'check' | 'download' | 'install' }
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
/**
 * What Update offers: a newer version of this window, or, while the Session runs the older
 * `sessionVersion`, a move of the Session onto this window's `version`, running now or waiting for
 * the reason `state` names.
 */
export type TerminalSettingsUpdateOffer =
	| { readonly kind: 'client'; readonly version: string }
	| {
			readonly kind: 'session';
			readonly sessionVersion: string;
			readonly state:
				| 'background-programs'
				| 'clients'
				| 'host-required'
				| 'moves-stopped'
				| 'other'
				| 'updating';
			readonly version: string;
	  };

export interface TerminalSettingsAbout {
	/** Which build this window is; null where the host does not say. */
	readonly build: 'development' | 'release' | 'test' | null;
	/** Check for updates; absent where the host has no update at all, which About says. */
	readonly check: TerminalSettingsUpdateCheck | undefined;
	/** Opens orchbee.com's install section in the browser when a check fails; absent where no external browser port is wired. */
	readonly downloadInstaller?: (() => void) | undefined;
	/**
	 * The newer version the window can switch to now, or the move of an older Session onto this
	 * window's version, which puts Update in Check's place.
	 */
	readonly offer:
		| {
				readonly getSnapshot: () => TerminalSettingsUpdateOffer | null;
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

/** A saved machine's color: a preset, the same name in every locale, or `#rrggbb`. */
export type TerminalSettingsMachineColor = MachineColor;

/** What a machine's Status button does. */
type TerminalSettingsMachineAct = 'connect' | 'disconnect' | 'log-in' | 'retry' | 'retry-now';

/** One saved machine as Settings › Machines shows it; every sentence is already in the page's language. */
export interface TerminalSettingsMachine {
	readonly action: { readonly act: TerminalSettingsMachineAct; readonly label: string } | null;
	readonly address: string | null;
	readonly agents: number | null;
	readonly capacity: TerminalSettingsMachineCapacity;
	/** The card a window shows over the terminals while the machine is chosen but not connected. */
	readonly card: {
		readonly lines: readonly string[];
		readonly notNow: string;
		readonly title: string;
	};
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

/** How the add in progress stands, as the TUI's Add Machine form shows it; the page keeps what is typed. */
interface TerminalSettingsMachineAdd {
	/** A failed check offers no way on until the address changes. */
	readonly blocked: boolean;
	/** The color the form offers first. */
	readonly color: TerminalSettingsMachineColor;
	/** The SSH config hosts the address can be chosen from: the alias and how it reads. */
	readonly hosts: readonly { readonly host: string; readonly label: string }[];
	/** The failed check's lines. */
	readonly lines: readonly string[];
	/** Why the add did not start: about the address, or null for a save that failed. */
	readonly problem: { readonly field: 'address' | null; readonly text: string } | null;
	/** The checks run: the fields wait, and Cancel gives the add up. */
	readonly running: boolean;
	/** The machine the add saved; the form closes. */
	readonly saved: string | null;
	readonly status: { readonly text: string; readonly tone: 'error' | 'pending' } | null;
	/** The primary button: Add, or the failed check's action. */
	readonly submit: string;
}

/** The notice of a command login the hub offers the window, worded as the TUI's notice. */
interface TerminalSettingsMachineOffer {
	/** The person chose Log in here; the login's question follows. */
	readonly accepted: boolean;
	/** What Not now does to the waiting commands; null once none waits. */
	readonly consequence: string | null;
	/** How many commands wait; null once none waits. */
	readonly count: string | null;
	/** Why an accepted login ended before its question; the notice only closes. */
	readonly ended: { readonly text: string; readonly tone: 'error' | 'success' | 'warning' } | null;
	readonly labels: {
		readonly close: string;
		readonly decline: string;
		readonly logIn: string;
		readonly starting: string;
		readonly title: string;
	};
	readonly login: number;
	/** `{name} · {target}`. */
	readonly machine: string;
}

/** The saved machines, this computer's capacity, and how the last request ended. */
export interface TerminalSettingsMachinesView {
	/** Add Machine's words, and its form while the page shows it. */
	readonly add: {
		readonly form: TerminalSettingsMachineAdd | null;
		readonly labels: {
			readonly add: string;
			readonly addMachine: string;
			readonly address: string;
			readonly cancel: string;
			readonly hosts: string;
			readonly title: string;
		};
	};
	readonly added: string | null;
	readonly catalog: 'read' | 'unreadable';
	/** Change machine's words, and its list in the order it takes when it opens. */
	readonly change: {
		readonly labels: {
			readonly find: string;
			readonly heading: string;
			readonly here: string;
			readonly manage: string;
			readonly noMatch: string;
			readonly note: string;
			readonly title: string;
		};
		/** This computer (null), then every saved machine by band and name. */
		readonly rows: readonly {
			readonly band: 'login' | 'low' | 'room' | 'unusable';
			readonly machine: string | null;
		}[];
	};
	/** Why no machine connection is reached, or null. */
	readonly hub: string | null;
	readonly local: TerminalSettingsMachineCapacity;
	/** The login question the person waits on, or null. */
	readonly login: TerminalSettingsMachineLogin | null;
	/** The notice of a command login offered to the window, or null. */
	readonly offer: TerminalSettingsMachineOffer | null;
	readonly machines: readonly TerminalSettingsMachine[];
	/** Change machine's move: the word while one runs, its question, how the last ended, where it landed. */
	readonly move: {
		readonly moved: {
			readonly machine: string | null;
			readonly number: number;
			readonly workspace: string;
		} | null;
		readonly notice: { readonly number: number; readonly text: string } | null;
		readonly pending: string | null;
		readonly question: TerminalSettingsMachineMoveQuestion | null;
	};
	/** How the last request ended; a new number is a new outcome even with the same words. */
	readonly notice: { readonly number: number; readonly text: string } | null;
	/** Whether saved passwords can be kept, so Saved password applies. */
	readonly passwords: boolean;
	/** The Workspaces each enabled machine listed when last read, kept for one not connected. */
	readonly seen: readonly TerminalSettingsMachineSeen[];
}

/** What a move asks before it stops a program, worded as the TUI's question. */
export interface TerminalSettingsMachineMoveQuestion {
	readonly cancel: string;
	readonly files: string;
	readonly move: string;
	readonly question: number;
	readonly sentence: string;
	readonly title: string;
	readonly workspace: string;
}

/** One machine's entry in the last-seen list, in its Session's order. */
interface TerminalSettingsMachineSeen {
	readonly machine: string;
	readonly projects: readonly { readonly id: string; readonly name: string }[];
	readonly workspaces: readonly {
		readonly id: string;
		readonly name: string;
		readonly projectId: string | null;
	}[];
}

/**
 * The login question the window shows, worded as the TUI's overlay words it; nothing typed is in
 * it. A login that ended without the person keeps it, with `ended`, until Close.
 */
export interface TerminalSettingsMachineLogin {
	/** Orchbee's asking sentence: one line, or a new host's lead, whole fingerprint and what to type. */
	readonly asked: readonly string[];
	readonly ended: { readonly succeeded: boolean; readonly text: string } | null;
	readonly kind: 'host-key' | 'other' | 'passphrase' | 'password';
	readonly labels: {
		readonly cancel: string;
		readonly close: string;
		/** Where the answer goes while the remember box is clear; null for a new host. */
		readonly privacy: string | null;
		/** Where the answer goes while the remember box is ticked. */
		readonly rememberPrivacy: string;
		readonly sending: string;
		readonly submit: string;
		readonly title: string;
		readonly typeYes: string;
	};
	readonly login: number;
	/** `{name} · {target}`. */
	readonly machine: string;
	/** Why the question asks again, such as a refused password. */
	readonly notice: string | null;
	readonly question: number;
	/** The remember box beside the field, ticked as the login's earlier answer left it. */
	readonly remember: { readonly label: string; readonly ticked: boolean } | null;
	/** The line in the box's place when answers cannot be remembered in this login. */
	readonly rememberLine: string | null;
	/** `ssh`'s own last lines, each marked as said by `ssh`. */
	readonly said: readonly string[];
	/** The answer is on its way; the next question or the end follows. */
	readonly sending: boolean;
}

/** The person's act on the login question named by its `login` and `question`. */
type TerminalSettingsMachineLoginIntent =
	| {
			readonly act: 'login-answer';
			readonly answer: string;
			readonly login: number;
			readonly question: number;
			/** The remember box, when the question offers one. */
			readonly remember: boolean | null;
	  }
	| {
			readonly act: 'login-cancel' | 'login-close' | 'login-trust';
			readonly login: number;
			readonly question: number;
	  };

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
	  }
	| TerminalSettingsMachineLoginIntent
	| { readonly act: 'add-close' | 'add-open' }
	| { readonly act: 'offer-accept' | 'offer-decline'; readonly login: number }
	| {
			readonly act: 'move';
			/** The machine whose Session holds the Workspace; null for this computer. */
			readonly from: string | null;
			readonly label: string;
			/** Where it goes; null for this computer. */
			readonly to: string | null;
			readonly workspace: string;
	  }
	| { readonly act: 'move-answer'; readonly confirm: boolean; readonly question: number }
	| {
			/** The machine takes its host's name; its page renames it once saved. */
			readonly act: 'add';
			readonly address: string;
			readonly color: TerminalSettingsMachineColor;
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
	/** Settings › File Bar; without it Settings has no File Bar category. */
	readonly fileBarOffered?: boolean | undefined;
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
