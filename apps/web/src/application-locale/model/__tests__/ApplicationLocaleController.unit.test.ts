import { describe, expect, it, vi } from 'vitest';

import { ApplicationLocaleController } from '../ApplicationLocaleController';
import type { ApplicationLocaleStorage } from '../application-locale-preference-storage';

describe('ApplicationLocaleController', () => {
	it('persists the latest locale after the older storage capability returns', () => {
		let controller: ApplicationLocaleController | undefined;
		let stored = '';
		let nestedResult: unknown;
		let writes = 0;
		const documentElement = { dir: '', lang: '' };
		controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['en-US'],
			storage: {
				getItem: () => null,
				setItem: (_key, value) => {
					writes += 1;
					if (writes === 1) nestedResult = controller?.select('en-US');
					stored = value;
				},
			},
		});

		expect(controller.select('zh-CN')).toEqual({ kind: 'applied-and-saved' });
		expect(stored).toBe('{"locale":"en-US"}');
		expect(controller.current()).toBe('en-US');
		expect(documentElement.lang).toBe('en-US');
		expect(nestedResult).toEqual({ kind: 'applied-session-only' });
		expect(writes).toBe(2);
	});

	it('uses automatic resolution only while an explicit preference is absent', () => {
		const documentElement = { dir: '', lang: '' };
		const storage = createStorage(null);
		const controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['zh-Hans-CN', 'en-US'],
			storage,
		});

		expect(controller.getSnapshot()).toEqual({
			effectiveLocale: 'zh-CN',
			persistenceIssue: null,
			preference: null,
		});
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(storage.setItem).not.toHaveBeenCalled();
	});

	it('restores an explicit preference ahead of automatic resolution', () => {
		const documentElement = { dir: '', lang: '' };
		const controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['zh-CN'],
			storage: createStorage('{"locale":"en-US"}'),
		});

		expect(controller.current()).toBe('en-US');
		expect(controller.getSnapshot().preference).toBe('en-US');
		expect(documentElement.lang).toBe('en-US');
	});

	it('persists a same-as-detected selection as an explicit choice', () => {
		const storage = createStorage(null);
		const controller = new ApplicationLocaleController({
			documentElement: { dir: '', lang: '' },
			preferredLocales: ['zh-CN'],
			storage,
		});

		expect(controller.select('zh-CN')).toEqual({ kind: 'applied-and-saved' });
		expect(controller.getSnapshot().preference).toBe('zh-CN');
		expect(storage.setItem).toHaveBeenCalledWith(
			'orchbee:web:application-locale:v2',
			'{"locale":"zh-CN"}',
		);
	});

	it('publishes a live selection and updates document metadata', () => {
		const documentElement = { dir: '', lang: '' };
		const controller = new ApplicationLocaleController({
			documentElement,
			preferredLocales: ['en-US'],
			storage: createStorage(null),
		});
		const listener = vi.fn<() => void>();
		controller.subscribe(listener);

		expect(controller.select('zh-CN')).toEqual({ kind: 'applied-and-saved' });
		expect(controller.current()).toBe('zh-CN');
		expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
		expect(listener).toHaveBeenCalled();
	});

	it('keeps a failed write effective for the session and retries it', () => {
		const storage = createStorage(null);
		storage.setItem.mockImplementationOnce(() => {
			throw new Error('injected write failure');
		});
		const controller = new ApplicationLocaleController({
			documentElement: { dir: '', lang: '' },
			preferredLocales: ['en-US'],
			storage,
		});

		expect(controller.select('zh-CN')).toEqual({ kind: 'applied-session-only' });
		expect(controller.current()).toBe('zh-CN');
		expect(controller.getSnapshot().persistenceIssue).toBe('write-failed');
		expect(controller.select('zh-CN')).toEqual({ kind: 'applied-and-saved' });
		expect(controller.getSnapshot().persistenceIssue).toBeNull();
	});

	it('uses automatic resolution with an issue after invalid persisted data', () => {
		const controller = new ApplicationLocaleController({
			documentElement: { dir: '', lang: '' },
			preferredLocales: ['zh-CN'],
			storage: createStorage('{"locale":"fr-FR"}'),
		});

		expect(controller.getSnapshot()).toEqual({
			effectiveLocale: 'zh-CN',
			persistenceIssue: 'invalid-record',
			preference: null,
		});
	});

	it('rejects unsupported input and all changes after disposal', () => {
		const controller = new ApplicationLocaleController({
			documentElement: { dir: '', lang: '' },
			preferredLocales: ['en-US'],
			storage: createStorage(null),
		});
		expect(controller.select('fr-FR')).toEqual({ kind: 'invalid' });

		controller.dispose();
		expect(controller.select('zh-CN')).toEqual({ kind: 'disposed' });
		expect(controller.current()).toBe('en-US');
	});
});

function createStorage(serialized: string | null): ApplicationLocaleStorage & {
	readonly setItem: ReturnType<typeof vi.fn<ApplicationLocaleStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<ApplicationLocaleStorage['getItem']>(() => serialized),
		setItem: vi.fn<ApplicationLocaleStorage['setItem']>(() => undefined),
	};
}
