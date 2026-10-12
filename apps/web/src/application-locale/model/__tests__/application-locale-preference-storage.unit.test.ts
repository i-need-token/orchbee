import { describe, expect, it, vi } from 'vitest';

import {
	readApplicationLocalePreference,
	writeApplicationLocalePreference,
	type ApplicationLocaleStorage,
} from '../application-locale-preference-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:application-locale:v2';

describe('application locale preference storage', () => {
	it('keeps a missing record distinct from an explicit preference', () => {
		expect(readApplicationLocalePreference(createStorage(null))).toEqual({
			issue: null,
			preference: null,
		});
		expect(readApplicationLocalePreference(createStorage('{"locale":"zh-CN"}'))).toEqual({
			issue: null,
			preference: 'zh-CN',
		});
	});

	it.each(['{', '{}', '{"locale":"fr-FR"}', '{"locale":"en-US","future":true}'])(
		'rejects the invalid strict record %s',
		(serialized) => {
			expect(readApplicationLocalePreference(createStorage(serialized))).toEqual({
				issue: 'invalid-record',
				preference: null,
			});
		},
	);

	it('reports read failure without inventing a preference', () => {
		const storage = createStorage(null);
		storage.getItem.mockImplementation(() => {
			throw new Error('injected read failure');
		});

		expect(readApplicationLocalePreference(storage)).toEqual({
			issue: 'read-failed',
			preference: null,
		});
	});

	it('writes only the owned versioned key', () => {
		const storage = createStorage(null);

		expect(writeApplicationLocalePreference(storage, 'en-US')).toBe(true);
		expect(storage.setItem).toHaveBeenCalledWith(
			EXPECTED_STORAGE_KEY,
			JSON.stringify({ locale: 'en-US' }),
		);
	});

	it('reports a failed write', () => {
		const storage = createStorage(null);
		storage.setItem.mockImplementation(() => {
			throw new Error('injected write failure');
		});

		expect(writeApplicationLocalePreference(storage, 'zh-CN')).toBe(false);
	});
});

function createStorage(serialized: string | null): ApplicationLocaleStorage & {
	readonly getItem: ReturnType<typeof vi.fn<ApplicationLocaleStorage['getItem']>>;
	readonly setItem: ReturnType<typeof vi.fn<ApplicationLocaleStorage['setItem']>>;
} {
	return {
		getItem: vi.fn<ApplicationLocaleStorage['getItem']>(() => serialized),
		setItem: vi.fn<ApplicationLocaleStorage['setItem']>(() => undefined),
	};
}
