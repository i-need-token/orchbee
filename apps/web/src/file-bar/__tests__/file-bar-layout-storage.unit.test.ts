import { describe, expect, it, vi } from 'vitest';

import { DEFAULT_FILE_BAR_LAYOUT, type FileBarLayout } from '../file-bar-layout';
import { readFileBarLayout, writeFileBarLayout } from '../file-bar-layout-storage';

const STORED_LAYOUT: FileBarLayout = { activeView: 'source-control', visible: true, width: 360 };

function storageHolding(value: string | null) {
	let storedValue = value;
	return {
		getItem: () => storedValue,
		setItem: vi.fn<(key: string, value: string) => void>((_key, nextValue) => {
			storedValue = nextValue;
		}),
	};
}

describe('File Bar layout storage', () => {
	it.each([
		null,
		'not-json',
		'null',
		'360',
		'{"activeView":"explorer","visible":true}',
		'{"activeView":"explorer","visible":true,"width":360,"extra":true}',
		'{"activeView":"timeline","visible":true,"width":360}',
		'{"activeView":"explorer","visible":"yes","width":360}',
		'{"activeView":"explorer","visible":true,"width":0}',
		'{"activeView":"explorer","visible":true,"width":100000}',
		'{"activeView":"explorer","visible":true,"width":300.5}',
	])('restores the defaults from %s', (storedValue) => {
		expect(readFileBarLayout(storageHolding(storedValue))).toEqual(DEFAULT_FILE_BAR_LAYOUT);
	});

	it('restores a layout it wrote under the canonical key', () => {
		const storage = storageHolding(null);

		writeFileBarLayout(storage, STORED_LAYOUT);

		expect(storage.setItem).toHaveBeenCalledWith(
			'orchbee:file-bar:layout:v1',
			'{"activeView":"source-control","visible":true,"width":360}',
		);
		expect(readFileBarLayout(storage)).toEqual(STORED_LAYOUT);
	});

	it.each([Number.NaN, Number.POSITIVE_INFINITY, 0, 100_000, 300.5])(
		'keeps the stored layout when asked to write the width %s',
		(width) => {
			const storage = storageHolding(JSON.stringify(STORED_LAYOUT));

			writeFileBarLayout(storage, { ...DEFAULT_FILE_BAR_LAYOUT, width });

			expect(storage.setItem).not.toHaveBeenCalled();
			expect(readFileBarLayout(storage)).toEqual(STORED_LAYOUT);
		},
	);

	it('falls back without throwing when storage is unavailable', () => {
		const unavailableStorage = {
			getItem: () => {
				throw new Error('Storage is unavailable');
			},
			setItem: () => {
				throw new Error('Storage is unavailable');
			},
		};

		expect(readFileBarLayout(unavailableStorage)).toEqual(DEFAULT_FILE_BAR_LAYOUT);
		expect(readFileBarLayout(undefined)).toEqual(DEFAULT_FILE_BAR_LAYOUT);
		expect(() => writeFileBarLayout(unavailableStorage, STORED_LAYOUT)).not.toThrow();
	});
});
