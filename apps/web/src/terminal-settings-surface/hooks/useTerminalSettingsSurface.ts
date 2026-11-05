import { formatKeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';
import { useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	useKeyboardShortcutSettings,
	type KeyboardShortcutId,
} from '../../keyboard-shortcuts';
import { useTerminalInteractionSettings } from '../../terminal-interaction-settings';
import type { TerminalDefaultSettingsClientSnapshot } from '../../terminal-default-settings-control';
import { normalizeTerminalHexColor, useTerminalSettings } from '../../terminal-settings';
import { TERMINAL_SETTINGS_SEARCH_RESULTS_ID } from '../constants/terminal-settings-search-catalog';
import {
	TERMINAL_SETTINGS_CATEGORIES,
	TERMINAL_SETTINGS_NAVIGATION_GROUPS,
} from '../constants/terminal-settings-information-architecture';
import type {
	TerminalSettingsCategory,
	TerminalSettingsSurfaceView,
	UseTerminalSettingsSurfaceOptions,
} from '../types/terminal-settings-surface';
import { buildColorThemePickerViews } from '../utils/buildColorThemePickerViews';
import { buildTerminalSettingsSearchCatalog } from '../utils/buildTerminalSettingsSearchCatalog';
import { buildTerminalSettingsSurfaceMessages } from '../utils/buildTerminalSettingsSurfaceMessages';
import { resolveTerminalSettingsSearch } from '../utils/resolveTerminalSettingsSearch';
import { useSharedPreferenceDraft } from './useSharedPreferenceDraft';
import {
	parseScrollbackBytes,
	parseTerminalFontWeight,
	parseTerminalPadding,
} from '../utils/terminal-settings-draft-parsing';

const UNAVAILABLE_TERMINAL_DEFAULT_SETTINGS_SNAPSHOT: TerminalDefaultSettingsClientSnapshot = {
	configuration: { scrollbackBytes: 10_000_000 },
	failureCode: 'control-unavailable',
};

export function useTerminalSettingsSurface({
	agentIntegrationManagementControl,
	controller,
	keyboardShortcutController,
	keyboardShortcutPlatform,
	macosTerminalInputSettingsController,
	onRequestClose,

	terminalFeedbackSettingsController,
	terminalDefaultSettingsControl,
	terminalInteractionSettingsController,
	terminalLaunchProfileClient,
}: UseTerminalSettingsSurfaceOptions): TerminalSettingsSurfaceView {
	const intl = useOrchbeeIntl();
	const snapshot = useTerminalSettings(controller);
	const interactionSnapshot = useTerminalInteractionSettings(terminalInteractionSettingsController);
	const unavailableTerminalDefaultSettingsSource = React.useMemo(
		() => ({
			getSnapshot: (): TerminalDefaultSettingsClientSnapshot =>
				UNAVAILABLE_TERMINAL_DEFAULT_SETTINGS_SNAPSHOT,
			subscribe: (): (() => void) => () => undefined,
		}),
		[],
	);
	const terminalDefaultSettingsSource =
		terminalDefaultSettingsControl ?? unavailableTerminalDefaultSettingsSource;
	const terminalDefaultSettingsSnapshot = React.useSyncExternalStore(
		terminalDefaultSettingsSource.subscribe,
		terminalDefaultSettingsSource.getSnapshot,
		terminalDefaultSettingsSource.getSnapshot,
	);
	const keyboardShortcutSnapshot = useKeyboardShortcutSettings(keyboardShortcutController);
	const availableCategories = new Set<TerminalSettingsCategory>([
		'appearance',
		'colors',
		'general',
		'keyboard-shortcuts',
		'remote-development',
		'text',
	]);
	if (agentIntegrationManagementControl !== undefined) {
		availableCategories.add('agent-integrations');
	}

	if (macosTerminalInputSettingsController !== undefined) {
		availableCategories.add('cursor-input');
	}
	if (terminalInteractionSettingsController !== undefined) {
		availableCategories.add('interaction');
		availableCategories.add('terminal');
	}
	if (terminalDefaultSettingsControl !== undefined) availableCategories.add('terminal');
	if (terminalLaunchProfileClient !== undefined) availableCategories.add('profiles');

	if (terminalFeedbackSettingsController !== undefined) {
		availableCategories.add('shell-integration');
	}
	const inputReference = React.useRef<HTMLInputElement>(null);
	const contentReference = React.useRef<HTMLElement>(null);
	const [category, setCategory] = React.useState<TerminalSettingsCategory>('general');
	const [navigationOpen, setNavigationOpen] = React.useState(true);
	const [query, setQuery] = React.useState('');
	const [activeSearchResultIndex, setActiveSearchResultIndex] = React.useState(0);
	const [pendingSearchTargetId, setPendingSearchTargetId] = React.useState<
		string | null | undefined
	>(undefined);
	const [fontFamilyDraft, setFontFamilyDraft] = React.useState(snapshot.preferences.fontFamily);
	const [fontFamilyInvalid, setFontFamilyInvalid] = React.useState(false);
	const [fontSizeDraft, setFontSizeDraft] = React.useState(String(snapshot.preferences.fontSize));
	const [fontSizeInvalid, setFontSizeInvalid] = React.useState(false);
	const [terminalPaddingDraft, setTerminalPaddingDraft] = React.useState(
		String(snapshot.preferences.terminalPadding),
	);
	const [terminalPaddingInvalid, setTerminalPaddingInvalid] = React.useState(false);
	const scrollback = useSharedPreferenceDraft(
		String(terminalDefaultSettingsSnapshot.configuration.scrollbackBytes),
		'scrollbackBytes',
		controller.userPreferences,
	);
	const scrollbackBytesDraft = scrollback.draft;
	const [scrollbackBytesInvalid, setScrollbackBytesInvalid] = React.useState(false);
	const foreground = useSharedPreferenceDraft(
		snapshot.preferences.customForeground,
		'customForeground',
		controller.userPreferences,
	);
	const foregroundDraft = foreground.draft;
	const [foregroundInvalid, setForegroundInvalid] = React.useState(false);
	const background = useSharedPreferenceDraft(
		snapshot.preferences.customBackground,
		'customBackground',
		controller.userPreferences,
	);
	const backgroundDraft = background.draft;
	const [backgroundInvalid, setBackgroundInvalid] = React.useState(false);

	React.useEffect(() => {
		setFontFamilyDraft(snapshot.preferences.fontFamily);
		setFontFamilyInvalid(false);
		setFontSizeDraft(String(snapshot.preferences.fontSize));
		setFontSizeInvalid(false);
		setTerminalPaddingDraft(String(snapshot.preferences.terminalPadding));
		setTerminalPaddingInvalid(false);
		setForegroundInvalid(false);
		setBackgroundInvalid(false);
	}, [snapshot.preferences]);

	React.useEffect(() => {
		setScrollbackBytesInvalid(false);
	}, [terminalDefaultSettingsSnapshot.configuration.scrollbackBytes]);

	React.useEffect(() => {
		if (!snapshot.open) return;
		if (snapshot.landing !== null) setCategory(snapshot.landing);
		setQuery('');
		setActiveSearchResultIndex(0);
		setPendingSearchTargetId(undefined);
		setNavigationOpen(true);
		inputReference.current?.focus();
	}, [snapshot.landing, snapshot.open]);
	React.useLayoutEffect(() => {
		if (!snapshot.open || !navigationOpen) return;
		inputReference.current?.focus();
	}, [navigationOpen, snapshot.open]);

	React.useEffect(() => {
		if (pendingSearchTargetId === undefined) return;
		const content = contentReference.current;
		if (content === null) return;
		if (pendingSearchTargetId === null) {
			content.scrollTop = 0;
			document.getElementById('terminal-settings-title')?.focus({ preventScroll: true });
		} else {
			const target = document.getElementById(pendingSearchTargetId);
			if (target !== null && content.contains(target)) {
				target.scrollIntoView({ block: 'center' });
				target.focus({ preventScroll: true });
			} else {
				content.focus({ preventScroll: true });
			}
		}
		setPendingSearchTargetId(undefined);
	}, [category, pendingSearchTargetId]);

	const messages = buildTerminalSettingsSurfaceMessages((descriptor, values) =>
		intl.formatMessage(descriptor, values),
	);
	const applicationThemeVisible = true;
	const terminalPaddingVisible = true;
	const fontFamilyVisible = true;
	const fontLigaturesVisible = true;
	const fontSizeVisible = true;
	const fontWeightVisible = true;
	const fontWeightBoldVisible = true;
	const displayThemeVisible = true;
	const foregroundVisible = true;
	const backgroundVisible = true;
	const opacityVisible = true;
	const resetVisible = true;
	const macosInputVisible = true;
	const copyOnSelectVisible = terminalInteractionSettingsController !== undefined;
	const trimTrailingBlankRowsForAgentsVisible = terminalInteractionSettingsController !== undefined;
	const scrollbackBytesVisible = terminalDefaultSettingsControl !== undefined;
	const navigationGroups = TERMINAL_SETTINGS_NAVIGATION_GROUPS.map((group) => ({
		id: group.id,
		items: group.categories
			.filter((categoryId) => availableCategories.has(categoryId))
			.map((categoryId) => ({
				icon: TERMINAL_SETTINGS_CATEGORIES[categoryId].icon,
				id: categoryId,
				label: intl.formatMessage(TERMINAL_SETTINGS_CATEGORIES[categoryId].label),
			})),
		label: intl.formatMessage(group.label),
	})).filter((group) => group.items.length > 0);
	const enabledCategories = navigationGroups.flatMap((group) => group.items.map((item) => item.id));
	const keyboardShortcutChords: Partial<Record<KeyboardShortcutId, string>> = {};
	for (const declaration of KEYBOARD_SHORTCUT_CATALOG) {
		keyboardShortcutChords[declaration.shortcutId] = formatKeyboardShortcutChord(
			keyboardShortcutSnapshot.bindings[declaration.shortcutId],
			keyboardShortcutPlatform,
		);
	}
	const enabledCapabilities = [
		...(agentIntegrationManagementControl === undefined
			? []
			: (['desktop-agent-integrations'] as const)),
		...(macosTerminalInputSettingsController === undefined
			? []
			: (['macos-option-as-alt'] as const)),
		...(terminalLaunchProfileClient === undefined ? [] : (['desktop-launch-profiles'] as const)),
		...(terminalDefaultSettingsControl === undefined
			? []
			: (['desktop-terminal-default-settings'] as const)),
		...(terminalInteractionSettingsController === undefined
			? []
			: (['terminal-interaction'] as const)),
	];
	const searchCatalog = buildTerminalSettingsSearchCatalog({
		enabledCapabilities,
		enabledCategories,
		formatMessage: (descriptor, values) => intl.formatMessage(descriptor, values),
		keyboardShortcutChords,
	});
	const resolvedSearchResults = resolveTerminalSettingsSearch({
		entries: searchCatalog,
		locale: intl.locale,
		query,
	});
	const boundedSearchResultIndex = Math.min(
		activeSearchResultIndex,
		Math.max(resolvedSearchResults.length - 1, 0),
	);
	const activeSearchEntry = resolvedSearchResults[boundedSearchResultIndex];
	const activeSearchResultId =
		activeSearchEntry === undefined
			? undefined
			: `terminal-settings-search-result-${activeSearchEntry.id}`;
	React.useEffect(() => {
		if (activeSearchResultId === undefined) return;
		document.getElementById(activeSearchResultId)?.scrollIntoView({ block: 'nearest' });
	}, [activeSearchResultId]);
	const activateSearchEntry = (resultIndex: number): void => {
		const entry = resolvedSearchResults[resultIndex];
		if (entry === undefined) return;
		setCategory(entry.category);
		setNavigationOpen(false);
		setQuery('');
		setActiveSearchResultIndex(0);
		setPendingSearchTargetId(entry.targetId);
	};
	const searchResults = resolvedSearchResults.map((entry, resultIndex) => ({
		...entry,
		active: resultIndex === boundedSearchResultIndex,
		elementId: `terminal-settings-search-result-${entry.id}`,
		onActivate: (): void => activateSearchEntry(resultIndex),
		onFocus: (): void => setActiveSearchResultIndex(resultIndex),
	}));
	const searchActive = query.trim().length > 0;
	const pageDefinition = TERMINAL_SETTINGS_CATEGORIES[category];
	const commitFontFamily = (): void => {
		const result = controller.update({ fontFamily: fontFamilyDraft });
		setFontFamilyInvalid(result.kind === 'invalid');
	};
	const commitFontSize = (): void => {
		const result = controller.update({ fontSize: Number(fontSizeDraft) });
		setFontSizeInvalid(result.kind === 'invalid');
	};
	const commitTerminalPadding = (): void => {
		const terminalPadding = parseTerminalPadding(terminalPaddingDraft);
		if (terminalPadding === null) {
			setTerminalPaddingInvalid(true);
			return;
		}
		const result = controller.update({ terminalPadding });
		setTerminalPaddingInvalid(result.kind === 'invalid');
	};
	const commitScrollbackBytes = (): void => {
		const scrollbackBytes = parseScrollbackBytes(scrollbackBytesDraft);
		if (scrollbackBytes === null) {
			setScrollbackBytesInvalid(true);
			return;
		}
		setScrollbackBytesInvalid(false);
		scrollback.submit({ scrollbackBytes });
	};
	const commitForeground = (): void => {
		const normalized = normalizeTerminalHexColor(foregroundDraft);
		if (normalized === null) {
			setForegroundInvalid(true);
			return;
		}
		setForegroundInvalid(false);
		foreground.submit({ customForeground: normalized });
	};
	const commitBackground = (): void => {
		const normalized = normalizeTerminalHexColor(backgroundDraft);
		if (normalized === null) {
			setBackgroundInvalid(true);
			return;
		}
		setBackgroundInvalid(false);
		background.submit({ customBackground: normalized });
	};
	let persistenceWarning: string | null = null;
	switch (snapshot.persistenceIssue) {
		case 'invalid-record':
			persistenceWarning = messages.persistenceInvalidWarning;
			break;
		case 'read-failed':
			persistenceWarning = messages.persistenceReadWarning;
			break;
		case 'write-failed':
			persistenceWarning = messages.persistenceWriteWarning;
			break;
	}
	// Save failures show in the shared status; only a missing preference snapshot is reported here.
	const terminalDefaultSettingsWarning =
		terminalDefaultSettingsControl !== undefined &&
		terminalDefaultSettingsSnapshot.failureCode === 'control-unavailable'
			? messages.scrollbackBytesControlUnavailableWarning
			: null;

	const colorThemePickers = buildColorThemePickerViews({
		colorThemes: controller.colorThemes,
		formatMessage: (descriptor) => intl.formatMessage(descriptor),
		preferences: snapshot.preferences,
		onCommit: (polarity, themeId) => controller.selectColorTheme(polarity, themeId),
		onPreview: (polarity, themeId) =>
			controller.previewColorTheme(themeId === null ? null : { polarity, themeId }),
	});

	return {
		activeSearchResultId,
		applicationThemeVisible,
		backgroundDraft,
		backgroundInvalid,
		backgroundVisible,
		category,
		colorThemePickers,
		contentReference,
		copyOnSelect: interactionSnapshot.preferences.copyOnSelect,
		copyOnSelectVisible,
		customColorsEnabled: snapshot.preferences.terminalDisplayTheme === 'custom',
		displayThemeVisible,
		fontFamilyDraft,
		fontFamilyInvalid,
		fontFamilyVisible,
		fontLigaturesVisible,
		fontSizeDraft,
		fontSizeInvalid,
		fontSizeVisible,
		fontWeightBoldVisible,
		fontWeightVisible,
		foregroundDraft,
		foregroundInvalid,
		foregroundVisible,
		inputReference,
		macosInputVisible,
		messages,
		navigationOpen,
		navigationGroups,
		onApplicationThemeChange: (event): void => {
			const applicationTheme = event.currentTarget.value;
			if (
				applicationTheme === 'system' ||
				applicationTheme === 'light' ||
				applicationTheme === 'dark'
			) {
				controller.update({ applicationTheme });
			}
		},
		onBackgroundColorChange: (event): void => {
			background.change(event.currentTarget.value);
			const normalized = normalizeTerminalHexColor(event.currentTarget.value);
			if (normalized === null) {
				setBackgroundInvalid(true);
				return;
			}
			setBackgroundInvalid(false);
			background.submit({ customBackground: normalized });
		},
		onBackgroundDraftChange: (event): void => background.change(event.currentTarget.value),
		onBackgroundDraftKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitBackground();
			event.currentTarget.blur();
		},
		onBackgroundInputBlur: (): void => commitBackground(),
		onBackgroundOpacityChange: (value): void => {
			const backgroundOpacityPercent = typeof value === 'number' ? value : value[0];
			if (backgroundOpacityPercent !== undefined) controller.update({ backgroundOpacityPercent });
		},
		onCategorySelect: (nextCategory): void => {
			setCategory(nextCategory);
			setNavigationOpen(false);
			setPendingSearchTargetId(null);
		},
		onCopyOnSelectChange: (checked): void => {
			terminalInteractionSettingsController?.update({ copyOnSelect: checked });
		},
		onFontFamilyBlur: (): void => commitFontFamily(),
		onFontFamilyChange: (event): void => setFontFamilyDraft(event.currentTarget.value),
		onFontFamilyKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitFontFamily();
			event.currentTarget.blur();
		},
		onFontLigaturesChange: (checked): void => {
			controller.update({ fontLigatures: checked });
		},
		onFontSizeBlur: (): void => commitFontSize(),
		onFontSizeChange: (event): void => setFontSizeDraft(event.currentTarget.value),
		onFontSizeKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitFontSize();
			event.currentTarget.blur();
		},
		onFontWeightBoldChange: (event): void => {
			const fontWeightBold = parseTerminalFontWeight(event.currentTarget.value);
			if (fontWeightBold !== null) controller.update({ fontWeightBold });
		},
		onFontWeightChange: (event): void => {
			const fontWeight = parseTerminalFontWeight(event.currentTarget.value);
			if (fontWeight !== null) controller.update({ fontWeight });
		},
		onForegroundColorChange: (event): void => {
			foreground.change(event.currentTarget.value);
			const normalized = normalizeTerminalHexColor(event.currentTarget.value);
			if (normalized === null) {
				setForegroundInvalid(true);
				return;
			}
			setForegroundInvalid(false);
			foreground.submit({ customForeground: normalized });
		},
		onForegroundDraftChange: (event): void => foreground.change(event.currentTarget.value),
		onForegroundDraftKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitForeground();
			event.currentTarget.blur();
		},
		onForegroundInputBlur: (): void => commitForeground(),

		onNavigationRequest: (): void => {
			setQuery('');
			setActiveSearchResultIndex(0);
			setNavigationOpen(true);
		},
		onRequestClose: (): void => onRequestClose(),
		onReset: (): void => {
			controller.resetHostAppearance();
			void controller.userPreferences.reset([
				'applicationTheme',
				'lightTheme',
				'darkTheme',
				'terminalDisplayTheme',
				'customForeground',
				'customBackground',
				'copyOnSelect',
				'trimTrailingBlankRowsForAgents',
			]);
		},
		onSearchChange: (event): void => {
			setQuery(event.currentTarget.value);
			setActiveSearchResultIndex(0);
		},
		onSearchKeyDown: (event): void => {
			if (event.key === 'ArrowDown' && resolvedSearchResults.length > 0) {
				event.preventDefault();
				setActiveSearchResultIndex((current) =>
					Math.min(current + 1, resolvedSearchResults.length - 1),
				);
				return;
			}
			if (event.key === 'ArrowUp' && resolvedSearchResults.length > 0) {
				event.preventDefault();
				setActiveSearchResultIndex((current) => Math.max(current - 1, 0));
				return;
			}
			if (event.key !== 'Enter' || activeSearchEntry === undefined) return;
			event.preventDefault();
			activateSearchEntry(boundedSearchResultIndex);
		},
		onScrollbackBytesBlur: (): void => commitScrollbackBytes(),
		onScrollbackBytesChange: (event): void => scrollback.change(event.currentTarget.value),
		onScrollbackBytesKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitScrollbackBytes();
			event.currentTarget.blur();
		},
		onSurfaceKeyDown: (event): void => {
			if (event.key !== 'Escape' || event.defaultPrevented) return;
			event.preventDefault();
			if (query.length > 0) {
				setQuery('');
				setActiveSearchResultIndex(0);
				inputReference.current?.focus();
				return;
			}
			onRequestClose();
		},
		onTerminalDisplayThemeChange: (event): void => {
			const terminalDisplayTheme = event.currentTarget.value;
			if (
				terminalDisplayTheme === 'theme' ||
				terminalDisplayTheme === 'host' ||
				terminalDisplayTheme === 'custom'
			) {
				controller.update({ terminalDisplayTheme });
			}
		},
		onTerminalPaddingBlur: (): void => commitTerminalPadding(),
		onTerminalPaddingChange: (event): void => setTerminalPaddingDraft(event.currentTarget.value),
		onTerminalPaddingKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitTerminalPadding();
			event.currentTarget.blur();
		},
		onTrimTrailingBlankRowsForAgentsChange: (checked): void => {
			terminalInteractionSettingsController?.update({
				trimTrailingBlankRowsForAgents: checked,
			});
		},
		open: snapshot.open,
		opacityVisible,
		persistenceWarning,
		preferences: snapshot.preferences,
		pageDescription: intl.formatMessage(pageDefinition.description),
		pageTitle: intl.formatMessage(pageDefinition.title),
		query,
		resetVisible,
		searchActive,
		searchResults,
		searchResultsId: TERMINAL_SETTINGS_SEARCH_RESULTS_ID,
		scrollbackBytesDraft,
		scrollbackBytesInvalid,
		scrollbackBytesVisible,
		terminalDefaultSettingsWarning,
		terminalPaddingDraft,
		terminalPaddingInvalid,
		terminalPaddingVisible,
		trimTrailingBlankRowsForAgents: interactionSnapshot.preferences.trimTrailingBlankRowsForAgents,
		trimTrailingBlankRowsForAgentsVisible,
	};
}
