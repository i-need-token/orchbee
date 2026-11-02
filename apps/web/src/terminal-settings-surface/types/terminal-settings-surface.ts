import type * as React from 'react';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ProductIconName } from '@orchbee/ui/components';
import type { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../../agent-integration-management';
import type {
	TerminalAppearancePreferences,
	TerminalSettingsController,
} from '../../terminal-settings';
import type { TerminalFeedbackSettingsController } from '../../terminal-feedback-settings';
import type { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import type { TerminalLaunchProfileControl } from '../../terminal-launch-profile-control';
import type { TerminalDefaultSettingsControl } from '../../terminal-default-settings-control';
import type { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
import type { ApplicationLocaleController } from '../../application-locale';

export type TerminalSettingsCategory =
	| 'agent-integrations'
	| 'appearance'
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
}

export interface TerminalSettingsSurfaceMessages {
	readonly applicationAppearanceSection: string;
	readonly applicationThemeDark: string;
	readonly applicationThemeDescription: string;
	readonly applicationThemeLabel: string;
	readonly applicationThemeLight: string;
	readonly applicationThemeSystem: string;
	readonly back: string;
	readonly backgroundDescription: string;
	readonly backgroundInvalid: string;
	readonly backgroundLabel: string;
	readonly backgroundOpacityDescription: string;
	readonly backgroundOpacityLabel: string;
	readonly backgroundOpacityPercentValue: (value: number) => string;

	readonly close: string;
	readonly currentDeviceOnlyNotice: string;
	readonly copyOnSelectDescription: string;
	readonly copyOnSelectLabel: string;
	readonly description: string;
	readonly fontFamilyDescription: string;
	readonly fontFamilyDeviceAvailability: string;
	readonly fontFamilyInvalid: string;
	readonly fontFamilyLabel: string;
	readonly fontLigaturesDescription: string;
	readonly fontLigaturesLabel: string;
	readonly fontSizeDescription: string;
	readonly fontSizeLabel: string;
	readonly fontSizeRangeInvalid: string;
	readonly fontWeightBoldDescription: string;
	readonly fontWeightBoldLabel: string;
	readonly fontWeightDescription: string;
	readonly fontWeightLabel: string;
	readonly foregroundDescription: string;
	readonly foregroundInvalid: string;
	readonly foregroundLabel: string;
	readonly ghosttyMigrationCategory: string;
	readonly ghosttyMigrationDescription: string;
	readonly ghosttyMigrationTitle: string;
	readonly keyboardShortcutsCategory: string;
	readonly keyboardShortcutsDescription: string;
	readonly keyboardShortcutsTitle: string;
	readonly localePersistenceInvalidWarning: string;
	readonly localePersistenceReadWarning: string;
	readonly localePersistenceWriteWarning: string;
	readonly macosInputCategory: string;
	readonly macosInputDescription: string;
	readonly macosInputTitle: string;
	readonly noResults: string;

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
	readonly scrollbackBytesBusyWarning: string;
	readonly scrollbackBytesControlUnavailableWarning: string;
	readonly scrollbackBytesDescription: string;
	readonly scrollbackBytesFutureNotice: string;
	readonly scrollbackBytesHostRejectedWarning: string;
	readonly scrollbackBytesInvalid: string;
	readonly scrollbackBytesLabel: string;
	readonly scrollbackBytesPersistenceInvalidWarning: string;
	readonly scrollbackBytesPersistenceReadWarning: string;
	readonly scrollbackBytesPersistenceWriteWarning: string;
	readonly trimTrailingBlankRowsForAgentsDescription: string;
	readonly trimTrailingBlankRowsForAgentsLabel: string;
	readonly shellIntegrationCategory: string;
	readonly shellIntegrationDescription: string;
	readonly shellIntegrationTitle: string;
	readonly terminalCategory: string;
	readonly terminalClipboardSection: string;
	readonly terminalColorsSection: string;
	readonly terminalHistorySection: string;
	readonly terminalLayoutSection: string;
	readonly terminalPaddingDescription: string;
	readonly terminalPaddingInvalid: string;
	readonly terminalPaddingLabel: string;
	readonly terminalTextShapingSection: string;
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
	readonly agentIntegrationManagementControl?: AgentIntegrationManagementControl | undefined;
	readonly agentIntegrationReminderController?: AgentIntegrationReminderController | undefined;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;

	readonly requestedCategory?: TerminalSettingsCategory | undefined;

	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
	readonly terminalLaunchProfileClient?: TerminalLaunchProfileControl | undefined;
	readonly terminalDefaultSettingsControl?: TerminalDefaultSettingsControl | undefined;
	readonly terminalInteractionSettingsController?:
		| TerminalInteractionSettingsController
		| undefined;
}

export interface TerminalSettingsSurfaceView {
	readonly activeSearchResultId: string | undefined;
	readonly applicationThemeVisible: boolean;
	readonly backgroundDraft: string;
	readonly backgroundInvalid: boolean;
	readonly backgroundVisible: boolean;
	readonly category: TerminalSettingsCategory;
	readonly contentReference: React.RefObject<HTMLElement | null>;
	readonly copyOnSelect: boolean;
	readonly copyOnSelectVisible: boolean;
	readonly customColorsEnabled: boolean;
	readonly displayThemeVisible: boolean;
	readonly fontFamilyDraft: string;
	readonly fontFamilyInvalid: boolean;
	readonly fontFamilyVisible: boolean;
	readonly fontLigaturesVisible: boolean;
	readonly fontSizeDraft: string;
	readonly fontSizeInvalid: boolean;
	readonly fontSizeVisible: boolean;
	readonly fontWeightBoldVisible: boolean;
	readonly fontWeightVisible: boolean;
	readonly foregroundDraft: string;
	readonly foregroundInvalid: boolean;
	readonly foregroundVisible: boolean;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly macosInputVisible: boolean;
	readonly localePersistenceWarning: string | null;
	readonly messages: TerminalSettingsSurfaceMessages;
	readonly navigationOpen: boolean;
	readonly navigationGroups: readonly TerminalSettingsNavigationGroupView[];
	readonly onApplicationThemeChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly onBackgroundColorChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBackgroundDraftChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onBackgroundDraftKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onBackgroundInputBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onBackgroundOpacityChange: (value: number | readonly number[]) => void;
	readonly onCategorySelect: (category: TerminalSettingsCategory) => void;
	readonly onCopyOnSelectChange: (checked: boolean) => void;
	readonly onFontFamilyBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onFontFamilyChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onFontFamilyKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onFontLigaturesChange: (checked: boolean) => void;
	readonly onFontSizeBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onFontSizeChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onFontSizeKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onFontWeightBoldChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly onFontWeightChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly onForegroundColorChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onForegroundDraftChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onForegroundDraftKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onForegroundInputBlur: React.FocusEventHandler<HTMLInputElement>;

	readonly onNavigationRequest: React.MouseEventHandler<HTMLButtonElement>;
	readonly onRequestClose: React.MouseEventHandler<HTMLButtonElement>;
	readonly onReset: React.MouseEventHandler<HTMLButtonElement>;
	readonly onSearchChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onSearchKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onScrollbackBytesBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onScrollbackBytesChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onScrollbackBytesKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onSurfaceKeyDown: React.KeyboardEventHandler<HTMLDivElement>;
	readonly onTerminalDisplayThemeChange: React.ChangeEventHandler<HTMLSelectElement>;
	readonly onTerminalPaddingBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onTerminalPaddingChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onTerminalPaddingKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onTrimTrailingBlankRowsForAgentsChange: (checked: boolean) => void;
	readonly open: boolean;
	readonly opacityVisible: boolean;
	readonly persistenceWarning: string | null;
	readonly preferences: TerminalAppearancePreferences;
	readonly pageDescription: string;
	readonly pageTitle: string;
	readonly query: string;
	readonly resetVisible: boolean;
	readonly searchActive: boolean;
	readonly searchResults: readonly TerminalSettingsSearchResultView[];
	readonly searchResultsId: string;
	readonly scrollbackBytesDraft: string;
	readonly scrollbackBytesInvalid: boolean;
	readonly scrollbackBytesPending: boolean;
	readonly scrollbackBytesVisible: boolean;
	readonly terminalDefaultSettingsWarning: string | null;
	readonly terminalPaddingDraft: string;
	readonly terminalPaddingInvalid: boolean;
	readonly terminalPaddingVisible: boolean;
	readonly trimTrailingBlankRowsForAgents: boolean;
	readonly trimTrailingBlankRowsForAgentsVisible: boolean;
}
