import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import type { TerminalSettingsCategory } from '../types/terminal-settings-surface';

interface TerminalSettingsCategoryDefinition {
	/** Whether the category ends with the row that points to config.toml for remaining options. */
	readonly configFile: boolean;
	readonly icon: ProductIconName;
	readonly id: TerminalSettingsCategory;
	readonly label: MessageDescriptor;
}

/** The Settings categories in navigation order. */
export const TERMINAL_SETTINGS_CATEGORIES: readonly TerminalSettingsCategoryDefinition[] = [
	{
		configFile: true,
		icon: 'settings',
		id: 'general',
		label: terminalSettingsMessages.pageCategoryGeneral,
	},
	{
		configFile: true,
		icon: 'appearance',
		id: 'appearance',
		label: terminalSettingsMessages.pageCategoryAppearance,
	},
	{
		configFile: false,
		icon: 'color',
		id: 'themes',
		label: terminalSettingsMessages.pageCategoryThemes,
	},
	{
		configFile: true,
		icon: 'terminal',
		id: 'terminal',
		label: terminalSettingsMessages.pageCategoryTerminal,
	},
	{
		configFile: true,
		icon: 'bell',
		id: 'notifications',
		label: terminalSettingsMessages.pageCategoryNotifications,
	},
	{
		configFile: true,
		icon: 'keyboard',
		id: 'keyboard',
		label: terminalSettingsMessages.pageCategoryKeyboard,
	},
	{
		configFile: true,
		icon: 'agent',
		id: 'agents',
		label: terminalSettingsMessages.pageCategoryAgents,
	},
];
