import { describe, expect, it, vi } from 'vitest';
import { parseAppearance, readAppearance, writeAppearance } from '../site-preferences-storage';

describe('website appearance preference', () => {
	it('starts with the system without writing a default record', () => {
		const storage = {
			getItem: vi.fn<(key: string) => string | null>(() => null),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};
		expect(readAppearance(storage)).toBe('system');
		expect(storage.getItem.mock.calls).toHaveLength(1);
		expect(storage.setItem).not.toHaveBeenCalled();
	});
	it.each(['system', 'light', 'dark'] as const)(
		'round-trips an explicit %s choice',
		(appearance) => {
			const data = new Map<string, string>();
			const storage = {
				getItem: (key: string) => data.get(key) ?? null,
				setItem: (key: string, value: string) => {
					data.set(key, value);
				},
			};
			writeAppearance(storage, appearance);
			expect(readAppearance(storage)).toBe(appearance);
			expect([...data.keys()]).toEqual([['orchbee', 'website', 'appearance', 'v1'].join(':')]);
			expect([...data.values()]).toEqual([JSON.stringify({ appearance })]);
		},
	);
	it.each([
		'{',
		'null',
		'"dark"',
		'{"appearance":"invalid"}',
		'{"appearance":"dark","extra":true}',
	])('rejects invalid stored data: %s', (value) => {
		expect(
			readAppearance({
				getItem: () => value,
				setItem: vi.fn<(key: string, value: string) => void>(),
			}),
		).toBe('system');
	});
	it('tolerates denied storage reads and writes', () => {
		const denied = {
			getItem: () => {
				throw new Error('Denied');
			},
			setItem: () => {
				throw new Error('Quota exceeded');
			},
		};
		expect(readAppearance(denied)).toBe('system');
		expect(() => writeAppearance(denied, 'dark')).not.toThrow();
	});
	it('validates control input independently of storage', () => {
		expect(parseAppearance('light')).toBe('light');
		expect(parseAppearance('unknown')).toBe('system');
		expect(parseAppearance(null)).toBe('system');
	});
});
