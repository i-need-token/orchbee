import type * as React from 'react';
import type {
	TerminalAppearancePreferences,
	TerminalSettingsController,
} from '../../terminal-settings';

type TerminalSettingsCategory =
	| 'browser-access'
	| 'ghostty-migration'
	| 'keyboard-shortcuts'
	| 'terminal';

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
	readonly noResults: string;
	readonly openBrowserAccess: string;
	readonly persistenceInvalidWarning: string;
	readonly persistenceReadWarning: string;
	readonly persistenceWriteWarning: string;
	readonly reset: string;
	readonly resetDescription: string;
	readonly searchPlaceholder: string;
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
	readonly onRequestClose: () => void;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
}

export interface TerminalSettingsSurfaceView {
	readonly applicationThemeVisible: boolean;
	readonly backgroundDraft: string;
	readonly backgroundInvalid: boolean;
	readonly backgroundVisible: boolean;
	readonly browserAccessVisible: boolean;
	readonly category: TerminalSettingsCategory;
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
	readonly ghosttyMigrationVisible: boolean;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly messages: TerminalSettingsSurfaceMessages;
	readonly onApplicationThemeChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly onBackgroundColorChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBackgroundDraftChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBackgroundDraftKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onBackgroundInputBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onBackgroundOpacityChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBrowserAccessCategoryClick: React.MouseEventHandler<HTMLButtonElement>;
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
	readonly onGhosttyMigrationCategoryClick: React.MouseEventHandler<HTMLButtonElement>;
	readonly onKeyboardShortcutsCategoryClick: React.MouseEventHandler<HTMLButtonElement>;
	readonly onOpenBrowserAccess: React.MouseEventHandler<HTMLButtonElement>;
	readonly onRequestClose: React.MouseEventHandler<HTMLButtonElement>;
	readonly onReset: React.MouseEventHandler<HTMLButtonElement>;
	readonly onSearchChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onSurfaceKeyDown: React.KeyboardEventHandler<HTMLDivElement>;
	readonly onTerminalCategoryClick: React.MouseEventHandler<HTMLButtonElement>;
	readonly onTerminalDisplayThemeChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly open: boolean;
	readonly opacityVisible: boolean;
	readonly persistenceWarning: string | null;
	readonly preferences: TerminalAppearancePreferences;
	readonly query: string;
	readonly resetVisible: boolean;
	readonly showBrowserAccessCategory: boolean;
	readonly terminalResultCount: number;
}
