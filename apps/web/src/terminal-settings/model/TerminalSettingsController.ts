import {
	orchbeeTerminalColorTheme,
	resolveTerminalColorTheme,
	terminalColorThemes,
	type TerminalColorTheme,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';
import {
	DEFAULT_USER_PREFERENCES,
	type UserPreferencesPatch,
} from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl } from '../../user-preferences';

import { PreferenceWriteQueue } from '../../PreferenceWriteQueue';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	resolveTerminalFontSizeChange,
	terminalAppearancePreferencesEqual,
	terminalDerivedPreferencesEqual,
	terminalPresentationConfigurationFromPreferences,
	terminalThemePreferencesEqual,
	normalizeTerminalHexColor,
	type TerminalAppearancePreferences,
	type TerminalFontSizeChange,
	type TerminalAppearancePresentationSource,
} from './terminal-appearance-preferences';
import {
	normalizeTerminalAppearancePreferences,
	readHostAppearancePreferences,
	writeHostAppearancePreferences,
	type TerminalAppearancePersistenceIssue,
	type TerminalAppearanceStorage,
} from './terminal-appearance-preferences-storage';
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
	readonly persistenceIssue: TerminalAppearancePersistenceIssue | null;
	readonly preferences: TerminalAppearancePreferences;
}

export type TerminalSettingsUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
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
	readonly storage: TerminalAppearanceStorage;
	readonly themeTarget: Pick<HTMLElement, 'dataset'>;
	readonly userPreferences: UserPreferencesControl;
	readonly systemTheme?: Pick<
		MediaQueryList,
		'matches' | 'addEventListener' | 'removeEventListener'
	>;
}

type SnapshotListener = () => void;
type PresentationListener = (configuration: TerminalPresentationConfiguration) => void;

/**
 * Six common appearance leaves follow the Session's shared preferences; the seven host leaves stay in
 * this renderer's browser storage.
 */
export class TerminalSettingsController
	implements TerminalAppearancePresentationSource, TerminalThemePresentationSource
{
	/** The themes a slot may name, for pickers; the shipped themes unless a test supplies more. */
	readonly colorThemes: readonly TerminalColorTheme[];
	readonly #presentationListeners = new Set<PresentationListener>();
	readonly #persistence = new PreferenceWriteQueue();
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #storage: TerminalAppearanceStorage;
	readonly userPreferences: UserPreferencesControl;
	readonly #removeUserPreferences: () => void;
	readonly #systemTheme: TerminalSettingsControllerOptions['systemTheme'];
	readonly #themeTarget: Pick<HTMLElement, 'dataset'>;
	readonly #themeParticipants = new Set<TerminalThemePresentationParticipant>();
	#disposed = false;
	#pendingPresentations: TerminalPresentationConfiguration[] = [];
	#presentationConfiguration: TerminalPresentationConfiguration;
	#publishingPresentation = false;
	#revision = 0;
	#snapshot: TerminalSettingsSnapshot;
	#themePresentationEpoch = 0;
	#themePreview: ActiveColorThemePreview | null = null;

	constructor({
		colorThemes = terminalColorThemes,
		storage,
		themeTarget,
		userPreferences,
		systemTheme,
	}: TerminalSettingsControllerOptions) {
		this.colorThemes = colorThemes;
		this.#storage = storage;
		this.userPreferences = userPreferences;
		this.#systemTheme = systemTheme;
		this.#themeTarget = themeTarget;
		const restored = readHostAppearancePreferences(storage);
		const preferences = commonAppearance(restored.preferences, userPreferences);
		this.#presentationConfiguration = terminalPresentationConfigurationFromPreferences(preferences);
		this.#snapshot = snapshot(false, null, restored.issue, preferences);
		this.#applyTheme(preferences);
		this.#removeUserPreferences = userPreferences.subscribe(() => this.#syncUserPreferences());
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
		this.#snapshot = snapshot(
			true,
			landing,
			this.#snapshot.persistenceIssue,
			this.#snapshot.preferences,
		);
		this.#emitSnapshot();
		return { kind: 'changed' };
	}

	close(): TerminalSettingsVisibilityResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (!this.#snapshot.open) return { kind: 'unchanged' };
		if (this.#themePreview?.committing === false) this.previewColorTheme(null);
		this.#snapshot = snapshot(
			false,
			null,
			this.#snapshot.persistenceIssue,
			this.#snapshot.preferences,
		);
		this.#emitSnapshot();
		return { kind: 'changed' };
	}

	update(patch: Partial<TerminalAppearancePreferences>): TerminalSettingsUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const currentPreferences = this.#snapshot.preferences;
		let candidate = normalizeTerminalAppearancePreferences({ ...currentPreferences, ...patch });
		if (candidate === null) return { kind: 'invalid' };
		const changes: UserPreferencesPatch = {
			...(patch.applicationTheme === undefined
				? {}
				: { applicationTheme: candidate.applicationTheme }),
			...(patch.lightTheme === undefined ? {} : { lightTheme: candidate.lightTheme }),
			...(patch.darkTheme === undefined ? {} : { darkTheme: candidate.darkTheme }),
			...(patch.terminalDisplayTheme === undefined
				? {}
				: { terminalDisplayTheme: candidate.terminalDisplayTheme }),
			...(patch.customForeground === undefined
				? {}
				: { customForeground: candidate.customForeground }),
			...(patch.customBackground === undefined
				? {}
				: { customBackground: candidate.customBackground }),
		};
		const commonPending = Object.keys(changes).length !== 0;
		if (commonPending) {
			const revision = this.#revision;
			void this.userPreferences.update(changes);
			// Shared admission publishes synchronously and can retire or replace this host edit.
			if (this.#disposed) return { kind: 'disposed' };
			if (this.#revision !== revision) return { kind: 'pending' };
		}
		candidate = commonAppearance(candidate, this.userPreferences);

		const preferencesChanged = !terminalAppearancePreferencesEqual(currentPreferences, candidate);
		if (!preferencesChanged && this.#snapshot.persistenceIssue === null) {
			return { kind: commonPending ? 'pending' : 'unchanged' };
		}

		const terminalChanged = !terminalDerivedPreferencesEqual(currentPreferences, candidate);
		const themeChanged = !terminalThemePreferencesEqual(currentPreferences, candidate);
		const revision = this.#commitCandidate(candidate, terminalChanged);
		if (themeChanged) this.#startThemePresentation(candidate);
		if (terminalChanged) {
			this.#emitPresentation(this.#presentationConfiguration);
		}
		if (this.#disposed) return { kind: 'disposed' };
		if (this.#revision === revision) this.#persist(candidate, revision);
		if (this.#disposed) return { kind: 'disposed' };
		return commonPending ? { kind: 'pending' } : this.#currentUpdateResult();
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
		const slot = polarity === 'dark' ? 'darkTheme' : 'lightTheme';
		const { applicationTheme } = this.#snapshot.preferences;
		const changes: UserPreferencesPatch = {
			[slot]: themeId,
			...(applicationTheme === 'system' || applicationTheme === polarity
				? {}
				: { applicationTheme: polarity }),
		};
		const before = this.#presented(this.#snapshot.preferences);
		const preview: ActiveColorThemePreview = { committing: true, polarity, themeId };
		this.#themePreview = preview;
		this.#presentIfChanged(before);
		const settle = (): void => {
			if (this.#disposed || this.#themePreview !== preview) return;
			this.previewColorTheme(null);
		};
		void this.userPreferences.update(changes).then(settle, settle);
	}

	/** Settings resets the shared appearance keys itself, together with the interaction keys. */
	resetHostAppearance(): TerminalSettingsUpdateResult {
		const {
			applicationTheme: _applicationTheme,
			darkTheme: _darkTheme,
			lightTheme: _lightTheme,
			terminalDisplayTheme: _terminalDisplayTheme,
			customForeground: _customForeground,
			customBackground: _customBackground,
			...host
		} = DEFAULT_TERMINAL_APPEARANCE_PREFERENCES;
		return this.update(host);
	}

	changeFontSize(change: TerminalFontSizeChange): TerminalSettingsUpdateResult {
		return this.update({
			fontSize: resolveTerminalFontSizeChange(this.#snapshot.preferences.fontSize, change),
		});
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserPreferences();
		this.#systemTheme?.removeEventListener('change', this.#handleSystemTheme);
		this.#revision += 1;
		this.#themePresentationEpoch += 1;
		this.#persistence.dispose();
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

	/**
	 * `data-theme` keeps its polarity meaning; a named theme adds `data-color-theme` on the same
	 * element, and the Orchbee pair sets none, so the hand-authored mapping applies.
	 */
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

	#syncUserPreferences(): void {
		if (this.#disposed) return;
		const preferences = commonAppearance(this.#snapshot.preferences, this.userPreferences);
		if (terminalAppearancePreferencesEqual(preferences, this.#snapshot.preferences)) return;
		this.#commitCandidate(preferences, false);
		this.#startThemePresentation(preferences);
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

	#commitCandidate(preferences: TerminalAppearancePreferences, terminalChanged: boolean): number {
		this.#revision += 1;
		this.#snapshot = snapshot(
			this.#snapshot.open,
			this.#snapshot.landing,
			this.#snapshot.persistenceIssue,
			preferences,
		);
		if (terminalChanged) {
			this.#presentationConfiguration =
				terminalPresentationConfigurationFromPreferences(preferences);
		}
		return this.#revision;
	}

	#currentUpdateResult(): TerminalSettingsUpdateResult {
		return this.#snapshot.persistenceIssue === null && !this.#persistence.pending
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
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

	#persist(preferences: TerminalAppearancePreferences, revision: number): void {
		this.#persistence.write(() => {
			const saved = writeHostAppearancePreferences(this.#storage, preferences);
			if (this.#disposed || this.#revision !== revision) return;
			this.#snapshot = snapshot(
				this.#snapshot.open,
				this.#snapshot.landing,
				saved ? null : 'write-failed',
				preferences,
			);
			this.#emitSnapshot();
		});
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
	persistenceIssue: TerminalAppearancePersistenceIssue | null,
	preferences: TerminalAppearancePreferences,
): TerminalSettingsSnapshot {
	return Object.freeze({ landing, open, persistenceIssue, preferences });
}

function commonAppearance(
	host: TerminalAppearancePreferences,
	control: UserPreferencesControl,
): TerminalAppearancePreferences {
	const values = control.getSnapshot().snapshot?.values ?? DEFAULT_USER_PREFERENCES;
	return {
		...host,
		applicationTheme: values.applicationTheme,
		darkTheme: values.darkTheme,
		lightTheme: values.lightTheme,
		terminalDisplayTheme: values.terminalDisplayTheme,
		customForeground:
			normalizeTerminalHexColor(values.customForeground) ??
			DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.customForeground,
		customBackground:
			normalizeTerminalHexColor(values.customBackground) ??
			DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.customBackground,
	};
}
