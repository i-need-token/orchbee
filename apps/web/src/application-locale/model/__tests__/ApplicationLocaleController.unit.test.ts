import { describe, expect, it, vi } from 'vitest';

import { memoryUserSettings } from '../../../__tests__/memoryUserSettings';
import { ApplicationLocaleController } from '../ApplicationLocaleController';

describe('ApplicationLocaleController', () => {
	it('uses automatic resolution only while the shared preference is System', () => {
		const documentElement = { dir: '', lang: '' };
		const controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['zh-Hans-CN', 'en-US'],
			userSettings: memoryUserSettings(),
		});

		expect(controller.getSnapshot()).toEqual({ effectiveLocale: 'zh-CN', preference: null });
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
	});

	it('restores an explicit shared preference ahead of automatic resolution', () => {
		const documentElement = { dir: '', lang: '' };
		const controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['zh-CN'],
			userSettings: memoryUserSettings({ values: { 'general.language': 'en-US' } }),
		});

		expect(controller.current()).toBe('en-US');
		expect(controller.getSnapshot().preference).toBe('en-US');
		expect(documentElement.lang).toBe('en-US');
	});

	it('applies a confirmed selection live and updates document metadata', async () => {
		const documentElement = { dir: '', lang: '' };
		const userSettings = memoryUserSettings();
		const controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['en-US'],
			userSettings,
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(controller.select('zh-CN')).toEqual({ kind: 'pending' });
		expect(controller.current()).toBe('en-US');
		await Promise.resolve();
		expect(userSettings.getSnapshot().snapshot?.values['general.language']).toBe('zh-CN');
		expect(controller.current()).toBe('zh-CN');
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(listener).toHaveBeenCalled();
	});

	it('rejects unsupported input and all changes after disposal', () => {
		const controller = new ApplicationLocaleController({
			documentElement: { dir: '', lang: '' },
			preferredLocales: ['en-US'],
			userSettings: memoryUserSettings(),
		});
		expect(controller.select('fr-FR')).toEqual({ kind: 'invalid' });

		controller.dispose();
		expect(controller.select('zh-CN')).toEqual({ kind: 'disposed' });
		expect(controller.current()).toBe('en-US');
	});
});
