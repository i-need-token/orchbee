import { keyboardShortcutMessages, terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import { normalizeTerminalHexColor, useTerminalSettings } from '../../terminal-settings';
import {
	REMOTE_DEVELOPMENT_BASELINE_TERM,
	REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND,
} from '../constants/remote-development-diagnostic';
import type {
	TerminalSettingsSurfaceMessages,
	TerminalSettingsSurfaceView,
	UseTerminalSettingsSurfaceOptions,
} from '../types/terminal-settings-surface';

export function useTerminalSettingsSurface({
	controller,
	macosTerminalInputSettingsController,
	onRequestClose,
	requestBrowserAccess,
	terminalFeedbackSettingsController,
}: UseTerminalSettingsSurfaceOptions): TerminalSettingsSurfaceView {
	const intl = useOrchbeeIntl();
	const snapshot = useTerminalSettings(controller);
	const inputReference = React.useRef<HTMLInputElement>(null);
	const [category, setCategory] = React.useState<
		| 'browser-access'
		| 'ghostty-migration'
		| 'keyboard-shortcuts'
		| 'macos-input'
		| 'remote-development'
		| 'shell-integration'
		| 'terminal'
	>('terminal');
	const [query, setQuery] = React.useState('');
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
	const normalizedQuery = query.trim().toLocaleLowerCase(intl.locale);
	const matchesQuery = (values: readonly string[]): boolean =>
		normalizedQuery.length === 0 ||
		values.some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
	const applicationThemeVisible = matchesQuery([
		messages.applicationAppearanceSection,
		messages.applicationThemeLabel,
		messages.applicationThemeDescription,
		messages.applicationThemeLight,
		messages.applicationThemeDark,
	]);
	const fontFamilyVisible = matchesQuery([
		messages.terminalTypographySection,
		messages.fontFamilyLabel,
		messages.fontFamilyDescription,
		messages.fontFamilyDeviceAvailability,
	]);
	const fontSizeVisible = matchesQuery([
		messages.terminalTypographySection,
		messages.fontSizeLabel,
		messages.fontSizeDescription,
	]);
	const displayThemeVisible = matchesQuery([
		messages.terminalColorsSection,
		messages.terminalDisplayThemeLabel,
		messages.terminalDisplayThemeDescription,
		messages.terminalDisplayThemeHost,
		messages.terminalDisplayThemeLight,
		messages.terminalDisplayThemeDark,
		messages.terminalDisplayThemeCustom,
	]);
	const foregroundVisible = matchesQuery([
		messages.terminalColorsSection,
		messages.foregroundLabel,
		messages.foregroundDescription,
	]);
	const backgroundVisible = matchesQuery([
		messages.terminalColorsSection,
		messages.backgroundLabel,
		messages.backgroundDescription,
	]);
	const opacityVisible = matchesQuery([
		messages.terminalColorsSection,
		messages.backgroundOpacityLabel,
		messages.backgroundOpacityDescription,
	]);
	const resetVisible = matchesQuery([messages.reset, messages.resetDescription]);
	const terminalResultCount = [
		applicationThemeVisible,
		fontFamilyVisible,
		fontSizeVisible,
		displayThemeVisible,
		foregroundVisible,
		backgroundVisible,
		opacityVisible,
		resetVisible,
	].filter(Boolean).length;
	const browserAccessVisible = matchesQuery([
		messages.browserAccessTitle,
		messages.browserAccessDescription,
		messages.openBrowserAccess,
	]);
	const ghosttyMigrationVisible = matchesQuery([
		messages.ghosttyMigrationCategory,
		messages.ghosttyMigrationTitle,
		messages.ghosttyMigrationDescription,
	]);
	const remoteDevelopmentVisible = matchesQuery([
		messages.remoteDevelopmentCategory,
		messages.remoteDevelopmentTitle,
		messages.remoteDevelopmentDescription,
		messages.remoteDevelopmentBaselineTitle,
		messages.remoteDevelopmentBaselineDescription,
		messages.remoteDevelopmentDiagnosticTitle,
		messages.remoteDevelopmentDiagnosticDescription,
		messages.remoteDevelopmentDiagnosticLabel,
		messages.remoteDevelopmentResultDescription,
		messages.remoteDevelopmentNoChangesTitle,
		messages.remoteDevelopmentNoChangesDescription,
		REMOTE_DEVELOPMENT_BASELINE_TERM,
		REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND,
	]);
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
		applicationThemeVisible,
		backgroundDraft,
		backgroundInvalid,
		backgroundVisible,
		browserAccessVisible,
		category,
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
		ghosttyMigrationVisible,
		inputReference,
		messages,
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
		onBrowserAccessCategoryClick: (): void => setCategory('browser-access'),
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
		onGhosttyMigrationCategoryClick: (): void => setCategory('ghostty-migration'),
		onKeyboardShortcutsCategoryClick: (): void => setCategory('keyboard-shortcuts'),
		onMacosInputCategoryClick: (): void => setCategory('macos-input'),
		onOpenBrowserAccess: (event): void => requestBrowserAccess?.(event.currentTarget),
		onRemoteDevelopmentCategoryClick: (): void => setCategory('remote-development'),
		onRequestClose: (): void => onRequestClose(),
		onReset: (): void => {
			controller.reset();
		},
		onSearchChange: (event): void => setQuery(event.currentTarget.value),
		onShellIntegrationCategoryClick: (): void => setCategory('shell-integration'),
		onSurfaceKeyDown: (event): void => {
			if (event.key !== 'Escape' || event.defaultPrevented) return;
			event.preventDefault();
			onRequestClose();
		},
		onTerminalCategoryClick: (): void => setCategory('terminal'),
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
		preferences: snapshot.preferences,
		query,
		resetVisible,
		remoteDevelopmentVisible,
		showBrowserAccessCategory: requestBrowserAccess !== undefined,
		showMacosInputCategory: macosTerminalInputSettingsController !== undefined,
		showShellIntegrationCategory: terminalFeedbackSettingsController !== undefined,
		terminalResultCount,
	};
}
