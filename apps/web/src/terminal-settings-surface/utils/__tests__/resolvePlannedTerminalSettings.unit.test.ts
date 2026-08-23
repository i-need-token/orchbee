import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { resolvePlannedTerminalSettings } from '../resolvePlannedTerminalSettings';

describe('resolvePlannedTerminalSettings', () => {
	it('provides a complete, uniquely identified settings catalog', () => {
		const intl = createOrchbeeIntl('en-US');
		const categories = [
			'appearance',
			'colors',
			'cursor-input',
			'general',
			'interaction',
			'profiles',
			'terminal',
			'text',
			'windows-tabs',
		] as const;
		const itemIds = categories.flatMap((category) =>
			resolvePlannedTerminalSettings({
				category,
				formatMessage: (descriptor) => intl.formatMessage(descriptor),
			}).flatMap((section) => section.items.map((item) => item.id)),
		);

		expect(itemIds.length).toBeGreaterThanOrEqual(70);
		expect(new Set(itemIds).size).toBe(itemIds.length);
	});

	it('formats the complete catalog in the effective locale', () => {
		const intl = createOrchbeeIntl('zh-CN');
		const result = resolvePlannedTerminalSettings({
			category: 'interaction',
			formatMessage: (descriptor) => intl.formatMessage(descriptor),
		});

		expect(result).toHaveLength(3);
		expect(result[0]?.title).toBe('选择与剪贴板');
		expect(result[0]?.items.map((item) => item.label)).toContain('复制格式与空白');
	});

	it('removes implemented rows without hiding the remaining planned section', () => {
		const intl = createOrchbeeIntl('en-US');
		const result = resolvePlannedTerminalSettings({
			category: 'terminal',
			formatMessage: (descriptor) => intl.formatMessage(descriptor),
			implementedSettingIds: new Set(['terminal.scrollback']),
		});

		expect(
			result.flatMap((section) => section.items).some((item) => item.id === 'terminal.scrollback'),
		).toBe(false);
		expect(result.flatMap((section) => section.items).length).toBeGreaterThan(0);
	});

	it('returns no planned rows for independently implemented categories', () => {
		const intl = createOrchbeeIntl('en-US');
		expect(
			resolvePlannedTerminalSettings({
				category: 'keyboard-shortcuts',
				formatMessage: (descriptor) => intl.formatMessage(descriptor),
			}),
		).toEqual([]);
	});
});
