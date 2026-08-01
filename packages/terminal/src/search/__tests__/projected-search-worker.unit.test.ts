import { RowPromptClass } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../projected-search-worker';
import type { ProjectedSearchWorkerResponse } from '../projected-search-protocol';

afterEach(() => vi.unstubAllGlobals());

describe('projected search Worker', () => {
	it('rejects pages that do not belong to the active immutable scan', () => {
		const postMessage = vi.fn<(message: ProjectedSearchWorkerResponse) => void>();
		vi.stubGlobal('postMessage', postMessage);

		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					final: true,
					generation: '1',
					kind: 'page',
					revision: '1',
					rows: [],
					searchId: 7,
				},
			}),
		);

		expect(postMessage).toHaveBeenCalledWith({
			code: 'invalid-sequence',
			kind: 'error',
			message: 'Search page does not match the active scan.',
			searchId: 7,
		});
	});

	it('returns matches only after the final contiguous page', () => {
		const postMessage = vi.fn<(message: ProjectedSearchWorkerResponse) => void>();
		vi.stubGlobal('postMessage', postMessage);
		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					generation: '2',
					kind: 'start',
					options: {},
					query: 'bee',
					revision: '9',
					searchId: 8,
				},
			}),
		);
		globalThis.onmessage?.call(
			globalThis as unknown as Window,
			new MessageEvent('message', {
				data: {
					final: true,
					generation: '2',
					kind: 'page',
					revision: '9',
					rows: [
						{
							cells: null,
							identityAscii: true,
							index: 0n,
							mappings: [],
							promptClass: RowPromptClass.none,
							text: 'orchbee',
							wrap: false,
							wrapContinuation: false,
						},
					],
					searchId: 8,
				},
			}),
		);

		expect(postMessage).toHaveBeenCalledWith(
			expect.objectContaining({
				generation: '2',
				kind: 'result',
				revision: '9',
				searchId: 8,
			}),
		);
	});
});
