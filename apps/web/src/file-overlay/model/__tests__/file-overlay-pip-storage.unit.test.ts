import { describe, expect, it, vi } from 'vitest';

import {
	readFileOverlayPipPosition,
	writeFileOverlayPipPosition,
} from '../file-overlay-pip-storage';
import type { FileOverlayPipPosition } from '../file-overlay-pip';

const STORED_POSITION: FileOverlayPipPosition = { corner: 'top-left', x: 40, y: 12 };

function storageHolding(value: string | null) {
	let storedValue = value;
	return {
		getItem: () => storedValue,
		setItem: vi.fn<(key: string, value: string) => void>((_key, nextValue) => {
			storedValue = nextValue;
		}),
	};
}

describe('picture-in-picture position storage', () => {
	it.each([
		null,
		'not-json',
		'null',
		'{"corner":"top-left","x":40}',
		'{"corner":"top-left","x":40,"y":12,"extra":true}',
		'{"corner":"middle","x":40,"y":12}',
		'{"corner":"top-left","x":-1,"y":12}',
		'{"corner":"top-left","x":40,"y":100001}',
		'{"corner":"top-left","x":40.5,"y":12}',
	])('places picture-in-picture at its first corner from %s', (storedValue) => {
		expect(readFileOverlayPipPosition(storageHolding(storedValue))).toBeNull();
	});

	it('restores a position it wrote under the canonical key', () => {
		const storage = storageHolding(null);

		writeFileOverlayPipPosition(storage, STORED_POSITION);

		expect(storage.setItem).toHaveBeenCalledWith(
			'orchbee:file-overlay:pip-position:v1',
			'{"corner":"top-left","x":40,"y":12}',
		);
		expect(readFileOverlayPipPosition(storage)).toEqual(STORED_POSITION);
	});

	it('keeps the stored position when asked to write one it cannot hold', () => {
		const storage = storageHolding(JSON.stringify(STORED_POSITION));

		writeFileOverlayPipPosition(storage, { corner: 'bottom-right', x: Number.NaN, y: 0 });

		expect(storage.setItem).not.toHaveBeenCalled();
		expect(readFileOverlayPipPosition(storage)).toEqual(STORED_POSITION);
	});

	it('falls back without throwing when storage is unavailable', () => {
		const unavailableStorage = {
			getItem: () => {
				throw new Error('Storage is unavailable');
			},
			setItem: () => {
				throw new Error('Storage is unavailable');
			},
		};

		expect(readFileOverlayPipPosition(unavailableStorage)).toBeNull();
		expect(readFileOverlayPipPosition(undefined)).toBeNull();
		expect(() => writeFileOverlayPipPosition(unavailableStorage, STORED_POSITION)).not.toThrow();
	});
});
