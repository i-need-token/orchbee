import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { TERMINAL_SETTINGS_NAVIGATION_GROUPS } from '../../constants/terminal-settings-information-architecture';
import { buildTerminalSettingsSearchCatalog } from '../buildTerminalSettingsSearchCatalog';

describe('buildTerminalSettingsSearchCatalog', () => {
	it('builds one stable global catalog from available categories and settings', () => {
		const intl = createOrchbeeIntl('en-US');
		const enabledCategories = TERMINAL_SETTINGS_NAVIGATION_GROUPS.flatMap(
			(group) => group.categories,
		);
		const entries = buildTerminalSettingsSearchCatalog({
			enabledCapabilities: ['macos-option-as-alt'],
			enabledCategories,
			formatMessage: (descriptor, values) => intl.formatMessage(descriptor, values),
			keyboardShortcutChords: { 'workbench.search.all.open': '⌘ ⇧ K' },
		});

		expect(entries.length).toBeGreaterThan(100);
		expect(new Set(entries.map((entry) => entry.id)).size).toBe(entries.length);
		expect(entries.find((entry) => entry.id === 'category:text')).toMatchObject({
			kind: 'category',
			label: 'Text',
			targetId: null,
		});
		expect(entries.find((entry) => entry.id === 'text.font-family')).toMatchObject({
			category: 'text',
			label: 'Terminal font family',
			targetId: 'text.font-family',
		});
		expect(entries.find((entry) => entry.id === 'text.ligatures')).toMatchObject({
			category: 'text',
			kind: 'setting',
			sectionLabel: 'Text shaping',
		});
		expect(
			entries.find((entry) => entry.id === 'keyboard-shortcuts.workbench.search.all.open')
				?.keywords,
		).toContain('⌘ ⇧ K');
		expect(
			entries.find((entry) => entry.id === 'keyboard-shortcuts.workbench.tab.select.2'),
		).toMatchObject({
			label: 'Select tab 2',
			sectionLabel: 'Workbench',
		});
		expect(
			entries.find((entry) => entry.id === 'keyboard-shortcuts.workbench.tab.select-next'),
		).toMatchObject({
			label: 'Select next tab',
			sectionLabel: 'Workbench',
		});
		expect(
			entries.find((entry) => entry.id === 'keyboard-shortcuts.workbench.tab.active.move-forward'),
		).toMatchObject({
			label: 'Move active tab forward',
			sectionLabel: 'Workbench',
		});
		expect(
			entries.find((entry) => entry.id === 'keyboard-shortcuts.workbench.panel.active.equalize'),
		).toMatchObject({
			label: 'Equalize panels',
			sectionLabel: 'Active panel',
		});
		expect(
			entries.find((entry) => entry.id === 'keyboard-shortcuts.workbench.panel.focus.10'),
		).toMatchObject({
			label: 'Focus panel 10',
			sectionLabel: 'Active panel',
		});
	});

	it('omits unavailable categories and capability-gated settings', () => {
		const intl = createOrchbeeIntl('en-US');
		const entries = buildTerminalSettingsSearchCatalog({
			enabledCapabilities: [],
			enabledCategories: ['cursor-input', 'general'],
			formatMessage: (descriptor, values) => intl.formatMessage(descriptor, values),
			keyboardShortcutChords: {},
		});

		expect(entries.some((entry) => entry.id === 'cursor-input.macos-option-as-alt')).toBe(false);
		expect(entries.map((entry) => entry.id)).toEqual([
			'category:general',
			'general.language',
			'general.reset',
			'category:cursor-input',
		]);
	});

	it('indexes scrollback only when Desktop owns the setting', () => {
		const intl = createOrchbeeIntl('en-US');
		const buildCatalog = (
			enabledCapabilities: readonly ['desktop-terminal-default-settings'] | [],
		) =>
			buildTerminalSettingsSearchCatalog({
				enabledCapabilities,
				enabledCategories: ['terminal'],
				formatMessage: (descriptor, values) => intl.formatMessage(descriptor, values),
				keyboardShortcutChords: {},
			});

		expect(buildCatalog([]).some((entry) => entry.id === 'terminal.scrollback')).toBe(false);
		expect(
			buildCatalog(['desktop-terminal-default-settings']).filter(
				(entry) => entry.id === 'terminal.scrollback',
			),
		).toHaveLength(1);
	});

	it('exposes the Agent tail-row setting only with terminal interaction control', () => {
		const intl = createOrchbeeIntl('en-US');
		const buildCatalog = (enabledCapabilities: readonly ['terminal-interaction'] | []) =>
			buildTerminalSettingsSearchCatalog({
				enabledCapabilities,
				enabledCategories: ['terminal'],
				formatMessage: (descriptor, values) => intl.formatMessage(descriptor, values),
				keyboardShortcutChords: {},
			});

		expect(
			buildCatalog([]).some((entry) => entry.id === 'terminal.trimTrailingBlankRowsForAgents'),
		).toBe(false);
		expect(
			buildCatalog(['terminal-interaction']).find(
				(entry) => entry.id === 'terminal.trimTrailingBlankRowsForAgents',
			),
		).toMatchObject({
			label: 'Trim trailing blank rows for CLI agents',
			sectionLabel: 'History',
		});
	});

	it('formats category, section, and setting copy in the effective locale', () => {
		const intl = createOrchbeeIntl('zh-CN');
		const entries = buildTerminalSettingsSearchCatalog({
			enabledCapabilities: ['terminal-interaction'],
			enabledCategories: ['interaction'],
			formatMessage: (descriptor, values) => intl.formatMessage(descriptor, values),
			keyboardShortcutChords: {},
		});

		expect(entries.find((entry) => entry.id === 'category:interaction')?.label).toBe('交互');
		expect(entries.find((entry) => entry.id === 'interaction.copyOnSelect')).toMatchObject({
			label: '选中时复制',
			sectionLabel: '选择与剪贴板',
		});
	});
});
