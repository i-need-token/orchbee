import type * as React from 'react';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ProductIconName } from '@orchbee/ui/components';
import type { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import type {
	TerminalAppearancePreferences,
	TerminalSettingsController,
} from '../../terminal-settings';
import type { TerminalFeedbackSettingsController } from '../../terminal-feedback-settings';
import type { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';

export type TerminalSettingsCategory =
	| 'appearance'
	| 'browser-access'
	| 'colors'
	| 'cursor-input'
	| 'general'
	| 'ghostty-migration'
	| 'interaction'
	| 'keyboard-shortcuts'
	| 'profiles'
	| 'remote-development'
	| 'shell-integration'
	| 'terminal'
	| 'text'
	| 'windows-tabs';

interface TerminalSettingsNavigationItemView {
	readonly icon: ProductIconName;
	readonly id: TerminalSettingsCategory;
	readonly label: string;
}

interface TerminalSettingsNavigationGroupView {
	readonly id: string;
	readonly items: readonly TerminalSettingsNavigationItemView[];
	readonly label: string;
}

export interface TerminalSettingsSearchEntry {
	readonly availability: 'available' | 'planned';
	readonly category: TerminalSettingsCategory;
	readonly categoryLabel: string;
	readonly description: string;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly keywords: readonly string[];
	readonly kind: 'category' | 'setting';
	readonly label: string;
	readonly sectionLabel: string | null;
	readonly targetId: string | null;
}

interface TerminalSettingsSearchResultView extends TerminalSettingsSearchEntry {
	readonly active: boolean;
	readonly elementId: string;
	readonly onActivate: () => void;
	readonly onFocus: () => void;
	readonly statusLabel: string | null;
}

export type PlannedTerminalSettingControlKind =
	| 'action'
	| 'color'
	| 'number'
	| 'select'
	| 'text'
	| 'toggle';

export interface PlannedTerminalSettingItemView {
	readonly control: PlannedTerminalSettingControlKind;
	readonly description: string;
	readonly id: string;
	readonly label: string;
}

export interface PlannedTerminalSettingSectionView {
	readonly id: string;
	readonly items: readonly PlannedTerminalSettingItemView[];
	readonly title: string;
}

export interface TerminalSettingsSurfaceMessages {
	readonly applicationAppearanceSection: string;
	readonly applicationThemeDark: string;
	readonly applicationThemeDescription: string;
	readonly applicationThemeLabel: string;
	readonly applicationThemeLight: string;
	readonly back: string;
	readonly backgroundDescription: string;
	readonly backgroundInvalid: string;
	readonly backgroundLabel: string;
	readonly backgroundOpacityDescription: string;
	readonly backgroundOpacityLabel: string;
	readonly backgroundOpacityPercentValue: (value: number) => string;
	readonly browserAccessCategory: string;
	readonly browserAccessDescription: string;
	readonly browserAccessTitle: string;
	readonly currentDeviceOnlyNotice: string;
	readonly description: string;
	readonly fontFamilyDescription: string;
	readonly fontFamilyDeviceAvailability: string;
	readonly fontFamilyInvalid: string;
	readonly fontFamilyLabel: string;
	readonly fontSizeDescription: string;
	readonly fontSizeLabel: string;
	readonly fontSizeRangeInvalid: string;
	readonly foregroundDescription: string;
	readonly foregroundInvalid: string;
	readonly foregroundLabel: string;
	readonly ghosttyMigrationCategory: string;
	readonly ghosttyMigrationDescription: string;
	readonly ghosttyMigrationTitle: string;
	readonly keyboardShortcutsCategory: string;
	readonly keyboardShortcutsDescription: string;
	readonly keyboardShortcutsTitle: string;
	readonly macosInputCategory: string;
	readonly macosInputDescription: string;
	readonly macosInputTitle: string;
	readonly noResults: string;
	readonly openBrowserAccess: string;
	readonly persistenceInvalidWarning: string;
	readonly persistenceReadWarning: string;
	readonly persistenceWriteWarning: string;
	readonly reset: string;
	readonly resetDescription: string;
	readonly remoteDevelopmentBaselineDescription: string;
	readonly remoteDevelopmentBaselineTitle: string;
	readonly remoteDevelopmentCategory: string;
	readonly remoteDevelopmentDescription: string;
	readonly remoteDevelopmentDiagnosticDescription: string;
	readonly remoteDevelopmentDiagnosticLabel: string;
	readonly remoteDevelopmentDiagnosticTitle: string;
	readonly remoteDevelopmentNoChangesDescription: string;
	readonly remoteDevelopmentNoChangesTitle: string;
	readonly remoteDevelopmentResultDescription: string;
	readonly remoteDevelopmentTitle: string;
	readonly searchPlaceholder: string;
	readonly shellIntegrationCategory: string;
	readonly shellIntegrationDescription: string;
	readonly shellIntegrationTitle: string;
	readonly terminalCategory: string;
	readonly terminalColorsSection: string;
	readonly terminalDisplayThemeCustom: string;
	readonly terminalDisplayThemeDark: string;
	readonly terminalDisplayThemeDescription: string;
	readonly terminalDisplayThemeHost: string;
	readonly terminalDisplayThemeLabel: string;
	readonly terminalDisplayThemeLight: string;
	readonly terminalTypographySection: string;
	readonly title: string;
}

export interface UseTerminalSettingsSurfaceOptions {
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
}

export interface TerminalSettingsSurfaceView {
	readonly activeSearchResultId: string | undefined;
	readonly applicationThemeVisible: boolean;
	readonly backgroundDraft: string;
	readonly backgroundInvalid: boolean;
	readonly backgroundVisible: boolean;
	readonly category: TerminalSettingsCategory;
	readonly contentReference: React.RefObject<HTMLElement | null>;
	readonly customColorsEnabled: boolean;
	readonly displayThemeVisible: boolean;
	readonly fontFamilyDraft: string;
	readonly fontFamilyInvalid: boolean;
	readonly fontFamilyVisible: boolean;
	readonly fontSizeDraft: string;
	readonly fontSizeInvalid: boolean;
	readonly fontSizeVisible: boolean;
	readonly foregroundDraft: string;
	readonly foregroundInvalid: boolean;
	readonly foregroundVisible: boolean;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly macosInputVisible: boolean;
	readonly messages: TerminalSettingsSurfaceMessages;
	readonly navigationGroups: readonly TerminalSettingsNavigationGroupView[];
	readonly onApplicationThemeChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly onBackgroundColorChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBackgroundDraftChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBackgroundDraftKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onBackgroundInputBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onBackgroundOpacityChange: (value: number | readonly number[]) => void;
	readonly onCategorySelect: (category: TerminalSettingsCategory) => void;
	readonly onFontFamilyBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onFontFamilyChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onFontFamilyKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onFontSizeBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onFontSizeChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onFontSizeKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onForegroundColorChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onForegroundDraftChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onForegroundDraftKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onForegroundInputBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onOpenBrowserAccess: React.MouseEventHandler<HTMLButtonElement>;
	readonly onRequestClose: React.MouseEventHandler<HTMLButtonElement>;
	readonly onReset: React.MouseEventHandler<HTMLButtonElement>;
	readonly onSearchChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onSearchKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onSurfaceKeyDown: React.KeyboardEventHandler<HTMLDivElement>;
	readonly onTerminalDisplayThemeChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly open: boolean;
	readonly opacityVisible: boolean;
	readonly persistenceWarning: string | null;
	readonly plannedNotice: string;
	readonly plannedSections: readonly PlannedTerminalSettingSectionView[];
	readonly preferences: TerminalAppearancePreferences;
	readonly pageDescription: string;
	readonly pageTitle: string;
	readonly query: string;
	readonly resetVisible: boolean;
	readonly searchActive: boolean;
	readonly searchResults: readonly TerminalSettingsSearchResultView[];
	readonly searchResultsId: string;
}
