import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import type { TerminalSettingsCategory } from '../types/terminal-settings-surface';

interface TerminalSettingsCategoryDefinition {
	/** Whether the category ends with the row that points to config.toml for remaining options. */
	readonly configFile: boolean;
	readonly icon: ProductIconName;
	readonly id: TerminalSettingsCategory;
	readonly label: MessageDescriptor;
	/** Whether Settings search lists the category's rows; an information category holds no setting. */
	readonly searchable: boolean;
}

/** The Settings categories in navigation order. */
export const TERMINAL_SETTINGS_CATEGORIES: readonly TerminalSettingsCategoryDefinition[] = [
	{
		configFile: true,
		icon: 'settings',
		id: 'general',
		label: terminalSettingsMessages.pageCategoryGeneral,
		searchable: true,
	},
	{
		configFile: true,
		icon: 'appearance',
		id: 'appearance',
		label: terminalSettingsMessages.pageCategoryAppearance,
		searchable: true,
	},
	{
		configFile: false,
		icon: 'color',
		id: 'themes',
		label: terminalSettingsMessages.pageCategoryThemes,
		searchable: true,
	},
	{
		configFile: true,
		icon: 'terminal',
		id: 'terminal',
		label: terminalSettingsMessages.pageCategoryTerminal,
		searchable: true,
	},
	{
		configFile: true,
		icon: 'bell',
		id: 'notifications',
		label: terminalSettingsMessages.pageCategoryNotifications,
		searchable: true,
	},
	{
		configFile: true,
		icon: 'keyboard',
		id: 'keyboard',
		label: terminalSettingsMessages.pageCategoryKeyboard,
		searchable: true,
	},
	{
		configFile: true,
		icon: 'agent',
		id: 'agents',
		label: terminalSettingsMessages.pageCategoryAgents,
		searchable: true,
	},
	// Last, after every setting, as Warp places it.
	{
		configFile: false,
		icon: 'info',
		id: 'about',
		label: terminalSettingsMessages.pageCategoryAbout,
		searchable: false,
	},
];
