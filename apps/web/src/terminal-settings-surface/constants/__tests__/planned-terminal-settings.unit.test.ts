import { describe, expect, it } from 'vitest';

import {
	isPlannedTerminalSettingsCategory,
	PLANNED_TERMINAL_SETTINGS,
} from '../planned-terminal-settings';

describe('planned terminal Settings catalog', () => {
	it('owns only the categories that expose inert future controls', () => {
		expect(Object.keys(PLANNED_TERMINAL_SETTINGS)).toEqual([
			'appearance',
			'colors',
			'cursor-input',
			'general',
			'interaction',
			'profiles',
			'terminal',
			'text',
			'windows-tabs',
		]);
		expect(isPlannedTerminalSettingsCategory('general')).toBe(true);
		expect(isPlannedTerminalSettingsCategory('keyboard-shortcuts')).toBe(false);
	});

	it('gives every section and row a stable nonempty identity', () => {
		const sections = Object.values(PLANNED_TERMINAL_SETTINGS).flat();
		expect(new Set(sections.map((section) => section.id)).size).toBe(sections.length);
		for (const section of sections) {
			expect(section.id).not.toBe('');
			expect(section.items.length).toBeGreaterThan(0);
		}
	});
});
