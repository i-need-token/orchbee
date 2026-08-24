import { keyboardShortcutMessages, type MessageDescriptor } from '@orchbee/i18n';

import { KEYBOARD_SHORTCUT_CATALOG, type KeyboardShortcutId } from '../../keyboard-shortcuts';
import {
	KEYBOARD_SHORTCUT_SEARCH_AVAILABILITY,
	KEYBOARD_SHORTCUT_SEARCH_LABELS,
	KEYBOARD_SHORTCUT_SEARCH_SCOPES,
	TERMINAL_SETTINGS_SEARCH_ITEMS,
	type TerminalSettingsSearchCapability,
} from '../constants/terminal-settings-search-catalog';
import {
	TERMINAL_SETTINGS_CATEGORIES,
	TERMINAL_SETTINGS_NAVIGATION_GROUPS,
} from '../constants/terminal-settings-information-architecture';
import {
	isPlannedTerminalSettingsCategory,
	PLANNED_TERMINAL_SETTINGS,
} from '../constants/planned-terminal-settings';
import type {
	TerminalSettingsCategory,
	TerminalSettingsSearchEntry,
} from '../types/terminal-settings-surface';

interface BuildTerminalSettingsSearchCatalogOptions {
	readonly enabledCapabilities: readonly TerminalSettingsSearchCapability[];
	readonly enabledCategories: readonly TerminalSettingsCategory[];
	readonly formatMessage: (
		descriptor: MessageDescriptor,
		values?: Readonly<Record<string, number | string>>,
	) => string;
	readonly keyboardShortcutChords: Readonly<Partial<Record<KeyboardShortcutId, string>>>;
}

export function buildTerminalSettingsSearchCatalog({
	enabledCapabilities,
	enabledCategories,
	formatMessage,
	keyboardShortcutChords,
}: BuildTerminalSettingsSearchCatalogOptions): readonly TerminalSettingsSearchEntry[] {
	const capabilitySet = new Set(enabledCapabilities);
	const categorySet = new Set(enabledCategories);
	const entries: TerminalSettingsSearchEntry[] = [];
	const availableSettingIds = new Set<string>();

	for (const navigationGroup of TERMINAL_SETTINGS_NAVIGATION_GROUPS) {
		for (const category of navigationGroup.categories) {
			if (!categorySet.has(category)) continue;
			const categoryDefinition = TERMINAL_SETTINGS_CATEGORIES[category];
			const categoryLabel = formatMessage(categoryDefinition.label);
			entries.push({
				availability: 'available',
				category,
				categoryLabel,
				description: formatMessage(categoryDefinition.description),
				icon: categoryDefinition.icon,
				id: `category:${category}`,
				keywords: [],
				kind: 'category',
				label: categoryLabel,
				sectionLabel: null,
				targetId: null,
			});

			for (const definition of TERMINAL_SETTINGS_SEARCH_ITEMS) {
				if (definition.category !== category) continue;
				if (definition.capability !== undefined && !capabilitySet.has(definition.capability)) {
					continue;
				}
				entries.push({
					availability: 'available',
					category,
					categoryLabel,
					description: formatMessage(definition.description),
					icon: categoryDefinition.icon,
					id: definition.id,
					keywords:
						definition.keywords?.map((keyword) =>
							typeof keyword === 'string' ? keyword : formatMessage(keyword),
						) ?? [],
					kind: 'setting',
					label: formatMessage(definition.label),
					sectionLabel: definition.section === undefined ? null : formatMessage(definition.section),
					targetId: definition.targetId,
				});
				availableSettingIds.add(definition.id);
			}

			if (category === 'keyboard-shortcuts') {
				for (const declaration of KEYBOARD_SHORTCUT_CATALOG) {
					entries.push({
						availability: 'available',
						category,
						categoryLabel,
						description: formatMessage(keyboardShortcutMessages.description),
						icon: categoryDefinition.icon,
						id: `keyboard-shortcuts.${declaration.shortcutId}`,
						keywords: [
							formatMessage(KEYBOARD_SHORTCUT_SEARCH_AVAILABILITY[declaration.availability]),
							keyboardShortcutChords[declaration.shortcutId] ?? '',
						],
						kind: 'setting',
						label: formatMessage(
							KEYBOARD_SHORTCUT_SEARCH_LABELS[declaration.name],
							declaration.name === 'select-tab' ? { position: declaration.position } : undefined,
						),
						sectionLabel: formatMessage(KEYBOARD_SHORTCUT_SEARCH_SCOPES[declaration.scope]),
						targetId: `keyboard-shortcuts.${declaration.shortcutId}`,
					});
				}
			}

			if (!isPlannedTerminalSettingsCategory(category)) continue;
			for (const section of PLANNED_TERMINAL_SETTINGS[category]) {
				const sectionLabel = formatMessage(section.title);
				for (const definition of section.items) {
					if (availableSettingIds.has(definition.id)) continue;
					entries.push({
						availability: 'planned',
						category,
						categoryLabel,
						description: formatMessage(definition.description),
						icon: categoryDefinition.icon,
						id: definition.id,
						keywords: [],
						kind: 'setting',
						label: formatMessage(definition.label),
						sectionLabel,
						targetId: definition.id,
					});
				}
			}
		}
	}

	return entries;
}
