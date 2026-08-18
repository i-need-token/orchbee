import { terminalSettingsCatalogMessages, type MessageDescriptor } from '@orchbee/i18n';

import type {
	PlannedTerminalSettingControlKind,
	TerminalSettingsCategory,
} from '../types/terminal-settings-surface';

export interface PlannedTerminalSettingDefinition {
	readonly control: PlannedTerminalSettingControlKind;
	readonly description: MessageDescriptor;
	readonly id: string;
	readonly label: MessageDescriptor;
}

export interface PlannedTerminalSettingSectionDefinition {
	readonly id: string;
	readonly items: readonly PlannedTerminalSettingDefinition[];
	readonly title: MessageDescriptor;
}

type PlannedTerminalSettingsCategory = Extract<
	TerminalSettingsCategory,
	| 'appearance'
	| 'colors'
	| 'cursor-input'
	| 'general'
	| 'interaction'
	| 'profiles'
	| 'terminal'
	| 'text'
	| 'windows-tabs'
>;

export const PLANNED_TERMINAL_SETTINGS = {
	appearance: [
		section('appearance.layout', 'terminal.settings.catalog.sections.appearance.layout', [
			item('appearance.density', 'appearance.density', 'select'),
			item('appearance.terminalPadding', 'appearance.terminalPadding', 'number'),
			item('appearance.sidebarPosition', 'appearance.sidebarPosition', 'select'),
		]),
		section('appearance.window', 'terminal.settings.catalog.sections.appearance.window', [
			item('appearance.decorations', 'appearance.decorations', 'select'),
			item('appearance.opacityBlur', 'appearance.opacityBlur', 'select'),
			item('appearance.backgroundImage', 'appearance.backgroundImage', 'action'),
			item('appearance.rememberGeometry', 'appearance.rememberGeometry', 'toggle'),
			item('appearance.windowLevelStartMode', 'appearance.windowLevelStartMode', 'select'),
		]),
	],
	colors: [
		section('colors.palette', 'terminal.settings.catalog.sections.colors.palette', [
			item('colors.ansiPalette', 'colors.ansiPalette', 'action'),
			item('colors.indexedColors', 'colors.indexedColors', 'action'),
		]),
		section('colors.semantic', 'terminal.settings.catalog.sections.colors.semantic', [
			item('colors.selection', 'colors.selection', 'color'),
			item('colors.cursor', 'colors.cursor', 'color'),
			item('colors.linksSearch', 'colors.linksSearch', 'color'),
		]),
		section('colors.readability', 'terminal.settings.catalog.sections.colors.readability', [
			item('colors.contrast', 'colors.contrast', 'number'),
			item('colors.boldDim', 'colors.boldDim', 'select'),
			item('colors.inactiveCellOpacity', 'colors.inactiveCellOpacity', 'select'),
		]),
	],
	'cursor-input': [
		section('cursor-input.cursor', 'terminal.settings.catalog.sections.cursorInput.cursor', [
			item('cursor-input.shape', 'cursorInput.shape', 'select'),
			item('cursor-input.blink', 'cursorInput.blink', 'toggle'),
			item('cursor-input.blinkTiming', 'cursorInput.blinkTiming', 'number'),
			item('cursor-input.thickness', 'cursorInput.thickness', 'number'),
			item('cursor-input.unfocused', 'cursorInput.unfocused', 'select'),
		]),
		section('cursor-input.keyboard', 'terminal.settings.catalog.sections.cursorInput.keyboard', [
			item('cursor-input.altGrMeta', 'cursorInput.altGrMeta', 'select'),
			item('cursor-input.ime', 'cursorInput.ime', 'select'),
			item('cursor-input.keyboardProtocolErase', 'cursorInput.keyboardProtocolErase', 'select'),
		]),
	],
	general: [
		section('general.startup', 'terminal.settings.catalog.sections.general.startup', [
			item('general.startupBehavior', 'general.startupBehavior', 'select'),
			item('general.restoreSessions', 'general.restoreSessions', 'toggle'),
			item('general.workingDirectory', 'general.workingDirectory', 'text'),
		]),
		section('general.defaults', 'terminal.settings.catalog.sections.general.defaults', [
			item('general.defaultProfile', 'general.defaultProfile', 'select'),
			item('general.defaultTerminal', 'general.defaultTerminal', 'action'),
			item('general.language', 'general.language', 'select'),
		]),
		section('general.lifecycle', 'terminal.settings.catalog.sections.general.lifecycle', [
			item('general.autoUpdates', 'general.autoUpdates', 'select'),
			item('general.quitConfirmation', 'general.quitConfirmation', 'toggle'),
		]),
	],
	interaction: [
		section('interaction.clipboard', 'terminal.settings.catalog.sections.interaction.clipboard', [
			item('interaction.copyOnSelect', 'interaction.copyOnSelect', 'toggle'),
			item('interaction.copyFormatTrim', 'interaction.copyFormatTrim', 'select'),
			item('interaction.wordSelection', 'interaction.wordSelection', 'text'),
		]),
		section('interaction.mouse', 'terminal.settings.catalog.sections.interaction.mouse', [
			item('interaction.mouseBindings', 'interaction.mouseBindings', 'action'),
			item('interaction.hidePointer', 'interaction.hidePointer', 'toggle'),
			item('interaction.scrollSpeed', 'interaction.scrollSpeed', 'number'),
			item('interaction.alternateScroll', 'interaction.alternateScroll', 'toggle'),
		]),
		section(
			'interaction.paste-links',
			'terminal.settings.catalog.sections.interaction.pasteLinks',
			[
				item('interaction.pasteProtection', 'interaction.pasteProtection', 'select'),
				item('interaction.osc52', 'interaction.osc52', 'select'),
				item('interaction.linksDrop', 'interaction.linksDrop', 'select'),
			],
		),
	],
	profiles: [
		section('profiles.management', 'terminal.settings.catalog.sections.profiles.management', [
			item('profiles.manageDefault', 'profiles.manageDefault', 'action'),
			item('profiles.identity', 'profiles.identity', 'action'),
		]),
		section('profiles.behavior', 'terminal.settings.catalog.sections.profiles.behavior', [
			item('profiles.launch', 'profiles.launch', 'action'),
			item('profiles.appearance', 'profiles.appearance', 'action'),
			item('profiles.connections', 'profiles.connections', 'action'),
		]),
		section('profiles.automation', 'terminal.settings.catalog.sections.profiles.automation', [
			item('profiles.autoSwitch', 'profiles.autoSwitch', 'toggle'),
			item('profiles.importExport', 'profiles.importExport', 'action'),
		]),
	],
	terminal: [
		section('terminal.launch', 'terminal.settings.catalog.sections.terminal.launch', [
			item('terminal.shell', 'terminal.shell', 'text'),
			item('terminal.argsEnv', 'terminal.argsEnv', 'action'),
			item('terminal.cwdLogin', 'terminal.cwdLogin', 'select'),
		]),
		section('terminal.emulation', 'terminal.settings.catalog.sections.terminal.emulation', [
			item('terminal.termEncoding', 'terminal.termEncoding', 'select'),
			item('terminal.unicodeNormalization', 'terminal.unicodeNormalization', 'select'),
		]),
		section('terminal.history', 'terminal.settings.catalog.sections.terminal.history', [
			item('terminal.scrollback', 'terminal.scrollback', 'number'),
			item('terminal.scrollbar', 'terminal.scrollbar', 'select'),
			item('terminal.scrollPolicy', 'terminal.scrollPolicy', 'select'),
			item('terminal.reflow', 'terminal.reflow', 'toggle'),
		]),
		section('terminal.feedback', 'terminal.settings.catalog.sections.terminal.feedback', [
			item('terminal.bellNotifications', 'terminal.bellNotifications', 'select'),
			item('terminal.exitBehavior', 'terminal.exitBehavior', 'select'),
			item('terminal.inlineImagesTitleProgress', 'terminal.inlineImagesTitleProgress', 'select'),
		]),
	],
	text: [
		section('text.typography', 'terminal.settings.catalog.sections.text.typography', [
			item('text.fallbackFonts', 'text.fallbackFonts', 'text'),
			item('text.weights', 'text.weights', 'select'),
			item('text.lineCellSpacing', 'text.lineCellSpacing', 'number'),
		]),
		section('text.shaping', 'terminal.settings.catalog.sections.text.shaping', [
			item('text.ligatures', 'text.ligatures', 'toggle'),
			item('text.openTypeAxes', 'text.openTypeAxes', 'action'),
		]),
		section('text.rendering', 'terminal.settings.catalog.sections.text.rendering', [
			item('text.unicodeWidth', 'text.unicodeWidth', 'select'),
			item('text.rasterizationEmoji', 'text.rasterizationEmoji', 'select'),
		]),
	],
	'windows-tabs': [
		section('windows-tabs.windows', 'terminal.settings.catalog.sections.windowsTabs.windows', [
			item('windows-tabs.initialSize', 'windowsTabs.initialSize', 'number'),
			item('windows-tabs.rememberGeometry', 'windowsTabs.rememberGeometry', 'toggle'),
			item('windows-tabs.startMode', 'windowsTabs.startMode', 'select'),
			item('windows-tabs.keepAbove', 'windowsTabs.keepAbove', 'toggle'),
		]),
		section('windows-tabs.tabs', 'terminal.settings.catalog.sections.windowsTabs.tabs', [
			item('windows-tabs.tabBar', 'windowsTabs.tabBar', 'select'),
			item('windows-tabs.newTabPlacement', 'windowsTabs.newTabPlacement', 'select'),
			item('windows-tabs.tabStyle', 'windowsTabs.tabStyle', 'select'),
			item('windows-tabs.dynamicTitle', 'windowsTabs.dynamicTitle', 'toggle'),
		]),
		section('windows-tabs.panes', 'terminal.settings.catalog.sections.windowsTabs.panes', [
			item('windows-tabs.paneInheritance', 'windowsTabs.paneInheritance', 'select'),
			item('windows-tabs.closeUndo', 'windowsTabs.closeUndo', 'select'),
		]),
	],
} as const satisfies Readonly<
	Record<PlannedTerminalSettingsCategory, readonly PlannedTerminalSettingSectionDefinition[]>
>;

export function isPlannedTerminalSettingsCategory(
	category: TerminalSettingsCategory,
): category is PlannedTerminalSettingsCategory {
	return category in PLANNED_TERMINAL_SETTINGS;
}

function item(
	id: string,
	messageStem:
		| `appearance.${string}`
		| `colors.${string}`
		| `cursorInput.${string}`
		| `general.${string}`
		| `interaction.${string}`
		| `profiles.${string}`
		| `terminal.${string}`
		| `text.${string}`
		| `windowsTabs.${string}`,
	control: PlannedTerminalSettingControlKind,
): PlannedTerminalSettingDefinition {
	return {
		control,
		description: message(`terminal.settings.catalog.items.${messageStem}.description`),
		id,
		label: message(`terminal.settings.catalog.items.${messageStem}.label`),
	};
}

function section(
	id: string,
	titleId:
		| `terminal.settings.catalog.sections.appearance.${string}`
		| `terminal.settings.catalog.sections.colors.${string}`
		| `terminal.settings.catalog.sections.cursorInput.${string}`
		| `terminal.settings.catalog.sections.general.${string}`
		| `terminal.settings.catalog.sections.interaction.${string}`
		| `terminal.settings.catalog.sections.profiles.${string}`
		| `terminal.settings.catalog.sections.terminal.${string}`
		| `terminal.settings.catalog.sections.text.${string}`
		| `terminal.settings.catalog.sections.windowsTabs.${string}`,
	items: readonly PlannedTerminalSettingDefinition[],
): PlannedTerminalSettingSectionDefinition {
	return { id, items, title: message(titleId) };
}

function message(messageId: string): MessageDescriptor {
	if (!terminalSettingsCatalogMessages.has(messageId)) {
		throw new Error(`Unknown terminal Settings catalog message: ${messageId}`);
	}
	return terminalSettingsCatalogMessages.get(messageId);
}
