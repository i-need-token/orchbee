import { describe, expect, it } from 'vitest';

import { readExplorerExpansion, writeExplorerExpansion } from '../explorer-expansion-storage';

const KEY = 'orchbee:file-explorer:expanded-folders:v1';

function workspaceId(index: number): string {
	return index.toString(16).padStart(32, '0');
}

function memoryStorage(initial: string | null = null): {
	readonly storage: Pick<Storage, 'getItem' | 'setItem'>;
	readonly value: () => string | null;
} {
	let stored = initial;
	return {
		storage: {
			getItem: (key) => (key === KEY ? stored : null),
			setItem: (key, value) => {
				if (key === KEY) stored = value;
			},
		},
		value: () => stored,
	};
}

describe('Explorer expansion storage', () => {
	it("saves each Workspace's folders under the canonical key and restores them", () => {
		const { storage, value } = memoryStorage();
		writeExplorerExpansion(storage, workspaceId(1), ['/w/src', '/w/docs']);
		writeExplorerExpansion(storage, workspaceId(2), ['/x']);

		expect(readExplorerExpansion(storage, workspaceId(1))).toEqual(['/w/src', '/w/docs']);
		expect(readExplorerExpansion(storage, workspaceId(2))).toEqual(['/x']);
		expect(readExplorerExpansion(storage, workspaceId(3))).toEqual([]);
		expect(value()).toBe(
			`{"workspaces":[{"expanded":["/x"],"workspaceId":"${workspaceId(2)}"},{"expanded":["/w/src","/w/docs"],"workspaceId":"${workspaceId(1)}"}]}`,
		);
	});

	it('restores nothing from a record outside its shape, and survives storage that fails', () => {
		const id = workspaceId(1);
		for (const stored of [
			'not-json',
			'null',
			`{"workspaces":[{"expanded":["/w"],"workspaceId":"${id}"}],"extra":true}`,
			`{"workspaces":[{"expanded":[""],"workspaceId":"${id}"}]}`,
			`{"workspaces":[{"expanded":["/w"],"workspaceId":"W1"}]}`,
		]) {
			expect(readExplorerExpansion(memoryStorage(stored).storage, id)).toEqual([]);
		}
		const failing = {
			getItem: (): string | null => {
				throw new Error('Storage is unavailable');
			},
			setItem: (): void => {
				throw new Error('Quota exceeded');
			},
		};
		expect(() => writeExplorerExpansion(failing, id, ['/w'])).not.toThrow();
		expect(readExplorerExpansion(failing, id)).toEqual([]);
	});

	it('keeps the most recently changed Workspaces and folders within its bounds', () => {
		const { storage, value } = memoryStorage();
		const folders = (parent: string, count: number): string[] =>
			Array.from({ length: count }, (_, index) => `/${parent}/${'f'.repeat(300)}${index}`);
		for (let index = 0; index < 51; index += 1) {
			writeExplorerExpansion(storage, workspaceId(index), ['/w']);
		}
		expect(readExplorerExpansion(storage, workspaceId(0))).toEqual([]);
		expect(readExplorerExpansion(storage, workspaceId(1))).toEqual(['/w']);

		// Two long Workspaces do not fit together, and one alone keeps its newest folders.
		writeExplorerExpansion(storage, workspaceId(60), folders('a', 1000));
		writeExplorerExpansion(storage, workspaceId(61), folders('b', 2001));
		expect(readExplorerExpansion(storage, workspaceId(61))).toEqual(
			folders('b', 2001).slice(-1000),
		);
		expect(readExplorerExpansion(storage, workspaceId(60))).toEqual([]);
		expect(value()?.length).toBeLessThanOrEqual(500_000);
	});
});
