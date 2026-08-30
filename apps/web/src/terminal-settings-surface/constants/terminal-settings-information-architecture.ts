import {
	agentIntegrationMessages,
	keyboardShortcutMessages,
	sshConnectionManagementMessages,
	terminalSettingsCatalogMessages,
	terminalSettingsMessages,
	type MessageDescriptor,
} from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import type { TerminalSettingsCategory } from '../types/terminal-settings-surface';

interface TerminalSettingsCategoryDefinition {
	readonly description: MessageDescriptor;
	readonly icon: ProductIconName;
	readonly label: MessageDescriptor;
	readonly title: MessageDescriptor;
}

interface TerminalSettingsNavigationGroupDefinition {
	readonly categories: readonly TerminalSettingsCategory[];
	readonly id: string;
	readonly label: MessageDescriptor;
}

const catalogMessage = terminalSettingsCatalogMessages.get;

export const TERMINAL_SETTINGS_CATEGORIES = {
	'agent-integrations': {
		description: agentIntegrationMessages.description,
		icon: 'agent',
		label: agentIntegrationMessages.category,
		title: agentIntegrationMessages.title,
	},
	appearance: {
		description: catalogMessage('terminal.settings.catalog.categories.appearance.description'),
		icon: 'layout',
		label: catalogMessage('terminal.settings.catalog.categories.appearance.title'),
		title: catalogMessage('terminal.settings.catalog.categories.appearance.title'),
	},
	'browser-access': {
		description: terminalSettingsMessages.browserAccessDescription,
		icon: 'screen-full',
		label: terminalSettingsMessages.browserAccessCategory,
		title: terminalSettingsMessages.browserAccessTitle,
	},
	colors: {
		description: catalogMessage('terminal.settings.catalog.categories.colors.description'),
		icon: 'panels',
		label: catalogMessage('terminal.settings.catalog.categories.colors.title'),
		title: catalogMessage('terminal.settings.catalog.categories.colors.title'),
	},
	'cursor-input': {
		description: catalogMessage('terminal.settings.catalog.categories.cursorInput.description'),
		icon: 'action',
		label: catalogMessage('terminal.settings.catalog.categories.cursorInput.title'),
		title: catalogMessage('terminal.settings.catalog.categories.cursorInput.title'),
	},
	general: {
		description: catalogMessage('terminal.settings.catalog.categories.general.description'),
		icon: 'settings',
		label: catalogMessage('terminal.settings.catalog.categories.general.title'),
		title: catalogMessage('terminal.settings.catalog.categories.general.title'),
	},
	'ghostty-migration': {
		description: terminalSettingsMessages.ghosttyMigrationDescription,
		icon: 'clipboard',
		label: terminalSettingsMessages.ghosttyMigrationCategory,
		title: terminalSettingsMessages.ghosttyMigrationTitle,
	},
	interaction: {
		description: catalogMessage('terminal.settings.catalog.categories.interaction.description'),
		icon: 'sidebar-left',
		label: catalogMessage('terminal.settings.catalog.categories.interaction.title'),
		title: catalogMessage('terminal.settings.catalog.categories.interaction.title'),
	},
	'keyboard-shortcuts': {
		description: keyboardShortcutMessages.description,
		icon: 'action',
		label: keyboardShortcutMessages.category,
		title: keyboardShortcutMessages.title,
	},
	profiles: {
		description: catalogMessage('terminal.settings.catalog.categories.profiles.description'),
		icon: 'agent',
		label: catalogMessage('terminal.settings.catalog.categories.profiles.title'),
		title: catalogMessage('terminal.settings.catalog.categories.profiles.title'),
	},
	'remote-development': {
		description: terminalSettingsMessages.remoteDevelopmentDescription,
		icon: 'git-branch',
		label: terminalSettingsMessages.remoteDevelopmentCategory,
		title: terminalSettingsMessages.remoteDevelopmentTitle,
	},
	'shell-integration': {
		description: terminalSettingsMessages.shellIntegrationDescription,
		icon: 'terminal',
		label: terminalSettingsMessages.shellIntegrationCategory,
		title: terminalSettingsMessages.shellIntegrationTitle,
	},
	'ssh-connections': {
		description: sshConnectionManagementMessages.description,
		icon: 'terminal',
		label: sshConnectionManagementMessages.category,
		title: sshConnectionManagementMessages.title,
	},
	terminal: {
		description: catalogMessage('terminal.settings.catalog.categories.terminal.description'),
		icon: 'terminal',
		label: catalogMessage('terminal.settings.catalog.categories.terminal.title'),
		title: catalogMessage('terminal.settings.catalog.categories.terminal.title'),
	},
	text: {
		description: catalogMessage('terminal.settings.catalog.categories.text.description'),
		icon: 'edit',
		label: catalogMessage('terminal.settings.catalog.categories.text.title'),
		title: catalogMessage('terminal.settings.catalog.categories.text.title'),
	},
	'windows-tabs': {
		description: catalogMessage('terminal.settings.catalog.categories.windowsTabs.description'),
		icon: 'tabs',
		label: catalogMessage('terminal.settings.catalog.categories.windowsTabs.title'),
		title: catalogMessage('terminal.settings.catalog.categories.windowsTabs.title'),
	},
} as const satisfies Readonly<Record<TerminalSettingsCategory, TerminalSettingsCategoryDefinition>>;

export const TERMINAL_SETTINGS_NAVIGATION_GROUPS = [
	{
		categories: ['general', 'appearance'],
		id: 'application',
		label: catalogMessage('terminal.settings.catalog.groups.application'),
	},
	{
		categories: ['text', 'colors', 'cursor-input', 'interaction', 'terminal'],
		id: 'terminal',
		label: catalogMessage('terminal.settings.catalog.groups.terminal'),
	},
	{
		categories: ['profiles', 'keyboard-shortcuts'],
		id: 'workspace',
		label: catalogMessage('terminal.settings.catalog.groups.workspace'),
	},
	{
		categories: [
			'agent-integrations',
			'ssh-connections',
			'shell-integration',
			'remote-development',
		],
		id: 'integrations',
		label: catalogMessage('terminal.settings.catalog.groups.integrations'),
	},
	{
		categories: ['ghostty-migration', 'browser-access'],
		id: 'data-access',
		label: catalogMessage('terminal.settings.catalog.groups.dataAccess'),
	},
] as const satisfies readonly TerminalSettingsNavigationGroupDefinition[];
