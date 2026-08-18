import { formatKeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';
import {
	keyboardShortcutMessages,
	terminalSettingsCatalogMessages,
	terminalSettingsMessages,
	useOrchbeeIntl,
} from '@orchbee/i18n';
import * as React from 'react';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	useKeyboardShortcutSettings,
	type KeyboardShortcutId,
} from '../../keyboard-shortcuts';
import { normalizeTerminalHexColor, useTerminalSettings } from '../../terminal-settings';
import { TERMINAL_SETTINGS_SEARCH_RESULTS_ID } from '../constants/terminal-settings-search-catalog';
import {
	TERMINAL_SETTINGS_CATEGORIES,
	TERMINAL_SETTINGS_NAVIGATION_GROUPS,
} from '../constants/terminal-settings-information-architecture';
import type {
	TerminalSettingsCategory,
	TerminalSettingsSurfaceMessages,
	TerminalSettingsSurfaceView,
	UseTerminalSettingsSurfaceOptions,
} from '../types/terminal-settings-surface';
import { buildTerminalSettingsSearchCatalog } from '../utils/buildTerminalSettingsSearchCatalog';
import { resolvePlannedTerminalSettings } from '../utils/resolvePlannedTerminalSettings';
import { resolveTerminalSettingsSearch } from '../utils/resolveTerminalSettingsSearch';

export function useTerminalSettingsSurface({
	controller,
	keyboardShortcutController,
	keyboardShortcutPlatform,
	macosTerminalInputSettingsController,
	onRequestClose,
	requestBrowserAccess,
	terminalFeedbackSettingsController,
}: UseTerminalSettingsSurfaceOptions): TerminalSettingsSurfaceView {
	const intl = useOrchbeeIntl();
	const snapshot = useTerminalSettings(controller);
	const keyboardShortcutSnapshot = useKeyboardShortcutSettings(keyboardShortcutController);
	const inputReference = React.useRef<HTMLInputElement>(null);
	const contentReference = React.useRef<HTMLElement>(null);
	const [category, setCategory] = React.useState<TerminalSettingsCategory>('general');
	const [query, setQuery] = React.useState('');
	const [activeSearchResultIndex, setActiveSearchResultIndex] = React.useState(0);
	const [pendingSearchTargetId, setPendingSearchTargetId] = React.useState<
		string | null | undefined
	>(undefined);
	const [fontFamilyDraft, setFontFamilyDraft] = React.useState(snapshot.preferences.fontFamily);
	const [fontFamilyInvalid, setFontFamilyInvalid] = React.useState(false);
	const [fontSizeDraft, setFontSizeDraft] = React.useState(String(snapshot.preferences.fontSize));
	const [fontSizeInvalid, setFontSizeInvalid] = React.useState(false);
	const [foregroundDraft, setForegroundDraft] = React.useState<string>(
		snapshot.preferences.customForeground,
	);
	const [foregroundInvalid, setForegroundInvalid] = React.useState(false);
	const [backgroundDraft, setBackgroundDraft] = React.useState<string>(
		snapshot.preferences.customBackground,
	);
	const [backgroundInvalid, setBackgroundInvalid] = React.useState(false);

	React.useEffect(() => {
		setFontFamilyDraft(snapshot.preferences.fontFamily);
		setFontFamilyInvalid(false);
		setFontSizeDraft(String(snapshot.preferences.fontSize));
		setFontSizeInvalid(false);
		setForegroundDraft(snapshot.preferences.customForeground);
		setForegroundInvalid(false);
		setBackgroundDraft(snapshot.preferences.customBackground);
		setBackgroundInvalid(false);
	}, [snapshot.preferences]);

	React.useEffect(() => {
		if (!snapshot.open) return;
		inputReference.current?.focus();
	}, [snapshot.open]);

	React.useEffect(() => {
		if (pendingSearchTargetId === undefined) return;
		const content = contentReference.current;
		if (content === null) return;
		if (pendingSearchTargetId === null) {
			content.scrollTop = 0;
		} else {
			const target = document.getElementById(pendingSearchTargetId);
			if (target !== null && content.contains(target)) {
				target.scrollIntoView({ block: 'center' });
			}
		}
		setPendingSearchTargetId(undefined);
	}, [category, pendingSearchTargetId]);

	const messages: TerminalSettingsSurfaceMessages = {
		applicationAppearanceSection: intl.formatMessage(
			terminalSettingsMessages.applicationAppearanceSection,
		),
		applicationThemeDark: intl.formatMessage(terminalSettingsMessages.applicationThemeDark),
		applicationThemeDescription: intl.formatMessage(
			terminalSettingsMessages.applicationThemeDescription,
		),
		applicationThemeLabel: intl.formatMessage(terminalSettingsMessages.applicationThemeLabel),
		applicationThemeLight: intl.formatMessage(terminalSettingsMessages.applicationThemeLight),
		back: intl.formatMessage(terminalSettingsMessages.back),
		backgroundDescription: intl.formatMessage(terminalSettingsMessages.backgroundDescription),
		backgroundInvalid: intl.formatMessage(terminalSettingsMessages.backgroundInvalid),
		backgroundLabel: intl.formatMessage(terminalSettingsMessages.backgroundLabel),
		backgroundOpacityDescription: intl.formatMessage(
			terminalSettingsMessages.backgroundOpacityDescription,
		),
		backgroundOpacityLabel: intl.formatMessage(terminalSettingsMessages.backgroundOpacityLabel),
		backgroundOpacityPercentValue: (value: number): string =>
			intl.formatMessage(terminalSettingsMessages.backgroundOpacityPercentValue, { value }),
		browserAccessCategory: intl.formatMessage(terminalSettingsMessages.browserAccessCategory),
		browserAccessDescription: intl.formatMessage(terminalSettingsMessages.browserAccessDescription),
		browserAccessTitle: intl.formatMessage(terminalSettingsMessages.browserAccessTitle),
		currentDeviceOnlyNotice: intl.formatMessage(terminalSettingsMessages.currentDeviceOnlyNotice),
		description: intl.formatMessage(terminalSettingsMessages.description),
		fontFamilyDescription: intl.formatMessage(terminalSettingsMessages.fontFamilyDescription),
		fontFamilyDeviceAvailability: intl.formatMessage(
			terminalSettingsMessages.fontFamilyDeviceAvailability,
		),
		fontFamilyInvalid: intl.formatMessage(terminalSettingsMessages.fontFamilyInvalid),
		fontFamilyLabel: intl.formatMessage(terminalSettingsMessages.fontFamilyLabel),
		fontSizeDescription: intl.formatMessage(terminalSettingsMessages.fontSizeDescription),
		fontSizeLabel: intl.formatMessage(terminalSettingsMessages.fontSizeLabel),
		fontSizeRangeInvalid: intl.formatMessage(terminalSettingsMessages.fontSizeRangeInvalid, {
			maximum: 48,
			minimum: 8,
		}),
		foregroundDescription: intl.formatMessage(terminalSettingsMessages.foregroundDescription),
		foregroundInvalid: intl.formatMessage(terminalSettingsMessages.foregroundInvalid),
		foregroundLabel: intl.formatMessage(terminalSettingsMessages.foregroundLabel),
		ghosttyMigrationCategory: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationCategory),
		ghosttyMigrationDescription: intl.formatMessage(
			terminalSettingsMessages.ghosttyMigrationDescription,
		),
		ghosttyMigrationTitle: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationTitle),
		keyboardShortcutsCategory: intl.formatMessage(keyboardShortcutMessages.category),
		keyboardShortcutsDescription: intl.formatMessage(keyboardShortcutMessages.description),
		keyboardShortcutsTitle: intl.formatMessage(keyboardShortcutMessages.title),
		macosInputCategory: intl.formatMessage(terminalSettingsMessages.macosInputCategory),
		macosInputDescription: intl.formatMessage(terminalSettingsMessages.macosInputDescription),
		macosInputTitle: intl.formatMessage(terminalSettingsMessages.macosInputTitle),
		noResults: intl.formatMessage(terminalSettingsMessages.noResults),
		openBrowserAccess: intl.formatMessage(terminalSettingsMessages.openBrowserAccess),
		persistenceInvalidWarning: intl.formatMessage(
			terminalSettingsMessages.persistenceInvalidWarning,
		),
		persistenceReadWarning: intl.formatMessage(terminalSettingsMessages.persistenceReadWarning),
		persistenceWriteWarning: intl.formatMessage(terminalSettingsMessages.persistenceWriteWarning),
		reset: intl.formatMessage(terminalSettingsMessages.reset),
		resetDescription: intl.formatMessage(terminalSettingsMessages.resetDescription),
		remoteDevelopmentBaselineDescription: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentBaselineDescription,
		),
		remoteDevelopmentBaselineTitle: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentBaselineTitle,
		),
		remoteDevelopmentCategory: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentCategory,
		),
		remoteDevelopmentDescription: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentDescription,
		),
		remoteDevelopmentDiagnosticDescription: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentDiagnosticDescription,
		),
		remoteDevelopmentDiagnosticLabel: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentDiagnosticLabel,
		),
		remoteDevelopmentDiagnosticTitle: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentDiagnosticTitle,
		),
		remoteDevelopmentNoChangesDescription: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentNoChangesDescription,
		),
		remoteDevelopmentNoChangesTitle: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentNoChangesTitle,
		),
		remoteDevelopmentResultDescription: intl.formatMessage(
			terminalSettingsMessages.remoteDevelopmentResultDescription,
		),
		remoteDevelopmentTitle: intl.formatMessage(terminalSettingsMessages.remoteDevelopmentTitle),
		searchPlaceholder: intl.formatMessage(terminalSettingsMessages.searchPlaceholder),
		shellIntegrationCategory: intl.formatMessage(terminalSettingsMessages.shellIntegrationCategory),
		shellIntegrationDescription: intl.formatMessage(
			terminalSettingsMessages.shellIntegrationDescription,
		),
		shellIntegrationTitle: intl.formatMessage(terminalSettingsMessages.shellIntegrationTitle),
		terminalCategory: intl.formatMessage(terminalSettingsMessages.terminalCategory),
		terminalColorsSection: intl.formatMessage(terminalSettingsMessages.terminalColorsSection),
		terminalDisplayThemeCustom: intl.formatMessage(
			terminalSettingsMessages.terminalDisplayThemeCustom,
		),
		terminalDisplayThemeDark: intl.formatMessage(terminalSettingsMessages.terminalDisplayThemeDark),
		terminalDisplayThemeDescription: intl.formatMessage(
			terminalSettingsMessages.terminalDisplayThemeDescription,
		),
		terminalDisplayThemeHost: intl.formatMessage(terminalSettingsMessages.terminalDisplayThemeHost),
		terminalDisplayThemeLabel: intl.formatMessage(
			terminalSettingsMessages.terminalDisplayThemeLabel,
		),
		terminalDisplayThemeLight: intl.formatMessage(
			terminalSettingsMessages.terminalDisplayThemeLight,
		),
		terminalTypographySection: intl.formatMessage(
			terminalSettingsMessages.terminalTypographySection,
		),
		title: intl.formatMessage(terminalSettingsMessages.title),
	};
	const applicationThemeVisible = true;
	const fontFamilyVisible = true;
	const fontSizeVisible = true;
	const displayThemeVisible = true;
	const foregroundVisible = true;
	const backgroundVisible = true;
	const opacityVisible = true;
	const resetVisible = true;
	const macosInputVisible = true;
	const plannedSections = resolvePlannedTerminalSettings({
		category,
		formatMessage: (descriptor) => intl.formatMessage(descriptor),
	});
	const navigationGroups = TERMINAL_SETTINGS_NAVIGATION_GROUPS.map((group) => ({
		id: group.id,
		items: group.categories
			.filter(
				(categoryId) =>
					(categoryId !== 'browser-access' || requestBrowserAccess !== undefined) &&
					(categoryId !== 'shell-integration' || terminalFeedbackSettingsController !== undefined),
			)
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
	const searchCatalog = buildTerminalSettingsSearchCatalog({
		enabledCapabilities:
			macosTerminalInputSettingsController === undefined ? [] : ['macos-option-as-alt'],
		enabledCategories,
		formatMessage: (descriptor) => intl.formatMessage(descriptor),
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
		setQuery('');
		setActiveSearchResultIndex(0);
		setPendingSearchTargetId(entry.targetId);
	};
	const plannedSearchResultLabel = intl.formatMessage(
		terminalSettingsCatalogMessages.get('terminal.settings.catalog.common.planned'),
	);
	const searchResults = resolvedSearchResults.map((entry, resultIndex) => ({
		...entry,
		active: resultIndex === boundedSearchResultIndex,
		elementId: `terminal-settings-search-result-${entry.id}`,
		onActivate: (): void => activateSearchEntry(resultIndex),
		onFocus: (): void => setActiveSearchResultIndex(resultIndex),
		statusLabel: entry.availability === 'planned' ? plannedSearchResultLabel : null,
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
	const commitForeground = (): void => {
		const normalized = normalizeTerminalHexColor(foregroundDraft);
		if (normalized === null) {
			setForegroundInvalid(true);
			return;
		}
		setForegroundInvalid(controller.update({ customForeground: normalized }).kind === 'invalid');
	};
	const commitBackground = (): void => {
		const normalized = normalizeTerminalHexColor(backgroundDraft);
		if (normalized === null) {
			setBackgroundInvalid(true);
			return;
		}
		setBackgroundInvalid(controller.update({ customBackground: normalized }).kind === 'invalid');
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

	return {
		activeSearchResultId,
		applicationThemeVisible,
		backgroundDraft,
		backgroundInvalid,
		backgroundVisible,
		category,
		contentReference,
		customColorsEnabled: snapshot.preferences.terminalDisplayTheme === 'custom',
		displayThemeVisible,
		fontFamilyDraft,
		fontFamilyInvalid,
		fontFamilyVisible,
		fontSizeDraft,
		fontSizeInvalid,
		fontSizeVisible,
		foregroundDraft,
		foregroundInvalid,
		foregroundVisible,
		inputReference,
		macosInputVisible,
		messages,
		navigationGroups,
		onApplicationThemeChange: (event): void => {
			const applicationTheme = event.currentTarget.value;
			if (applicationTheme === 'light' || applicationTheme === 'dark') {
				controller.update({ applicationTheme });
			}
		},
		onBackgroundColorChange: (event): void => {
			setBackgroundDraft(event.currentTarget.value);
			const normalized = normalizeTerminalHexColor(event.currentTarget.value);
			if (normalized === null) {
				setBackgroundInvalid(true);
				return;
			}
			setBackgroundInvalid(controller.update({ customBackground: normalized }).kind === 'invalid');
		},
		onBackgroundDraftChange: (event): void => setBackgroundDraft(event.currentTarget.value),
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
			setPendingSearchTargetId(null);
		},
		onFontFamilyBlur: (): void => commitFontFamily(),
		onFontFamilyChange: (event): void => setFontFamilyDraft(event.currentTarget.value),
		onFontFamilyKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitFontFamily();
			event.currentTarget.blur();
		},
		onFontSizeBlur: (): void => commitFontSize(),
		onFontSizeChange: (event): void => setFontSizeDraft(event.currentTarget.value),
		onFontSizeKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitFontSize();
			event.currentTarget.blur();
		},
		onForegroundColorChange: (event): void => {
			setForegroundDraft(event.currentTarget.value);
			const normalized = normalizeTerminalHexColor(event.currentTarget.value);
			if (normalized === null) {
				setForegroundInvalid(true);
				return;
			}
			setForegroundInvalid(controller.update({ customForeground: normalized }).kind === 'invalid');
		},
		onForegroundDraftChange: (event): void => setForegroundDraft(event.currentTarget.value),
		onForegroundDraftKeyDown: (event): void => {
			if (event.key !== 'Enter') return;
			event.preventDefault();
			commitForeground();
			event.currentTarget.blur();
		},
		onForegroundInputBlur: (): void => commitForeground(),
		onOpenBrowserAccess: (event): void => requestBrowserAccess?.(event.currentTarget),
		onRequestClose: (): void => onRequestClose(),
		onReset: (): void => {
			controller.reset();
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
				terminalDisplayTheme === 'host' ||
				terminalDisplayTheme === 'light' ||
				terminalDisplayTheme === 'dark' ||
				terminalDisplayTheme === 'custom'
			) {
				controller.update({ terminalDisplayTheme });
			}
		},
		open: snapshot.open,
		opacityVisible,
		persistenceWarning,
		plannedNotice: intl.formatMessage(
			terminalSettingsCatalogMessages.get('terminal.settings.catalog.common.plannedNotice'),
		),
		plannedSections,
		preferences: snapshot.preferences,
		pageDescription: intl.formatMessage(pageDefinition.description),
		pageTitle: intl.formatMessage(pageDefinition.title),
		query,
		resetVisible,
		searchActive,
		searchResults,
		searchResultsId: TERMINAL_SETTINGS_SEARCH_RESULTS_ID,
	};
}
