import {
	orchbeeTerminalColorTheme,
	resolveTerminalColorTheme,
	terminalColorThemes,
	type TerminalColorTheme,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';
import type {
	SettingChange,
	SettingKey,
	SettingsSnapshot,
} from '@orchbee/workbench-client-protocol';

import { readSetting, settingChange, type UserSettingsControl } from '../../user-settings';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	normalizeTerminalAppearancePreferences,
	normalizeTerminalHexColor,
	resolveTerminalFontSizeChange,
	TERMINAL_FONT_WEIGHT_OPTIONS,
	terminalAppearancePreferencesEqual,
	terminalDerivedPreferencesEqual,
	terminalPresentationConfigurationFromPreferences,
	terminalThemePreferencesEqual,
	type TerminalAppearancePreferences,
	type TerminalAppearancePresentationSource,
	type TerminalFontSizeChange,
} from './terminal-appearance-preferences';
import type {
	TerminalThemePresentationParticipant,
	TerminalThemePresentationPreparation,
	TerminalThemePresentationSource,
} from './isTerminalThemePresentationSource';

/** A Settings section that an opening asks to show first. */
export type TerminalSettingsLanding = 'keyboard-shortcuts';

export interface TerminalSettingsSnapshot {
	/** The section the latest opening asked for; null shows the surface's own choice. */
	readonly landing: TerminalSettingsLanding | null;
	readonly open: boolean;
	readonly preferences: TerminalAppearancePreferences;
}

/** An accepted change is `pending` until the settings file answers with it. */
export type TerminalSettingsUpdateResult =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'pending' }
	| { readonly kind: 'unchanged' };

/** A theme shown in place of its slot's committed theme until it is committed or cancelled. */
export interface TerminalColorThemePreview {
	readonly polarity: TerminalColorTheme['polarity'];
	readonly themeId: string;
}

interface ActiveColorThemePreview extends TerminalColorThemePreview {
	/** A committed pick stays shown until the Session answers, so it never flashes the old theme. */
	readonly committing: boolean;
}

export type TerminalSettingsVisibilityResult =
	| { readonly kind: 'changed' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'unchanged' };

export interface TerminalSettingsControllerOptions {
	/** The themes a stored slot id may name; the shipped themes unless a test supplies a fixture. */
	readonly colorThemes?: readonly TerminalColorTheme[];
	readonly themeTarget: Pick<HTMLElement, 'dataset'>;
	readonly userSettings: UserSettingsControl;
	readonly systemTheme?: Pick<
		MediaQueryList,
		'matches' | 'addEventListener' | 'removeEventListener'
	>;
}

type SnapshotListener = () => void;
type PresentationListener = (configuration: TerminalPresentationConfiguration) => void;

/** The settings key behind each appearance field. */
const APPEARANCE_KEYS = {
	applicationTheme: 'appearance.mode',
	backgroundOpacityPercent: 'terminal.background_opacity',
	customBackground: 'appearance.custom_background',
	customForeground: 'appearance.custom_foreground',
	darkTheme: 'appearance.dark_theme',
	fontFamily: 'terminal.font_family',
	fontLigatures: 'terminal.font_ligatures',
	fontSize: 'terminal.font_size',
	fontWeight: 'terminal.font_weight',
	fontWeightBold: 'terminal.font_weight_bold',
	lightTheme: 'appearance.light_theme',
	terminalDisplayTheme: 'appearance.terminal_colors',
	terminalPadding: 'terminal.padding',
} as const satisfies Record<keyof TerminalAppearancePreferences, SettingKey>;

/** The keys the Appearance category's Reset gives back to their defaults. */
const TERMINAL_APPEARANCE_KEYS = [
	APPEARANCE_KEYS.backgroundOpacityPercent,
	APPEARANCE_KEYS.fontFamily,
	APPEARANCE_KEYS.fontLigatures,
	APPEARANCE_KEYS.fontSize,
	APPEARANCE_KEYS.fontWeight,
	APPEARANCE_KEYS.fontWeightBold,
	APPEARANCE_KEYS.terminalPadding,
] as const;

/**
 * Every appearance field is a key of the settings file on the Server's machine. A change is shown
 * once the file answers with it, so every Client and the TUI see one appearance.
 */
export class TerminalSettingsController
	implements TerminalAppearancePresentationSource, TerminalThemePresentationSource
{
	/** The themes a slot may name, for pickers; the shipped themes unless a test supplies more. */
	readonly colorThemes: readonly TerminalColorTheme[];
	readonly userSettings: UserSettingsControl;
	readonly #presentationListeners = new Set<PresentationListener>();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #removeUserSettings: () => void;
	readonly #systemTheme: TerminalSettingsControllerOptions['systemTheme'];
	readonly #themeTarget: Pick<HTMLElement, 'dataset'>;
	readonly #themeParticipants = new Set<TerminalThemePresentationParticipant>();
	#disposed = false;
	#pendingPresentations: TerminalPresentationConfiguration[] = [];
	#presentationConfiguration: TerminalPresentationConfiguration;
	#publishingPresentation = false;
	/** The size the last font-size step asked for, until the file answers, so steps accumulate. */
	#requestedFontSize: number | null = null;
	#snapshot: TerminalSettingsSnapshot;
	#themePresentationEpoch = 0;
	#themePreview: ActiveColorThemePreview | null = null;

	constructor({
		colorThemes = terminalColorThemes,
		themeTarget,
		userSettings,
		systemTheme,
	}: TerminalSettingsControllerOptions) {
		this.colorThemes = colorThemes;
		this.userSettings = userSettings;
		this.#systemTheme = systemTheme;
		this.#themeTarget = themeTarget;
		const preferences = appearanceFromSettings(userSettings.getSnapshot().snapshot);
		this.#presentationConfiguration = terminalPresentationConfigurationFromPreferences(preferences);
		this.#snapshot = snapshot(false, null, preferences);
		this.#applyTheme(preferences);
		this.#removeUserSettings = userSettings.subscribe(() => this.#syncUserSettings());
		systemTheme?.addEventListener('change', this.#handleSystemTheme);
	}

	current = (): TerminalPresentationConfiguration => this.#presentationConfiguration;

	getSnapshot = (): TerminalSettingsSnapshot => this.#snapshot;

	subscribe = (listener: PresentationListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#presentationListeners.add(listener);
		return () => this.#presentationListeners.delete(listener);
	};

	subscribeSnapshot = (listener: SnapshotListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#snapshotListeners.add(listener);
		return () => this.#snapshotListeners.delete(listener);
	};

	registerThemeParticipant = (participant: TerminalThemePresentationParticipant): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#themeParticipants.add(participant);
		this.#startThemePresentation(this.#snapshot.preferences);
		return () => {
			if (!this.#themeParticipants.delete(participant) || this.#disposed) return;
			this.#startThemePresentation(this.#snapshot.preferences);
		};
	};

	open(landing: TerminalSettingsLanding | null = null): TerminalSettingsVisibilityResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#snapshot.open) return { kind: 'unchanged' };
		this.#snapshot = snapshot(true, landing, this.#snapshot.preferences);
		this.#emitSnapshot();
		return { kind: 'changed' };
	}

	close(): TerminalSettingsVisibilityResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (!this.#snapshot.open) return { kind: 'unchanged' };
		if (this.#themePreview?.committing === false) this.previewColorTheme(null);
		this.#snapshot = snapshot(false, null, this.#snapshot.preferences);
		this.#emitSnapshot();
		return { kind: 'changed' };
	}

	/** One edit of every field `patch` changes. */
	update(patch: Partial<TerminalAppearancePreferences>): TerminalSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const current = this.#snapshot.preferences;
		const candidate = normalizeTerminalAppearancePreferences({ ...current, ...patch });
		if (candidate === null) return { kind: 'invalid' };
		const changes = appearanceChanges(current, candidate);
		if (changes.length === 0) return { kind: 'unchanged' };
		void this.userSettings.edit(changes);
		return this.#disposed ? { kind: 'disposed' } : { kind: 'pending' };
	}

	/**
	 * Shows a theme in place of its slot's committed theme, at its own polarity, without saving
	 * anything; null restores the committed presentation.
	 */
	previewColorTheme(preview: TerminalColorThemePreview | null): void {
		if (this.#disposed) return;
		const current = this.#themePreview;
		if (preview === null ? current === null : sameColorThemePreview(current, preview)) return;
		const before = this.#presented(this.#snapshot.preferences);
		this.#themePreview =
			preview === null
				? null
				: { committing: false, polarity: preview.polarity, themeId: preview.themeId };
		this.#presentIfChanged(before);
	}

	/**
	 * Picking a theme sets the slot of its polarity. When Appearance is fixed to the other polarity,
	 * the pick switches Appearance too, so it shows at once; under System it applies when the system
	 * matches.
	 */
	selectColorTheme(polarity: TerminalColorTheme['polarity'], themeId: string): void {
		if (this.#disposed) return;
		const { applicationTheme } = this.#snapshot.preferences;
		const changes: SettingChange[] = [
			{
				key: polarity === 'dark' ? APPEARANCE_KEYS.darkTheme : APPEARANCE_KEYS.lightTheme,
				value: themeId,
			},
		];
		if (applicationTheme !== 'system' && applicationTheme !== polarity) {
			changes.push({ key: APPEARANCE_KEYS.applicationTheme, value: polarity });
		}
		const before = this.#presented(this.#snapshot.preferences);
		const preview: ActiveColorThemePreview = { committing: true, polarity, themeId };
		this.#themePreview = preview;
		this.#presentIfChanged(before);
		const settle = (): void => {
			if (this.#disposed || this.#themePreview !== preview) return;
			this.previewColorTheme(null);
		};
		void this.userSettings.edit(changes).then(settle, settle);
	}

	/** Gives the terminal's font, padding and opacity back to their defaults in one edit. */
	resetTerminalAppearance(): TerminalSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		void this.userSettings.restore(TERMINAL_APPEARANCE_KEYS);
		return { kind: 'pending' };
	}

	changeFontSize(change: TerminalFontSizeChange): TerminalSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const base = this.#requestedFontSize ?? this.#snapshot.preferences.fontSize;
		const fontSize = resolveTerminalFontSizeChange(base, change);
		if (fontSize === base) return { kind: 'unchanged' };
		this.#requestedFontSize = fontSize;
		const settle = (): void => {
			if (this.#requestedFontSize === fontSize) this.#requestedFontSize = null;
		};
		void this.userSettings
			.edit([
				fontSize === DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontSize
					? { key: APPEARANCE_KEYS.fontSize, restore: true }
					: { key: APPEARANCE_KEYS.fontSize, value: fontSize },
			])
			.then(settle, settle);
		return { kind: 'pending' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserSettings();
		this.#systemTheme?.removeEventListener('change', this.#handleSystemTheme);
		this.#themePresentationEpoch += 1;
		this.#pendingPresentations = [];
		this.#presentationListeners.clear();
		this.#snapshotListeners.clear();
		this.#themeParticipants.clear();
	}

	/** The polarity Appearance resolves to now: System follows the operating system. */
	#polarity(preferences: TerminalAppearancePreferences): 'dark' | 'light' {
		if (preferences.applicationTheme !== 'system') return preferences.applicationTheme;
		return this.#systemTheme?.matches === true ? 'dark' : 'light';
	}

	/** The active slot's theme; an id this build does not ship resolves to the slot's Orchbee theme. */
	#colorTheme(preferences: TerminalAppearancePreferences): TerminalColorTheme {
		const polarity = this.#polarity(preferences);
		return resolveTerminalColorTheme(
			polarity,
			polarity === 'dark' ? preferences.darkTheme : preferences.lightTheme,
			this.colorThemes,
		);
	}

	/** The preferences presented now: the committed ones with any previewed theme in its slot. */
	#presented(preferences: TerminalAppearancePreferences): TerminalAppearancePreferences {
		const preview = this.#themePreview;
		if (preview === null) return preferences;
		return {
			...preferences,
			applicationTheme: preview.polarity,
			[preview.polarity === 'dark' ? 'darkTheme' : 'lightTheme']: preview.themeId,
		};
	}

	#presentIfChanged(before: TerminalAppearancePreferences): void {
		const after = this.#presented(this.#snapshot.preferences);
		if (
			this.#polarity(before) === this.#polarity(after) &&
			this.#colorTheme(before).id === this.#colorTheme(after).id
		) {
			return;
		}
		this.#startThemePresentation(this.#snapshot.preferences);
	}

	/**
	 * `data-theme` keeps its polarity meaning; a named theme adds `data-color-theme` on the same
	 * element, and the Orchbee pair sets none, so the hand-authored mapping applies.
	 */
	#applyTheme(preferences: TerminalAppearancePreferences): void {
		const polarity = this.#polarity(preferences);
		const theme = this.#colorTheme(preferences);
		const { dataset } = this.#themeTarget;
		dataset['theme'] = polarity;
		if (theme.id === orchbeeTerminalColorTheme(polarity).id) delete dataset['colorTheme'];
		else dataset['colorTheme'] = theme.id;
	}

	readonly #handleSystemTheme = (): void => {
		if (this.#snapshot.preferences.applicationTheme === 'system')
			this.#startThemePresentation(this.#snapshot.preferences);
	};

	#syncUserSettings(): void {
		if (this.#disposed) return;
		const current = this.#snapshot.preferences;
		const preferences = appearanceFromSettings(this.userSettings.getSnapshot().snapshot);
		if (terminalAppearancePreferencesEqual(preferences, current)) return;
		const terminalChanged = !terminalDerivedPreferencesEqual(current, preferences);
		this.#snapshot = snapshot(this.#snapshot.open, this.#snapshot.landing, preferences);
		if (!terminalThemePreferencesEqual(current, preferences)) {
			this.#startThemePresentation(preferences);
		}
		if (terminalChanged) {
			this.#presentationConfiguration =
				terminalPresentationConfigurationFromPreferences(preferences);
			this.#emitPresentation(this.#presentationConfiguration);
		}
		this.#emitSnapshot();
	}

	#startThemePresentation(committed: TerminalAppearancePreferences): void {
		const preferences = this.#presented(committed);
		const applicationTheme = this.#polarity(preferences);
		const colorTheme = this.#colorTheme(preferences);
		const epoch = ++this.#themePresentationEpoch;
		const participants = [...this.#themeParticipants];
		if (participants.length === 0) {
			this.#applyTheme(preferences);
			return;
		}
		void Promise.all(
			participants.map(async (participant): Promise<TerminalThemePresentationPreparation> => {
				try {
					return await participant.prepareThemePresentation({
						...preferences,
						applicationTheme,
						colorTheme,
					});
				} catch {
					return { code: 'theme-preparation-rejected', kind: 'failed', retryable: true };
				}
			}),
		).then((results) => {
			const prepared = results.flatMap((result) =>
				result.kind === 'prepared' ? [result.commit] : [],
			);
			if (
				this.#disposed ||
				epoch !== this.#themePresentationEpoch ||
				prepared.length !== participants.length ||
				!prepared.every((commit) => commit.canCommit())
			) {
				for (const commit of prepared) commit.abort();
				return;
			}
			for (const commit of prepared) commit.commit();
			this.#applyTheme(preferences);
		});
	}

	#emitPresentation(configuration: TerminalPresentationConfiguration): void {
		if (this.#publishingPresentation) {
			this.#pendingPresentations.push(configuration);
			return;
		}
		this.#publishingPresentation = true;
		let next: TerminalPresentationConfiguration | undefined = configuration;
		try {
			while (next !== undefined && !this.#disposed) {
				const listeners = new Set(this.#presentationListeners);
				for (const listener of listeners) {
					if (this.#disposed) break;
					try {
						listener(next);
					} catch {
						// One presentation consumer cannot prevent the remaining stable subscribers.
					}
				}
				next = this.#pendingPresentations.shift();
			}
		} finally {
			this.#publishingPresentation = false;
		}
	}

	#emitSnapshot(): void {
		const listeners = new Set(this.#snapshotListeners);
		for (const listener of listeners) {
			if (this.#disposed) break;
			try {
				listener();
			} catch {
				// External-store listeners are isolated from each other.
			}
		}
	}
}

function sameColorThemePreview(
	current: ActiveColorThemePreview | null,
	next: TerminalColorThemePreview,
): boolean {
	return (
		current !== null &&
		!current.committing &&
		current.polarity === next.polarity &&
		current.themeId === next.themeId
	);
}

function snapshot(
	open: boolean,
	landing: TerminalSettingsLanding | null,
	preferences: TerminalAppearancePreferences,
): TerminalSettingsSnapshot {
	return Object.freeze({ landing, open, preferences });
}

function fontWeight(value: number): TerminalAppearancePreferences['fontWeight'] | null {
	return TERMINAL_FONT_WEIGHT_OPTIONS.find((weight) => weight === value) ?? null;
}

/** The appearance a settings snapshot gives; before the first one, the defaults. */
function appearanceFromSettings(settings: SettingsSnapshot | null): TerminalAppearancePreferences {
	const defaults = DEFAULT_TERMINAL_APPEARANCE_PREFERENCES;
	const read = <Key extends SettingKey>(key: Key) => readSetting(settings, key);
	return Object.freeze({
		applicationTheme: read(APPEARANCE_KEYS.applicationTheme),
		backgroundOpacityPercent: read(APPEARANCE_KEYS.backgroundOpacityPercent),
		customBackground:
			normalizeTerminalHexColor(read(APPEARANCE_KEYS.customBackground)) ??
			defaults.customBackground,
		customForeground:
			normalizeTerminalHexColor(read(APPEARANCE_KEYS.customForeground)) ??
			defaults.customForeground,
		darkTheme: read(APPEARANCE_KEYS.darkTheme),
		fontFamily: read(APPEARANCE_KEYS.fontFamily)?.trim() || defaults.fontFamily,
		fontLigatures: read(APPEARANCE_KEYS.fontLigatures),
		fontSize: read(APPEARANCE_KEYS.fontSize),
		fontWeight: fontWeight(read(APPEARANCE_KEYS.fontWeight)) ?? defaults.fontWeight,
		fontWeightBold: fontWeight(read(APPEARANCE_KEYS.fontWeightBold)) ?? defaults.fontWeightBold,
		lightTheme: read(APPEARANCE_KEYS.lightTheme),
		terminalDisplayTheme: read(APPEARANCE_KEYS.terminalDisplayTheme),
		terminalPadding: read(APPEARANCE_KEYS.terminalPadding),
	});
}

/**
 * The changes that turn `current` into `candidate`, field by field. The product font is the
 * default of `terminal.font_family`, so choosing it restores the key.
 */
function appearanceChanges(
	current: TerminalAppearancePreferences,
	candidate: TerminalAppearancePreferences,
): SettingChange[] {
	const changes: SettingChange[] = [];
	for (const field of Object.keys(APPEARANCE_KEYS)) {
		if (!isAppearanceField(field) || current[field] === candidate[field]) continue;
		if (field === 'fontFamily') {
			changes.push(
				settingChange(
					APPEARANCE_KEYS.fontFamily,
					candidate.fontFamily === DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontFamily
						? null
						: candidate.fontFamily,
				),
			);
			continue;
		}
		changes.push({ key: APPEARANCE_KEYS[field], value: candidate[field] });
	}
	return changes;
}

function isAppearanceField(field: string): field is keyof TerminalAppearancePreferences {
	return Object.hasOwn(APPEARANCE_KEYS, field);
}
