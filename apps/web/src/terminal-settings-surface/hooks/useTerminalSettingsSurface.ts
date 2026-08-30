import { formatKeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';
import { terminalSettingsCatalogMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	useKeyboardShortcutSettings,
	type KeyboardShortcutId,
} from '../../keyboard-shortcuts';
import { useApplicationLocale } from '../../application-locale';
import { useTerminalInteractionSettings } from '../../terminal-interaction-settings';
import type { DesktopTerminalDefaultSettingsClientSnapshot } from '../../terminal-default-settings-control';
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
import { buildTerminalSettingsSearchCatalog } from '../utils/buildTerminalSettingsSearchCatalog';
import { buildTerminalSettingsSurfaceMessages } from '../utils/buildTerminalSettingsSurfaceMessages';
import { resolvePlannedTerminalSettings } from '../utils/resolvePlannedTerminalSettings';
import { resolveTerminalSettingsSearch } from '../utils/resolveTerminalSettingsSearch';
import {
	parseScrollbackBytes,
	parseTerminalFontWeight,
	parseTerminalPadding,
} from '../utils/terminal-settings-draft-parsing';

const APPLICATION_SETTING_IDS: ReadonlySet<string> = new Set(['general.language']);
const UNAVAILABLE_TERMINAL_DEFAULT_SETTINGS_SNAPSHOT: DesktopTerminalDefaultSettingsClientSnapshot =
	{
		configuration: { scrollbackBytes: 10_000_000 },
		failureCode: 'control-unavailable',
		pending: false,
		persistenceIssue: null,
		source: 'runtime',
	};

export function useTerminalSettingsSurface({
	agentIntegrationManagementControl,
	applicationLocaleController,
	controller,
	keyboardShortcutController,
	keyboardShortcutPlatform,
	macosTerminalInputSettingsController,
	onRequestClose,
	requestBrowserAccess,
	requestedCategory,
	sshConnectionManagementControl,
	terminalFeedbackSettingsController,
	terminalDefaultSettingsControl,
	terminalInteractionSettingsController,
	terminalLaunchProfileClient,
}: UseTerminalSettingsSurfaceOptions): TerminalSettingsSurfaceView {
	const intl = useOrchbeeIntl();
	const applicationLocaleSnapshot = useApplicationLocale(applicationLocaleController);
	const snapshot = useTerminalSettings(controller);
	const interactionSnapshot = useTerminalInteractionSettings(terminalInteractionSettingsController);
	const unavailableTerminalDefaultSettingsSource = React.useMemo(
		() => ({
			getSnapshot: (): DesktopTerminalDefaultSettingsClientSnapshot =>
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
	const [terminalPaddingDraft, setTerminalPaddingDraft] = React.useState(
		String(snapshot.preferences.terminalPadding),
	);
	const [terminalPaddingInvalid, setTerminalPaddingInvalid] = React.useState(false);
	const [scrollbackBytesDraft, setScrollbackBytesDraft] = React.useState(
		String(terminalDefaultSettingsSnapshot.configuration.scrollbackBytes),
	);
	const [scrollbackBytesInvalid, setScrollbackBytesInvalid] = React.useState(false);
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
		setTerminalPaddingDraft(String(snapshot.preferences.terminalPadding));
		setTerminalPaddingInvalid(false);
		setForegroundDraft(snapshot.preferences.customForeground);
		setForegroundInvalid(false);
		setBackgroundDraft(snapshot.preferences.customBackground);
		setBackgroundInvalid(false);
	}, [snapshot.preferences]);

	React.useEffect(() => {
		setScrollbackBytesDraft(String(terminalDefaultSettingsSnapshot.configuration.scrollbackBytes));
		setScrollbackBytesInvalid(false);
	}, [terminalDefaultSettingsSnapshot.configuration.scrollbackBytes]);

	React.useEffect(() => {
		if (!snapshot.open) return;
		inputReference.current?.focus();
	}, [snapshot.open]);

	React.useLayoutEffect(() => {
		if (!snapshot.open || requestedCategory === undefined) return;
		setCategory(requestedCategory);
		setQuery('');
		setActiveSearchResultIndex(0);
		setPendingSearchTargetId(null);
	}, [requestedCategory, snapshot.open]);

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
	const implementedSettingIds = new Set(APPLICATION_SETTING_IDS);
	if (scrollbackBytesVisible) implementedSettingIds.add('terminal.scrollback');
	if (trimTrailingBlankRowsForAgentsVisible) {
		implementedSettingIds.add('terminal.trimTrailingBlankRowsForAgents');
	}
	const plannedSections = resolvePlannedTerminalSettings({
		category,
		formatMessage: (descriptor) => intl.formatMessage(descriptor),
		implementedSettingIds,
	});
	const navigationGroups = TERMINAL_SETTINGS_NAVIGATION_GROUPS.map((group) => ({
		id: group.id,
		items: group.categories
			.filter(
				(categoryId) =>
					(categoryId !== 'agent-integrations' ||
						agentIntegrationManagementControl !== undefined) &&
					(categoryId !== 'browser-access' || requestBrowserAccess !== undefined) &&
					(categoryId !== 'ssh-connections' || sshConnectionManagementControl !== undefined) &&
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
		...(sshConnectionManagementControl === undefined
			? []
			: (['desktop-ssh-connection-management'] as const)),
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
		void terminalDefaultSettingsControl?.update({ scrollbackBytes });
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
	const persistenceIssue = snapshot.persistenceIssue ?? interactionSnapshot.persistenceIssue;
	switch (persistenceIssue) {
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
	let localePersistenceWarning: string | null = null;
	switch (applicationLocaleSnapshot.persistenceIssue) {
		case 'invalid-record':
			localePersistenceWarning = messages.localePersistenceInvalidWarning;
			break;
		case 'read-failed':
			localePersistenceWarning = messages.localePersistenceReadWarning;
			break;
		case 'write-failed':
			localePersistenceWarning = messages.localePersistenceWriteWarning;
			break;
		case null:
			break;
	}
	let terminalDefaultSettingsWarning: string | null = null;
	if (terminalDefaultSettingsControl !== undefined) {
		switch (terminalDefaultSettingsSnapshot.failureCode) {
			case 'busy':
				terminalDefaultSettingsWarning = messages.scrollbackBytesBusyWarning;
				break;
			case 'control-unavailable':
				terminalDefaultSettingsWarning = messages.scrollbackBytesControlUnavailableWarning;
				break;
			case 'host-rejected':
				terminalDefaultSettingsWarning = messages.scrollbackBytesHostRejectedWarning;
				break;
			case null:
				switch (terminalDefaultSettingsSnapshot.persistenceIssue) {
					case 'invalid-record':
						terminalDefaultSettingsWarning = messages.scrollbackBytesPersistenceInvalidWarning;
						break;
					case 'read-failed':
						terminalDefaultSettingsWarning = messages.scrollbackBytesPersistenceReadWarning;
						break;
					case 'write-failed':
						terminalDefaultSettingsWarning = messages.scrollbackBytesPersistenceWriteWarning;
						break;
					case null:
						break;
				}
		}
	}

	return {
		activeSearchResultId,
		applicationThemeVisible,
		backgroundDraft,
		backgroundInvalid,
		backgroundVisible,
		category,
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
		localePersistenceWarning,
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
			terminalInteractionSettingsController?.reset();
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
		onScrollbackBytesChange: (event): void => setScrollbackBytesDraft(event.currentTarget.value),
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
				terminalDisplayTheme === 'host' ||
				terminalDisplayTheme === 'light' ||
				terminalDisplayTheme === 'dark' ||
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
		scrollbackBytesDraft,
		scrollbackBytesInvalid,
		scrollbackBytesPending: terminalDefaultSettingsSnapshot.pending,
		scrollbackBytesVisible,
		terminalDefaultSettingsWarning,
		terminalPaddingDraft,
		terminalPaddingInvalid,
		terminalPaddingVisible,
		trimTrailingBlankRowsForAgents: interactionSnapshot.preferences.trimTrailingBlankRowsForAgents,
		trimTrailingBlankRowsForAgentsVisible,
	};
}
