import type { MessageDescriptor } from '@orchbee/i18n';

import {
	isPlannedTerminalSettingsCategory,
	PLANNED_TERMINAL_SETTINGS,
} from '../constants/planned-terminal-settings';
import type {
	PlannedTerminalSettingItemView,
	PlannedTerminalSettingSectionView,
	TerminalSettingsCategory,
} from '../types/terminal-settings-surface';

interface ResolvePlannedTerminalSettingsOptions {
	readonly category: TerminalSettingsCategory;
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
}

export function resolvePlannedTerminalSettings({
	category,
	formatMessage,
}: ResolvePlannedTerminalSettingsOptions): readonly PlannedTerminalSettingSectionView[] {
	if (!isPlannedTerminalSettingsCategory(category)) return [];

	const resolvedSections: PlannedTerminalSettingSectionView[] = [];
	for (const section of PLANNED_TERMINAL_SETTINGS[category]) {
		const title = formatMessage(section.title);
		const items: PlannedTerminalSettingItemView[] = section.items.map((definition) => ({
			control: definition.control,
			description: formatMessage(definition.description),
			id: definition.id,
			label: formatMessage(definition.label),
		}));
		resolvedSections.push({ id: section.id, items, title });
	}
	return resolvedSections;
}
